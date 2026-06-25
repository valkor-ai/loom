import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument, stateNotInitialized } from "../errors";
import {
  type ManualReviewResolution,
  type ReviewResult,
  type RepairRequest,
  manualReviewResolutionSchema,
  reviewResultSchema,
  repairRequestSchema,
  taskPlanSchema,
} from "../contracts";
import { ensureDir, pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import { loadDeliveryIndex, resolveLocator } from "../state/delivery";
import {
  type DeliveryPhaseLocator,
  architectureRequestPath,
  repairCandidatePath,
  repairRequestPath,
  repositoryContextPath,
  reviewLatestPath,
  reviewResultPath,
  taskPlanGroupCandidatePath,
  taskPlanOutlineCandidatePath,
  taskPlanRequestPath,
  taskPlanPath,
  taskPlanRunPath,
  taskExecutionRequestPath,
  taskExecutionResultCandidatePath,
  toProjectRelative,
} from "../state/paths";
import {
  closeOperationLease,
  createOperationLease,
  operationRef,
  updateRouteState,
} from "./control";
import { repairSubmitRouting } from "./repair-routing";
import { loadCurrentTaskPlanRun, materializeExecutionRepairTask, sourceEditPreparationContract, verificationCommandSchedulingRules } from "./tasks";
import { artifactGenerationProtocolPolicy, artifactInstructionPolicy, compactContextReadStep } from "./output-policy";
import { withAutoRunnableTransition } from "./routing-instructions";
import { agentActionContract } from "./agent-action";
import { referencedArtifactReadGuide } from "./artifact-read-guide";
import { hydrateRequestManifest, writeRequestManifestAtomic } from "./request-manifest";
import { getDeploymentRepairPaths } from "../deployment/paths";
import {
  readDeployExecutionRepairRequest,
  readDeployExecutionRepairTaskResult,
  readDeploymentFailureReport,
  writeDeployExecutionRepairRequest,
} from "../deployment/state";
import type { DeployExecutionRepairRequest, DeploymentFailureReport } from "../deployment/types";

export type CreateRepairRequestInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  type: "execution" | "task-result" | "taskplan" | "architecture";
};

export async function createRepairRequest(input: CreateRepairRequestInput): Promise<{
  operation: "repair_request_created";
  deliveryId: string;
  phaseId: string;
  repairRequestId: string;
  repairType: RepairRequest["repairType"];
  requestRef: string;
  candidateFile: string | null;
  lease: ReturnType<typeof operationRef> | null;
  request: RepairRequest;
  instruction?: Record<string, unknown>;
  materializedNextTask?: Awaited<ReturnType<typeof materializeExecutionRepairTask>>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const repairType = normalizeRepairType(input.type);
  const repairRequestId = createId("repair-req");
  const candidateFile = shouldProduceCandidate(repairType)
    ? toProjectRelative(root, repairCandidatePath(root, locator, repairRequestId))
    : null;
  const inputs = await inputsFor(root, locator, repairType);
  const repairContext = await repairContextFor(root, locator, repairType);
  const repositoryContextRef = toProjectRelative(root, repositoryContextPath(root, locator));
  const outputContract = outputContractFor(root, repairType, locator, candidateFile, repairRequestId, repairContext);
  const submitCommand = concreteOutputSubmitCommand(outputContract, candidateFile, repairRequestId);
  const agentAction = agentActionForRepairRequest(repairType, outputContract, candidateFile, repairRequestId, submitCommand);
  const request = repairRequestSchema.parse({
    schemaVersion: "1.0",
    repairRequestId,
    ...(agentAction ? { agentAction } : {}),
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    repairType,
    source: {
      nextActionType: repairType,
      trigger: "route_decision",
      ...(repairType === "task_result_repair" ? {
        sourceTaskId: firstStringAt(inputs, ["sourceFacts", 0, "sourceTaskId"]) ?? null,
        originalTaskExecutionRequestRef: repairContext.requestRef,
        invalidTaskResultFile: repairContext.requestId
          ? toProjectRelative(root, taskExecutionResultCandidatePath(root, locator, repairContext.requestId))
          : null,
      } : {}),
    },
    scope: {
      currentPhaseOnly: true,
      allowFuturePhaseImplementation: false,
      allowContractChanges: repairType !== "execution_repair" && repairType !== "task_result_repair",
    },
    workspaceContext: {
      repositoryContextRef,
      changeContextSnapshotRef: null,
    },
    inputs,
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      repositoryContextRef,
      originalRequestRef: repairContext.requestRef,
      reviewResultRef: firstStringAt(inputs, ["sourceFacts", 0, "reviewResultRef"]),
      taskPlanRef: firstStringAt(inputs, ["taskPlanRef"]),
      taskPlanRunRef: firstStringAt(inputs, ["taskPlanRunRef"]),
    }),
    repairRules: repairRulesFor(repairType),
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: repairType !== "execution_repair",
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: repairType !== "execution_repair",
      modifyProjectFilesAllowed: repairType === "execution_repair",
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
    },
    enumRefs: {
      repairType: ["execution_repair", "task_result_repair", "taskplan_repair", "architecture_artifact_repair"],
      taskResultStatus: ["completed", "completed_with_notes", "blocked", "failed"],
    },
    outputContract,
    resumePolicy: resumePolicyFor(repairType),
    operation: {
      operationId: createId("op"),
      progressSignal: repairType === "execution_repair" ? "project_files_and_result_file" : "candidate_file",
      resumeCommand: {
        name: "continue",
        argv: ["continue"],
      },
    },
    createdAt: new Date().toISOString(),
  });
  const requestFile = repairRequestPath(root, locator, repairType, repairRequestId);
  let lease: Awaited<ReturnType<typeof createOperationLease>> | null = null;
  if (repairType !== "execution_repair") {
    lease = await createOperationLease({
      projectRoot: root,
      locator,
      operationType: repairType,
      refs: {
        requestRef: toProjectRelative(root, requestFile),
        candidateFile,
      },
    });
  }
  try {
    await writeRequestManifestAtomic(root, requestFile, request);
  } catch (error) {
    if (lease) {
      await closeOperationLease({
        projectRoot: root,
        locator,
        operationType: repairType,
        reason: "request_write_failed",
      });
    }
    throw error;
  }
  const materializedNextTask = repairType === "execution_repair"
    ? await materializeExecutionRepairTask({
        projectRoot: root,
        locator,
        repairRequestId,
        targetTaskIds: Array.isArray(inputs.targetTaskIds)
          ? inputs.targetTaskIds.filter((taskId): taskId is string => typeof taskId === "string")
          : [],
      })
    : undefined;
  await updateRouteState({
    projectRoot: root,
    locator,
    deliveryStatus: "repairing",
    phaseStatus: "repairing",
    latestRefs: {
      repairRequestId,
      repairRequest: toProjectRelative(root, requestFile),
    },
    nextAction: materializedNextTask?.executionRequestPath
      ? {
        type: "continue_execution",
        source: "repair_request",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: materializedNextTask.executionRequestPath,
        reason: "EXECUTION_REPAIR_REQUEST_CREATED",
        refs: {
          repairRequestRef: toProjectRelative(root, requestFile),
          executionRequestRef: materializedNextTask.executionRequestPath,
          resultFile: materializedNextTask.executionRequest?.resultFile ?? null,
          taskId: materializedNextTask.task?.taskId ?? null,
          groupId: materializedNextTask.executionRequest?.groupId ?? materializedNextTask.task?.groupId ?? null,
          taskPlanRunId: materializedNextTask.executionRequest?.taskPlanRunId ?? null,
          activeOperationType: "task_execution",
        },
      }
      : {
        type: repairType,
        source: "repair_request",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: toProjectRelative(root, requestFile),
        reason: "REPAIR_REQUEST_CREATED",
        refs: {
          requestRef: toProjectRelative(root, requestFile),
          candidateFile,
          activeOperationType: repairType,
        },
      },
  });
  return {
    operation: "repair_request_created",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    repairRequestId,
    repairType,
    requestRef: toProjectRelative(root, requestFile),
    candidateFile,
    lease: lease ? operationRef(lease) : null,
    request,
    ...(materializedNextTask ? {
      materializedNextTask,
      instruction: materializedNextTask.instruction
        ? withAutoRunnableTransition({
          ...materializedNextTask.instruction,
          routingRule: "Execution repair has reopened the target task and created a TaskExecutionRequest. Execute it now; do not run repair request, loom continue, or send a progress-only summary before submitting its TaskResult.",
          userMessage: "Execution repair request created. Target task is reopened; execute the TaskExecutionRequest now and submit TaskResult before any interim summary.",
          }, {
            sourceCommand: "repair request",
            sourceSummary: "Execution repair request reopened the target task and created a TaskExecutionRequest.",
            primaryAction: "execute_reopened_task_request",
            mustStartImmediately: true,
          })
        : undefined,
    } : {
      instruction: repairRequestGenerationInstruction({
        repairType,
        requestRef: toProjectRelative(root, requestFile),
        candidateFile,
        request,
      }),
    }),
  };
}

