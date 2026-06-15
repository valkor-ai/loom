import { createHash } from "node:crypto";
import { execFile } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";
import { invalidArgument, stateNotInitialized } from "../errors";
import {
  type ManualReviewResolution,
  type ReviewRequest,
  type ReviewResult,
  type TaskPlan,
  type TaskResult,
  conceptEvidenceTypeSchema,
  manualReviewResolutionSchema,
  reviewDecisionSchema,
  reviewEvidenceKindSchema,
  reviewFailureClassSchema,
  reviewFindingCategorySchema,
  reviewFindingSeverityClassSchema,
  reviewNextActionTypeSchema,
  reviewRequestSchema,
  reviewResultSchema,
  taskResultSchema,
} from "../contracts";
import { pathExists, readJsonFile, writeJsonAtomic, writeTextAtomic } from "../state/fs";
import { getActiveLocator, getLocatorForBrainstormRun, loadDeliveryIndex, resolveLocator } from "../state/delivery";
import {
  type DeliveryPhaseLocator,
  brainstormContractPath,
  manualReviewRequestPath,
  reviewChangeContextPath,
  reviewPacketPath,
  reviewResultCandidatePath,
  reviewArtifactsDir,
  reviewLatestPath,
  reviewRequestPath,
  reviewResolutionPath,
  reviewResultPath,
  phaseConceptGroundingPath,
  taskResultPath,
  toProjectRelative,
} from "../state/paths";
import { REVIEW_CATEGORY_TO_ACTION, resetIssueCounter, validateReviewResult } from "../validators";
import {
  loadArchitectureArtifact,
  loadPlanningContract,
  loadRequiredTechnicalBaseline,
} from "./contracts";
import {
  loadCurrentTaskPlan,
  loadCurrentTaskPlanRun,
} from "./tasks";
import {
  closeOperationLease,
  createOperationLease,
  operationRef,
  readOperationLease,
  updateRouteState,
} from "./control";
import { repairSubmitRouting } from "./repair-routing";
import { instructionForRouteAction, withAutoRunnableTransition } from "./routing-instructions";
import { artifactGenerationProtocolPolicy, artifactInstructionPolicy, artifactRepairPolicy, compactContextReadStep } from "./output-policy";
import { agentActionContract } from "./agent-action";
import { referencedArtifactReadGuide } from "./artifact-read-guide";
import { hydrateRequestManifest, writeRequestManifestAtomic } from "./request-manifest";
import {
  buildWorkflowClosureRequirements,
  frontendSelfCheckViolatesRequiredClosure,
  taskCoversWorkflowClosure,
} from "../workflow-closure";

const execFileAsync = promisify(execFile);

export type CreateReviewRequestInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  taskPlanRunId?: string;
};

export type AcceptReviewResultInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  resultFile: string;
};

export type ResolveReviewInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  candidateFile: string;
};

