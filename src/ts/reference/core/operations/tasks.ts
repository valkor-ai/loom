import { createHash } from "node:crypto";
import path from "node:path";
import { ZodError } from "zod";
import { invalidArgument, noActivePlan, stateNotInitialized } from "../errors";
import {
  type ArchitectureArtifactContract,
  type ContractIssue,
  type PlanningGenerationContract,
  type Task,
  type TaskExecutionRequest,
  type TaskPlanGroupCandidate,
  type TaskPlanOutline,
  type TaskPlan,
  type TaskPlanAcceptResult,
  type TaskPlanGenerationRequest,
  type TaskPlanRun,
  type TaskResult,
  conceptEvidenceTypeSchema,
  taskResultStatusSchema,
  taskExecutionRequestSchema,
  taskPlanGroupCandidateSchema,
  taskPlanGenerationRequestSchema,
  taskPlanOutlineSchema,
  taskPlanRunSchema,
  taskPlanSchema,
  taskResultSchema,
  verificationEvidenceSchema,
} from "../contracts";
import type { RouteAction } from "../schemas";
import type { UserFacingLanguageConstraint } from "../schemas";
import { userFacingLanguageRule } from "../requirements/user-facing-language";
import { ensureDir, pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import { getActiveLocator, resolveLocator } from "../state/delivery";
import {
  type DeliveryPhaseLocator,
  taskExecutionRequestPath,
  taskExecutionResultCandidatePath,
  taskPlanCandidatePath,
  taskPlanGroupCandidatePath,
  taskPlanLatestPath,
  taskPlanOutlineCandidatePath,
  taskPlanPath as resolveTaskPlanPath,
  planningContractPath,
  architectureContractPath,
  taskPlanRequestPath,
  taskPlanRunLatestPath,
  taskPlanRunPath,
  taskResultPath,
  technicalBaselinePath,
  phaseConceptGroundingPath,
  toProjectRelative,
} from "../state/paths";
import { BLOCKED_OUTPUT_MAPPING, resetIssueCounter, validateTaskPlanCandidate, validateTaskResult } from "../validators";
import {
  loadArchitectureArtifact,
  loadPlanningContract,
  loadRequiredTechnicalBaseline,
} from "./contracts";
import {
  closeOperationLease,
  createOperationLease,
  operationRef,
  readOperationLease,
  updateRouteState,
} from "./control";
import { autoRunInstruction, postRepairSubmitRouting, withAutoRunnableTransition } from "./routing-instructions";
import { artifactGenerationProtocolPolicy, artifactInstructionPolicy, artifactRepairPolicy, compactContextReadStep, taskExecutionOutputPolicy } from "./output-policy";
import { agentActionContract } from "./agent-action";
import { referencedArtifactReadGuide } from "./artifact-read-guide";
import { hydrateRequestManifest, writeRequestManifestAtomic } from "./request-manifest";
import { runtimeDeliveryClosureRequirementContract, type RuntimeDeliveryClosureRequirementContract } from "../runtime-delivery-closure";
import { runtimeForegroundProbeCloseoutRules } from "./runtime-stall";
import {
  buildWorkflowClosureRequirements,
  closureRequirementsForTask,
  type WorkflowClosureRequirement,
} from "../workflow-closure";

const TASKPLAN_WORKFLOW_CLOSURE_REQUIREMENTS_SOURCE_PATH = "contextProjection.requirementDetailTransfer.workflowClosureRequirements";
const TASK_EXECUTION_WORKFLOW_CLOSURE_DETAIL_SOURCES = [
  "sourceRefs.architectureArtifactContractRef#/frontendExperience",
  "sourceRefs.architectureArtifactContractRef#/userFlows",
  "sourceRefs.architectureArtifactContractRef#/interfaces",
  "sourceRefs.architectureArtifactContractRef#/stateMachines",
];

export type CreateTaskPlanRequestInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  planningContractId?: string;
  architectureArtifactContractId?: string;
};

export type AcceptTaskPlanInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  candidateFile?: string;
  requestId?: string;
  repairId?: string;
};

export type NextTaskInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  taskPlanRunId?: string;
};

export type RecordTaskResultInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  inputFile: string;
  taskPlanRunId?: string;
};

type TaskPlanRunSummary = Pick<TaskPlanRun, "runId" | "taskPlanId" | "status" | "summary" | "nextAction" | "updatedAt"> & {
  currentTask: {
    taskId: string;
    groupId?: string;
    status: string;
  } | null;
};

type TaskExecutionRequestSummary = {
  requestId: string;
  requestType: TaskExecutionRequest["requestType"];
  taskId: string;
  groupId?: string;
  taskPlanRunId: string;
  title: string;
  taskKind: string;
  acceptanceRefs: string[];
  resultFile: unknown;
  submitCommand: unknown;
  requestRef: string;
};

const taskExecutionCompletionBarrierRules = [
  "A TaskExecutionRequest is not complete until TaskResult JSON exists at outputContract.resultFile and submitCommand has been run successfully.",
  "Do not send progress-only summaries, interim handoff notes, or next-step summaries before submitCommand succeeds.",
  "If implementation or verification cannot be completed inside the current task boundary, still write a failed or blocked TaskResult and run submitCommand so loom can route repair or user decision.",
  "Only stop before submitCommand when requestRef cannot be read, submitCommand returns a non-repairable failure, or the returned instruction is user-gated.",
];

export function taskExecutionCompletionContinuityRequirement(): Record<string, unknown> {
  return {
    rule: "Verification method is agent-chosen, but it must return control before TaskResult submission.",
    forbiddenOutcome: "Do not leave the task waiting on any long-running command, browser session, interactive tool, server, watcher, worker, progress summary, or handoff note before record-result.",
    requiredCloseout: "Write TaskResult and run submitCommand in this same task turn unless a declared stopCondition is reached.",
    taskResultField: "executionContinuity",
    statusRule: "If agent-owned long-running work was started and its release state is unknown, use completed_with_notes with notes unless an independent failure or blocked condition remains.",
  };
}

export const verificationCommandSchedulingRules = [
  "Run verification commands serially by default. Only read-only inspection commands may be parallelized.",
  "Do not issue multiple tool calls in the same assistant response for commands that may install dependencies, build artifacts, run tests, start or probe runtimes, clean outputs, generate code, format files, mutate caches, or write files.",
  "For write-producing verification commands, run exactly one command, wait for it to finish, inspect the result, then decide the next command.",
  "Treat commands as write-producing when unsure. Examples include install, build, clean, test, e2e, lint with cache/fix, format with write, codegen, dev/start/preview servers, runtime checks, and any command that may write node_modules, dist, build, coverage, .cache, test-results, playwright-report, logs, or lockfiles.",
  "When a temporary runtime is running for a bounded probe, run only the readiness/HTTP/browser probes against that runtime until cleanup is complete; do not run build, clean, test, install, or another runtime command against the same workspace at the same time.",
  "Record verification commands in TaskResult in the actual order they completed.",
];

export const controlledRuntimeProbeRules = [
  "Never run long-lived runtime/server commands as foreground blocking verification commands. This applies to any technology stack command that listens on a port, serves requests, watches files, opens a preview/dev server, starts a worker, or otherwise keeps the process alive.",
  "Examples are non-exhaustive: npm/pnpm/yarn start or dev or preview, node/deno/bun server entries, python/uvicorn/gunicorn/flask/django servers, ruby/rails servers, java/spring boot runs, go/rust compiled HTTP servers, dotnet run, php artisan serve, static preview servers, database emulators, queues, and worker processes.",
  "If a runtime/server probe is needed, start only a task-owned temporary runtime in the background with a bounded readiness/probe window, record pid/port/command when available, run the probe, then stop that task-owned runtime before writing TaskResult.",
  ...runtimeForegroundProbeCloseoutRules,
  "Do not leave the task waiting on server stdout or an open foreground process. A server that keeps running is not a completed verification command.",
  "If the environment or shell tooling cannot safely start, probe, and clean up a temporary runtime, skip the live probe and record static/code-level evidence plus runtimeDeliveryEvidence.unverifiedItems or completed_with_notes.",
  "Runtime probe cleanup failure, unknown cleanup, or not-safe cleanup is non-blocking by itself: record runtimeDeliveryEvidence.runtimeProbeCleanup and use completed_with_notes unless an independent implementation or verification defect remains.",
];

export const frontendModuleEntryPreservationRule = "When adding a frontend module to an existing app, add it as a new reachable entry, route, tab, or navigation item in the existing app shell; do not replace, hide, or remove existing reachable module entries unless the requirement explicitly asks for that replacement or removal.";

export const frontendImplementationOrganizationRules = [
  "For frontend tasks, organize implementation by responsibility boundaries rather than by one giant mixed file.",
  "Use the project's existing frontend structure when present.",
  frontendModuleEntryPreservationRule,
  "For frontend tasks, follow sourceContext.userFacingLanguage or executionRules.userFacingLanguage for user-visible copy. Do not translate code identifiers, API paths, database fields, enum values, package names, framework terms, or internal artifact ids.",
  "When creating new frontend code, choose the smallest maintainable structure appropriate to the detected stack.",
  "The implementation should make UI/view, API/service interaction, state/feedback handling, and verification/test evidence distinguishable.",
  "Do not force every responsibility into a separate directory or file when the task is small.",
  "Do not collapse multiple frontend responsibilities into an unmaintainable single blob.",
];

export function interactiveVerificationProbePolicy(): Record<string, unknown> {
  return {
    appliesWhen: "The agent chooses browser, e2e, interactive UI, runtime UI, or API-backed UI interaction verification.",
    purpose: "Preserve browser/e2e verification quality by decomposing it into task-scoped bounded probes before tool execution.",
    deriveProbePlanFrom: [
      "task.verificationIntents[].behavior",
      "task.frontendExperienceRequirement.executionGuidance.surfacesInScope",
      "task.frontendExperienceRequirement.executionGuidance.workflowsInScope",
      "task.frontendExperienceRequirement.executionGuidance.interactionStatesExpected",
      "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
      "task.runtimeDeliveryRequirement.requiredCodeLevelChecks",
    ],
    requiredExecutionPattern: [
      "Before running browser/e2e/interactive code, derive the smallest applicable probe plan from the listed task fields.",
      "Select only probes that match the current task responsibility; do not run probes for absent surfaces, workflows, forms, actions, bindings, or runtime checks.",
      "Each probe must verify exactly one interaction target: one verification intent, one workflow step, one user action, one frontend/backend binding, or one runtime check.",
      "Each probe must be bounded, must return before the next probe starts, and must produce an observable result such as reachable page, visible state, response status, result message, list/detail change, or state transition.",
      "Do not bundle multiple business workflows into one browser/e2e script.",
      "All completed probes together must cover every verification intent assigned to the current task.",
    ],
    failureProgressRule: [
      "When a probe fails, the next attempt must be smaller, more specific, reset the tool context, or change the failure condition.",
      "Continue while new observable evidence appears or the failure signature changes.",
      "Stop retrying that verification method only when the same failure signature repeats without new observable evidence.",
      "After stopping a verification method, close out with the existing TaskResult status, verificationResults, notes, and runtimeDeliveryEvidence rules; do not invent browser-specific TaskResult fields.",
    ],
    taskResultEvidence: [
      "Record successful probe facts in verificationResults[].summary for the matching verificationId.",
      "Record runtime command or probe evidence in runtimeDeliveryEvidence.commandsRun when runtimeDeliveryEvidence applies.",
      "Record remaining unverified responsibility in notes or runtimeDeliveryEvidence.unverifiedItems according to the existing TaskResult rules.",
    ],
  };
}

export function sourceEditPreparationContract(input: {
  resultFile: string;
  submitCommandName: string;
  artifactWriteTarget?: string;
}): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    contractKind: "source_edit_preparation",
    authority: "TaskExecutionRequest.executionRules.sourceEditPreparationContract",
    appliesTo: [
      "project_source_edits",
      "loom_result_artifact_writes",
    ],
    resultFile: input.resultFile,
    artifactWriteTarget: input.artifactWriteTarget ?? input.resultFile,
    submitCommandName: input.submitCommandName,
    requiredWritePlanFields: {
      targetPath: "Concrete project-relative or absolute file path to create, replace, edit, or write as result artifact.",
      writeKind: ["create", "replace", "edit", "multi_edit", "artifact_result"],
      contentBasis: [
        "task.objective",
        "task.acceptanceRefs/sourceContext.acceptanceSnapshot",
        "task.conceptResponsibilities/taskConceptGrounding",
        "task.frontendExperienceRequirement when present",
        "task.runtimeDeliveryRequirement when present",
        "current source file contents for source edits",
        "verification or self-repair feedback when repairing a failed check",
      ],
      writeMethod: ["native_file_tool", "quiet_programmatic_write"],
      writePayloadReady: "true only when complete file content or a complete edit set has been formed before invoking the write method.",
    },
    sequence: [
      {
        step: 1,
        name: "collect_required_context",
        requiredAction: "Read the required requestReadPlan.groups and the current source files that will be changed before deciding edit content.",
      },
      {
        step: 2,
        name: "form_write_plan",
        requiredAction: "Build an internal write plan containing targetPath, writeKind, contentBasis, writeMethod, and writePayloadReady=true.",
      },
      {
        step: 3,
        name: "execute_write",
        requiredAction: "Invoke a native file write/edit tool or a quiet programmatic write only after the write plan is complete.",
      },
      {
        step: 4,
        name: "recover_write_tool_validation_failure",
        requiredAction: "If the write/edit tool rejects missing or invalid path/content/edit arguments before writing, return to form_write_plan, rebuild complete arguments, retry with valid native tool arguments, then switch to quiet programmatic write if native validation fails again.",
      },
      {
        step: 5,
        name: "terminal_uncertain_write_boundary",
        requiredAction: `After required context and source inspection are complete, if targetPath or write payload still cannot be determined within this task boundary, write a failed or blocked TaskResult to ${input.resultFile} and run ${input.submitCommandName}.`,
      },
    ],
    forbiddenOutcomes: [
      "Do not invoke native write/edit tools with missing path, content, or edit arguments.",
      "Do not repeat a malformed write/edit tool call.",
      "Do not begin a write while writePayloadReady is false.",
      "Do not ask the user how to continue while resultFile is missing and the uncertainty can be represented as a failed or blocked TaskResult.",
      "Do not stop with a progress-only summary after source edits; write resultFile and run submitCommand.",
    ],
  };
}

const taskResultRequiredTopLevelFields = [
  "schemaVersion",
  "taskResultId",
  "taskId",
  "taskPlanId",
  "status",
  "changedFiles",
  "noChangeReason",
  "verificationResults",
  "selfRepairSummary",
  "failure",
  "executionContinuity",
  "notes",
  "requirementDetailEvidence",
  "blockedReasons",
  "createdAt",
  "updatedAt",
];

type ExecuteTaskInstructionInput = {
  requestRef: string;
  request: TaskExecutionRequest;
  recovery: boolean;
};

export async function createTaskPlanRequest(input: CreateTaskPlanRequestInput): Promise<{
  request: TaskPlanGenerationRequest;
  requestPath: string;
  lease: ReturnType<typeof operationRef>;
  instruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const activeLease = await readOperationLease(root, locator.deliveryId);
  if (activeLease?.status === "active" && new Date(activeLease.expiresAt).getTime() > Date.now()) {
    if (activeLease.operationType === "taskplan_generation" && activeLease.phaseId === locator.phaseId) {
      const existingRequestRef = typeof activeLease.refs.requestRef === "string" ? activeLease.refs.requestRef : null;
      if (existingRequestRef && await pathExists(path.join(root, existingRequestRef))) {
        const existingRequest = taskPlanGenerationRequestSchema.parse(await hydrateRequestManifest(root, path.join(root, existingRequestRef)));
        return {
          request: existingRequest,
          requestPath: existingRequestRef,
          lease: operationRef(activeLease),
          instruction: taskPlanGenerationInstruction(existingRequestRef, existingRequest, {
            recovery: true,
            userMessage: "TaskPlanGenerationRequest is already active. Generate the existing request outputs and submit them; do not create another request.",
          }),
        };
      }
    }
    throw invalidArgument("Another loom operation is already active.", {
      operationId: activeLease.operationId,
      operationType: activeLease.operationType,
      expiresAt: activeLease.expiresAt,
    });
  }
  const baseline = await loadRequiredTechnicalBaseline(root, locator);
  const pgc = await loadPlanningContract(root, input.planningContractId, locator);
  const aac = await loadArchitectureArtifact(root, input.architectureArtifactContractId, locator);
  const requestId = createId("taskplan-gen");
  const outlineFile = toProjectRelative(root, taskPlanOutlineCandidatePath(root, locator, requestId));
  const groupFilePattern = toProjectRelative(root, taskPlanGroupCandidatePath(root, locator, requestId, "{groupId}"));
  const requirementDetailTransfer = taskPlanRequirementDetailProjection(pgc, aac);
  const submitCommand = {
    name: "task-plan accept",
    argv: [
      "task-plan",
      "accept",
      "--delivery-id",
      locator.deliveryId,
      "--phase-id",
      locator.phaseId,
      "--request-id",
      requestId,
    ],
  };
  const request: TaskPlanGenerationRequest = {
    schemaVersion: "1.0",
    requestId,
    requestType: "taskplan_grouped_generation",
    agentAction: agentActionContract({
      actionKind: "generate_taskplan_grouped",
      instruction: "Generate TaskPlan grouped outputs. First write outputContract.outlineFile, then one group file per outline.groups[].groupId using outputContract.groupFilePattern, then run submitCommand exactly.",
      read: {
        required: [
          "this request",
          "sourceRefs",
          "fieldAccessHints",
          "contextProjection.requirementDetailTransfer",
          "allowedRefs",
          "generationRules.requirementDetailTransferRules",
          "generationRules.groupedOutputRules",
          "generationRules.scopeAndReferenceRules",
          "generationRules.writeBoundaryRules",
          "generationRules.verificationEvidenceRules",
          "generationRules.conceptGroundingRules",
          "generationRules.frontendExperienceRules.required",
          "generationRules.frontendExperienceRules.rules",
          "generationRules.workflowClosureRules.derivationAuthority",
          "generationRules.workflowClosureRules.appliesWhen",
          "generationRules.workflowClosureRules.taskAssignmentRule",
          "generationRules.workflowClosureRules.taskCoverageShape",
          "generationRules.workflowClosureRules.resultExpectation",
          "generationRules.workflowClosureRules.repairRule",
          "generationRules.workflowClosureRules.rules",
          "generationRules.runtimeDeliveryRules.status",
          "generationRules.runtimeDeliveryRules.closureRequirement",
          "generationRules.runtimeDeliveryRules.closureTaskTemplate",
          "generationRules.runtimeDeliveryRules.taskPlanningGuidance",
          "generationRules.runtimeDeliveryRules.rules",
          "enumRefs",
          "outputContract.outlineFile",
          "outputContract.groupFilePattern",
          "outputContract.pathAuthority",
          "outputContract.outlineSchemaShape",
          "outputContract.groupSchemaShape",
          "outputContract.runtimeDeliveryClosureRequirement",
          "outputContract.runtimeDeliveryClosureTaskTemplate",
        ],
        optional: [
          "referencedArtifactReadGuide",
          "outputContract.frontendExperienceProjection",
          "outputContract.runtimeDeliveryProjection",
        ],
        fieldGroups: [
          {
            groupId: "generate_taskplan_grouped_core",
            required: true,
            purpose: "TaskPlan generation authority: current source refs, selector hints, current phase requirement-detail transfer, and allowed refs.",
            whenToRead: "Read first before writing the outline or any group file.",
            fields: [
              "sourceRefs",
              "fieldAccessHints",
              "contextProjection.requirementDetailTransfer",
              "allowedRefs",
            ],
            readCommand: {
              name: "inspect",
              argv: ["inspect", "--request", "{requestRef}", "--field", "sourceRefs,fieldAccessHints,contextProjection.requirementDetailTransfer,allowedRefs"],
            },
            fallbackRule: "If grouped inspect fails, read these exact fields through requestManifest refs and targeted selectors. Do not print full TaskPlan or AAC artifacts.",
          },
          {
            groupId: "generate_taskplan_grouped_rules",
            required: true,
            purpose: "TaskPlan generation rules without duplicate large requirement payloads. Workflow closure requirements themselves live in contextProjection.requirementDetailTransfer.workflowClosureRequirements.",
            whenToRead: "Read after core authority and before deciding groups, task responsibilities, workflow closure assignment, evidence, and runtime/frontend task shape.",
            fields: [
              "generationRules.requirementDetailTransferRules",
              "generationRules.groupedOutputRules",
              "generationRules.scopeAndReferenceRules",
              "generationRules.writeBoundaryRules",
              "generationRules.verificationEvidenceRules",
              "generationRules.conceptGroundingRules",
              "generationRules.frontendExperienceRules.required",
              "generationRules.frontendExperienceRules.rules",
              "generationRules.workflowClosureRules.derivationAuthority",
              "generationRules.workflowClosureRules.requirementSource",
              "generationRules.workflowClosureRules.appliesWhen",
              "generationRules.workflowClosureRules.taskAssignmentRule",
              "generationRules.workflowClosureRules.taskCoverageShape",
              "generationRules.workflowClosureRules.resultExpectation",
              "generationRules.workflowClosureRules.repairRule",
              "generationRules.workflowClosureRules.rules",
              "generationRules.runtimeDeliveryRules.status",
              "generationRules.runtimeDeliveryRules.closureRequirement",
              "generationRules.runtimeDeliveryRules.closureTaskTemplate",
              "generationRules.runtimeDeliveryRules.taskPlanningGuidance",
              "generationRules.runtimeDeliveryRules.rules",
            ],
            readCommand: {
              name: "inspect",
              argv: ["inspect", "--request", "{requestRef}", "--field", "generationRules.requirementDetailTransferRules,generationRules.groupedOutputRules,generationRules.scopeAndReferenceRules,generationRules.writeBoundaryRules,generationRules.verificationEvidenceRules,generationRules.conceptGroundingRules,generationRules.frontendExperienceRules.required,generationRules.frontendExperienceRules.rules,generationRules.workflowClosureRules.derivationAuthority,generationRules.workflowClosureRules.requirementSource,generationRules.workflowClosureRules.appliesWhen,generationRules.workflowClosureRules.taskAssignmentRule,generationRules.workflowClosureRules.taskCoverageShape,generationRules.workflowClosureRules.resultExpectation,generationRules.workflowClosureRules.repairRule,generationRules.workflowClosureRules.rules,generationRules.runtimeDeliveryRules.status,generationRules.runtimeDeliveryRules.closureRequirement,generationRules.runtimeDeliveryRules.closureTaskTemplate,generationRules.runtimeDeliveryRules.taskPlanningGuidance,generationRules.runtimeDeliveryRules.rules"],
            },
            fallbackRule: "If grouped inspect fails, read only these generationRules subfields. Do not read the whole generationRules object.",
          },
          {
            groupId: "generate_taskplan_grouped_candidate_contract",
            required: true,
            purpose: "TaskPlan output files, schema shapes, enum values, and runtime closure contract required before writing candidates.",
            whenToRead: "Read before writing outputContract.outlineFile or any group file.",
            fields: [
              "enumRefs",
              "outputContract.outlineFile",
              "outputContract.groupFilePattern",
              "outputContract.pathAuthority",
              "outputContract.outlineSchemaShape",
              "outputContract.groupSchemaShape",
              "outputContract.runtimeDeliveryClosureRequirement",
              "outputContract.runtimeDeliveryClosureTaskTemplate",
            ],
            readCommand: {
              name: "inspect",
              argv: ["inspect", "--request", "{requestRef}", "--field", "enumRefs,outputContract.outlineFile,outputContract.groupFilePattern,outputContract.pathAuthority,outputContract.outlineSchemaShape,outputContract.groupSchemaShape,outputContract.runtimeDeliveryClosureRequirement,outputContract.runtimeDeliveryClosureTaskTemplate"],
            },
            fallbackRule: "If grouped inspect fails, read only these outputContract and enumRefs fields through requestManifest refs.",
          },
          {
            groupId: "generate_taskplan_grouped_optional_context",
            required: false,
            purpose: "Fallback selector guide and full frontend/runtime projections for cases where the core projection is insufficient.",
            whenToRead: "Read only when a required group points to missing selector detail or when the taskplan cannot assign frontend/runtime responsibilities from the core projection.",
            fields: [
              "referencedArtifactReadGuide",
              "outputContract.frontendExperienceProjection",
              "outputContract.runtimeDeliveryProjection",
            ],
            readCommand: {
              name: "inspect",
              argv: ["inspect", "--request", "{requestRef}", "--field", "referencedArtifactReadGuide,outputContract.frontendExperienceProjection,outputContract.runtimeDeliveryProjection"],
            },
            fallbackRule: "Use this group as a correctness fallback, not as the default first read.",
          },
        ],
        displayPolicy: "compact",
      },
      write: {
        outlineFile,
        groupFilePattern,
        blockedFile: outlineFile.replace(/outline\.json$/, "blocked.json"),
        rules: [
          "This TaskPlanGenerationRequest is auto-runnable. Do not stop after request creation; generate the grouped outputs and submit them now.",
          "Use enumRefs.implementationAction exactly; do not invent natural-language implementationActions.",
          "Use enumRefs.verificationEvidence exactly; map AAC verification hints through generationRules.verificationEvidenceRules.",
          "Use sourceRefs.phaseConceptGroundingRef as the only path for current phase concept grounding; do not guess historical concept grounding paths.",
          "For the runtime_delivery_closure task, copy outputContract.runtimeDeliveryClosureTaskTemplate.runtimeDeliveryRequirement exactly when present; do not rename checkId values, contractField values, or acceptableEvidence arrays.",
          "Use contextProjection.requirementDetailTransfer as the current phase requirement-detail authority for TaskPlan. Do not reduce detailed scope items, acceptance statements/sourceRefs/capabilityRefs, business flow details, AAC coverage, or AAC artifact refs to generic task labels.",
          "When contextProjection.requirementDetailTransfer.workflowClosureRequirements is non-empty, assign every closure requirement to TaskPlan tasks using generationRules.workflowClosureRules. These requirements are already derived from AAC user flows and interfaces; do not route them to AAC_INSUFFICIENT.",
          "Every task objective and verificationIntents[].behavior should carry the concrete rule, field, flow, state, UI, API, or blocking detail that the task is responsible for when such detail exists in contextProjection.requirementDetailTransfer.",
          "Assign covered current-phase requirement details using task.requirementDetailRefs and verificationIntents[].requirementDetailRefs. Use detailId values only from contextProjection.requirementDetailTransfer.requirementDetailAssignment.items[].detailId.",
          "Every task writeBoundary.artifactRefs should point to the AAC artifacts that carry those details. If a required current-phase detail has no AAC artifact to reference, write a blocked output with blockedReasonCode AAC_INSUFFICIENT instead of generating a vague task.",
          "Do not write final taskplan.json.",
          "Use only this request's outputContract.outlineFile and outputContract.groupFilePattern as current candidate output paths.",
          "Do not inspect, copy, or repair historical .loom tmp task-plan outputs from previous phases or previous request ids.",
          "Do not merge all work into one group unless outline has one genuinely tiny group.",
        ],
      },
      submit: {
        command: submitCommand,
        requiredArgs: ["--delivery-id", "--phase-id", "--request-id"],
        placeholders: {},
        runAfter: "outlineFile and all group files exist",
      },
      schema: {
        primary: "TaskPlanGroupedOutputs",
        shapeLocation: "outputContract.outlineSchemaShape and outputContract.groupSchemaShape",
        enumLocation: "enumRefs and generationRules.*Enum",
        allowedRefsLocation: "allowedRefs",
      },
      stopConditions: ["blockedOutput is required", "submitCommand returns non-repairable failure"],
    }),
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      ifBlockedWriteBlockedOutput: true,
      submitWithProvidedCommand: true,
      progressSignal: "candidate_files",
      heartbeatRequired: false,
      resumeViaContinue: true,
      currentRequestOutputOnly: true,
      ignoreHistoricalCandidateOutputs: true,
      historicalTaskPlanRefsAreAuditOnly: true,
      ...artifactGenerationProtocolPolicy(),
    },
    sourceRefs: {
      technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
      planningGenerationContractRef: toProjectRelative(root, planningContractPath(root, pgc.planningContractId, locator)),
      architectureArtifactContractRef: toProjectRelative(root, architectureContractPath(root, aac.architectureArtifactContractId, locator)),
      ...(pgc.contextRefs?.repositoryContextRef ? { repositoryContextRef: pgc.contextRefs.repositoryContextRef } : {}),
      ...(pgc.contextRefs?.phaseConceptGroundingRef ? { phaseConceptGroundingRef: pgc.contextRefs.phaseConceptGroundingRef } : {}),
      ...(pgc.contextRefs?.deliveryConceptGlossaryRef ? { deliveryConceptGlossaryRef: pgc.contextRefs.deliveryConceptGlossaryRef } : {}),
    },
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      technicalBaselineRef: toProjectRelative(root, technicalBaselinePath(root, locator.deliveryId)),
      planningGenerationContractRef: toProjectRelative(root, planningContractPath(root, pgc.planningContractId, locator)),
      architectureArtifactContractRef: toProjectRelative(root, architectureContractPath(root, aac.architectureArtifactContractId, locator)),
      repositoryContextRef: pgc.contextRefs?.repositoryContextRef,
      phaseConceptGroundingRef: pgc.contextRefs?.phaseConceptGroundingRef,
      deliveryConceptGlossaryRef: pgc.contextRefs?.deliveryConceptGlossaryRef,
    }),
    fieldAccessHints: {
      sourceRefs: "Use .sourceRefs for all source artifact paths. Do not guess .loom paths from phase id or older request ids.",
      phaseConceptGroundingRef: "Use .sourceRefs.phaseConceptGroundingRef when present. This is the current phase concept grounding path.",
      deliveryConceptGlossaryRef: "Use .sourceRefs.deliveryConceptGlossaryRef when present. This is delivery-wide concept glossary context, not a replacement for current phase grounding.",
      outputContract: "Use .outputContract.outlineFile and .outputContract.groupFilePattern for current output files.",
      runtimeDeliveryClosureTaskTemplate: "When present, copy .outputContract.runtimeDeliveryClosureTaskTemplate.runtimeDeliveryRequirement exactly into the runtime_delivery_closure task.",
      requirementDetailTransfer: "Use .contextProjection.requirementDetailTransfer to convert current phase PGC/AAC details into TaskPlan groups, tasks, artifactRefs, verificationIntents, frontendExperienceRequirement, and runtimeDeliveryRequirement.",
      workflowClosureRequirements: "Use .contextProjection.requirementDetailTransfer.workflowClosureRequirements when present. Each item is mechanically derived from AAC frontendExperience/userFlows/interfaces and must be assigned to a task that wires the user action to the declared interface and verifies the outcome.",
      taskPlanDetailSelectors: [
        ".contextProjection.requirementDetailTransfer.currentPhaseScope.included[].items",
        ".contextProjection.requirementDetailTransfer.acceptanceDetails[]",
        ".contextProjection.requirementDetailTransfer.businessFlowDetails[]",
        ".contextProjection.requirementDetailTransfer.objectOperationDetailRules",
        ".contextProjection.requirementDetailTransfer.workflowClosureRequirements[]",
        ".contextProjection.requirementDetailTransfer.requirementDetailAssignment",
        ".contextProjection.requirementDetailTransfer.architectureDetails",
        ".contextProjection.requirementDetailTransfer.architectureDetails.frontendOperationPathDetails",
        ".contextProjection.requirementDetailTransfer.taskPlanningFieldMapping",
      ],
      compactJqExamples: [
        ".sourceRefs",
        ".contextProjection.requirementDetailTransfer.acceptanceDetails[] | {id,statement,sourceRefs,capabilityRefs,aacCoverage}",
        ".contextProjection.requirementDetailTransfer.objectOperationDetailRules | {scopeCoverageRule,taskAssignmentRule,conceptBindingRule,evidenceRule}",
        ".contextProjection.requirementDetailTransfer.architectureDetails | {modules,interfaces,userFlows}",
        ".contextProjection.requirementDetailTransfer.requirementDetailAssignment | {items,assignmentRule,verificationRule}",
        ".contextProjection.requirementDetailTransfer.architectureDetails.frontendOperationPathDetails | {dataViews,actions,operationPaths}",
        ".contextProjection.requirementDetailTransfer.workflowClosureRequirements[] | {closureId,workflowRef,interfaceRefs,acceptanceRefs,requiredDataBindingMode,requiredEvidence}",
        ".sourceRefs.phaseConceptGroundingRef",
        ".referencedArtifactReadGuide[] | select(.refKey == \"phaseConceptGroundingRef\")",
        ".generationRules.conceptGroundingRules",
        ".generationRules.workflowClosureRules | {derivationAuthority,requirementSource,appliesWhen,taskAssignmentRule,taskCoverageShape,resultExpectation,repairRule,rules}",
        ".outputContract.runtimeDeliveryClosureTaskTemplate.runtimeDeliveryRequirement",
        ".outputContract | {outlineFile,groupFilePattern}",
      ],
    },
    contextProjection: {
      phaseId: locator.phaseId,
      planningContractId: pgc.planningContractId,
      architectureArtifactContractId: aac.architectureArtifactContractId,
      requirementDetailTransfer,
    },
    conceptGroundingSource: {
      phaseConceptGroundingRef: pgc.contextRefs?.phaseConceptGroundingRef ?? null,
      deliveryConceptGlossaryRef: pgc.contextRefs?.deliveryConceptGlossaryRef ?? null,
      authorityRule: "When assigning task conceptRefs, use only concepts from sourceRefs.phaseConceptGroundingRef for the current phase. deliveryConceptGlossaryRef is explanatory context only.",
      missingRefPolicy: "If phaseConceptGroundingRef is absent, omit task conceptRefs rather than guessing concept ids.",
    },
    allowedRefs: allowedRefsFrom(pgc, aac),
    generationRules: buildTaskGenerationRules(aac),
    validatorRulesSummary: {
      blockingChecks: [
        "source readiness",
        "scope refs",
        "acceptance refs",
        "artifact refs",
        "task DAG",
        "verification intents",
        "implementation intent conflicts",
        "frontend experience requirements",
        "frontend workflow closure requirements",
        "runtime delivery closure requirements",
      ],
    },
    enumRefs: taskPlanEnumRefs(),
    outputContract: {
      format: "json",
      schema: "TaskPlanGroupedOutputs",
      outlineFile,
      groupFilePattern,
      pathAuthority: {
        currentRequestOnly: true,
        currentPhaseId: locator.phaseId,
        currentRequestId: requestId,
        writeOnly: [outlineFile, groupFilePattern],
        auditOnlyHistoricalPatterns: [
          ".loom/deliveries/*/tmp/phase-*/task-plan/*",
          ".loom/deliveries/*/tasks/phase-*/requests/taskplan-gen-*.json",
        ],
        rule: "Only the paths in this outputContract belong to the current TaskPlan generation. Historical task-plan tmp/request files are audit-only and must not be read as inputs or reused.",
      },
      outlineSchemaShape: taskPlanOutlineSchemaShape(locator, requestId, pgc, aac),
      groupSchemaShape: taskPlanGroupSchemaShape(locator, requestId, aac),
      frontendExperienceProjection: aac.frontendExperience ?? null,
      workflowClosureRequirementRefs: workflowClosureRequirementSourceSummary(buildWorkflowClosureRequirements(aac), {
        sourcePath: TASKPLAN_WORKFLOW_CLOSURE_REQUIREMENTS_SOURCE_PATH,
        phase: "taskplan_generation",
      }),
      runtimeDeliveryProjection: runtimeDeliveryProjection(aac),
      runtimeDeliveryClosureRequirement: runtimeDeliveryClosureRequirement(aac),
      runtimeDeliveryClosureTaskTemplate: runtimeDeliveryClosureTaskTemplate(aac),
      returnCompleteReplacement: false,
    },
    submitCommand,
    blockedOutput: {
      status: "blocked",
      blockedReasonCode: "AAC_INSUFFICIENT",
      nextNode: "architecture_artifact_repair",
      candidateFile: toProjectRelative(root, taskPlanOutlineCandidatePath(root, locator, requestId).replace(/outline\.json$/, "blocked.json")),
    },
    createdAt: new Date().toISOString(),
  };
  const parsed = taskPlanGenerationRequestSchema.parse(request);
  const absolutePath = taskPlanRequestPath(root, requestId, locator);
  const lease = await createOperationLease({
    projectRoot: root,
    locator,
    operationType: "taskplan_generation",
    refs: {
      requestRef: toProjectRelative(root, absolutePath),
      outlineFile: parsed.outputContract.outlineFile,
      groupFilePattern: parsed.outputContract.groupFilePattern,
    },
  });
  try {
    await writeRequestManifestAtomic(root, absolutePath, parsed);
    await updateRouteState({
      projectRoot: root,
      locator,
      deliveryStatus: "planning",
      phaseStatus: "planning",
      latestRefs: {
        taskPlanRequestId: requestId,
        taskPlanRequest: toProjectRelative(root, absolutePath),
      },
      nextAction: {
        type: "taskplan_generation",
        source: "taskplan_request",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: toProjectRelative(root, absolutePath),
        reason: "TASKPLAN_REQUEST_CREATED",
        refs: {
          requestRef: toProjectRelative(root, absolutePath),
          outlineFile: parsed.outputContract.outlineFile,
          groupFilePattern: parsed.outputContract.groupFilePattern,
          activeOperationType: "taskplan_generation",
        },
      },
    });
  } catch (error) {
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "taskplan_generation",
      reason: "request_write_failed",
    });
    throw error;
  }
  return {
    request: parsed,
    requestPath: toProjectRelative(root, absolutePath),
    lease: operationRef(lease),
    instruction: taskPlanGenerationInstruction(toProjectRelative(root, absolutePath), parsed),
  };
}