export async function createDeployExecutionRepairRequest(input: {
  projectRoot: string;
  failureRef: string;
}): Promise<{
  operation: "deploy_execution_repair_request_created";
  repairId: string;
  requestRef: string;
  executionRequestRef: string;
  resultFile: string;
  failureRef: string;
  request: DeployExecutionRepairRequest;
  instruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const failure = await readDeploymentFailureReport(root, input.failureRef);
  if (!failure) {
    throw invalidArgument("Deployment failure report does not exist.", { failureRef: input.failureRef });
  }
  if (failure.source !== "deploy" || failure.repairRoute !== "execution_repair") {
    throw invalidArgument("Deployment failure report is not routable to execution repair.", {
      failureRef: input.failureRef,
      repairRoute: failure.repairRoute,
      failureOwner: failure.failureOwner,
    });
  }
  if (failure.loopGuard.attempt > failure.loopGuard.maxAttempts) {
    throw invalidArgument("Deployment execution repair loop guard exceeded.", {
      failureRef: input.failureRef,
      signature: failure.loopGuard.signature,
      attempt: failure.loopGuard.attempt,
      maxAttempts: failure.loopGuard.maxAttempts,
    });
  }
  const repairId = createId("deploy-exec-repair");
  const paths = getDeploymentRepairPaths(root, repairId);
  const requestRef = toProjectRelative(root, paths.requestFile);
  const executionRequestRef = toProjectRelative(root, paths.taskExecutionRequestFile);
  const resultFile = toProjectRelative(root, paths.resultFile);
  const failureRef = toProjectRelative(root, path.resolve(root, input.failureRef));
  const request = deployExecutionRepairRequestFor({
    repairId,
    failure,
    failureRef,
    resultFile,
  });
  await writeDeployExecutionRepairRequest(root, request);
  await ensureDir(path.dirname(paths.resultFile));
  await writeRequestManifestAtomic(root, paths.taskExecutionRequestFile, deployTaskExecutionRequestFor({
    request,
    requestRef,
    executionRequestRef,
  }));

  return {
    operation: "deploy_execution_repair_request_created",
    repairId,
    requestRef,
    executionRequestRef,
    resultFile,
    failureRef,
    request,
    instruction: deployExecutionRepairInstruction(requestRef, resultFile, request),
  };
}

export async function submitDeployExecutionRepairResult(input: {
  projectRoot: string;
  repairId: string;
  resultFile: string;
}): Promise<{
  accepted: boolean;
  status: "accepted" | "not_accepted";
  repairId: string;
  resultFile: string;
  nextAction: "deploy_retry" | "manual_review";
  instruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const request = await readDeployExecutionRepairRequest(root, input.repairId);
  const result = await readDeployExecutionRepairTaskResult(root, input.resultFile, request);
  if (result.repairId !== request.repairId) {
    throw invalidArgument("Deploy execution repair result repairId does not match request.", {
      expected: request.repairId,
      actual: result.repairId,
    });
  }
  if (result.deploymentFailureRef !== request.deploymentFailureRef) {
    throw invalidArgument("Deploy execution repair result deploymentFailureRef does not match request.", {
      expected: request.deploymentFailureRef,
      actual: result.deploymentFailureRef,
    });
  }
  const forbidden = result.changedFiles.filter((file) =>
    file === ".loom" ||
    file.startsWith(".loom/") ||
    request.syntheticTask.writeBoundary.forbiddenPaths.some((blocked) => file === blocked || file.startsWith(`${blocked.replace(/\/+$/, "")}/`))
  );
  if (forbidden.length > 0) {
    throw invalidArgument("Deploy execution repair result changedFiles includes forbidden paths.", {
      forbidden,
      forbiddenPaths: request.syntheticTask.writeBoundary.forbiddenPaths,
    });
  }
  if ((result.status === "completed" || result.status === "completed_with_notes") && result.changedFiles.length === 0) {
    throw invalidArgument("Deploy execution repair completed result requires changedFiles.", {
      repairId: input.repairId,
    });
  }
  if (result.status === "blocked" || result.status === "failed") {
    return {
      accepted: false,
      status: "not_accepted",
      repairId: input.repairId,
      resultFile: toProjectRelative(root, path.resolve(root, input.resultFile)),
      nextAction: "manual_review",
      instruction: {
        mode: "report_blocked",
        autoContinue: false,
        mustRunImmediately: false,
        repairId: input.repairId,
        resultStatus: result.status,
        deploymentFailureRef: result.deploymentFailureRef,
        routingRule: "Deploy-sourced execution repair did not complete. Do not retry deploy automatically; report the blocked/failed repair result for user or manual review.",
        userMessage: "Deploy-sourced execution repair did not complete, so deployment retry is not safe.",
      },
    };
  }
  validateDeployExecutionRepairRuntimeEvidence(result, request);

  return {
    accepted: true,
    status: "accepted",
    repairId: input.repairId,
    resultFile: toProjectRelative(root, path.resolve(root, input.resultFile)),
    nextAction: "deploy_retry",
    instruction: deployRetryInstruction(),
  };
}