export async function createReviewRequest(input: CreateReviewRequestInput): Promise<{
  status: "ready";
  request: ReviewRequest;
  requestPath: string;
  lease: ReturnType<typeof operationRef>;
  instruction: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const activeLease = await readOperationLease(root, locator.deliveryId);
  if (activeLease?.status === "active" && new Date(activeLease.expiresAt).getTime() > Date.now()) {
    if (activeLease.operationType === "review_generation" && activeLease.phaseId === locator.phaseId) {
      const requestRef = typeof activeLease.refs.requestRef === "string" ? activeLease.refs.requestRef : null;
      if (requestRef && await pathExists(path.join(root, requestRef))) {
        const request = reviewRequestSchema.parse(await hydrateRequestManifest(root, path.join(root, requestRef)));
        return {
          status: "ready",
          request,
          requestPath: requestRef,
          lease: operationRef(activeLease),
          instruction: reviewGenerationInstruction(requestRef, request, {
            recovery: true,
            userMessage: "ReviewRequest is already active. Generate the existing ReviewResult and submit it; do not create another review request.",
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
  const taskPlan = await loadCurrentTaskPlan(root, undefined, locator);
  const run = await loadCurrentTaskPlanRun(root, input.taskPlanRunId, locator);
  if (!["completed", "completed_with_notes", "failed", "blocked"].includes(run.status)) {
    throw invalidArgument("ReviewRequest requires a terminal TaskPlanRun.", { status: run.status });
  }
  const baseline = await loadRequiredTechnicalBaseline(root, locator);
  const pgc = await loadPlanningContract(root, taskPlan.source.planningGenerationContractId, locator);
  const aac = await loadArchitectureArtifact(root, taskPlan.source.architectureArtifactContractId, locator);
  const taskResults = await loadEffectiveTaskResults(root, locator, run);
  const reviewId = createId(`review-${taskPlan.source.phaseId}`);
  const nextPhasePreview = await loadNextPhasePreview(root, locator, pgc.source.brainstormRunId);
  const changeSet = await buildChangeSet(root, locator, reviewId, taskResults);
  const reviewPacketFile = reviewPacketPath(root, reviewId, locator);
  const changeContextFile = reviewChangeContextPath(root, reviewId, locator);
  const reviewPacket = buildReviewPacket(taskPlan, run, taskResults, aac);
  const changeContext = buildChangeContext(changeSet);
  const conceptReviewMatrix = await buildConceptReviewMatrix(root, locator, taskPlan, taskResults);
  const detailReviewMatrix = buildRequirementDetailReviewMatrix(taskPlan, taskResults);
  await writeJsonAtomic(reviewPacketFile, reviewPacket);
  await writeJsonAtomic(changeContextFile, changeContext);
  const resultFile = toProjectRelative(root, reviewResultCandidatePath(root, locator, reviewId));
  const reviewPacketRef = toProjectRelative(root, reviewPacketFile);
  const changeContextRef = toProjectRelative(root, changeContextFile);
  const submitCommand = {
    name: "review accept",
    argv: [
      "review",
      "accept",
      "--delivery-id",
      locator.deliveryId,
      "--phase-id",
      locator.phaseId,
      "--result-file",
      "{resultFile}",
    ],
  };
  const request: ReviewRequest = {
    schemaVersion: "1.0",
    requestId: reviewId,
    requestType: "review_gate",
    agentAction: agentActionContract({
      actionKind: "review_gate",
      instruction: "Review the phase run using reviewPacketRef and changeContextRef, write ReviewResult to outputContract.resultFile, then run submitCommand exactly.",
      read: {
        required: [
          "this request",
          "referencedArtifactReadGuide",
          "reviewPacketRef",
          "changeContextRef",
          "conceptReviewMatrix",
          "detailReviewMatrix",
          "reviewRules.commonRules",
          "reviewRules.changeSetRules",
          "enumRefs",
          "outputContract.schemaShape",
          "outputContract.allowedRefs",
          "outputContract.severityPolicy",
          "outputContract.routingRules",
          "outputContract.conceptReviewRules",
          "outputContract.requirementDetailReview",
          "outputContract.frontendExperienceReview",
          "outputContract.reviewSignals",
        ],
        optional: ["per-file diffRefs from changeContextRef", "task results referenced by reviewPacketRef"],
        displayPolicy: "compact",
      },
      write: {
        resultFile,
        rules: [
          "Use only outputContract.allowedRefs for taskRefs, groupRefs, acceptanceRefs, taskResult refs, readRefs, and evidenceRefs.",
          "changed_file refs may cite a current changed file path from changeContextRef or task results; equivalent forms such as ./path and changed_file:path are accepted.",
          "verification_evidence readRefs and verification_result evidenceRefs may cite verificationId, taskResultId:verificationId, or taskId:verificationId from current task results.",
          "Classify severity semantically using outputContract.severityPolicy; CLI validates structure only.",
          "Use outputContract.reviewSignals for mechanical facts such as missing workflow closure assignment or TaskResult self-check static/gap contradictions. Do not approve when a workflow closure signal says closureSatisfied=false.",
          "For concept-related findings, use findingType and conceptRef from conceptReviewMatrix; do not invent concept ids.",
          "Use detailReviewMatrix and outputContract.reviewSignals for requirement detail evidence. Do not approve when a requirement_detail_evidence signal says detailSatisfied=false.",
          "Do not modify project files during review.",
        ],
      },
      submit: {
        command: submitCommand,
        requiredArgs: ["--delivery-id", "--phase-id", "--result-file"],
        placeholders: { "{resultFile}": resultFile },
        runAfter: "resultFile exists and follows outputContract.schemaShape",
      },
      schema: {
        primary: "ReviewResult",
        shapeLocation: "outputContract.schemaShape",
        enumLocation: "enumRefs and outputContract.*Enum",
        allowedRefsLocation: "outputContract.allowedRefs",
      },
      stopConditions: ["unable to review reliably", "submitCommand returns non-repairable failure", "manual_review or needs_user_decision is the accepted next action"],
    }),
    source: {
      roadmapId: taskPlan.source.roadmapId,
      phaseId: taskPlan.source.phaseId,
      taskPlanId: taskPlan.taskPlanId,
      taskPlanRunId: run.runId,
      technicalBaselineId: baseline.technicalBaselineId,
      architectureArtifactContractId: aac.architectureArtifactContractId,
    },
    reviewScope: {
      type: "phase_run",
      groupIds: run.groupStates.map((state) => state.groupId),
      acceptanceRefs: taskPlan.scopeSnapshot.acceptanceRefs,
      nextPhaseId: nextPhasePreview.kind === "candidate" ? nextPhasePreview.suggestedPhaseId : null,
      nextPhasePreview,
    },
    sourceRefs: {
      technicalBaselineRef: "contracts/technical-baseline.json",
      planningGenerationContractRef: "contracts/planning/current/pgc.json",
      architectureArtifactRef: "artifacts/architecture/current/aac.json",
      taskPlanRef: "tasks/current/taskplan.json",
      taskPlanRunRef: "tasks/current/run.json",
    },
    reviewPacketRef,
    changeContextRef,
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      technicalBaselineRef: "contracts/technical-baseline.json",
      planningGenerationContractRef: "contracts/planning/current/pgc.json",
      architectureArtifactRef: "artifacts/architecture/current/aac.json",
      taskPlanRef: "tasks/current/taskplan.json",
      taskPlanRunRef: "tasks/current/run.json",
      reviewPacketRef,
      changeContextRef,
    }),
    generationProtocol: {
      readRequestBeforeActing: true,
      readReferencedFilesBeforeJudging: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      ifUnableToReviewReturnReviewLimitation: true,
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
    },
    conceptReviewMatrix,
    detailReviewMatrix,
    enumRefs: {
      decision: [...reviewDecisionSchema.options],
      findingSeverity: ["critical", "major", "minor", "note"],
      severityClass: [...reviewFindingSeverityClassSchema.options],
      evidenceKind: [...reviewEvidenceKindSchema.options],
      failureClass: [...reviewFailureClassSchema.options],
      findingCategory: [...reviewFindingCategorySchema.options],
      conceptFindingType: [
        "concept_missing",
        "concept_misunderstanding",
        "concept_evidence_missing",
        "deferred_concept_leakage",
        "surface_only_concept_implementation",
      ],
      conceptEvidenceType: [...conceptEvidenceTypeSchema.options],
      nextAction: [...reviewNextActionTypeSchema.options],
      readRefType: ["review_packet", "change_context", "diff_ref", "changed_file", "task_result", "verification_evidence"],
      evidenceRefType: ["task_result", "verification_result", "diff_ref", "changed_file", "manual_note"],
    },
    reviewRules: {
      commonRules: [
        "Read reviewPacketRef and changeContextRef before producing ReviewResult.",
        "Every finding must include non-empty readRefs.",
        "Review the current effective task results by default.",
        "Top-level nextAction is the first action to take, not a script that resolves every finding.",
        "Put other blocking action groups into pendingActions.",
        "After the top-level nextAction is handled, run Review again before executing pendingActions.",
        "Do not redefine scope.",
        "Do not redesign AAC.",
        "Do not modify code.",
        "Do not make blocking findings for subjective style preferences.",
        "Use severityPolicy to classify findings. CLI validates structure only; you own semantic classification.",
        "Do not convert every failed verification into execution_repair.",
        "Classify environment blockers separately from product defects.",
        "Do not add new acceptance.",
        "Do not change TechnicalBaseline.",
        "Report design gaps as blocked findings and route them instead of fixing them in review.",
        "If outputContract.reviewSignals contains frontend_workflow_closure with closureSatisfied=false and recommendedNextAction=execution_repair, write a blocking frontend_experience finding and route execution_repair.",
        "If outputContract.reviewSignals contains frontend_workflow_closure with recommendedNextAction=taskplan_repair, write a blocking task_artifact_mapping_issue or task_verification_mapping_issue finding and route taskplan_repair.",
        "If outputContract.reviewSignals contains requirement_detail_evidence with detailSatisfied=false, write a blocking evidence_insufficient or task_verification_mapping_issue finding and route execution_repair.",
      ],
      changeSetRules: changeSet.mode === "git_diff_ref"
        ? [
            "Diff content is not inlined.",
            "Read reviewPacketRef and changeContextRef first; use them to decide which per-file diffRefs are relevant.",
            "Prefer per-file diffRefs from changeContextRef. Do not read fullDiffRef unless per-file diffs are insufficient for a cross-file finding.",
            "Before making any code-level finding for a changed file, read that file's diffRef. Use fullDiffRef only for cross-file evidence.",
            "If a diffRef cannot be read, report a review limitation instead of guessing.",
            "Line-level findings are allowed only when based on a read diffRef or fullDiffRef.",
          ]
        : [
            "Only changed file paths are provided.",
            "Read changed files only when needed.",
            "Do not claim a finding was introduced by the current task.",
            "Only findings with taskRelevance=direct and scopeRelation=within_task_changed_files may impact decision.",
            "Use severityPolicy, failureClass, taskRelevance, and scopeRelation to decide whether a finding is blocking, manual review, or warning.",
            "All other findings must be non-blocking notes.",
          ],
    },
    outputContract: {
      format: "json",
      schema: "ReviewResult",
      resultFile,
      decisionEnum: ["approved", "approved_with_notes", "changes_requested", "blocked", "needs_user_decision"],
      findingSeverityEnum: ["critical", "major", "minor", "note"],
      recommendedNextActionEnum: [...reviewNextActionTypeSchema.options],
      nextActionEnum: [...reviewNextActionTypeSchema.options],
      severityClassEnum: [...reviewFindingSeverityClassSchema.options],
      evidenceKindEnum: [...reviewEvidenceKindSchema.options],
      failureClassEnum: [...reviewFailureClassSchema.options],
      findingCategoryEnum: [...reviewFindingCategorySchema.options],
      findingCategoryToAction: REVIEW_CATEGORY_TO_ACTION,
      severityPolicy: buildReviewSeverityPolicy(),
      frontendExperienceReview: buildFrontendExperienceReview(aac),
      runtimeDeliveryReview: buildRuntimeDeliveryReview(aac),
      conceptReviewMatrix,
      requirementDetailReview: {
        detailReviewMatrix,
        rules: [
          "Every detailReviewMatrix item with status other than satisfied requires a blocking finding before approval.",
          "Use taskRefs and evidenceRefs from detailReviewMatrix; do not invent detail ids.",
          "Route missing or not_satisfied task evidence to execution_repair unless the finding shows TaskPlan assignment itself is wrong.",
        ],
      },
      conceptReviewRules: [
        "Use conceptReviewMatrix as the review scope for concept coverage; do not re-extract concepts from scratch.",
        "Report concept_missing or concept_evidence_missing when a must concept lacks implementation evidence.",
        "Report concept_misunderstanding only when code/test/API/UI evidence clearly contradicts the concept.",
        "Use manual_review when concept semantics are uncertain rather than guessing.",
      ],
      reviewSignals: buildReviewSignals(taskPlan, run, taskResults, aac),
      changeContextMode: changeSet.mode,
      routingRules: {
        actionPriority: [
          "needs_user_decision",
          "manual_review",
          "architecture_artifact_repair",
          "taskplan_repair",
          "execution_repair",
          "continue_to_next_phase",
          "done",
        ],
        manualReviewPriorityRule: "manual_review only outranks automatic repair when caused by blocking review_limitation or environment_or_dependency that prevents reliable review.",
      },
      schemaShape: buildReviewResultSchemaShape(reviewId, taskPlan.source.phaseId, taskPlan.taskPlanId, run.runId),
      validatorRules: buildReviewResultValidatorRules(changeSet.mode),
      allowedRefs: {
        taskIds: run.taskStates.map((state) => state.taskId),
        groupIds: run.groupStates.map((state) => state.groupId),
        acceptanceRefs: taskPlan.scopeSnapshot.acceptanceRefs,
        taskResultIds: taskResults.map((result) => result.taskResultId),
        changedFilePaths: changeContext.changedFiles.map((file: { path: string }) => file.path),
        verificationEvidenceRefs: taskResults.flatMap((result) =>
          result.verificationResults.flatMap((verification) => [
            verification.verificationId,
            `${result.taskResultId}:${verification.verificationId}`,
            `${result.taskId}:${verification.verificationId}`,
          ]),
        ),
        readRefs: [
          reviewPacketRef,
          changeContextRef,
          ...(taskResults.map((result) => result.taskResultId)),
          ...changeContext.changedFiles.map((file: { diffRef?: string; path: string }) => file.diffRef ?? file.path),
          ...(typeof changeSet.fullDiffRef === "string" ? [changeSet.fullDiffRef] : []),
        ],
        readOrderGuidance: changeSet.mode === "git_diff_ref"
          ? [
              "reviewPacketRef",
              "changeContextRef",
              "only the per-file diffRefs needed for findings",
              "fullDiffRef only when a cross-file finding cannot be supported by per-file diffRefs",
            ]
          : [
              "reviewPacketRef",
              "changeContextRef",
              "only changed files needed for findings",
            ],
      },
      requiredFields: [
        "reviewId",
        "source",
        "decision",
        "findings",
        "coverageAssessment",
        "limitations",
        "pendingActions",
        "nextAction",
      ],
    },
    submitCommand,
    createdAt: new Date().toISOString(),
  };
  const parsed = reviewRequestSchema.parse(request);
  const requestFile = reviewRequestPath(root, reviewId, locator);
  const lease = await createOperationLease({
    projectRoot: root,
    locator,
    operationType: "review_generation",
    refs: {
      requestRef: toProjectRelative(root, requestFile),
      resultFile: parsed.outputContract.resultFile,
    },
  });
  try {
    await writeRequestManifestAtomic(root, requestFile, parsed);
    await writeJsonAtomic(reviewLatestPath(root, locator), {
      schemaVersion: "1.0",
      latestReviewId: reviewId,
      latestRequestRef: toProjectRelative(root, requestFile),
      effectiveDecision: null,
      effectiveNextAction: null,
      updatedAt: new Date().toISOString(),
    });
    await updateRouteState({
      projectRoot: root,
      locator,
      deliveryStatus: "reviewing",
      phaseStatus: "reviewing",
      nextAction: {
        type: "review",
        source: "review_request",
        deliveryId: locator.deliveryId,
        phaseId: locator.phaseId,
        ref: toProjectRelative(root, requestFile),
        reason: "REVIEW_REQUEST_CREATED",
      },
    });
  } catch (error) {
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "review_generation",
      reason: "request_write_failed",
    });
    throw error;
  }
  return {
    status: "ready",
    request: parsed,
    requestPath: toProjectRelative(root, requestFile),
    lease: operationRef(lease),
    instruction: reviewGenerationInstruction(toProjectRelative(root, requestFile), parsed),
  };
}

function reviewGenerationInstruction(
  requestRef: string,
  request: ReviewRequest,
  options?: {
    recovery?: boolean;
    userMessage?: string;
  },
): Record<string, unknown> {
  return withAutoRunnableTransition({
    mode: "generate_candidate",
    ...artifactInstructionPolicy(),
    candidateKind: "ReviewResult",
    requestRef,
    candidateFile: request.outputContract.resultFile,
    resultFile: request.outputContract.resultFile,
    agentAction: request.agentAction ?? null,
    blockedOutput: null,
    submitCommand: request.submitCommand,
    recovery: options?.recovery ?? false,
    requestAlreadyExists: options?.recovery === true,
    mustNotRunCommandsBeforeSubmit: ["review", "repair request", "loom continue"],
    generationSteps: [
      "Read requestRef.",
      compactContextReadStep,
      "Use referencedArtifactReadGuide for reviewPacketRef, changeContextRef, and sourceRefs; do not guess jq wrapper roots.",
      "Read reviewPacketRef and changeContextRef before judging.",
      "Use outputContract.schemaShape, allowedRefs, severityPolicy, validatorRules, and routingRules.",
      "Write the ReviewResult JSON to outputContract.resultFile.",
      "Run submitCommand after resultFile exists.",
      "Follow the submit command response instruction immediately when it is auto-runnable.",
    ],
    routingRule: "The ReviewRequest has already been created. Generate the ReviewResult now; do not run review again or loom continue before review accept succeeds.",
    userMessage: options?.userMessage ?? "ReviewRequest created. Generate and submit the ReviewResult now.",
  }, {
    sourceCommand: "review",
    sourceSummary: "ReviewRequest was created.",
    primaryAction: "generate_review_result_and_submit",
    mustStartImmediately: true,
  });
}

export async function acceptReviewResult(input: AcceptReviewResultInput): Promise<{
  accepted: boolean;
  status: ReviewResult["decision"] | "invalid_result";
  reviewId: string | null;
  issues: ReturnType<typeof validateReviewResult>["issues"];
  resultPath: string | null;
  fallbackResult: ReviewResult | null;
  instruction?: Record<string, unknown>;
  repairInstruction?: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  resetIssueCounter();
  const root = path.resolve(input.projectRoot);
  const locator = await resolveLocator(root, input.deliveryId, input.phaseId);
  const candidate = await readJsonFile(resolveCliPath(root, input.resultFile));
  const partial = typeof candidate === "object" && candidate !== null ? candidate as { source?: { requestId?: unknown }; reviewId?: unknown } : {};
  const requestId = typeof partial.source?.requestId === "string" ? partial.source.requestId : await latestReviewId(root, locator);
  if (!requestId) {
    throw invalidArgument("ReviewRequest does not exist. Run loom review first.");
  }
  const request = reviewRequestSchema.parse(await hydrateRequestManifest(root, reviewRequestPath(root, requestId, locator)));
  const validation = validateReviewResult(candidate, request);
  if (!validation.value || validation.issues.length > 0) {
    const attempts = await incrementInvalidReviewAttempts(root, locator, request.requestId);
    if (attempts < 3) {
      const repairInstruction = {
        mode: "repair_result_contract",
        ...artifactRepairPolicy(),
        schema: "ReviewResult",
        resultFile: toProjectRelative(root, resolveCliPath(root, input.resultFile)),
        issues: validation.issues,
        repairSubmitRouting: repairSubmitRouting({
          kind: "result",
          submitCommandName: "review accept",
        }),
        instructions: [
          compactContextReadStep,
          "Repair only the ReviewResult JSON contract fields.",
          "Do not modify project source code.",
          "Do not change TaskPlan, AAC, PGC, or TechnicalBaseline.",
          "Use the original ReviewRequest outputContract.schemaShape, validatorRules, allowedRefs, severityPolicy, and routingRules.",
          "Every repaired finding recommendedNextAction must be one of outputContract.recommendedNextActionEnum and structurally consistent with severityClass, failureClass, category, and routingRules.",
          "The top-level nextAction.type must be one of outputContract.nextActionEnum and follow the routing priority from outputContract.routingRules.",
          "Return a complete replacement ReviewResult to the same resultFile.",
          "Run review accept again with the same result-file.",
        ],
        submitCommand: {
          name: "review accept",
          argv: [
            "review",
            "accept",
            "--delivery-id",
            locator.deliveryId,
            "--phase-id",
            locator.phaseId,
            "--result-file",
            toProjectRelative(root, resolveCliPath(root, input.resultFile)),
          ],
        },
      };
      return {
        accepted: false,
        status: "invalid_result",
        reviewId: validation.value?.reviewId ?? request.requestId,
        issues: validation.issues,
        resultPath: null,
        fallbackResult: null,
        instruction: repairInstruction,
        repairInstruction,
      };
    }
    const fallback = buildFallbackReviewResult(request);
    await writeJsonAtomic(reviewResultPath(root, fallback.reviewId, locator), fallback);
    await materializeManualReviewRequest(root, locator, fallback);
    await updateReviewLatest(root, locator, fallback, null);
    await closeOperationLease({
      projectRoot: root,
      locator,
      operationType: "review_generation",
      reason: "review_result_invalid_fallback",
    });
    await syncRouteFromReview(root, locator, fallback.nextAction, "review_result");
    return {
      accepted: false,
      status: "invalid_result",
      reviewId: validation.value?.reviewId ?? fallback.reviewId,
      issues: validation.issues,
      resultPath: toProjectRelative(root, reviewResultPath(root, fallback.reviewId, locator)),
      fallbackResult: fallback,
    };
  }

  const now = new Date().toISOString();
  const result = reviewResultSchema.parse({
    ...validation.value,
    updatedAt: now,
  });
  const resultFile = reviewResultPath(root, result.reviewId, locator);
  await writeJsonAtomic(resultFile, result);
  if (result.nextAction.type === "manual_review") {
    await materializeManualReviewRequest(root, locator, result);
  }
  await updateReviewLatest(root, locator, result, null);
  await closeOperationLease({
    projectRoot: root,
    locator,
    operationType: "review_generation",
    reason: "review_result_accepted",
  });
  await syncRouteFromReview(root, locator, result.nextAction, "review_result");
  return {
    accepted: true,
    status: result.decision,
    reviewId: result.reviewId,
    issues: [],
    resultPath: toProjectRelative(root, resultFile),
    fallbackResult: null,
    instruction: instructionForRouteAction(routeActionFromReviewResult(result, locator), locator),
  };
}

export async function resolveReview(input: ResolveReviewInput): Promise<{
  resolved: true;
  resolution: ManualReviewResolution;
  resolutionPath: string;
  effectiveDecision: "approved_with_notes" | "changes_requested";
  effectiveNextAction: Record<string, unknown>;
  instruction?: Record<string, unknown>;
}> {
  await requireInitialized(input.projectRoot);
  const root = path.resolve(input.projectRoot);
  const candidate = await readJsonFile(resolveCliPath(root, input.candidateFile));
  const resolution = manualReviewResolutionSchema.parse(candidate);
  const locator = await resolveLocator(root, input.deliveryId ?? resolution.deliveryId, input.phaseId ?? resolution.phaseId);
  if (resolution.deliveryId !== locator.deliveryId || resolution.phaseId !== locator.phaseId) {
    throw invalidArgument("ManualReviewResolution deliveryId/phaseId does not match target locator.", {
      locator,
      resolution: { deliveryId: resolution.deliveryId, phaseId: resolution.phaseId },
    });
  }
  if (resolution.decision === "approve_override" && resolution.changeRequest !== null) {
    throw invalidArgument("approve_override ManualReviewResolution requires changeRequest=null.");
  }
  if (resolution.decision === "request_changes" && resolution.changeRequest === null) {
    throw invalidArgument("request_changes ManualReviewResolution requires changeRequest.");
  }
  const effectiveDecision = resolution.decision === "approve_override" ? "approved_with_notes" : "changes_requested";
  const effectiveNextAction = resolution.decision === "approve_override"
    ? resolution.nextAction
    : {
        type: resolution.changeRequest?.route,
        targetNode: resolution.nextAction.targetNode,
        reason: resolution.nextAction.reason,
        changeRequest: resolution.changeRequest,
      };
  const file = reviewResolutionPath(root, resolution.manualReviewResolutionId, locator);
  await writeJsonAtomic(file, resolution);
  await updateReviewLatest(root, locator, null, {
    latestResolutionRef: toProjectRelative(root, file),
    effectiveDecision,
    effectiveNextAction,
  });
  await syncRouteFromReview(root, locator, effectiveNextAction, "manual_review_resolution");
  const refreshedDelivery = await loadDeliveryIndex(root, locator.deliveryId);
  const refreshedPhase = refreshedDelivery.phases.find((phase) => phase.phaseId === locator.phaseId);
  const routeAction = refreshedPhase?.nextAction ?? routeActionFromRawReviewAction(effectiveNextAction, locator, "manual_review_resolution");
  return {
    resolved: true,
    resolution,
    resolutionPath: toProjectRelative(root, file),
    effectiveDecision,
    effectiveNextAction,
    instruction: instructionForRouteAction(routeAction, locator),
  };
}

async function buildChangeSet(projectRoot: string, locator: DeliveryPhaseLocator, reviewId: string, taskResults: TaskResult[]): Promise<Record<string, unknown> & { mode: string }> {
  const changedFiles = [...new Set(taskResults.flatMap((result) => result.changedFiles))];
  if (await isGitRepo(projectRoot)) {
    const artifactsDir = path.join(reviewArtifactsDir(projectRoot, reviewId, locator), "diffs");
    const fullDiffPath = path.join(artifactsDir, "full.diff");
    const statArgs = changedFiles.length > 0 ? ["diff", "HEAD", "--numstat", "--", ...changedFiles] : ["diff", "HEAD", "--numstat", "--", "__loom_no_changed_files__"];
    const stat = changedFiles.length > 0 ? await git(statArgs, projectRoot).catch(() => "") : "";
    const statFiles = new Map(stat.split("\n").filter(Boolean).map((line) => {
      const [insertionsRaw, deletionsRaw, ...fileParts] = line.split(/\s+/);
      const file = fileParts.join(" ");
      return [file, {
        path: file,
        changeType: "modified",
        insertions: Number(insertionsRaw) || 0,
        deletions: Number(deletionsRaw) || 0,
      }];
    }));
    const fileDiffs = new Map<string, string>();
    const files = changedFiles.map((file) => {
      const statFile = statFiles.get(file);
      const diffRef = path.join(artifactsDir, `${sanitizePath(file)}.diff`);
      return {
        ...statFile,
        path: file,
        changeType: statFile?.changeType ?? "declared_changed",
        insertions: statFile?.insertions ?? 0,
        deletions: statFile?.deletions ?? 0,
        diffRef: toProjectRelative(projectRoot, diffRef),
      };
    });
    for (const file of files) {
      const fileDiff = await gitDiffForDeclaredFile(projectRoot, file.path);
      fileDiffs.set(file.path, fileDiff);
      await writeTextAtomic(path.join(projectRoot, file.diffRef), fileDiff);
    }
    await writeTextAtomic(fullDiffPath, files.map((file) => fileDiffs.get(file.path) ?? "").filter(Boolean).join("\n"));
    return {
      mode: "git_diff_ref",
      gitAvailable: true,
      diffAvailable: true,
      diffInline: false,
      summary: {
        changedFileCount: changedFiles.length,
      },
      files,
      fullDiffRef: toProjectRelative(projectRoot, fullDiffPath),
    };
  }

  return {
    mode: "current_file_content",
    gitAvailable: false,
    diffAvailable: false,
    files: changedFiles.map((file) => ({
      path: file,
      changeType: "declared_changed",
    })),
  };
}

async function loadEffectiveTaskResults(projectRoot: string, locator: DeliveryPhaseLocator, run: Awaited<ReturnType<typeof loadCurrentTaskPlanRun>>): Promise<TaskResult[]> {
  const results: TaskResult[] = [];
  for (const state of run.taskStates) {
    if (!state.resultId) {
      continue;
    }
    const file = taskResultPath(projectRoot, run.runId, state.taskId, state.resultId, locator);
    if (await pathExists(file)) {
      results.push(taskResultSchema.parse(await readJsonFile(file)));
    }
  }
  return results;
}

function summarizeVerification(taskResults: TaskResult[]): {
  total: number;
  passed: number;
  notRun: number;
  failed: number;
  inconclusive: number;
} {
  const all = taskResults.flatMap((result) => result.verificationResults);
  return {
    total: all.length,
    passed: all.filter((item) => item.status === "passed").length,
    notRun: all.filter((item) => item.status === "not_run").length,
    failed: all.filter((item) => item.status === "failed").length,
    inconclusive: all.filter((item) => item.status === "inconclusive").length,
  };
}

function buildReviewPacket(
  taskPlan: Awaited<ReturnType<typeof loadCurrentTaskPlan>>,
  run: Awaited<ReturnType<typeof loadCurrentTaskPlanRun>>,
  taskResults: TaskResult[],
  aac: Awaited<ReturnType<typeof loadArchitectureArtifact>>,
): Record<string, unknown> {
  const workflowClosureRequirements = buildWorkflowClosureRequirements(aac);
  return {
    schemaVersion: "1.0",
    taskPlanId: taskPlan.taskPlanId,
    taskPlanRunId: run.runId,
    workflowClosureRequirements: workflowClosureRequirementReviewView(workflowClosureRequirements),
    groups: taskPlan.groups.map((group) => ({
      groupId: group.groupId,
      status: run.groupStates.find((state) => state.groupId === group.groupId)?.status ?? "pending",
      taskIds: group.taskIds,
      acceptanceRefs: group.acceptanceRefs,
    })),
    tasks: taskPlan.tasks.map((task) => {
      const state = run.taskStates.find((item) => item.taskId === task.taskId);
      const result = taskResults.find((item) => item.taskId === task.taskId);
      return {
        taskId: task.taskId,
        groupId: task.groupId,
        status: state?.status ?? "pending",
        acceptanceRefs: task.acceptanceRefs,
        requirementDetailRefs: task.requirementDetailRefs ?? [],
        scopeRefs: task.scopeRefs,
        changedFiles: result?.changedFiles ?? [],
        taskResultId: result?.taskResultId ?? null,
        workflowClosureRequirementIds: workflowClosureRequirements
          .filter((requirement) => taskCoversWorkflowClosure(task, requirement))
          .map((requirement) => requirement.closureId),
      };
    }),
    taskResults: taskResults.map((result) => ({
      taskResultId: result.taskResultId,
      taskId: result.taskId,
      status: result.status,
      changedFiles: result.changedFiles,
      verificationResults: result.verificationResults,
      requirementDetailEvidence: result.requirementDetailEvidence ?? [],
      conceptEvidence: result.conceptEvidence ?? [],
      runtimeDeliveryEvidence: summarizeRuntimeDeliveryEvidence(result.runtimeDeliveryEvidence),
      frontendExperienceSelfCheck: result.frontendExperienceSelfCheck ?? null,
    })),
    verificationSummary: summarizeVerificationValue(taskResults),
  };
}

async function buildConceptReviewMatrix(projectRoot: string, locator: DeliveryPhaseLocator, taskPlan: TaskPlan, taskResults: TaskResult[]): Promise<NonNullable<ReviewRequest["conceptReviewMatrix"]>> {
  const conceptsById = await readPhaseConcepts(projectRoot, locator);
  const resultEvidenceByConcept = new Map<string, Set<string>>();
  for (const result of taskResults) {
    for (const evidence of result.conceptEvidence ?? []) {
      const refs = resultEvidenceByConcept.get(evidence.conceptRef) ?? new Set<string>();
      refs.add(result.taskResultId);
      for (const ref of evidence.refs) {
        refs.add(ref);
      }
      resultEvidenceByConcept.set(evidence.conceptRef, refs);
    }
  }

  const taskRefsByConcept = new Map<string, Set<string>>();
  const expectedEvidenceByConcept = new Map<string, Set<string>>();
  for (const task of taskPlan.tasks) {
    for (const conceptRef of task.conceptRefs ?? []) {
      const taskRefs = taskRefsByConcept.get(conceptRef) ?? new Set<string>();
      taskRefs.add(task.taskId);
      taskRefsByConcept.set(conceptRef, taskRefs);
    }
    for (const intent of task.conceptVerificationIntents ?? []) {
      const expected = expectedEvidenceByConcept.get(intent.conceptRef) ?? new Set<string>();
      expected.add(intent.evidenceType);
      expectedEvidenceByConcept.set(intent.conceptRef, expected);
    }
  }

  return [...taskRefsByConcept.entries()].map(([conceptRef, taskRefs]) => ({
    conceptRef,
    priority: conceptsById.get(conceptRef)?.priority ?? "unknown",
    expectedEvidenceTypes: [...(expectedEvidenceByConcept.get(conceptRef) ?? new Set(["code"]))],
    taskRefs: [...taskRefs],
    evidenceRefs: [...(resultEvidenceByConcept.get(conceptRef) ?? new Set())],
    mustNotMisinterpretAs: conceptsById.get(conceptRef)?.mustNotMisinterpretAs ?? [],
  }));
}

function buildRequirementDetailReviewMatrix(taskPlan: TaskPlan, taskResults: TaskResult[]): NonNullable<ReviewRequest["detailReviewMatrix"]> {
  const resultByTaskId = new Map(taskResults.map((result) => [result.taskId, result]));
  const byDetail = new Map<string, {
    taskRefs: Set<string>;
    expectedVerificationIds: Set<string>;
    evidenceRefs: Set<string>;
    statuses: Set<"satisfied" | "partially_satisfied" | "not_satisfied" | "not_verified" | "missing">;
  }>();
  for (const task of taskPlan.tasks) {
    const detailIds = uniqueRefs([
      ...(task.requirementDetailRefs ?? []),
      ...task.verificationIntents.flatMap((intent) => intent.requirementDetailRefs ?? []),
    ]);
    if (detailIds.length === 0) {
      continue;
    }
    const result = resultByTaskId.get(task.taskId);
    for (const detailId of detailIds) {
      const entry = byDetail.get(detailId) ?? {
        taskRefs: new Set<string>(),
        expectedVerificationIds: new Set<string>(),
        evidenceRefs: new Set<string>(),
        statuses: new Set<"satisfied" | "partially_satisfied" | "not_satisfied" | "not_verified" | "missing">(),
      };
      entry.taskRefs.add(task.taskId);
      for (const intent of task.verificationIntents.filter((item) => (item.requirementDetailRefs ?? []).includes(detailId))) {
        entry.expectedVerificationIds.add(intent.verificationId);
      }
      const evidence = result?.requirementDetailEvidence?.find((item) => item.detailId === detailId);
      if (!result || !evidence) {
        entry.statuses.add("missing");
      } else {
        entry.statuses.add(evidence.status);
        entry.evidenceRefs.add(result.taskResultId);
        for (const ref of evidence.evidenceRefs) {
          entry.evidenceRefs.add(ref);
        }
        for (const verificationId of evidence.verificationIds) {
          entry.evidenceRefs.add(`${result.taskResultId}:${verificationId}`);
        }
      }
      byDetail.set(detailId, entry);
    }
  }
  return [...byDetail.entries()].map(([detailId, entry]) => {
    const status = aggregateDetailEvidenceStatus(entry.statuses);
    return {
      detailId,
      taskRefs: [...entry.taskRefs],
      expectedVerificationIds: [...entry.expectedVerificationIds],
      evidenceRefs: [...entry.evidenceRefs],
      status,
      summary: status === "satisfied"
        ? "All assigned task result evidence reports this requirement detail as satisfied."
        : "At least one assigned task result is missing or does not report this requirement detail as satisfied.",
    };
  });
}

function aggregateDetailEvidenceStatus(statuses: Set<"satisfied" | "partially_satisfied" | "not_satisfied" | "not_verified" | "missing">): "satisfied" | "partially_satisfied" | "not_satisfied" | "not_verified" | "missing" {
  if (statuses.has("missing")) return "missing";
  if (statuses.has("not_satisfied")) return "not_satisfied";
  if (statuses.has("not_verified")) return "not_verified";
  if (statuses.has("partially_satisfied")) return "partially_satisfied";
  return "satisfied";
}

async function readPhaseConcepts(projectRoot: string, locator: DeliveryPhaseLocator): Promise<Map<string, { priority: string; mustNotMisinterpretAs: string[] }>> {
  const file = phaseConceptGroundingPath(projectRoot, locator.deliveryId, locator.phaseId);
  if (!(await pathExists(file))) {
    return new Map();
  }
  const value = await readJsonFile(file);
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return new Map();
  }
  const concepts = (value as { concepts?: unknown }).concepts;
  if (!Array.isArray(concepts)) {
    return new Map();
  }
  const byId = new Map<string, { priority: string; mustNotMisinterpretAs: string[] }>();
  for (const concept of concepts) {
    if (!concept || typeof concept !== "object" || Array.isArray(concept)) {
      continue;
    }
    const record = concept as Record<string, unknown>;
    const conceptId = typeof record.conceptId === "string" ? record.conceptId : null;
    if (!conceptId) {
      continue;
    }
    byId.set(conceptId, {
      priority: typeof record.priority === "string" ? record.priority : "unknown",
      mustNotMisinterpretAs: Array.isArray(record.mustNotMisinterpretAs)
        ? record.mustNotMisinterpretAs.filter((item): item is string => typeof item === "string")
        : [],
    });
  }
  return byId;
}

function summarizeVerificationValue(taskResults: TaskResult[]): Record<string, number> {
  const all = taskResults.flatMap((result) => result.verificationResults);
  return {
    total: all.length,
    passed: all.filter((item) => item.status === "passed").length,
    notRun: all.filter((item) => item.status === "not_run").length,
    failed: all.filter((item) => item.status === "failed").length,
    inconclusive: all.filter((item) => item.status === "inconclusive").length,
  };
}

function uniqueRefs(refs: string[]): string[] {
  return [...new Set(refs.filter((ref) => ref.length > 0))];
}

function summarizeRuntimeDeliveryEvidence(evidence: TaskResult["runtimeDeliveryEvidence"]): Record<string, unknown> | null {
  if (!evidence || typeof evidence !== "object") {
    return null;
  }
  const value = evidence as {
    checkedFields?: unknown;
    codeLevelChecks?: unknown;
    commandsRun?: unknown;
    unverifiedItems?: unknown;
    runtimeProbeCleanup?: unknown;
    addressedFailedContractFields?: unknown;
  };
  return {
    checkedFields: Array.isArray(value.checkedFields) ? value.checkedFields.slice(0, 12) : [],
    codeLevelChecks: Array.isArray(value.codeLevelChecks) ? value.codeLevelChecks.slice(0, 12) : [],
    commandsRun: Array.isArray(value.commandsRun) ? value.commandsRun.slice(0, 8) : [],
    unverifiedItems: Array.isArray(value.unverifiedItems) ? value.unverifiedItems.slice(0, 8) : [],
    runtimeProbeCleanup: value.runtimeProbeCleanup && typeof value.runtimeProbeCleanup === "object" ? value.runtimeProbeCleanup : null,
    addressedFailedContractFields: Array.isArray(value.addressedFailedContractFields) ? value.addressedFailedContractFields.slice(0, 8) : [],
  };
}

function buildChangeContext(changeSet: Record<string, unknown> & { mode: string }): { schemaVersion: "1.0"; mode: string; changedFiles: Array<{ path: string; diffRef?: string }> } {
  const files = Array.isArray(changeSet.files) ? changeSet.files : [];
  return {
    schemaVersion: "1.0",
    mode: changeSet.mode,
    changedFiles: files
      .filter((file): file is { path: string; diffRef?: string } => typeof file === "object" && file !== null && typeof (file as { path?: unknown }).path === "string")
      .map((file) => ({
        path: file.path,
        ...(typeof file.diffRef === "string" ? { diffRef: file.diffRef } : {}),
      })),
  };
}

function buildPhaseProjection(aac: Awaited<ReturnType<typeof loadArchitectureArtifact>>, taskPlan: Awaited<ReturnType<typeof loadCurrentTaskPlan>>): Record<string, unknown> {
  const moduleRefs = new Set<string>();
  const entityRefs = new Set<string>();
  const interfaceRefs = new Set<string>();
  const userFlowRefs = new Set<string>();
  const stateMachineRefs = new Set<string>();
  const decisionRefs = new Set<string>();
  const riskRefs = new Set<string>();
  for (const task of taskPlan.tasks) {
    task.writeBoundary.artifactRefs.modules.forEach((ref) => moduleRefs.add(ref));
    task.writeBoundary.artifactRefs.entities.forEach((ref) => entityRefs.add(ref));
    task.writeBoundary.artifactRefs.interfaces.forEach((ref) => interfaceRefs.add(ref));
    task.writeBoundary.artifactRefs.userFlows.forEach((ref) => userFlowRefs.add(ref));
    task.writeBoundary.artifactRefs.stateMachines.forEach((ref) => stateMachineRefs.add(ref));
    task.writeBoundary.artifactRefs.decisions.forEach((ref) => decisionRefs.add(ref));
    task.writeBoundary.artifactRefs.risks.forEach((ref) => riskRefs.add(ref));
  }
  return {
    engineeringBoundary: aac.engineeringBoundary,
    modules: aac.modules.filter((item) => moduleRefs.has(item.moduleId)),
    dataModel: {
      entities: aac.dataModel.entities.filter((item) => entityRefs.has(item.entityId)),
      relationships: aac.dataModel.relationships.filter((item) => entityRefs.has(item.fromEntityRef) || entityRefs.has(item.toEntityRef)),
      globalConstraints: aac.dataModel.constraints,
    },
    interfaces: aac.interfaces.filter((item) => interfaceRefs.has(item.interfaceId)),
    userFlows: aac.userFlows.filter((item) => userFlowRefs.has(item.flowId)),
    stateMachines: aac.stateMachines.filter((item) => stateMachineRefs.has(item.stateMachineId)),
    acceptanceMatrix: aac.acceptanceMatrix.filter((item) => taskPlan.scopeSnapshot.acceptanceRefs.includes(item.acceptanceId)),
    risksAndDecisions: {
      decisions: aac.risksAndDecisions.decisions.filter((item) => decisionRefs.has(item.decisionId)),
      risks: aac.risksAndDecisions.risks.filter((item) => riskRefs.has(item.riskId)),
      assumptions: aac.risksAndDecisions.assumptions,
      deferredNotes: aac.risksAndDecisions.deferredNotes,
    },
  };
}

function buildFallbackReviewResult(request: ReviewRequest): ReviewResult {
  const now = new Date().toISOString();
  const mustAcceptance = request.reviewScope.acceptanceRefs
    .map((acceptanceRef) => ({
      acceptanceRef,
      status: "not_reviewed" as const,
      supportingTaskResults: [],
      evidenceStatus: "missing" as const,
      notes: ["ReviewResult contract validation failed after maxReviewGenerationAttempts."],
    }));
  return reviewResultSchema.parse({
    schemaVersion: "1.0",
    reviewId: request.requestId,
    source: {
      requestId: request.requestId,
      phaseId: request.source.phaseId,
      taskPlanId: request.source.taskPlanId,
      taskPlanRunId: request.source.taskPlanRunId,
    },
    decision: "blocked",
    findings: [{
      findingId: "review-result-invalid",
      severity: "critical",
      category: "review_limitation",
      summary: "ReviewResult could not be generated in a valid contract shape after multiple attempts.",
      evidence: "ReviewResult contract validation failed after maxReviewGenerationAttempts.",
      readRefs: [{
        type: "review_packet",
        ref: request.reviewPacketRef ?? request.requestId,
        reason: "Fallback review was created because the generated ReviewResult did not validate against the request contract.",
      }],
      evidenceRefs: [],
      groupRefs: [],
      taskRefs: [],
      acceptanceRefs: [],
      artifactRefs: {},
      location: null,
      taskRelevance: "unknown",
      scopeRelation: "unknown",
      introducedByCurrentTask: "unknown",
      recommendedNextAction: "manual_review",
    }],
    coverageAssessment: {
      mustAcceptance,
      summary: {
        totalMust: mustAcceptance.length,
        satisfied: 0,
        insufficientEvidence: 0,
        notSatisfied: 0,
        notReviewed: mustAcceptance.length,
      },
    },
    limitations: [{
      code: "REVIEW_RESULT_INVALID",
      summary: "ReviewResult contract validation failed after 3 attempts.",
      impact: "Review cannot safely approve or request code repair without manual resolution.",
    }],
    pendingActions: [],
    nextAction: {
      type: "manual_review",
      reason: "review_result_invalid_after_retries",
      targetNode: "manual_review",
    },
    createdAt: now,
    updatedAt: now,
  });
}

function buildReviewResultSchemaShape(
  requestId: string,
  phaseId: string,
  taskPlanId: string,
  taskPlanRunId: string,
): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    reviewId: requestId,
    source: {
      requestId,
      phaseId,
      taskPlanId,
      taskPlanRunId,
    },
    decision: "approved | approved_with_notes | changes_requested | blocked | needs_user_decision",
    findings: [{
      findingId: "finding-001",
      findingType: "concept_missing | concept_misunderstanding | concept_evidence_missing | deferred_concept_leakage | surface_only_concept_implementation",
      conceptRef: "concept ref from conceptReviewMatrix when findingType is concept-related",
      severity: "critical | major | minor | note",
      severityClass: "blocking | manual_review | warning",
      evidenceKind: "static | command | runtime_probe | browser | environment | user_observation",
      failureClass: "product_defect | contract_violation | environment_blocker | insufficient_evidence | subjective_quality",
      category: reviewFindingCategorySchema.options.join(" | "),
      summary: "Short review finding summary.",
      evidence: "Concrete evidence from diffRef, fullDiffRef, task result, verification output, or reviewed file content.",
      readRefs: [{
        type: "review_packet | change_context | diff_ref | changed_file | task_result | verification_evidence",
        ref: "Allowed source ref. For changed_file use a current changed file path; for verification_evidence use verificationId, taskResultId:verificationId, or taskId:verificationId.",
        reason: "Why this ref supports the finding.",
      }],
      evidenceRefs: [{
        type: "task_result | verification_result | diff_ref | changed_file | manual_note",
        ref: "Allowed evidence ref. changed_file refs must be current changed files; verification_result refs must come from current task result verification ids.",
        reason: "Why this evidence matters.",
      }],
      groupRefs: ["group-id-from-reviewScope"],
      taskRefs: ["task-id-from-reviewScope"],
      acceptanceRefs: ["acceptance-ref-from-reviewScope"],
      artifactRefs: {
        modules: [],
        entities: [],
        interfaces: [],
        userFlows: [],
        stateMachines: [],
        decisions: [],
        risks: [],
      },
      location: {
        file: null,
        line: null,
        diffRef: null,
      },
      taskRelevance: "direct | indirect | unrelated | unknown",
      scopeRelation: "within_task_changed_files | outside_changed_files | unknown",
      introducedByCurrentTask: "yes | no | unknown",
      recommendedNextAction: reviewNextActionTypeSchema.options.join(" | "),
      recommendedNextActionRule: "Use exactly one enum value above. Do not write prose such as 'run tests later' or 'no repair required'. Put explanation in evidence, summary, limitations, or nextAction.reason.",
    }],
    coverageAssessment: {
      mustAcceptance: [{
        acceptanceRef: "must acceptance ref",
        status: "satisfied | not_satisfied | insufficient_evidence | not_reviewed",
        supportingTaskResults: ["taskResultId"],
        evidenceStatus: "sufficient | insufficient | missing | not_applicable",
        notes: ["Short note."],
      }],
      summary: {
        totalMust: 0,
        satisfied: 0,
        insufficientEvidence: 0,
        notSatisfied: 0,
        notReviewed: 0,
      },
    },
    limitations: [{
      code: "DIFF_REF_UNREADABLE | NO_GIT_DIFF | FILE_CONTENT_UNREADABLE | INSUFFICIENT_CONTEXT | REVIEW_RESULT_INVALID",
      summary: "Short limitation summary.",
      impact: "How this affects review confidence or routing.",
    }],
    pendingActions: [{
      type: reviewNextActionTypeSchema.options.filter((item) => item !== "done" && item !== "continue_to_next_phase").join(" | "),
      findingRefs: ["finding-id-not-used-by-top-level-nextAction"],
      reason: "Why this action remains pending after the top-level action.",
    }],
    nextAction: {
      type: reviewNextActionTypeSchema.options.join(" | "),
      typeRule: "Use exactly one enum value above. Do not write prose.",
      reason: "Why this is the first action to take.",
      targetNode: "same as type unless a specific route node is required",
      targetPhaseId: "next phase id when type=continue_to_next_phase, otherwise omit",
      targetTaskIds: ["task ids relevant to the action, optional"],
      findingRefs: ["blocking finding ids routed by this action, optional"],
      userVisibleState: "short user-facing status, optional",
    },
    createdAt: new Date(0).toISOString(),
    updatedAt: new Date(0).toISOString(),
  };
}