export async function acceptTaskPlan(input: AcceptTaskPlanInput): Promise<TaskPlanAcceptResult> {
  await requireInitialized(input.projectRoot);
  resetIssueCounter();
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const candidateResult = input.requestId
    ? await assembleTaskPlanFromGroupedOutputs(root, locator, input.requestId)
    : { value: await readJsonFile(resolveCliPath(root, requireTaskPlanCandidateFile(input.candidateFile))), issues: [] };
  if (candidateResult.issues.length > 0 || !candidateResult.value) {
    const aac = await loadArchitectureArtifact(root, undefined, locator);
    const repairInstruction = buildTaskPlanRepairInstruction(null, candidateResult.issues, {
      requestId: input.requestId,
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      architectureArtifact: aac,
    });
    return {
      accepted: false,
      status: "needs_candidate_repair",
      taskPlanId: null,
      issues: candidateResult.issues,
      taskPlanPath: null,
      repairRequest: buildTaskPlanRepairRequest(null, candidateResult.issues, {
        requestId: input.requestId,
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        architectureArtifact: aac,
      }),
      instruction: repairInstruction ?? undefined,
      repairInstruction,
      run: null,
    };
  }
  const candidate = candidateResult.value;
  const candidateSource = typeof candidate === "object" && candidate !== null && "source" in candidate
    ? (candidate as { source?: { planningGenerationContractId?: unknown; architectureArtifactContractId?: unknown } }).source
    : undefined;
  const pgc = await loadPlanningContract(
    root,
    typeof candidateSource?.planningGenerationContractId === "string" ? candidateSource.planningGenerationContractId : undefined,
    locator,
  );
  const aac = await loadArchitectureArtifact(
    root,
    typeof candidateSource?.architectureArtifactContractId === "string" ? candidateSource.architectureArtifactContractId : undefined,
    locator,
  );
  const baseline = await loadRequiredTechnicalBaseline(root, locator);
  const validation = validateTaskPlanCandidate(candidate, pgc, aac, baseline);
  if (!validation.value || validation.status !== "ready" || validation.issues.length > 0) {
    const repairInstruction = buildTaskPlanRepairInstruction(validation.value, validation.issues, {
      requestId: input.requestId,
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      architectureArtifact: aac,
    });
    return {
      accepted: false,
      status: validation.status,
      taskPlanId: validation.value?.taskPlanId ?? null,
      issues: validation.issues,
      taskPlanPath: null,
      repairRequest: buildTaskPlanRepairRequest(validation.value, validation.issues, {
        requestId: input.requestId,
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        architectureArtifact: aac,
      }),
      instruction: repairInstruction ?? undefined,
      repairInstruction,
      run: null,
    };
  }

  const now = new Date().toISOString();
  normalizeTaskPlanWriteBoundaries(validation.value);
  const taskPlan: TaskPlan = taskPlanSchema.parse({
    ...validation.value,
    status: "ready",
    handoff: {
      ...validation.value.handoff,
      readyForExecution: true,
      nextNode: "task_execution",
      blockedReasons: [],
    },
    updatedAt: now,
  });
  const taskPlanFile = resolveTaskPlanPath(root, taskPlan.taskPlanId, locator);
  await writeJsonAtomic(taskPlanFile, taskPlan);
  await writeJsonAtomic(taskPlanLatestPath(root, locator), {
    schemaVersion: "1.0",
    taskPlanId: taskPlan.taskPlanId,
    taskPlanRef: toProjectRelative(root, taskPlanFile),
    updatedAt: now,
  });
  const run = createTaskPlanRun(taskPlan);
  const runFile = taskPlanRunPath(root, run.runId, locator);
  await writeJsonAtomic(runFile, run);
  await writeJsonAtomic(taskPlanRunLatestPath(root, locator), {
    schemaVersion: "1.0",
    taskPlanRunId: run.runId,
    runRef: toProjectRelative(root, runFile),
    taskPlanId: taskPlan.taskPlanId,
    updatedAt: now,
  });
  await updateRouteState({
    projectRoot: root,
    locator,
    deliveryStatus: "executing",
    phaseStatus: "ready_for_execution",
    latestRefs: {
      taskPlan: toProjectRelative(root, taskPlanFile),
      taskPlanRun: toProjectRelative(root, runFile),
      ...(input.requestId ? {
        taskPlanRequestId: input.requestId,
        taskPlanRequest: toProjectRelative(root, taskPlanRequestPath(root, input.requestId, locator)),
      } : {}),
    },
    nextAction: {
      type: "continue_execution",
      source: "task_plan",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      ref: toProjectRelative(root, runFile),
      reason: "TASKPLAN_READY",
      targetNode: "task_execution",
    },
  });
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "taskplan_generation",
    reason: "task_plan_accepted",
  });
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "taskplan_repair",
    reason: "taskplan_repair_accepted",
  });

  return {
    accepted: true,
    status: "ready",
    taskPlanId: taskPlan.taskPlanId,
    issues: [],
    taskPlanPath: toProjectRelative(root, taskPlanFile),
    repairRequest: null,
    run,
    instruction: autoRunInstruction({
      actionType: "continue_execution",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      reason: "TASKPLAN_READY",
      targetNode: "task_execution",
      argv: ["next-task", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "TaskPlan accepted. Continue immediately with the first execution task.",
    }),
    ...(input.repairId ? {
      postRepairSubmitRouting: postRepairSubmitRouting({
        source: "taskplan_repair",
        submitCommand: "task-plan accept",
        nextActionTypes: ["continue_execution"],
        repairedArtifact: { taskPlanId: taskPlan.taskPlanId, runId: run.runId },
      }),
    } : {}),
  };
}

function normalizeTaskPlanWriteBoundaries(taskPlan: TaskPlan): void {
  for (const task of taskPlan.tasks) {
    if (
      task.writeBoundary.forbiddenPaths.length !== 1 ||
      task.writeBoundary.forbiddenPaths[0] !== ".loom"
    ) {
      task.writeBoundary.forbiddenPaths = [".loom"];
    }
  }
}

export async function getNextTask(input: NextTaskInput): Promise<{
  hasTask: boolean;
  reason: string | null;
  task: Task | null;
  run: TaskPlanRunSummary | null;
  executionRequest: TaskExecutionRequestSummary | null;
  executionRequestPath: string | null;
  instruction?: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const taskPlan = await loadCurrentTaskPlan(root, undefined, locator);
  const run = await loadCurrentTaskPlanRun(root, input.taskPlanRunId, locator);
  return materializeNextTaskExecution(root, locator, taskPlan, run);
}

async function materializeNextTaskExecution(
  root: string,
  locator: DeliveryPhaseLocator,
  taskPlan: TaskPlan,
  run: TaskPlanRun,
): Promise<{
  hasTask: boolean;
  reason: string | null;
  task: Task | null;
  run: TaskPlanRunSummary | null;
  executionRequest: TaskExecutionRequestSummary | null;
  executionRequestPath: string | null;
  instruction?: Record<string, unknown>;
}> {
  await closeCompletedTaskExecutionLease(root, locator, run);
  const running = run.taskStates.find((state) => state.status === "running");
  const taskState = running ?? findReadyTaskState(run);
  if (!taskState) {
    return {
      hasTask: false,
      reason: run.status === "completed" || run.status === "completed_with_notes" ? "RUN_READY_FOR_REVIEW" : "NO_READY_TASK",
      task: null,
      run: summarizeRun(run),
      executionRequest: null,
      executionRequestPath: null,
    };
  }

  const now = new Date().toISOString();
  if (taskState.status === "pending") {
    const pendingTask = taskPlan.tasks.find((item) => item.taskId === taskState.taskId);
    if (!pendingTask) {
      throw invalidArgument("Task state references a task not present in TaskPlan.", { taskId: taskState.taskId });
    }
    taskState.status = "running";
    taskState.startedAt = now;
    const groupState = run.groupStates.find((group) => group.groupId === taskState.groupId);
    if (groupState && groupState.status === "pending") {
      groupState.status = "running";
      groupState.startedAt = now;
    }
    run.status = "running";
    run.scheduler.startedAt = run.scheduler.startedAt ?? now;
    run.updatedAt = now;
    updateRunSummaryAndNextAction(run);
    await saveTaskPlanRun(root, run, locator);
  }

  const task = taskPlan.tasks.find((item) => item.taskId === taskState.taskId);
  if (!task) {
    throw invalidArgument("Task state references a task not present in TaskPlan.", { taskId: taskState.taskId });
  }
  const activeLease = await readOperationLease(root, locator.deliveryId);
  if (
    activeLease?.status === "active" &&
    activeLease.operationType === "task_execution" &&
    activeLease.refs.taskId === task.taskId &&
    activeLease.refs.taskPlanRunId === run.runId &&
    typeof activeLease.refs.requestRef === "string" &&
    await pathExists(path.join(root, activeLease.refs.requestRef))
  ) {
    const existingRequest = taskExecutionRequestSchema.parse(await hydrateRequestManifest(root, path.join(root, activeLease.refs.requestRef)));
    const refreshedRequest = taskExecutionRequestSchema.parse({
      ...await buildTaskExecutionRequest(root, locator, taskPlan, run, task, {
        requestId: existingRequest.requestId,
        resultFile: typeof activeLease.refs.resultFile === "string"
          ? activeLease.refs.resultFile
          : String(existingRequest.outputContract.resultFile),
      }),
      operation: operationRef(activeLease),
    });
    await writeRequestManifestAtomic(root, path.join(root, activeLease.refs.requestRef), refreshedRequest);
    await ensureTaskResultParentDir(root, refreshedRequest.outputContract.resultFile);
    await syncRouteForTaskExecutionRequest(root, locator, run, {
      requestRef: activeLease.refs.requestRef,
      resultFile: typeof refreshedRequest.outputContract.resultFile === "string"
        ? refreshedRequest.outputContract.resultFile
        : null,
      taskId: task.taskId,
      groupId: task.groupId,
      taskPlanRunId: run.runId,
    });
    return {
      hasTask: true,
      reason: null,
      task,
      run: summarizeRun(run),
      executionRequest: summarizeExecutionRequest(refreshedRequest, activeLease.refs.requestRef),
      executionRequestPath: activeLease.refs.requestRef,
      instruction: executeTaskInstruction({
        requestRef: activeLease.refs.requestRef,
        request: refreshedRequest,
        recovery: true,
      }),
    };
  }
  const request = await buildTaskExecutionRequest(root, locator, taskPlan, run, task);
  const requestFile = taskExecutionRequestPath(root, request.requestId, locator);
  let lease: Awaited<ReturnType<typeof createOperationLease>>;
  try {
    lease = await createOperationLease({
      projectRoot: root,
      locator,
      operationType: "task_execution",
      refs: {
        requestRef: toProjectRelative(root, requestFile),
        resultFile: request.outputContract.resultFile,
        taskId: task.taskId,
        taskPlanRunId: run.runId,
      },
    });
  } catch (error) {
    if (running === undefined) {
      taskState.status = "pending";
      taskState.startedAt = null;
      updateRunSummaryAndNextAction(run);
      await saveTaskPlanRun(root, run, locator);
    }
    throw error;
  }
  const requestWithOperation = taskExecutionRequestSchema.parse({
    ...request,
    operation: operationRef(lease),
  });
  await writeRequestManifestAtomic(root, requestFile, requestWithOperation);
  await ensureTaskResultParentDir(root, requestWithOperation.outputContract.resultFile);
  const executionRequestPathRef = toProjectRelative(root, requestFile);
  await syncRouteForTaskExecutionRequest(root, locator, run, {
    requestRef: executionRequestPathRef,
    resultFile: typeof requestWithOperation.outputContract.resultFile === "string"
      ? requestWithOperation.outputContract.resultFile
      : null,
    taskId: task.taskId,
    groupId: task.groupId,
    taskPlanRunId: run.runId,
  });
  return {
    hasTask: true,
    reason: null,
    task,
    run: summarizeRun(run),
    executionRequest: summarizeExecutionRequest(requestWithOperation, executionRequestPathRef),
    executionRequestPath: executionRequestPathRef,
    instruction: executeTaskInstruction({
      requestRef: executionRequestPathRef,
      request: requestWithOperation,
      recovery: running !== undefined,
    }),
  };
}

async function syncRouteForTaskExecutionRequest(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  run: TaskPlanRun,
  refs: {
    requestRef: string;
    resultFile: string | null;
    taskId: string;
    groupId: string;
    taskPlanRunId: string;
  },
): Promise<void> {
  const routeAction = routeActionFromRun(run, locator);
  if (!routeAction || routeAction.type !== "continue_execution") {
    return;
  }
  await updateRouteState({
    projectRoot,
    locator,
    deliveryStatus: deliveryStatusForRun(run),
    phaseStatus: phaseStatusForRun(run),
    latestRefs: {},
    nextAction: {
      ...routeAction,
      ref: refs.requestRef,
      refs: {
        ...(routeAction.refs ?? {}),
        taskId: refs.taskId,
        groupId: refs.groupId,
        taskPlanRunId: refs.taskPlanRunId,
        executionRequestRef: refs.requestRef,
        resultFile: refs.resultFile,
        activeOperationType: "task_execution",
      },
    },
  });
}

export async function materializeExecutionRepairTask(input: {
  projectRoot: string;
  locator: DeliveryPhaseLocator;
  targetTaskIds: string[];
  repairRequestId: string;
}): Promise<Awaited<ReturnType<typeof materializeNextTaskExecution>>> {
  const taskPlan = await loadCurrentTaskPlan(input.projectRoot, undefined, input.locator);
  const run = await loadCurrentTaskPlanRun(input.projectRoot, undefined, input.locator);
  const targetTaskId = input.targetTaskIds.find((taskId) =>
    run.taskStates.some((state) => state.taskId === taskId),
  );
  if (!targetTaskId) {
    throw invalidArgument("Execution repair requires at least one target task from the current run.", {
      repairRequestId: input.repairRequestId,
      targetTaskIds: input.targetTaskIds,
    });
  }
  const now = new Date().toISOString();
  const targetState = run.taskStates.find((state) => state.taskId === targetTaskId);
  if (!targetState) {
    throw invalidArgument("Execution repair target task does not exist in current run.", {
      repairRequestId: input.repairRequestId,
      targetTaskId,
    });
  }
  targetState.status = "running";
  targetState.startedAt = now;
  targetState.finishedAt = null;
  const groupState = run.groupStates.find((group) => group.groupId === targetState.groupId);
  if (groupState) {
    groupState.status = "running";
    groupState.finishedAt = null;
    groupState.startedAt = groupState.startedAt ?? now;
  }
  run.status = "running";
  run.scheduler.finishedAt = null;
  run.nextAction = {
    type: "continue_execution",
    reason: "EXECUTION_REPAIR_TASK_REOPENED",
    sourceTaskId: targetTaskId,
    targetNode: "task_execution",
  };
  run.updatedAt = now;
  updateRunSummaryAndNextAction(run, false);
  await saveTaskPlanRun(input.projectRoot, run, input.locator);
  const materialized = await materializeNextTaskExecution(input.projectRoot, input.locator, taskPlan, run);
  if (!materialized.hasTask || !materialized.instruction) {
    throw invalidArgument("Execution repair could not materialize a TaskExecutionRequest.", {
      repairRequestId: input.repairRequestId,
      targetTaskId,
    });
  }
  return materialized;
}

async function directInstructionForRunNextAction(
  root: string,
  locator: DeliveryPhaseLocator,
  taskPlan: TaskPlan,
  run: TaskPlanRun,
): Promise<{
  instruction?: Record<string, unknown>;
  materializedNextTask?: Awaited<ReturnType<typeof materializeNextTaskExecution>>;
}> {
  if (run.nextAction?.type !== "continue_execution") {
    return {
      instruction: instructionForRunNextAction(locator, run.nextAction),
    };
  }

  const nextTask = await materializeNextTaskExecution(root, locator, taskPlan, run);
  if (nextTask.hasTask && nextTask.instruction) {
    return {
      instruction: withAutoRunnableTransition({
        ...nextTask.instruction,
        routingRule: "The next TaskExecutionRequest has already been created. Execute it now; do not run next-task or loom continue before submitting its TaskResult.",
        userMessage: "TaskResult recorded. Next TaskExecutionRequest is already created; execute it now.",
      }, {
        sourceCommand: "record-result",
        sourceSummary: "TaskResult was recorded and the next TaskExecutionRequest was created.",
        primaryAction: "execute_materialized_next_task",
        mustStartImmediately: true,
        userVisibleSummary: "TaskResult was recorded and the next TaskExecutionRequest is already created. Continue immediately: read instruction.requestRef, execute the next task now, write instruction.resultFile, and run instruction.submitCommand.",
      }),
      materializedNextTask: nextTask,
    };
  }

  return {
    instruction: instructionForRunNextAction(locator, run.nextAction),
    materializedNextTask: nextTask,
  };
}

async function syncRouteFromMaterializedTask(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  run: TaskPlanRun,
  materializedNextTask: Awaited<ReturnType<typeof materializeNextTaskExecution>> | undefined,
): Promise<void> {
  if (!materializedNextTask?.hasTask || !materializedNextTask.executionRequestPath) {
    return;
  }
  const routeAction = routeActionFromRun(run, locator);
  if (!routeAction || routeAction.type !== "continue_execution") {
    return;
  }
  await updateRouteState({
    projectRoot,
    locator,
    deliveryStatus: deliveryStatusForRun(run),
    phaseStatus: phaseStatusForRun(run),
    latestRefs: {},
    nextAction: {
      ...routeAction,
      ref: materializedNextTask.executionRequestPath,
      refs: {
        ...(routeAction.refs ?? {}),
        taskId: materializedNextTask.task?.taskId ?? null,
        groupId: materializedNextTask.executionRequest?.groupId ?? materializedNextTask.task?.groupId ?? null,
        taskPlanRunId: materializedNextTask.executionRequest?.taskPlanRunId ?? run.runId,
        executionRequestRef: materializedNextTask.executionRequestPath,
        resultFile: materializedNextTask.executionRequest?.resultFile ?? null,
        activeOperationType: "task_execution",
      },
    },
  });
}

async function closeCompletedTaskExecutionLease(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  run: TaskPlanRun,
): Promise<void> {
  const lease = await readOperationLease(projectRoot, locator.deliveryId);
  if (!lease || lease.status !== "active" || lease.operationType !== "task_execution") {
    return;
  }
  if (typeof lease.refs.taskPlanRunId === "string" && lease.refs.taskPlanRunId !== run.runId) {
    return;
  }
  if (typeof lease.refs.taskId !== "string") {
    return;
  }
  const state = run.taskStates.find((item) => item.taskId === lease.refs.taskId);
  if (!state || !["completed", "completed_with_notes", "blocked", "failed"].includes(state.status)) {
    return;
  }
  await closeOperationLease({
    projectRoot,
    locator,
    operationType: "task_execution",
    reason: "task_execution_lease_already_resolved",
  });
}