function validateDeployExecutionRepairRuntimeEvidence(
  result: Awaited<ReturnType<typeof readDeployExecutionRepairTaskResult>>,
  request: DeployExecutionRepairRequest,
): void {
  const requirement = request.syntheticTask.runtimeDeliveryRequirement;
  const addressedFields = new Set(result.runtimeDeliveryEvidence.addressedFailedContractFields);
  const missingFields = requirement.affectedContractFields.filter((field) => !addressedFields.has(field));
  if (missingFields.length > 0) {
    throw invalidArgument("Deploy execution repair result did not address every failed runtime contract field.", {
      repairId: request.repairId,
      missingFields,
    });
  }

  const allowedCheckIds = new Set(requirement.requiredCodeLevelChecks.map((check) => check.checkId));
  const resultCheckIds = new Set(result.runtimeDeliveryEvidence.codeLevelChecks.map((check) => check.checkId));
  const invalidCheckIds = result.runtimeDeliveryEvidence.codeLevelChecks
    .map((check) => check.checkId)
    .filter((checkId) => !allowedCheckIds.has(checkId));
  const missingCheckIds = requirement.requiredCodeLevelChecks
    .map((check) => check.checkId)
    .filter((checkId) => !resultCheckIds.has(checkId));
  if (invalidCheckIds.length > 0 || missingCheckIds.length > 0) {
    throw invalidArgument("Deploy execution repair result runtime code-level checks do not match the repair request.", {
      repairId: request.repairId,
      invalidCheckIds,
      missingCheckIds,
    });
  }

  const failedChecks = result.runtimeDeliveryEvidence.codeLevelChecks.filter((check) =>
    check.status === "failed" || check.status === "blocked"
  );
  if ((result.status === "completed" || result.status === "completed_with_notes") && failedChecks.length > 0) {
    throw invalidArgument("Deploy execution repair completed result cannot contain failed or blocked runtime checks.", {
      repairId: request.repairId,
      failedCheckIds: failedChecks.map((check) => check.checkId),
    });
  }
}

function repairRequestGenerationInstruction(input: {
  repairType: RepairRequest["repairType"];
  requestRef: string;
  candidateFile: string | null;
  request: RepairRequest;
}): Record<string, unknown> {
  return withAutoRunnableTransition({
    mode: "generate_candidate",
    ...artifactInstructionPolicy(),
    candidateKind: candidateKindForRepairType(input.repairType),
    requestRef: input.requestRef,
    candidateFile: input.candidateFile,
    outputContract: input.request.outputContract,
    blockedOutput: null,
    submitCommand: concreteRepairSubmitCommand(input.request, input.candidateFile),
    recovery: false,
    requestAlreadyExists: true,
    mustNotRunCommandsBeforeSubmit: ["repair request", "review", "next-task", "architecture request", "task-plan request", "loom continue"],
    generationSteps: [
      "Read requestRef.",
      compactContextReadStep,
      "Use referencedArtifactReadGuide for workspaceContext and inputs refs; do not guess jq wrapper roots.",
      "Use repairRules, enumRefs, outputContract, and resumePolicy from the RepairRequest.",
      "Write only the requested repair candidate/result file(s); do not modify unrelated project files.",
      "If submitCommand.argv is empty, stop and report that the original generation request id/ref is missing; do not guess requestId.",
      "Run submitCommand after the repair candidate/result exists.",
      "Follow the submit command response instruction immediately when it is auto-runnable.",
    ],
    routingRule: "RepairRequest has already been created. Generate the requested repair candidate now and submit it; do not run repair request or loom continue before the submit command succeeds.",
    userMessage: "RepairRequest created. Generate the repair candidate now and submit it with the provided command.",
  }, {
    sourceCommand: "repair request",
    sourceSummary: "RepairRequest was created.",
    primaryAction: "generate_repair_artifact_and_submit",
  });
}

function deployExecutionRepairRequestFor(input: {
  repairId: string;
  failure: DeploymentFailureReport;
  failureRef: string;
  resultFile: string;
}): DeployExecutionRepairRequest {
  const affectedField = input.failure.failedContract.field;
  const request = {
    schemaVersion: "1.0" as const,
    repairId: input.repairId,
    repairType: "execution_repair" as const,
    source: "deploy_failure" as const,
    deploymentFailureRef: input.failureRef,
    sourceRefs: input.failure.sourceRefs,
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      deploymentFailureRef: input.failureRef,
      runtimeDeliveryRef: input.failure.sourceRefs.runtimeDeliveryRef,
      taskPlanRef: input.failure.sourceRefs.taskPlanRef,
      taskPlanRunRef: input.failure.sourceRefs.taskPlanRunRef,
      reviewResultRef: input.failure.sourceRefs.reviewResultRef,
      deploymentSpecRef: input.failure.sourceRefs.deploymentSpecRef,
    }),
    syntheticTask: {
      taskId: `deploy-runtime-${input.repairId}`,
      taskKind: "runtime_delivery" as const,
      title: "Repair runtime delivery failure found by deploy",
      objective: "Fix application code, package scripts, project configuration, or runtime chain so the RuntimeDeliveryContract can pass the failed deploy check.",
      mutatesOriginalTaskPlan: false as const,
      relatedTaskIds: [],
      writeBoundary: {
        forbiddenPaths: unique([
          ".loom",
          ...input.failure.routing.mustNotEdit,
        ]),
      },
      runtimeDeliveryRequirement: {
        appliesToThisTask: true as const,
        source: "deploy_failure" as const,
        deploymentFailureRef: input.failureRef,
        runtimeDeliveryRef: input.failure.runtimeDeliveryRef,
        affectedContractFields: [affectedField],
        requiredCodeLevelChecks: [{
          checkId: `repair-${affectedField.replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase() || "runtime-delivery"}`,
          contractField: affectedField,
          objective: `Repair the application-side code/script/runtime chain that failed at ${input.failure.evidence.failedAt}.`,
          acceptableEvidence: ["error_window", "manual_command_output", "static_check", "full_log_ref_if_needed"],
        }],
        forbiddenActions: [
          "do_not_edit_deploy_generated_files",
          "do_not_edit_runtime_delivery_contract",
          "do_not_modify_original_task_plan",
          "do_not_claim_container_success_without_deploy_retry",
        ],
      },
    },
    executionRules: {
      mayEditApplicationCode: true,
      mayEditPackageScripts: true,
      mayEditDeployGeneratedFiles: false,
      mayEditLoomArtifacts: false,
      doNotModifyOriginalTaskPlan: true,
      doNotRunDeployRepair: true,
      doNotRequireCleanInstallOrContainerBuild: true,
      mustAddressDeploymentFailureRef: true,
      mustSubmitResult: true,
      sourceEditPreparationContract: sourceEditPreparationContract({
        resultFile: input.resultFile,
        submitCommandName: "repair submit",
      }),
      verificationCommandSchedulingRules,
      evidenceReadPolicy: {
        firstRead: "deploymentFailureRef#.evidence.errorWindow",
        fullLogFallback: "deploymentFailureRef#.evidence.fullLogRef",
        rule: "Use the compact errorWindow first. Read the full deploy log only when the error window and diagnostics are insufficient.",
      },
    },
    outputContract: {
      format: "json" as const,
      schema: "DeployExecutionRepairTaskResult" as const,
      resultFile: input.resultFile,
      schemaShape: deployExecutionRepairResultSchemaShape(input.repairId, input.failureRef, affectedField),
      submitCommand: {
        name: "repair submit",
        argv: [
          "repair",
          "submit",
          "--type",
          "execution",
          "--source",
          "deploy",
          "--repair-id",
          input.repairId,
          "--result-file",
          input.resultFile,
        ],
      },
    },
    createdAt: new Date().toISOString(),
  };
  return {
    ...request,
    agentAction: deployExecutionRepairAgentAction({
      taskField: "syntheticTask",
      task: request.syntheticTask,
      outputContract: request.outputContract,
      includeSourceField: false,
    }),
  };
}