function buildReviewResultValidatorRules(changeSetMode: string): string[] {
  const common = [
    "source.requestId, source.phaseId, source.taskPlanId, and source.taskPlanRunId must exactly match this ReviewRequest.",
    "Every finding.taskRefs value must come from outputContract.allowedRefs.taskIds, derived from reviewPacket.tasks.",
    "Every finding.acceptanceRefs value must come from reviewScope.acceptanceRefs.",
    "Every coverageAssessment.mustAcceptance[].acceptanceRef must come from reviewScope.acceptanceRefs.",
    "Every acceptanceRef in reviewScope.acceptanceRefs must appear in coverageAssessment.mustAcceptance.",
    "Every finding must include readRefs that cite reviewPacketRef, changeContextRef, a diffRef, or a changed file path.",
    "changed_file readRefs/evidenceRefs may use equivalent current changed file path forms such as ./path or changed_file:path.",
    "verification_evidence readRefs and verification_result evidenceRefs may cite verificationId, taskResultId:verificationId, or taskId:verificationId from current task results.",
    "If a must acceptance is not satisfied, at least one finding must reference that acceptance.",
    "finding.recommendedNextAction must be structurally consistent with finding category, severityClass, and failureClass.",
    "severityPolicy is Agent-facing decision guidance, not a CLI semantic rule engine.",
    "reviewSignals are facts only; do not copy signal kinds into severity/failureClass without semantic review.",
    "If a frontend_workflow_closure reviewSignal has closureSatisfied=false, approved/approved_with_notes is structurally inconsistent. Route execution_repair when a covering task/result exists, or taskplan_repair when missingTaskAssignment=true.",
    "pendingActions[].findingRefs must refer to existing findings whose recommendedNextAction equals pendingActions[].type.",
    "pendingActions must not repeat the top-level nextAction.type.",
    "Top-level nextAction.type is the first action to take. Put other blocking action groups into pendingActions.",
    "Top-level nextAction priority is needs_user_decision, then manual_review for blocking review_limitation/environment_or_dependency, then architecture_artifact_repair, taskplan_repair, execution_repair, then continue_to_next_phase or done.",
    "approved and approved_with_notes must not include blocking or manual_review findings and must not include pendingActions.",
    "changes_requested requires at least one execution_repair finding.",
    "blocked requires at least one architecture_artifact_repair, taskplan_repair, or manual_review finding.",
    "needs_user_decision requires at least one needs_user_decision finding.",
    "Use continue_to_next_phase only when reviewScope.nextPhasePreview.kind=candidate; use done when reviewScope.nextPhasePreview.kind=none.",
  ];
  if (changeSetMode === "current_file_content") {
    return [
      ...common,
      "For non-git review, only findings with taskRelevance=direct and scopeRelation=within_task_changed_files may be critical or major.",
      "For non-git review, indirect, unrelated, outside_changed_files, or unknown findings must be minor or note and must not drive top-level blocking decisions.",
    ];
  }
  return [
    ...common,
    "For git review, read reviewPacketRef and changeContextRef before reading diffs.",
    "For git review, prefer per-file diffRefs and use fullDiffRef only for cross-file findings that cannot be supported by per-file diffs.",
    "For git review, read file diffRef before making line-level findings for that file.",
    "If a required diffRef cannot be read, use a review_limitation finding instead of guessing.",
  ];
}

