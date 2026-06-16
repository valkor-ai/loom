import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import { ZodError } from "zod";
import {
  brainstormNotFound,
  brainstormNotReady,
  invalidArgument,
  LoomError,
  stateCorrupted,
  stateNotInitialized,
} from "../errors";
import {
  type DeliveryIndex,
  type DeliveryIndexPhase,
  type BrainstormCandidate,
  brainstormContractSchema,
  brainstormCandidateSchema,
  deliveryIndexSchema,
  type BrainstormContract,
  type BrainstormStatus,
  type ClarificationAnswer,
  type RequirementInput,
  type RequirementSource,
  type RequirementSourceType,
} from "../schemas";
import { pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import {
  getLocatorForBrainstormRun,
  loadDeliveryIndex,
  loadProjectStatus,
  saveDeliveryIndex,
  saveProjectStatus,
  updatePhase,
  upsertStatusDelivery,
} from "../state/delivery";
import {
  brainstormContractPath,
  brainstormDecisionPath,
  brainstormDecisionsIndexPath,
  brainstormRequestCandidatePath,
  brainstormSessionRequestPath,
  brainstormLatestPath,
  confirmedFrontendExperienceTargetPath,
  currentFrontendExperienceTargetPath,
  deliveryConceptGlossaryPath,
  deliveryIndexPath,
  phaseConceptGroundingPath,
  getLoomPaths,
  toProjectRelative,
} from "../state/paths";
import { createRequirementContext, type RequirementContextResult } from "../requirements/context";
import { brainstormSessionAgentActionContract, type AgentActionContract } from "./agent-action";
import { referencedArtifactReadGuide, type ReferencedArtifactReadGuideEntry } from "./artifact-read-guide";
import { repairSubmitRouting } from "./repair-routing";
import { autoRunInstruction } from "./routing-instructions";
import {
  artifactGenerationProtocolPolicy,
  artifactRepairPolicy,
  brainstormAskUserInstructionPolicy,
  brainstormAskUserReadStep,
  compactContextReadStep,
} from "./output-policy";
import {
  businessLifecycleScanRules,
  businessScenarioConfirmationRules,
  businessObjectOperationCandidateRules,
  businessObjectOperationClarificationRules,
  brainstormCandidateSelfReviewRules,
  brainstormRequirementSemanticCompactRules,
  brainstormRequirementSemanticRules,
  confirmedBlockDetailRetentionRules,
  conceptGroundingSelfCheckRules,
  decisionImpactOrderingRules,
  finalSummaryReviewRules,
  frontendExperienceSelfCheckRules,
  frontendOperationPathCandidateRules,
  frontendOperationPathClarificationRules,
  nextPhasePreviewCandidateRules,
  phaseScopeOptionComparisonRules,
  phaseScopeSelfCheckRules,
  scopeItemCoverageCandidateRules,
  scopeItemCoverageClarificationRules,
} from "./brainstorm-rules";
import { writeRequestManifestAtomic } from "./request-manifest";

export type StartBrainstormInput = {
  projectRoot: string;
  requirementInput: RequirementInput;
};

export type AnswerBrainstormInput = {
  projectRoot: string;
  brainstormRunId: string;
  answerText: string;
  questionId?: string;
  selectedOptionIds?: string[];
  deliveryId?: string;
  phaseId?: string;
};

export type ConfirmBrainstormInput = {
  projectRoot: string;
  brainstormRunId: string;
  decision: "confirmed" | "revise";
  revisionText?: string;
  confirmationId?: string;
  deliveryId?: string;
  phaseId?: string;
};

export type BrainstormStatusInput = {
  projectRoot: string;
  brainstormRunId: string;
};

export type BrainstormAnswerResult = {
  brainstormRunId: string;
  contractId: string;
  status: BrainstormStatus;
  patches: BrainstormContract["clarification"]["patches"];
  confirmationRequests: BrainstormContract["clarification"]["confirmations"];
  contractPath: string;
};

export type BrainstormConfirmResult = {
  brainstormRunId: string;
  contractId: string;
  status: BrainstormStatus;
  handoff: BrainstormContract["handoff"];
  roadmap: BrainstormContract["roadmap"];
  nextActions: NonNullable<BrainstormContract["roadmap"]>["nextActions"];
  contractPath: string;
};

export type BrainstormStatusResult = {
  brainstormRunId: string;
  contractId: string;
  status: BrainstormStatus;
  summary: BrainstormContract["summary"];
  deliveryStrategy: BrainstormContract["deliveryStrategy"];
  pendingQuestions: BrainstormContract["clarification"]["questions"];
  pendingConfirmations: BrainstormContract["clarification"]["confirmations"];
  handoff: BrainstormContract["handoff"];
  contractPath: string;
};

export type BrainstormSessionRequest = {
  schemaVersion: "1.0";
  requestId: string;
  requestType: "brainstorm_session";
  agentAction: AgentActionContract;
  brainstormRunId: string;
  deliveryId: string;
  phaseId: string;
  originalRequest: {
    text: string;
    inputRefs: string[];
  };
  interactionMode: "agent_managed_conversation";
  generationProtocol: {
    readRequestBeforeActing: true;
    writeCandidateFileOnly: true;
    doNotWriteAcceptedArtifact: true;
    doNotModifyProjectFiles: true;
    ifBlockedWriteBlockedOutput: true;
    submitWithProvidedCommand: true;
  };
  contextRefs: {
    requestTextRef: string | null;
    originalRequirementContextRef: string;
    requirementContextRef: string;
    normalizedRequirementTextRef: string | null;
    keywordHintsRef: string | null;
    latestConfirmedRequirementDecisionRef?: string | null;
    confirmedRequirementDecisionsIndexRef?: string | null;
    sourceRefs: string[];
  };
  sourceFieldAccessHints: {
    requirementContextInput: {
      sourceItemsSelector: ".sourceItems[]";
      itemIdField: "itemId";
      kindField: "kind";
      originField: "origin";
      originalPathField: "path";
      extractedTextRefField: "extractedTextRef";
    };
    candidateOutput: {
      sourcesSelector: ".sources[]";
      sourceIdField: "sourceId";
      typeField: "type";
      pathField: "path";
      titleField: "title";
      textDigestField: "textDigest";
    };
    mappingRules: string[];
  };
  referencedArtifactReadGuide: ReferencedArtifactReadGuideEntry[];
  keywordHintsPolicy: {
    status: "advisory_only";
    mustNotTreatAsScope: true;
    mustNotTreatAsAcceptance: true;
    mustNotTreatAsConfirmedConcept: true;
    mayUseForClarificationQuestions: true;
    mayUseForConceptGroundingCandidates: true;
    ignoreWhenIrrelevant: true;
  };
  clarificationGuidance: Record<string, unknown>;
  conceptGroundingRequest: Record<string, unknown>;
  firstClarificationGate: {
    required: true;
    initialUserRequestDoesNotCountAsConfirmation: true;
    mustPresentBeforeAccept: string[];
    confirmationMustOccurAfterPresentation: true;
  };
  clarificationConversationProtocol: {
    mode: "progressive_blocks";
    oneTopicPerTurn: true;
    maxOptionsPerQuestion: number;
    avoidSchemaLanguageToUser: true;
    requiredBlocks: string[];
    frontendBlockRequiredWhen: string[];
    blockExecutionRules: string[];
    blockConfirmationRules: Record<string, string>;
  };
  riskGuidance: Record<string, unknown>;
  confirmationRules: Record<string, unknown>;
  rules: Record<string, unknown>;
  enumRefs: Record<string, string[]>;
  outputContract: {
    format: "json";
    schemaRef: "brainstorm-candidate-v1";
    candidateFile: string;
    schemaShape: Record<string, unknown>;
  };
  blockedOutput: {
    schemaRef: "brainstorm-candidate-blocked-v1";
    candidateFile: string;
    schemaShape: Record<string, unknown>;
  };
  submitCommand: {
    name: "brainstorm accept";
    argv: string[];
  };
  createdAt: string;
};

export type BrainstormStartResult = {
  deliveryId: string;
  phaseId: string;
  brainstormRunId: string;
  requestId: string;
  requestType: "brainstorm_session";
  requestPath: string;
  request: BrainstormSessionRequest;
};

const FIRST_CLARIFICATION_PRESENTED_ITEMS = [
  "currentPhaseScopeSummary",
  "includedDeferredExcludedBoundary",
  "nextPhasePreview",
  "conceptSummary",
  "businessObjectOperationSummary",
];

const PROGRESSIVE_CLARIFICATION_BLOCKS: Array<"phase_scope" | "concept_grounding" | "frontend_experience" | "final_summary"> = [
  "phase_scope",
  "concept_grounding",
  "frontend_experience",
  "final_summary",
];

export type AcceptBrainstormCandidateInput = {
  projectRoot: string;
  deliveryId?: string;
  phaseId?: string;
  brainstormRunId?: string;
  requestId?: string;
  candidateFile: string;
};

export type BrainstormAcceptResult = {
  accepted: boolean;
  deliveryId: string;
  phaseId: string;
  brainstormRunId: string;
  contractId: string | null;
  status: BrainstormStatus | "needs_clarification" | "blocked";
  issues: Array<{ code: string; path: string; message: string }>;
  contractPath: string | null;
  routeDecision: {
    type: "technical_baseline_request" | "brainstorm_clarification" | "blocked";
    reason: string;
  };
  repairInstruction?: Record<string, unknown>;
  instruction?: Record<string, unknown>;
};

export async function startBrainstorm(input: StartBrainstormInput): Promise<BrainstormStartResult> {
  const paths = await requireInitialized(input.projectRoot);
  const now = new Date().toISOString();
  const deliveryId = createId("delivery");
  const phaseId = "phase-1";
  const brainstormRunId = createId("bs");
  const contractId = createId("bc");
  const sourceCount = input.requirementInput.requestSources.length + input.requirementInput.contextSources.length;
  const mode = shouldUseRoadmap(input.requirementInput) ? "roadmap" : "single_phase";
  const title = inferTitle(input.requirementInput);
  const sources = buildSources(input.requirementInput);

  const contract: BrainstormContract = {
    schemaVersion: "1.0",
    contractId,
    brainstormRunId,
    status: "needs_clarification",
    sources,
    summary: {
      title,
      oneLine: input.requirementInput.primaryRequest,
      businessGoal: input.requirementInput.primaryRequest,
      complexity: mode === "roadmap" ? "large" : sourceCount > 1 ? "medium" : "unknown",
    },
    domainModel: {
      actors: [],
      capabilityGroups: [
        {
          id: "cap-primary-request",
          name: "待确认核心能力",
          description: "从需求输入中识别出的初始能力域，后续由 PlanningGenerationContract 继续细化。",
        },
      ],
      businessFlows: [],
    },
    scope: {
      included: [],
      deferred: [],
      excluded: [],
      assumptions: [],
    },
    acceptance: {
      candidates: [],
      coverageNotes: [],
    },
    deliveryStrategy: {
      mode,
      reason: mode === "roadmap"
        ? "需求输入较大或包含文件/多来源上下文，需要 Agent 与用户确认 roadmap 和当前 Phase 范围。"
        : "当前输入规模较小，但仍由 Agent 与用户确认结构化范围后再进入规划。",
      recommendedCurrentPhaseId: "phase-1",
    },
    deliveryContext: {
      originalRequest: {
        text: input.requirementInput.primaryRequest,
        inputRefs: sources.map((source) => source.sourceId),
      },
      initialSummary: {
        title,
        oneLine: input.requirementInput.primaryRequest,
        businessGoal: input.requirementInput.primaryRequest,
        complexity: mode === "roadmap" ? "large" : sourceCount > 1 ? "medium" : "unknown",
      },
    },
    clarification: emptyClarification(),
    roadmap: buildInitialRoadmap(title, now),
    phasePlan: {
      current: {
        phaseId,
        title: "Phase 1",
        goal: input.requirementInput.primaryRequest,
        scopeRefs: ["scope-initial-request"],
        acceptanceRefs: [],
        status: "scope_confirmed",
      },
      nextPhasePreview: {
        kind: "candidate",
        suggestedPhaseId: "phase-2",
        title: "Next phase",
        goal: "Confirm the next phase after the current phase is reviewed.",
        scopePreview: [],
        reason: "Initial placeholder. Agent must replace it in BrainstormCandidate based on user-confirmed scope.",
      },
    },
    handoff: {
      ready: false,
      nextNode: "planning_generation_contract",
      blockingReasons: ["Agent-managed BrainstormCandidate must be accepted before planning."],
      confirmedAt: null,
    },
    createdAt: now,
    updatedAt: now,
  };

  await saveBrainstormContract(paths.root, contract, deliveryId);
  await writeLatestPointer(paths.root, deliveryId, brainstormRunId);
  await createDeliveryIndex(paths.root, deliveryId, phaseId, title, contract, now);
  await updateProjectStatusForActiveDelivery(paths.root, deliveryId, phaseId, title, "brainstorming", {
    type: "brainstorm_clarification",
    source: "brainstorm_contract",
    deliveryId,
    phaseId,
    ref: toProjectRelative(paths.root, brainstormContractPath(paths.root, deliveryId)),
    reason: "BRAINSTORM_SESSION_REQUEST_CREATED",
  }, now);

  const requestId = createId("brainstorm-session");
  const requirementContext = await createRequirementContext({
    projectRoot: paths.root,
    deliveryId,
    requirementInput: input.requirementInput,
    createdAt: now,
  });
  const request = buildBrainstormSessionRequest({
    projectRoot: paths.root,
    deliveryId,
    phaseId,
    brainstormRunId,
    requestId,
    originalText: input.requirementInput.primaryRequest,
    sources,
    requirementContext,
    now,
  });
  const requestPath = brainstormSessionRequestPath(paths.root, deliveryId, requestId);
  await writeRequestManifestAtomic(paths.root, requestPath, request);
  await attachBrainstormRequestRefs(paths.root, deliveryId, phaseId, {
    brainstormRequest: toProjectRelative(paths.root, requestPath),
    brainstormRequestId: requestId,
    brainstormRunId,
    brainstormCandidateFile: request.outputContract.candidateFile,
    requirementContextRef: requirementContext.contextRef,
    normalizedRequirementTextRef: requirementContext.normalizedTextRef,
    keywordHintsRef: requirementContext.keywordHintsRef,
  });

  return {
    deliveryId,
    phaseId,
    brainstormRunId,
    requestId,
    requestType: "brainstorm_session",
    requestPath: toProjectRelative(paths.root, requestPath),
    request,
  };
}

export async function answerBrainstorm(input: AnswerBrainstormInput): Promise<BrainstormAnswerResult> {
  if (!input.answerText.trim()) {
    throw invalidArgument("Brainstorm answer requires non-empty answer text.");
  }

  const paths = await requireInitialized(input.projectRoot);
  const contract = await loadBrainstormContract(paths.root, input.brainstormRunId);
  const now = new Date().toISOString();
  const question = input.questionId
    ? contract.clarification.questions.find((item) => item.questionId === input.questionId)
    : pendingQuestions(contract)[0];

  if (!question) {
    throw brainstormNotReady("Brainstorm has no pending clarification question.", {
      brainstormRunId: input.brainstormRunId,
      status: contract.status,
    });
  }

  const answerId = `ans-${pad(contract.clarification.answers.length + 1)}`;
  const patchId = `patch-${pad(contract.clarification.patches.length + 1)}`;
  const confirmationId = `confirm-${pad(contract.clarification.confirmations.length + 1)}`;
  const answer: ClarificationAnswer = {
    answerId,
    turnId: question.turnId,
    questionId: question.questionId,
    answeredAt: now,
    answerText: input.answerText.trim(),
    selectedOptionIds: input.selectedOptionIds ?? inferSelectedOptions(input.answerText, question),
    freeformText: input.answerText.trim(),
    source: "user",
  };
  const scopeId = `scope-user-${shortHash(input.answerText)}`;
  const deferredScopeId = `scope-deferred-${shortHash(`${input.answerText}:deferred`)}`;
  const interpretedScope = interpretScopeAnswer(input.answerText);
  const operations: BrainstormContract["clarification"]["patches"][number]["operations"] = [
    {
      op: "add",
      path: "/scope/included",
      value: {
        id: scopeId,
        label: truncateText(interpretedScope.includedItems.join("、") || input.answerText.trim(), 80),
        items: interpretedScope.includedItems.length > 0 ? interpretedScope.includedItems : [input.answerText.trim()],
        source: "user_confirmed",
        reason: "来自用户澄清回答，等待确认后生效。",
      },
    },
  ];

  if (interpretedScope.deferredItems.length > 0) {
    operations.push({
      op: "add",
      path: "/scope/deferred",
      value: {
        id: deferredScopeId,
        label: truncateText(interpretedScope.deferredItems.join("、"), 80),
        items: interpretedScope.deferredItems,
        reason: "用户明确这些范围后续阶段再做。",
        source: "user_confirmed",
      },
    });
  } else {
    operations.push({
      op: "add",
      path: "/scope/deferred",
      value: {
        id: deferredScopeId,
        label: "未进入当前阶段的其余能力",
        reason: "当前回答只确认当前 Phase 候选范围，其余需求延迟到后续阶段继续确认。",
        source: "model_recommended",
      },
    });
  }

  const patch = {
    patchId,
    turnId: question.turnId,
    answerId,
    target: "scope" as const,
    status: "needs_confirmation" as const,
    summary: `用户回答被解释为当前阶段范围候选：${truncateText(input.answerText.trim(), 120)}`,
    operations,
    confidence: "medium" as const,
    needsUserConfirmation: true,
  };
  const confirmation = {
    confirmationId,
    turnId: question.turnId,
    patchIds: [patchId],
    message: `我理解当前阶段先按“${truncateText(input.answerText.trim(), 80)}”推进，其余范围后续再确认。这样理解对吗？`,
    confirmOptions: [
      { id: "yes" as const, label: "对，按这个来" },
      { id: "revise" as const, label: "需要调整" },
    ],
    allowFreeformRevision: true,
    status: "pending" as const,
  };

  question.status = "answered";
  contract.clarification.answers.push(answer);
  contract.clarification.patches.push(patch);
  contract.clarification.confirmations.push(confirmation);
  contract.clarification.pendingQuestionIds = contract.clarification.pendingQuestionIds.filter(
    (id) => id !== question.questionId,
  );
  contract.clarification.pendingConfirmationIds.push(confirmationId);
  contract.clarification.status = "needs_confirmation";
  contract.status = "needs_confirmation";
  contract.updatedAt = now;

  const turn = contract.clarification.turns.find((item) => item.turnId === question.turnId);
  if (turn) {
    if (!turn.answers.includes(answerId)) {
      turn.answers.push(answerId);
    }
    if (!turn.patches.includes(patchId)) {
      turn.patches.push(patchId);
    }
    if (!turn.confirmations.includes(confirmationId)) {
      turn.confirmations.push(confirmationId);
    }
    turn.status = "needs_confirmation";
  }

  const { deliveryId } = await resolveBrainstormLocator(paths.root, contract.brainstormRunId, input.deliveryId, input.phaseId);
  await saveBrainstormContract(paths.root, contract, deliveryId);

  return {
    brainstormRunId: contract.brainstormRunId,
    contractId: contract.contractId,
    status: contract.status,
    patches: contract.clarification.patches.filter((item) => item.status === "needs_confirmation"),
    confirmationRequests: pendingConfirmations(contract),
    contractPath: toProjectRelative(paths.root, brainstormContractPath(paths.root, deliveryId)),
  };
}

export async function acceptBrainstormCandidate(input: AcceptBrainstormCandidateInput): Promise<BrainstormAcceptResult> {
  if (!input.candidateFile.trim()) {
    throw invalidArgument("brainstorm accept requires --candidate-file.");
  }
  const paths = await requireInitialized(input.projectRoot);
  const root = paths.root;
  const candidatePath = resolveCliPath(root, input.candidateFile);
  const rawCandidate = await readJsonFileForCandidate(candidatePath);
  const now = new Date().toISOString();
  const parsed = brainstormCandidateSchema.safeParse(normalizeBrainstormCandidateForAccept(rawCandidate, now));
  if (!parsed.success) {
    return invalidBrainstormAcceptResult(input, parsed.error.issues.map((issue) => ({
      code: "SCHEMA_INVALID",
      path: issue.path.join("."),
      message: issue.message,
    })));
  }

  const parsedCandidate = parsed.data;
  const deliveryId = input.deliveryId ?? parsedCandidate.deliveryId;
  if (deliveryId !== parsedCandidate.deliveryId) {
    throw invalidArgument("Brainstorm candidate deliveryId does not match command deliveryId.", {
      commandDeliveryId: input.deliveryId,
      candidateDeliveryId: parsedCandidate.deliveryId,
    });
  }
  const locator = await resolveBrainstormLocator(root, parsedCandidate.brainstormRunId, deliveryId, input.phaseId ?? parsedCandidate.phaseId);
  const existing = await loadBrainstormContract(root, parsedCandidate.brainstormRunId);
  const candidate = normalizeBrainstormCandidateRoadmapRequirementForAccept(
    normalizeBrainstormCandidateRoadmapForAccept(parsedCandidate, existing, locator.phaseId),
    locator.phaseId,
  );
  const requestIssues = await validateBrainstormAcceptRequest(root, {
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    brainstormRunId: candidate.brainstormRunId,
    commandBrainstormRunId: input.brainstormRunId,
    requestId: input.requestId,
    candidateFile: input.candidateFile,
  });
  const issues = [
    ...requestIssues,
    ...validateBrainstormCandidate(candidate, locator.deliveryId, locator.phaseId),
  ];
  if (issues.length > 0) {
    const instruction = brainstormAcceptFailureInstruction(root, {
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      brainstormRunId: candidate.brainstormRunId,
      requestId: input.requestId,
      candidateFile: input.candidateFile,
      issues,
    });
    return {
      accepted: false,
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      brainstormRunId: candidate.brainstormRunId,
      contractId: null,
      status: candidate.status,
      issues,
      contractPath: null,
      routeDecision: {
        type: candidate.status === "blocked" ? "blocked" : "brainstorm_clarification",
        reason: "BRAINSTORM_CANDIDATE_INVALID",
      },
      ...(instruction.mode === "repair_candidate" ? { repairInstruction: instruction } : { instruction }),
    };
  }

  if (candidate.status !== "confirmed") {
    const issues = [{
      code: "CANDIDATE_NOT_CONFIRMED",
      path: "status",
      message: "Only confirmed BrainstormCandidate can be accepted.",
    }];
    const instruction = candidate.status === "blocked"
      ? brainstormRepairInstruction(root, {
          deliveryId: locator.deliveryId,
          phaseId: locator.phaseId,
          brainstormRunId: candidate.brainstormRunId,
          requestId: input.requestId,
          candidateFile: input.candidateFile,
          issues,
        })
      : brainstormMissingClarificationInstruction(root, {
          deliveryId: locator.deliveryId,
          phaseId: locator.phaseId,
          brainstormRunId: candidate.brainstormRunId,
          requestId: input.requestId,
          candidateFile: input.candidateFile,
          issues,
        });
    return {
      accepted: false,
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      brainstormRunId: candidate.brainstormRunId,
      contractId: null,
      status: candidate.status,
      issues,
      contractPath: null,
      routeDecision: {
        type: candidate.status === "blocked" ? "blocked" : "brainstorm_clarification",
        reason: candidate.status === "blocked" ? "BRAINSTORM_BLOCKED" : "BRAINSTORM_NEEDS_CLARIFICATION",
      },
      ...(instruction.mode === "repair_candidate" ? { repairInstruction: instruction } : { instruction }),
    };
  }

  const derivedRefs = await writeBrainstormDerivedArtifacts(root, locator, candidate, now);
  const contract = brainstormContractSchema.parse({
    ...brainstormContractFromCandidate(existing, candidate, locator.phaseId, now),
    conceptGrounding: candidate.conceptGrounding,
    conceptConfirmation: candidate.conceptConfirmation,
    clarificationProgress: candidate.clarificationProgress,
    conceptGroundingRefs: derivedRefs.conceptGroundingRefs,
    frontendExperience: candidate.frontendExperience,
    frontendExperienceDelta: candidate.frontendExperienceDelta,
    frontendExperienceRefs: derivedRefs.frontendExperienceRefs,
  });
  await saveBrainstormContract(root, contract, locator.deliveryId);
  const decisionRefs = await writeBrainstormDecisionSnapshot(root, locator, contract, candidate, now, input.requestId);
  await writeLatestPointer(root, locator.deliveryId, candidate.brainstormRunId);
  await updateDeliveryAfterBrainstormConfirmation(root, locator.deliveryId, locator.phaseId, contract, now, decisionRefs);
  await updateProjectStatusAfterConfirmation(root, locator.deliveryId, locator.phaseId, contract.contractId, contract.summary.title, now);

  return {
    accepted: true,
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    brainstormRunId: candidate.brainstormRunId,
    contractId: contract.contractId,
    status: contract.status,
    issues: [],
    contractPath: toProjectRelative(root, brainstormContractPath(root, locator.deliveryId)),
    routeDecision: {
      type: "technical_baseline_request",
      reason: "BRAINSTORM_CANDIDATE_ACCEPTED",
    },
    instruction: autoRunInstruction({
      actionType: "technical_baseline_request",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      reason: "BRAINSTORM_CANDIDATE_ACCEPTED",
      targetNode: "technical_baseline_request",
      argv: ["technical-baseline", "request", "--delivery-id", locator.deliveryId, "--phase-id", locator.phaseId],
      userMessage: "BrainstormCandidate accepted. Continue immediately by creating TechnicalBaselineRequest.",
    }),
  };
}

function brainstormAcceptFailureInstruction(
  root: string,
  input: {
    deliveryId: string;
    phaseId: string;
    brainstormRunId: string;
    requestId?: string;
    candidateFile: string;
    issues: Array<{ code: string; path: string; message: string }>;
  },
): Record<string, unknown> {
  if (requiresBrainstormUserClarification(input.issues)) {
    return brainstormMissingClarificationInstruction(root, input);
  }
  return brainstormRepairInstruction(root, input);
}

function requiresBrainstormUserClarification(issues: Array<{ code: string; path: string; message: string }>): boolean {
  const userConfirmationIssueCodes = new Set([
    "USER_CONFIRMATION_MISSING",
    "CONFIRMATION_BASIS_MISSING",
    "INITIAL_REQUEST_CANNOT_CONFIRM",
    "SUMMARY_NOT_PRESENTED",
    "CONFIRMATION_NOT_AFTER_SUMMARY",
    "CONFIRMATION_PRESENTED_ITEM_MISSING",
    "CLARIFICATION_PROGRESS_MISSING",
    "FINAL_SUMMARY_NOT_CONFIRMED",
    "CLARIFICATION_BLOCK_MISSING",
    "CLARIFICATION_BLOCK_NOT_CONFIRMED",
    "CONCEPT_GROUNDING_MISSING",
    "CONCEPT_CONFIRMATION_MISSING",
    "CONCEPTS_NOT_SHOWN_TO_USER",
    "FRONTEND_BLOCK_UNRESOLVED",
    "FRONTEND_TARGET_MISSING",
  ]);
  return issues.some((issue) => userConfirmationIssueCodes.has(issue.code));
}

function brainstormMissingClarificationInstruction(
  root: string,
  input: {
    deliveryId: string;
    phaseId: string;
    brainstormRunId: string;
    requestId?: string;
    candidateFile: string;
    issues: Array<{ code: string; path: string; message: string }>;
  },
): Record<string, unknown> {
  const requestRef = input.requestId
    ? toProjectRelative(root, brainstormSessionRequestPath(root, input.deliveryId, input.requestId))
    : null;
  return {
    mode: "ask_user",
    ...brainstormAskUserInstructionPolicy(),
    requestRef,
    issues: input.issues,
    userMessage: "BrainstormCandidate is incomplete because one or more progressive clarification blocks have not been confirmed. Continue the Brainstorm conversation with the next missing block; do not repair JSON or rerun brainstorm accept until final_summary is explicitly confirmed.",
    expectedResponse: {
      kind: "brainstorm_progressive_clarification",
      rule: "Treat the failed accept as an early-submit signal, not a JSON repair task. Read requestRef through agentAction.read.fieldGroups, present the next missing Brainstorm block to the user, and continue the progressive conversation. Only after final_summary is explicitly confirmed should you read candidate_write_contract, write BrainstormCandidate, and run brainstorm accept.",
      requestReadRule: brainstormAskUserReadStep,
      requestRef,
      currentTurnAnswerRule: {
        consumeCurrentUserMessage: false,
        meaning: "The previous user confirmation was already consumed for an earlier Brainstorm block. Ask or present the next missing block instead of treating the earlier confirmation as final_summary confirmation.",
        doNotAskAgainWhenCurrentMessageIsExplicit: false,
        ifAmbiguousAskUser: true,
      },
    },
  };
}

export async function confirmBrainstorm(input: ConfirmBrainstormInput): Promise<BrainstormConfirmResult> {
  const paths = await requireInitialized(input.projectRoot);
  const contract = await loadBrainstormContract(paths.root, input.brainstormRunId);
  const now = new Date().toISOString();
  const confirmation = input.confirmationId
    ? contract.clarification.confirmations.find((item) => item.confirmationId === input.confirmationId)
    : pendingConfirmations(contract)[0];

  if (!confirmation) {
    throw brainstormNotReady("Brainstorm has no pending confirmation request.", {
      brainstormRunId: input.brainstormRunId,
      status: contract.status,
    });
  }

  if (input.decision === "revise") {
    const revisionText = input.revisionText?.trim();
    if (!revisionText) {
      throw invalidArgument("Revision confirmation requires --revision or confirmationFile.revisionText.");
    }

    confirmation.status = "revise";
    contract.clarification.pendingConfirmationIds = contract.clarification.pendingConfirmationIds.filter(
      (id) => id !== confirmation.confirmationId,
    );
    const turnId = `turn-${pad(contract.clarification.turns.length + 1)}`;
    const questionId = `q-revision-${pad(contract.clarification.questions.length + 1)}`;
    const answerId = `ans-${pad(contract.clarification.answers.length + 1)}`;
    const patchId = `patch-${pad(contract.clarification.patches.length + 1)}`;
    const nextConfirmationId = `confirm-${pad(contract.clarification.confirmations.length + 1)}`;
    const interpretedScope = interpretScopeAnswer(revisionText);
    const scopeId = `scope-user-${shortHash(revisionText)}`;
    const deferredScopeId = `scope-deferred-${shortHash(`${revisionText}:deferred`)}`;
    const operations: BrainstormContract["clarification"]["patches"][number]["operations"] = [
      {
        op: "add",
        path: "/scope/included",
        value: {
          id: scopeId,
          label: truncateText(interpretedScope.includedItems.join("、") || revisionText, 80),
          items: interpretedScope.includedItems.length > 0 ? interpretedScope.includedItems : [revisionText],
          source: "user_confirmed",
          reason: "来自用户确认修订文本，等待再次确认后生效。",
        },
      },
    ];
    if (interpretedScope.deferredItems.length > 0) {
      operations.push({
        op: "add",
        path: "/scope/deferred",
        value: {
          id: deferredScopeId,
          label: truncateText(interpretedScope.deferredItems.join("、"), 80),
          items: interpretedScope.deferredItems,
          reason: "用户在修订中明确这些范围后续阶段再做。",
          source: "user_confirmed",
        },
      });
    }

    contract.clarification.turns.push({
      turnId,
      startedAt: now,
      completedAt: null,
      reason: "用户要求调整 Brainstorm patch，需要重新确认范围。",
      questions: [questionId],
      answers: [answerId],
      patches: [patchId],
      confirmations: [nextConfirmationId],
      status: "needs_confirmation",
    });
    contract.clarification.questions.push({
      questionId,
      turnId,
      type: "free_text",
      severity: "blocking",
      question: "请补充你希望如何调整当前阶段范围。",
      whyAsked: "用户没有确认上一轮 patch，必须重新解释为新的 patch 后再确认。",
      suggestedOptions: [],
      allowFreeform: true,
      freeformHint: "可以直接说明新增、移除或延后的范围。",
      status: "answered",
    });
    contract.clarification.answers.push({
      answerId,
      turnId,
      questionId,
      answeredAt: now,
      answerText: revisionText,
      selectedOptionIds: [],
      freeformText: revisionText,
      source: "user",
    });
    contract.clarification.patches.push({
      patchId,
      turnId,
      answerId,
      target: "scope",
      status: "needs_confirmation",
      summary: `用户修订被解释为当前阶段范围候选：${truncateText(revisionText, 120)}`,
      operations,
      confidence: "medium",
      needsUserConfirmation: true,
    });
    contract.clarification.confirmations.push({
      confirmationId: nextConfirmationId,
      turnId,
      patchIds: [patchId],
      message: `我已按你的修订理解为“${truncateText(revisionText, 80)}”。这样理解对吗？`,
      confirmOptions: [
        { id: "yes", label: "对，按这个来" },
        { id: "revise", label: "需要调整" },
      ],
      allowFreeformRevision: true,
      status: "pending",
    });
    contract.clarification.pendingQuestionIds = contract.clarification.pendingQuestionIds.filter(
      (id) => id !== questionId,
    );
    contract.clarification.pendingConfirmationIds.push(nextConfirmationId);
    contract.clarification.status = "needs_confirmation";
    contract.status = "needs_confirmation";
    contract.updatedAt = now;
    const { deliveryId } = await resolveBrainstormLocator(paths.root, contract.brainstormRunId, input.deliveryId, input.phaseId);
    await saveBrainstormContract(paths.root, contract, deliveryId);

    return {
      brainstormRunId: contract.brainstormRunId,
      contractId: contract.contractId,
      status: contract.status,
      handoff: contract.handoff,
      roadmap: contract.roadmap,
      nextActions: contract.roadmap?.nextActions ?? [],
      contractPath: toProjectRelative(paths.root, brainstormContractPath(paths.root, deliveryId)),
    };
  }

  confirmation.status = "confirmed";
  contract.clarification.pendingConfirmationIds = contract.clarification.pendingConfirmationIds.filter(
    (id) => id !== confirmation.confirmationId,
  );

  const appliedPatchIds: string[] = [];
  const appliedScopeRefs = {
    included: [] as string[],
    deferred: [] as string[],
    excluded: [] as string[],
  };
  for (const patchId of confirmation.patchIds) {
    const patch = contract.clarification.patches.find((item) => item.patchId === patchId);
    if (!patch || patch.status !== "needs_confirmation") {
      continue;
    }
    const scopeRefs = applyPatchOperations(contract, patch.operations);
    appliedScopeRefs.included.push(...scopeRefs.included);
    appliedScopeRefs.deferred.push(...scopeRefs.deferred);
    appliedScopeRefs.excluded.push(...scopeRefs.excluded);
    patch.status = "applied";
    appliedPatchIds.push(patch.patchId);
  }

  const { deliveryId, phaseId } = await resolveBrainstormLocator(paths.root, contract.brainstormRunId, input.deliveryId, input.phaseId);
  if (contract.roadmap) {
    contract.roadmap.currentPhaseId = phaseId;
  }

  if (contract.roadmap?.currentPhaseId) {
    const currentPhase = contract.roadmap.phases.find(
      (phase) => phase.phaseId === contract.roadmap?.currentPhaseId,
    );
    if (currentPhase) {
      currentPhase.status = "scope_confirmed";
      currentPhase.scope.includedRefs = uniqueStrings([
        ...currentPhase.scope.includedRefs,
        ...appliedScopeRefs.included,
      ]);
      currentPhase.scope.deferredRefs = uniqueStrings([
        ...currentPhase.scope.deferredRefs,
        ...appliedScopeRefs.deferred,
      ]);
      currentPhase.scope.excludedRefs = uniqueStrings([
        ...currentPhase.scope.excludedRefs,
        ...appliedScopeRefs.excluded,
      ]);
      currentPhase.handoff.readyForPlanning = true;
      currentPhase.handoff.planningContractId = `pgc-${currentPhase.phaseId}`;
      currentPhase.confirmation = {
        confirmedBy: "user",
        confirmedAt: now,
        sourcePatchIds: appliedPatchIds,
      };
      currentPhase.nextActions = [
        {
          actionId: "continue-planning",
          type: "continue_planning",
          label: "进入当前阶段规划",
          recommended: true,
          phaseId: currentPhase.phaseId,
          userPrompt: "当前阶段范围已确认，可以继续生成规划契约。",
        },
      ];
    }
    contract.roadmap.status = "active";
    contract.roadmap.nextActions = buildRoadmapNextActions(contract.roadmap.phases);
  }

  const turn = contract.clarification.turns.find((item) => item.turnId === confirmation.turnId);
  if (turn) {
    turn.completedAt = now;
    turn.status = "confirmed";
  }

  contract.clarification.status = "confirmed";
  contract.status = "confirmed";
  contract.handoff = {
    ready: true,
    nextNode: "planning_generation_contract",
    blockingReasons: [],
    confirmedAt: now,
  };
  contract.updatedAt = now;
  await saveBrainstormContract(paths.root, contract, deliveryId);
  await updateDeliveryAfterBrainstormConfirmation(paths.root, deliveryId, phaseId, contract, now);
  await updateProjectStatusAfterConfirmation(paths.root, deliveryId, phaseId, contract.contractId, contract.summary.title, now);

  return {
    brainstormRunId: contract.brainstormRunId,
    contractId: contract.contractId,
    status: contract.status,
    handoff: contract.handoff,
    roadmap: contract.roadmap,
    nextActions: contract.roadmap?.nextActions ?? [],
    contractPath: toProjectRelative(paths.root, brainstormContractPath(paths.root, deliveryId)),
  };
}

export async function getBrainstormStatus(input: BrainstormStatusInput): Promise<BrainstormStatusResult> {
  const paths = await requireInitialized(input.projectRoot);
  const contract = await loadBrainstormContract(paths.root, input.brainstormRunId);

  return {
    brainstormRunId: contract.brainstormRunId,
    contractId: contract.contractId,
    status: contract.status,
    summary: contract.summary,
    deliveryStrategy: contract.deliveryStrategy,
    pendingQuestions: pendingQuestions(contract),
    pendingConfirmations: pendingConfirmations(contract),
    handoff: contract.handoff,
    contractPath: toProjectRelative(paths.root, await contractPathFor(paths.root, contract.brainstormRunId)),
  };
}

async function requireInitialized(projectRoot: string): Promise<ReturnType<typeof getLoomPaths>> {
  const paths = getLoomPaths(projectRoot);
  if (!(await pathExists(paths.configFile)) || !(await pathExists(paths.statusFile))) {
    throw stateNotInitialized(paths.root);
  }
  return paths;
}

async function resolveBrainstormLocator(
  projectRoot: string,
  brainstormRunId: string,
  deliveryId?: string,
  phaseId?: string,
): Promise<{ deliveryId: string; phaseId: string }> {
  const locator = await getLocatorForBrainstormRun(projectRoot, brainstormRunId);
  return {
    deliveryId: deliveryId ?? locator.deliveryId,
    phaseId: phaseId ?? locator.phaseId,
  };
}

function buildInitialClarification(
  isRoadmap: boolean,
  turnId: string,
  questionId: string,
  now: string,
  initialScopeLabel: string,
): BrainstormContract["clarification"] {
  if (!isRoadmap) {
    const patchId = "patch-001";
    const confirmationId = "confirm-001";
    return {
      status: "needs_confirmation",
      turns: [
        {
          turnId,
          startedAt: now,
          completedAt: null,
          reason: "单阶段需求需要用户确认后才能进入规划。",
          questions: [],
          answers: [],
          patches: [patchId],
          confirmations: [confirmationId],
          status: "needs_confirmation",
        },
      ],
      questions: [],
      answers: [],
      patches: [
        {
          patchId,
          turnId,
          answerId: null,
          target: "scope",
          status: "needs_confirmation",
          summary: `将初始需求确认为当前阶段范围：${initialScopeLabel}`,
          operations: [
            {
              op: "add",
              path: "/scope/included",
              value: {
                id: "scope-single-phase-confirmed",
                label: initialScopeLabel,
                source: "user_confirmed",
                reason: "单阶段需求经用户确认后作为当前规划范围。",
              },
            },
          ],
          confidence: "medium",
          needsUserConfirmation: true,
        },
      ],
      confirmations: [
        {
          confirmationId,
          turnId,
          patchIds: [patchId],
          message: `我理解当前阶段就按“${initialScopeLabel}”进入规划。这样理解对吗？`,
          confirmOptions: [
            { id: "yes", label: "对，按这个来" },
            { id: "revise", label: "需要调整" },
          ],
          allowFreeformRevision: true,
          status: "pending",
        },
      ],
      pendingQuestionIds: [],
      pendingConfirmationIds: [confirmationId],
    };
  }

  return {
    status: "needs_answer",
    turns: [
      {
        turnId,
        startedAt: now,
        completedAt: null,
        reason: "需要确认 roadmap 当前阶段范围。",
        questions: [questionId],
        answers: [],
        patches: [],
        confirmations: [],
        status: "needs_answer",
      },
    ],
    questions: [
      {
        questionId,
        turnId,
        type: "open_choice",
        severity: "blocking",
        question: "第一阶段你希望优先交付什么？",
        whyAsked: "需求包含较多内容，单阶段交付风险较高，需要确认当前 Phase 范围。",
        suggestedOptions: [
          {
            optionId: "vertical-core-loop",
            label: "可运行核心闭环",
            description: "先完成最小可运行业务闭环，后续再补后台、统计、发布等扩展能力。",
            recommended: true,
          },
          {
            optionId: "roadmap-only",
            label: "只确认 roadmap",
            description: "先不进入实现，只确认阶段划分和范围。",
          },
          {
            optionId: "custom-phase",
            label: "自定义当前阶段",
            description: "直接描述你希望当前阶段包含和暂缓的范围。",
          },
        ],
        allowFreeform: true,
        freeformHint: "可以直接描述你自己的第一阶段范围，或组合多个选项。",
        defaultIfSkipped: {
          optionId: "vertical-core-loop",
          assumptionText: "如果用户未明确选择，默认建议第一阶段做可运行核心闭环，但必须标记为 assumption。",
        },
        status: "pending",
      },
    ],
    answers: [],
    patches: [],
    confirmations: [],
    pendingQuestionIds: [questionId],
    pendingConfirmationIds: [],
  };
}

function buildInitialRoadmap(title: string, now: string): NonNullable<BrainstormContract["roadmap"]> {
  const phase1 = {
    phaseId: "phase-1",
    name: "可运行核心闭环",
    status: "scope_confirming" as const,
    goal: `先完成 ${title} 的最小可运行核心闭环。`,
    scope: {
      includedRefs: ["scope-initial-request"],
      deferredRefs: [],
      excludedRefs: [],
    },
    acceptanceRefs: ["AC-001"],
    dependsOn: [],
    handoff: {
      readyForPlanning: false,
      planningContractId: null,
      planId: null,
    },
    confirmation: {
      confirmedBy: null,
      confirmedAt: null,
      sourcePatchIds: [],
    },
    nextActions: [],
  };

  return {
    roadmapId: createId("rm"),
    status: "needs_confirmation",
    strategy: "multi_phase",
    reason: "需求规模建议按 roadmap 粗确认、当前 Phase 细确认的方式推进。",
    currentPhaseId: "phase-1",
    recommendedPhaseId: "phase-1",
    phases: [
      phase1,
      {
        phaseId: "phase-2",
        name: "能力完善与边界补齐",
        status: "proposed",
        goal: "在核心闭环可运行后，补齐后续业务能力、管理能力和体验细节。",
        scope: {
          includedRefs: [],
          deferredRefs: [],
          excludedRefs: [],
        },
        acceptanceRefs: [],
        dependsOn: ["phase-1"],
        handoff: {
          readyForPlanning: false,
          planningContractId: null,
          planId: null,
        },
        confirmation: {
          confirmedBy: null,
          confirmedAt: null,
          sourcePatchIds: [],
        },
        nextActions: [
          {
            actionId: "confirm-phase-2",
            type: "confirm_next_phase",
            label: "确认 Phase 2 范围",
            recommended: false,
            phaseId: "phase-2",
            userPrompt: `${now.slice(0, 10)} 后续可以继续确认 Phase 2 的具体范围。`,
          },
        ],
      },
    ],
    nextActions: [],
  };
}

function buildBrainstormSessionRequest(input: {
  projectRoot: string;
  deliveryId: string;
  phaseId: string;
  brainstormRunId: string;
  requestId: string;
  originalText: string;
  sources: RequirementSource[];
  requirementContext: RequirementContextResult;
  now: string;
}): BrainstormSessionRequest {
  const candidateFile = toProjectRelative(input.projectRoot, brainstormRequestCandidatePath(input.projectRoot, input.deliveryId, input.phaseId, input.requestId));
  const blockedFile = candidateFile.replace(/candidate\.json$/, "blocked.json");
  const requestTextRef = input.requirementContext.normalizedTextRef;
  const submitCommand: BrainstormSessionRequest["submitCommand"] = {
    name: "brainstorm accept",
    argv: [
      "brainstorm",
      "accept",
      "--delivery-id",
      input.deliveryId,
      "--phase-id",
      input.phaseId,
      "--request-id",
      input.requestId,
      "--run-id",
      input.brainstormRunId,
      "--candidate-file",
      "{candidateFile}",
    ],
  };
  return {
    schemaVersion: "1.0",
    requestId: input.requestId,
    requestType: "brainstorm_session",
    agentAction: brainstormSessionAgentActionContract({
      candidateFile,
      blockedFile,
      submitCommand,
    }),
    brainstormRunId: input.brainstormRunId,
    deliveryId: input.deliveryId,
    phaseId: input.phaseId,
    originalRequest: {
      text: input.originalText,
      inputRefs: input.sources.map((source) => source.sourceId),
    },
    interactionMode: "agent_managed_conversation",
    generationProtocol: {
      readRequestBeforeActing: true,
      writeCandidateFileOnly: true,
      doNotWriteAcceptedArtifact: true,
      doNotModifyProjectFiles: true,
      ifBlockedWriteBlockedOutput: true,
      submitWithProvidedCommand: true,
      ...artifactGenerationProtocolPolicy(),
    },
    contextRefs: {
      requestTextRef,
      originalRequirementContextRef: input.requirementContext.contextRef,
      requirementContextRef: input.requirementContext.contextRef,
      normalizedRequirementTextRef: input.requirementContext.normalizedTextRef,
      keywordHintsRef: input.requirementContext.keywordHintsRef,
      sourceRefs: input.sources.map((source) => source.path ?? source.sourceId),
    },
    sourceFieldAccessHints: {
      requirementContextInput: {
        sourceItemsSelector: ".sourceItems[]",
        itemIdField: "itemId",
        kindField: "kind",
        originField: "origin",
        originalPathField: "path",
        extractedTextRefField: "extractedTextRef",
      },
      candidateOutput: {
        sourcesSelector: ".sources[]",
        sourceIdField: "sourceId",
        typeField: "type",
        pathField: "path",
        titleField: "title",
        textDigestField: "textDigest",
      },
      mappingRules: [
        "When reading contextRefs.requirementContextRef, source records live under sourceItems[] and use itemId/kind; do not query sourceId/type there.",
        "When writing BrainstormCandidate.sources[], create sourceId/type fields from the sourceItems[] facts and enumRefs.requirementSourceType.",
        "Map RequirementContext sourceItems[].kind=file with mimeType application/pdf to candidate sources[].type=pdf.",
        "Map direct user/request text source items to candidate sources[].type=user_text or text based on the source origin.",
        "If a jq selector returns null for sourceId/type while reading RequirementContext, switch to sourceFieldAccessHints.requirementContextInput instead of probing guessed paths.",
      ],
    },
    referencedArtifactReadGuide: referencedArtifactReadGuide({
      requestTextRef,
      originalRequirementContextRef: input.requirementContext.contextRef,
      requirementContextRef: input.requirementContext.contextRef,
      normalizedRequirementTextRef: input.requirementContext.normalizedTextRef,
      keywordHintsRef: input.requirementContext.keywordHintsRef,
    }),
    keywordHintsPolicy: {
      status: "advisory_only",
      mustNotTreatAsScope: true,
      mustNotTreatAsAcceptance: true,
      mustNotTreatAsConfirmedConcept: true,
      mayUseForClarificationQuestions: true,
      mayUseForConceptGroundingCandidates: true,
      ignoreWhenIrrelevant: true,
    },
    clarificationGuidance: {
      choiceFirstClarification: true,
      preferredOptionCount: { min: 3, max: 5 },
      eachOptionMustExplainImpact: true,
      allowFreeformAlternative: true,
      avoidOpenEndedQuestionsWhenOptionsArePossible: true,
      askOneDecisionTopicPerTurn: true,
      doNotBundleUnrelatedDecisionTopics: true,
      askOnlyWhenNeeded: true,
      preferConcreteTradeoffQuestions: true,
      avoidLargeQuestionnaires: true,
      separateScopeBaselineAcceptance: true,
      confirmIncludedExcludedDeferredExplicitly: true,
      confirmPhase1ForRoadmap: true,
      phaseScopeOptionComparison: {
        requiredByDefault: true,
        nextPhaseSeedIsPreselectedAnswer: false,
        optionCount: { min: 2, max: 3 },
        recommendationRequired: true,
        recommendationMustPreserveCoreCurrentPhaseOutcome: true,
        narrowerOptionCannotBeRecommendedWhenDefersExplicitSeedItems: true,
        scopeReductionRequiresExplicitUserConfirmation: true,
        atomicSingleScopeException: {
          allowedOnlyWhenNoSourceGroundedAlternativeCutExists: true,
          mustExplainMissingAlternativeCuts: [
            "narrower cut",
            "broader adjacent-workflow cut",
            "dependency or ordering cut",
            "deferred lifecycle action",
            "alternate UI or runtime boundary",
          ],
          disallowedWhenMultipleScopePreviewItemsOrLifecycleActionsExist: true,
        },
      },
    },
    conceptGroundingRequest: {
      stage: input.phaseId === "phase-1" ? "initial_delivery" : "phase_continuation",
      mustProduceDeliveryConceptGlossary: input.phaseId === "phase-1",
      mustReuseDeliveryConceptGlossaryRef: input.phaseId !== "phase-1",
      mustProducePhaseConceptGrounding: true,
      mayProduceGlossaryUpdates: input.phaseId !== "phase-1",
      mustShowConceptsToUserBeforeAccept: true,
      mustNotTreatFutureConceptsAsCurrentScope: true,
      selectionGuidance: {
        deliveryGlossaryPurpose: "Delivery-wide high-risk concepts that may affect multiple phases or system-wide correctness.",
        phaseGroundingPurpose: "Current-phase high-risk concepts that must be understood before architecture and task planning.",
        preferConceptsAffecting: [
          "business_invariant",
          "state_transition",
          "resource_consistency",
          "permission_boundary",
          "external_contract",
          "scope_confusion_risk",
          "user_visible_flow",
          "runtime_or_delivery_semantics",
          "frontend_experience_semantics",
        ],
        includeConceptTypes: [
          "Abstract concepts summarized across multiple requirement sections.",
          "Concrete terms explicitly appearing in the source requirement.",
        ],
        extractionRules: [
          "For PDFs, long documents, or multi-phase systems, look across the whole requirement before choosing delivery-wide concepts.",
          "Explain low-frequency, domain-specific, or easy-to-overlook terms in common implementation language before coding begins.",
          "Rank concepts by phaseRelevance, priority, then attentionRank; put the highest-risk concepts first.",
        ],
        antiPatterns: [
          "Do not use only a generic project label as deliveryConceptGlossary.",
          "Do not restate the whole requirement as a concept.",
          "Do not treat future, deferred, or excluded concepts as current scope.",
        ],
      },
      userPresentationGuidance: {
        showConceptSummaryInPlainLanguage: true,
        askUserToConfirmOrCorrect: true,
        avoidSchemaLanguage: true,
      },
    },
    firstClarificationGate: {
      required: true,
      initialUserRequestDoesNotCountAsConfirmation: true,
      mustPresentBeforeAccept: FIRST_CLARIFICATION_PRESENTED_ITEMS,
      confirmationMustOccurAfterPresentation: true,
    },
    clarificationConversationProtocol: {
      mode: "progressive_blocks",
      oneTopicPerTurn: true,
      maxOptionsPerQuestion: 5,
      avoidSchemaLanguageToUser: true,
      requiredBlocks: PROGRESSIVE_CLARIFICATION_BLOCKS,
      blockExecutionRules: [
        "Do not merge required clarification blocks.",
        "Each required block must be presented as its own user-visible step or a clearly separated section before it can be marked confirmed.",
        "A phase_scope option may mention concept or frontend context, but those mentions are context only and do not satisfy concept_grounding or frontend_experience.",
        ...phaseScopeOptionComparisonRules(),
        ...phaseScopeSelfCheckRules(),
        "Do not set clarificationProgress.confirmedBlocks for a block until the user has seen that block's dedicated question or summary and confirmed or corrected it.",
        "The concept_grounding block must first map every confirmed scope.included item to its applicable requirement details before asking for concept confirmation.",
        ...scopeItemCoverageClarificationRules(),
        ...businessScenarioConfirmationRules(),
        ...decisionImpactOrderingRules(),
        ...businessLifecycleScanRules(),
        ...conceptGroundingSelfCheckRules(),
        "The concept_grounding block must clarify applicable objects or subjects, actions or behaviors, inputs or fields, preconditions, validation or blocking reasons, success state/data/UI/API/result changes, visible or returned feedback, and unresolved notes before final_summary when those details apply.",
        ...businessObjectOperationClarificationRules(),
        "The final_summary block must be shown after all applicable prior blocks and must summarize scope, concept understanding, frontend target or skip reason, and nextPhasePreview.",
        "The frontend_experience block must clarify page operation paths before final_summary when UI or user-visible workflow applies: how users find or receive target objects, where actions start, and how results are observed.",
        ...frontendExperienceSelfCheckRules(),
        ...frontendOperationPathClarificationRules(),
        "When the current phase involves business flows, user operations, state changes, forms/fields, validation/blocking rules, frontend/backend interaction, or user-facing operation paths, those details must be confirmed in their owning blocks and written into structured BrainstormCandidate fields; final_summary should only summarize the confirmed outcome and any corrections.",
        "When those business-detail categories do not apply, the final_summary block must state the concrete not-applicable reason before the user confirms.",
        ...finalSummaryReviewRules(),
        ...brainstormCandidateSelfReviewRules(),
      ],
      blockConfirmationRules: {
        phase_scope: "Satisfied only after the phase_scope self-check is complete and the user confirms current phase included, excluded, deferred scope and nextPhasePreview direction, including the recommended option when real alternative phase cuts were presented.",
        concept_grounding: "Satisfied only after the concept_grounding self-check is complete and the user sees a dedicated concept and business-rules summary that first covers every confirmed scope.included item, then lists business scenario confirmation, decision impact ordering, lifecycle scan, applicable key concepts, objects or subjects, actions or behaviors, inputs or fields, preconditions, rule boundaries, blocking reasons, success changes, visible feedback, source refs, unresolved notes, and must-not-misinterpret-as guards when applicable, then confirms or corrects it.",
        frontend_experience: "Satisfied only after the frontend_experience self-check is complete and the user sees a dedicated frontend target question or summary covering UI need, experience level, main users/workflows, how users find or receive target objects, action entry points, result/refresh feedback, and explicit unacceptable shapes, then confirms or corrects it.",
        final_summary: "Satisfied only after the user sees a concise combined final summary review of confirmed scope, concept/business-rule outcome, frontend target or skip reason, deferred/not-done boundaries, nextPhasePreview, and any corrections after the prior applicable blocks.",
      },
      frontendBlockRequiredWhen: [
        "requirement asks for user-facing UI",
        "repository contains frontend entrypoints",
        "current phase changes user-visible workflow",
      ],
    },
    riskGuidance: {
      askWhenRealMoneyOrExternalProductionSystemsAreImplied: true,
      separateSimulationFromProductionExecution: true,
      doNotAssumeDangerousCapabilitiesAreIncluded: true,
      surfaceComplianceSecurityAndDataRisksWhenRelevant: true,
    },
    confirmationRules: {
      mustShowSummaryBeforeAccept: true,
      mustWaitForExplicitUserConfirmation: true,
      initialUserRequestNeverCountsAsConfirmation: true,
      mustPresentUnderstandingSummaryBeforeAccept: true,
      userConfirmationMustOccurAfterPresentedSummary: true,
      doNotAcceptOnAmbiguousApproval: true,
      currentTurnExplicitConfirmationCounts: true,
      ifCurrentUserMessageConfirmsScopeDoNotAskAgain: true,
      consumeCurrentUserMessageBeforePromptingAgain: true,
    },
    rules: {
      agentOwnsClarificationConversation: true,
      askUserUntilScopeIsClear: true,
      separateIncludedExcludedDeferred: true,
      doNotLetDeferredLeakIntoIncluded: true,
      doNotUseCliNaturalLanguageParsing: true,
      doNotUseMarkerBasedParsing: true,
      ifUnclearAskUserInsteadOfGuessing: true,
      doNotProducePgcAacTaskPlanReviewOrCode: true,
      ifCurrentUserMessageContainsClearConfirmationWriteCandidateAndSubmit: "only_after_understanding_summary_presented",
      useProgressiveClarificationBlocks: true,
      preservePhase1InRoadmapPhases: true,
      phasePlanCurrentScopeRefsMustUseOnlyIncludedScopeIds: true,
      doNotPutExcludedOrDeferredScopeIdsInPhasePlanCurrentScopeRefs: true,
      phasePlanCurrentAcceptanceRefsMustUseOnlyAcceptanceIds: true,
      phaseScopeOptionComparison: {
        validationMode: "generation_guidance_only",
        requiredByDefault: true,
        nextPhaseSeedIsPreselectedAnswer: false,
        optionCount: { min: 2, max: 3 },
        recommendationMustPreserveCoreCurrentPhaseOutcome: true,
        narrowerOptionCannotBeRecommendedWhenDefersExplicitSeedItems: true,
        scopeReductionRequiresExplicitUserConfirmation: true,
        atomicSingleScopeExceptionAllowedOnlyForAtomicScope: true,
        rules: phaseScopeOptionComparisonRules(),
      },
      nextPhasePreviewGeneration: {
        validationMode: "generation_guidance_only",
        rules: nextPhasePreviewCandidateRules(),
      },
      candidateSelfReview: {
        validationMode: "generation_guidance_only",
        rules: brainstormCandidateSelfReviewRules(),
      },
      requirementSemanticGrounding: {
        validationMode: "generation_guidance_only",
        compactRules: brainstormRequirementSemanticCompactRules(),
        finalSummaryBusinessDetailContract: {
          appliesWhenAgentFinds: [
            "business flows",
            "user operations",
            "state changes",
            "forms or fields",
            "validation or blocking rules",
            "frontend/backend interaction",
            "user-facing operation paths",
          ],
          requiredUserVisibleTopicsWhenApplicable: [
            "confirmed current phase scope summary",
            "concept/business-rule block confirmed or skipped reason",
            "frontend_experience block confirmed or skipped reason",
            "explicit final_summary corrections that must be written back to structured fields",
            "deferred or not-done boundaries",
            "nextPhasePreview",
          ],
          notApplicableRule: "If none of these categories applies, state the concrete not-applicable reason in final_summary instead of fabricating business rules.",
          candidateFieldMapping: {
            scopeIncludedItems: "modules/actions/rules/fields/boundaries",
            acceptanceStatements: "verifiable business outcomes",
            businessFlowSummary: "business scenario, lifecycle actions, flow steps, preconditions, validation/blocking, success state",
            conceptGrounding: "high-risk concepts, business scenario meaning, decision impact, lifecycle semantics, applicable objects or subjects, actions or behaviors, inputs or fields, hard rules, state changes, blocking reasons, visible or returned feedback, unresolved notes, misunderstanding boundaries",
            frontendExperience: "target discovery, selection, input, display, action entry, refresh, and feedback expectations",
          },
          confirmedBlockDetailRetentionContract: {
            sourceOfTruth: "all_confirmed_brainstorm_blocks_plus_final_summary_corrections",
            finalSummaryRole: "review_gate_and_delta_confirmation_not_the_only_candidate_source",
            candidateFields: [
              "scope.included[].items",
              "scope.excluded",
              "scope.deferred",
              "acceptance[].statement",
              "domainModel.businessFlows[].summary",
              "conceptGrounding.phaseConceptGrounding.concepts[].explanation",
              "frontendExperience.dataViews/actions/operationPaths",
              "frontendExperienceDelta.dataViewDeltas/actionDeltas/operationPathDeltas",
              "phasePlan.nextPhasePreview",
            ],
            rules: confirmedBlockDetailRetentionRules(),
          },
          blockSelfCheckContract: {
            phase_scope: {
              owningBlock: "phase_scope",
              rules: phaseScopeSelfCheckRules(),
            },
            concept_grounding: {
              owningBlock: "concept_grounding",
              rules: [
                ...businessScenarioConfirmationRules(),
                ...decisionImpactOrderingRules(),
                ...businessLifecycleScanRules(),
                ...conceptGroundingSelfCheckRules(),
              ],
            },
            frontend_experience: {
              owningBlock: "frontend_experience",
              rules: frontendExperienceSelfCheckRules(),
            },
            final_summary: {
              owningBlock: "final_summary",
              rules: finalSummaryReviewRules(),
            },
          },
          finalSummaryReviewContract: {
            owningBlock: "final_summary",
            purpose: "Final summary is the user's last concise review surface before BrainstormCandidate submit. It confirms or corrects prior blocks; it is not the source of detailed requirements.",
            rules: finalSummaryReviewRules(),
          },
          scopeItemCoverageContract: {
            owningBlock: "concept_grounding",
            userLanguageRule: "Use the confirmed scope wording; do not expose internal schema language or force a fixed capability taxonomy.",
            candidateFields: ["scope.included[].items", "acceptance[].statement", "domainModel.businessFlows[].summary", "conceptGrounding.phaseConceptGrounding.concepts[].explanation", "frontendExperience/frontendExperienceDelta when UI applies"],
            rules: scopeItemCoverageCandidateRules(),
          },
          objectOperationContract: {
            owningBlock: "concept_grounding",
            userLanguageRule: "Use natural user-facing wording in the conversation; do not expose internal schema field names as if they were user choices.",
            candidateFields: ["scope.included[].items", "acceptance[].statement", "domainModel.businessFlows[].summary", "conceptGrounding.phaseConceptGrounding.concepts[].explanation", "frontendExperience/frontendExperienceDelta when UI applies"],
            rules: businessObjectOperationCandidateRules(),
          },
          frontendOperationPathContract: {
            owningBlock: "frontend_experience",
            userLanguageRule: "Use natural user-facing wording in the conversation; do not expose internal schema enum values.",
            candidateFields: ["frontendExperience.dataViews", "frontendExperience.actions", "frontendExperience.operationPaths", "frontendExperienceDelta.dataViewDeltas", "frontendExperienceDelta.actionDeltas", "frontendExperienceDelta.operationPathDeltas"],
            rules: frontendOperationPathCandidateRules(),
          },
        },
        rules: brainstormRequirementSemanticRules(),
      },
    },
    enumRefs: brainstormEnumRefs(),
    outputContract: {
      format: "json",
      schemaRef: "brainstorm-candidate-v1",
      candidateFile,
      schemaShape: brainstormCandidateSchemaShape(input),
    },
    blockedOutput: {
      schemaRef: "brainstorm-candidate-blocked-v1",
      candidateFile: blockedFile,
      schemaShape: {
        schemaVersion: "1.0",
        candidateId: "brainstorm-candidate-001",
        brainstormRunId: input.brainstormRunId,
        deliveryId: input.deliveryId,
        phaseId: input.phaseId,
        status: "blocked",
        requestSummary: {
          title: "Short title",
          oneLine: "Why Brainstorm cannot proceed.",
          complexity: "unknown",
        },
        scope: { included: [], excluded: [], deferred: [], assumptions: [] },
        roadmap: {
          required: false,
          currentPhaseId: input.phaseId,
          phases: [],
        },
        phasePlan: {
          current: {
            phaseId: input.phaseId,
            title: "Blocked current phase",
            goal: "Blocked before current phase could be confirmed.",
            scopeRefs: [],
            acceptanceRefs: [],
            status: "scope_confirmed",
          },
          nextPhasePreview: {
            kind: "none",
            reason: "Blocked before next phase preview could be confirmed.",
          },
        },
        acceptance: [],
        userConfirmation: {
          confirmed: false,
          confirmationSummary: "Explain what is blocking.",
        },
        handoff: {
          ready: false,
          nextNode: "blocked",
          blockingReasons: ["Describe missing input or unsafe ambiguity."],
        },
      },
    },
    submitCommand,
    createdAt: input.now,
  };
}

function brainstormEnumRefs(): Record<string, string[]> {
  return {
    candidateStatus: ["confirmed", "needs_clarification", "blocked"],
    requirementSourceType: ["user_text", "pdf", "word", "markdown", "text", "code", "spreadsheet", "unknown"],
    scopeBucket: ["included", "excluded", "deferred"],
    roadmapRequirement: ["required", "not_required"],
    phaseStatus: ["scope_confirmed", "proposed", "delivered", "paused", "skipped", "revised"],
    scopeSource: ["source_explicit", "user_confirmed", "user_overridden", "model_recommended", "derived"],
    acceptancePriority: ["must", "should", "could"],
    conceptGroundingMode: ["concepts_present", "none_required", "not_applicable"],
    conceptPhaseRelevance: ["current", "current_adjacent", "future", "deferred", "excluded"],
    conceptPriority: ["must_understand", "should_understand", "nice_to_understand"],
    conceptRiskFactor: [
      "business_invariant",
      "state_transition",
      "resource_consistency",
      "permission_boundary",
      "external_contract",
      "scope_confusion_risk",
      "user_visible_flow",
      "runtime_or_delivery_semantics",
      "frontend_experience_semantics",
    ],
    clarificationBlock: PROGRESSIVE_CLARIFICATION_BLOCKS,
    frontendExperienceLevel: ["none", "technical_demo", "usable_internal_product", "polished_product"],
    frontendTargetSelectionMode: ["query_and_select", "direct_id_lookup", "preselected_context", "not_applicable"],
    frontendActionEntryPoint: ["result_row_action", "detail_button", "form_submit", "bulk_action", "inline_action", "navigation_entry"],
    frontendResultObservationMode: ["list_refresh", "detail_refresh", "inline_status_update", "response_message", "not_applicable"],
    frontendInteractionState: ["loading", "success", "error", "empty", "business_blocking"],
  };
}

function brainstormCandidateSchemaShape(input: {
  deliveryId: string;
  phaseId: string;
  brainstormRunId: string;
}): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    candidateId: "brainstorm-candidate-001",
    brainstormRunId: input.brainstormRunId,
    deliveryId: input.deliveryId,
    phaseId: input.phaseId,
    status: "confirmed",
    requestSummary: {
      title: "Short product title",
      oneLine: "One-line confirmed delivery goal.",
      businessGoal: "Confirmed business goal.",
      complexity: "small|medium|large|unknown",
    },
    sources: [{
      sourceId: "src-001",
      type: "user_text | pdf | word | markdown | text | code | spreadsheet | unknown",
      path: "optional project-relative or absolute source file path",
      title: "Optional source title",
      textDigest: "sha256:<digest-of-source-text>",
      extracted: true,
    }],
    scope: {
      included: [{
        id: "scope-current-phase",
        label: "Current phase scope",
        items: [
          "Current phase business object or technical object.",
          "Current phase user/system action.",
          "Current phase key field set: identity/input/display/relationship/status/result-feedback fields when applicable.",
          "Current phase precondition, validation, blocking rule and reason, success state change, feedback expectation, or boundary when applicable.",
          "Business scenario, decision impact, or lifecycle action detail confirmed in the relevant Brainstorm block when applicable.",
        ],
        reason: "Why it is included now.",
        source: "user_confirmed",
      }],
      excluded: [{
        id: "scope-excluded-example",
        label: "Explicitly excluded scope",
        items: ["Concrete excluded item"],
        reason: "Why it is not included in the current phase.",
        source: "user_confirmed",
      }],
      deferred: [{
        id: "scope-deferred-example",
        label: "Deferred scope",
        items: ["Concrete deferred item"],
        reason: "Why it should wait for a later phase.",
        source: "user_confirmed",
      }],
      assumptions: [{
        id: "assumption-001",
        text: "A concrete assumption that remains true for this candidate.",
        requiresConfirmation: false,
      }],
    },
    roadmap: {
      required: false,
      currentPhaseId: input.phaseId,
      phases: input.phaseId === "phase-1"
        ? [{
            phaseId: "phase-1",
            title: "Current phase",
            status: "scope_confirmed",
            goal: "Confirmed current phase goal.",
            scopeRefs: ["scope-current-phase"],
            acceptanceRefs: ["AC-001"],
            dependsOn: [],
          }]
        : [
            {
              phaseId: "phase-1",
              title: "Prior phase",
              status: "delivered",
              goal: "Prior confirmed phase retained for roadmap continuity.",
              scopeRefs: [],
              acceptanceRefs: [],
              dependsOn: [],
            },
            {
              phaseId: input.phaseId,
              title: "Current phase",
              status: "scope_confirmed",
              goal: "Confirmed current phase goal.",
              scopeRefs: ["scope-current-phase"],
              acceptanceRefs: ["AC-001"],
              dependsOn: ["phase-1"],
            },
          ],
    },
    phasePlan: {
      current: {
        phaseId: input.phaseId,
        title: "Phase 1",
        goal: "Confirmed current phase goal.",
        scopeRefs: ["scope-current-phase"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview: {
        oneOf: [
          {
            kind: "candidate",
            suggestedPhaseId: "phase-2",
            title: "Source-grounded next phase candidate title",
            goal: "Concrete non-binding next phase candidate goal",
            scopePreview: ["Concrete source-grounded business object/action/workflow candidate"],
            reason: "Why another phase is useful.",
          },
          {
            kind: "none",
            reason: "Why no next phase remains.",
          },
        ],
        rule: "Use kind=none only when the confirmed delivery has no deferred scope and no later roadmap phase.",
      },
    },
    domainModel: {
      actors: [{
        id: "actor-user",
        name: "User",
        description: "Who uses or benefits from the requested software.",
      }],
      capabilityGroups: [{
        id: "capability-core",
        name: "Core capability",
        description: "Main capability group confirmed for the current phase.",
      }],
      businessFlows: [{
        id: "flow-core",
        name: "Core flow",
        actors: ["actor-user"],
        capabilityRefs: ["capability-core"],
        summary: "When applicable, include the current business scenario confirmation, decision impacts ordered by downstream effect, relevant lifecycle actions for key objects, scope-item coverage, objects or subjects, actions or behaviors, inputs or fields, flow steps, preconditions, validation or blocking rules and reasons, success outcome, state/data/UI/API/result changes, visible or returned feedback, and fields to input/display/pass through. If this phase is non-domain technical work, describe the technical workflow and why business detail is not applicable.",
      }],
    },
    acceptance: [{
      id: "AC-001",
      statement: "A concrete, source-grounded, verifiable outcome for the current phase, including key rule/field/status expectations when applicable.",
      capabilityRefs: [],
      sourceRefs: [],
      priority: "must",
    }],
    userConfirmation: {
      confirmed: true,
      confirmedAt: new Date(0).toISOString(),
      confirmationSummary: "What the user explicitly confirmed.",
      confirmationBasis: {
        initialRequestOnly: false,
        summaryPresentedToUser: true,
        confirmedAfterSummary: true,
        presentedItems: FIRST_CLARIFICATION_PRESENTED_ITEMS,
      },
    },
    conceptGrounding: {
      deliveryConceptGlossary: input.phaseId === "phase-1" ? {
        mode: "concepts_present | none_required | not_applicable",
        reason: "Required when mode is none_required or not_applicable.",
        concepts: [{
          conceptId: "concept-global-001",
          term: "Business term",
          normalizedName: "business_term",
          explanation: "Plain explanation confirmed with the user.",
          mustNotMisinterpretAs: ["Incorrect interpretation"],
          phaseRelevance: "current | current_adjacent | future | deferred | excluded",
          priority: "must_understand | should_understand | nice_to_understand",
          attentionRank: 1,
          riskFactors: ["business_invariant"],
          scopeRefs: ["scope-current-phase"],
          acceptanceRefs: ["AC-001"],
          humanReadableReason: "Why this concept matters.",
        }],
      } : undefined,
      phaseConceptGrounding: {
        mode: "concepts_present | none_required | not_applicable",
        reason: "Required when mode is none_required or not_applicable.",
        concepts: [{
          conceptId: "concept-current-001",
          term: "Current phase concept",
          normalizedName: "current_phase_concept",
          explanation: "Concept explanation shown to the user, including scope-item coverage, current business scenario meaning, decision impact, lifecycle semantics, current phase object or subject semantics, key field meaning, supported actions or behaviors, inputs or fields, validation or blocking rules, state transition expectations, visible feedback, unresolved notes, and implementation misunderstanding boundaries when applicable.",
          mustNotMisinterpretAs: ["Incorrect implementation meaning"],
          phaseRelevance: "current",
          priority: "must_understand",
          attentionRank: 1,
          riskFactors: ["scope_confusion_risk"],
          scopeRefs: ["scope-current-phase"],
          acceptanceRefs: ["AC-001"],
          humanReadableReason: "Why misunderstanding this concept would harm the phase.",
        }],
      },
      glossaryUpdates: [],
    },
    conceptConfirmation: {
      shownToUser: true,
      confirmedConceptRefs: ["concept-current-001"],
      confirmationSummary: "What key concepts were shown to and confirmed by the user.",
    },
    clarificationProgress: {
      mode: "progressive_blocks",
      confirmedBlocks: PROGRESSIVE_CLARIFICATION_BLOCKS.map((block) => ({
        block,
        summary: `User confirmed ${block}.`,
        confirmedByUser: true,
      })),
      skippedBlocks: [],
      finalSummaryConfirmed: true,
    },
    frontendExperience: {
      required: true,
      kind: "business_application | technical_demo | none",
      experienceLevel: "none | technical_demo | usable_internal_product | polished_product",
      audiences: [{
        audienceId: "audience-operator",
        name: "Operator",
        primaryJobs: ["Operate the current phase workflow."],
      }],
      surfaces: [{
        surfaceId: "surface-main",
        name: "Main workspace",
        audienceRefs: ["audience-operator"],
        primaryJobs: ["Complete current phase workflow."],
      }],
      dataViews: [{
        viewId: "view-current-results",
        name: "Current phase result list or detail",
        purpose: "Let users find, select, or inspect the current phase target object before acting.",
        targetObject: "Business object users operate on, when applicable.",
        selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
        paginationRequired: true,
        defaultLoadsFirstPage: true,
        searchCriteria: [{
          criterionId: "criterion-confirmed-field",
          label: "User-facing query condition grounded in confirmed object fields or user wording.",
          fieldRef: "optional confirmed object/entity field ref",
          reason: "Why this query condition is needed for the operation path.",
          sourceRefs: ["src-001"],
        }],
        criteriaUnclearNote: "If confirmed fields are insufficient, use a basic paginated list with no advanced filters and record this note.",
        sourceRefs: ["src-001"],
      }],
      actions: [{
        actionId: "action-current-operation",
        label: "User-facing operation name",
        targetObject: "Business object acted on, when applicable.",
        entryPoint: "result_row_action | detail_button | form_submit | bulk_action | inline_action | navigation_entry",
        inputFields: ["Confirmed input field needed for this action."],
        resultObservation: ["list_refresh", "response_message"],
        refreshPolicy: "refresh_current_query | refresh_detail | update_inline_state | show_message_only | not_applicable",
        successFeedback: ["Success message, refreshed row/detail, or changed status visible to the user."],
        blockingOrErrorFeedback: ["Business blocking reason or validation error visible to the user."],
        sourceRefs: ["src-001"],
      }],
      operationPaths: [{
        pathId: "path-current-operation",
        name: "Current phase operation path",
        userGoal: "What the user is trying to complete.",
        surfaceRef: "surface-main",
        workflowRef: "flow-core",
        targetObject: "Business object users operate on, when applicable.",
        selectionMode: "query_and_select | direct_id_lookup | preselected_context | not_applicable",
        selectionSummary: "Natural-language summary, e.g. paginated query results -> select record -> trigger action -> observe refreshed status.",
        dataViewRefs: ["view-current-results"],
        actionRefs: ["action-current-operation"],
        requiredStates: ["loading", "success", "error", "empty", "business_blocking"],
        sourceRefs: ["src-001"],
      }],
      mustNot: ["single_page_form_stack", "unstyled_browser_default", "phase_by_phase_demo_append_only"],
      confirmationSummary: "User confirmed the frontend delivery level, main users/workspaces, and page operation path in natural language.",
    },
    handoff: {
      ready: true,
      nextNode: "technical_baseline_generation",
      blockingReasons: [],
    },
    candidateRules: [
      "For Phase N continuation, preserve prior roadmap phases such as phase-1 in roadmap.phases; do not output only the current phase.",
      "roadmap.currentPhaseId and phasePlan.current.phaseId must equal the active request phaseId.",
      "sources[].type must use exactly one enumRefs.requirementSourceType value; never use user_file. Use pdf for an original PDF requirement source, text for extracted text, markdown for markdown files, code for source code, user_text for direct user text, and unknown only when the source kind is unclear.",
      "phasePlan.current.scopeRefs may reference only scope.included ids.",
      "Excluded scope belongs only in scope.excluded and roadmap phase excluded refs when supported; deferred scope belongs only in scope.deferred or nextPhasePreview.",
      "phasePlan.current.acceptanceRefs may reference only acceptance[].id values.",
      "roadmap.required is normalized by Loom on accept from confirmed scope and phasePlan signals. Do not let this boolean override scope.deferred or phasePlan.nextPhasePreview.",
      "If clarificationProgress confirms frontend_experience, include frontendExperience or frontendExperienceDelta. If the frontend block is skipped, include skippedBlocks with a concrete reason and do not invent frontend work.",
      "When frontendExperience is present, it is the user-confirmed product target that AAC must consume later; do not use it for implementation details.",
      "Write page operation path details into frontendExperience.dataViews/actions/operationPaths or frontendExperienceDelta.*Deltas; do not leave them only in confirmationSummary or chat.",
      "Do not show internal frontend enum values to the user during clarification. Use natural language when asking or summarizing.",
      "For Phase 1, deliveryConceptGlossary should capture delivery-wide high-risk concepts from the whole requirement; do not collapse it to a single generic project label.",
      "For every phase, phaseConceptGrounding should capture current-phase high-risk concepts and must not promote future/deferred/excluded concepts into current scope.",
      "Before setting conceptConfirmation.shownToUser=true, concept_grounding must show a scope-item coverage summary for every confirmed scope.included item. Each item must be covered, explicitly unresolved, or explicitly deferred; do not silently omit included scope.",
      "Set conceptConfirmation.shownToUser=true only after a dedicated concept_grounding block showed scope-item coverage plus applicable objects or subjects, actions or behaviors, inputs or fields, preconditions, validation or blocking reasons, success state/data/UI/API/result changes, visible or returned feedback, source refs, unresolved notes, and must-not-misinterpret-as guards.",
      "When deriving sources from RequirementContext, read sourceFieldAccessHints: input sources use sourceItems[].itemId/kind, while BrainstormCandidate output sources use sources[].sourceId/type.",
      "Required clarification blocks must not be merged: phase_scope mentions are context only and do not satisfy concept_grounding or frontend_experience.",
      "Set frontend_experience confirmed only after a dedicated frontend_experience block showed the UI target or skip reason to the user.",
      "Set finalSummaryConfirmed=true only after a dedicated final_summary block concisely summarized scope, concept/business-rule outcome, frontend target or skip reason, nextPhasePreview, deferred/not-done boundaries, and explicit corrections.",
      "If the current phase involves business flows, user operations, state changes, forms/fields, validation/blocking rules, or frontend/backend interaction, structured candidate fields must preserve those confirmed details using existing fields; do not rely on final_summary text and do not leave details for PGC, AAC, TaskPlan, or TaskExecution to rediscover from the original requirement.",
      "If those business-detail categories do not apply, final_summary must record the concrete reason and the candidate should avoid fabricating domain rules.",
      ...phaseScopeOptionComparisonRules(),
      ...brainstormCandidateSelfReviewRules(),
      ...brainstormRequirementSemanticRules(),
      ...nextPhasePreviewCandidateRules(),
    ],
  };
}

function emptyClarification(): BrainstormContract["clarification"] {
  return {
    status: "needs_answer",
    turns: [],
    questions: [],
    answers: [],
    patches: [],
    confirmations: [],
    pendingQuestionIds: [],
    pendingConfirmationIds: [],
  };
}

function buildSources(input: RequirementInput): RequirementSource[] {
  const sources = [...input.requestSources, ...input.contextSources];
  return sources.map((source, index) => ({
    sourceId: `src-${pad(index + 1)}`,
    type: sourceTypeFor(source.path, source.kind === "context" || source.kind === "context-file"),
    ...(source.path ? { path: source.path } : {}),
    ...(source.label ? { title: source.label } : source.kind ? { title: source.kind } : {}),
    textDigest: `sha256:${hash(source.content)}`,
    extracted: Boolean(source.content),
  }));
}

function sourceTypeFor(sourcePath: string | undefined, isContext: boolean): RequirementSourceType {
  if (!sourcePath) {
    return isContext ? "text" : "user_text";
  }
  const ext = path.extname(sourcePath).toLowerCase();
  if (ext === ".pdf") {
    return "pdf";
  }
  if (ext === ".doc" || ext === ".docx") {
    return "word";
  }
  if (ext === ".md" || ext === ".markdown") {
    return "markdown";
  }
  if ([".ts", ".tsx", ".js", ".jsx", ".java", ".py", ".go", ".rs"].includes(ext)) {
    return "code";
  }
  if ([".txt", ".json", ".yaml", ".yml"].includes(ext)) {
    return "text";
  }
  if ([".csv", ".tsv", ".xlsx", ".xls"].includes(ext)) {
    return "spreadsheet";
  }
  return "unknown";
}

function shouldUseRoadmap(input: RequirementInput): boolean {
  const combined = [input.primaryRequest, ...input.requestSources.map((source) => source.content)].join("\n");
  const fileCount = input.requestSources.filter((source) => source.path).length + input.contextSources.filter((source) => source.path).length;
  const markers = [
    "pdf",
    "系统",
    "完整",
    "roadmap",
    "阶段",
    "后台",
    "管理",
    "统计",
    "交易",
    "部署",
  ];
  return combined.length > 800 || fileCount > 0 || markers.some((marker) => combined.includes(marker));
}

function inferTitle(input: RequirementInput): string {
  const fileSource = [...input.requestSources, ...input.contextSources].find((source) => source.path || source.label);
  const fromFile = fileSource?.path ? path.basename(fileSource.path, path.extname(fileSource.path)) : fileSource?.label;
  if (fromFile && fromFile !== "stdin") {
    return truncateText(fromFile, 60);
  }
  return truncateText(input.primaryRequest.split(/[，。,.!?！？\n]/)[0] ?? "loom 需求", 60) || "loom 需求";
}

async function loadBrainstormContract(projectRoot: string, brainstormRunId: string): Promise<BrainstormContract> {
  const filePath = await contractPathFor(projectRoot, brainstormRunId);
  if (!(await pathExists(filePath))) {
    throw brainstormNotFound(projectRoot, brainstormRunId);
  }

  const raw = await fs.readFile(filePath, "utf8");
  let json: unknown;
  try {
    json = JSON.parse(raw);
  } catch {
    throw stateCorrupted("Brainstorm contract contains invalid JSON.", { file: filePath });
  }

  try {
    return brainstormContractSchema.parse(json);
  } catch (error) {
    if (error instanceof ZodError) {
      throw stateCorrupted("Brainstorm contract does not match schema.", {
        file: filePath,
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

async function readJsonFileForCandidate(filePath: string): Promise<unknown> {
  try {
    const raw = await fs.readFile(filePath, "utf8");
    return JSON.parse(raw) as unknown;
  } catch (error) {
    if (error instanceof SyntaxError) {
      throw stateCorrupted("Brainstorm candidate contains invalid JSON.", { file: filePath });
    }
    throw stateCorrupted("Brainstorm candidate cannot be read.", { file: filePath });
  }
}

async function validateBrainstormAcceptRequest(
  projectRoot: string,
  input: {
    deliveryId: string;
    phaseId: string;
    brainstormRunId: string;
    commandBrainstormRunId?: string;
    requestId?: string;
    candidateFile: string;
  },
): Promise<Array<{ code: string; path: string; message: string }>> {
  const issues: Array<{ code: string; path: string; message: string }> = [];
  if (!input.commandBrainstormRunId) {
    issues.push({
      code: "RUN_ID_REQUIRED",
      path: "runId",
      message: "brainstorm accept requires --run-id so the command is bound to the current BrainstormSessionRequest.",
    });
  } else if (input.commandBrainstormRunId !== input.brainstormRunId) {
    issues.push({
      code: "COMMAND_RUN_MISMATCH",
      path: "runId",
      message: "Command --run-id must match BrainstormCandidate brainstormRunId.",
    });
  }
  if (!input.requestId) {
    issues.push({
      code: "REQUEST_ID_REQUIRED",
      path: "requestId",
      message: "brainstorm accept requires --request-id so the candidate can be validated against its BrainstormSessionRequest.",
    });
    return issues;
  }

  const requestPath = brainstormSessionRequestPath(projectRoot, input.deliveryId, input.requestId);
  if (!(await pathExists(requestPath))) {
    issues.push({
      code: "REQUEST_NOT_FOUND",
      path: "requestId",
      message: "BrainstormSessionRequest does not exist for the provided requestId.",
    });
    return issues;
  }

  const request = await readJsonFileForCandidate(requestPath) as Partial<BrainstormSessionRequest>;
  if (request.deliveryId !== input.deliveryId) {
    issues.push({ code: "REQUEST_DELIVERY_MISMATCH", path: "request.deliveryId", message: "BrainstormSessionRequest deliveryId must match command deliveryId." });
  }
  if (request.phaseId !== input.phaseId) {
    issues.push({ code: "REQUEST_PHASE_MISMATCH", path: "request.phaseId", message: "BrainstormSessionRequest phaseId must match command phaseId." });
  }
  if (request.brainstormRunId !== input.brainstormRunId) {
    issues.push({ code: "REQUEST_RUN_MISMATCH", path: "request.brainstormRunId", message: "BrainstormSessionRequest brainstormRunId must match candidate brainstormRunId." });
  }
  const index = await loadDeliveryIndex(projectRoot, input.deliveryId);
  const phase = index.phases.find((item) => item.phaseId === input.phaseId);
  if (!phase) {
    issues.push({ code: "REQUEST_PHASE_NOT_FOUND", path: "phaseId", message: "BrainstormSessionRequest phase does not exist in DeliveryRun." });
  } else {
    if (index.activePhaseId !== input.phaseId) {
      issues.push({ code: "REQUEST_NOT_ACTIVE_PHASE", path: "phaseId", message: "Brainstorm accept can only accept the active phase's latest BrainstormSessionRequest." });
    }
    if (phase.latestRefs.brainstormRequestId !== input.requestId) {
      issues.push({ code: "REQUEST_NOT_LATEST", path: "requestId", message: "BrainstormSessionRequest is not the active phase latest request." });
    }
    if (phase.latestRefs.brainstormRunId !== input.brainstormRunId) {
      issues.push({ code: "REQUEST_RUN_NOT_LATEST", path: "brainstormRunId", message: "BrainstormCandidate run id is not the active phase latest run id." });
    }
  }
  const expectedCandidateFile = request.outputContract?.candidateFile;
  if (typeof expectedCandidateFile === "string") {
    const actualCandidateFile = toProjectRelative(projectRoot, resolveCliPath(projectRoot, input.candidateFile));
    if (actualCandidateFile !== expectedCandidateFile) {
      issues.push({
        code: "REQUEST_CANDIDATE_FILE_MISMATCH",
        path: "candidateFile",
        message: "Candidate file must match BrainstormSessionRequest outputContract.candidateFile.",
      });
    }
  }
  return issues;
}

async function attachBrainstormRequestRefs(
  projectRoot: string,
  deliveryId: string,
  phaseId: string,
  refs: {
    brainstormRequest: string;
    brainstormRequestId: string;
    brainstormRunId: string;
    brainstormCandidateFile: string;
    requirementContextRef?: string | null;
    normalizedRequirementTextRef?: string | null;
    keywordHintsRef?: string | null;
  },
): Promise<void> {
  const index = await loadDeliveryIndex(projectRoot, deliveryId);
  const phase = index.phases.find((item) => item.phaseId === phaseId);
  if (!phase) {
    throw invalidArgument("Phase does not exist in DeliveryRun.", { deliveryId, phaseId });
  }
  updatePhase(index, phaseId, {
    latestRefs: {
      ...phase.latestRefs,
      ...refs,
    },
  });
  index.updatedAt = new Date().toISOString();
  await saveDeliveryIndex(projectRoot, index);
  await upsertStatusDelivery(projectRoot, index);
}

function validateBrainstormCandidate(
  candidate: BrainstormCandidate,
  deliveryId: string,
  phaseId: string,
): Array<{ code: string; path: string; message: string }> {
  const issues: Array<{ code: string; path: string; message: string }> = [];
  if (candidate.deliveryId !== deliveryId) {
    issues.push({ code: "DELIVERY_ID_MISMATCH", path: "deliveryId", message: "Candidate deliveryId must match active delivery." });
  }
  if ((candidate.phaseId ?? phaseId) !== phaseId) {
    issues.push({ code: "PHASE_ID_MISMATCH", path: "phaseId", message: "Candidate phaseId must match target phase." });
  }
  if (candidate.status !== "confirmed") {
    return issues;
  }
  if (candidate.userConfirmation.confirmed !== true) {
    issues.push({ code: "USER_CONFIRMATION_MISSING", path: "userConfirmation.confirmed", message: "BrainstormCandidate requires explicit user confirmation." });
  }
  validateMandatoryClarificationGate(candidate, issues);
  validateConceptGrounding(candidate, phaseId, issues);
  validateFrontendExperienceConfirmation(candidate, issues);
  if (candidate.handoff.ready !== true) {
    issues.push({ code: "HANDOFF_NOT_READY", path: "handoff.ready", message: "Confirmed BrainstormCandidate must be ready for handoff." });
  }
  if (candidate.handoff.nextNode !== "technical_baseline_generation") {
    issues.push({ code: "INVALID_NEXT_NODE", path: "handoff.nextNode", message: "Confirmed BrainstormCandidate must hand off to technical_baseline_generation." });
  }
  const duplicatedScopeIds = duplicates([
    ...candidate.scope.included.map((item) => item.id),
    ...candidate.scope.excluded.map((item) => item.id),
    ...candidate.scope.deferred.map((item) => item.id),
  ]);
  for (const duplicate of duplicatedScopeIds) {
    issues.push({ code: "DUPLICATE_SCOPE_ID", path: `scope.${duplicate}`, message: "Scope item id cannot appear in multiple buckets or multiple times." });
  }
  const phaseIds = candidate.roadmap.phases.map((phase) => phase.phaseId);
  for (const duplicate of duplicates(phaseIds)) {
    issues.push({ code: "DUPLICATE_PHASE_ID", path: `roadmap.phases.${duplicate}`, message: "Phase id must be unique." });
  }
  if (!phaseIds.includes("phase-1")) {
    issues.push({ code: "PHASE_1_REQUIRED", path: "roadmap.phases", message: "Roadmap must contain phase-1 even for small requests." });
  }
  if (!phaseIds.includes(candidate.roadmap.currentPhaseId)) {
    issues.push({ code: "CURRENT_PHASE_MISSING", path: "roadmap.currentPhaseId", message: "Current phase must exist in roadmap.phases." });
  }
  const currentPhaseId = candidate.roadmap.currentPhaseId || phaseId;
  const currentPhase = candidate.roadmap.phases.find((phase) => phase.phaseId === candidate.roadmap.currentPhaseId);
  if (currentPhase?.status !== "scope_confirmed") {
    issues.push({ code: "CURRENT_PHASE_NOT_CONFIRMED", path: "roadmap.currentPhaseId", message: "Current phase must be scope_confirmed." });
  }
  if (candidate.phasePlan.current.phaseId !== currentPhaseId) {
    issues.push({ code: "PHASE_PLAN_CURRENT_MISMATCH", path: "phasePlan.current.phaseId", message: "phasePlan.current.phaseId must equal roadmap.currentPhaseId." });
  }
  validateNextPhasePreviewConsistency(candidate, currentPhaseId, issues);
  for (const scopeRef of candidate.phasePlan.current.scopeRefs) {
    if (!candidate.scope.included.some((item) => item.id === scopeRef)) {
      issues.push({ code: "PHASE_PLAN_SCOPE_REF_INVALID", path: `phasePlan.current.scopeRefs.${scopeRef}`, message: "phasePlan.current.scopeRefs must reference current candidate scope.included ids." });
    }
  }
  for (const acceptanceRef of candidate.phasePlan.current.acceptanceRefs) {
    if (!candidate.acceptance.some((item) => item.id === acceptanceRef)) {
      issues.push({ code: "PHASE_PLAN_ACCEPTANCE_REF_INVALID", path: `phasePlan.current.acceptanceRefs.${acceptanceRef}`, message: "phasePlan.current.acceptanceRefs must reference current candidate acceptance ids." });
    }
  }
  const acceptanceIds = new Set(candidate.acceptance.map((item) => item.id));
  for (const phase of candidate.roadmap.phases.filter((item) => item.phaseId === currentPhaseId)) {
    for (const acceptanceRef of phase.acceptanceRefs) {
      if (!acceptanceIds.has(acceptanceRef)) {
        issues.push({
          code: "INVALID_ACCEPTANCE_REF",
          path: `roadmap.phases.${phase.phaseId}.acceptanceRefs.${acceptanceRef}`,
          message: "Current phase acceptanceRefs must reference candidate.acceptance ids.",
        });
      }
    }
  }
  return issues;
}

function validateNextPhasePreviewConsistency(
  candidate: BrainstormCandidate,
  currentPhaseId: string,
  issues: Array<{ code: string; path: string; message: string }>,
): void {
  const preview = candidate.phasePlan.nextPhasePreview;
  const currentPhaseIndex = candidate.roadmap.phases.findIndex((phase) => phase.phaseId === currentPhaseId);
  const hasRoadmapPhaseAfterCurrent = currentPhaseIndex >= 0 && candidate.roadmap.phases.slice(currentPhaseIndex + 1).length > 0;

  if (preview.kind === "none") {
    if (candidate.scope.deferred.length > 0) {
      issues.push({
        code: "NEXT_PHASE_PREVIEW_REQUIRED_FOR_DEFERRED_SCOPE",
        path: "phasePlan.nextPhasePreview",
        message: "Confirmed deferred scope means this delivery still has a next-phase candidate. Use nextPhasePreview.kind=candidate instead of kind=none, with concrete scopePreview items derived from scope.deferred.",
      });
    }
    if (hasRoadmapPhaseAfterCurrent) {
      issues.push({
        code: "NEXT_PHASE_PREVIEW_REQUIRED_FOR_ROADMAP_PHASE",
        path: "phasePlan.nextPhasePreview",
        message: "Roadmap contains a phase after the current phase, so nextPhasePreview.kind must be candidate rather than none.",
      });
    }
    return;
  }

  if (preview.suggestedPhaseId === currentPhaseId) {
    issues.push({
      code: "NEXT_PHASE_PREVIEW_PHASE_ID_REUSES_CURRENT",
      path: "phasePlan.nextPhasePreview.suggestedPhaseId",
      message: "nextPhasePreview.suggestedPhaseId must identify the next Brainstorm phase, not the current confirmed phase.",
    });
  }
  if (preview.scopePreview.length === 0) {
    issues.push({
      code: "NEXT_PHASE_PREVIEW_SCOPE_EMPTY",
      path: "phasePlan.nextPhasePreview.scopePreview",
      message: "nextPhasePreview.kind=candidate requires at least one concrete scopePreview item.",
    });
  }
}

function validateMandatoryClarificationGate(
  candidate: BrainstormCandidate,
  issues: Array<{ code: string; path: string; message: string }>,
): void {
  const basis = candidate.userConfirmation.confirmationBasis;
  if (!basis) {
    issues.push({ code: "CONFIRMATION_BASIS_MISSING", path: "userConfirmation.confirmationBasis", message: "BrainstormCandidate must prove confirmation happened after a presented summary." });
    return;
  }
  if (basis.initialRequestOnly !== false) {
    issues.push({ code: "INITIAL_REQUEST_CANNOT_CONFIRM", path: "userConfirmation.confirmationBasis.initialRequestOnly", message: "Initial user input cannot count as Brainstorm confirmation." });
  }
  if (basis.summaryPresentedToUser !== true) {
    issues.push({ code: "SUMMARY_NOT_PRESENTED", path: "userConfirmation.confirmationBasis.summaryPresentedToUser", message: "Agent must present a phase understanding summary before accept." });
  }
  if (basis.confirmedAfterSummary !== true) {
    issues.push({ code: "CONFIRMATION_NOT_AFTER_SUMMARY", path: "userConfirmation.confirmationBasis.confirmedAfterSummary", message: "User confirmation must happen after the summary was presented." });
  }
  const presented = new Set(basis.presentedItems);
  for (const item of FIRST_CLARIFICATION_PRESENTED_ITEMS) {
    if (!presented.has(item)) {
      issues.push({ code: "CONFIRMATION_PRESENTED_ITEM_MISSING", path: `userConfirmation.confirmationBasis.presentedItems.${item}`, message: `Brainstorm confirmation summary must include ${item}.` });
    }
  }

  const progress = candidate.clarificationProgress;
  if (!progress) {
    issues.push({ code: "CLARIFICATION_PROGRESS_MISSING", path: "clarificationProgress", message: "BrainstormCandidate must record progressive clarification blocks." });
    return;
  }
  if (progress.finalSummaryConfirmed !== true) {
    issues.push({ code: "FINAL_SUMMARY_NOT_CONFIRMED", path: "clarificationProgress.finalSummaryConfirmed", message: "Final Brainstorm summary must be confirmed before accept." });
  }
  const confirmedBlocks = new Map(progress.confirmedBlocks.map((block) => [block.block, block]));
  const skippedBlocks = new Map(progress.skippedBlocks.map((block) => [block.block, block]));
  for (const block of PROGRESSIVE_CLARIFICATION_BLOCKS) {
    const confirmed = confirmedBlocks.get(block);
    const skipped = skippedBlocks.get(block);
    if (!confirmed && !skipped) {
      issues.push({ code: "CLARIFICATION_BLOCK_MISSING", path: `clarificationProgress.${block}`, message: `Progressive clarification block ${block} must be confirmed or explicitly skipped with a reason.` });
    }
    if (confirmed && confirmed.confirmedByUser !== true) {
      issues.push({ code: "CLARIFICATION_BLOCK_NOT_CONFIRMED", path: `clarificationProgress.confirmedBlocks.${block}`, message: `Clarification block ${block} must be confirmed by the user.` });
    }
  }
}

function validateConceptGrounding(
  candidate: BrainstormCandidate,
  phaseId: string,
  issues: Array<{ code: string; path: string; message: string }>,
): void {
  if (!candidate.conceptGrounding) {
    issues.push({ code: "CONCEPT_GROUNDING_MISSING", path: "conceptGrounding", message: "BrainstormCandidate must include ConceptGrounding for the current phase." });
    return;
  }
  if (phaseId === "phase-1" && !candidate.conceptGrounding.deliveryConceptGlossary) {
    issues.push({ code: "DELIVERY_CONCEPT_GLOSSARY_MISSING", path: "conceptGrounding.deliveryConceptGlossary", message: "Phase 1 BrainstormCandidate must include deliveryConceptGlossary." });
  }
  validateConceptSet("conceptGrounding.deliveryConceptGlossary", candidate.conceptGrounding.deliveryConceptGlossary, candidate, issues);
  validateConceptSet("conceptGrounding.phaseConceptGrounding", candidate.conceptGrounding.phaseConceptGrounding, candidate, issues);

  const conceptIds = allConceptIds(candidate);
  const confirmation = candidate.conceptConfirmation;
  if (!confirmation) {
    issues.push({ code: "CONCEPT_CONFIRMATION_MISSING", path: "conceptConfirmation", message: "BrainstormCandidate must state that concepts were shown to the user." });
    return;
  }
  if (confirmation.shownToUser !== true) {
    issues.push({ code: "CONCEPTS_NOT_SHOWN_TO_USER", path: "conceptConfirmation.shownToUser", message: "Key concepts must be shown to the user before accept." });
  }
  for (const ref of confirmation.confirmedConceptRefs) {
    if (!conceptIds.has(ref)) {
      issues.push({ code: "CONCEPT_CONFIRMATION_REF_INVALID", path: `conceptConfirmation.confirmedConceptRefs.${ref}`, message: "confirmedConceptRefs must reference a concept in conceptGrounding." });
    }
  }
}

function validateConceptSet(
  pathPrefix: string,
  conceptSet: NonNullable<BrainstormCandidate["conceptGrounding"]>["phaseConceptGrounding"] | NonNullable<BrainstormCandidate["conceptGrounding"]>["deliveryConceptGlossary"] | undefined,
  candidate: BrainstormCandidate,
  issues: Array<{ code: string; path: string; message: string }>,
): void {
  if (!conceptSet) {
    return;
  }
  if (conceptSet.mode === "concepts_present" && conceptSet.concepts.length === 0) {
    issues.push({ code: "CONCEPTS_REQUIRED", path: `${pathPrefix}.concepts`, message: "concepts_present requires at least one concept." });
  }
  if ((conceptSet.mode === "none_required" || conceptSet.mode === "not_applicable") && !conceptSet.reason?.trim()) {
    issues.push({ code: "CONCEPT_REASON_REQUIRED", path: `${pathPrefix}.reason`, message: "none_required/not_applicable requires a reason." });
  }
  const conceptIds = new Set<string>();
  const rankKeys = new Set<string>();
  const scopeIds = new Set([
    ...candidate.scope.included.map((item) => item.id),
    ...candidate.scope.excluded.map((item) => item.id),
    ...candidate.scope.deferred.map((item) => item.id),
  ]);
  const acceptanceIds = new Set(candidate.acceptance.map((item) => item.id));
  for (const concept of conceptSet.concepts) {
    if (conceptIds.has(concept.conceptId)) {
      issues.push({ code: "CONCEPT_ID_DUPLICATE", path: `${pathPrefix}.concepts.${concept.conceptId}`, message: "Concept ids must be unique within a concept set." });
    }
    conceptIds.add(concept.conceptId);
    const rankKey = `${concept.phaseRelevance}:${concept.priority}:${concept.attentionRank}`;
    if (rankKeys.has(rankKey)) {
      issues.push({ code: "CONCEPT_ATTENTION_RANK_DUPLICATE", path: `${pathPrefix}.concepts.${concept.conceptId}.attentionRank`, message: "attentionRank must be unique within each phaseRelevance + priority group." });
    }
    rankKeys.add(rankKey);
    for (const scopeRef of concept.scopeRefs) {
      if (!scopeIds.has(scopeRef)) {
        issues.push({ code: "CONCEPT_SCOPE_REF_INVALID", path: `${pathPrefix}.concepts.${concept.conceptId}.scopeRefs.${scopeRef}`, message: "Concept scopeRefs must reference candidate scope ids." });
      }
    }
    for (const acceptanceRef of concept.acceptanceRefs) {
      if (!acceptanceIds.has(acceptanceRef)) {
        issues.push({ code: "CONCEPT_ACCEPTANCE_REF_INVALID", path: `${pathPrefix}.concepts.${concept.conceptId}.acceptanceRefs.${acceptanceRef}`, message: "Concept acceptanceRefs must reference candidate acceptance ids." });
      }
    }
  }
}

function validateFrontendExperienceConfirmation(
  candidate: BrainstormCandidate,
  issues: Array<{ code: string; path: string; message: string }>,
): void {
  const progress = candidate.clarificationProgress;
  if (!progress) {
    return;
  }
  const frontendConfirmed = progress.confirmedBlocks.some((block) => block.block === "frontend_experience" && block.confirmedByUser);
  const frontendSkipped = progress.skippedBlocks.some((block) => block.block === "frontend_experience");
  if (frontendConfirmed && !candidate.frontendExperience && !candidate.frontendExperienceDelta) {
    issues.push({ code: "FRONTEND_TARGET_MISSING", path: "frontendExperience", message: "A confirmed frontend_experience block requires frontendExperience or frontendExperienceDelta." });
  }
  if (!frontendConfirmed && !frontendSkipped) {
    issues.push({ code: "FRONTEND_BLOCK_UNRESOLVED", path: "clarificationProgress", message: "frontend_experience block must be confirmed or explicitly skipped." });
  }
}

function allConceptIds(candidate: BrainstormCandidate): Set<string> {
  return new Set([
    ...(candidate.conceptGrounding?.deliveryConceptGlossary?.concepts ?? []).map((concept) => concept.conceptId),
    ...(candidate.conceptGrounding?.phaseConceptGrounding.concepts ?? []).map((concept) => concept.conceptId),
  ]);
}

function normalizeBrainstormCandidateForAccept(candidate: unknown, now: string): unknown {
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
    return candidate;
  }
  const normalized = structuredClone(candidate) as Record<string, unknown>;
  const userConfirmation = normalized.userConfirmation;
  if (!userConfirmation || typeof userConfirmation !== "object" || Array.isArray(userConfirmation)) {
    return normalized;
  }
  const record = userConfirmation as Record<string, unknown>;
  const confirmedAt = record.confirmedAt;
  if (confirmedAt === undefined || confirmedAt === null || confirmedAt === "") {
    record.confirmedAt = now;
    return normalized;
  }
  if (typeof confirmedAt === "string") {
    const parsed = new Date(confirmedAt);
    record.confirmedAt = Number.isNaN(parsed.getTime()) ? now : parsed.toISOString();
  }
  return normalized;
}

function normalizeBrainstormCandidateRoadmapForAccept(
  candidate: BrainstormCandidate,
  existing: BrainstormContract,
  activePhaseId: string,
): BrainstormCandidate {
  const existingPhases = existing.roadmap?.phases ?? [];
  if (existingPhases.length === 0) {
    return candidate;
  }
  const candidatePhaseIds = new Set(candidate.roadmap.phases.map((phase) => phase.phaseId));
  const priorPhases = existingPhases
    .filter((phase) => isPriorRoadmapPhase(phase, activePhaseId) && !candidatePhaseIds.has(phase.phaseId))
    .map((phase) => ({
      phaseId: phase.phaseId,
      title: phase.name,
      status: phase.status,
      goal: phase.goal,
      scopeRefs: uniqueStrings([
        ...phase.scope.includedRefs,
        ...phase.scope.deferredRefs,
        ...phase.scope.excludedRefs,
      ]),
      acceptanceRefs: phase.acceptanceRefs,
      dependsOn: phase.dependsOn,
    }));
  if (priorPhases.length === 0) {
    return candidate;
  }
  return brainstormCandidateSchema.parse({
    ...candidate,
    roadmap: {
      ...candidate.roadmap,
      phases: [
        ...priorPhases,
        ...candidate.roadmap.phases,
      ],
    },
  });
}

function normalizeBrainstormCandidateRoadmapRequirementForAccept(
  candidate: BrainstormCandidate,
  activePhaseId: string,
): BrainstormCandidate {
  const required = deriveRoadmapRequiredFromCandidate(candidate, activePhaseId);
  if (candidate.roadmap.required === required) {
    return candidate;
  }
  return brainstormCandidateSchema.parse({
    ...candidate,
    roadmap: {
      ...candidate.roadmap,
      required,
    },
  });
}

function deriveRoadmapRequiredFromCandidate(
  candidate: BrainstormCandidate,
  activePhaseId: string,
): boolean {
  if (candidate.scope.deferred.length > 0) {
    return true;
  }
  if (candidate.phasePlan.nextPhasePreview.kind === "candidate") {
    return true;
  }
  return hasFutureProposedPhase(candidate, activePhaseId);
}

function hasFutureProposedPhase(
  candidate: BrainstormCandidate,
  activePhaseId: string,
): boolean {
  const currentPhaseId = candidate.roadmap.currentPhaseId || activePhaseId;
  return candidate.roadmap.phases.some((phase) =>
    phase.phaseId !== currentPhaseId && phase.status === "proposed"
  );
}

function isPriorRoadmapPhase(
  phase: NonNullable<BrainstormContract["roadmap"]>["phases"][number],
  activePhaseId: string,
): boolean {
  if (phase.phaseId === activePhaseId) {
    return false;
  }
  const phaseOrdinal = phaseOrdinalFromId(phase.phaseId);
  const activeOrdinal = phaseOrdinalFromId(activePhaseId);
  if (phaseOrdinal !== null && activeOrdinal !== null) {
    return phaseOrdinal < activeOrdinal;
  }
  return phase.status !== "proposed";
}

function phaseOrdinalFromId(phaseId: string): number | null {
  const match = /^phase-(\d+)$/.exec(phaseId);
  if (!match) {
    return null;
  }
  return Number.parseInt(match[1] ?? "", 10);
}

async function writeBrainstormDerivedArtifacts(
  projectRoot: string,
  locator: { deliveryId: string; phaseId: string },
  candidate: BrainstormCandidate,
  now: string,
): Promise<{
  conceptGroundingRefs: NonNullable<BrainstormContract["conceptGroundingRefs"]>;
  frontendExperienceRefs: NonNullable<BrainstormContract["frontendExperienceRefs"]>;
}> {
  const deliveryGlossaryAbs = deliveryConceptGlossaryPath(projectRoot, locator.deliveryId);
  const phaseConceptsAbs = phaseConceptGroundingPath(projectRoot, locator.deliveryId, locator.phaseId);
  const confirmedFrontendAbs = confirmedFrontendExperienceTargetPath(projectRoot, locator.deliveryId, locator.phaseId);
  const currentFrontendAbs = currentFrontendExperienceTargetPath(projectRoot, locator.deliveryId);

  if (!candidate.conceptGrounding) {
    throw stateCorrupted("Confirmed BrainstormCandidate is missing conceptGrounding.");
  }

  const previousDeliveryGlossary = await readOptionalRecord(deliveryGlossaryAbs);
  const deliveryGlossary = candidate.conceptGrounding.deliveryConceptGlossary
    ? {
      schemaVersion: "1.0",
      deliveryId: locator.deliveryId,
      updatedAt: now,
      ...candidate.conceptGrounding.deliveryConceptGlossary,
    }
    : applyGlossaryUpdates(previousDeliveryGlossary, candidate.conceptGrounding.glossaryUpdates ?? [], locator.deliveryId, now);
  if (deliveryGlossary) {
    await writeJsonAtomic(deliveryGlossaryAbs, deliveryGlossary);
  }
  await writeJsonAtomic(phaseConceptsAbs, {
    schemaVersion: "1.0",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    updatedAt: now,
    ...candidate.conceptGrounding.phaseConceptGrounding,
  });

  let confirmedFrontendExperienceRef: string | null = null;
  let currentFrontendExperienceRef: string | null = null;
  if (candidate.frontendExperience || candidate.frontendExperienceDelta) {
    const previousCurrentFrontend = await readOptionalRecord(currentFrontendAbs);
    const inheritedFrontendExperience =
      candidate.frontendExperience
        ? null
        : (previousCurrentFrontend?.frontendExperience ?? previousCurrentFrontend?.inheritedFrontendExperience ?? null);
    const target = {
      schemaVersion: "1.0",
      deliveryId: locator.deliveryId,
      phaseId: locator.phaseId,
      updatedAt: now,
      frontendExperience: candidate.frontendExperience ?? null,
      frontendExperienceDelta: candidate.frontendExperienceDelta ?? null,
      ...(inheritedFrontendExperience ? { inheritedFrontendExperience } : {}),
      source: "brainstorm_user_confirmed",
    };
    await writeJsonAtomic(confirmedFrontendAbs, target);
    await writeJsonAtomic(currentFrontendAbs, {
      ...target,
      currentPhaseId: locator.phaseId,
      confirmedFrontendExperienceRef: toProjectRelative(projectRoot, confirmedFrontendAbs),
    });
    confirmedFrontendExperienceRef = toProjectRelative(projectRoot, confirmedFrontendAbs);
    currentFrontendExperienceRef = toProjectRelative(projectRoot, currentFrontendAbs);
  } else if (await pathExists(currentFrontendAbs)) {
    currentFrontendExperienceRef = toProjectRelative(projectRoot, currentFrontendAbs);
  }

  return {
    conceptGroundingRefs: {
      deliveryConceptGlossaryRef: await pathExists(deliveryGlossaryAbs)
        ? toProjectRelative(projectRoot, deliveryGlossaryAbs)
        : null,
      phaseConceptGroundingRef: toProjectRelative(projectRoot, phaseConceptsAbs),
    },
    frontendExperienceRefs: {
      confirmedFrontendExperienceRef,
      currentFrontendExperienceRef,
    },
  };
}

async function readOptionalRecord(file: string): Promise<Record<string, unknown> | null> {
  if (!(await pathExists(file))) {
    return null;
  }
  const value = await readJsonFile(file);
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function applyGlossaryUpdates(
  previous: Record<string, unknown> | null,
  updates: NonNullable<NonNullable<BrainstormCandidate["conceptGrounding"]>["glossaryUpdates"]>,
  deliveryId: string,
  now: string,
): Record<string, unknown> | null {
  if (!previous || updates.length === 0) {
    return previous;
  }
  const concepts = Array.isArray(previous.concepts) ? [...previous.concepts] : [];
  const indexById = new Map<string, number>();
  concepts.forEach((concept, index) => {
    if (concept && typeof concept === "object" && !Array.isArray(concept)) {
      const conceptId = (concept as Record<string, unknown>).conceptId;
      if (typeof conceptId === "string") {
        indexById.set(conceptId, index);
      }
    }
  });

  for (const update of updates) {
    const targetId = update.conceptRef ?? update.concept?.conceptId;
    if (!targetId) {
      continue;
    }
    if (update.operation === "remove") {
      const index = indexById.get(targetId);
      if (index !== undefined) {
        concepts.splice(index, 1);
        indexById.clear();
        concepts.forEach((concept, nextIndex) => {
          if (concept && typeof concept === "object" && !Array.isArray(concept)) {
            const conceptId = (concept as Record<string, unknown>).conceptId;
            if (typeof conceptId === "string") {
              indexById.set(conceptId, nextIndex);
            }
          }
        });
      }
      continue;
    }
    if (!update.concept) {
      continue;
    }
    const existingIndex = indexById.get(targetId);
    if (existingIndex === undefined) {
      concepts.push(update.concept);
      indexById.set(update.concept.conceptId, concepts.length - 1);
    } else {
      concepts[existingIndex] = update.concept;
      indexById.set(update.concept.conceptId, existingIndex);
    }
  }

  return {
    ...previous,
    schemaVersion: "1.0",
    deliveryId,
    updatedAt: now,
    concepts,
  };
}

function brainstormContractFromCandidate(
  existing: BrainstormContract,
  candidate: BrainstormCandidate,
  phaseId: string,
  now: string,
): BrainstormContract {
  const currentPhaseId = candidate.roadmap.currentPhaseId || phaseId;
  const roadmapRequired = deriveRoadmapRequiredFromCandidate(candidate, phaseId);
  const includedIds = new Set(candidate.scope.included.map((item) => item.id));
  const deferredIds = new Set(candidate.scope.deferred.map((item) => item.id));
  const excludedIds = new Set(candidate.scope.excluded.map((item) => item.id));
  const roadmapId = existing.roadmap?.roadmapId ?? createId("rm");
  const roadmap: NonNullable<BrainstormContract["roadmap"]> = {
    roadmapId,
    status: "active" as const,
    strategy: "multi_phase" as const,
    reason: roadmapRequired
      ? "Agent-managed Brainstorm confirmed a multi-phase roadmap."
      : "Internal phase model is used even for a single-phase delivery.",
    currentPhaseId,
    recommendedPhaseId: currentPhaseId,
    phases: candidate.roadmap.phases.map((phase) => ({
      phaseId: phase.phaseId,
      name: phase.name ?? phase.title ?? phase.phaseId,
      status: phase.status,
      goal: phase.goal ?? phase.title ?? candidate.requestSummary.oneLine,
      scope: {
        includedRefs: currentBucketRefs(phase, includedIds, currentPhaseId),
        deferredRefs: currentBucketRefs(phase, deferredIds, currentPhaseId),
        excludedRefs: currentBucketRefs(phase, excludedIds, currentPhaseId),
      },
      acceptanceRefs: phase.acceptanceRefs,
      dependsOn: phase.dependsOn ?? [],
      handoff: {
        readyForPlanning: phase.phaseId === currentPhaseId && phase.status === "scope_confirmed",
        planningContractId: phase.phaseId === currentPhaseId ? `pgc-${phase.phaseId}` : null,
        planId: null,
      },
      confirmation: {
        confirmedBy: phase.phaseId === currentPhaseId ? "user" : null,
        confirmedAt: phase.phaseId === currentPhaseId ? candidate.userConfirmation.confirmedAt ?? now : null,
        sourcePatchIds: [],
      },
      nextActions: [],
    })),
    nextActions: [],
  };
  roadmap.nextActions = buildRoadmapNextActions(roadmap.phases);

  return brainstormContractSchema.parse({
    ...existing,
    brainstormRunId: candidate.brainstormRunId,
    status: "confirmed",
    sources: candidate.sources ?? existing.sources,
    summary: {
      title: candidate.requestSummary.title,
      oneLine: candidate.requestSummary.oneLine,
      businessGoal: candidate.requestSummary.businessGoal ?? candidate.requestSummary.oneLine,
      complexity: candidate.requestSummary.complexity,
    },
    domainModel: candidate.domainModel ?? existing.domainModel,
    scope: {
      included: candidate.scope.included,
      deferred: candidate.scope.deferred,
      excluded: candidate.scope.excluded,
      assumptions: candidate.scope.assumptions ?? [],
    },
    acceptance: {
      candidates: candidate.acceptance,
      coverageNotes: ["BrainstormCandidate accepted from Agent-managed conversation."],
    },
    deliveryStrategy: {
      mode: roadmapRequired ? "roadmap" : "single_phase",
      reason: roadmapRequired
        ? "Agent-managed Brainstorm confirmed a roadmap delivery."
        : "Agent-managed Brainstorm confirmed a single-phase delivery using phase-1.",
      recommendedCurrentPhaseId: currentPhaseId,
    },
    clarification: {
      ...existing.clarification,
      status: "confirmed",
      pendingQuestionIds: [],
      pendingConfirmationIds: [],
    },
    roadmap,
    phasePlan: candidate.phasePlan,
    handoff: {
      ready: true,
      nextNode: "planning_generation_contract",
      blockingReasons: [],
      confirmedAt: candidate.userConfirmation.confirmedAt ?? now,
    },
    updatedAt: now,
  });
}

function currentBucketRefs(
  phase: BrainstormCandidate["roadmap"]["phases"][number],
  bucketIds: Set<string>,
  currentPhaseId: string,
): string[] {
  if (phase.phaseId !== currentPhaseId) {
    return (phase.scopeRefs ?? []).filter((ref) => bucketIds.has(ref));
  }
  const scopedRefs = phase.scopeRefs?.filter((ref) => bucketIds.has(ref));
  return scopedRefs?.length ? scopedRefs : [...bucketIds];
}

async function writeBrainstormDecisionSnapshot(
  projectRoot: string,
  locator: { deliveryId: string; phaseId: string },
  contract: BrainstormContract,
  candidate: BrainstormCandidate,
  acceptedAt: string,
  requestId?: string,
): Promise<{ brainstormDecision: string; brainstormDecisionsIndex: string }> {
  const decisionRef = toProjectRelative(projectRoot, brainstormDecisionPath(projectRoot, locator.deliveryId, locator.phaseId));
  const indexRef = toProjectRelative(projectRoot, brainstormDecisionsIndexPath(projectRoot, locator.deliveryId));
  const requestContextRefs = requestId
    ? await readBrainstormRequestContextRefs(projectRoot, locator.deliveryId, requestId)
    : {};
  const sourceRefs = {
    originalRequirementContextRef: firstString(requestContextRefs.originalRequirementContextRef, requestContextRefs.requirementContextRef),
    requirementContextRef: firstString(requestContextRefs.requirementContextRef, requestContextRefs.originalRequirementContextRef),
    normalizedRequirementTextRef: firstString(requestContextRefs.normalizedRequirementTextRef),
    keywordHintsRef: firstString(requestContextRefs.keywordHintsRef),
  };
  const snapshot = {
    schemaVersion: "1.0",
    artifactType: "brainstorm_phase_decision",
    deliveryId: locator.deliveryId,
    phaseId: locator.phaseId,
    brainstormRunId: contract.brainstormRunId,
    contractId: contract.contractId,
    acceptedAt,
    sources: contract.sources,
    sourceRefs,
    summary: contract.summary,
    scope: contract.scope,
    acceptance: contract.acceptance,
    domainModel: contract.domainModel,
    conceptGrounding: contract.conceptGrounding ?? null,
    conceptConfirmation: contract.conceptConfirmation ?? null,
    clarificationProgress: contract.clarificationProgress ?? null,
    frontendExperience: contract.frontendExperience ?? null,
    frontendExperienceDelta: contract.frontendExperienceDelta ?? null,
    phasePlan: contract.phasePlan,
    userConfirmation: candidate.userConfirmation,
  };
  await writeJsonAtomic(path.resolve(projectRoot, decisionRef), snapshot);

  const existingIndex = await readBrainstormDecisionsIndex(projectRoot, locator.deliveryId);
  const decisions = [
    ...existingIndex.decisions.filter((item) => item.phaseId !== locator.phaseId),
    {
      phaseId: locator.phaseId,
      decisionRef,
      brainstormRunId: contract.brainstormRunId,
      contractId: contract.contractId,
      acceptedAt,
      title: contract.phasePlan.current.title,
      goal: contract.phasePlan.current.goal,
      scopeLabels: [
        ...contract.scope.included.map((item) => item.label),
        ...contract.scope.deferred.map((item) => item.label),
        ...contract.scope.excluded.map((item) => item.label),
      ],
      acceptanceStatements: contract.acceptance.candidates.map((item) => item.statement),
      nextPhasePreview: contract.phasePlan.nextPhasePreview,
    },
  ];
  await writeJsonAtomic(path.resolve(projectRoot, indexRef), {
    schemaVersion: "1.0",
    artifactType: "brainstorm_phase_decisions_index",
    deliveryId: locator.deliveryId,
    latestConfirmedPhaseId: locator.phaseId,
    updatedAt: acceptedAt,
    decisions,
  });
  return {
    brainstormDecision: decisionRef,
    brainstormDecisionsIndex: indexRef,
  };
}

async function readBrainstormRequestContextRefs(
  projectRoot: string,
  deliveryId: string,
  requestId: string,
): Promise<Record<string, unknown>> {
  const requestPath = brainstormSessionRequestPath(projectRoot, deliveryId, requestId);
  if (!(await pathExists(requestPath))) {
    return {};
  }
  const request = await readJsonFile(requestPath);
  if (!isRecord(request) || !isRecord(request.contextRefs)) {
    return {};
  }
  return request.contextRefs;
}

async function readBrainstormDecisionsIndex(
  projectRoot: string,
  deliveryId: string,
): Promise<{ decisions: Array<Record<string, unknown> & { phaseId: string }> }> {
  const indexPath = brainstormDecisionsIndexPath(projectRoot, deliveryId);
  if (!(await pathExists(indexPath))) {
    return { decisions: [] };
  }
  const index = await readJsonFile(indexPath);
  if (!isRecord(index) || !Array.isArray(index.decisions)) {
    return { decisions: [] };
  }
  return {
    decisions: index.decisions.filter((item): item is Record<string, unknown> & { phaseId: string } =>
      isRecord(item) && typeof item.phaseId === "string" && item.phaseId.length > 0
    ),
  };
}

function firstString(...values: unknown[]): string | null {
  return values.find((value): value is string => typeof value === "string" && value.length > 0) ?? null;
}

async function invalidBrainstormAcceptResult(
  input: AcceptBrainstormCandidateInput,
  issues: Array<{ code: string; path: string; message: string }>,
): Promise<BrainstormAcceptResult> {
  const root = path.resolve(input.projectRoot);
  return {
    accepted: false,
    deliveryId: input.deliveryId ?? "unknown",
    phaseId: input.phaseId ?? "unknown",
    brainstormRunId: input.brainstormRunId ?? "unknown",
    contractId: null,
    status: "blocked",
    issues,
    contractPath: null,
    routeDecision: {
      type: "brainstorm_clarification",
      reason: "BRAINSTORM_CANDIDATE_SCHEMA_INVALID",
    },
    repairInstruction: brainstormRepairInstruction(root, {
      deliveryId: input.deliveryId ?? "unknown",
      phaseId: input.phaseId ?? "unknown",
      brainstormRunId: input.brainstormRunId ?? "unknown",
      requestId: input.requestId,
      candidateFile: input.candidateFile,
      issues,
    }),
  };
}

function brainstormRepairInstruction(
  root: string,
  input: {
    deliveryId: string;
    phaseId: string;
    brainstormRunId: string;
    requestId?: string;
    candidateFile: string;
    issues: Array<{ code: string; path: string; message: string }>;
  },
): Record<string, unknown> {
  return {
    mode: "repair_candidate",
    schema: "BrainstormCandidate",
    ...artifactRepairPolicy(),
    candidateFile: toProjectRelative(root, resolveCliPath(root, input.candidateFile)),
    issues: input.issues,
    repairSubmitRouting: repairSubmitRouting({
      kind: "candidate",
      submitCommandName: "brainstorm accept",
    }),
    schemaShape: brainstormCandidateSchemaShape({
      deliveryId: input.deliveryId,
      phaseId: input.phaseId,
      brainstormRunId: input.brainstormRunId,
    }),
    enumRefs: brainstormEnumRefs(),
    instructions: [
      compactContextReadStep,
      "Repair only the BrainstormCandidate JSON contract fields.",
      "Preserve phase-1 in roadmap.phases. For Phase N, roadmap.phases must include prior phases plus the current phase.",
      "phasePlan.current.scopeRefs must contain only ids from scope.included. Remove any scope.excluded or scope.deferred ids from phasePlan.current.scopeRefs.",
      "phasePlan.current.acceptanceRefs must contain only ids from acceptance.",
      "If scope.deferred is non-empty, phasePlan.nextPhasePreview.kind must be candidate. Build its title, goal, and scopePreview from the deferred scope items; do not use kind=none to mean future priority is undecided.",
      "Use phasePlan.nextPhasePreview.kind=none only when the confirmed candidate has no deferred scope and no later roadmap phase.",
      "If issues mention confirmationBasis, clarificationProgress, conceptConfirmation, or frontend_experience confirmation, do not fabricate confirmation fields. Present the missing confirmation block to the user, wait for explicit confirmation, then update the same candidateFile.",
      "If repairing only schema formatting, refs, enum values, or candidate structure without changing user-visible scope/acceptance/phasePlan/concepts/frontend target, preserve existing confirmationBasis and clarificationProgress.",
      "Do not modify project source code.",
      "Do not modify TechnicalBaseline, PGC, AAC, TaskPlan, TaskResult, or ReviewResult.",
      "Return a complete replacement BrainstormCandidate to the same candidateFile.",
      "Run brainstorm accept again with the same delivery-id, phase-id, run-id, request-id when present, and candidate-file.",
      "Do not run loom continue before brainstorm accept succeeds.",
    ],
    submitCommand: {
      name: "brainstorm accept",
      argv: [
        "brainstorm",
        "accept",
        "--delivery-id",
        input.deliveryId,
        "--phase-id",
        input.phaseId,
        ...(input.requestId ? ["--request-id", input.requestId] : []),
        "--run-id",
        input.brainstormRunId,
        "--candidate-file",
        toProjectRelative(root, resolveCliPath(root, input.candidateFile)),
      ],
    },
  };
}

function resolveCliPath(projectRoot: string, filePath: string): string {
  return path.isAbsolute(filePath) ? filePath : path.resolve(projectRoot, filePath);
}

function duplicates(values: string[]): string[] {
  const seen = new Set<string>();
  const dupes = new Set<string>();
  for (const value of values) {
    if (seen.has(value)) {
      dupes.add(value);
    } else {
      seen.add(value);
    }
  }
  return [...dupes];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

async function saveBrainstormContract(projectRoot: string, contract: BrainstormContract, deliveryId: string): Promise<void> {
  const parsed = brainstormContractSchema.parse(contract);
  await writeJsonAtomic(brainstormContractPath(projectRoot, deliveryId), parsed);
}

async function writeLatestPointer(projectRoot: string, deliveryId: string, brainstormRunId: string): Promise<void> {
  await writeJsonAtomic(brainstormLatestPath(projectRoot, deliveryId), {
    schemaVersion: "1.0",
    brainstormRunId,
    contractRef: toProjectRelative(projectRoot, brainstormContractPath(projectRoot, deliveryId)),
    updatedAt: new Date().toISOString(),
  });
}

async function createDeliveryIndex(
  projectRoot: string,
  deliveryId: string,
  phaseId: string,
  title: string,
  contract: BrainstormContract,
  now: string,
): Promise<void> {
  const index: DeliveryIndex = deliveryIndexSchema.parse({
    schemaVersion: "1.0",
    deliveryId,
    status: "brainstorming",
    requestSummary: title,
    roadmapId: contract.roadmap?.roadmapId ?? null,
    activePhaseId: phaseId,
    phases: deliveryPhasesFromBrainstorm(projectRoot, deliveryId, phaseId, contract),
    createdAt: now,
    updatedAt: now,
  });
  await writeJsonAtomic(deliveryIndexPath(projectRoot, deliveryId), index);
}

async function updateProjectStatusAfterConfirmation(
  projectRoot: string,
  deliveryId: string,
  phaseId: string,
  contractId: string,
  requestSummary: string,
  now: string,
): Promise<void> {
  try {
    const status = await loadProjectStatus(projectRoot);
    const deliveries = [...(status.deliveries ?? [])];
    const deliveryEntry = {
      deliveryId,
      status: "planning" as const,
      requestSummary,
      activePhaseId: phaseId,
      indexRef: toProjectRelative(projectRoot, deliveryIndexPath(projectRoot, deliveryId)),
      updatedAt: now,
    };
    const existingIndex = deliveries.findIndex((item) => item.deliveryId === deliveryId);
    if (existingIndex >= 0) {
      deliveries[existingIndex] = deliveryEntry;
    } else {
      deliveries.push(deliveryEntry);
    }
    status.activeDeliveryId = deliveryId;
    status.deliveries = deliveries;
    status.effectiveNextAction = {
      type: "technical_baseline_request",
      source: "brainstorm_contract",
      deliveryId,
      phaseId,
      ref: toProjectRelative(projectRoot, brainstormContractPath(projectRoot, deliveryId)),
      reason: "BRAINSTORM_CONFIRMED",
    };
    status.phase = "planning";
    status.current.requirementId = contractId;
    status.lastAction = "brainstorm.confirm";
    status.nextAction = "plan";
    status.updatedAt = now;
    await saveProjectStatus(projectRoot, status);
  } catch (error) {
    if (error instanceof LoomError) {
      throw error;
    }
    if (error instanceof ZodError) {
      throw stateCorrupted("loom status file does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

async function updateProjectStatusForActiveDelivery(
  projectRoot: string,
  deliveryId: string,
  phaseId: string,
  requestSummary: string,
  deliveryStatus: DeliveryIndex["status"],
  effectiveNextAction: NonNullable<DeliveryIndex["phases"][number]["nextAction"]>,
  now: string,
): Promise<void> {
  const status = await loadProjectStatus(projectRoot);
  const deliveries = [...(status.deliveries ?? [])];
  const deliveryEntry = {
    deliveryId,
    status: deliveryStatus,
    requestSummary,
    activePhaseId: phaseId,
    indexRef: toProjectRelative(projectRoot, deliveryIndexPath(projectRoot, deliveryId)),
    updatedAt: now,
  };
  const existingIndex = deliveries.findIndex((item) => item.deliveryId === deliveryId);
  if (existingIndex >= 0) {
    deliveries[existingIndex] = deliveryEntry;
  } else {
    deliveries.push(deliveryEntry);
  }
  status.activeDeliveryId = deliveryId;
  status.deliveries = deliveries;
  status.effectiveNextAction = effectiveNextAction;
  status.phase = "planning";
  status.lastAction = "brainstorm.start";
  status.nextAction = "plan";
  status.updatedAt = now;
  await saveProjectStatus(projectRoot, status);
}

async function updateDeliveryAfterBrainstormConfirmation(
  projectRoot: string,
  deliveryId: string,
  phaseId: string,
  contract: BrainstormContract,
  now: string,
  decisionRefs?: { brainstormDecision: string; brainstormDecisionsIndex: string },
): Promise<void> {
  const index = await loadDeliveryIndex(projectRoot, deliveryId);
  index.status = "planning";
  index.roadmapId = contract.roadmap?.roadmapId ?? null;
  index.activePhaseId = phaseId;
  index.phases = mergeDeliveryPhasesFromBrainstorm(projectRoot, deliveryId, phaseId, contract, index.phases);
  index.updatedAt = now;
  updatePhase(index, phaseId, {
    status: "scope_confirmed",
    latestRefs: {
      ...index.phases.find((phase) => phase.phaseId === phaseId)?.latestRefs,
      brainstormContract: toProjectRelative(projectRoot, brainstormContractPath(projectRoot, deliveryId)),
      ...(decisionRefs ?? {}),
    },
    nextAction: {
      type: "technical_baseline_request",
      source: "brainstorm_contract",
      deliveryId,
      phaseId,
      ref: toProjectRelative(projectRoot, brainstormContractPath(projectRoot, deliveryId)),
      reason: "BRAINSTORM_CONFIRMED",
    },
  });
  await saveDeliveryIndex(projectRoot, index);
  await upsertStatusDelivery(projectRoot, index);
}

function mergeDeliveryPhasesFromBrainstorm(
  projectRoot: string,
  deliveryId: string,
  activePhaseId: string,
  contract: BrainstormContract,
  existingPhases: DeliveryIndexPhase[],
): DeliveryIndexPhase[] {
  const phases = [...existingPhases];
  const activePhase = deliveryPhasesFromBrainstorm(projectRoot, deliveryId, activePhaseId, contract)[0];
  const existingIndex = phases.findIndex((item) => item.phaseId === activePhaseId);
  if (existingIndex >= 0) {
    const existing = phases[existingIndex];
    phases[existingIndex] = {
      ...activePhase,
      latestRefs: {
        ...existing.latestRefs,
        ...activePhase.latestRefs,
      },
      nextAction: activePhase.nextAction,
    };
  } else {
    phases.push(activePhase);
  }
  return phases;
}

function deliveryPhasesFromBrainstorm(
  projectRoot: string,
  deliveryId: string,
  activePhaseId: string,
  contract: BrainstormContract,
): DeliveryIndexPhase[] {
  const brainstormRef = toProjectRelative(projectRoot, brainstormContractPath(projectRoot, deliveryId));
  const roadmapPhase = contract.roadmap?.phases.find((phase) => phase.phaseId === activePhaseId);
  const currentPhase = contract.phasePlan.current.phaseId === activePhaseId
    ? contract.phasePlan.current
    : null;
  const phaseName =
    roadmapPhase?.name ??
    currentPhase?.title ??
    activePhaseId;
  return [{
    phaseId: activePhaseId,
    name: phaseName,
    status: contract.status === "confirmed" ? "scope_confirmed" : "pending",
    latestRefs: {
      brainstormContract: brainstormRef,
      ...(contract.conceptGroundingRefs?.deliveryConceptGlossaryRef ? {
        deliveryConceptGlossary: contract.conceptGroundingRefs.deliveryConceptGlossaryRef,
      } : {}),
      ...(contract.conceptGroundingRefs?.phaseConceptGroundingRef ? {
        phaseConceptGrounding: contract.conceptGroundingRefs.phaseConceptGroundingRef,
      } : {}),
      ...(contract.frontendExperienceRefs?.confirmedFrontendExperienceRef ? {
        confirmedFrontendExperience: contract.frontendExperienceRefs.confirmedFrontendExperienceRef,
      } : {}),
      ...(contract.frontendExperienceRefs?.currentFrontendExperienceRef ? {
        currentFrontendExperience: contract.frontendExperienceRefs.currentFrontendExperienceRef,
      } : {}),
    },
    nextAction: {
      type: contract.status === "needs_clarification" ? "brainstorm_clarification" : "brainstorm_confirmation",
      source: "brainstorm_contract",
      deliveryId,
      phaseId: activePhaseId,
      ref: brainstormRef,
      reason: contract.status === "needs_clarification" ? "BRAINSTORM_NEEDS_CLARIFICATION" : "BRAINSTORM_NEEDS_CONFIRMATION",
    },
  }];
}

function applyPatchOperations(
  contract: BrainstormContract,
  operations: BrainstormContract["clarification"]["patches"][number]["operations"],
): {
  included: string[];
  deferred: string[];
  excluded: string[];
} {
  const refs = {
    included: [] as string[],
    deferred: [] as string[],
    excluded: [] as string[],
  };
  for (const operation of operations) {
    if (operation.op !== "add") {
      continue;
    }
    if (operation.path === "/scope/included") {
      const value = operation.value as BrainstormContract["scope"]["included"][number];
      contract.scope.included.push(value);
      refs.included.push(value.id);
    }
    if (operation.path === "/scope/deferred") {
      const value = operation.value as BrainstormContract["scope"]["deferred"][number];
      contract.scope.deferred.push(value);
      refs.deferred.push(value.id);
    }
    if (operation.path === "/scope/excluded") {
      const value = operation.value as BrainstormContract["scope"]["excluded"][number];
      contract.scope.excluded.push(value);
      refs.excluded.push(value.id);
    }
  }
  return refs;
}

function buildRoadmapNextActions(
  phases: NonNullable<BrainstormContract["roadmap"]>["phases"],
): NonNullable<BrainstormContract["roadmap"]>["nextActions"] {
  const nextPhase = phases.find((phase) => phase.status === "proposed");
  return [
    {
      actionId: "continue-planning",
      type: "continue_planning",
      label: "进入当前阶段规划",
      recommended: true,
      phaseId: "phase-1",
      userPrompt: "当前阶段范围已确认，可以继续生成规划契约。",
    },
    ...(nextPhase
      ? [
          {
            actionId: `confirm-${nextPhase.phaseId}`,
            type: "confirm_next_phase" as const,
            label: `后续确认 ${nextPhase.name}`,
            recommended: false,
            phaseId: nextPhase.phaseId,
            userPrompt: `当前阶段完成后，可以继续确认 ${nextPhase.name}。`,
          },
        ]
      : []),
    {
      actionId: "revise-roadmap",
      type: "revise_roadmap",
      label: "调整后续路线图",
      recommended: false,
      userPrompt: "可以调整后续阶段范围或顺序。",
    },
    {
      actionId: "deploy-current",
      type: "deploy_current",
      label: "部署当前版本预览",
      recommended: false,
      userPrompt: "可以先部署当前阶段做预览。",
    },
  ];
}

function pendingQuestions(contract: BrainstormContract): BrainstormContract["clarification"]["questions"] {
  return contract.clarification.questions.filter((question) =>
    contract.clarification.pendingQuestionIds.includes(question.questionId),
  );
}

function pendingConfirmations(contract: BrainstormContract): BrainstormContract["clarification"]["confirmations"] {
  return contract.clarification.confirmations.filter((confirmation) =>
    contract.clarification.pendingConfirmationIds.includes(confirmation.confirmationId),
  );
}

async function contractPathFor(projectRoot: string, brainstormRunId: string): Promise<string> {
  const { deliveryId } = await getLocatorForBrainstormRun(projectRoot, brainstormRunId);
  return brainstormContractPath(projectRoot, deliveryId);
}

function createId(prefix: string): string {
  return `${prefix}-${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${Math.random()
    .toString(36)
    .slice(2, 8)}`;
}

function pad(value: number): string {
  return value.toString().padStart(3, "0");
}

function hash(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function shortHash(value: string): string {
  return hash(value).slice(0, 10);
}

function truncateText(value: string, maxLength: number): string {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= maxLength) {
    return normalized;
  }
  return normalized.slice(0, Math.max(1, maxLength - 1)).trim();
}

function splitScopeItems(answerText: string): string[] {
  const items = answerText
    .split(/[，,、;；\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
  return items.length > 1 ? items : [answerText.trim()];
}

function interpretScopeAnswer(answerText: string): {
  includedItems: string[];
  deferredItems: string[];
} {
  const includedItems: string[] = [];
  const deferredItems: string[] = [];
  const clauses = splitScopeClauses(answerText)
    .split(/[；;\n。]/)
    .map((clause) => clause.trim())
    .filter(Boolean);

  for (const clause of clauses.length > 0 ? clauses : [answerText]) {
    const clauseBucket = scopeBucketForClause(clause);
    for (const item of splitScopeItems(clause)) {
      const bucket = scopeBucketForItem(item, clauseBucket) === "deferred" ? deferredItems : includedItems;
      const cleaned = cleanScopeItem(item);
      if (cleaned) {
        bucket.push(cleaned);
      }
    }
  }

  return {
    includedItems: uniqueStrings(includedItems),
    deferredItems: uniqueStrings(deferredItems),
  };
}

function splitScopeClauses(answerText: string): string {
  return answerText
    .replace(/(明确)?(暂缓|延后|后续阶段再做|后续再做|后续|先不做|不做|暂不包含|不包含)\s*[:：]/g, "。$2：")
    .replace(/(范围包含|包含|仅包含|只包含|当前阶段包含|phase-1\s*范围包含|Phase-1\s*范围包含)\s*[:：]/g, "。$1：");
}

function scopeBucketForClause(clause: string): "included" | "deferred" | null {
  const trimmed = clause.trim();
  if (/^(暂缓|延后|后续阶段再做|后续再做|后续|先不做|不做|暂不包含|不包含)\s*[:：]/.test(trimmed)) {
    return "deferred";
  }
  if (/^(范围包含|包含|仅包含|只包含|当前阶段包含|phase-1\s*范围包含|Phase-1\s*范围包含)\s*[:：]/.test(trimmed)) {
    return "included";
  }
  return null;
}

function scopeBucketForItem(item: string, clauseBucket: "included" | "deferred" | null): "included" | "deferred" {
  if (hasDeferredMarker(item)) {
    return "deferred";
  }
  return clauseBucket ?? "included";
}

function hasDeferredMarker(value: string): boolean {
  return /后续|后面|以后|暂缓|延后|先不做|不做/.test(value);
}

function cleanScopeItem(value: string): string {
  return value
    .replace(/^(范围包含|包含|仅包含|只包含|当前阶段包含|phase-1\s*范围包含|Phase-1\s*范围包含)\s*[:：]/, "")
    .replace(/^(暂缓|延后|后续阶段再做|后续再做|后续|先不做|不做|暂不包含|不包含)\s*[:：]/, "")
    .replace(/^(先只做|只做|先做|包括|当前阶段|第一阶段|先|做|加上|加入)/, "")
    .replace(/(后续阶段再做|后续再做|后面再说|以后再做|暂缓|延后|先不做|不做|后续阶段|后续|再做)$/g, "")
    .replace(/^和/, "")
    .replace(/[。；;，,、\s]+$/g, "")
    .trim();
}

function uniqueStrings(values: string[]): string[] {
  return [...new Set(values.filter(Boolean))];
}

function inferSelectedOptions(
  answerText: string,
  question: BrainstormContract["clarification"]["questions"][number],
): string[] {
  const lower = answerText.toLowerCase();
  return question.suggestedOptions
    .filter((option) => lower.includes(option.optionId.toLowerCase()) || answerText.includes(option.label))
    .map((option) => option.optionId);
}

export async function toProjectRelativeContractPath(projectRoot: string, brainstormRunId: string): Promise<string> {
  return toProjectRelative(projectRoot, await contractPathFor(projectRoot, brainstormRunId));
}