function deployTaskExecutionRequestFor(input: {
  request: DeployExecutionRepairRequest;
  requestRef: string;
  executionRequestRef: string;
}): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    requestId: input.request.repairId,
    requestType: "deploy_sourced_execution_repair",
    agentAction: deployExecutionRepairAgentAction({
      taskField: "task",
      task: input.request.syntheticTask,
      outputContract: input.request.outputContract,
      includeSourceField: true,
    }),
    source: {
      repairRequestRef: input.requestRef,
      deploymentFailureRef: input.request.deploymentFailureRef,
      mutatesOriginalTaskPlan: false,
    },
    task: input.request.syntheticTask,
    sourceRefs: input.request.sourceRefs,
    referencedArtifactReadGuide: input.request.referencedArtifactReadGuide,
    executionRules: input.request.executionRules,
    outputContract: input.request.outputContract,
    submitCommand: input.request.outputContract.submitCommand,
    postSubmitRouting: {
      submitCommandReturnsInstruction: true,
      followReturnedInstructionImmediately: true,
      nextAction: "deploy_retry",
    },
    requestRef: input.executionRequestRef,
    createdAt: new Date().toISOString(),
  };
}

function deployExecutionRepairAgentAction(input: {
  taskField: "syntheticTask" | "task";
  task: DeployExecutionRepairRequest["syntheticTask"];
  outputContract: DeployExecutionRepairRequest["outputContract"];
  includeSourceField: boolean;
}): Record<string, unknown> {
  const submitCommand = input.outputContract.submitCommand;
  const requiredCodeLevelChecks = input.task.runtimeDeliveryRequirement.requiredCodeLevelChecks;
  return agentActionContract({
    actionKind: "execute_task",
    instruction: "Execute this deploy-sourced runtime repair against application code or scripts, write the deploy repair TaskResult to outputContract.resultFile, then run submitCommand exactly.",
    read: {
      required: unique([
        "this request",
        "referencedArtifactReadGuide",
        input.taskField,
        ...(input.includeSourceField ? ["source"] : ["deploymentFailureRef"]),
        "sourceRefs",
        "executionRules",
        "outputContract.schemaShape",
        "outputContract.resultFile",
        "outputContract.submitCommand",
        ...(input.includeSourceField ? ["postSubmitRouting"] : []),
      ]),
      optional: unique([
        "sourceRefs.runtimeDeliveryRef",
        "sourceRefs.deploymentSpecRef",
        "sourceRefs.taskPlanRef",
        "sourceRefs.taskPlanRunRef",
        "sourceRefs.reviewResultRef",
      ]),
      displayPolicy: "compact",
    },
    write: {
      resultFile: input.outputContract.resultFile,
      requiredTopLevelFields: [
        "schemaVersion",
        "repairId",
        "status",
        "deploymentFailureRef",
        "changedFiles",
        "runtimeDeliveryEvidence",
        "selfRepairSummary",
        "notes",
      ],
      requiredTopLevelFieldRule: "DeployExecutionRepairTaskResult must include every requiredTopLevelFields entry before submitCommand runs.",
      requiredRuntimeEvidence: {
        source: "deploy_failure_repair",
        requiredCheckIds: requiredCodeLevelChecks.map((check) => check.checkId),
        requiredCodeLevelChecks,
        rule: "Copy each exact checkId into runtimeDeliveryEvidence.codeLevelChecks[].checkId and address the failed deploy contract field without editing generated deployment files.",
      },
      rules: [
        "Repair application code, package scripts, or runtime configuration only within the deploy-sourced repair boundary.",
        "Do not edit generated Dockerfile, Compose, dockerignore, .loom files, RuntimeDeliveryContract, AAC, ReviewResult, or the original TaskPlan.",
        "Use deploymentFailureRef evidence.errorWindow first; read fullLogRef only when the compact evidence is insufficient.",
        "Write result JSON only to outputContract.resultFile.",
        "Before writing the result, copy every field named in agentAction.write.requiredTopLevelFields.",
        "After resultFile exists, run submitCommand exactly and follow the returned deploy retry instruction when it is auto-runnable.",
      ],
    },
    submit: {
      command: submitCommand,
      requiredArgs: requiredArgsForCommand(submitCommand),
      placeholders: {},
      runAfter: "outputContract.resultFile exists and follows outputContract.schemaShape",
    },
    schema: {
      primary: "DeployExecutionRepairTaskResult",
      shapeLocation: "outputContract.schemaShape",
      enumLocation: "outputContract.schemaShape",
    },
    stopConditions: [
      "request cannot be read",
      "submitCommand returns non-repairable failure",
      "returned instruction is user-gated, blocked, done, manual_review, or needs_user_decision",
    ],
  });
}

function deployExecutionRepairResultSchemaShape(
  repairId: string,
  failureRef: string,
  affectedField: string,
): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    repairId,
    status: "completed | completed_with_notes | blocked | failed",
    deploymentFailureRef: failureRef,
    changedFiles: ["package.json", "src/server/index.ts"],
    runtimeDeliveryEvidence: {
      source: "deploy_failure_repair",
      addressedFailedContractFields: [affectedField],
      codeLevelChecks: [{
        checkId: "check-id-from-requiredCodeLevelChecks",
        status: "passed | failed | blocked | not_applicable",
        evidence: "Short code-level evidence summary.",
      }],
      commandsRun: [{
        command: "local code-level command run, if any",
        status: "passed | failed | not_run",
        environment: "local_warm | project_workspace | unknown",
        summary: "Short command outcome.",
      }],
      unverifiedItems: [{
        item: "field or check that could not be verified",
        reason: "Environment/dependency reason or why it is not applicable.",
      }],
      runtimeProbeCleanup: {
        temporaryRuntimeStarted: "boolean. true only when this repair task started a temporary local runtime, dev server, preview server, container, or probe process.",
        attempted: "boolean. true when cleanup was attempted for a task-owned temporary runtime.",
        status: "not_needed | succeeded | failed | unknown | not_safe_to_cleanup",
        targets: [{
          kind: "process | port | container | dev_server | other",
          pid: "number or null when known",
          port: "number or null when known",
          command: "string or null when known",
          summary: "Short target summary.",
        }],
        summary: "Cleanup failed/unknown/not_safe_to_cleanup is completed_with_notes only unless there is an independent product defect.",
      },
    },
    selfRepairSummary: {
      attempted: true,
      attemptCount: 1,
      stopReason: "verification_passed",
      progressObserved: true,
    },
    notes: [],
  };
}