function buildReviewSeverityPolicy(): Record<string, unknown> {
  return {
    blockingMeans: "contract broken or product clearly unusable",
    manualReviewMeans: "environment blocker, insufficient evidence requiring user judgment, or subjective quality decision",
    warningMeans: "risk recorded but phase can be approved",
    doNotBlockOn: [
      "Playwright unavailable by itself",
      "Docker unavailable by itself",
      "registry or dependency network failure by itself",
      "temporary runtime/probe cleanup failure, unknown cleanup status, or not_safe_to_cleanup by itself",
      "unknown runtimeKind when build/start/probe are complete",
      "subjective visual polish preference",
    ],
    mustBlockOn: [
      "missing required RuntimeDeliveryContract",
      "missing build/start/previewPath for modified runtime contract",
      "sleep placeholder start command",
      "root preview path objectively returns 404/500 due to app wiring",
      "frontend required but no visible UI surface",
      "usable_internal_product multi-workflow UI without navigation",
      "required frontend workflow closure self-reports satisfied while data binding is static/mock/not_applicable or knownGaps remain",
    ],
    cliBoundary: [
      "CLI validates structural consistency only.",
      "CLI does not classify stderr, screenshots, UI quality, or root cause semantics.",
      "Agent produces semantic finding classification from policy, facts, and artifact refs.",
    ],
  };
}