export async function recordTaskResult(input: RecordTaskResultInput): Promise<{
  recorded: boolean;
  status: TaskResult["status"] | "invalid_result";
  taskResultId: string | null;
  issues: ReturnType<typeof validateTaskResult>["issues"];
  resultPath: string | null;
  run: TaskPlanRunSummary | null;
  nextAction: TaskPlanRun["nextAction"] | null;
  materializedNextTask?: Awaited<ReturnType<typeof materializeNextTaskExecution>>;
  instruction?: Record<string, unknown>;
  repairInstruction?: Record<string, unknown>;
  postRepairSubmitRouting?: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  resetIssueCounter();
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const candidate = await readJsonFile(resolveCliPath(root, input.inputFile));
  const partial = typeof candidate === "object" && candidate !== null ? candidate as { taskId?: unknown; taskPlanId?: unknown } : {};
  const taskPlan = await loadCurrentTaskPlan(root, typeof partial.taskPlanId === "string" ? partial.taskPlanId : undefined, locator);
  const run = await loadCurrentTaskPlanRun(root, input.taskPlanRunId, locator);
  const taskId = typeof partial.taskId === "string" ? partial.taskId : run.taskStates.find((state) => state.status === "running")?.taskId;
  if (!taskId) {
    throw invalidArgument("Cannot infer taskId for TaskResult.");
  }
  const task = taskPlan.tasks.find((item) => item.taskId === taskId);
  if (!task) {
    throw invalidArgument("TaskResult taskId does not exist in current TaskPlan.", { taskId });
  }
  const request = await buildTaskExecutionRequest(root, locator, taskPlan, run, task);
  const validation = validateTaskResult(candidate, request);
  if (!validation.value || validation.issues.length > 0) {
    const activeLease = await readOperationLease(root, locator.deliveryId);
    const requestRef = activeLease?.operationType === "task_execution" && typeof activeLease.refs.requestRef === "string"
      ? activeLease.refs.requestRef
      : toProjectRelative(root, taskExecutionRequestPath(root, request.requestId, locator));
    const state = run.taskStates.find((item) => item.taskId === task.taskId);
    if (state) {
      state.status = "failed";
      state.finishedAt = new Date().toISOString();
    }
    run.status = "failed";
    run.nextAction = {
      type: "task_result_repair",
      reason: "TASK_RESULT_CONTRACT_INVALID",
      sourceTaskId: task.taskId,
      targetNode: "task_result_repair",
    };
    run.updatedAt = new Date().toISOString();
    updateRunSummaryAndNextAction(run, false);
    await saveTaskPlanRun(root, run, locator);
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "task_execution",
      reason: "task_result_invalid",
    });
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "task_result_repair",
      reason: "task_result_repair_invalid",
    });
    await syncRouteFromRun(root, locator, run);
    const repairInstruction = withAutoRunnableTransition({
      mode: "repair_result_contract",
      routingRule: "Repair the same TaskResult file, then run submitCommand immediately. Do not ask whether to continue.",
      userMessage: "TaskResult contract validation failed. Repair the TaskResult JSON and submit record-result again.",
      primaryAction: {
        action: "repair_task_result_contract",
        resultFile: toProjectRelative(root, resolveCliPath(root, input.inputFile)),
        rule: "Repair only the TaskResult JSON contract fields reported in issues.",
      },
      completionCondition: {
        completeWhen: "record-result succeeds for the repaired TaskResult file.",
        afterSubmit: "Follow returned data.instruction immediately when auto-runnable.",
        stopOnlyWhen: [
          "record-result returns a non-repairable failure",
          "returned instruction is user-gated",
        ],
      },
      finalResponseGuard: {
        invalidFinalResponseWhen: [
          "the TaskResult file has not been repaired",
          "record-result has not succeeded for the repaired TaskResult",
          "the current response only summarizes progress, remaining work, or asks whether to continue",
        ],
        requiredActionBeforeFinalResponse: [
          "repair TaskResult JSON in resultFile",
          "run submitCommand",
          "follow returned data.instruction immediately when auto-runnable",
        ],
      },
      ...artifactRepairPolicy(),
      schema: "TaskResult",
      resultFile: toProjectRelative(root, resolveCliPath(root, input.inputFile)),
      issues: validation.issues,
      ...taskResultRepairContract({
        candidate,
        issues: validation.issues,
        request,
        requestRef,
      }),
      repairSubmitRouting: {
        submitCommandReturnsInstruction: true,
        followReturnedInstructionImmediately: true,
        doNotAskUserAfterSuccessfulSubmit: true,
        continueAutomaticallyWhenNextActionIs: [
          "continue_execution",
          "review",
          "execution_repair",
          "taskplan_repair",
          "architecture_artifact_repair",
        ],
        rule: "After the repaired record-result command succeeds, follow its data.instruction immediately. Do not summarize and ask whether to continue while the next action is auto-runnable.",
      },
      instructions: [
        "Repair only the TaskResult JSON contract fields.",
        "Do not modify project source code for this contract repair.",
        "Use refs from the TaskExecutionRequest.",
        "Use only verificationResults ids listed in allowedVerificationResults. Do not create separate verificationResults for build/test/lint commands.",
        "Put command-level evidence in the allowed verificationResults[].summary and runtimeDeliveryEvidence.commandsRun.",
        "For runtimeDeliveryEvidence.codeLevelChecks[].reason, omit the field when the check passed; use a non-empty string only when status is failed, blocked, or not_applicable. Never write reason: null.",
        "Return a complete replacement TaskResult to the same input file.",
        "Run record-result again with the same input-file.",
        "If the repaired record-result succeeds and returns data.instruction with autoContinue or mustRunImmediately, execute that instruction immediately.",
      ],
      submitCommand: {
        name: "record-result",
        argv: [
          "record-result",
          "--delivery-id",
          locator.deliveryId,
          "--phase-id",
          locator.phaseId,
          "--input-file",
          toProjectRelative(root, resolveCliPath(root, input.inputFile)),
        ],
      },
    }, {
      sourceCommand: "record-result",
      sourceSucceeded: false,
      sourceSummary: "TaskResult validation failed with repairable contract issues.",
      primaryAction: "repair_task_result_contract_and_submit",
      userVisibleSummary: "TaskResult validation failed with repairable contract issues. Repair the same resultFile and run instruction.submitCommand now.",
    });
    return {
      recorded: false,
      status: "invalid_result",
      taskResultId: validation.value?.taskResultId ?? null,
      issues: validation.issues,
      resultPath: null,
      run: summarizeRun(run),
      nextAction: run.nextAction,
      instruction: repairInstruction,
      repairInstruction,
    };
  }

  const stateBeforeRecord = run.taskStates.find((item) => item.taskId === task.taskId);
  if (
    stateBeforeRecord &&
    ["completed", "completed_with_notes", "blocked", "failed"].includes(stateBeforeRecord.status) &&
    !(
      stateBeforeRecord.status === "failed" &&
      run.nextAction?.type === "task_result_repair" &&
      run.nextAction.sourceTaskId === task.taskId
    )
  ) {
    updateRunSummaryAndNextAction(run);
    await saveTaskPlanRun(root, run, locator);
    await syncRouteFromRun(root, locator, run);
    const directInstruction = await directInstructionForRunNextAction(root, locator, taskPlan, run);
    await syncRouteFromMaterializedTask(root, locator, run, directInstruction.materializedNextTask);
    return {
      recorded: false,
      status: "invalid_result",
      taskResultId: validation.value?.taskResultId ?? null,
      issues: [staleTaskResultIssue()],
      resultPath: null,
      run: directInstruction.materializedNextTask?.run ?? summarizeRun(run),
      nextAction: run.nextAction,
      ...(directInstruction.materializedNextTask ? { materializedNextTask: directInstruction.materializedNextTask } : {}),
      instruction: directInstruction.instruction,
    };
  }
  const repairedContractSubmit =
    stateBeforeRecord?.status === "failed" &&
    run.nextAction?.type === "task_result_repair" &&
    run.nextAction.sourceTaskId === task.taskId;

  const result = taskResultSchema.parse({
    ...validation.value,
    updatedAt: new Date().toISOString(),
  });
  const resultFile = taskResultPath(root, run.runId, task.taskId, result.taskResultId, locator);
  await writeJsonAtomic(resultFile, result);
  const state = run.taskStates.find((item) => item.taskId === task.taskId);
  if (!state) {
    throw invalidArgument("Task state does not exist for TaskResult.", { taskId: task.taskId });
  }
  state.status = result.status;
  state.resultId = result.taskResultId;
  state.finishedAt = new Date().toISOString();
  state.attempts.push({
    attempt: state.attempts.length + 1,
    resultId: result.taskResultId,
    status: result.status,
  });
  updateGroupStateAfterTaskResult(run, state.groupId);
  updateRunSummaryAndNextAction(run);
  if (result.status === "blocked") {
    run.nextAction = nextActionFromBlockedResult(result, task.taskId);
  }
  await saveTaskPlanRun(root, run, locator);
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "task_execution",
    expectedRefs: {
      taskId: task.taskId,
      taskPlanRunId: run.runId,
    },
    reason: "task_result_recorded",
  });
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "execution_repair",
    reason: "execution_repair_result_recorded",
  });
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "task_result_repair",
    reason: "task_result_repair_recorded",
  });
  await syncRouteFromRun(root, locator, run);
  const directInstruction = await directInstructionForRunNextAction(root, locator, taskPlan, run);
  await syncRouteFromMaterializedTask(root, locator, run, directInstruction.materializedNextTask);

  return {
    recorded: true,
    status: result.status,
    taskResultId: result.taskResultId,
    issues: [],
    resultPath: toProjectRelative(root, resultFile),
    run: directInstruction.materializedNextTask?.run ?? summarizeRun(run),
    nextAction: run.nextAction,
    ...(directInstruction.materializedNextTask ? { materializedNextTask: directInstruction.materializedNextTask } : {}),
    instruction: directInstruction.instruction,
    ...(repairedContractSubmit ? {
      postRepairSubmitRouting: postRepairSubmitRouting({
        source: "task_result_repair",
        submitCommand: "record-result",
        repairedArtifact: { taskId: task.taskId, taskResultId: result.taskResultId },
        nextActionTypes: [
          "continue_execution",
          "review",
          "execution_repair",
          "taskplan_repair",
          "architecture_artifact_repair",
        ],
      }),
    } : {}),
  };
}

function summarizeRun(run: TaskPlanRun): TaskPlanRunSummary {
  const currentTask =
    run.taskStates.find((state) => state.status === "running") ??
    run.taskStates.find((state) => state.status === "pending") ??
    null;
  return {
    runId: run.runId,
    taskPlanId: run.taskPlanId,
    status: run.status,
    summary: run.summary,
    nextAction: run.nextAction,
    updatedAt: run.updatedAt,
    currentTask: currentTask
      ? {
        taskId: currentTask.taskId,
        ...(currentTask.groupId ? { groupId: currentTask.groupId } : {}),
        status: currentTask.status,
      }
      : null,
  };
}

function summarizeExecutionRequest(request: TaskExecutionRequest, requestPath: string): TaskExecutionRequestSummary {
  return {
    requestId: request.requestId,
    requestType: request.requestType,
    taskId: request.source.taskId,
    ...(request.source.groupId ? { groupId: request.source.groupId } : {}),
    taskPlanRunId: request.source.taskPlanRunId,
    title: request.task.title,
    taskKind: request.task.taskKind,
    acceptanceRefs: request.task.acceptanceRefs,
    resultFile: request.outputContract.resultFile,
    submitCommand: request.submitCommand,
    requestRef: requestPath,
  };
}

async function ensureTaskResultParentDir(projectRoot: string, resultFile: unknown): Promise<void> {
  if (typeof resultFile !== "string" || resultFile.length === 0) {
    return;
  }
  await ensureDir(path.dirname(resolveCliPath(projectRoot, resultFile)));
}

function executeTaskInstruction(input: ExecuteTaskInstructionInput): Record<string, unknown> {
  const resultFile = String(input.request.outputContract.resultFile);
  const submitCommand = input.request.submitCommand;
  const completionBarrier = taskExecutionCompletionBarrier(resultFile, submitCommand);
  const finalResponseGuard = taskExecutionFinalResponseGuard(resultFile);
  const completionContinuityRequirement = taskExecutionCompletionContinuityRequirement();
  return withAutoRunnableTransition({
    mode: "execute_task",
    ...taskExecutionOutputPolicy(),
    requestRef: input.requestRef,
    resultFile,
    submitCommand,
    completionBarrier,
    finalResponseGuard,
    completionContinuityRequirement,
    task: {
      taskId: input.request.source.taskId,
      groupId: input.request.source.groupId ?? null,
      title: input.request.task.title,
      taskKind: input.request.task.taskKind,
      acceptanceRefs: input.request.task.acceptanceRefs,
    },
    recovery: input.recovery,
    mustNotReportProgressBeforeExecuting: true,
    primaryAction: {
      action: "execute_current_task",
      requestRef: input.requestRef,
      resultFile,
      submitCommand,
      rule: "Execute this TaskExecutionRequest now. Do not replace execution with a recovery prompt.",
    },
    completionCondition: {
      completeWhen: "TaskResult exists at resultFile and submitCommand has succeeded.",
      afterSubmit: "Follow returned data.instruction immediately when auto-runnable.",
      stopOnlyWhen: [
        "request cannot be read",
        "submitCommand returns non-repairable failure",
        "returned instruction is user-gated, blocked, done, manual_review, or needs_user_decision",
      ],
    },
    mustNotDuringPrimaryAction: [
      "Do not replace execution with a recovery prompt while tool calls are still available.",
      "Do not send progress-only summaries, interim handoff notes, or next-step summaries before submitCommand succeeds.",
      "Do not run any routing command before submitting this TaskResult.",
      "Do not let any agent-chosen verification method prevent TaskResult and submitCommand closeout.",
      ...verificationCommandSchedulingRules.slice(0, 4),
      ...controlledRuntimeProbeRules,
    ],
    runtimeCommandGuard: {
      appliesWhen: "task.runtimeDeliveryRequirement is present or the task starts a temporary runtime/server/probe process",
      rules: controlledRuntimeProbeRules,
    },
    runtimeForegroundProbeCloseoutRules,
    executionSteps: [
      "Read requestRef before modifying files.",
      compactContextReadStep,
      "Use referencedArtifactReadGuide when you need full sourceRefs; do not guess jq wrapper roots.",
      "Execute only the requested task and obey executionRules, writeBoundary, enumRefs, and outputContract.",
      "Treat completionBarrier as mandatory: do not stop for progress reporting until resultFile exists and submitCommand has succeeded.",
      "Keep chat output compact: do not paste source diffs, large patches, full source files, or TaskResult JSON.",
      "Run appropriate verification for the task, but obey verificationCommandSchedulingRules: write-producing verification commands must be run one at a time, each in its own completed tool call.",
      "Do not run long-lived runtime/server commands as foreground verification commands.",
      "If you start a temporary runtime/probe server for verification, stop only that task-owned runtime before writing TaskResult and record runtimeDeliveryEvidence.runtimeProbeCleanup when runtimeDeliveryEvidence applies.",
      "If a runtime/server command is already running in the foreground and has shown a ready URL, listening port, or health-ready signal, do not wait for it to exit. Use that ready target for verification, stop only task-owned runtime when safe, then submit TaskResult.",
      "Record executionContinuity in TaskResult. If any agent-owned long-running work, browser session, interactive tool, server, watcher, or worker may still be unreleased, do not claim pure completed; use completed_with_notes with notes unless an independent failure or blocked condition remains.",
      "A failed/unknown/not-safe runtime probe cleanup is completed_with_notes with notes, not failed or blocked by itself.",
      "Write TaskResult JSON to resultFile even when the task ends failed or blocked.",
      "If verification remains failed after allowed self-repair, write a failed or blocked TaskResult and run submitCommand; do not stop in chat to ask the user whether to continue.",
      "Run submitCommand after resultFile exists.",
      "After submitCommand succeeds, follow returned data.instruction immediately when auto-runnable.",
    ],
    verificationCommandSchedulingRules,
    stopConditions: [
      "request cannot be read",
      "task returns blocked",
      "task returns failed after allowed self-repair",
      "submitCommand fails and does not return a repairInstruction",
      "returned instruction is ask_user, report_blocked, report_done, manual_review, or needs_user_decision",
    ],
    routingRule: "Execute this TaskExecutionRequest now. Do not stop after next-task creates the request. Progress-only summaries are not completion. Continue within this task until resultFile exists and submitCommand has succeeded, or a declared stopCondition is reached.",
    userMessage: "TaskExecutionRequest created. Execute the task now, write TaskResult, and submit it; do not stop with an interim progress summary.",
  }, {
    sourceCommand: "next-task",
    sourceSummary: "TaskExecutionRequest was created.",
    primaryAction: "execute_current_task",
    mustStartImmediately: true,
    userVisibleSummary: "TaskExecutionRequest was created. Read instruction.requestRef, execute the task now, write instruction.resultFile, and run instruction.submitCommand before any progress-only response.",
  });
}

function taskResultRepairContract(input: {
  candidate: unknown;
  issues: ContractIssue[];
  request: TaskExecutionRequest;
  requestRef: string | null;
}): Record<string, unknown> {
  const fields = taskResultRepairInspectFields(input.issues);
  return {
    repairContractProfile: "minimal_task_result_repair",
    issueConflicts: input.issues.map((issue) => taskResultIssueConflict(issue, input.candidate, input.request)),
    minimalRepairRules: taskResultMinimalRepairRules(input.issues),
    inspectRepairContractCommand: input.requestRef ? {
      name: "inspect",
      argv: ["inspect", "--request", input.requestRef, "--field", fields.join(",")],
      purpose: "Read only these TaskResult contract fields if the issue summary is insufficient. Do not read the full schemaShape.",
    } : null,
    resultRules: taskResultResultRules(),
  };
}

function taskResultRepairInspectFields(issues: ContractIssue[]): string[] {
  const fields = new Set<string>([
    "enumRefs",
    "outputContract.statusEnum",
    "outputContract.requiredTopLevelFields",
    "outputContract.schemaShape.status",
    "outputContract.schemaShape.changedFiles",
    "outputContract.schemaShape.noChangeReason",
    "outputContract.schemaShape.verificationResults",
    "outputContract.schemaShape.allowedVerificationResults",
    "outputContract.schemaShape.selfRepairSummary",
    "outputContract.schemaShape.selfRepairSummaryRules",
    "outputContract.schemaShape.failure",
    "outputContract.schemaShape.executionContinuity",
    "outputContract.schemaShape.notes",
    "outputContract.schemaShape.blockedReasons",
  ]);
  for (const issue of issues) {
    if (issue.code === "TASK_RESULT_WORKFLOW_CLOSURE_INVALID") {
      fields.add("outputContract.schemaShape.frontendExperienceSelfCheck.status");
      fields.add("outputContract.schemaShape.frontendExperienceSelfCheck.dataBinding");
      fields.add("outputContract.schemaShape.frontendExperienceSelfCheck.knownGaps");
      fields.add("task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs");
      fields.add("task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource");
    }
    if (issue.code === "TASK_RESULT_RUNTIME_CHECK_ID_INVALID") {
      fields.add("outputContract.schemaShape.runtimeDeliveryEvidence");
      fields.add("task.runtimeDeliveryRequirement");
    }
  }
  return [...fields];
}

function taskResultIssueConflict(issue: ContractIssue, candidate: unknown, request: TaskExecutionRequest): Record<string, unknown> {
  const base = {
    issueId: issue.issueId,
    code: issue.code,
    path: issue.path,
    message: issue.message,
    repairHint: issue.repairHint,
  };
  if (issue.code !== "TASK_RESULT_WORKFLOW_CLOSURE_INVALID") {
    return base;
  }
  const selfCheck = isRecord(candidate) && isRecord(candidate.frontendExperienceSelfCheck)
    ? candidate.frontendExperienceSelfCheck
    : {};
  const dataBinding = isRecord(selfCheck.dataBinding) ? selfCheck.dataBinding : {};
  const knownGaps = Array.isArray(selfCheck.knownGaps) ? selfCheck.knownGaps : null;
  return {
    ...base,
    current: {
      frontendExperienceSelfCheckStatus: typeof selfCheck.status === "string" ? selfCheck.status : null,
      dataBindingMode: typeof dataBinding.mode === "string" ? dataBinding.mode : null,
      knownGapsCount: knownGaps ? knownGaps.length : null,
    },
    expectedForSatisfied: {
      frontendExperienceSelfCheckStatus: "satisfied",
      dataBindingMode: "wired",
      knownGaps: [],
      closureRequirementIds: workflowClosureRequirementsFromTaskGuidance(request.task)
        .map((requirement) => String(requirement.closureId ?? ""))
        .filter((closureId) => closureId.length > 0),
    },
    validRepairChoices: [
      "If the implementation and evidence are actually wired, repair TaskResult frontendExperienceSelfCheck.dataBinding.mode to wired, clear knownGaps, and cite evidence.",
      "If the task still has static/demo/mocked binding or known gaps, change frontendExperienceSelfCheck.status away from satisfied and use completed_with_notes/failed/blocked according to the actual task outcome.",
    ],
  };
}

function taskResultMinimalRepairRules(issues: ContractIssue[]): string[] {
  const rules = [
    "Repair the same TaskResult JSON file only.",
    "Do not paste the complete TaskResult JSON into chat.",
    "Use exact verificationResults[].verificationId values from allowedVerificationResults.",
    "Command-level build/test/probe evidence belongs in verificationResults[].summary or runtimeDeliveryEvidence.commandsRun, not extra verificationResults.",
    "Never combine selfRepairSummary.attempted=false with stopReason verification_passed.",
    "For runtimeDeliveryEvidence.codeLevelChecks[].reason, omit the field when the check passed; use a non-empty string only when status is failed, blocked, or not_applicable. Never write reason: null.",
  ];
  if (issues.some((issue) => issue.code === "TASK_RESULT_WORKFLOW_CLOSURE_INVALID")) {
    rules.push(
      "For required frontend workflow closure, frontendExperienceSelfCheck.status=satisfied is valid only when dataBinding.mode=wired and knownGaps is an empty array.",
      "If wired evidence is missing, do not claim satisfied; report the remaining gap through frontendExperienceSelfCheck and the normal TaskResult status."
    );
  }
  return rules;
}

function taskExecutionCompletionBarrier(resultFile: string, submitCommand: Record<string, unknown>): Record<string, unknown> {
  return {
    resultFile,
    submitCommand,
    rules: taskExecutionCompletionBarrierRules,
  };
}

function taskExecutionFinalResponseGuard(resultFile: string): Record<string, unknown> {
  return {
    invalidFinalResponseWhen: [
      `${resultFile} does not exist`,
      "submitCommand has not succeeded",
      "the current response only summarizes progress, remaining work, or asks whether to continue",
    ],
    requiredActionBeforeFinalResponse: [
      "write TaskResult JSON to resultFile",
      "run submitCommand",
      "follow returned data.instruction immediately when auto-runnable",
    ],
  };
}

function taskExecutionActionRequired(input: {
  taskId: string;
  resultFile: string;
  submitCommand: Record<string, unknown>;
  completionBarrier: Record<string, unknown>;
  finalResponseGuard: Record<string, unknown>;
  completionContinuityRequirement: Record<string, unknown>;
}): Record<string, unknown> {
  return withAutoRunnableTransition({
    mode: "execute_task",
    taskId: input.taskId,
    mustNotReportProgress: true,
    mustNotAskBeforeExecuting: true,
    resultFile: input.resultFile,
    submitCommand: input.submitCommand,
    completionBarrier: input.completionBarrier,
    finalResponseGuard: input.finalResponseGuard,
    completionContinuityRequirement: input.completionContinuityRequirement,
    summary: "Execute the current TaskExecutionRequest now, write TaskResult, run submitCommand, then follow the returned instruction when auto-runnable.",
  }, {
    sourceCommand: "task-execution-request",
    sourceSummary: "TaskExecutionRequest actionRequired was created.",
    primaryAction: "execute_current_task",
    mustStartImmediately: true,
  });
}

function staleTaskResultIssue(): ContractIssue {
  return {
    issueId: "issue-stale-task-result",
    code: "TASK_RESULT_REF_INVALID",
    severity: "blocking",
    path: "/taskId",
    message: "TaskResult targets a task that has already reached a terminal state. Resume from the current run instruction instead of resubmitting an old task result.",
    repairability: "agent_repairable",
    repairHint: "Run loom continue and follow the active task execution instruction; do not resubmit this stale TaskResult.",
  };
}