function deployExecutionRepairInstruction(
  requestRef: string,
  resultFile: string,
  request: DeployExecutionRepairRequest,
): Record<string, unknown> {
  const submitCommand = request.outputContract.submitCommand;
  return withAutoRunnableTransition({
    mode: "execute_task",
    requestRef,
    resultFile,
    submitCommand,
    task: {
      taskId: request.syntheticTask.taskId,
      title: request.syntheticTask.title,
      taskKind: request.syntheticTask.taskKind,
    },
    completionBarrier: {
      resultFile,
      submitCommand,
      rules: [
        "This repair task is not complete until resultFile exists and submitCommand has run successfully.",
        "Do not send progress-only summaries, interim handoff notes, or next-step summaries before submitCommand succeeds.",
      ],
    },
    primaryAction: {
      action: "execute_deploy_sourced_execution_repair",
      requestRef,
      resultFile,
      submitCommand,
      rule: "Execute this deploy-sourced synthetic repair task now. Do not replace execution with a recovery prompt.",
    },
    completionCondition: {
      completeWhen: "TaskResult exists at resultFile and submitCommand has succeeded.",
      afterSubmit: "Follow returned deploy retry instruction immediately when auto-runnable.",
      stopOnlyWhen: [
        "request cannot be read",
        "submitCommand returns non-repairable failure",
        "returned instruction is user-gated, blocked, done, manual_review, or needs_user_decision",
      ],
    },
    mustNotDuringPrimaryAction: [
      "Do not replace this repair task with a recovery prompt while tool calls are still available.",
      "Do not send progress-only summaries, interim handoff notes, or next-step summaries before submitCommand succeeds.",
      "Do not mutate the original TaskPlan, deploy state, Dockerfile/Compose/dockerignore, RuntimeDeliveryContract, AAC, or ReviewResult.",
      ...verificationCommandSchedulingRules.slice(0, 4),
    ],
    verificationCommandSchedulingRules,
    routingRule: "Execute this deploy-sourced synthetic repair task now. It does not mutate the original TaskPlan. Repair application code/scripts only, write resultFile, then run submitCommand. Do not stop with a progress-only summary before submitCommand succeeds.",
    userMessage: "Deploy-sourced execution repair request created. Execute the synthetic repair task now and submit its result before any interim summary.",
  }, {
    sourceCommand: "repair request",
    sourceSummary: "Deploy-sourced execution repair request was created.",
    primaryAction: "execute_deploy_sourced_execution_repair",
    mustStartImmediately: true,
  });
}

function deployRetryInstruction(): Record<string, unknown> {
  return withAutoRunnableTransition({
    mode: "run_cli",
    command: {
      name: "deploy run",
      argv: ["deploy", "run"],
    },
    routingRule: "Deploy-sourced execution repair was accepted. Retry deploy now; do not ask the user whether to continue.",
    userMessage: "Execution repair accepted. Retry deployment now.",
  }, {
    sourceCommand: "record-result",
    sourceSummary: "Deploy-sourced execution repair result was accepted.",
    primaryAction: "retry_deploy",
  });
}

function agentActionForRepairRequest(
  repairType: RepairRequest["repairType"],
  outputContract: Record<string, unknown>,
  candidateFile: string | null,
  repairRequestId: string,
  submitCommand: { name: string; argv: string[] },
) {
  if (repairType === "execution_repair") {
    return null;
  }
  return agentActionContract({
    actionKind: "generate_candidate",
    instruction: "Repair the requested candidate/result contract only, write the files named by outputContract, then run submitCommand exactly.",
    read: {
      required: ["this RepairRequest", "referencedArtifactReadGuide", "inputs", "repairRules", "enumRefs", "outputContract", "resumePolicy"],
      optional: ["workspaceContext.repositoryContextRef", "inputs.originalRequestRef", "inputs.validationIssues"],
      displayPolicy: "compact",
    },
    write: {
      candidateFile: candidateFile ?? undefined,
      outlineFile: typeof outputContract.outlineFile === "string" ? outputContract.outlineFile : undefined,
      groupFilePattern: typeof outputContract.groupFilePattern === "string" ? outputContract.groupFilePattern : undefined,
      sectionOutputs: Array.isArray(outputContract.sectionOutputs)
        ? outputContract.sectionOutputs
            .filter((item): item is { section: string; candidateFile: string } => isRecord(item) && typeof item.section === "string" && typeof item.candidateFile === "string")
            .map((item) => ({ section: item.section, candidateFile: item.candidateFile }))
        : undefined,
      rules: [
        "Repair only the artifact allowed by repairRules and scope.",
        "Do not create a new request.",
        "Do not modify project files from artifact repair requests.",
        "Do not guess requestId when submitCommand is empty; report blocked instead.",
        "Do not run loom continue before submitCommand succeeds.",
      ],
    },
    submit: {
      command: submitCommand,
      requiredArgs: requiredArgsForCommand(submitCommand),
      placeholders: {
        "{candidateFile}": candidateFile ?? "",
        "{repairRequestId}": repairRequestId,
      },
      runAfter: "the requested repair candidate files exist",
    },
    schema: {
      primary: candidateKindForRepairType(repairType),
      shapeLocation: "outputContract.schemaShape or outputContract.originalOutputContract",
      enumLocation: "enumRefs",
    },
    stopConditions: ["submitCommand argv is empty", "submitCommand returns non-repairable failure"],
  });
}

function candidateKindForRepairType(repairType: RepairRequest["repairType"]): string {
  if (repairType === "task_result_repair") {
    return "TaskResult";
  }
  if (repairType === "taskplan_repair") {
    return "TaskPlanGroupedReplacement";
  }
  if (repairType === "architecture_artifact_repair") {
    return "ArchitectureSectionReplacement";
  }
  return "TaskExecutionRepair";
}

function concreteRepairSubmitCommand(request: RepairRequest, candidateFile: string | null): unknown {
  const submitCommand = request.outputContract.submitCommand;
  if (!isCommand(submitCommand)) {
    return submitCommand ?? null;
  }
  return {
    ...submitCommand,
    argv: submitCommand.argv.map((part) => {
      if (part === "{candidateFile}") {
        return candidateFile ?? part;
      }
      if (part === "{repairRequestId}") {
        return request.repairRequestId;
      }
      return part;
    }),
  };
}

function concreteOutputSubmitCommand(outputContract: Record<string, unknown>, candidateFile: string | null, repairRequestId: string): { name: string; argv: string[] } {
  const submitCommand = outputContract.submitCommand;
  if (!isCommand(submitCommand)) {
    return { name: "unknown", argv: [] };
  }
  return {
    ...submitCommand,
    argv: submitCommand.argv.map((part) => {
      if (part === "{candidateFile}") {
        return candidateFile ?? part;
      }
      if (part === "{repairRequestId}") {
        return repairRequestId;
      }
      return part;
    }),
  };
}

function requiredArgsForCommand(command: { argv: string[] }): string[] {
  return command.argv.filter((part) => part.startsWith("--"));
}

function isCommand(value: unknown): value is { name: string; argv: string[] } {
  return typeof value === "object" &&
    value !== null &&
    typeof (value as { name?: unknown }).name === "string" &&
    Array.isArray((value as { argv?: unknown }).argv) &&
    (value as { argv: unknown[] }).argv.every((part) => typeof part === "string");
}

function firstStringAt(value: unknown, pathParts: Array<string | number>): string | undefined {
  let current: unknown = value;
  for (const part of pathParts) {
    if (typeof part === "number") {
      if (!Array.isArray(current)) return undefined;
      current = current[part];
      continue;
    }
    if (!isRecord(current)) return undefined;
    current = current[part];
  }
  return typeof current === "string" && current.length > 0 ? current : undefined;
}

function normalizeRepairType(type: CreateRepairRequestInput["type"]): RepairRequest["repairType"] {
  if (type === "execution") return "execution_repair";
  if (type === "task-result") return "task_result_repair";
  if (type === "taskplan") return "taskplan_repair";
  if (type === "architecture") return "architecture_artifact_repair";
  throw invalidArgument("Unsupported repair request type.", { type });
}

function shouldProduceCandidate(repairType: RepairRequest["repairType"]): boolean {
  return repairType !== "execution_repair";
}

async function inputsFor(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  repairType: RepairRequest["repairType"],
): Promise<Record<string, unknown>> {
  if (repairType === "execution_repair") {
    return executionRepairInputs(projectRoot, locator, repairType);
  }
  if (repairType === "task_result_repair") {
    return taskResultRepairInputs(projectRoot, locator, repairType);
  }
  return {
    sourceFacts: [],
    repairType,
    note: "Route engine supplies concrete source refs from latest facts when available.",
  };
}