function buildFrontendExperienceReview(aac: Awaited<ReturnType<typeof loadArchitectureArtifact>>): Record<string, unknown> {
  const workflowClosureRequirements = buildWorkflowClosureRequirements(aac);
  return {
    required: aac.frontendExperience?.required ?? false,
    frontendExperienceRef: "architectureArtifactRef#/frontendExperience",
    contract: aac.frontendExperience ?? null,
    workflowClosureRequirements: workflowClosureRequirementReviewView(workflowClosureRequirements),
    workflowClosureReviewRule: workflowClosureRequirements.length > 0
      ? "Required workflow closures are satisfied only by wired user-action-to-interface evidence with no required closure known gaps. Static-only or demo-only self-checks are not satisfied."
      : "No AAC-derived workflow closure requirements for this phase.",
    reviewGuidance: [
      "Block objective contract violations such as no visible UI when frontend is required.",
      "When workflowClosureRequirements is non-empty, check matching reviewSignals before approving.",
      "For workflow closures with operationPathRefs/dataViewRefs/actionRefs, check that evidence covers target discovery, action entry, declared interface invocation, result refresh/readback or status observation, and success/blocking feedback.",
      "Use manual_review for subjective visual polish decisions.",
      "Playwright failure alone is not a frontend product defect.",
    ],
  };
}