function instructionForRunNextAction(
  locator: DeliveryPhaseLocator,
  nextAction: TaskPlanRun["nextAction"] | null,
): Record<string, unknown> | undefined {
  if (!nextAction) {
    return undefined;
  }
  const commandByAction: Record<string, { shouldAutoRun: boolean; argv: string[]; userMessage: string }> = {
    continue_execution: {
      shouldAutoRun: true,
      argv: ["next-task", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "TaskResult recorded. Continue immediately with the next ready task.",
    },
    review: {
      shouldAutoRun: true,
      argv: ["review", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "TaskPlanRun completed. Continue immediately with phase review.",
    },
    execution_repair: {
      shouldAutoRun: true,
      argv: ["repair", "request", "--type", "execution", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "Task execution failed after self-repair. Continue immediately by materializing an execution repair request.",
    },
    task_result_repair: {
      shouldAutoRun: false,
      argv: ["repair", "request", "--type", "task-result", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "TaskResult contract validation failed. Repair the TaskResult JSON and submit record-result again.",
    },
    taskplan_repair: {
      shouldAutoRun: true,
      argv: ["repair", "request", "--type", "taskplan", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "The task plan needs repair before execution can continue.",
    },
    architecture_artifact_repair: {
      shouldAutoRun: true,
      argv: ["repair", "request", "--type", "architecture", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "The architecture artifact needs repair before execution can continue.",
    },
    wait_dependency: {
      shouldAutoRun: false,
      argv: [],
      userMessage: "Execution is waiting for a dependency that cannot be advanced automatically.",
    },
  };
  const command = commandByAction[nextAction.type];
  if (!command) {
    return undefined;
  }
  if (command.shouldAutoRun && command.argv.length > 0) {
    return autoRunInstruction({
      actionType: nextAction.type,
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      reason: nextAction.reason ?? "RUN_NEXT_ACTION",
      targetNode: nextAction.targetNode,
      argv: command.argv,
      userMessage: command.userMessage,
      refs: {
        sourceTaskId: nextAction.sourceTaskId ?? null,
      },
    });
  }
  return {
    mode: command.argv.length > 0 ? "run_cli" : "report_blocked",
    nextAction: {
      type: nextAction.type,
      reason: nextAction.reason,
      targetNode: nextAction.targetNode,
      sourceTaskId: nextAction.sourceTaskId ?? null,
    },
    command: command.argv.length > 0 ? {
      name: nextAction.type,
      argv: command.argv,
    } : null,
    stopConditions: [
      "instruction.mode is ask_user, report_blocked, or report_done",
      "nextAction.type is manual_review or needs_user_decision",
      "a command exits non-zero and cannot be repaired through the returned repair instruction",
      "phase review completes and continue returns done or a user-gated next phase confirmation",
    ],
    chainUntil: [
      "phase review request/result is accepted",
      "continue returns report_done",
      "continue returns ask_user and the current user message has no usable answer",
      "manual_review or needs_user_decision is required",
      "a real blocking condition or unrecoverable command failure occurs",
    ],
    mustNotStopAfterSingleTask: nextAction.type === "continue_execution",
    userMessage: command.userMessage,
  };
}

async function syncRouteFromRun(projectRoot: string, locator: DeliveryPhaseLocator, run: TaskPlanRun): Promise<void> {
  await updateRouteState({
    projectRoot,
    locator,
    deliveryStatus: deliveryStatusForRun(run),
    phaseStatus: phaseStatusForRun(run),
    latestRefs: {},
    nextAction: routeActionFromRun(run, locator),
  });
}

function routeActionFromRun(run: TaskPlanRun, locator: DeliveryPhaseLocator): RouteAction | null {
  if (!run.nextAction) {
    return null;
  }
  if (run.nextAction.type === "wait_dependency") {
    return {
      type: "manual_review",
      source: "task_plan_run",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      reason: run.nextAction.reason,
      targetNode: "manual_review",
    };
  }
  return {
    type: run.nextAction.type,
    source: "task_plan_run",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    reason: run.nextAction.reason,
    targetNode: run.nextAction.targetNode,
  };
}

function deliveryStatusForRun(run: TaskPlanRun): "executing" | "reviewing" | "repairing" | "blocked" {
  if (run.nextAction?.type === "review") {
    return "reviewing";
  }
  if (run.nextAction?.type.endsWith("_repair")) {
    return "repairing";
  }
  if (run.status === "blocked") {
    return "blocked";
  }
  if (run.status === "failed") {
    return "repairing";
  }
  return "executing";
}

function phaseStatusForRun(run: TaskPlanRun): "executing" | "reviewing" | "repairing" | "blocked" {
  if (run.nextAction?.type === "review") {
    return "reviewing";
  }
  if (run.nextAction?.type.endsWith("_repair")) {
    return "repairing";
  }
  if (run.status === "blocked") {
    return "blocked";
  }
  if (run.status === "failed") {
    return "repairing";
  }
  return "executing";
}

export async function loadCurrentTaskPlan(projectRoot: string, taskPlanId?: string, locator?: DeliveryPhaseLocator): Promise<TaskPlan> {
  const resolvedLocator = locator ?? await getActiveLocator(projectRoot);
  const id = taskPlanId ?? await latestId(taskPlanLatestPath(projectRoot, resolvedLocator), "taskPlanId");
  if (!id) {
    throw noActivePlan(projectRoot);
  }
  const filePath = resolveTaskPlanPath(projectRoot, id, resolvedLocator);
  return taskPlanSchema.parse(await readJsonFile(filePath));
}

export async function loadCurrentTaskPlanRun(projectRoot: string, taskPlanRunId?: string, locator?: DeliveryPhaseLocator): Promise<TaskPlanRun> {
  const resolvedLocator = locator ?? await getActiveLocator(projectRoot);
  const id = taskPlanRunId ?? await latestId(taskPlanRunLatestPath(projectRoot, resolvedLocator), "taskPlanRunId");
  if (!id) {
    throw noActivePlan(projectRoot);
  }
  const filePath = taskPlanRunPath(projectRoot, id, resolvedLocator);
  return taskPlanRunSchema.parse(await readJsonFile(filePath));
}

function createTaskPlanRun(taskPlan: TaskPlan): TaskPlanRun {
  const now = new Date().toISOString();
  const run: TaskPlanRun = {
    schemaVersion: "1.0",
    runId: createId(`run-${taskPlan.source.phaseId}`),
    taskPlanId: taskPlan.taskPlanId,
    status: "not_started",
    scheduler: {
      mode: "group_dag",
      startedAt: null,
      finishedAt: null,
    },
    groupStates: taskPlan.groups.map((group) => ({
      groupId: group.groupId,
      status: "pending",
      startedAt: null,
      finishedAt: null,
      dependsOn: group.dependsOn,
      taskIds: group.taskIds,
    })),
    taskStates: taskPlan.tasks.map((task) => ({
      taskId: task.taskId,
      groupId: task.groupId,
      status: "pending",
      resultId: null,
      startedAt: null,
      finishedAt: null,
      dependsOn: task.dependsOn,
      attempts: [],
    })),
    summary: {
      total: taskPlan.tasks.length,
      completed: 0,
      completedWithNotes: 0,
      blocked: 0,
      failed: 0,
      pending: taskPlan.tasks.length,
      running: 0,
    },
    nextAction: {
      type: "continue_execution",
      reason: "TASKPLAN_READY",
      targetNode: "task_execution",
    },
    createdAt: now,
    updatedAt: now,
  };
  return taskPlanRunSchema.parse(run);
}

function buildTaskConceptGrounding(task: Task, phaseConceptGroundingRef: string | null): Record<string, unknown> {
  const conceptRefs = task.conceptRefs ?? [];
  if (conceptRefs.length === 0) {
    return {
      mode: "none_required",
      reason: "TaskPlan did not assign conceptRefs to this task.",
      responsibleConcepts: [],
      relatedConcepts: [],
      avoidConcepts: [],
    };
  }
  return {
    mode: "concepts_present",
    phaseConceptGroundingRef,
    responsibleConcepts: conceptRefs.map((conceptRef) => ({
      conceptRef,
      responsibility: task.conceptResponsibilities?.find((item) => item.conceptRef === conceptRef)?.responsibility ?? "Read phaseConceptGroundingRef and satisfy this concept for the current task.",
      verificationIntent: task.conceptVerificationIntents?.find((item) => item.conceptRef === conceptRef)?.intent ?? "Provide conceptEvidence in TaskResult.",
    })),
    relatedConcepts: [],
    avoidConcepts: [],
  };
}

function buildTaskAcceptanceSnapshot(
  pgc: PlanningGenerationContract,
  aac: ArchitectureArtifactContract,
  task: Task,
): Record<string, unknown>[] {
  const pgcAcceptanceById = new Map(pgc.phaseScope.acceptanceCandidates.map((item) => [item.id, item]));
  const aacCoverageById = new Map(aac.acceptanceMatrix.map((item) => [item.acceptanceId, item]));
  return task.acceptanceRefs.map((acceptanceId) => {
    const pgcAcceptance = pgcAcceptanceById.get(acceptanceId);
    const aacCoverage = aacCoverageById.get(acceptanceId);
    return {
      acceptanceId,
      statement: pgcAcceptance?.statement ?? aacCoverage?.statement ?? null,
      priority: pgcAcceptance?.priority ?? aacCoverage?.priority ?? null,
      sourceRefs: pgcAcceptance?.sourceRefs ?? [],
      capabilityRefs: pgcAcceptance?.capabilityRefs ?? [],
      aacCoverage: aacCoverage ? {
        statement: aacCoverage.statement,
        priority: aacCoverage.priority,
        coverageStatus: aacCoverage.coverageStatus,
        coverage: aacCoverage.coverage,
        verificationHints: aacCoverage.verificationHints,
        reason: aacCoverage.reason ?? null,
        reasonCategory: aacCoverage.reasonCategory ?? null,
      } : null,
      taskResponsibilityRule: "Use this combined PGC acceptance detail and AAC coverage when implementing task objective, verificationResults, conceptEvidence, frontend/runtime evidence, and object-operation behavior. If the statement names fields, rules, states, blocking reasons, or feedback, TaskResult evidence must address them.",
    };
  });
}

function buildTaskRequirementDetailSnapshot(
  pgc: PlanningGenerationContract,
  aac: ArchitectureArtifactContract,
  task: Task,
): Record<string, unknown>[] {
  if (!pgc.requirementDetails) {
    return [];
  }
  const detailIds = uniqueRefs([
    ...(task.requirementDetailRefs ?? []),
    ...task.verificationIntents.flatMap((intent) => intent.requirementDetailRefs ?? []),
  ]);
  if (detailIds.length === 0) {
    return [];
  }
  const detailById = new Map(pgc.requirementDetails.items.map((item) => [item.detailId, item]));
  const coverageById = new Map((aac.detailCoverage ?? []).map((entry) => [entry.detailId, entry]));
  return detailIds.map((detailId) => {
    const detail = detailById.get(detailId);
    const coverage = coverageById.get(detailId);
    return {
      detailId,
      foundInPgc: Boolean(detail),
      kind: detail?.kind ?? null,
      title: detail?.title ?? null,
      summary: detail?.summary ?? null,
      priority: detail?.priority ?? null,
      sourceRefs: detail?.sourceRefs ?? [],
      scopeRefs: detail?.scopeRefs ?? [],
      acceptanceRefs: detail?.acceptanceRefs ?? [],
      impactTags: detail?.impactTags ?? [],
      lifecycleStage: detail?.lifecycleStage ?? null,
      quality: detail?.quality ?? null,
      unresolvedNote: detail?.unresolvedNote ?? null,
      aacCoverage: coverage ? {
        coverageStatus: coverage.coverageStatus,
        artifactRefs: coverage.artifactRefs,
        reason: coverage.reason ?? null,
      } : null,
      verificationIntentRefs: task.verificationIntents
        .filter((intent) => (intent.requirementDetailRefs ?? []).includes(detailId))
        .map((intent) => intent.verificationId),
      taskResponsibilityRule: "Implement and verify this task-scoped requirement detail using the referenced AAC artifacts. Do not treat this snapshot as an allowlist; if project code reveals a necessary supporting change inside task scope, implement it and record evidence.",
    };
  });
}

type FrontendExperienceContract = NonNullable<ArchitectureArtifactContract["frontendExperience"]>;

function uniqueRefs(refs: string[]): string[] {
  return [...new Set(refs.filter((ref) => ref.length > 0))];
}

function intersectRefs(refs: string[], allowedRefs: Set<string>): string[] {
  return refs.filter((ref) => allowedRefs.has(ref));
}

function frontendTaskResponsibility(task: Task): string {
  if (task.taskKind === "frontend_experience") {
    return "shell_or_surface_implementation";
  }
  if (task.taskKind === "ui_flow_increment") {
    return "workflow_implementation";
  }
  if (task.taskKind === "verification_increment") {
    return "frontend_verification";
  }
  return "frontend_supporting_change";
}

function frontendBindingInterfaceContract(contract: ArchitectureArtifactContract["interfaces"][number]): Record<string, unknown> {
  return {
    interfaceId: contract.interfaceId,
    name: contract.name,
    type: contract.type,
    role: contract.role ?? null,
    moduleRefs: contract.moduleRefs,
    entityRefs: contract.entityRefs,
    scopeRefs: contract.scopeRefs,
    acceptanceRefs: contract.acceptanceRefs,
    method: contract.method ?? null,
    path: contract.path ?? null,
    requestSchema: contract.requestSchema ?? [],
    responseSchema: contract.responseSchema ?? [],
    errorSchema: contract.errorSchema ?? [],
  };
}

function frontendStepUiResponsibilities(
  step: ArchitectureArtifactContract["userFlows"][number]["steps"][number] | null,
  frontendExperience: FrontendExperienceContract,
): Record<string, unknown> {
  const states = new Set(frontendExperience.interactionStates);
  return {
    beforeRequest: [
      "Read the user input/state needed for this workflow step before invoking the bound interface.",
    ],
    duringRequest: states.has("loading")
      ? ["Represent loading or in-progress state while the interface call is pending."]
      : [],
    onSuccess: [
      step?.systemResponse
        ? `Render the declared system response: ${step.systemResponse}`
        : "Render the successful workflow outcome in the relevant surface.",
    ],
    onError: states.has("error")
      ? ["Render an actionable error state when the interface call fails or returns invalid data."]
      : [],
  };
}

function frontendBindingReadToResolve(): string[] {
  return [
    "sourceRefs.architectureArtifactContractRef#/interfaces",
    "sourceRefs.architectureArtifactContractRef#/userFlows",
    "sourceRefs.taskPlanRef#/tasks",
    "project source files",
  ];
}

function frontendOperationPathProjection(
  frontendExperience: FrontendExperienceContract,
  selectedSurfaceIds: Set<string>,
  workflowRefsInScope: string[],
  workflowClosureRequirements: WorkflowClosureRequirement[],
): Record<string, unknown> {
  const workflowRefSet = new Set(workflowRefsInScope);
  const closureOperationPathRefs = new Set(workflowClosureRequirements.flatMap((requirement) => requirement.operationPathRefs));
  const selectedOperationPaths = (frontendExperience.operationPaths ?? []).filter((operationPath) =>
    closureOperationPathRefs.has(operationPath.pathId) ||
    (operationPath.workflowRef ? workflowRefSet.has(operationPath.workflowRef) : false) ||
    (operationPath.surfaceRef ? selectedSurfaceIds.has(operationPath.surfaceRef) : false)
  );
  const selectedDataViewRefs = new Set(selectedOperationPaths.flatMap((operationPath) => operationPath.dataViewRefs));
  const selectedActionRefs = new Set(selectedOperationPaths.flatMap((operationPath) => operationPath.actionRefs));
  for (const requirement of workflowClosureRequirements) {
    requirement.dataViewRefs.forEach((ref) => selectedDataViewRefs.add(ref));
    requirement.actionRefs.forEach((ref) => selectedActionRefs.add(ref));
  }

  const dataViewsInScope = (frontendExperience.dataViews ?? []).filter((dataView) =>
    selectedDataViewRefs.has(dataView.viewId) ||
    selectedOperationPaths.some((operationPath) => operationPath.targetObject && operationPath.targetObject === dataView.targetObject)
  );
  const actionsInScope = (frontendExperience.actions ?? []).filter((action) =>
    selectedActionRefs.has(action.actionId) ||
    selectedOperationPaths.some((operationPath) => operationPath.targetObject && operationPath.targetObject === action.targetObject)
  );

  const operationPathWarnings: string[] = [];
  if ((frontendExperience.operationPaths?.length ?? 0) > 0 && selectedOperationPaths.length === 0) {
    operationPathWarnings.push("Frontend operation paths exist in AAC but none matched this task's surface/workflow refs; read the full architectureArtifactContractRef if this task still owns UI behavior.");
  }

  return {
    dataViewsInScope,
    actionsInScope,
    operationPathsInScope: selectedOperationPaths,
    operationPathEvidenceRule: "When this task touches UI behavior, frontendExperienceSelfCheck should cite the operation paths, data views, actions, and result feedback it implemented or verified. If these projections are empty but the task owns UI behavior, read the full AAC frontendExperience before deciding.",
    operationPathWarnings,
  };
}

function buildFrontendBackendBindingProjection(
  task: Task,
  aac: ArchitectureArtifactContract,
  frontendExperience: FrontendExperienceContract,
  workflowRefsInScope: string[],
): Record<string, unknown> {
  const flowById = new Map(aac.userFlows.map((flow) => [flow.flowId, flow]));
  const interfaceById = new Map(aac.interfaces.map((contract) => [contract.interfaceId, contract]));
  const frontendBackendBindings: Record<string, unknown>[] = [];
  const unresolvedBindingInputs: Record<string, unknown>[] = [];

  for (const workflowRef of workflowRefsInScope) {
    const flow = flowById.get(workflowRef);
    if (!flow) {
      unresolvedBindingInputs.push({
        unresolvedId: `${workflowRef}:workflow_ref_not_found`,
        workflowRef,
        stepRef: null,
        userAction: null,
        reason: "workflow_ref_not_found_in_aac",
        readToResolve: frontendBindingReadToResolve(),
        agentInstruction: "Read the listed authority fields and project source before deciding whether this task can implement the frontend/backend binding. Do not invent an API.",
      });
      continue;
    }

    if (flow.steps.length === 0) {
      const flowInterfaceRefs = uniqueRefs(flow.interfaceRefs);
      const resolvedInterfaces = flowInterfaceRefs
        .map((ref) => interfaceById.get(ref))
        .filter((contract): contract is NonNullable<typeof contract> => Boolean(contract))
        .map(frontendBindingInterfaceContract);
      const missingInterfaceRefs = flowInterfaceRefs.filter((ref) => !interfaceById.has(ref));
      if (resolvedInterfaces.length > 0) {
        frontendBackendBindings.push({
          bindingId: `${flow.flowId}:flow`,
          bindingSource: "flow_interfaceRefs",
          workflowRef: flow.flowId,
          workflowName: flow.name,
          stepRef: null,
          userAction: flow.name,
          entry: flow.entry,
          interfaces: resolvedInterfaces,
          uiResponsibilities: frontendStepUiResponsibilities(null, frontendExperience),
          completionRule: "Use this binding as the first coding guide for wiring the workflow to existing AAC interfaces. This list is not an allowlist; read AAC/TaskPlan/source when another declared interface is needed.",
        });
      }
      if (flowInterfaceRefs.length === 0 || missingInterfaceRefs.length > 0) {
        unresolvedBindingInputs.push({
          unresolvedId: `${flow.flowId}:flow_interfaceRefs_unresolved`,
          workflowRef: flow.flowId,
          stepRef: null,
          userAction: flow.name,
          reason: flowInterfaceRefs.length === 0 ? "flow_has_no_interfaceRefs" : "interfaceRefs_not_found_in_aac",
          missingInterfaceRefs,
          readToResolve: frontendBindingReadToResolve(),
          agentInstruction: "If this workflow requires a backend/API binding, read the listed authority fields and project source to identify a declared interface. Continue the task if the binding can be determined; otherwise record the data-binding gap in frontendExperienceSelfCheck.",
        });
      }
      continue;
    }

    for (const step of flow.steps) {
      const stepInterfaceRefs = uniqueRefs(step.interfaceRefs);
      const flowInterfaceRefs = uniqueRefs(flow.interfaceRefs);
      const candidateInterfaceRefs = stepInterfaceRefs.length > 0 ? stepInterfaceRefs : flowInterfaceRefs;
      const bindingSource = stepInterfaceRefs.length > 0 ? "step_interfaceRefs" : "flow_interfaceRefs_fallback";
      const resolvedInterfaces = candidateInterfaceRefs
        .map((ref) => interfaceById.get(ref))
        .filter((contract): contract is NonNullable<typeof contract> => Boolean(contract))
        .map(frontendBindingInterfaceContract);
      const missingInterfaceRefs = candidateInterfaceRefs.filter((ref) => !interfaceById.has(ref));

      if (resolvedInterfaces.length > 0) {
        frontendBackendBindings.push({
          bindingId: `${flow.flowId}:${step.stepId}`,
          bindingSource,
          workflowRef: flow.flowId,
          workflowName: flow.name,
          stepRef: step.stepId,
          userAction: step.action,
          systemResponse: step.systemResponse ?? null,
          entry: flow.entry,
          interfaces: resolvedInterfaces,
          uiResponsibilities: frontendStepUiResponsibilities(step, frontendExperience),
          completionRule: "Use this binding as the first coding guide for wiring the user action to existing AAC interfaces. This list is not an allowlist; read AAC/TaskPlan/source when another declared interface is needed.",
        });
      }

      if (candidateInterfaceRefs.length === 0 || missingInterfaceRefs.length > 0) {
        unresolvedBindingInputs.push({
          unresolvedId: `${flow.flowId}:${step.stepId}:interfaceRefs_unresolved`,
          workflowRef: flow.flowId,
          stepRef: step.stepId,
          userAction: step.action,
          reason: candidateInterfaceRefs.length === 0 ? "step_has_no_interfaceRefs" : "interfaceRefs_not_found_in_aac",
          missingInterfaceRefs,
          readToResolve: frontendBindingReadToResolve(),
          agentInstruction: "If this user action needs data/API binding, read the listed authority fields and project source to identify a declared interface. Continue the task if the binding can be determined; otherwise record the data-binding gap in frontendExperienceSelfCheck.",
        });
      }
    }
  }

  return {
    frontendBackendBindings,
    unresolvedBindingInputs,
    bindingProjectionRules: {
      apiContractAuthority: "AAC global interfaces. Task scope and workflow refs only select the most relevant bindings.",
      bindingsAreNotAllowlist: true,
      ifBindingMissing: "Read architectureArtifactContractRef, taskPlanRef, and project source before deciding. Do not invent an API.",
      validatorBehavior: "These bindings guide coding and TaskResult evidence; they are not a blocking validator allowlist.",
    },
    bindingProjectionSummary: {
      bindingCount: frontendBackendBindings.length,
      unresolvedCount: unresolvedBindingInputs.length,
      taskInterfaceRefs: task.writeBoundary.artifactRefs.interfaces,
    },
  };
}

function buildFrontendExecutionGuidance(
  task: Task,
  aac: ArchitectureArtifactContract,
  frontendExperience: FrontendExperienceContract,
  userFacingLanguage?: UserFacingLanguageConstraint | null,
): Record<string, unknown> {
  const moduleRefs = new Set(task.writeBoundary.artifactRefs.modules);
  const userFlowRefs = new Set(task.writeBoundary.artifactRefs.userFlows);
  const flowById = new Map(aac.userFlows.map((flow) => [flow.flowId, flow]));
  const guidanceWarnings: string[] = [];
  const workflowClosureRequirements = closureRequirementsForTask(task, aac);

  const directlyMatchedSurfaces = frontendExperience.surfaces.filter((surface) =>
    surface.workflowRefs.some((ref) => userFlowRefs.has(ref)) ||
    surface.moduleRefs.some((ref) => moduleRefs.has(ref))
  );
  const surfacesInScope = directlyMatchedSurfaces.length > 0
    ? directlyMatchedSurfaces
    : frontendExperience.surfaces;
  if (directlyMatchedSurfaces.length === 0 && frontendExperience.surfaces.length > 0) {
    guidanceWarnings.push("No frontend surface matched current task artifact refs directly, so all declared frontend surfaces are listed for agent judgment.");
  }
  if (frontendExperience.surfaces.length === 0) {
    guidanceWarnings.push("Frontend experience is required but no surfaces were declared in AAC frontendExperience.");
  }

  const selectedSurfaceIds = new Set(surfacesInScope.map((surface) => surface.surfaceId));
  const surfaceWorkflowRefs = uniqueRefs(surfacesInScope.flatMap((surface) => surface.workflowRefs));
  const taskWorkflowRefs = uniqueRefs([
    ...task.writeBoundary.artifactRefs.userFlows,
    ...workflowClosureRequirements.map((requirement) => requirement.workflowRef),
  ]);
  const workflowRefsInScope = taskWorkflowRefs.length > 0
    ? taskWorkflowRefs
    : surfaceWorkflowRefs;
  const extraSurfaceWorkflowRefs = surfaceWorkflowRefs.filter((ref) => !workflowRefsInScope.includes(ref));
  if (extraSurfaceWorkflowRefs.length > 0) {
    guidanceWarnings.push("Shared frontend surfaces declare additional workflows outside this task; they are not included in workflowsInScope or frontendBackendBindings.");
  }
  const interfaceRefsInScope = uniqueRefs([
    ...task.writeBoundary.artifactRefs.interfaces,
    ...workflowRefsInScope.flatMap((flowRef) => flowById.get(flowRef)?.interfaceRefs ?? []),
  ]);
  const interfaceRefSet = new Set(interfaceRefsInScope);
  const interfaceById = new Map(aac.interfaces.map((contract) => [contract.interfaceId, contract]));
  const interfaceRolesInScope = interfaceRefsInScope.map((interfaceRef) => {
    const contract = interfaceById.get(interfaceRef);
    return {
      interfaceRef,
      role: contract?.role ?? "unknown",
      method: contract?.method ?? null,
      path: contract?.path ?? null,
    };
  });
  const bindingProjection = buildFrontendBackendBindingProjection(task, aac, frontendExperience, workflowRefsInScope);
  const operationPathProjection = frontendOperationPathProjection(
    frontendExperience,
    selectedSurfaceIds,
    workflowRefsInScope,
    workflowClosureRequirements,
  );

  return {
    schemaVersion: "1.0",
    purpose: "Use this to implement the current frontend task. It is guidance for the task, not a separate source of truth.",
    sourceAuthority: "task.frontendExperienceRequirement plus architectureArtifactContractRef#/frontendExperience",
    userFacingLanguage: userFacingLanguage ?? null,
    userFacingLanguageRule: userFacingLanguageRule(userFacingLanguage),
    responsibility: frontendTaskResponsibility(task),
    surfacesInScope: surfacesInScope.map((surface) => {
      const surfaceInterfaceRefs = uniqueRefs(surface.workflowRefs.flatMap((flowRef) => flowById.get(flowRef)?.interfaceRefs ?? []));
      return {
        surfaceId: surface.surfaceId,
        name: surface.name,
        purpose: surface.purpose,
        userRoleRefs: surface.userRoleRefs,
        workflowRefs: surface.workflowRefs,
        moduleRefs: surface.moduleRefs,
        matchedBy: {
          workflowRefs: intersectRefs(surface.workflowRefs, userFlowRefs),
          moduleRefs: intersectRefs(surface.moduleRefs, moduleRefs),
          interfaceRefs: intersectRefs(surfaceInterfaceRefs, interfaceRefSet),
        },
      };
    }),
    navigationInScope: {
      required: frontendExperience.navigation.required,
      pattern: frontendExperience.navigation.pattern,
      items: frontendExperience.navigation.items.filter((item) => selectedSurfaceIds.has(item.targetSurfaceRef)),
    },
    workflowsInScope: workflowRefsInScope.map((workflowRef) => {
      const flow = flowById.get(workflowRef);
      return {
        workflowRef,
        name: flow?.name,
        kind: flow?.kind,
        entry: flow?.entry,
        interfaceRefs: flow?.interfaceRefs ?? [],
        responsibility: "implement_or_wire_current_task_flow",
        minimumCompletionRule: "Do not satisfy this workflow with static descriptive text only when this task is responsible for workflow implementation.",
      };
    }),
    interactionStatesExpected: frontendExperience.interactionStates,
    ...operationPathProjection,
    mustNot: frontendExperience.mustNot,
    dataBindingExpectation: {
      interfacesInScope: interfaceRefsInScope,
      interfaceRolesInScope,
      allowedModes: ["wired", "mocked_with_reason", "static_only_with_reason", "not_applicable"],
      ...(workflowClosureRequirements.length > 0 ? {
        requiredModeForSatisfaction: "wired",
        closureRequirementIds: workflowClosureRequirements.map((requirement) => requirement.closureId),
        staticModePolicy: "not_satisfied",
        knownGapPolicy: "not_satisfied_when_required_closure",
      } : {}),
      guidance: "Describe how UI data/actions are wired. Use frontendBackendBindings first when present. If the binding is missing, read AAC/TaskPlan/source before deciding; if still static-only, record it as a known gap instead of claiming full workflow completion.",
    },
    closureRequirementRefs: workflowClosureRequirementExecutionView(workflowClosureRequirements),
    workflowClosureDetailSource: workflowClosureExecutionDetailSource(workflowClosureRequirements),
    ...bindingProjection,
    taskResultEvidenceGuide: {
      field: "frontendExperienceSelfCheck",
      recommendedFields: [
        "status",
        "surfacesTouched",
        "workflowsCovered",
        "userActionsImplemented",
        "interactionStatesCovered",
        "dataBinding",
        "knownGaps",
      ],
    },
    guidanceWarnings,
  };
}

function workflowClosureRequirementExecutionView(requirements: WorkflowClosureRequirement[]): Array<Record<string, unknown>> {
  return requirements.map((requirement) => ({
    closureId: requirement.closureId,
    workflowRef: requirement.workflowRef,
    workflowName: requirement.workflowName,
    surfaceRefs: requirement.surfaceRefs,
    operationPathRefs: requirement.operationPathRefs,
    dataViewRefs: requirement.dataViewRefs,
    actionRefs: requirement.actionRefs,
    moduleRefs: requirement.moduleRefs,
    acceptanceRefs: requirement.acceptanceRefs,
    interfaceRefs: requirement.interfaceRefs,
    stateMachineRefs: requirement.stateMachineRefs,
    stepRefs: requirement.stepRefs,
    requiredDataBindingMode: requirement.requiredDataBindingMode,
    requiredEvidence: requirement.requiredEvidence,
    interfaceRoles: requirement.interfaces.map((contract) => ({
      interfaceRef: contract.interfaceId,
      role: contract.role ?? "unknown",
      type: contract.type,
      method: contract.method ?? null,
      path: contract.path ?? null,
    })),
    evidenceRule: "Evidence must cover user action, declared interface invocation, state or persistence change, and success or blocking feedback.",
  }));
}

function workflowClosureRequirementSourceSummary(
  requirements: WorkflowClosureRequirement[],
  options: { sourcePath: string; phase: "taskplan_generation" | "task_execution" },
): Record<string, unknown> {
  return {
    sourcePath: options.sourcePath,
    phase: options.phase,
    requirementCount: requirements.length,
    closureRequirementIds: requirements.map((requirement) => requirement.closureId),
    rule: "Read the sourcePath only when full closure detail is needed. Do not duplicate the full workflow closure payload into generation rules, task guidance, or TaskResult schema.",
    requiredTaskEvidence: [
      "user_action",
      "declared_interface_invocation",
      "state_or_persistence_change",
      "success_or_blocking_feedback",
    ],
  };
}

function workflowClosureExecutionDetailSource(requirements: WorkflowClosureRequirement[]): Record<string, unknown> {
  return {
    sourcePaths: TASK_EXECUTION_WORKFLOW_CLOSURE_DETAIL_SOURCES,
    closureRequirementIds: requirements.map((requirement) => requirement.closureId),
    readWhen: "Read these AAC detail sources only when closureRequirementRefs, frontendBackendBindings, and sourceContext.architectureArtifactProjection are not enough to implement or verify the task.",
    derivationRule: "Closure refs are derived from AAC frontendExperience surfaces, task userFlows, userFlow steps, and executable interfaces.",
  };
}

function workflowClosureRequirementsFromTaskGuidance(task: Task): Array<Record<string, unknown>> {
  const requirement = task.frontendExperienceRequirement;
  if (!requirement || typeof requirement !== "object" || Array.isArray(requirement)) {
    return [];
  }
  const guidance = (requirement as Record<string, unknown>).executionGuidance;
  if (!guidance || typeof guidance !== "object" || Array.isArray(guidance)) {
    return [];
  }
  const closureRefs = (guidance as Record<string, unknown>).closureRequirementRefs;
  if (Array.isArray(closureRefs)) {
    return closureRefs
      .map((item): Record<string, unknown> | null => {
        if (typeof item === "string" && item.length > 0) {
          return { closureId: item };
        }
        if (typeof item === "object" && item !== null && !Array.isArray(item)) {
          return item as Record<string, unknown>;
        }
        return null;
      })
      .filter((item): item is Record<string, unknown> => item !== null);
  }
  const requirements = (guidance as Record<string, unknown>).workflowClosureRequirements;
  return Array.isArray(requirements)
    ? requirements.filter((item): item is Record<string, unknown> => typeof item === "object" && item !== null && !Array.isArray(item))
    : [];
}

function withFrontendExecutionGuidance(task: Task, aac: ArchitectureArtifactContract, userFacingLanguage?: UserFacingLanguageConstraint | null): Task {
  if (!task.frontendExperienceRequirement || !aac.frontendExperience) {
    return task;
  }
  const requirement = task.frontendExperienceRequirement as Record<string, unknown>;
  const existingGuidance = typeof requirement.executionGuidance === "object" && requirement.executionGuidance !== null && !Array.isArray(requirement.executionGuidance)
    ? requirement.executionGuidance as Record<string, unknown>
    : {};
  const { workflowClosureRequirements: _legacyWorkflowClosureRequirements, ...safeExistingGuidance } = existingGuidance;
  return {
    ...task,
    frontendExperienceRequirement: {
      ...requirement,
      executionGuidance: {
        ...safeExistingGuidance,
        ...buildFrontendExecutionGuidance(task, aac, aac.frontendExperience, userFacingLanguage),
      },
    },
  };
}

async function buildTaskExecutionRequest(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  taskPlan: TaskPlan,
  run: TaskPlanRun,
  task: Task,
  identity?: { requestId?: string; resultFile?: string },
): Promise<TaskExecutionRequest> {
  const baseline = await loadRequiredTechnicalBaseline(projectRoot, locator);
  const aac = await loadArchitectureArtifact(projectRoot, taskPlan.source.architectureArtifactContractId, locator);
  const pgc = await loadPlanningContract(projectRoot, taskPlan.source.planningGenerationContractId, locator);
  const userFacingLanguage = pgc.planningInputs.userFacingLanguage ?? null;
  const requestTask = withFrontendExecutionGuidance(task, aac, userFacingLanguage);
  const requirementDetailSnapshot = buildTaskRequirementDetailSnapshot(pgc, aac, requestTask);
  const workflowClosureRequirements = workflowClosureRequirementsFromTaskGuidance(requestTask);
  const phaseConceptGroundingAbs = phaseConceptGroundingPath(projectRoot, locator.deliveryId, locator.phaseId);
  const phaseConceptGroundingRef = await pathExists(phaseConceptGroundingAbs)
    ? toProjectRelative(projectRoot, phaseConceptGroundingAbs)
    : null;
  const requestId = identity?.requestId ?? createId(`exec-${requestTask.taskId}`);
  const resultFile = identity?.resultFile ?? toProjectRelative(projectRoot, taskExecutionResultCandidatePath(projectRoot, locator, requestId));
  const runtimeRequiredChecks = requestTask.runtimeDeliveryRequirement?.requiredCodeLevelChecks ?? [];
  const runtimeAffectedFields = requestTask.runtimeDeliveryRequirement?.affectedContractFields ?? [];
  const requiredRuntimeEvidence = requestTask.runtimeDeliveryRequirement ? {
    appliesToThisTask: true,
    rule: "Copy these exact checkId values into runtimeDeliveryEvidence.codeLevelChecks[].checkId. Do not invent or rename runtime checkIds.",
    checkedFields: runtimeAffectedFields,
    requiredCheckIds: runtimeRequiredChecks.map((check) => check.checkId),
    requiredCodeLevelChecks: runtimeRequiredChecks.map((check) => ({
      checkId: check.checkId,
      contractField: check.contractField,
      objective: check.objective,
      acceptableEvidence: check.acceptableEvidence,
    })),
  } : undefined;
  const submitCommand = {
    name: "record-result",
    argv: [
      "record-result",
      "--delivery-id",
      locator.deliveryId,
      "--phase-id",
      locator.phaseId,
      "--input-file",
      "{resultFile}",
    ],
  };
  const completionBarrier = taskExecutionCompletionBarrier(resultFile, submitCommand);
  const finalResponseGuard = taskExecutionFinalResponseGuard(resultFile);
  const completionContinuityRequirement = taskExecutionCompletionContinuityRequirement();
  const frontendGuidanceReadFields = requestTask.frontendExperienceRequirement ? [
    "task.frontendExperienceRequirement.frontendExperienceRef",
    "task.frontendExperienceRequirement.experienceLevel",
    "task.frontendExperienceRequirement.mustSatisfy",
    "task.frontendExperienceRequirement.executionGuidance.schemaVersion",
    "task.frontendExperienceRequirement.executionGuidance.purpose",
    "task.frontendExperienceRequirement.executionGuidance.sourceAuthority",
    "task.frontendExperienceRequirement.executionGuidance.userFacingLanguage",
    "task.frontendExperienceRequirement.executionGuidance.userFacingLanguageRule",
    "task.frontendExperienceRequirement.executionGuidance.responsibility",
    "task.frontendExperienceRequirement.executionGuidance.surfacesInScope",
    "task.frontendExperienceRequirement.executionGuidance.navigationInScope",
    "task.frontendExperienceRequirement.executionGuidance.workflowsInScope",
    "task.frontendExperienceRequirement.executionGuidance.interactionStatesExpected",
    "task.frontendExperienceRequirement.executionGuidance.dataViewsInScope",
    "task.frontendExperienceRequirement.executionGuidance.actionsInScope",
    "task.frontendExperienceRequirement.executionGuidance.operationPathsInScope",
    "task.frontendExperienceRequirement.executionGuidance.operationPathEvidenceRule",
    "task.frontendExperienceRequirement.executionGuidance.mustNot",
    "task.frontendExperienceRequirement.executionGuidance.dataBindingExpectation",
    "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
    "task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource",
    "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
    "task.frontendExperienceRequirement.executionGuidance.bindingProjectionRules",
    "task.frontendExperienceRequirement.executionGuidance.bindingProjectionSummary",
    "task.frontendExperienceRequirement.executionGuidance.taskResultEvidenceGuide",
    "task.frontendExperienceRequirement.executionGuidance.guidanceWarnings",
    "task.frontendExperienceRequirement.executionGuidance.unresolvedBindingInputs",
  ] : [];
  const taskResultContractReadFields = [
    "enumRefs",
    "outputContract.resultFile",
    "outputContract.requiredTopLevelFields",
    "outputContract.requiredTopLevelFieldRule",
    ...(requiredRuntimeEvidence ? ["outputContract.requiredRuntimeEvidence"] : []),
    "outputContract.statusEnum",
    "outputContract.resultRules",
    "outputContract.schemaShape.status",
    "outputContract.schemaShape.changedFiles",
    "outputContract.schemaShape.noChangeReason",
    "outputContract.schemaShape.verificationResults",
    "outputContract.schemaShape.allowedVerificationResults",
    "outputContract.schemaShape.selfRepairSummary",
    "outputContract.schemaShape.selfRepairSummaryExamples",
    "outputContract.schemaShape.selfRepairSummaryRules",
    "outputContract.schemaShape.failure",
    "outputContract.schemaShape.executionContinuity",
    ...(requestTask.frontendExperienceRequirement ? [
      "outputContract.schemaShape.frontendExperienceSelfCheck.status",
      "outputContract.schemaShape.frontendExperienceSelfCheck.dataBinding",
      "outputContract.schemaShape.frontendExperienceSelfCheck.operationPathsCovered",
      "outputContract.schemaShape.frontendExperienceSelfCheck.knownGaps",
    ] : []),
    ...(requirementDetailSnapshot.length > 0 ? ["outputContract.schemaShape.requirementDetailEvidence"] : []),
    ...(requestTask.conceptRefs && requestTask.conceptRefs.length > 0 ? ["outputContract.schemaShape.conceptEvidence"] : []),
    "outputContract.schemaShape.notes",
    "outputContract.schemaShape.blockedReasons",
    "outputContract.schemaShape.createdAt",
    "outputContract.schemaShape.updatedAt",
  ];
  const request: TaskExecutionRequest = {
    schemaVersion: "1.0",
    requestId,
    requestType: "execute_task",
    agentAction: agentActionContract({
      actionKind: "execute_task",
      instruction: "Execute this exact task against project files, write TaskResult to outputContract.resultFile, then run submitCommand exactly. Do not stop after reading the request, and do not stop with a progress summary before submitCommand succeeds.",
      actionRequired: taskExecutionActionRequired({
        taskId: requestTask.taskId,
        resultFile,
        submitCommand,
        completionBarrier,
        finalResponseGuard,
        completionContinuityRequirement,
      }),
      finalResponseGuard,
      read: {
        required: uniqueRefs(["this request", "referencedArtifactReadGuide", "task", ...frontendGuidanceReadFields, "sourceContext.architectureArtifactProjection", "sourceContext.acceptanceSnapshot", "sourceContext.userFacingLanguage", ...(requirementDetailSnapshot.length > 0 ? ["sourceContext.requirementDetailSnapshot"] : []), "executionRules", ...taskResultContractReadFields, "taskConceptGrounding when mode=concepts_present"]),
        optional: uniqueRefs([
          "sourceRefs",
          ...(requestTask.frontendExperienceRequirement ? [
            "outputContract.schemaShape.frontendExperienceSelfCheck.requirementRef",
            "outputContract.schemaShape.frontendExperienceSelfCheck.surfacesTouched",
            "outputContract.schemaShape.frontendExperienceSelfCheck.workflowsCovered",
            "outputContract.schemaShape.frontendExperienceSelfCheck.dataViewsUsed",
            "outputContract.schemaShape.frontendExperienceSelfCheck.actionsImplemented",
            "outputContract.schemaShape.frontendExperienceSelfCheck.operationPathsCovered",
            "outputContract.schemaShape.frontendExperienceSelfCheck.userActionsImplemented",
            "outputContract.schemaShape.frontendExperienceSelfCheck.interactionStatesCovered",
            "outputContract.schemaShape.frontendExperienceSelfCheck.notes",
            "outputContract.schemaShape.frontendExperienceSelfCheck.closureRequirementIds",
            "outputContract.schemaShape.frontendExperienceSelfCheck.workflowClosureEvidenceRule",
          ] : ["task.frontendExperienceRequirement"]),
          ...(requestTask.runtimeDeliveryRequirement ? ["outputContract.schemaShape.runtimeDeliveryEvidence"] : []),
          "task.runtimeDeliveryRequirement",
          "sourceContext.dependencyResults",
        ]),
        displayPolicy: "compact",
      },
      write: {
        resultFile,
        requiredTopLevelFields: taskResultRequiredTopLevelFields,
        requiredTopLevelFieldRule: "TaskResult must include every requiredTopLevelFields entry even when the value is null or an empty array.",
        ...(requiredRuntimeEvidence ? { requiredRuntimeEvidence } : {}),
        rules: [
          "Modify project files only as needed for this task.",
          "Write TaskResult JSON only to outputContract.resultFile.",
          "Before writing TaskResult, copy every field named in agentAction.write.requiredTopLevelFields. Do not omit blockedReasons, createdAt, or updatedAt just because the task completed successfully.",
          "Do not send progress-only summaries, interim handoff notes, or next-step summaries before TaskResult is written and submitCommand succeeds.",
          "Agent-chosen verification methods must return control before TaskResult submission; do not leave the task waiting on a long-running command, browser session, interactive tool, server, watcher, worker, progress summary, or handoff note.",
          "Use outputContract.schemaShape and enumRefs exactly.",
          "When choosing browser/e2e/interactive verification, follow executionRules.interactiveVerificationProbePolicy before running the tool.",
          "When task.frontendExperienceRequirement.executionGuidance is present, use it to decide the surfaces, workflows, operation paths, interaction states, and data-binding evidence for this task.",
          "When task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings is present, use it as the first coding guide for wiring user actions to AAC-declared interfaces, including method/path/schemas and success/error UI states.",
          "When operationPathsInScope/dataViewsInScope/actionsInScope are present, implement or verify the declared target discovery, selection, action entry, refresh policy, and visible feedback; record matching evidence in frontendExperienceSelfCheck.",
          userFacingLanguageRule(userFacingLanguage),
          "When sourceContext.requirementDetailSnapshot is non-empty, use it as the task-scoped requirement detail authority. Implement and verify those detailIds through task.requirementDetailRefs and verificationIntents[].requirementDetailRefs.",
          "When task objective, acceptanceSnapshot, taskConceptGrounding, or sourceContext.architectureArtifactProjection names key fields, object operations, operation inputs, preconditions, blocking reasons, state changes, or feedback, implement and verify those details explicitly. Do not replace them with a generic completed feature summary.",
          ...(requestTask.frontendExperienceRequirement ? frontendImplementationOrganizationRules : []),
          "frontendBackendBindings is not an API allowlist. If a needed interface is missing from the projection, read sourceRefs.architectureArtifactContractRef, sourceRefs.taskPlanRef, and project source before deciding; do not invent an undeclared API.",
          "If unresolvedBindingInputs is non-empty, use its readToResolve guidance to continue coding when possible, or record the remaining data-binding gap in frontendExperienceSelfCheck. Do not stop only because CLI projection was incomplete.",
          ...(workflowClosureRequirements.length > 0 ? [
            "This task has closureRequirementRefs. The workflow is not satisfied by a static/demo-only UI or by known gaps. To write frontendExperienceSelfCheck.status=satisfied, dataBinding.mode must be wired and knownGaps must be empty.",
            "For every closureRequirementRefs[] item, implement or verify user action, declared interface invocation, state or persistence change, and success or blocking feedback.",
          ] : []),
          ...(requiredRuntimeEvidence ? [
            "RuntimeDeliveryEvidence is required. Copy every exact checkId from agentAction.write.requiredRuntimeEvidence.requiredCheckIds into runtimeDeliveryEvidence.codeLevelChecks[].checkId.",
            "Do not create generic runtime checkIds such as build/test/lint/probe; put command details in runtimeDeliveryEvidence.commandsRun.",
          ] : []),
          "changedFiles must list intended deliverables only, not node_modules, caches, logs, dist, build, or other incidental side effects.",
          "When task.conceptRefs is non-empty, write conceptEvidence for every task conceptRef using the exact concept ids from taskConceptGrounding.",
          "When task.requirementDetailRefs is non-empty, write requirementDetailEvidence for every task.requirementDetailRefs detailId using the exact detailId values from sourceContext.requirementDetailSnapshot.",
          "When conceptEvidence is required for an object operation, make the summary identify the concrete field meaning, operation invariant, validation/blocking reason, state transition, or visible feedback that was protected.",
          "If verification fails after allowed self-repair, still submit a failed or blocked TaskResult instead of asking the user whether to continue.",
        ],
      },
      submit: {
        command: submitCommand,
        requiredArgs: ["--delivery-id", "--phase-id", "--input-file"],
        placeholders: { "{resultFile}": resultFile },
        runAfter: "resultFile exists",
      },
      schema: {
        primary: "TaskResult",
        shapeLocation: "outputContract.schemaShape",
        enumLocation: "enumRefs",
        allowedRefsLocation: "task and sourceContext",
      },
      stopConditions: ["request cannot be read", "submitCommand returns non-repairable failure", "returned instruction is user-gated"],
    }),
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: false,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: false,
      modifyProjectFilesAllowed: true,
      ifBlockedWriteBlockedOutput: true,
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
      contextReadOutputPolicy: taskExecutionOutputPolicy().contextReadOutputPolicy,
      sourceChangeOutputPolicy: taskExecutionOutputPolicy().sourceChangeOutputPolicy,
    },
    enumRefs: {
      taskResultStatus: [...taskResultStatusSchema.options],
      verificationStatus: ["passed", "not_run", "failed", "inconclusive"],
      verificationEvidence: [...verificationEvidenceSchema.options],
      conceptEvidenceType: [...conceptEvidenceTypeSchema.options],
      blockedCode: Object.keys(BLOCKED_OUTPUT_MAPPING),
      blockedNextNode: ["architecture_artifact_repair", "taskplan_repair", "wait_dependency"],
      selfRepairStopReason: [
        "not_attempted",
        "verification_passed",
        "blocked_condition_detected",
        "same_failure_repeated_without_progress",
        "hard_attempt_limit_reached",
        "repair_requires_contract_change",
        "repair_requires_scope_expansion",
      ],
    },
    source: {
      taskPlanId: taskPlan.taskPlanId,
      taskId: requestTask.taskId,
      groupId: requestTask.groupId,
      technicalBaselineId: baseline.technicalBaselineId,
      architectureArtifactContractId: aac.architectureArtifactContractId,
      taskPlanRunId: run.runId,
    },
    sourceRefs: {
      technicalBaselineRef: toProjectRelative(projectRoot, technicalBaselinePath(projectRoot, locator.deliveryId)),
      planningGenerationContractRef: toProjectRelative(projectRoot, planningContractPath(projectRoot, pgc.planningContractId, locator)),
      architectureArtifactContractRef: toProjectRelative(projectRoot, architectureContractPath(projectRoot, aac.architectureArtifactContractId, locator)),
      taskPlanRef: toProjectRelative(projectRoot, resolveTaskPlanPath(projectRoot, taskPlan.taskPlanId, locator)),
      taskPlanRunRef: toProjectRelative(projectRoot, taskPlanRunPath(projectRoot, run.runId, locator)),
      ...(phaseConceptGroundingRef ? { phaseConceptGroundingRef } : {}),
    },
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      technicalBaselineRef: toProjectRelative(projectRoot, technicalBaselinePath(projectRoot, locator.deliveryId)),
      planningGenerationContractRef: toProjectRelative(projectRoot, planningContractPath(projectRoot, pgc.planningContractId, locator)),
      architectureArtifactContractRef: toProjectRelative(projectRoot, architectureContractPath(projectRoot, aac.architectureArtifactContractId, locator)),
      taskPlanRef: toProjectRelative(projectRoot, resolveTaskPlanPath(projectRoot, taskPlan.taskPlanId, locator)),
      taskPlanRunRef: toProjectRelative(projectRoot, taskPlanRunPath(projectRoot, run.runId, locator)),
      phaseConceptGroundingRef,
    }),
    task: requestTask,
    sourceContext: {
      technicalBaseline: buildTechnicalBaselineSummary(baseline),
      architectureArtifactProjection: buildArchitectureProjection(aac, requestTask),
      acceptanceSnapshot: buildTaskAcceptanceSnapshot(pgc, aac, requestTask),
      requirementDetailSnapshot,
      userFacingLanguage,
      dependencyResults: run.taskStates
        .filter((state) => requestTask.dependsOn.includes(state.taskId) && state.resultId)
        .map((state) => ({
          taskId: state.taskId,
          status: state.status,
          resultId: state.resultId,
        })),
    },
    executionRules: {
      completionBarrier: {
        ...completionBarrier,
      },
      completionContinuityRequirement,
      sourceEditPreparationContract: sourceEditPreparationContract({
        resultFile,
        submitCommandName: submitCommand.name,
      }),
      userFacingLanguage: {
        constraint: userFacingLanguage,
        rule: userFacingLanguageRule(userFacingLanguage),
      },
      interactiveVerificationProbePolicy: interactiveVerificationProbePolicy(),
      rules: [
        "Execute only the given task.",
        "Before any project source edit or TaskResult artifact write, follow sourceEditPreparationContract in this executionRules object.",
        "This task is not complete until TaskResult JSON exists at outputContract.resultFile and submitCommand has been run successfully.",
        "Do not stop with a progress summary, partial completion note, or next-step handoff before writing TaskResult and running submitCommand.",
        "Do not modify Brainstorm, TechnicalBaseline, PGC, AAC, or TaskPlan.",
        "Do not implement deferred scope.",
        "Do not modify .loom except writing the exact candidate/result file required by the current loom request.",
        "You may run package-manager, build, test, and tooling commands that create workspace side-effect files such as dependency directories, caches, logs, or build output.",
        "Verification method is agent-chosen, but it must return control before TaskResult submission.",
        "Do not leave the task waiting on any long-running command, browser session, interactive tool, server, watcher, worker, progress summary, or handoff note before record-result.",
        "When choosing browser/e2e/interactive verification, follow interactiveVerificationProbePolicy before running the tool.",
        ...verificationCommandSchedulingRules,
        ...controlledRuntimeProbeRules,
        "If this task starts a temporary local runtime, dev server, preview server, container, or probe process, record the PID/port/command when available and attempt to stop only that task-owned runtime before writing TaskResult.",
        "If a runtime/server command is already running and has shown a ready URL, listening port, or health-ready signal, do not wait for it to exit naturally. Probe the ready target, stop only task-owned runtime when safe, then write TaskResult and run submitCommand.",
        "If runtime probe cleanup fails, is unknown, or is not safe because the runtime was not clearly started by this task, use status completed_with_notes with notes and runtimeDeliveryEvidence.runtimeProbeCleanup; do not use failed or blocked for cleanup alone.",
        "If dependency installation succeeds, rerun the relevant verification before writing TaskResult.",
        "Only record verification status not_run for missing dependencies after attempting the allowed package-manager install or when installation is impossible; explain the attempted command and failure in notes.",
        "Do not implement full lifecycle for reference_only entities.",
        "Preserve task-scoped requirement details from sourceContext.requirementDetailSnapshot. When present, each listed detailId must be addressed by implementation or verification within the current task boundary.",
        "Preserve task-scoped object-operation details from acceptanceSnapshot, taskConceptGrounding, and architectureArtifactProjection: key fields, operation inputs, preconditions, validation/blocking reasons, success states, state transitions, and visible feedback.",
        userFacingLanguageRule(userFacingLanguage),
        "Use project structure and framework conventions discovered in the workspace.",
        compactContextReadStep,
        "Record intended deliverable changes in TaskResult.changedFiles: source, tests, configs, manifests, lockfiles, docs, and other task-relevant project files.",
        "If implementation or verification cannot be completed inside this task boundary, write a failed or blocked TaskResult and run submitCommand; do not leave the task running with only a chat summary.",
        "Record executionContinuity in TaskResult. If any agent-owned long-running work may still be unreleased, do not claim pure completed; use completed_with_notes with notes unless an independent failure or blocked condition remains.",
        "Do not list incidental workspace side effects such as dependency directories, caches, logs, or generated build output in changedFiles unless they are intended deliverables.",
        "Keep chat output compact while editing source: do not paste unified diffs, large patches, full source file contents, or full git diff output unless the user explicitly asks.",
        "Avoid apply_patch or any chat-visible patch workflow for source edits when it would paste a large source patch into chat; use quiet file editing and report changed paths instead.",
        "When reporting source changes, list changed file paths and a short summary instead of printing diffs.",
      ],
      contextReadOutputPolicy: taskExecutionOutputPolicy().contextReadOutputPolicy,
      sourceChangeOutputPolicy: taskExecutionOutputPolicy().sourceChangeOutputPolicy,
      environmentPreparation: {
        packageManager: baseline.stack.packageManager,
        installAllowedWhen: [
          "package manifest exists",
          "verification command fails because local dependencies or test runner binaries are missing",
          "install command stays within the current project workspace",
          "install does not require changing loom artifacts or forbidden paths by hand",
        ],
        installGuidance: [
          "Use the package manager declared by TechnicalBaseline.",
          "For npm projects, prefer npm install so package-lock.json can be created or updated consistently when absent.",
          "Treat generated lockfile changes as project changes and include them in changedFiles when they are relevant deliverables.",
          "Dependency directories and caches are allowed workspace side effects, but do not list them in changedFiles unless explicitly intended by the task.",
        ],
      },
      selfRepairPolicy: {
        enabled: true,
        mode: "progress_based",
        softAttemptLimit: 3,
        hardAttemptLimit: 8,
        progressSignals: [
          "failure_signature_changed",
          "failing_test_count_decreased",
          "type_error_count_decreased",
          "build_progressed_to_later_stage",
          "runtime_error_moved_to_different_boundary",
          "verification_scope_reduced_to_known_remaining_issue",
        ],
        continueWhen: [
          "failure_is_within_current_task_boundary",
          "progress_signal_detected",
          "repair_does_not_require_contract_change",
          "repair_does_not_expand_scope",
          "repair_does_not_touch_forbidden_paths",
        ],
        stopConditions: [
          "verification_passed",
          "blocked_condition_detected",
          "same_failure_repeated_without_progress",
          "hard_attempt_limit_reached",
          "repair_requires_contract_change",
          "repair_requires_scope_expansion",
        ],
      },
      frontendExperienceExecutionRules: requestTask.frontendExperienceRequirement ? {
        readFrontendExperienceRequirement: true,
        executionGuidanceField: "task.frontendExperienceRequirement.executionGuidance",
        frontendBackendBindingsField: "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
        unresolvedBindingInputsField: "task.frontendExperienceRequirement.executionGuidance.unresolvedBindingInputs",
        closureRequirementRefsField: "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
        workflowClosureDetailSourceField: "task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource",
        operationPathsField: "task.frontendExperienceRequirement.executionGuidance.operationPathsInScope",
        dataViewsField: "task.frontendExperienceRequirement.executionGuidance.dataViewsInScope",
        actionsField: "task.frontendExperienceRequirement.executionGuidance.actionsInScope",
        useExecutionGuidanceWhenPresent: true,
        useFrontendBackendBindingsFirst: true,
        frontendBackendBindingsAreNotAllowlist: true,
        userFacingLanguage,
        userFacingLanguageRule: userFacingLanguageRule(userFacingLanguage),
        executionGuidanceIsNonBlocking: true,
        mustKeepUiAlignedWithContract: true,
        doNotCreateDemoOnlyUiWhenUsableProductRequired: true,
        recordFrontendExperienceSelfCheck: true,
        workflowClosureRequired: workflowClosureRequirements.length > 0,
        workflowClosureRequirementIds: workflowClosureRequirements.map((requirement) => String(requirement.closureId ?? "")),
        workflowClosureSatisfactionRule: workflowClosureRequirements.length > 0
          ? "When workflowClosureRequired=true, frontendExperienceSelfCheck.status=satisfied requires dataBinding.mode=wired, knownGaps=[], and evidence for user action, declared interface invocation, state/persistence change, and success/blocking feedback."
          : "No workflow closure requirement is assigned to this task.",
        implementationOrganizationRules: frontendImplementationOrganizationRules,
        unresolvedBindingInputsRule: "Use unresolvedBindingInputs to read AAC/TaskPlan/source and continue when the CLI projection is incomplete; record a data-binding gap only when the binding still cannot be determined.",
        evidenceRule: "Fill outputContract.schemaShape.frontendExperienceSelfCheck using task.frontendExperienceRequirement.executionGuidance and frontendBackendBindings when present.",
      } : undefined,
      runtimeDeliveryExecutionRules: requestTask.runtimeDeliveryRequirement ? {
        readRuntimeDeliveryRequirement: true,
        readRuntimeDeliveryContract: true,
        verificationBoundary: "code_level_only",
        mustKeepContractAndCodeAligned: true,
        mayEditApplicationCode: true,
        mayEditPackageScripts: true,
        mayEditDeployGeneratedFiles: false,
        mayEditRuntimeDeliveryContract: false,
        doNotRequireCleanInstallOrContainerBuild: true,
        mustNotUseSleepPlaceholder: true,
        mustRecordRuntimeDeliveryEvidence: true,
        mustRecordRuntimeProbeCleanupWhenTemporaryRuntimeStarted: true,
        foregroundRuntimeCommandsForbidden: true,
        controlledRuntimeProbeRequired: true,
        runtimeCommandGuardRules: controlledRuntimeProbeRules,
        runtimeProbeCleanupFailureSeverity: "completed_with_notes_only",
        selfRepairWhenCodeLevelCheckFails: true,
      } : undefined,
    },
    taskConceptGrounding: buildTaskConceptGrounding(requestTask, phaseConceptGroundingRef),
    blockedOutput: {
      mapping: BLOCKED_OUTPUT_MAPPING,
    },
    outputContract: {
      format: "json",
      schema: "TaskResult",
      resultFile,
      requiredTopLevelFields: taskResultRequiredTopLevelFields,
      requiredTopLevelFieldRule: "TaskResult must include every requiredTopLevelFields entry. Use notes: [] and blockedReasons: [] when empty; use ISO datetimes for createdAt and updatedAt.",
      completionContinuityRequirement,
      ...(requiredRuntimeEvidence ? { requiredRuntimeEvidence } : {}),
      schemaShape: taskResultSchemaShape(taskPlan, requestTask),
      statusEnum: ["completed", "completed_with_notes", "blocked", "failed"],
      resultRules: [
        "TaskExecution is not complete until this TaskResult is written to resultFile and submitCommand succeeds.",
        "TaskResult must include every outputContract.requiredTopLevelFields entry; required empty collections such as notes and blockedReasons must be present as empty arrays.",
        "TaskResult must include executionContinuity. Set taskResultSubmittedAfterVerification=true only when the chosen verification path returned control and this TaskResult is being submitted.",
        "If executionContinuity.agentOwnedLongRunningWork is unknown, status cannot be completed; use completed_with_notes with notes unless an independent failure or blocked condition remains.",
        "changedFiles is required and may be an empty array.",
        "changedFiles should contain intended deliverable files, not incidental dependency directories, caches, logs, or build output.",
        "If status is completed or completed_with_notes and changedFiles is empty, the task must be verification_increment or noChangeReason.code must be NO_CODE_CHANGE_REQUIRED, VERIFICATION_ONLY_TASK, or ENVIRONMENT_CHECK_ONLY; otherwise record-result will reject the TaskResult and return repairInstruction.",
        "If status is completed, every verificationIntent must have a passed verificationResult.",
        "For tasks with runtimeDeliveryRequirement.appliesToThisTask=true, include runtimeDeliveryEvidence with checkedFields, codeLevelChecks, commandsRun when commands were run, and unverifiedItems when environment prevents a check.",
        "For runtimeDeliveryEvidence.codeLevelChecks, use only the exact checkId values listed in outputContract.schemaShape.runtimeDeliveryEvidence.requiredCheckIds and task.runtimeDeliveryRequirement.requiredCodeLevelChecks[].checkId.",
        "If a temporary runtime/probe/server/container was started during this task, include runtimeDeliveryEvidence.runtimeProbeCleanup. Cleanup failed/unknown/not_safe_to_cleanup is non-blocking: use completed_with_notes and notes, not failed or blocked, unless there is an independent implementation or verification defect.",
        "If a foreground runtime command has already reported ready/listening, do not wait for natural process exit before writing TaskResult. Finish the probe, record cleanup state, and submit.",
        "For browser/e2e/interactive verification, follow executionRules.interactiveVerificationProbePolicy and record evidence through existing TaskResult fields.",
        "For frontend tasks, use task.frontendExperienceRequirement.executionGuidance when present and fill frontendExperienceSelfCheck with surfaces, workflows, user actions, states, data binding mode, and known gaps.",
        "For frontend tasks, use frontendBackendBindings as coding guidance for user-action-to-interface wiring. If a needed binding is absent, read AAC/TaskPlan/source and continue; do not treat projection absence as a validator failure.",
        "When task acceptance or concept grounding names concrete object fields, object operations, blocking reasons, state changes, or visible feedback, verificationResults and conceptEvidence must mention the matching implemented or verified behavior instead of only reporting that the module exists.",
        "When sourceContext.requirementDetailSnapshot is non-empty, verificationResults should identify the relevant detailId or detail title in the summary for each verificationIntents[].requirementDetailRefs item.",
        "When task.requirementDetailRefs is non-empty, requirementDetailEvidence must include each detailId with status, verificationIds, evidenceRefs, and a summary tied to implementation or verification evidence.",
        ...(workflowClosureRequirements.length > 0 ? [
          "For tasks with closureRequirementRefs, frontendExperienceSelfCheck.status=satisfied is valid only when frontendExperienceSelfCheck.dataBinding.mode=wired and frontendExperienceSelfCheck.knownGaps is empty.",
          "If the task remains static_only_with_reason, mocked_with_reason, or has knownGaps for required closure, use partially_satisfied/not_satisfied in frontendExperienceSelfCheck and completed_with_notes/failed/blocked according to the actual implementation and verification outcome.",
        ] : []),
        "For taskKind runtime_delivery_closure, runtimeDeliveryEvidence.checkedFields must cover every affectedContractFields entry and codeLevelChecks must report every requiredCodeLevelChecks item.",
        "For taskKind runtime_delivery_closure, verify code-level closure for build.command, start.command, runtimeSurfaces, httpProbes, deliveryMechanics.staticAssets, deliveryMechanics.api, frontend, api, environment, and deliveryMechanics.codegen when codegen is declared.",
        "RuntimeDeliveryEvidence is code-level evidence. Do not require clean install, container build, registry access, browser proof, or full deploy for TaskResult completion.",
        "If status is failed, failure is required.",
        "verificationResults[].status records whether verification passed; selfRepairSummary.stopReason records only a self-repair loop stop condition.",
        "If no self-repair loop was attempted, always use selfRepairSummary exactly { attempted:false, attemptCount:0, stopReason:'not_attempted', progressObserved:false }, even when verificationResults[].status is passed.",
        "Use selfRepairSummary.stopReason verification_passed only when you actually attempted self-repair after a failure, then reran verification, and that post-repair verification passed; in that case attempted must be true and attemptCount must be 1..8.",
        "Never combine attempted=false with stopReason verification_passed.",
        "If status is failed, selfRepairSummary is required and may use attempted=false with stopReason not_attempted.",
        "If no self-repair was attempted, selfRepairSummary must be exactly { attempted:false, attemptCount:0, stopReason:'not_attempted', progressObserved:false }.",
        "Use stopReason verification_passed only when self-repair was actually attempted and verification passed after that repair; then attempted must be true and attemptCount must be greater than 0.",
        "If verification still fails after allowed self-repair, do not stop in chat and ask the user what to do. Write a TaskResult with status failed or blocked, include failure/blocking details and verification evidence, then run submitCommand so loom can route repair or manual review.",
        "Use status blocked when the stop reason matches blockedOutput mapping or requires contract/scope/dependency decisions; use status failed when implementation or verification remains broken within the current task boundary.",
      ],
    },
    submitCommand,
    postSubmitRouting: {
      submitCommandReturnsInstruction: true,
      followReturnedInstructionImmediately: true,
      doNotAskUserAfterSuccessfulSubmit: true,
      continueAutomaticallyWhenNextActionIs: [
        "continue_execution",
        "review",
        "execution_repair",
        "taskplan_repair",
        "architecture_artifact_repair",
      ],
      stopOnlyWhen: [
        "done",
        "ask_user without a usable answer in the current user message",
        "manual_review",
        "needs_user_decision",
        "report_blocked",
        "unrecoverable command failure",
      ],
      rule: "After record-result succeeds, follow data.instruction immediately. Do not summarize and ask whether to continue while the next action is auto-runnable.",
    },
    createdAt: new Date().toISOString(),
  };
  return taskExecutionRequestSchema.parse(request);
}

function taskResultSchemaShape(taskPlan: TaskPlan, task: Task): Record<string, unknown> {
  const runtimeRequiredChecks = task.runtimeDeliveryRequirement?.requiredCodeLevelChecks ?? [];
  const runtimeAffectedFields = task.runtimeDeliveryRequirement?.affectedContractFields ?? [];
  const workflowClosureRequirements = workflowClosureRequirementsFromTaskGuidance(task);
  return {
    schemaVersion: "1.0",
    taskResultId: `result-${task.taskId}`,
    taskId: task.taskId,
    taskPlanId: taskPlan.taskPlanId,
    status: "completed | completed_with_notes | blocked | failed",
    changedFiles: ["project-relative file path"],
    noChangeReason: {
      code: "NO_CODE_CHANGE_REQUIRED | VERIFICATION_ONLY_TASK | ENVIRONMENT_CHECK_ONLY",
      summary: "Required only when changedFiles is empty for a completed/completed_with_notes result. Use null when this task changed project files.",
    },
    verificationResults: task.verificationIntents.map((intent) => ({
      verificationId: intent.verificationId,
      status: "passed | not_run | failed | inconclusive",
      evidenceType: intent.acceptableEvidence[0] ?? "manual_command_output",
      allowedEvidenceTypes: intent.acceptableEvidence,
      evidenceTypeRule: "Use exactly one allowedEvidenceTypes value. Do not use conceptEvidenceType values such as code/test/ui/runtime here unless that exact value is listed in allowedEvidenceTypes.",
      summary: "Short verification result summary.",
    })),
    allowedVerificationResults: task.verificationIntents.map((intent) => ({
      verificationId: intent.verificationId,
      acceptableEvidence: intent.acceptableEvidence,
      rule: "Use this exact verificationId. Do not create extra verificationResults for individual commands; summarize command evidence under this result or runtimeDeliveryEvidence.commandsRun.",
    })),
    selfRepairSummary: {
      attempted: "boolean. Use false when you did not run a self-repair loop after a verification failure.",
      attemptCount: "0 when attempted=false; integer 1..8 when attempted=true.",
      stopReason: "not_attempted when attempted=false. Use verification_passed only when attempted=true and verification passed after self-repair. Other attempted=true stop reasons: blocked_condition_detected | same_failure_repeated_without_progress | hard_attempt_limit_reached | repair_requires_contract_change | repair_requires_scope_expansion.",
      progressObserved: "false when attempted=false; boolean progress signal when attempted=true.",
    },
    selfRepairSummaryExamples: {
      noSelfRepairAttempted: {
        attempted: false,
        attemptCount: 0,
        stopReason: "not_attempted",
        progressObserved: false,
      },
      selfRepairAttemptedAndVerificationPassed: {
        attempted: true,
        attemptCount: 1,
        stopReason: "verification_passed",
        progressObserved: true,
      },
    },
    selfRepairSummaryRules: taskResultResultRules(),
    failure: null,
    executionContinuity: {
      taskResultSubmittedAfterVerification: "boolean. Use true only when the task's chosen verification path returned control and TaskResult will be submitted with submitCommand now.",
      agentOwnedLongRunningWork: "none | started_and_released | unknown. Use unknown when any agent-started long-running command, browser session, interactive tool, server, watcher, or worker may still be unreleased.",
      notes: ["Required details when agentOwnedLongRunningWork is unknown; otherwise [] is allowed."],
    },
    frontendExperienceSelfCheck: task.frontendExperienceRequirement ? {
      requirementRef: "task.frontendExperienceRequirement.frontendExperienceRef",
      status: "satisfied | partially_satisfied | not_satisfied",
      surfacesTouched: ["surface-id from task.frontendExperienceRequirement.executionGuidance.surfacesInScope"],
      workflowsCovered: [{
        workflowRef: "workflow ref from task.frontendExperienceRequirement.executionGuidance.workflowsInScope",
        coverage: "implemented | wired | verified | static_only | not_applicable",
        evidenceRefs: ["project-relative UI/test/runtime evidence file"],
        summary: "What user-visible behavior this task added or verified.",
      }],
      dataViewsUsed: [{
        viewRef: "view id from task.frontendExperienceRequirement.executionGuidance.dataViewsInScope",
        coverage: "implemented | wired | verified | not_applicable",
        evidenceRefs: ["project-relative UI/test/runtime evidence file"],
        summary: "How users can find, select, or inspect the target object through this view.",
      }],
      actionsImplemented: [{
        actionRef: "action id from task.frontendExperienceRequirement.executionGuidance.actionsInScope",
        coverage: "implemented | wired | verified | static_only | not_applicable",
        evidenceRefs: ["project-relative UI/test/runtime evidence file"],
        summary: "How the user triggers the action and sees success, blocking, or error feedback.",
      }],
      operationPathsCovered: [{
        pathRef: "path id from task.frontendExperienceRequirement.executionGuidance.operationPathsInScope",
        coverage: "implemented | wired | verified | static_only | not_applicable",
        evidenceRefs: ["project-relative UI/test/runtime evidence file"],
        summary: "How the target discovery, action entry, refresh/result observation, and feedback path is covered.",
      }],
      userActionsImplemented: ["plain-language user action implemented or verified"],
      interactionStatesCovered: ["idle | loading | success | error | empty | business_blocking"],
      dataBinding: {
        mode: "wired | mocked_with_reason | static_only_with_reason | not_applicable",
        ...(workflowClosureRequirements.length > 0 ? {
          requiredModeForSatisfaction: "wired",
          closureRequirementIds: workflowClosureRequirements.map((requirement) => String(requirement.closureId ?? "")),
          modeRule: "When closureRequirementIds is non-empty, status=satisfied requires mode=wired. mocked_with_reason/static_only_with_reason/not_applicable cannot be satisfied for these closure requirements.",
        } : {}),
        interfaceRefs: ["AAC interface refs actually used or verified by this task; this is evidence, not a validator allowlist"],
        interfaceRolesCovered: ["read_model | command | readback | component_binding | external_contract | unknown"],
        evidenceRefs: ["project-relative refs"],
        notes: ["which user actions were wired to read_model/command/readback interfaces, or why binding remains a known gap"],
      },
      ...(workflowClosureRequirements.length > 0 ? {
        closureRequirementIds: workflowClosureRequirements.map((requirement) => String(requirement.closureId ?? "")),
        workflowClosureEvidenceRule: "For each closureRequirementIds item, evidence must cover user_action, declared_interface_invocation, state_or_persistence_change, and success_or_blocking_feedback. If any remains a known gap, do not write status=satisfied.",
      } : {}),
      knownGaps: ["remaining frontend gaps or []"],
      notes: ["How the UI work satisfies or partially satisfies the frontend requirement."],
    } : undefined,
    requirementDetailEvidence: (task.requirementDetailRefs ?? []).map((detailId) => ({
      detailId,
      status: "satisfied | partially_satisfied | not_satisfied | not_verified",
      verificationIds: task.verificationIntents
        .filter((intent) => (intent.requirementDetailRefs ?? []).includes(detailId))
        .map((intent) => intent.verificationId),
      evidenceRefs: ["project-relative changed file, test, API/UI/runtime evidence ref, or verificationId"],
      summary: "How this task implemented or verified the detail. Mention the concrete field, rule, state, flow, UI/API binding, blocking reason, or feedback covered.",
    })),
    conceptEvidence: (task.conceptRefs ?? []).length > 0 ? (task.conceptRefs ?? []).map((conceptRef) => ({
      conceptRef,
      evidenceType: "code | test | api | ui | runtime | documentation",
      refs: ["project-relative code/test/UI/API/runtime evidence file"],
      summary: "How this task implemented or protected the concept without misinterpretation. For object-operation concepts, name the field meaning, operation invariant, validation/blocking reason, state transition, or visible feedback covered.",
    })) : undefined,
    runtimeDeliveryEvidence: task.runtimeDeliveryRequirement ? {
      requirementRef: "task.runtimeDeliveryRequirement.runtimeDeliveryRef",
      allowedCheckedFields: runtimeAffectedFields,
      requiredCheckIds: runtimeRequiredChecks.map((check) => check.checkId),
      allowedCodeLevelChecks: runtimeRequiredChecks.map((check) => ({
        checkId: check.checkId,
        contractField: check.contractField,
        objective: check.objective,
        acceptableEvidence: check.acceptableEvidence,
      })),
      checkedFields: runtimeAffectedFields.length > 0 ? runtimeAffectedFields : ["field from task.runtimeDeliveryRequirement.affectedContractFields"],
      codeLevelChecks: runtimeRequiredChecks.length > 0
        ? runtimeRequiredChecks.map((check) => ({
            checkId: check.checkId,
            contractField: check.contractField,
            status: "passed | failed | blocked | not_applicable",
            evidence: `Short code-level evidence for ${check.objective}`,
            reason: "Required when status is failed, blocked, or not_applicable.",
          }))
        : [{
            checkId: "exact checkId from task.runtimeDeliveryRequirement.requiredCodeLevelChecks",
            contractField: "field from task.runtimeDeliveryRequirement.requiredCodeLevelChecks",
            status: "passed | failed | blocked | not_applicable",
            evidence: "Short code-level evidence summary.",
            reason: "Required when status is failed, blocked, or not_applicable.",
          }],
      codeLevelCheckRules: [
        "Use only checkId values from requiredCheckIds.",
        "Do not invent generic runtime checkIds.",
        "runtimeDeliveryEvidence.codeLevelChecks[].reason is optional. Omit it when the check passed; use a non-empty string only when status is failed, blocked, or not_applicable. Never write reason: null.",
        "For runtime_delivery_closure, include exactly one codeLevelChecks item for every requiredCheckIds entry unless status is blocked and blockedReasons explains why.",
        "Do not run long-lived runtime/server commands as foreground verification commands.",
        "Live runtime probes, when needed, must use a bounded task-owned background runtime and must be cleaned up before TaskResult is written.",
      ],
      commandsRun: [{
        command: "local command run for this task if any",
        status: "passed | failed | not_run",
        environment: "local_warm | project_workspace | unknown",
        summary: "Short command outcome.",
      }],
      unverifiedItems: [{
        item: "field or check that could not be verified",
        reason: "Environment/dependency reason or why it is not applicable.",
      }],
      runtimeProbeCleanup: {
        temporaryRuntimeStarted: "boolean. true only when this task started a temporary local runtime, dev server, preview server, container, or probe process.",
        attempted: "boolean. true when cleanup was attempted for a task-owned temporary runtime.",
        status: "not_needed | succeeded | failed | unknown | not_safe_to_cleanup",
        targets: [{
          kind: "process | port | container | dev_server | other",
          pid: "number or null when known",
          port: "number or null when known",
          command: "string or null when known",
          summary: "Short target summary.",
        }],
        summary: "Short cleanup result. Cleanup failed/unknown/not_safe_to_cleanup should make TaskResult completed_with_notes with notes, not failed or blocked by itself.",
      },
      runtimeClosureRules: task.taskKind === "runtime_delivery_closure" ? [
        "checkedFields must cover every task.runtimeDeliveryRequirement.affectedContractFields entry.",
        "codeLevelChecks must include one result for every task.runtimeDeliveryRequirement.requiredCodeLevelChecks[].checkId.",
        "Evidence must be code-level: cite scripts, entry files, route/static handlers, config, build output references, local warm command output, or explicit unverifiedItems.",
        "Do not claim full deploy, container, clean install, registry, or browser proof unless actually performed.",
        "Do not execute start.command as a foreground long-running process. If a live probe is useful, use a bounded background runtime/probe/cleanup sequence; otherwise record static/code-level evidence and unverifiedItems.",
      ] : undefined,
    } : undefined,
    notes: [],
    blockedReasons: [],
    createdAt: new Date(0).toISOString(),
    updatedAt: new Date(0).toISOString(),
  };
}

function taskResultResultRules(): string[] {
  return [
    "verificationResults must use exactly the verificationId values from task.verificationIntents/outputContract.schemaShape.allowedVerificationResults.",
    "verificationResults[].evidenceType must use task.verificationIntents[].acceptableEvidence / outputContract.schemaShape.verificationResults[].allowedEvidenceTypes. Do not use conceptEvidenceType values for verificationResults unless they are explicitly listed as acceptableEvidence.",
    "Do not invent verificationResults for individual commands such as build, test, lint, or runtime probe. Put command-level evidence in verificationResults[].summary and runtimeDeliveryEvidence.commandsRun.",
    "verificationResults[].status records verification outcome; selfRepairSummary.stopReason records only why a self-repair loop stopped.",
    "When no self-repair loop was attempted, selfRepairSummary must be exactly { attempted:false, attemptCount:0, stopReason:'not_attempted', progressObserved:false }, even if verificationResults[].status is passed.",
    "Use selfRepairSummary.stopReason verification_passed only when attempted=true, attemptCount is 1..8, and verification passed after self-repair.",
    "Never combine attempted=false with stopReason verification_passed.",
  ];
}

function buildTechnicalBaselineSummary(baseline: Awaited<ReturnType<typeof loadRequiredTechnicalBaseline>>): Record<string, unknown> {
  return {
    technicalBaselineId: baseline.technicalBaselineId,
    projectKind: baseline.projectKind,
    scope: baseline.scope,
    stack: baseline.stack,
    constraints: baseline.constraints,
    mustFollow: true,
  };
}

function buildArchitectureProjection(aac: ArchitectureArtifactContract, task: Task): Record<string, unknown> {
  const entityRefs = new Set(task.writeBoundary.artifactRefs.entities);
  const moduleRefs = new Set(task.writeBoundary.artifactRefs.modules);
  const interfaceRefs = new Set(task.writeBoundary.artifactRefs.interfaces);
  const userFlowRefs = new Set(task.writeBoundary.artifactRefs.userFlows);
  const stateMachineRefs = new Set(task.writeBoundary.artifactRefs.stateMachines);

  for (const entity of aac.dataModel.entities) {
    if (entityRefs.has(entity.entityId)) {
      entity.moduleRefs.forEach((ref) => moduleRefs.add(ref));
    }
  }
  for (const contract of aac.interfaces) {
    if (interfaceRefs.has(contract.interfaceId)) {
      contract.moduleRefs.forEach((ref) => moduleRefs.add(ref));
      contract.entityRefs.forEach((ref) => entityRefs.add(ref));
    }
  }
  for (const flow of aac.userFlows) {
    if (userFlowRefs.has(flow.flowId)) {
      flow.moduleRefs.forEach((ref) => moduleRefs.add(ref));
      flow.interfaceRefs.forEach((ref) => interfaceRefs.add(ref));
      flow.entityRefs.forEach((ref) => entityRefs.add(ref));
      for (const step of flow.steps) {
        step.interfaceRefs.forEach((ref) => interfaceRefs.add(ref));
        step.stateMachineRefs.forEach((ref) => stateMachineRefs.add(ref));
      }
    }
  }
  for (const machine of aac.stateMachines) {
    if (stateMachineRefs.has(machine.stateMachineId)) {
      machine.moduleRefs.forEach((ref) => moduleRefs.add(ref));
      machine.entityRefs.forEach((ref) => entityRefs.add(ref));
    }
  }
  for (const contract of aac.interfaces) {
    if (interfaceRefs.has(contract.interfaceId)) {
      contract.moduleRefs.forEach((ref) => moduleRefs.add(ref));
      contract.entityRefs.forEach((ref) => entityRefs.add(ref));
    }
  }
  for (const entity of aac.dataModel.entities) {
    if (entityRefs.has(entity.entityId)) {
      entity.moduleRefs.forEach((ref) => moduleRefs.add(ref));
    }
  }

  const acceptanceRefs = new Set(task.acceptanceRefs);
  const taskRelevantGlobalConstraints = aac.dataModel.constraints.filter((constraint) =>
    constraint.entityRefs.some((ref) => entityRefs.has(ref)) ||
    constraint.acceptanceRefs.some((ref) => acceptanceRefs.has(ref))
  );
  const taskRelevantRelationships = aac.dataModel.relationships.filter((relationship) =>
    entityRefs.has(relationship.fromEntityRef) ||
    entityRefs.has(relationship.toEntityRef) ||
    relationship.acceptanceRefs.some((ref) => acceptanceRefs.has(ref))
  );
  const relevantFrontendOperationPaths = (aac.frontendExperience?.operationPaths ?? []).filter((operationPath) =>
    (operationPath.workflowRef ? userFlowRefs.has(operationPath.workflowRef) : false) ||
    (operationPath.surfaceRef ? (aac.frontendExperience?.surfaces ?? []).some((surface) =>
      surface.surfaceId === operationPath.surfaceRef &&
      (surface.moduleRefs.some((ref) => moduleRefs.has(ref)) || surface.workflowRefs.some((ref) => userFlowRefs.has(ref)))
    ) : false)
  );
  const relevantDataViewRefs = new Set(relevantFrontendOperationPaths.flatMap((operationPath) => operationPath.dataViewRefs));
  const relevantActionRefs = new Set(relevantFrontendOperationPaths.flatMap((operationPath) => operationPath.actionRefs));

  return {
    profile: "compact_task_execution_context",
    readFullArchitectureArtifactRefWhenNeeded: true,
    projectionCompleteness: {
      includesTaskRelevantDescriptions: true,
      includedDetailKinds: [
        "entity fields and constraints",
        "global data constraints",
        "relationships",
        "interface request/response/error schemas",
        "user flow steps and outcomes",
        "state machine transitions guards effects and rules",
        "frontend operation paths data views actions and feedback",
        "task-relevant object operations, field sets, blocking reasons, state changes, and visible feedback when AAC preserved them",
      ],
      fallbackRule: "If this projection still lacks a detail needed for the task, read sourceRefs.architectureArtifactContractRef with targeted selectors; do not guess missing requirement rules.",
    },
    engineeringBoundary: {
      projectKind: aac.engineeringBoundary.projectKind,
      strategy: aac.engineeringBoundary.strategy,
      applications: aac.engineeringBoundary.applications,
      creationPolicy: aac.engineeringBoundary.creationPolicy,
      modulePaths: aac.engineeringBoundary.modules
        .filter((item) => moduleRefs.has(item.moduleId))
        .map((item) => ({
          moduleId: item.moduleId,
          paths: item.paths,
          layerMappings: item.layerMappings,
        })),
    },
    modules: aac.modules
      .filter((item) => moduleRefs.has(item.moduleId))
      .map((item) => ({
        moduleId: item.moduleId,
        name: item.name,
        responsibility: item.responsibility,
      })),
    artifactRefs: {
      modules: [...moduleRefs],
      entities: [...entityRefs],
      interfaces: [...interfaceRefs],
      userFlows: [...userFlowRefs],
      stateMachines: [...stateMachineRefs],
      decisions: task.writeBoundary.artifactRefs.decisions,
      risks: task.writeBoundary.artifactRefs.risks,
    },
    entitySummaries: aac.dataModel.entities
      .filter((item) => entityRefs.has(item.entityId))
      .map((item) => ({
        entityId: item.entityId,
        name: item.name,
        type: item.type,
        implementationIntent: item.implementationIntent,
        fieldNames: item.fields.map((field) => field.name),
        constraintIds: item.constraints.map((constraint) => constraint.constraintId),
      })),
    entityDetails: aac.dataModel.entities
      .filter((item) => entityRefs.has(item.entityId))
      .map((item) => ({
        entityId: item.entityId,
        name: item.name,
        type: item.type,
        implementationIntent: item.implementationIntent,
        moduleRefs: item.moduleRefs,
        scopeRefs: item.scopeRefs,
        acceptanceRefs: item.acceptanceRefs,
        fields: item.fields,
        constraints: item.constraints,
      })),
    dataConstraints: taskRelevantGlobalConstraints,
    relationships: taskRelevantRelationships,
    interfaceSummaries: aac.interfaces
      .filter((item) => interfaceRefs.has(item.interfaceId))
      .map((item) => ({
        interfaceId: item.interfaceId,
        name: item.name,
        type: item.type,
        role: item.role ?? null,
        method: item.method,
        path: item.path,
      })),
    interfaceContracts: aac.interfaces
      .filter((item) => interfaceRefs.has(item.interfaceId))
      .map((item) => ({
        interfaceId: item.interfaceId,
        name: item.name,
        type: item.type,
        role: item.role ?? null,
        moduleRefs: item.moduleRefs,
        entityRefs: item.entityRefs,
        scopeRefs: item.scopeRefs,
        acceptanceRefs: item.acceptanceRefs,
        method: item.method ?? null,
        path: item.path ?? null,
        requestSchema: item.requestSchema ?? [],
        responseSchema: item.responseSchema ?? [],
        errorSchema: item.errorSchema ?? [],
      })),
    userFlowSummaries: aac.userFlows
      .filter((item) => userFlowRefs.has(item.flowId))
      .map((item) => ({
        flowId: item.flowId,
        name: item.name,
        kind: item.kind,
        entry: item.entry,
      })),
    userFlowDetails: aac.userFlows
      .filter((item) => userFlowRefs.has(item.flowId))
      .map((item) => ({
        flowId: item.flowId,
        name: item.name,
        kind: item.kind,
        moduleRefs: item.moduleRefs,
        interfaceRefs: item.interfaceRefs,
        entityRefs: item.entityRefs,
        scopeRefs: item.scopeRefs,
        acceptanceRefs: item.acceptanceRefs,
        entry: item.entry,
        steps: item.steps,
        outcomes: item.outcomes,
      })),
    frontendOperationPathDetails: {
      dataViews: (aac.frontendExperience?.dataViews ?? []).filter((dataView) => relevantDataViewRefs.has(dataView.viewId)),
      actions: (aac.frontendExperience?.actions ?? []).filter((action) => relevantActionRefs.has(action.actionId)),
      operationPaths: relevantFrontendOperationPaths,
      fallbackRule: "If this task owns UI behavior and this projection is empty or incomplete, read sourceRefs.architectureArtifactContractRef#/frontendExperience with targeted selectors before deciding the operation path is not applicable.",
    },
    stateMachineSummaries: aac.stateMachines
      .filter((item) => stateMachineRefs.has(item.stateMachineId))
      .map((item) => ({
        stateMachineId: item.stateMachineId,
        name: item.name,
        entityRefs: item.entityRefs,
        states: item.states.map((state) => state.stateId),
      })),
    stateMachineDetails: aac.stateMachines
      .filter((item) => stateMachineRefs.has(item.stateMachineId))
      .map((item) => ({
        stateMachineId: item.stateMachineId,
        name: item.name,
        entityRef: item.entityRef,
        entityRefs: item.entityRefs,
        moduleRefs: item.moduleRefs,
        scopeRefs: item.scopeRefs,
        acceptanceRefs: item.acceptanceRefs,
        states: item.states,
        initialState: item.initialState,
        events: item.events,
        transitions: item.transitions,
        rules: item.rules,
      })),
  };
}

function findReadyTaskState(run: TaskPlanRun): TaskPlanRun["taskStates"][number] | undefined {
  const readyGroup = run.groupStates.find((group) =>
    ["pending", "running"].includes(group.status) &&
    group.dependsOn.every((dep) => {
      const depState = run.groupStates.find((candidate) => candidate.groupId === dep);
      return depState?.status === "completed" || depState?.status === "completed_with_notes";
    }),
  );
  if (!readyGroup) {
    return undefined;
  }
  return run.taskStates.find((state) =>
    state.groupId === readyGroup.groupId &&
    state.status === "pending" &&
    state.dependsOn.every((dep) => {
      const depState = run.taskStates.find((candidate) => candidate.taskId === dep);
      return depState?.status === "completed" || depState?.status === "completed_with_notes";
    }),
  );
}

function updateGroupStateAfterTaskResult(run: TaskPlanRun, groupId: string | undefined): void {
  if (!groupId) {
    return;
  }
  const groupState = run.groupStates.find((group) => group.groupId === groupId);
  if (!groupState) {
    return;
  }
  const taskStates = run.taskStates.filter((state) => state.groupId === groupId);
  const now = new Date().toISOString();
  if (taskStates.some((state) => state.status === "failed")) {
    groupState.status = "failed";
    groupState.finishedAt = now;
    return;
  }
  if (taskStates.some((state) => state.status === "blocked")) {
    groupState.status = "blocked";
    groupState.finishedAt = now;
    return;
  }
  if (taskStates.some((state) => state.status === "running")) {
    groupState.status = "running";
    groupState.startedAt = groupState.startedAt ?? now;
    return;
  }
  if (taskStates.length > 0 && taskStates.every((state) => state.status === "completed" || state.status === "completed_with_notes")) {
    groupState.status = taskStates.some((state) => state.status === "completed_with_notes") ? "completed_with_notes" : "completed";
    groupState.finishedAt = now;
  }
}

function updateRunSummaryAndNextAction(run: TaskPlanRun, recomputeNextAction = true): void {
  run.summary = {
    total: run.taskStates.length,
    completed: run.taskStates.filter((state) => state.status === "completed").length,
    completedWithNotes: run.taskStates.filter((state) => state.status === "completed_with_notes").length,
    blocked: run.taskStates.filter((state) => state.status === "blocked").length,
    failed: run.taskStates.filter((state) => state.status === "failed").length,
    pending: run.taskStates.filter((state) => state.status === "pending").length,
    running: run.taskStates.filter((state) => state.status === "running").length,
  };
  if (!recomputeNextAction) {
    return;
  }
  const now = new Date().toISOString();
  if (run.summary.failed > 0) {
    const failedTask = run.taskStates.find((state) => state.status === "failed");
    const attempts = failedTask?.attempts.filter((attempt) => attempt.status === "failed").length ?? 0;
    run.status = "failed";
    run.scheduler.finishedAt = now;
    run.nextAction = attempts > 3
      ? { type: "review", reason: "EXECUTION_REPAIR_ATTEMPTS_EXHAUSTED", sourceTaskId: failedTask?.taskId, targetNode: "review" }
      : { type: "execution_repair", reason: "TASK_EXECUTION_FAILED", sourceTaskId: failedTask?.taskId, targetNode: "execution_repair" };
    return;
  }
  if (run.summary.blocked > 0) {
    const blockedTask = run.taskStates.find((state) => state.status === "blocked");
    run.status = "blocked";
    run.scheduler.finishedAt = now;
    run.nextAction = { type: "architecture_artifact_repair", reason: "TASK_BLOCKED", sourceTaskId: blockedTask?.taskId, targetNode: "architecture_artifact_repair" };
    return;
  }
  if (run.summary.running > 0) {
    run.status = "running";
    const runningTask = run.taskStates.find((state) => state.status === "running");
    run.nextAction = {
      type: "continue_execution",
      reason: "RUNNING_TASK_RECOVERABLE",
      sourceTaskId: runningTask?.taskId,
      targetNode: "task_execution",
    };
    return;
  }
  if (run.summary.pending === 0) {
    run.status = run.summary.completedWithNotes > 0 ? "completed_with_notes" : "completed";
    run.scheduler.finishedAt = now;
    run.nextAction = { type: "review", reason: "TASKPLAN_RUN_COMPLETED", targetNode: "review" };
    return;
  }
  run.status = run.scheduler.startedAt ? "running" : "not_started";
  run.nextAction = { type: "continue_execution", reason: "READY_TASK_AVAILABLE", targetNode: "task_execution" };
}

async function saveTaskPlanRun(projectRoot: string, run: TaskPlanRun, locator: DeliveryPhaseLocator): Promise<void> {
  run.updatedAt = new Date().toISOString();
  await writeJsonAtomic(taskPlanRunPath(projectRoot, run.runId, locator), taskPlanRunSchema.parse(run));
}

function nextActionFromBlockedResult(result: TaskResult, taskId: string): TaskPlanRun["nextAction"] {
  const reason = result.blockedReasons[0];
  if (!reason) {
    return {
      type: "taskplan_repair",
      reason: "TASK_BLOCKED",
      sourceTaskId: taskId,
      targetNode: "taskplan_repair",
    };
  }
  const typeByNode = {
    architecture_artifact_repair: "architecture_artifact_repair",
    taskplan_repair: "taskplan_repair",
    wait_dependency: "wait_dependency",
  } as const;
  return {
    type: typeByNode[reason.nextNode],
    reason: reason.code,
    sourceTaskId: taskId,
    targetNode: reason.nextNode,
  };
}

function allowedRefsFrom(
  pgc: Awaited<ReturnType<typeof loadPlanningContract>>,
  aac: ArchitectureArtifactContract,
): TaskPlanGenerationRequest["allowedRefs"] {
  return {
    scopeRefs: pgc.phaseScope.included.map((item) => item.scopeId),
    acceptanceRefs: pgc.phaseScope.acceptanceCandidates.map((item) => item.id),
    moduleRefs: aac.modules.map((item) => item.moduleId),
    entityRefs: aac.dataModel.entities.map((item) => item.entityId),
    interfaceRefs: aac.interfaces.map((item) => item.interfaceId),
    userFlowRefs: aac.userFlows.map((item) => item.flowId),
    stateMachineRefs: aac.stateMachines.map((item) => item.stateMachineId),
    decisionRefs: aac.risksAndDecisions.decisions.map((item) => item.decisionId),
    riskRefs: aac.risksAndDecisions.risks.map((item) => item.riskId),
  };
}

function taskPlanRequirementDetailProjection(
  pgc: PlanningGenerationContract,
  aac: ArchitectureArtifactContract,
): Record<string, unknown> {
  const acceptanceMatrixById = new Map(aac.acceptanceMatrix.map((entry) => [entry.acceptanceId, entry]));
  const detailCoverageById = new Map((aac.detailCoverage ?? []).map((entry) => [entry.detailId, entry]));
  const requirementDetailAssignment = pgc.requirementDetails ? {
    authority: "PGC.requirementDetails.items + AAC.detailCoverage",
    items: pgc.requirementDetails.items
      .filter((item) => item.requiredForCurrentPhase)
      .map((item) => {
        const coverage = detailCoverageById.get(item.detailId);
        return {
          detailId: item.detailId,
          kind: item.kind,
          title: item.title,
          summary: item.summary,
          priority: item.priority,
          impactTags: item.impactTags,
          lifecycleStage: item.lifecycleStage,
          quality: item.quality,
          scopeRefs: item.scopeRefs,
          acceptanceRefs: item.acceptanceRefs,
          conceptRefs: item.conceptRefs,
          frontendRefs: item.frontendRefs,
          coverageStatus: coverage?.coverageStatus ?? "uncovered",
          artifactRefs: coverage?.artifactRefs ?? null,
          coverageReason: coverage?.reason ?? null,
        };
      }),
    assignmentRule: "Every item with coverageStatus=covered must be assigned to at least one task.requirementDetailRefs entry using its detailId.",
    verificationRule: "Every assigned covered detail should be referenced by at least one verificationIntents[].requirementDetailRefs entry that proves the concrete behavior.",
    insufficientAacRule: "If a required detail has coverageStatus other than covered because AAC lacks a taskable artifact, write blocked output with blockedReasonCode AAC_INSUFFICIENT instead of inventing vague tasks.",
  } : null;
  const frontendOperationPathDetails = {
    dataViews: aac.frontendExperience?.dataViews ?? [],
    actions: aac.frontendExperience?.actions ?? [],
    operationPaths: aac.frontendExperience?.operationPaths ?? [],
    rule: "Use these AAC frontendExperience operation path details when assigning frontend tasks. They describe target discovery, selection, action entry, refresh/result observation, and user-visible feedback.",
  };
  return {
    authority: "planning_generation_contract_plus_architecture_artifact_contract",
    purpose: "Mechanically carry Brainstorm-confirmed current phase details and AAC taskable artifacts into TaskPlan generation.",
    requirementDetailAssignment,
    currentPhaseScope: {
      included: pgc.phaseScope.included.map((item) => ({
        scopeId: item.scopeId,
        label: item.label,
        items: item.items ?? [],
        source: item.source,
      })),
      deferred: pgc.phaseScope.deferred.map((item) => ({
        scopeId: item.scopeId,
        label: item.label,
        items: item.items ?? [],
        source: item.source,
      })),
      excluded: pgc.phaseScope.excluded.map((item) => ({
        scopeId: item.scopeId,
        label: item.label,
        items: item.items ?? [],
        source: item.source,
      })),
    },
    acceptanceDetails: pgc.phaseScope.acceptanceCandidates.map((item) => {
      const coverage = acceptanceMatrixById.get(item.id);
      return {
        id: item.id,
        statement: item.statement,
        priority: item.priority,
        capabilityRefs: item.capabilityRefs ?? [],
        sourceRefs: item.sourceRefs ?? [],
        aacCoverage: coverage ? {
          coverageStatus: coverage.coverageStatus,
          coverage: coverage.coverage,
          verificationHints: coverage.verificationHints,
          reason: coverage.reason ?? null,
          reasonCategory: coverage.reasonCategory ?? null,
        } : null,
      };
    }),
    businessFlowDetails: pgc.planningInputs.businessFlows,
    objectOperationDetailRules: {
      sourceFields: [
        "currentPhaseScope.included[].items",
        "acceptanceDetails[].statement",
        "businessFlowDetails[].summary",
        "architectureDetails.entities",
        "architectureDetails.userFlows",
        "architectureDetails.stateMachines",
        "architectureDetails.interfaces",
        "conceptRefs.phaseConceptGroundingRef",
      ],
      scopeCoverageRule: "Every currentPhaseScope.included item should remain traceable into TaskPlan tasks, verificationIntents, or an explicit blocked/unresolved reason. Do not plan only the most obvious subset of included scope.",
      taskAssignmentRule: "When Brainstorm/PGC/AAC carries key business objects, key field sets, object operations, operation inputs, preconditions, validation/blocking reasons, success state changes, or visible feedback, assign those details to concrete tasks and verificationIntents instead of reducing the work to generic service or UI labels.",
      conceptBindingRule: "If a task owns an operation invariant, state transition rule, blocking reason, or field meaning from phaseConceptGroundingRef, include conceptRefs, conceptResponsibilities, and conceptVerificationIntents for that task.",
      evidenceRule: "TaskResult must be able to show which fields, rules, state changes, blocking feedback, interface roles, or page operation paths were implemented or verified.",
    },
    architectureDetails: compactArchitectureDetailsForTaskPlan(aac, frontendOperationPathDetails),
    workflowClosureRequirements: buildWorkflowClosureRequirements(aac),
    conceptRefs: {
      deliveryConceptGlossaryRef: pgc.contextRefs?.deliveryConceptGlossaryRef ?? null,
      phaseConceptGroundingRef: pgc.contextRefs?.phaseConceptGroundingRef ?? null,
    },
    taskPlanningFieldMapping: {
      groupsObjective: "Summarize the engineering capability slice using currentPhaseScope items plus acceptanceDetails.",
      taskObjective: "Name the concrete business object, field set, operation, input, rule, flow, state, UI, API, blocking detail, or feedback detail the task owns.",
      taskRequirementDetailRefs: "When requirementDetailAssignment.items contains covered current-phase details owned by the task, list their detailId values in task.requirementDetailRefs.",
      implementationActions: "Choose enum actions that match the AAC artifact kind and concrete detail.",
      writeBoundaryArtifactRefs: "Reference AAC modules/entities/interfaces/userFlows/stateMachines/decisions/risks that carry the detail.",
      verificationIntentsBehavior: "Describe the specific acceptance rule, object operation, input, field, state, flow, blocking reason, operation path, readback, or feedback this task must verify.",
      verificationIntentRequirementDetailRefs: "When a verification intent proves a requirement detail, list that detailId in verificationIntents[].requirementDetailRefs.",
      frontendExperienceRequirement: "Use when frontendExperience is required or a task owns UI surfaces/workflows/states/bindings.",
      workflowClosureRequirement: "When workflowClosureRequirements exists, assign each closureId to at least one task whose artifact refs include the workflowRef and interfaceRefs, whose implementationActions include wire_reference_in_api_or_ui, and whose verificationIntents can prove the wired user action through automated_test or runtime_api_check evidence.",
      runtimeDeliveryRequirement: "Use when the task touches build/start/runtime entry/serving/configuration/generated artifacts/runtime surface.",
    },
    insufficientAacPolicy: {
      rule: "If a PGC detail is required for the current phase but AAC lacks any taskable artifact ref for it, write blocked output with blockedReasonCode AAC_INSUFFICIENT.",
      doNotUseFor: "Do not use AAC_INSUFFICIENT when AAC contains the detail but TaskPlan needs to assign it correctly; generate or repair the TaskPlan instead.",
    },
  };
}

function compactArchitectureDetailsForTaskPlan(
  aac: ArchitectureArtifactContract,
  frontendOperationPathDetails: Record<string, unknown>,
): Record<string, unknown> {
  return {
    compaction: {
      mode: "artifact_summary",
      rule: "This projection is intentionally compact. Use sourceRefs.architectureArtifactContractRef with targeted selectors when full AAC artifact detail is needed.",
    },
    modules: aac.modules.map((item) => ({
      moduleId: item.moduleId,
      name: item.name,
      responsibility: item.responsibility,
      dependsOn: item.dependsOn,
      scopeRefs: item.scopeRefs,
      acceptanceRefs: item.acceptanceRefs,
    })),
    entities: aac.dataModel.entities.map((entity) => ({
      entityId: entity.entityId,
      name: entity.name,
      type: entity.type,
      implementationIntent: entity.implementationIntent,
      moduleRefs: entity.moduleRefs,
      scopeRefs: entity.scopeRefs,
      acceptanceRefs: entity.acceptanceRefs,
      fields: entity.fields.map((field) => compactFieldShape(field)),
      constraints: entity.constraints.map((constraint) => ({
        constraintId: constraint.constraintId,
        type: constraint.type,
        description: constraint.description,
      })),
    })),
    interfaces: aac.interfaces.map((item) => ({
      interfaceId: item.interfaceId,
      name: item.name,
      type: item.type,
      role: item.role ?? null,
      moduleRefs: item.moduleRefs,
      entityRefs: item.entityRefs,
      scopeRefs: item.scopeRefs,
      acceptanceRefs: item.acceptanceRefs,
      method: item.method ?? null,
      path: item.path ?? null,
      requestSchema: (item.requestSchema ?? []).map((field) => compactFieldShape(field)),
      responseSchema: (item.responseSchema ?? []).map((field) => compactFieldShape(field)),
      errorSchema: (item.errorSchema ?? []).map((field) => compactFieldShape(field)),
    })),
    userFlows: aac.userFlows.map((flow) => ({
      flowId: flow.flowId,
      name: flow.name,
      kind: flow.kind,
      moduleRefs: flow.moduleRefs,
      interfaceRefs: flow.interfaceRefs,
      entityRefs: flow.entityRefs,
      scopeRefs: flow.scopeRefs,
      acceptanceRefs: flow.acceptanceRefs,
      entry: flow.entry,
      steps: flow.steps.map((step) => ({
        stepId: step.stepId,
        actor: step.actor ?? null,
        action: step.action,
        interfaceRefs: step.interfaceRefs,
        stateMachineRefs: step.stateMachineRefs,
      })),
      outcomes: flow.outcomes.map((outcome) => ({
        type: outcome.type,
        description: outcome.description,
        errorCode: outcome.errorCode ?? null,
      })),
    })),
    stateMachines: aac.stateMachines.map((machine) => ({
      stateMachineId: machine.stateMachineId,
      name: machine.name,
      entityRef: machine.entityRef,
      entityRefs: machine.entityRefs,
      moduleRefs: machine.moduleRefs,
      scopeRefs: machine.scopeRefs,
      acceptanceRefs: machine.acceptanceRefs,
      states: machine.states.map((state) => ({ stateId: state.stateId, name: state.name, terminal: state.terminal })),
      transitionRefs: machine.transitions.map((transition) => transition.transitionId),
      rules: machine.rules,
    })),
    frontendExperience: aac.frontendExperience ? {
      required: aac.frontendExperience.required,
      kind: aac.frontendExperience.kind,
      experienceLevel: aac.frontendExperience.experienceLevel,
      surfaceRefs: aac.frontendExperience.surfaces.map((surface) => surface.surfaceId),
      interactionStates: aac.frontendExperience.interactionStates,
      navigationRequired: aac.frontendExperience.navigation.required,
    } : null,
    frontendOperationPathDetails,
    runtimeDelivery: aac.runtimeDelivery ? runtimeDeliveryProjection(aac) : null,
  };
}

function compactFieldShape(field: NonNullable<ArchitectureArtifactContract["interfaces"][number]["requestSchema"]>[number]): Record<string, unknown> {
  return {
    fieldId: field.fieldId,
    name: field.name,
    type: field.type,
    semanticType: field.semanticType ?? null,
    required: field.required,
    enumValues: field.enumValues ?? [],
    itemFieldId: field.items?.fieldId ?? null,
    nestedFieldIds: field.fields?.map((nested) => nested.fieldId) ?? [],
  };
}

function runtimeDeliveryProjection(aac: ArchitectureArtifactContract): Record<string, unknown> | null {
  const runtime = aac.runtimeDelivery;
  if (!runtime) return null;
  return {
    status: runtime.status,
    runtimeKind: runtime.runtimeKind,
    buildCommand: runtime.build?.command ?? null,
    startCommand: runtime.start?.command ?? null,
    previewPath: runtime.httpProbes?.previewPath ?? null,
    healthPath: runtime.httpProbes?.healthPath ?? null,
    apiPaths: runtime.httpProbes?.apiPaths ?? [],
    frontendOutputDir: runtime.frontend?.outputDir ?? null,
    frontendServedBy: runtime.frontend?.servedBy ?? null,
    apiEntry: runtime.api?.entry ?? null,
  };
}

function runtimeDeliveryClosureRequirement(aac?: ArchitectureArtifactContract): Record<string, unknown> | null {
  return runtimeDeliveryClosureRequirementContract(aac);
}

function runtimeDeliveryClosureTaskTemplate(aac?: ArchitectureArtifactContract): Record<string, unknown> | null {
  const closureRequirement = runtimeDeliveryClosureRequirementContract(aac);
  if (!closureRequirement) {
    return null;
  }
  return {
    taskKind: "runtime_delivery_closure",
    copyRule: "Copy runtimeDeliveryRequirement exactly into the runtime_delivery_closure task. Do not derive or rename nested fields.",
    runtimeDeliveryRequirement: runtimeDeliveryClosureRequirementObject(closureRequirement),
  };
}

function runtimeDeliveryClosureRequirementObject(
  closureRequirement: RuntimeDeliveryClosureRequirementContract,
): Record<string, unknown> {
  return {
    appliesToThisTask: true,
    reason: "Final closure task required by outputContract.runtimeDeliveryClosureRequirement.",
    runtimeDeliveryRef: closureRequirement.runtimeDeliveryRef,
    affectedContractFields: closureRequirement.requiredContractFields,
    requiredCodeLevelChecks: closureRequirement.requiredCodeLevelChecks,
    evidenceExpectedInTaskResult: closureRequirement.requiredClosureTaskShape.runtimeDeliveryRequirement.evidenceExpectedInTaskResult,
    forbiddenActions: closureRequirement.requiredClosureTaskShape.runtimeDeliveryRequirement.forbiddenActions,
  };
}

function taskPlanGenerationInstruction(
  requestRef: string,
  request: TaskPlanGenerationRequest,
  options?: {
    recovery?: boolean;
    userMessage?: string;
  },
): Record<string, unknown> {
  return withAutoRunnableTransition({
    mode: "generate_candidate",
    ...artifactInstructionPolicy(),
    candidateKind: "TaskPlanGroupedOutputs",
    requestRef,
    outputSummary: {
      outlineFile: request.outputContract.outlineFile,
      groupFilePattern: request.outputContract.groupFilePattern,
    },
    blockedOutput: request.blockedOutput,
    submitCommand: request.submitCommand,
    recovery: options?.recovery ?? false,
    requestAlreadyExists: true,
    mustNotRunCommandsBeforeSubmit: [
      "task-plan request",
      "architecture request",
      "technical-baseline request",
      "repository-context request",
      "loom continue",
    ],
    generationSteps: [
      "Read requestRef.",
      compactContextReadStep,
      "This TaskPlanGenerationRequest already exists; do not summarize request creation or ask whether to continue before generating the grouped outputs.",
      "Use referencedArtifactReadGuide for sourceRefs before reading TechnicalBaseline, PGC, AAC, RepositoryContext, TaskPlan, or TaskPlanRun artifacts.",
      "Use sourceRefs.phaseConceptGroundingRef and fieldAccessHints.phaseConceptGroundingRef for current phase concept grounding; do not guess concept grounding paths from older phases or filenames.",
      "Use the request's outputContract schemaShape, enumRefs, allowedRefs, generationRules, and validatorRulesSummary.",
      "When outputContract.runtimeDeliveryClosureTaskTemplate exists, copy its runtimeDeliveryRequirement object exactly into the runtime_delivery_closure task.",
      "Use only the current request's outputContract.outlineFile and outputContract.groupFilePattern; ignore historical task-plan tmp/request files from other phases or request ids.",
      "Write outlineFile and every group candidate file required by the outline.",
      "The outline/group candidate files are the only required progress signal; do not run a heartbeat command.",
      "Run submitCommand only after grouped outputs exist.",
      "Follow the submit command response instruction or run loom continue after submit succeeds.",
    ],
    routingRule: "TaskPlanGenerationRequest has already been created. Generate grouped TaskPlan files now, submit them with submitCommand, do not run loom continue before submitCommand succeeds, and do not stop after request creation.",
    userMessage: options?.userMessage ?? "TaskPlanGenerationRequest created. Generate grouped TaskPlan candidate files now and submit them with the provided command.",
  }, {
    sourceCommand: "task-plan request",
    sourceSummary: "TaskPlanGenerationRequest was created.",
    primaryAction: "generate_taskplan_grouped_outputs_and_submit",
  });
}

function requireTaskPlanCandidateFile(candidateFile: string | undefined): string {
  if (!candidateFile?.trim()) {
    throw invalidArgument("task-plan accept requires --request-id or --candidate-file.");
  }
  return candidateFile;
}

async function assembleTaskPlanFromGroupedOutputs(
  root: string,
  locator: DeliveryPhaseLocator,
  requestId: string,
): Promise<{ value: TaskPlan | null; issues: TaskPlanAcceptResult["issues"] }> {
  const request = taskPlanGenerationRequestSchema.parse(await hydrateRequestManifest(root, taskPlanRequestPath(root, requestId, locator)));
  const outlineFile = typeof request.outputContract.outlineFile === "string"
    ? request.outputContract.outlineFile
    : toProjectRelative(root, taskPlanOutlineCandidatePath(root, locator, requestId));
  const outlineJson = await readGroupedCandidateJson(root, outlineFile, `/outline:${outlineFile}`);
  if (!outlineJson.value) {
    return { value: null, issues: outlineJson.issues };
  }
  const outlineParsed = parseGroupedCandidate(taskPlanOutlineSchema, outlineJson.value, `/outline:${outlineFile}`);
  if (!outlineParsed.value) {
    return { value: null, issues: outlineParsed.issues };
  }
  const outline = outlineParsed.value;
  if (outline.status !== "ready") {
    throw invalidArgument("TaskPlan outline is blocked.", { requestId, blockedReasons: outline.blockedReasons ?? [] });
  }
  const issues: TaskPlanAcceptResult["issues"] = [];
  if (outline.requestId !== requestId || outline.deliveryId !== locator.deliveryId || outline.phaseId !== locator.phaseId) {
    issues.push(groupedCandidateIssue(
      "SCHEMA_INVALID",
      `/outline:${outlineFile}/requestIdentity`,
      "TaskPlan outline requestId, deliveryId, or phaseId does not match the active request.",
      "Rewrite the outline file using requestId, deliveryId, and phaseId from the TaskPlanGenerationRequest.",
    ));
  }
  const groups: TaskPlanGroupCandidate[] = [];
  for (const group of outline.groups) {
    const groupFile = toProjectRelative(root, taskPlanGroupCandidatePath(root, locator, requestId, group.groupId));
    const groupJson = await readGroupedCandidateJson(root, groupFile, `/groups/${group.groupId}:${groupFile}`);
    if (!groupJson.value) {
      issues.push(...groupJson.issues);
      continue;
    }
    const candidateParsed = parseGroupedCandidate(taskPlanGroupCandidateSchema, groupJson.value, `/groups/${group.groupId}:${groupFile}`);
    if (!candidateParsed.value) {
      issues.push(...candidateParsed.issues);
      continue;
    }
    const candidate = candidateParsed.value;
    if (candidate.status !== "ready") {
      throw invalidArgument("TaskPlan group candidate is blocked.", { requestId, groupId: group.groupId, blockedReasons: candidate.blockedReasons ?? [] });
    }
    if (candidate.requestId !== requestId || candidate.deliveryId !== locator.deliveryId || candidate.phaseId !== locator.phaseId) {
      issues.push(groupedCandidateIssue(
        "SCHEMA_INVALID",
        `/groups/${group.groupId}/requestIdentity`,
        "TaskPlan group requestId, deliveryId, or phaseId does not match the active request.",
        "Rewrite the group file using requestId, deliveryId, and phaseId from the TaskPlanGenerationRequest.",
      ));
    }
    if (candidate.group.groupId !== group.groupId || JSON.stringify(candidate.group.taskIds) !== JSON.stringify(group.taskIds)) {
      issues.push(groupedCandidateIssue(
        "SCHEMA_INVALID",
        `/groups/${group.groupId}/group`,
        "TaskPlan group candidate does not match the outline group snapshot.",
        "Rewrite the group file so group.groupId and group.taskIds exactly match the outline group entry.",
      ));
      continue;
    }
    if (candidate.tasks.some((task) => task.groupId !== group.groupId)) {
      issues.push(groupedCandidateIssue(
        "SCHEMA_INVALID",
        `/groups/${group.groupId}/tasks/groupId`,
        "TaskPlan group candidate contains a task with mismatched groupId.",
        "Rewrite the group file so every task.groupId equals the containing group.groupId.",
      ));
      continue;
    }
    groups.push(candidate);
  }
  if (issues.length > 0) {
    return { value: null, issues };
  }
  const baseline = await loadRequiredTechnicalBaseline(root, locator);
  const pgc = await loadPlanningContract(root, undefined, locator);
  const aac = await loadArchitectureArtifact(root, undefined, locator);
  const tasks = groups.flatMap((group) => group.tasks);
  const now = new Date().toISOString();
  return {
    value: taskPlanSchema.parse({
    schemaVersion: "1.0",
    taskPlanId: outline.taskPlanId,
    version: 1,
    status: "ready",
    source: {
      roadmapId: pgc.source.roadmapId ?? null,
      phaseId: locator.phaseId,
      planningGenerationContractId: pgc.planningContractId,
      architectureArtifactContractId: aac.architectureArtifactContractId,
      technicalBaselineId: baseline.technicalBaselineId,
    },
    scopeSnapshot: {
      includedScopeRefs: pgc.phaseScope.included.map((item) => item.scopeId),
      excludedScopeRefs: pgc.phaseScope.excluded.map((item) => item.scopeId),
      deferredScopeRefs: pgc.phaseScope.deferred.map((item) => item.scopeId),
      acceptanceRefs: pgc.phaseScope.acceptanceCandidates.map((item) => item.id),
    },
    planningPolicy: {
      taskGranularity: "engineering_increment",
      groupGranularity: "engineering_capability",
      allowTaskSplitDuringRepair: true,
      allowTaskMergeDuringRepair: true,
    },
    groups: outline.groups,
    tasks,
    handoff: {
      readyForExecution: true,
      nextNode: "task_execution",
      blockedReasons: [],
    },
    createdAt: now,
    updatedAt: now,
    }),
    issues: [],
  };
}

async function readGroupedCandidateJson(
  root: string,
  candidateFile: string,
  pathPrefix: string,
): Promise<{ value: unknown | null; issues: TaskPlanAcceptResult["issues"] }> {
  const absolutePath = resolveCliPath(root, candidateFile);
  if (!(await pathExists(absolutePath))) {
    return {
      value: null,
      issues: [groupedCandidateIssue(
        "SCHEMA_INVALID",
        pathPrefix,
        "TaskPlan grouped output file is missing.",
        "Create the missing TaskPlan grouped output file at the path required by outputContract.",
      )],
    };
  }
  try {
    return { value: await readJsonFile(absolutePath), issues: [] };
  } catch {
    return {
      value: null,
      issues: [groupedCandidateIssue(
        "SCHEMA_INVALID",
        pathPrefix,
        "TaskPlan grouped output file is not valid readable JSON.",
        "Rewrite the TaskPlan grouped output file as valid JSON matching the schemaShape in the TaskPlanGenerationRequest.",
      )],
    };
  }
}

function parseGroupedCandidate<T>(
  schema: { parse(value: unknown): T },
  value: unknown,
  pathPrefix: string,
): { value: T | null; issues: TaskPlanAcceptResult["issues"] } {
  try {
    return { value: schema.parse(normalizeGroupedCandidateMechanicalFields(value)), issues: [] };
  } catch (error) {
    if (error instanceof ZodError) {
      return {
        value: null,
        issues: error.issues.map((zodIssue, index) => groupedCandidateIssue(
          "SCHEMA_INVALID",
          `${pathPrefix}/${zodIssue.path.map(String).join("/") || ""}`,
          zodIssue.message,
          "Repair the TaskPlan grouped output file to match the schemaShape in the TaskPlanGenerationRequest.",
          index,
        )),
      };
    }
    throw error;
  }
}

function normalizeGroupedCandidateMechanicalFields(value: unknown): unknown {
  if (!isRecord(value)) {
    return value;
  }
  const normalized: Record<string, unknown> = { ...value };
  if (!isIsoDateTimeString(normalized.createdAt)) {
    normalized.createdAt = new Date().toISOString();
  }
  return normalized;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isIsoDateTimeString(value: unknown): value is string {
  return typeof value === "string" && /^\d{4}-\d{2}-\d{2}T.+Z$/.test(value) && !Number.isNaN(Date.parse(value));
}

function groupedCandidateIssue(
  code: string,
  pointer: string,
  message: string,
  repairHint: string,
  index = 0,
): TaskPlanAcceptResult["issues"][number] {
  return {
    issueId: `issue-grouped-${createHash("sha1").update(`${pointer}:${message}:${index}`).digest("hex").slice(0, 8)}`,
    code,
    severity: "blocking",
    path: pointer,
    message,
    repairability: "agent_repairable",
    repairHint,
  };
}

function taskPlanEnumRefs(): Record<string, string[]> {
  return {
    outlineStatus: ["ready", "blocked"],
    groupStatus: ["ready", "blocked"],
    blockedReasonCode: [
      "AAC_INSUFFICIENT",
      "SOURCE_NOT_READY",
      "BASELINE_MISMATCH",
      "PGC_SCOPE_NOT_CONFIRMED",
      "USER_DECISION_REQUIRED",
      "TASKPLAN_CANDIDATE_INCOMPLETE",
    ],
    blockedNextNode: [
      "architecture_artifact_repair",
      "planning_contract_create",
      "needs_user_decision",
      "blocked",
    ],
    taskKind: [
      "feature_increment",
      "data_model_increment",
      "interface_increment",
      "ui_flow_increment",
      "frontend_experience",
      "runtime_delivery",
      "runtime_delivery_closure",
      "integration_increment",
      "verification_increment",
      "refactor_support",
      "configuration_support",
    ],
    implementationAction: [
      "create_or_update_entity",
      "create_or_update_persistence",
      "create_or_update_interface",
      "create_or_update_ui_flow",
      "create_or_update_state_machine",
      "create_or_update_business_rule",
      "add_reference_field",
      "validate_reference_format",
      "use_fixture_or_mock_data",
      "wire_reference_in_api_or_ui",
      "create_entity_crud",
      "create_entity_repository",
      "create_entity_admin_page",
      "create_entity_migration",
      "implement_entity_lifecycle",
      "add_or_update_tests",
      "add_or_update_config",
      "implement_frontend_experience_contract",
      "implement_runtime_delivery_contract",
      "refactor_supporting_code",
    ],
    verificationEvidence: [
      "automated_test",
      "manual_command_output",
      "runtime_api_check",
      "static_check",
      "agent_review_explanation",
    ],
    conceptEvidenceType: ["code", "test", "api", "ui", "runtime", "documentation"],
    aacVerificationHintKind: ["unit", "integration", "e2e", "manual", "static", "contract"],
  };
}

function taskPlanOutlineSchemaShape(
  locator: DeliveryPhaseLocator,
  requestId: string,
  pgc: Pick<PlanningGenerationContract, "phaseScope">,
  aac?: ArchitectureArtifactContract,
): Record<string, unknown> {
  const closureRequirement = runtimeDeliveryClosureRequirementContract(aac);
  const groups: Array<Record<string, unknown>> = [{
    groupId: "group-core",
    title: "Core capability",
    objective: "Deliver one engineering capability slice.",
    dependsOn: [],
    scopeRefs: pgc.phaseScope.included.map((item) => item.scopeId).slice(0, 1),
    acceptanceRefs: pgc.phaseScope.acceptanceCandidates.map((item) => item.id).slice(0, 2),
    taskIds: ["task-core-001"],
  }];
  if (closureRequirement) {
    groups.push({
      groupId: "group-runtime-delivery-closure",
      title: "Runtime delivery closure",
      objective: "Final code-level closure for RuntimeDeliveryContract.",
      dependsOn: ["group-core"],
      scopeRefs: pgc.phaseScope.included.map((item) => item.scopeId).slice(0, 1),
      acceptanceRefs: pgc.phaseScope.acceptanceCandidates.map((item) => item.id).slice(0, 2),
      taskIds: ["task-runtime-delivery-closure"],
      shapeAuthority: "outputContract.runtimeDeliveryClosureRequirement.requiredClosureGroupShape",
    });
  }
  return {
    schemaVersion: "1.0",
    requestId,
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    status: "ready",
    taskPlanId: `taskplan-${locator.phaseId}`,
    groups,
    runtimeDeliveryClosureOutlineRule: closureRequirement
      ? "Because runtimeDeliveryClosureRequirement.required=true, outline.groups must end with exactly one final closure group shaped like requiredClosureGroupShape."
      : "Omit runtime_delivery_closure group when runtimeDeliveryClosureRequirement is null.",
    createdAt: new Date(0).toISOString(),
  };
}

function taskPlanGroupSchemaShape(locator: DeliveryPhaseLocator, requestId: string, aac?: ArchitectureArtifactContract): Record<string, unknown> {
  const hasFrontend = aac?.frontendExperience?.required === true;
  const runtimeModified = aac?.runtimeDelivery?.status === "modified";
  const closureRequirement = runtimeDeliveryClosureRequirementContract(aac);
  const workflowClosureRequirement = aac ? buildWorkflowClosureRequirements(aac)[0] : undefined;
  return {
    schemaVersion: "1.0",
    requestId,
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    status: "ready",
    group: {
      groupId: "group-core",
      title: "Core capability",
      objective: "Deliver one engineering capability slice.",
      dependsOn: [],
      scopeRefs: ["scope-id"],
      acceptanceRefs: ["AC-001"],
      taskIds: ["task-core-001"],
    },
    tasks: [{
      taskId: "task-core-001",
      groupId: "group-core",
      title: "Implement core slice",
      taskKind: workflowClosureRequirement ? "ui_flow_increment" : "feature_increment",
      implementationActions: workflowClosureRequirement
        ? ["create_or_update_ui_flow", "wire_reference_in_api_or_ui", "add_or_update_tests"]
        : ["create_or_update_interface", "add_or_update_tests"],
      objective: workflowClosureRequirement
        ? "Implement the current workflow so the user action invokes the declared interface and renders success/blocking feedback."
        : "Implement a small executable increment.",
      dependsOn: [],
      scopeRefs: ["scope-id"],
      acceptanceRefs: workflowClosureRequirement?.acceptanceRefs ?? ["AC-001"],
      requirementDetailRefs: ["detail-id-from-contextProjection.requirementDetailTransfer.requirementDetailAssignment.items"],
      writeBoundary: {
        forbiddenPaths: [".loom"],
        artifactRefs: {
          modules: workflowClosureRequirement?.moduleRefs ?? [],
          entities: [],
          interfaces: workflowClosureRequirement?.interfaceRefs ?? [],
          userFlows: workflowClosureRequirement ? [workflowClosureRequirement.workflowRef] : [],
          stateMachines: workflowClosureRequirement?.stateMachineRefs ?? [],
          decisions: [],
          risks: [],
        },
      },
      verificationIntents: [{
        verificationId: "verify-core-001",
        acceptanceRefs: workflowClosureRequirement?.acceptanceRefs ?? ["AC-001"],
        requirementDetailRefs: ["detail-id-from-parent-task.requirementDetailRefs"],
        behavior: workflowClosureRequirement
          ? "Verify the user action invokes the declared interface and produces the expected state or feedback."
          : "Verify the implemented increment satisfies AC-001.",
        preferredEvidence: workflowClosureRequirement ? ["automated_test", "runtime_api_check"] : ["automated_test"],
        acceptableEvidence: workflowClosureRequirement ? ["automated_test", "runtime_api_check", "manual_command_output"] : ["automated_test", "manual_command_output", "static_check"],
      }],
      conceptRefs: ["concept-current-001"],
      conceptResponsibilities: [{
        conceptRef: "concept-current-001",
        responsibility: "Describe the task's responsibility for this concept, using refs from phaseConceptGroundingRef when applicable.",
      }],
      conceptVerificationIntents: [{
        conceptRef: "concept-current-001",
        evidenceType: "test",
        intent: "Explain what evidence should prove the concept was implemented without misunderstanding.",
      }],
      frontendExperienceRequirement: hasFrontend ? {
        frontendExperienceRef: "architectureArtifactContractRef#/frontendExperience",
        experienceLevel: aac?.frontendExperience?.experienceLevel ?? "usable_internal_product",
        mustSatisfy: [
          "surface fits frontendExperience.surfaces",
          "navigation follows frontendExperience.navigation when required",
          "operation path covers declared dataViews/actions/operationPaths when present",
          "interaction states include idle/loading/success/error where applicable",
          ...(workflowClosureRequirement ? ["workflow closure uses wired data binding, not static-only/demo-only feedback"] : []),
        ],
        ...(workflowClosureRequirement ? {
          executionGuidance: {
            closureRequirementRefs: workflowClosureRequirementExecutionView([workflowClosureRequirement]),
            workflowClosureDetailSource: workflowClosureExecutionDetailSource([workflowClosureRequirement]),
          },
        } : {}),
      } : undefined,
      runtimeDeliveryRequirement: runtimeModified ? {
        appliesToThisTask: true,
        reason: "This task may touch build, start, runtime entry, static serving, declared codegen, or runtime surfaces.",
        runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
        affectedContractFields: [
          "build.command",
          "start.command",
          "deliveryMechanics.staticAssets.output",
          "deliveryMechanics.staticAssets.servedBy",
          "deliveryMechanics.api.entry",
          "runtimeSurfaces",
        ],
        requiredCodeLevelChecks: [
          {
            checkId: "rd-check-build-script",
            contractField: "build.command",
            objective: "Confirm the declared build command still builds deliverable application parts touched by this task.",
            acceptableEvidence: ["static_check", "manual_command_output"],
          },
          {
            checkId: "rd-check-runtime-surface",
            contractField: "runtimeSurfaces",
            objective: "Confirm runtime entry, serving, and probe wiring remain aligned with RuntimeDeliveryContract.",
            acceptableEvidence: ["static_check", "runtime_api_check", "manual_command_output"],
          },
        ],
        evidenceExpectedInTaskResult: [
          "runtimeDeliveryEvidence.checkedFields includes affected fields changed or depended on by this task.",
          "runtimeDeliveryEvidence.codeLevelChecks reports each requiredCodeLevelChecks item.",
          "Do not claim clean/container deployability unless actually verified.",
        ],
        forbiddenActions: [
          "do_not_create_or_edit_deploy_generated_files",
          "do_not_use_sleep_placeholder_start",
          "do_not_claim_container_running_as_preview_verified",
          "do_not_require_clean_install_or_container_build_for_this_task",
        ],
      } : undefined,
    }, ...(closureRequirement ? [{
      taskId: "task-runtime-delivery-closure",
      groupId: "group-runtime-delivery-closure",
      title: "Close runtime delivery",
      taskKind: "runtime_delivery_closure",
      implementationActions: ["implement_runtime_delivery_contract", "add_or_update_tests"],
      objective: "Final code-level closure that proves the RuntimeDeliveryContract is internally consistent after all runtime-affecting groups.",
      dependsOn: [],
      scopeRefs: ["scope-id"],
      acceptanceRefs: ["AC-001"],
      requirementDetailRefs: ["detail-id-from-contextProjection.requirementDetailTransfer.requirementDetailAssignment.items"],
      writeBoundary: {
        forbiddenPaths: [".loom"],
        artifactRefs: {
          modules: [],
          entities: [],
          interfaces: [],
          userFlows: [],
          stateMachines: [],
          decisions: [],
          risks: [],
        },
      },
      verificationIntents: [{
        verificationId: "verify-runtime-delivery-closure",
        acceptanceRefs: ["AC-001"],
        requirementDetailRefs: ["detail-id-from-parent-task.requirementDetailRefs"],
        behavior: "Verify RuntimeDeliveryContract code-level closure without Docker, registry, clean install, browser proof, or full deploy.",
        preferredEvidence: ["static_check"],
        acceptableEvidence: ["static_check", "manual_command_output", "runtime_api_check"],
      }],
      runtimeDeliveryRequirement: runtimeDeliveryClosureRequirementObject(closureRequirement),
    }] : [])],
    createdAt: new Date(0).toISOString(),
  };
}

function buildTaskGenerationRules(aac?: ArchitectureArtifactContract): Record<string, unknown> {
  const workflowClosureRequirements = aac ? buildWorkflowClosureRequirements(aac) : [];
  return {
    taskGranularity: "engineering_increment",
    groupGranularity: "engineering_capability",
    requirementDetailTransferRules: {
      authorityField: "contextProjection.requirementDetailTransfer",
      rules: [
        "Use contextProjection.requirementDetailTransfer before writing the outline or any group file.",
        "Carry concrete current phase details from acceptanceDetails, currentPhaseScope.included[].items, businessFlowDetails, objectOperationDetailRules, and architectureDetails into TaskPlan groups and tasks.",
        "Task objective must identify the concrete business object, key field set, operation, operation input, rule, workflow, state, UI surface, operation path, API/interface, blocking reason, visible feedback, or runtime contract detail the task owns when such detail exists.",
        "For every requirementDetailAssignment.items entry with coverageStatus=covered, assign its detailId to at least one task.requirementDetailRefs and at least one verificationIntents[].requirementDetailRefs.",
        "verificationIntents[].requirementDetailRefs must be a subset of the parent task.requirementDetailRefs.",
        "verificationIntents[].behavior must describe the concrete behavior to verify, not only repeat an acceptance id or module label.",
        "writeBoundary.artifactRefs must point to the AAC artifacts that carry the task detail; do not leave artifactRefs empty for implementation tasks when matching AAC artifacts exist.",
        "When objectOperationDetailRules names scope coverage, operation inputs, field sets, blocking reasons, state changes, or feedback, assign them through task objectives, conceptResponsibilities, writeBoundary artifactRefs, and verificationIntents so TaskExecution can implement and prove them.",
        "When architectureDetails.frontendOperationPathDetails.operationPaths is non-empty, assign target discovery, action entry, refresh/result observation, and feedback responsibilities to frontend or integration tasks through frontendExperienceRequirement.",
        "When AAC interfaces include role=read_model, role=command, or role=readback for the same frontend workflow or operation path, TaskPlan must make producer implementation, frontend consumer wiring, and post-action readback/refresh verification responsibilities explicit in one coherent task or dependent task group.",
        "If required detail exists in PGC but no AAC artifact can carry it, write blocked output with blockedReasonCode AAC_INSUFFICIENT.",
      ],
    },
    groupedOutputRules: [
      "First write the outline file.",
      "Then write one group file per outline.groups[].groupId.",
      "Each task must include groupId equal to its group.groupId.",
      "Do not create cross-group task dependsOn; group dependencies belong in outline.groups[].dependsOn.",
    ],
    scopeAndReferenceRules: [
      "Use only refs from allowedRefs.",
      "Do not invent new scope, acceptance, or AAC artifact refs.",
      "If required current-phase design information is missing from AAC, return status blocked with blockedReasonCode AAC_INSUFFICIENT.",
    ],
    writeBoundaryRules: [
      "writeBoundary.forbiddenPaths must be exactly ['.loom'].",
      "Do not add .git, node_modules, dependency folders, caches, logs, dist, build, coverage, or tool output directories to forbiddenPaths.",
      "Package-manager installs and tool caches are allowed workspace side effects, but TaskResult.changedFiles must omit incidental side-effect paths.",
      "Do not use allowedPaths or expectedFiles.",
    ],
    verificationEvidenceRules: [
      "verificationIntents[].preferredEvidence and acceptableEvidence must use only enumRefs.verificationEvidence values.",
      "Do not copy AAC verificationHints[].kind directly into preferredEvidence or acceptableEvidence.",
      "Map AAC verificationHints kind unit, integration, e2e, and contract to automated_test when automated verification is intended.",
      "Map AAC verificationHints kind manual to manual_command_output.",
      "Map AAC verificationHints kind static to static_check.",
      "Use runtime_api_check only when the task can verify behavior through a running local API or command endpoint.",
      "Use agent_review_explanation only as a fallback acceptableEvidence, not as the only preferredEvidence for implementation tasks.",
    ],
    conceptGroundingRules: {
      phaseConceptGroundingRef: "sourceRefs.phaseConceptGroundingRef when present",
      rules: [
        "If a task is responsible for a current high-risk concept, include conceptRefs, conceptResponsibilities, and conceptVerificationIntents.",
        "If a task owns a business object field meaning, operation invariant, validation/blocking reason, state transition, or visible feedback from phaseConceptGroundingRef, bind the corresponding conceptRef and describe that concrete responsibility.",
        "Use conceptRefs only from phaseConceptGroundingRef; do not invent concept ids.",
        "Every conceptRefs item must have a matching conceptResponsibilities item.",
        "ConceptVerificationIntents evidenceType must use enumRefs.conceptEvidenceType.",
        "Do not bind deferred or excluded concepts to implementation tasks except explicit boundary/negative verification tasks.",
      ],
    },
    frontendExperienceRules: {
      required: aac?.frontendExperience?.required === true,
      contract: aac?.frontendExperience ?? null,
      rules: [
        "If frontendExperience.required=true, include frontend task coverage for the current phase UI surfaces.",
        "Do not treat frontend_experience as a visual preference only; it is a delivery contract for usable UI structure.",
        "When frontendExperience.dataViews/actions/operationPaths are present, TaskPlan must make those operation-path responsibilities visible in task objectives, verificationIntents, and frontendExperienceRequirement; do not reduce them to a generic UI shell task.",
        "If a task owns a frontend operation path with interfaces in scope, its verificationIntents should cover the user-visible read model, the submitted command, and the result readback/refresh or status observation when those interface roles are declared in AAC.",
        ...frontendImplementationOrganizationRules,
        "Do not block small API-only phases when frontendExperience.required=false.",
      ],
    },
    workflowClosureRules: {
      derivationAuthority: "AAC structure only. No acceptance text, phase title, or business keyword scanning is used.",
      requirementSource: workflowClosureRequirementSourceSummary(workflowClosureRequirements, {
        sourcePath: TASKPLAN_WORKFLOW_CLOSURE_REQUIREMENTS_SOURCE_PATH,
        phase: "taskplan_generation",
      }),
      appliesWhen: "requirementSource.requirementCount > 0",
      taskAssignmentRule: "Every closure requirement from requirementSource.sourcePath must be assigned to at least one TaskPlan task.",
      taskCoverageShape: {
        writeBoundaryArtifactRefs: {
          userFlows: "must include requirement.workflowRef",
          interfaces: "must include every requirement.interfaceRefs item",
        },
        acceptanceRefs: "must include every requirement.acceptanceRefs item",
        frontendExperienceRequirement: "required",
        implementationActions: "must include wire_reference_in_api_or_ui",
        verificationIntents: "at least one intent covering requirement.acceptanceRefs with acceptableEvidence automated_test or runtime_api_check",
        frontendExperienceRequirementExecutionGuidance: "should include closureRequirementRefs so TaskExecution can enforce wired evidence, including operationPathRefs/dataViewRefs/actionRefs when present. Do not embed the full workflow closure payload.",
      },
      resultExpectation: {
        requiredDataBindingModeForSatisfaction: "wired",
        staticOnlyStatus: "not_satisfied",
        knownGapStatus: "not_satisfied_when_required_closure",
      },
      repairRule: "If a closure requirement is unassigned, repair the TaskPlan assignment. Do not ask AAC repair to recreate the same already-declared userFlow/interface structure.",
      rules: [
        "Use contextProjection.requirementDetailTransfer.workflowClosureRequirements as the exact workflow closure requirement list.",
        "For each closure requirement, create or update tasks so the user action, declared interface invocation, state/persistence change, and success/blocking feedback are owned by executable tasks.",
        "When closure requirement interfaces include read_model/command/readback roles, assigned tasks must state which role each task implements or verifies; do not approve a static frontend consumer when the command/readback wiring is still a known gap.",
        "Do not satisfy a workflow closure requirement with a static or demo-only UI task.",
        "Do not split closure into unrelated backend-only and static frontend-only tasks without a task that owns the user-action-to-interface wiring and verification responsibility.",
        "The assigned task may be ui_flow_increment, frontend_experience, integration_increment, or feature_increment; taskKind is less important than the coverage shape above.",
        "This rule is structural and references only AAC ids. Do not infer closure requirements from natural-language keywords.",
      ],
    },
    runtimeDeliveryRules: {
      status: aac?.runtimeDelivery?.status ?? "not_applicable",
      contract: aac?.runtimeDelivery ?? null,
      closureRequirement: runtimeDeliveryClosureRequirement(aac),
      closureTaskTemplate: runtimeDeliveryClosureTaskTemplate(aac),
      taskPlanningGuidance: aac?.runtimeDelivery?.taskPlanningGuidance ?? null,
      rules: [
        "If runtimeDelivery.status=modified, TaskPlan must include exactly one runtime_delivery_closure task near the end of the DAG.",
        "Use generationRules.runtimeDeliveryRules.closureRequirement.requiredClosureGroupShape and requiredClosureTaskShape as a fill-in template, not as optional guidance.",
        "For the runtime_delivery_closure task, copy generationRules.runtimeDeliveryRules.closureTaskTemplate.runtimeDeliveryRequirement or outputContract.runtimeDeliveryClosureTaskTemplate.runtimeDeliveryRequirement exactly.",
        "The runtime_delivery_closure task belongs in a final closure group. The final closure group must be the last outline.groups entry and must depend on every group containing runtime-affecting tasks. Do not add cross-group task dependsOn; use outline.groups[].dependsOn for cross-group ordering.",
        "The runtime_delivery_closure task must have runtimeDeliveryRequirement.appliesToThisTask=true.",
        "Use outputContract.runtimeDeliveryClosureRequirement.requiredContractFields and requiredCodeLevelChecks as the exact field/check authority for runtime_delivery_closure. affectedContractFields must equal requiredContractFields. requiredCodeLevelChecks must equal requiredCodeLevelChecks, including exact checkId and contractField values.",
        "The runtime_delivery_closure task must not require Docker, clean install, registry access, browser proof, or full deploy.",
        "For each task, decide whether it affects RuntimeDeliveryContract using runtimeDelivery.taskPlanningGuidance, task objective, implementationActions, acceptanceRefs, and artifact refs.",
        "If a task affects runtime delivery, write runtimeDeliveryRequirement with appliesToThisTask=true, reason, runtimeDeliveryRef, affectedContractFields, requiredCodeLevelChecks, evidenceExpectedInTaskResult, and forbiddenActions.",
        "If a task does not affect runtime delivery, omit runtimeDeliveryRequirement or write appliesToThisTask=false with a reason.",
        "RuntimeDeliveryRequirement is code-level only; do not require clean install, container build, registry access, browser proof, or full deploy in TaskPlan.",
        "Do not create deploy files in runtime delivery tasks.",
      ],
    },
    taskKindEnum: [
      "feature_increment",
      "data_model_increment",
      "interface_increment",
      "ui_flow_increment",
      "frontend_experience",
      "runtime_delivery",
      "runtime_delivery_closure",
      "integration_increment",
      "verification_increment",
      "refactor_support",
      "configuration_support",
    ],
    implementationActionEnum: [
      "create_or_update_entity",
      "create_or_update_persistence",
      "create_or_update_interface",
      "create_or_update_ui_flow",
      "create_or_update_state_machine",
      "create_or_update_business_rule",
      "add_reference_field",
      "validate_reference_format",
      "use_fixture_or_mock_data",
      "wire_reference_in_api_or_ui",
      "create_entity_crud",
      "create_entity_repository",
      "create_entity_admin_page",
      "create_entity_migration",
      "implement_entity_lifecycle",
      "add_or_update_tests",
      "add_or_update_config",
      "implement_frontend_experience_contract",
      "implement_runtime_delivery_contract",
      "refactor_supporting_code",
    ],
    verificationEvidenceEnum: [
      "automated_test",
      "manual_command_output",
      "runtime_api_check",
      "static_check",
      "agent_review_explanation",
    ],
    aacVerificationHintToTaskPlanEvidence: {
      unit: "automated_test",
      integration: "automated_test",
      e2e: "automated_test",
      contract: "automated_test",
      manual: "manual_command_output",
      static: "static_check",
    },
  };
}

function buildTaskPlanRepairRequest(
  taskPlan: TaskPlan | null,
  issues: TaskPlanAcceptResult["issues"],
  context?: { requestId?: string; deliveryId: string; phaseId: string; architectureArtifact?: ArchitectureArtifactContract },
): Record<string, unknown> | null {
  if (issues.length === 0) return null;
  return {
    repairRequestId: createId("taskplan-repair"),
    reason: "task_graph_validator_failed",
    requestId: context?.requestId ?? null,
    deliveryId: context?.deliveryId,
    phaseId: context?.phaseId,
    candidateTaskPlan: taskPlan,
    validatorIssues: issues,
    protocolRef: "Step 6A TaskPlanCandidateRepairRequest",
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
    },
    repairInstructions: [
      "Repair TaskPlan grouped outputs, not a whole TaskPlan JSON file.",
      "For outline-level issues, rewrite outline and all group files.",
      "For group-level issues, rewrite the complete affected group file.",
      "Repair only TaskPlan contract mechanics.",
      "Do not change Brainstorm scope, TechnicalBaseline, PGC, or AAC.",
      "Do not invent new AAC artifacts.",
      "Do not add deferred scope to current Phase tasks.",
      "Use task split or merge only if needed to resolve validator issues.",
      "For runtime_delivery_closure repairs, use outputContract.runtimeDeliveryClosureRequirement.requiredClosureGroupShape and requiredClosureTaskShape as the exact replacement shape.",
      "For runtime_delivery_closure fields, affectedContractFields must exactly match outputContract.runtimeDeliveryClosureRequirement.requiredContractFields.",
      "For runtime_delivery_closure checks, requiredCodeLevelChecks must exactly match outputContract.runtimeDeliveryClosureRequirement.requiredCodeLevelChecks, including checkId and contractField.",
      "For runtime_delivery_closure ordering, the closure group must be the final outline group and must use outline.groups[].dependsOn between groups; do not add cross-group task dependsOn.",
    ],
    outputContract: {
      schema: "TaskPlanGroupedReplacement",
      outlineFile: context?.requestId ? `.loom/deliveries/${context.deliveryId}/tmp/${context.phaseId}/task-plan/${context.requestId}/outline.json` : null,
      groupFilePattern: context?.requestId ? `.loom/deliveries/${context.deliveryId}/tmp/${context.phaseId}/task-plan/${context.requestId}/groups/{groupId}.json` : null,
      outlineSchemaShape: context?.requestId
        ? taskPlanOutlineSchemaShape({ deliveryId: context.deliveryId, phaseId: context.phaseId }, context.requestId, {
            phaseScope: {
              phaseName: "Current phase",
              phaseGoal: "Current phase goal",
              included: [],
              deferred: [],
              excluded: [],
              acceptanceCandidates: [],
            },
          }, context.architectureArtifact)
        : null,
      groupSchemaShape: context?.requestId
        ? taskPlanGroupSchemaShape({ deliveryId: context.deliveryId, phaseId: context.phaseId }, context.requestId, context.architectureArtifact)
        : null,
      runtimeDeliveryClosureRequirement: runtimeDeliveryClosureRequirement(context?.architectureArtifact),
      enumRefs: taskPlanEnumRefs(),
      submitCommand: {
        name: "task-plan accept",
        argv: [
          "task-plan",
          "accept",
          "--delivery-id",
          context?.deliveryId ?? "{deliveryId}",
          "--phase-id",
          context?.phaseId ?? "{phaseId}",
          "--request-id",
          context?.requestId ?? "{requestId}",
          "--repair-id",
          "{repairRequestId}",
        ],
      },
      returnCompleteReplacement: false,
    },
  };
}

function buildTaskPlanRepairInstruction(
  taskPlan: TaskPlan | null,
  issues: TaskPlanAcceptResult["issues"],
  context?: { requestId?: string; deliveryId: string; phaseId: string; architectureArtifact?: ArchitectureArtifactContract },
): Record<string, unknown> | null {
  const repairRequest = buildTaskPlanRepairRequest(taskPlan, issues, context);
  if (!repairRequest) return null;
  return withAutoRunnableTransition({
    mode: "repair_candidate",
    routingRule: "Repair the existing TaskPlan grouped output files now, rerun task-plan accept with the same request-id, then follow the returned instruction. Do not run loom continue before the repaired accept succeeds.",
    ...artifactRepairPolicy(),
    schema: "TaskPlanGroupedOutputs",
    issues,
    repairRequest,
    repairSubmitRouting: {
      submitCommandReturnsInstruction: true,
      followReturnedInstructionImmediately: true,
      doNotRunContinueBeforeSuccessfulSubmit: true,
      doNotAskUserAfterSuccessfulSubmit: true,
      submitCommandName: "task-plan accept",
      rule: "Repair the TaskPlan grouped output files first, then rerun task-plan accept with the same request-id and repair-id if provided. Do not run loom continue or next-task until task-plan accept succeeds.",
    },
    instructions: [
      compactContextReadStep,
      "Repair only the TaskPlan grouped output contract files.",
      "Do not modify project source code.",
      "Do not modify Brainstorm, TechnicalBaseline, PGC, or AAC.",
      "For outline issues, rewrite outline and required group files.",
      "For group issues, rewrite complete affected group files.",
      "For runtime_delivery_closure issues, use repairRequest.outputContract.runtimeDeliveryClosureRequirement as the authority for the final closure group, closure task, affected fields, and check ids.",
      "Run task-plan accept again with the same request-id.",
      "After task-plan accept succeeds, follow data.instruction immediately.",
    ],
    submitCommand: {
      name: "task-plan accept",
      argv: [
      "task-plan",
      "accept",
        "--delivery-id",
        context?.deliveryId ?? "{deliveryId}",
        "--phase-id",
        context?.phaseId ?? "{phaseId}",
        "--request-id",
        context?.requestId ?? "{requestId}",
      ],
    },
  }, {
    sourceCommand: "task-plan accept",
    sourceSucceeded: false,
    sourceSummary: "TaskPlan accept returned repairable validation issues.",
    primaryAction: "repair_taskplan_candidate_and_submit",
  });
}

async function latestId(filePath: string, key: string): Promise<string | null> {
  if (!(await pathExists(filePath))) return null;
  const json = await readJsonFile(filePath);
  if (typeof json === "object" && json !== null && key in json) {
    const value = (json as Record<string, unknown>)[key];
    return typeof value === "string" ? value : null;
  }
  return null;
}

async function requireInitialized(projectRoot: string): Promise<void> {
  if (!(await pathExists(path.join(projectRoot, ".loom", "config.json")))) {
    throw stateNotInitialized(projectRoot);
  }
}

function resolveCliPath(projectRoot: string, filePath: string): string {
  return path.isAbsolute(filePath) ? filePath : path.join(projectRoot, filePath);
}

function createId(prefix: string): string {
  return `${prefix}-${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${createHash("sha1")
    .update(`${process.pid}:${Math.random()}:${Date.now()}`)
    .digest("hex")
    .slice(0, 8)}`;
}