type RepairContext = {
  requestId: string | null;
  requestRef: string | null;
  request: Record<string, unknown> | null;
};

async function repairContextFor(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  repairType: RepairRequest["repairType"],
): Promise<RepairContext> {
  if (repairType === "task_result_repair") {
    return taskResultRepairContext(projectRoot, locator);
  }
  if (repairType !== "taskplan_repair" && repairType !== "architecture_artifact_repair") {
    return { requestId: null, requestRef: null, request: null };
  }
  const delivery = await loadDeliveryIndex(projectRoot, locator.deliveryId);
  const phase = delivery.phases.find((item) => item.phaseId === locator.phaseId);
  if (!phase) {
    return { requestId: null, requestRef: null, request: null };
  }
  const requestIdKey = repairType === "taskplan_repair" ? "taskPlanRequestId" : "architectureRequestId";
  const requestRefKey = repairType === "taskplan_repair" ? "taskPlanRequest" : "architectureRequest";
  const requestId = phase.latestRefs[requestIdKey] ?? null;
  const requestRef = phase.latestRefs[requestRefKey] ?? null;
  if (requestId && requestRef && await pathExists(path.join(projectRoot, requestRef))) {
    const loaded = await hydrateRequestManifest(projectRoot, path.join(projectRoot, requestRef));
    return {
      requestId,
      requestRef,
      request: isRecord(loaded) ? loaded : null,
    };
  }
  if (requestId) {
    const requestPath = repairType === "taskplan_repair"
      ? taskPlanRequestPath(projectRoot, requestId, locator)
      : architectureRequestPath(projectRoot, requestId, locator);
    if (await pathExists(requestPath)) {
      const loaded = await hydrateRequestManifest(projectRoot, requestPath);
      return {
        requestId,
        requestRef: toProjectRelative(projectRoot, requestPath),
        request: isRecord(loaded) ? loaded : null,
      };
    }
  }
  return { requestId: null, requestRef: null, request: null };
}

async function taskResultRepairInputs(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  repairType: RepairRequest["repairType"],
): Promise<Record<string, unknown>> {
  const run = await loadCurrentTaskPlanRun(projectRoot, undefined, locator);
  const taskPlan = taskPlanSchema.parse(await readJsonFile(taskPlanPath(projectRoot, run.taskPlanId, locator)));
  const sourceTaskId = run.nextAction?.type === "task_result_repair" ? run.nextAction.sourceTaskId : undefined;
  const task = sourceTaskId ? taskPlan.tasks.find((item) => item.taskId === sourceTaskId) : undefined;
  const context = await taskResultRepairContext(projectRoot, locator);
  const invalidResultRef = context.requestId
    ? toProjectRelative(projectRoot, taskExecutionResultCandidatePath(projectRoot, locator, context.requestId))
    : null;

  return {
    sourceFacts: [{
      source: "latest_task_result_validation_failure",
      taskPlanRunId: run.runId,
      taskPlanRef: toProjectRelative(projectRoot, taskPlanPath(projectRoot, run.taskPlanId, locator)),
      taskPlanRunRef: toProjectRelative(projectRoot, taskPlanRunPath(projectRoot, run.runId, locator)),
      sourceTaskId: sourceTaskId ?? null,
      task: task ? {
        taskId: task.taskId,
        taskPlanId: taskPlan.taskPlanId,
        title: task.title,
        verificationIntents: task.verificationIntents,
        runtimeDeliveryRequirement: task.runtimeDeliveryRequirement ?? null,
        frontendExperienceRequirement: task.frontendExperienceRequirement ?? null,
        conceptRefs: task.conceptRefs ?? [],
      } : null,
      originalTaskExecutionRequestRef: context.requestRef,
      invalidTaskResultFile: invalidResultRef,
    }],
    repairType,
    note: "Repair the TaskResult contract for sourceTaskId only. Use originalTaskExecutionRequestRef/outputContract for exact verificationResults and runtimeDeliveryEvidence shape.",
  };
}

async function taskResultRepairContext(projectRoot: string, locator: DeliveryPhaseLocator): Promise<RepairContext> {
  const run = await loadCurrentTaskPlanRun(projectRoot, undefined, locator);
  const sourceTaskId = run.nextAction?.type === "task_result_repair" ? run.nextAction.sourceTaskId : undefined;
  if (!sourceTaskId) {
    return { requestId: null, requestRef: null, request: null };
  }
  const requestDir = path.dirname(taskExecutionRequestPath(projectRoot, "__placeholder__", locator));
  if (!(await pathExists(requestDir))) {
    return { requestId: null, requestRef: null, request: null };
  }
  const entries = await fs.readdir(requestDir);
  const matches: Array<{ requestId: string; requestRef: string; request: Record<string, unknown>; mtimeMs: number }> = [];
  for (const entry of entries.filter((item) => item.endsWith(".json"))) {
    const requestPath = path.join(requestDir, entry);
    const loaded = await hydrateRequestManifest(projectRoot, requestPath);
    if (!isRecord(loaded)) {
      continue;
    }
    const taskId = firstStringAt(loaded, ["source", "taskId"]) ?? firstStringAt(loaded, ["task", "taskId"]);
    if (taskId !== sourceTaskId) {
      continue;
    }
    const stat = await fs.stat(requestPath);
    matches.push({
      requestId: path.basename(entry, ".json"),
      requestRef: toProjectRelative(projectRoot, requestPath),
      request: loaded,
      mtimeMs: stat.mtimeMs,
    });
  }
  const latest = matches.sort((a, b) => b.mtimeMs - a.mtimeMs)[0];
  if (!latest) {
    return { requestId: null, requestRef: null, request: null };
  }
  return {
    requestId: latest.requestId,
    requestRef: latest.requestRef,
    request: latest.request,
  };
}