function workflowClosureRequirementReviewView(requirements: ReturnType<typeof buildWorkflowClosureRequirements>): Array<Record<string, unknown>> {
  return requirements.map((requirement) => ({
    closureId: requirement.closureId,
    workflowRef: requirement.workflowRef,
    workflowName: requirement.workflowName,
    acceptanceRefs: requirement.acceptanceRefs,
    interfaceRefs: requirement.interfaceRefs,
    surfaceRefs: requirement.surfaceRefs,
    operationPathRefs: requirement.operationPathRefs,
    dataViewRefs: requirement.dataViewRefs,
    actionRefs: requirement.actionRefs,
    requiredDataBindingMode: requirement.requiredDataBindingMode,
    staticModePolicy: requirement.staticModePolicy,
    knownGapPolicy: requirement.knownGapPolicy,
    requiredEvidence: requirement.requiredEvidence,
    interfaces: requirement.interfaces.map((contract) => ({
      interfaceId: contract.interfaceId,
      name: contract.name,
      type: contract.type,
      role: contract.role ?? null,
      method: contract.method ?? null,
      path: contract.path ?? null,
      requestSchema: contract.requestSchema,
      responseSchema: contract.responseSchema,
      errorSchema: contract.errorSchema,
    })),
  }));
}

function buildRuntimeDeliveryReview(aac: Awaited<ReturnType<typeof loadArchitectureArtifact>>): Record<string, unknown> {
  return {
    required: Boolean(aac.runtimeDelivery && aac.runtimeDelivery.status !== "not_applicable"),
    runtimeDeliveryRef: "architectureArtifactRef#/runtimeDelivery",
    contract: aac.runtimeDelivery ?? null,
    reviewGuidance: [
      "Block missing or structurally broken runtime contract.",
      "Block code/runtime mismatch when supported by static or command evidence.",
      "Runtime delivery review checks code-level consistency between RuntimeDeliveryContract, task runtimeDeliveryRequirement, changed files, and runtimeDeliveryEvidence.",
      "When runtimeDelivery.status=modified, require exactly one runtime_delivery_closure task and verify its runtimeDeliveryEvidence covers every required code-level check.",
      "Do not require Docker, clean install, registry access, browser proof, or full deploy for Review approval by default.",
      "If runtimeDeliveryEvidence omits required code-level checks or leaves relevant changed fields unaddressed, request execution repair or manual review according to severityPolicy.",
      "Classify Docker, registry, browser, and local port failures as environment blockers unless separate product defect evidence exists.",
      "Treat runtimeDeliveryEvidence.runtimeProbeCleanup status failed, unknown, or not_safe_to_cleanup as warning/manual note only. Do not recommend execution repair for cleanup alone unless there is separate evidence of a product/runtime contract defect.",
    ],
  };
}