async function executionRepairInputs(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  repairType: RepairRequest["repairType"],
): Promise<Record<string, unknown>> {
  const latestReview = await loadLatestReviewResult(projectRoot, locator);
  const manualResolution = await loadLatestManualReviewResolution(projectRoot, locator);
  if (!latestReview) {
    return {
      sourceFacts: [],
      repairType,
      note: "No accepted ReviewResult was available; repair agent must inspect latest task and review artifacts before modifying code.",
    };
  }

  const resolutionFindingRefs = Array.isArray(manualResolution?.changeRequest?.details?.findingRefs)
    ? manualResolution.changeRequest.details.findingRefs.filter((ref): ref is string => typeof ref === "string")
    : [];
  const targetFindingRefs = unique([
    ...(latestReview.nextAction.findingRefs ?? []),
    ...resolutionFindingRefs,
  ]);
  const findings = latestReview.findings.filter((finding) => {
    if (manualResolution?.changeRequest?.route === "execution_repair") {
      return targetFindingRefs.length === 0 || targetFindingRefs.includes(finding.findingId);
    }
    return finding.recommendedNextAction === "execution_repair" &&
      (targetFindingRefs.length === 0 || targetFindingRefs.includes(finding.findingId));
  });
  const targetTaskIds = unique([
    ...(latestReview.nextAction.targetTaskIds ?? []),
    ...(
      Array.isArray(manualResolution?.changeRequest?.details?.targetTaskIds)
        ? manualResolution.changeRequest.details.targetTaskIds.filter((taskId): taskId is string => typeof taskId === "string")
        : []
    ),
    ...findings.flatMap((finding) => finding.taskRefs),
  ]);
  const taskPlan = await loadJsonIfExists(taskPlanPath(projectRoot, latestReview.source.taskPlanId, locator));
  const taskPlanRun = await loadJsonIfExists(taskPlanRunPath(projectRoot, latestReview.source.taskPlanRunId, locator));
  const taskStates = taskPlanRun && typeof taskPlanRun === "object" && Array.isArray((taskPlanRun as { taskStates?: unknown }).taskStates)
    ? (taskPlanRun as { taskStates: Array<Record<string, unknown>> }).taskStates
    : [];
  const taskResultIds = unique(taskStates
    .filter((state) => typeof state.taskId === "string" && targetTaskIds.includes(state.taskId))
    .map((state) => state.resultId)
    .filter((resultId): resultId is string => typeof resultId === "string" && resultId.length > 0));

  return {
    sourceFacts: [{
      source: "latest_review_result",
      reviewId: latestReview.reviewId,
      reviewResultRef: toProjectRelative(projectRoot, reviewResultPath(projectRoot, latestReview.reviewId, locator)),
      decision: latestReview.decision,
      nextAction: latestReview.nextAction,
      findings,
      coverageAssessment: latestReview.coverageAssessment,
    }],
    manualReviewResolution: manualResolution
      ? {
          resolutionId: manualResolution.manualReviewResolutionId,
          decision: manualResolution.decision,
          changeRequest: manualResolution.changeRequest,
          userAnswer: manualResolution.userAnswer,
        }
      : null,
    repairType,
    targetTaskIds,
    targetAcceptanceRefs: unique(findings.flatMap((finding) => finding.acceptanceRefs)),
    targetTaskResultIds: taskResultIds,
    taskPlanRef: toProjectRelative(projectRoot, taskPlanPath(projectRoot, latestReview.source.taskPlanId, locator)),
    taskPlanRunRef: toProjectRelative(projectRoot, taskPlanRunPath(projectRoot, latestReview.source.taskPlanRunId, locator)),
    taskPlanTasks: projectTasksFor(taskPlan, targetTaskIds),
    note: "Repair the execution issues identified by latest ReviewResult findings. Do not alter contracts or unrelated tasks.",
  };
}

async function loadLatestReviewResult(projectRoot: string, locator: DeliveryPhaseLocator): Promise<ReviewResult | null> {
  const latestPath = reviewLatestPath(projectRoot, locator);
  if (!(await pathExists(latestPath))) return null;
  const latest = await readJsonFile(latestPath);
  if (typeof latest !== "object" || latest === null) return null;
  const latestResultRef = (latest as Record<string, unknown>).latestResultRef;
  if (typeof latestResultRef !== "string" || latestResultRef.length === 0) return null;
  const resultPath = path.resolve(projectRoot, latestResultRef);
  if (!(await pathExists(resultPath))) return null;
  return reviewResultSchema.parse(await readJsonFile(resultPath));
}

async function loadLatestManualReviewResolution(projectRoot: string, locator: DeliveryPhaseLocator): Promise<ManualReviewResolution | null> {
  const latestPath = reviewLatestPath(projectRoot, locator);
  if (!(await pathExists(latestPath))) return null;
  const latest = await readJsonFile(latestPath);
  if (typeof latest !== "object" || latest === null) return null;
  const latestResolutionRef = (latest as Record<string, unknown>).latestResolutionRef;
  if (typeof latestResolutionRef !== "string" || latestResolutionRef.length === 0) return null;
  const resolutionPath = path.resolve(projectRoot, latestResolutionRef);
  if (!(await pathExists(resolutionPath))) return null;
  return manualReviewResolutionSchema.parse(await readJsonFile(resolutionPath));
}

async function loadJsonIfExists(filePath: string): Promise<unknown | null> {
  if (!(await pathExists(filePath))) return null;
  return readJsonFile(filePath);
}

function projectTasksFor(taskPlan: unknown, taskIds: string[]): unknown[] {
  if (!taskPlan || typeof taskPlan !== "object" || !Array.isArray((taskPlan as { tasks?: unknown }).tasks)) return [];
  return (taskPlan as { tasks: Array<Record<string, unknown>> }).tasks.filter((task) =>
    typeof task.taskId === "string" && taskIds.includes(task.taskId),
  );
}

function unique(values: string[]): string[] {
  return [...new Set(values)];
}

function repairRulesFor(repairType: RepairRequest["repairType"]): string[] {
  const common = [
    "Use RepositoryContext and WorkspaceChangeContext when available.",
    "Do not implement deferred or future phase scope.",
    "Do not invent user decisions.",
  ];
  if (repairType === "execution_repair") {
    return [
      "Repair only the target task implementation.",
      "Do not modify Brainstorm, TechnicalBaseline, PGC, AAC, or TaskPlan.",
      "If repair requires contract changes, return blocked TaskResult using fixed blocked output mapping.",
      ...common,
    ];
  }
  if (repairType === "task_result_repair") {
    return [
      "Repair only TaskResult contract fields.",
      "Do not modify project source code.",
      "Return a complete replacement TaskResult.",
      ...common,
    ];
  }
  if (repairType === "taskplan_repair") {
    return [
      "Return TaskPlan grouped replacement outputs according to Step 6A.",
      "For outline-level issues, rewrite outline and all group files.",
      "For group-level issues, rewrite only the affected complete group files.",
      "Do not return a whole TaskPlan JSON replacement.",
      "Repair only TaskPlan mechanics, scope mapping, task graph, verification mapping, or artifact mapping.",
      "Do not change Brainstorm scope, TechnicalBaseline, PGC, or AAC.",
      ...common,
    ];
  }
  return [
    "Return section-level ArchitectureSection replacement candidates according to Step 5A.",
    "Rewrite the affected section and any downstream sections required by the section dependency order.",
    "Do not return a whole ArchitectureArtifactContract JSON replacement.",
    "Repair only ArchitectureArtifactContract design facts needed for the current phase.",
    "Use outputContract.contextProjection.requirementDetailTransfer and outputContract.sectionOutputs[].generationRules from the original ArchitectureSectionsGenerationRequest when present; preserve phaseScope items, acceptance sourceRefs/capabilityRefs, business flow summaries, concept refs, and frontend refs.",
    "Do not modify Brainstorm scope, TechnicalBaseline, or PGC.",
    "After AAC repair, TaskPlan must be regenerated.",
    ...common,
  ];
}