function buildReviewSignals(
  taskPlan: Awaited<ReturnType<typeof loadCurrentTaskPlan>>,
  run: Awaited<ReturnType<typeof loadCurrentTaskPlanRun>>,
  taskResults: TaskResult[],
  aac: Awaited<ReturnType<typeof loadArchitectureArtifact>>,
): Array<Record<string, unknown>> {
  const signals: Array<Record<string, unknown>> = [{
    signalId: "sig-task-run-summary",
    kind: "task_run_summary",
    status: run.status,
    totalTasks: run.summary.total,
    completedTasks: run.summary.completed,
    failedTasks: run.summary.failed,
    blockedTasks: run.summary.blocked,
  }];
  const workflowClosureRequirements = buildWorkflowClosureRequirements(aac);
  for (const detail of buildRequirementDetailReviewMatrix(taskPlan, taskResults)) {
    const detailSatisfied = detail.status === "satisfied";
    signals.push({
      signalId: `sig-requirement-detail-${safeSignalId(detail.detailId)}`,
      kind: "requirement_detail_evidence",
      detailId: detail.detailId,
      taskRefs: detail.taskRefs,
      expectedVerificationIds: detail.expectedVerificationIds,
      evidenceRefs: detail.evidenceRefs,
      detailSatisfied,
      actualStatus: detail.status,
      recommendedNextAction: detailSatisfied ? "none" : "execution_repair",
      reason: detailSatisfied
        ? "Assigned TaskResult evidence reports this requirement detail as satisfied."
        : "Assigned TaskResult evidence is missing or does not report this requirement detail as satisfied.",
    });
  }
  for (const requirement of workflowClosureRequirements) {
    const coveringTasks = taskPlan.tasks.filter((task) => taskCoversWorkflowClosure(task, requirement));
    if (coveringTasks.length === 0) {
      signals.push({
        signalId: `sig-workflow-closure-missing-${safeSignalId(requirement.closureId)}`,
        kind: "frontend_workflow_closure",
        closureId: requirement.closureId,
        workflowRef: requirement.workflowRef,
        interfaceRefs: requirement.interfaceRefs,
        acceptanceRefs: requirement.acceptanceRefs,
        taskRefs: [],
        closureSatisfied: false,
        missingTaskAssignment: true,
        recommendedNextAction: "taskplan_repair",
        reason: "No TaskPlan task structurally covers this AAC-derived workflow closure requirement.",
      });
      continue;
    }
    for (const task of coveringTasks) {
      const result = taskResults.find((item) => item.taskId === task.taskId);
      const violation = result
        ? frontendSelfCheckViolatesRequiredClosure(result, [requirement.closureId])
        : { violates: true, actualMode: null, status: null, knownGapCount: 0 };
      const closureSatisfied = Boolean(
        result &&
        !violation.violates &&
        result.frontendExperienceSelfCheck &&
        typeof result.frontendExperienceSelfCheck === "object" &&
        !Array.isArray(result.frontendExperienceSelfCheck) &&
        (result.frontendExperienceSelfCheck as Record<string, unknown>).status === "satisfied" &&
        ((result.frontendExperienceSelfCheck as Record<string, unknown>).dataBinding as Record<string, unknown> | undefined)?.mode === "wired" &&
        Array.isArray((result.frontendExperienceSelfCheck as Record<string, unknown>).knownGaps) &&
        ((result.frontendExperienceSelfCheck as Record<string, unknown>).knownGaps as unknown[]).length === 0,
      );
      signals.push({
        signalId: `sig-workflow-closure-${safeSignalId(requirement.closureId)}-${safeSignalId(task.taskId)}`,
        kind: "frontend_workflow_closure",
        closureId: requirement.closureId,
        workflowRef: requirement.workflowRef,
        interfaceRefs: requirement.interfaceRefs,
        acceptanceRefs: requirement.acceptanceRefs,
        taskRefs: [task.taskId],
        taskResultId: result?.taskResultId ?? null,
        closureSatisfied,
        actualFrontendSelfCheckStatus: violation.status,
        actualDataBindingMode: violation.actualMode,
        knownGapCount: violation.knownGapCount,
        requiredDataBindingMode: "wired",
        recommendedNextAction: closureSatisfied ? "none" : "execution_repair",
        reason: closureSatisfied
          ? "TaskResult self-check reports wired closure evidence with no known gaps."
          : "Required workflow closure is not satisfied by TaskResult frontend self-check evidence.",
      });
    }
  }
  for (const task of taskPlan.tasks) {
    if (task.frontendExperienceRequirement) {
      signals.push({
        signalId: `sig-frontend-task-${task.taskId}`,
        kind: "task_contract_presence",
        taskId: task.taskId,
        contractType: "frontend_experience",
      });
    }
    if (task.runtimeDeliveryRequirement) {
      signals.push({
        signalId: `sig-runtime-task-${task.taskId}`,
        kind: "task_contract_presence",
        taskId: task.taskId,
        contractType: "runtime_delivery",
        isClosureTask: task.taskKind === "runtime_delivery_closure",
        affectedContractFields: task.runtimeDeliveryRequirement.affectedContractFields ?? [],
        requiredCodeLevelChecks: (task.runtimeDeliveryRequirement.requiredCodeLevelChecks ?? []).map((check) => check.checkId),
      });
    }
  }
  for (const result of taskResults) {
    const task = taskPlan.tasks.find((item) => item.taskId === result.taskId);
    if (result.runtimeDeliveryEvidence) {
      const runtimeEvidence = summarizeRuntimeDeliveryEvidence(result.runtimeDeliveryEvidence);
      const codeLevelChecks = Array.isArray(runtimeEvidence?.codeLevelChecks) ? runtimeEvidence.codeLevelChecks : [];
      const checkedFields = Array.isArray(runtimeEvidence?.checkedFields) ? runtimeEvidence.checkedFields : [];
      signals.push({
        signalId: `sig-runtime-evidence-${result.taskResultId}`,
        kind: "task_result_evidence_presence",
        taskResultId: result.taskResultId,
        taskId: result.taskId,
        evidenceType: "runtime_delivery",
        isClosureTaskEvidence: task?.taskKind === "runtime_delivery_closure",
        checkedFieldCount: checkedFields.length,
        checkedFields,
        codeLevelCheckCount: codeLevelChecks.length,
        codeLevelCheckIds: codeLevelChecks
          .filter((check): check is { checkId: string } => typeof check === "object" && check !== null && typeof (check as { checkId?: unknown }).checkId === "string")
          .map((check) => check.checkId),
        commandCount: Array.isArray(runtimeEvidence?.commandsRun) ? runtimeEvidence.commandsRun.length : 0,
        unverifiedItemCount: Array.isArray(runtimeEvidence?.unverifiedItems) ? runtimeEvidence.unverifiedItems.length : 0,
      });
    }
    if (result.frontendExperienceSelfCheck) {
      signals.push({
        signalId: `sig-frontend-evidence-${result.taskResultId}`,
        kind: "task_result_evidence_presence",
        taskResultId: result.taskResultId,
        evidenceType: "frontend_experience",
      });
    }
  }
  return signals;
}

function safeSignalId(value: string): string {
  return value.replace(/[^a-zA-Z0-9_-]+/g, "-");
}

async function materializeManualReviewRequest(projectRoot: string, locator: DeliveryPhaseLocator, result: ReviewResult): Promise<void> {
  const requestId = createId("manual-review");
  const file = manualReviewRequestPath(projectRoot, requestId, locator);
  const candidateFile = path.join(".loom", "deliveries", locator.deliveryId, "tmp", locator.phaseId, "manual-review", requestId, "resolution.json")
    .split(path.sep)
    .join("/");
  const manualRequest = {
    schemaVersion: "1.0",
    manualReviewRequestId: requestId,
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    status: "waiting_user",
    source: {
      trigger: "review_result",
      triggerRef: toProjectRelative(projectRoot, reviewResultPath(projectRoot, result.reviewId, locator)),
      reason: result.nextAction.reason,
      findingRefs: result.findings.map((finding) => finding.findingId),
    },
    instruction: {
      title: "需要人工确认当前阶段结果",
      message: manualReviewUserMessage(result),
      visibleReason: manualReviewVisibleReason(result),
      userPrompt: "请直接回复下面两种选择之一：1. 可以继续：表示你已经人工确认当前问题不阻塞交付，loom 会按带备注通过继续推进；2. 需要修改：请同时说明要改什么，Agent 会按你的说明选择 repair 路由并继续修复。",
      acceptedShortReplies: [
        {
          text: "可以继续",
          mapsTo: "approve_override",
          effect: "人工确认当前 review 阻塞项可接受，按 approved_with_notes 继续后续阶段或完成交付。",
          exampleUserReply: "可以继续，我已确认这个 Playwright 环境问题不阻塞交付。",
        },
        {
          text: "需要修改",
          mapsTo: "request_changes",
          effect: "进入 repair。请说明要修改代码、测试、任务计划、架构设计，还是需要用户决策。",
          exampleUserReply: "需要修改，请修复 Playwright 超时并补齐浏览器级验证。",
        },
      ],
      agentResolutionProtocol: {
        responsibility: "When the user replies with one of the short replies or equivalent natural language, Agent must create ManualReviewResolution and run review resolve. Do not ask the user to write JSON or run review resolve manually.",
        approveOverride: {
          decision: "approve_override",
          changeRequest: null,
          nextActionType: result.nextAction.type === "continue_to_next_phase" ? "continue_to_next_phase" : "done",
          rule: "Use when the user says the manual review issue is acceptable or non-blocking.",
        },
        requestChanges: {
          decision: "request_changes",
          routeOptions: ["execution_repair", "taskplan_repair", "architecture_artifact_repair", "needs_user_decision"],
          defaultRoute: "execution_repair",
          routeRule: "Use execution_repair for code, test, local verification, or project configuration changes; taskplan_repair for task structure; architecture_artifact_repair for AAC/design facts; needs_user_decision for scope, acceptance, external environment, credential, network, or policy decisions.",
        },
      },
    },
    routingGuidance: [
      {
        route: "execution_repair",
        useWhen: "用户指出的是当前工作区内可由 Agent 修改代码、测试或项目配置解决的问题，例如实现错误、测试断言错误、可本地执行的验证证据不足。",
        doNotUseWhen: "依赖仓库、网络、凭证、外部服务、浏览器下载源或其他工作区外部资源不可用；这类情况不能靠代码执行修复闭环。",
      },
      {
        route: "taskplan_repair",
        useWhen: "用户指出的是任务拆分、任务顺序、任务覆盖、任务粒度或 TaskPlan 引用关系问题。",
      },
      {
        route: "architecture_artifact_repair",
        useWhen: "用户指出的是模块、接口、数据模型、状态机、验收覆盖、工程边界等 AAC 设计事实问题。",
      },
      {
        route: "needs_user_decision",
        useWhen: "用户反馈表达了范围取舍、是否保留越界实现、是否改变验收口径、是否修改技术栈，或需要用户处理工作区外部环境/网络/凭证/服务后才能继续的问题。",
      },
    ],
    resolutionContract: {
      candidateFile,
      schema: "ManualReviewResolution",
      submitCommand: {
        name: "review resolve",
        argv: [
          "review",
          "resolve",
          "--delivery-id",
          locator.deliveryId,
          "--phase-id",
          locator.phaseId,
          "--candidate-file",
          "{candidateFile}",
        ],
      },
    },
    validatorPolicy: {
      cliValidates: ["schema", "decision enum", "route enum", "nextAction mapping", "ref existence"],
      cliDoesNotValidate: ["whether user judgment is correct", "whether route is semantically best"],
    },
    createdAt: new Date().toISOString(),
  };
  await writeJsonAtomic(file, manualRequest);
  await updateReviewLatest(projectRoot, locator, null, {
    latestManualReviewRequestRef: toProjectRelative(projectRoot, file),
  });
}

function manualReviewUserMessage(result: ReviewResult): string {
  const hasEnvironmentIssue = result.findings.some((finding) => finding.category === "environment_or_dependency" || finding.category === "external_system_unavailable");
  if (hasEnvironmentIssue) {
    return "自动 review / repair 无法可靠继续，因为存在环境、依赖、网络、凭证或外部服务问题。请先查看下面的原因摘要。如果你已经处理环境并确认可以继续，回复：可以继续。如果还需要修改代码或配置，回复：需要修改，并说明要改什么。";
  }
  return "自动 review / repair 无法继续。请检查本阶段变更、ReviewResult 和相关 evidence。如果你确认可以继续，回复：可以继续。如果还需要修改，回复：需要修改，并说明要改什么。";
}

function manualReviewVisibleReason(result: ReviewResult): Record<string, unknown> {
  const relevantFindings = result.findings.filter((finding) =>
    result.nextAction.findingRefs?.includes(finding.findingId) ||
    finding.recommendedNextAction === "manual_review" ||
    finding.recommendedNextAction === "needs_user_decision"
  );
  return {
    nextAction: {
      type: result.nextAction.type,
      reason: result.nextAction.reason,
      userVisibleState: result.nextAction.userVisibleState ?? null,
    },
    findings: relevantFindings.map((finding) => ({
      findingId: finding.findingId,
      severity: finding.severity,
      category: finding.category,
      summary: finding.summary,
      evidence: finding.evidence,
      taskRefs: finding.taskRefs,
      acceptanceRefs: finding.acceptanceRefs,
      recommendedNextAction: finding.recommendedNextAction,
    })),
    limitations: result.limitations.map((limitation) => ({
      code: limitation.code,
      summary: limitation.summary,
      impact: limitation.impact,
    })),
    userDecisionHint: "如果原因是网络、依赖仓库、凭证、外部服务或本机环境不可用，请先处理外部环境，再用“可以继续”放行；如果需要改代码或项目配置，请用“需要修改”并说明问题。",
  };
}