function outputContractFor(
  projectRoot: string,
  repairType: RepairRequest["repairType"],
  locator: DeliveryPhaseLocator,
  candidateFile: string | null,
  repairRequestId: string,
  repairContext: RepairContext,
): Record<string, unknown> {
  const { deliveryId, phaseId } = locator;
  if (repairType === "execution_repair") {
    return {
      kind: "task_result",
      schema: "TaskResult",
      schemaShape: repairTaskResultSchemaShape(deliveryId, phaseId),
      resultRules: taskResultResultRules(),
      repairSubmitRouting: repairSubmitRouting({
        kind: "result",
        submitCommandName: "record-result",
      }),
      submitCommand: {
        name: "record-result",
        argv: ["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", "{taskResultFile}"],
      },
    };
  }
  if (repairType === "task_result_repair") {
    const originalOutputContract = isRecord(repairContext.request?.outputContract) ? repairContext.request.outputContract : null;
    return {
      kind: "task_result",
      schema: "TaskResult",
      candidateFile,
      originalTaskExecutionRequestRef: repairContext.requestRef,
      sourceResultFile: repairContext.requestId
        ? toProjectRelative(projectRoot, taskExecutionResultCandidatePath(projectRoot, locator, repairContext.requestId))
        : null,
      schemaShape: isRecord(originalOutputContract?.schemaShape)
        ? originalOutputContract.schemaShape
        : repairTaskResultSchemaShape(deliveryId, phaseId),
      originalOutputContract,
      resultRules: taskResultResultRules(),
      repairSubmitRouting: repairSubmitRouting({
        kind: "result",
        submitCommandName: "record-result",
      }),
      submitCommand: {
        name: "record-result",
        argv: ["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", "{candidateFile}"],
      },
    };
  }
  if (repairType === "taskplan_repair") {
    const originalRequestId = repairContext.requestId;
    const outlineFile = originalRequestId
      ? toProjectRelative(projectRoot, taskPlanOutlineCandidatePath(projectRoot, locator, originalRequestId))
      : null;
    const groupFilePattern = originalRequestId
      ? toProjectRelative(projectRoot, taskPlanGroupCandidatePath(projectRoot, locator, originalRequestId, "{groupId}"))
      : null;
    return {
      kind: "taskplan_grouped_replacement",
      schema: "TaskPlanGroupedReplacement",
      candidateFile,
      originalRequestId,
      originalRequestRef: repairContext.requestRef,
      returnCompleteReplacement: false,
      protocolRef: "Step 6A TaskPlanCandidateRepairRequest",
      schemaShapeRef: "Read the original TaskPlanGenerationRequest outputContract.outlineSchemaShape and groupSchemaShape before writing replacement files.",
      outlineFile,
      groupFilePattern,
      originalOutputContract: isRecord(repairContext.request?.outputContract) ? repairContext.request.outputContract : null,
      repairSubmitRouting: repairSubmitRouting({
        kind: "candidate",
        submitCommandName: "task-plan accept",
      }),
      submitCommand: {
        name: "task-plan accept",
        argv: originalRequestId
          ? ["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", originalRequestId, "--repair-id", repairRequestId]
          : [],
      },
    };
  }
  const originalRequestId = repairContext.requestId;
  const architectureOutputContract = isRecord(repairContext.request?.outputContract) ? repairContext.request.outputContract : null;
  return {
    kind: "architecture_section_replacement",
    schema: "ArchitectureSectionGroup",
    candidateFile,
    originalRequestId,
    originalRequestRef: repairContext.requestRef,
    returnCompleteReplacement: false,
    protocolRef: "Step 5A ArchitectureSectionRepairRequest",
    schemaShapeRef: "Read the original ArchitectureSectionsGenerationRequest outputContract.sectionOutputs[].schemaShape before writing replacement section files.",
    sectionOutputs: architectureRepairSectionOutputs(repairContext.request),
    contextProjection: isRecord(repairContext.request?.contextProjection) ? repairContext.request.contextProjection : null,
    fieldAccessHints: isRecord(repairContext.request?.fieldAccessHints) ? repairContext.request.fieldAccessHints : null,
    originalOutputContract: architectureOutputContract,
    requirementDetailTransferRule: "When contextProjection.requirementDetailTransfer is present, repair AAC sections by preserving current phase scope items, acceptance statement/sourceRefs/capabilityRefs, business flow details, concept refs, frontend refs, and target section generationRules.",
    repairSubmitRouting: repairSubmitRouting({
      kind: "candidate",
      submitCommandName: "architecture accept",
    }),
    submitCommand: {
      name: "architecture accept",
      argv: originalRequestId
        ? ["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", originalRequestId, "--repair-id", repairRequestId]
        : [],
    },
  };
}

function architectureRepairSectionOutputs(request: Record<string, unknown> | null): unknown[] {
  const outputContract = isRecord(request?.outputContract) ? request.outputContract : null;
  return Array.isArray(outputContract?.sectionOutputs) ? outputContract.sectionOutputs : [];
}

function repairTaskResultSchemaShape(_deliveryId: string, _phaseId: string): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    taskResultId: "result-target-task-id",
    taskId: "target-task-id",
    taskPlanId: "current-taskplan-id",
    status: "completed | completed_with_notes | blocked | failed",
    changedFiles: ["project-relative/path"],
    noChangeReason: null,
    verificationResults: [{
      verificationId: "verification-id-from-task",
      status: "passed | not_run | failed | inconclusive",
      evidenceType: "automated_test | manual_command_output | runtime_api_check | static_check | agent_review_explanation",
      summary: "Short verification summary.",
    }],
    selfRepairSummary: {
      attempted: false,
      attemptCount: 0,
      stopReason: "not_attempted",
      progressObserved: false,
    },
    failure: null,
    executionContinuity: {
      taskResultSubmittedAfterVerification: true,
      agentOwnedLongRunningWork: "none | started_and_released | unknown",
      notes: [],
    },
    runtimeDeliveryEvidence: "Only include when the original TaskExecutionRequest outputContract requires it; reuse that original outputContract.runtimeDeliveryEvidence shape exactly.",
    notes: [],
    blockedReasons: [],
    createdAt: new Date(0).toISOString(),
    updatedAt: new Date(0).toISOString(),
  };
}

function taskResultResultRules(): string[] {
  return [
    "Use only verificationResults ids listed in originalOutputContract.schemaShape.allowedVerificationResults or original TaskExecutionRequest task.verificationIntents.",
    "Do not create separate verificationResults for build/test/lint/runtime commands. Summarize those command outcomes under the allowed verificationResults[].summary and runtimeDeliveryEvidence.commandsRun.",
    "If no self-repair was attempted, selfRepairSummary must be exactly { attempted:false, attemptCount:0, stopReason:'not_attempted', progressObserved:false }.",
    "Use stopReason verification_passed only when self-repair was actually attempted and verification passed after that repair; then attempted must be true and attemptCount must be greater than 0.",
    "Never combine attempted=false with stopReason=verification_passed.",
    "Include executionContinuity. If any agent-owned long-running work may still be unreleased, use agentOwnedLongRunningWork:'unknown' and do not return status completed.",
  ];
}

function resumePolicyFor(repairType: RepairRequest["repairType"]): Record<string, unknown> {
  if (repairType === "architecture_artifact_repair") {
    return {
      onAcceptedReplacement: "regenerate_task_plan",
      supersedeCurrentArchitectureArtifact: true,
      supersedeCurrentTaskPlan: true,
      supersedeCurrentTaskPlanRun: true,
      rollbackWorkspace: false,
    };
  }
  if (repairType === "taskplan_repair") {
    return {
      onAcceptedReplacement: "create_new_task_plan_run",
      supersedeCurrentTaskPlan: true,
      supersedeCurrentTaskPlanRun: true,
      rollbackWorkspace: false,
    };
  }
  return {
    onAcceptedResult: "continue_original_task_plan_run",
    preservePreviousResults: true,
  };
}

async function requireInitialized(projectRoot: string): Promise<void> {
  if (!(await pathExists(path.join(projectRoot, ".loom", "config.json")))) {
    throw stateNotInitialized(projectRoot);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function createId(prefix: string): string {
  return `${prefix}-${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${createHash("sha1")
    .update(`${process.pid}:${Math.random()}:${Date.now()}`)
    .digest("hex")
    .slice(0, 8)}`;
}