async function updateReviewLatest(projectRoot: string, locator: DeliveryPhaseLocator, result: ReviewResult | null, patch: Record<string, unknown> | null): Promise<void> {
  const latestPath = reviewLatestPath(projectRoot, locator);
  const existing = await pathExists(latestPath)
    ? await readJsonFile(latestPath)
    : {};
  const next = {
    ...(typeof existing === "object" && existing !== null ? existing as Record<string, unknown> : {}),
    ...(result
      ? {
          latestReviewId: result.reviewId,
          latestResultRef: toProjectRelative(projectRoot, reviewResultPath(projectRoot, result.reviewId, locator)),
          effectiveDecision: result.decision,
          effectiveNextAction: result.nextAction,
        }
      : {}),
    ...(patch ?? {}),
    updatedAt: new Date().toISOString(),
  };
  await writeJsonAtomic(latestPath, next);
}

async function syncRouteFromReview(
  projectRoot: string,
  locator: DeliveryPhaseLocator,
  rawAction: Record<string, unknown>,
  source: string,
): Promise<void> {
  const type = typeof rawAction.type === "string" ? rawAction.type : "manual_review";
  const targetNode = typeof rawAction.targetNode === "string" ? rawAction.targetNode : type;
  const reason = typeof rawAction.reason === "string" ? rawAction.reason : "REVIEW_ROUTE_DECISION";
  const targetPhaseId = typeof rawAction.targetPhaseId === "string" ? rawAction.targetPhaseId : undefined;
  const targetTaskIds = Array.isArray(rawAction.targetTaskIds) ? rawAction.targetTaskIds.filter((item): item is string => typeof item === "string") : [];
  const findingRefs = Array.isArray(rawAction.findingRefs) ? rawAction.findingRefs.filter((item): item is string => typeof item === "string") : [];
  const routeType = mapReviewActionToRouteAction(type);
  await updateRouteState({
    projectRoot,
    locator,
    deliveryStatus: deliveryStatusForReviewRoute(routeType),
    phaseStatus: phaseStatusForReviewRoute(routeType),
    nextAction: {
      type: routeType,
      source,
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      reason,
      targetNode,
      ...(targetPhaseId ? { targetPhaseId } : {}),
      refs: {
        ...(targetPhaseId ? { targetPhaseId } : {}),
        ...(targetTaskIds.length > 0 ? { targetTaskIds } : {}),
        ...(findingRefs.length > 0 ? { findingRefs } : {}),
      },
    },
  });
}

function routeActionFromReviewResult(result: ReviewResult, locator: DeliveryPhaseLocator) {
  const rawAction = result.nextAction as unknown as Record<string, unknown>;
  return routeActionFromRawReviewAction(rawAction, locator, "review_result");
}

function routeActionFromRawReviewAction(rawAction: Record<string, unknown>, locator: DeliveryPhaseLocator, source: string) {
  const type = typeof rawAction.type === "string" ? rawAction.type : "manual_review";
  const targetNode = typeof rawAction.targetNode === "string" ? rawAction.targetNode : type;
  const reason = typeof rawAction.reason === "string" ? rawAction.reason : "REVIEW_ROUTE_DECISION";
  const routeType = mapReviewActionToRouteAction(type);
  const targetPhaseId = typeof rawAction.targetPhaseId === "string" ? rawAction.targetPhaseId : undefined;
  const targetTaskIds = Array.isArray(rawAction.targetTaskIds) ? rawAction.targetTaskIds.filter((item): item is string => typeof item === "string") : [];
  const findingRefs = Array.isArray(rawAction.findingRefs) ? rawAction.findingRefs.filter((item): item is string => typeof item === "string") : [];
  return {
    type: routeType,
    source,
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    reason,
    targetNode,
    ...(targetPhaseId ? { targetPhaseId } : {}),
    refs: {
      ...(targetPhaseId ? { targetPhaseId } : {}),
      ...(targetTaskIds.length > 0 ? { targetTaskIds } : {}),
      ...(findingRefs.length > 0 ? { findingRefs } : {}),
    },
  };
}

function mapReviewActionToRouteAction(type: string):
  | "continue_to_next_phase"
  | "done"
  | "execution_repair"
  | "taskplan_repair"
  | "architecture_artifact_repair"
  | "needs_user_decision"
  | "manual_review" {
  if (type === "continue_to_next_phase") return "continue_to_next_phase";
  if (type === "done") return "done";
  if (type === "execution_repair") return "execution_repair";
  if (type === "taskplan_repair") return "taskplan_repair";
  if (type === "architecture_artifact_repair") return "architecture_artifact_repair";
  if (type === "needs_user_decision") return "needs_user_decision";
  return "manual_review";
}

function deliveryStatusForReviewRoute(type: ReturnType<typeof mapReviewActionToRouteAction>): "planning" | "repairing" | "waiting_user" | "completed" {
  if (type === "continue_to_next_phase") return "planning";
  if (type === "done") return "completed";
  if (type === "needs_user_decision" || type === "manual_review") return "waiting_user";
  return "repairing";
}

function phaseStatusForReviewRoute(type: ReturnType<typeof mapReviewActionToRouteAction>): "pending" | "repairing" | "waiting_user" | "completed" {
  if (type === "continue_to_next_phase") return "completed";
  if (type === "done") return "completed";
  if (type === "needs_user_decision" || type === "manual_review") return "waiting_user";
  return "repairing";
}

async function latestReviewId(projectRoot: string, locator: DeliveryPhaseLocator): Promise<string | null> {
  const latestPath = reviewLatestPath(projectRoot, locator);
  if (!(await pathExists(latestPath))) return null;
  const json = await readJsonFile(latestPath);
  if (typeof json === "object" && json !== null && typeof (json as Record<string, unknown>).latestReviewId === "string") {
    return (json as Record<string, string>).latestReviewId;
  }
  return null;
}

async function incrementInvalidReviewAttempts(projectRoot: string, locator: DeliveryPhaseLocator, reviewId: string): Promise<number> {
  const latestPath = reviewLatestPath(projectRoot, locator);
  const existing = await pathExists(latestPath) ? await readJsonFile(latestPath) : {};
  const object = typeof existing === "object" && existing !== null ? existing as Record<string, unknown> : {};
  const attemptsByReview = typeof object.invalidResultAttemptsByReview === "object" && object.invalidResultAttemptsByReview !== null
    ? object.invalidResultAttemptsByReview as Record<string, unknown>
    : {};
  const attempts = (typeof attemptsByReview[reviewId] === "number" ? attemptsByReview[reviewId] as number : 0) + 1;
  attemptsByReview[reviewId] = attempts;
  await writeJsonAtomic(latestPath, {
    ...object,
    invalidResultAttemptsByReview: attemptsByReview,
    updatedAt: new Date().toISOString(),
  });
  return attempts;
}

async function loadNextPhasePreview(projectRoot: string, locator: DeliveryPhaseLocator, brainstormRunId: string): Promise<NonNullable<ReviewRequest["reviewScope"]["nextPhasePreview"]>> {
  const brainstormLocator = await getLocatorForBrainstormRun(projectRoot, brainstormRunId).catch(() => locator);
  const file = brainstormContractPath(projectRoot, brainstormLocator.deliveryId);
  if (!(await pathExists(file))) {
    return { kind: "none", reason: "BrainstormContract is missing, so no next phase preview is available." };
  }
  const json = await readJsonFile(file);
  if (typeof json !== "object" || json === null) {
    return { kind: "none", reason: "BrainstormContract is invalid, so no next phase preview is available." };
  }
  const preview = (json as { phasePlan?: { nextPhasePreview?: unknown } }).phasePlan?.nextPhasePreview;
  if (
    typeof preview === "object" &&
    preview !== null &&
    (preview as { kind?: unknown }).kind === "candidate" &&
    typeof (preview as { suggestedPhaseId?: unknown }).suggestedPhaseId === "string" &&
    typeof (preview as { title?: unknown }).title === "string" &&
    typeof (preview as { goal?: unknown }).goal === "string" &&
    Array.isArray((preview as { scopePreview?: unknown }).scopePreview) &&
    typeof (preview as { reason?: unknown }).reason === "string"
  ) {
    return {
      kind: "candidate",
      suggestedPhaseId: (preview as { suggestedPhaseId: string }).suggestedPhaseId,
      title: (preview as { title: string }).title,
      goal: (preview as { goal: string }).goal,
      scopePreview: (preview as { scopePreview: unknown[] }).scopePreview.filter((item): item is string => typeof item === "string"),
      reason: (preview as { reason: string }).reason,
    };
  }
  if (
    typeof preview === "object" &&
    preview !== null &&
    (preview as { kind?: unknown }).kind === "none" &&
    typeof (preview as { reason?: unknown }).reason === "string"
  ) {
    return {
      kind: "none",
      reason: (preview as { reason: string }).reason,
    };
  }
  return { kind: "none", reason: "BrainstormContract has no phasePlan.nextPhasePreview." };
}

async function isGitRepo(projectRoot: string): Promise<boolean> {
  try {
    await git(["rev-parse", "--is-inside-work-tree"], projectRoot);
    return true;
  } catch {
    return false;
  }
}

async function git(args: string[], cwd: string): Promise<string> {
  const result = await execFileAsync("git", args, { cwd, maxBuffer: 20 * 1024 * 1024 });
  return result.stdout;
}

async function gitDiffForDeclaredFile(projectRoot: string, filePath: string): Promise<string> {
  const tracked = await git(["ls-files", "--error-unmatch", "--", filePath], projectRoot)
    .then(() => true)
    .catch(() => false);
  if (tracked) {
    return git(["diff", "HEAD", "--", filePath], projectRoot).catch(() => "");
  }
  if (!(await pathExists(path.join(projectRoot, filePath)))) {
    return "";
  }
  try {
    const result = await execFileAsync("git", ["diff", "--no-index", "--", "/dev/null", filePath], {
      cwd: projectRoot,
      maxBuffer: 20 * 1024 * 1024,
    });
    return result.stdout;
  } catch (error) {
    if (typeof error === "object" && error !== null && "stdout" in error && typeof (error as { stdout?: unknown }).stdout === "string") {
      return (error as { stdout: string }).stdout;
    }
    return "";
  }
}

function sanitizePath(filePath: string): string {
  return filePath.replace(/[^a-zA-Z0-9._-]+/g, "_");
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
