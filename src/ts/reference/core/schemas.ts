import { z } from "zod";

export const loomConfigV1Schema = z.object({
  schemaVersion: z.literal(1),
  project: z.object({
    name: z.string().min(1),
    createdAtRoot: z.string().min(1),
  }),
  defaults: z.object({
    language: z.enum(["auto", "zh", "en"]),
    mode: z.enum(["plan", "build"]),
    verificationLevel: z.enum(["light", "standard", "strict"]),
  }),
  git: z.object({
    policy: z.literal("local"),
  }),
  features: z.object({
    plan: z.boolean(),
    build: z.boolean(),
    review: z.boolean(),
    repair: z.boolean(),
    deploy: z.boolean(),
  }),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type LoomConfigV1 = z.infer<typeof loomConfigV1Schema>;

export const routeActionTypeSchema = z.enum([
  "brainstorm_start",
  "brainstorm_clarification",
  "brainstorm_confirmation",
  "technical_baseline_request",
  "repository_context_request",
  "planning_contract_create",
  "architecture_artifact_contract",
  "taskplan_generation",
  "continue_execution",
  "review",
  "execution_repair",
  "task_result_repair",
  "taskplan_repair",
  "architecture_artifact_repair",
  "needs_user_decision",
  "manual_review",
  "continue_to_next_phase",
  "done",
]);

export type RouteActionType = z.infer<typeof routeActionTypeSchema>;

export const routeActionSchema = z.object({
  type: routeActionTypeSchema,
  source: z.string().optional(),
  deliveryId: z.string().optional(),
  phaseId: z.string().optional(),
  ref: z.string().nullable().optional(),
  reason: z.string().optional(),
  targetNode: z.string().optional(),
  targetPhaseId: z.string().optional(),
  refs: z.record(z.unknown()).optional(),
});

export type RouteAction = z.infer<typeof routeActionSchema>;

export const loomDeliveryStatusSchema = z.enum([
  "initialized",
  "brainstorming",
  "planning",
  "executing",
  "reviewing",
  "repairing",
  "waiting_user",
  "completed",
  "blocked",
  "superseded",
]);

export const loomDeliveryPhaseStatusSchema = z.enum([
  "pending",
  "scope_confirmed",
  "planning",
  "ready_for_execution",
  "executing",
  "reviewing",
  "repairing",
  "waiting_user",
  "completed",
  "blocked",
  "superseded",
]);

export const deliveryIndexPhaseSchema = z.object({
  phaseId: z.string().min(1),
  name: z.string().min(1),
  status: loomDeliveryPhaseStatusSchema,
  latestRefs: z.record(z.string().nullable()),
  nextAction: routeActionSchema.nullable(),
});

export const deliveryIndexSchema = z.object({
  schemaVersion: z.literal("1.0"),
  deliveryId: z.string().min(1),
  status: loomDeliveryStatusSchema,
  requestSummary: z.string().min(1),
  roadmapId: z.string().nullable(),
  activePhaseId: z.string().min(1),
  phases: z.array(deliveryIndexPhaseSchema),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type DeliveryIndex = z.infer<typeof deliveryIndexSchema>;
export type DeliveryIndexPhase = z.infer<typeof deliveryIndexPhaseSchema>;

export const loomStatusV1Schema = z.object({
  schemaVersion: z.literal(1),
  activeDeliveryId: z.string().nullable().optional(),
  lastCompletedDeliveryId: z.string().nullable().optional(),
  deliveries: z.array(z.object({
    deliveryId: z.string().min(1),
    status: loomDeliveryStatusSchema,
    requestSummary: z.string().min(1),
    activePhaseId: z.string().nullable(),
    indexRef: z.string().min(1),
    updatedAt: z.string().datetime(),
  })).optional(),
  effectiveNextAction: routeActionSchema.nullable().optional(),
  phase: z.enum([
    "idle",
    "planning",
    "building",
    "reviewing",
    "repairing",
    "deploying",
    "completed",
    "blocked",
  ]),
  current: z.object({
    requirementId: z.string().nullable(),
    planId: z.string().nullable(),
    taskId: z.string().nullable(),
    reviewId: z.string().nullable(),
    repairId: z.string().nullable(),
    deploymentId: z.string().nullable(),
  }),
  lastAction: z.string().nullable(),
  nextAction: z.enum(["plan", "next-task", "execute-task", "review", "repair", "deploy", "none"]),
  updatedAt: z.string().datetime(),
});

export type LoomStatusV1 = z.infer<typeof loomStatusV1Schema>;

export type InputSourceKind =
  | "positional"
  | "request-option"
  | "request-file"
  | "stdin"
  | "context"
  | "context-file";

export type InputSource = {
  kind: InputSourceKind;
  label?: string;
  path?: string;
  content: string;
  extractionStatus?: "completed" | "unsupported" | "failed";
  extractionReason?: string;
  mimeType?: string;
  digest?: string;
  textDigest?: string;
};

export type RequirementInput = {
  primaryRequest: string;
  requestSources: InputSource[];
  contextSources: InputSource[];
  skipKeywordHints?: boolean;
};

const stringArraySchema = z.array(z.string());

export const userFacingLanguageSchema = z.object({
  defaultLocale: z.enum(["zh-CN", "en", "und"]),
  source: z.enum(["project_default", "requirement_primary_language", "fallback"]),
  appliesTo: stringArraySchema,
  doesNotApplyTo: stringArraySchema,
  rule: z.string().min(1),
});

export type UserFacingLanguageConstraint = z.infer<typeof userFacingLanguageSchema>;

export const requirementSourceTypeSchema = z.enum([
  "user_text",
  "pdf",
  "word",
  "markdown",
  "text",
  "code",
  "spreadsheet",
  "unknown",
]);

export type RequirementSourceType = z.infer<typeof requirementSourceTypeSchema>;

export const requirementSourceSchema = z.object({
  sourceId: z.string().min(1),
  type: requirementSourceTypeSchema,
  path: z.string().optional(),
  title: z.string().optional(),
  textDigest: z.string().optional(),
  digest: z.string().optional(),
  extracted: z.boolean().optional(),
});

export type RequirementSource = z.infer<typeof requirementSourceSchema>;

export const brainstormStatusSchema = z.enum([
  "analyzing",
  "needs_clarification",
  "draft_ready",
  "needs_confirmation",
  "confirmed",
  "blocked",
]);

export type BrainstormStatus = z.infer<typeof brainstormStatusSchema>;

export const scopeSourceSchema = z.enum([
  "source_explicit",
  "user_confirmed",
  "user_overridden",
  "model_recommended",
  "derived",
]);

export const scopeItemSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  items: stringArraySchema.optional(),
  reason: z.string().optional(),
  source: scopeSourceSchema,
});

export const scopeAssumptionSchema = z.object({
  id: z.string().min(1),
  text: z.string().min(1),
  requiresConfirmation: z.boolean(),
});

export const brainstormScopeSchema = z.object({
  included: z.array(scopeItemSchema),
  deferred: z.array(scopeItemSchema),
  excluded: z.array(scopeItemSchema),
  assumptions: z.array(scopeAssumptionSchema),
});

export const brainstormSummarySchema = z.object({
  title: z.string().min(1),
  oneLine: z.string().min(1),
  businessGoal: z.string().min(1),
  complexity: z.enum(["small", "medium", "large", "unknown"]),
});

export const actorSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  description: z.string().optional(),
});

export const capabilityGroupSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  description: z.string().optional(),
});

export const businessFlowSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  actors: stringArraySchema,
  capabilityRefs: stringArraySchema,
  summary: z.string().min(1),
});

export const domainModelSchema = z.object({
  actors: z.array(actorSchema),
  capabilityGroups: z.array(capabilityGroupSchema),
  businessFlows: z.array(businessFlowSchema),
});

export const acceptanceCandidateSchema = z.object({
  id: z.string().min(1),
  statement: z.string().min(1),
  capabilityRefs: stringArraySchema,
  sourceRefs: stringArraySchema,
  priority: z.enum(["must", "should", "could"]),
});

export const brainstormAcceptanceSchema = z.object({
  candidates: z.array(acceptanceCandidateSchema),
  coverageNotes: stringArraySchema,
});

export const deliveryStrategySchema = z.object({
  mode: z.enum(["single_phase", "roadmap", "blocked"]),
  reason: z.string().min(1),
  recommendedCurrentPhaseId: z.string().nullable(),
});

export const roadmapStatusSchema = z.enum([
  "draft",
  "needs_confirmation",
  "confirmed",
  "active",
  "completed",
  "revised",
]);

export const phaseStatusSchema = z.enum([
  "proposed",
  "scope_confirming",
  "scope_confirmed",
  "planning",
  "building",
  "reviewing",
  "delivered",
  "paused",
  "skipped",
  "revised",
]);

export const brainstormCandidateStatusSchema = z.enum([
  "confirmed",
  "needs_clarification",
  "blocked",
]);

export const roadmapRequirementSchema = z.enum(["required", "not_required"]);

export const nextActionSchema = z.object({
  actionId: z.string().min(1),
  type: z.enum([
    "confirm_next_phase",
    "revise_roadmap",
    "deploy_current",
    "pause",
    "continue_planning",
  ]),
  label: z.string().min(1),
  recommended: z.boolean(),
  phaseId: z.string().optional(),
  userPrompt: z.string().optional(),
});

export const phaseSchema = z.object({
  phaseId: z.string().min(1),
  name: z.string().min(1),
  status: phaseStatusSchema,
  goal: z.string().min(1),
  scope: z.object({
    includedRefs: stringArraySchema,
    deferredRefs: stringArraySchema,
    excludedRefs: stringArraySchema,
  }),
  acceptanceRefs: stringArraySchema,
  dependsOn: stringArraySchema,
  handoff: z.object({
    readyForPlanning: z.boolean(),
    planningContractId: z.string().nullable(),
    planId: z.string().nullable(),
  }),
  confirmation: z.object({
    confirmedBy: z.string().nullable(),
    confirmedAt: z.string().datetime().nullable(),
    sourcePatchIds: stringArraySchema,
  }),
  nextActions: z.array(nextActionSchema),
});

export const roadmapSchema = z.object({
  roadmapId: z.string().min(1),
  status: roadmapStatusSchema,
  strategy: z.literal("multi_phase"),
  reason: z.string().min(1),
  currentPhaseId: z.string().nullable(),
  recommendedPhaseId: z.string().nullable(),
  phases: z.array(phaseSchema),
  nextActions: z.array(nextActionSchema),
});

export type Roadmap = z.infer<typeof roadmapSchema>;

export const brainstormCandidateScopeSchema = z.object({
  included: z.array(scopeItemSchema),
  excluded: z.array(scopeItemSchema),
  deferred: z.array(scopeItemSchema),
  assumptions: z.array(scopeAssumptionSchema).optional(),
});

export const brainstormCandidatePhaseSchema = z.object({
  phaseId: z.string().min(1),
  title: z.string().min(1).optional(),
  name: z.string().min(1).optional(),
  status: phaseStatusSchema,
  goal: z.string().min(1).optional(),
  scopeRefs: stringArraySchema.optional(),
  acceptanceRefs: stringArraySchema,
  dependsOn: stringArraySchema.optional(),
});

export const phasePlanCurrentSchema = z.object({
  phaseId: z.string().min(1),
  title: z.string().min(1),
  goal: z.string().min(1),
  scopeRefs: stringArraySchema,
  acceptanceRefs: stringArraySchema,
  status: z.literal("scope_confirmed"),
});

export const nextPhasePreviewSchema = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("candidate"),
    suggestedPhaseId: z.string().min(1),
    title: z.string().min(1),
    goal: z.string().min(1),
    scopePreview: stringArraySchema,
    reason: z.string().min(1),
  }),
  z.object({
    kind: z.literal("none"),
    reason: z.string().min(1),
  }),
]);

export const phasePlanSchema = z.object({
  current: phasePlanCurrentSchema,
  nextPhasePreview: nextPhasePreviewSchema,
});

export const deliveryContextSchema = z.object({
  originalRequest: z.object({
    text: z.string().min(1),
    inputRefs: stringArraySchema,
  }),
  userFacingLanguage: userFacingLanguageSchema.optional(),
  initialSummary: z.object({
    title: z.string().min(1),
    oneLine: z.string().min(1),
    businessGoal: z.string().min(1),
    complexity: z.enum(["small", "medium", "large", "unknown"]),
  }),
});

export const conceptGroundingModeSchema = z.enum([
  "concepts_present",
  "none_required",
  "not_applicable",
]);

export const conceptPhaseRelevanceSchema = z.enum([
  "current",
  "current_adjacent",
  "future",
  "deferred",
  "excluded",
]);

export const conceptPrioritySchema = z.enum([
  "must_understand",
  "should_understand",
  "nice_to_understand",
]);

export const conceptRiskFactorSchema = z.enum([
  "business_invariant",
  "state_transition",
  "resource_consistency",
  "permission_boundary",
  "external_contract",
  "scope_confusion_risk",
  "user_visible_flow",
  "runtime_or_delivery_semantics",
  "frontend_experience_semantics",
]);

export const requirementConceptSchema = z.object({
  conceptId: z.string().min(1),
  term: z.string().min(1),
  normalizedName: z.string().min(1),
  explanation: z.string().min(1),
  mustNotMisinterpretAs: stringArraySchema,
  phaseRelevance: conceptPhaseRelevanceSchema,
  priority: conceptPrioritySchema,
  attentionRank: z.number().int().positive(),
  riskFactors: z.array(conceptRiskFactorSchema),
  scopeRefs: stringArraySchema,
  acceptanceRefs: stringArraySchema,
  humanReadableReason: z.string().min(1),
});

const conceptGroundingBaseSchema = z.object({
  mode: conceptGroundingModeSchema,
  reason: z.string().min(1).optional(),
  concepts: z.array(requirementConceptSchema),
});

export const deliveryConceptGlossarySchema = conceptGroundingBaseSchema;
export const phaseConceptGroundingSchema = conceptGroundingBaseSchema;

export const glossaryUpdateSchema = z.object({
  updateId: z.string().min(1),
  operation: z.enum(["add", "replace", "remove"]),
  conceptRef: z.string().min(1).optional(),
  concept: requirementConceptSchema.optional(),
  reason: z.string().min(1),
});

export const conceptGroundingSchema = z.object({
  deliveryConceptGlossary: deliveryConceptGlossarySchema.optional(),
  phaseConceptGrounding: phaseConceptGroundingSchema,
  glossaryUpdates: z.array(glossaryUpdateSchema).optional(),
});

export const conceptConfirmationSchema = z.object({
  shownToUser: z.boolean(),
  confirmedConceptRefs: stringArraySchema,
  confirmationSummary: z.string().min(1),
});

export const confirmationBasisSchema = z.object({
  initialRequestOnly: z.boolean(),
  summaryPresentedToUser: z.boolean(),
  confirmedAfterSummary: z.boolean(),
  presentedItems: stringArraySchema,
});

export const clarificationBlockNameSchema = z.enum([
  "phase_scope",
  "concept_grounding",
  "frontend_experience",
  "final_summary",
]);

export const clarificationBlockSchema = z.object({
  block: clarificationBlockNameSchema,
  summary: z.string().min(1),
  confirmedByUser: z.boolean(),
});

export const skippedClarificationBlockSchema = z.object({
  block: clarificationBlockNameSchema,
  reason: z.string().min(1),
});

export const clarificationProgressSchema = z.object({
  mode: z.literal("progressive_blocks"),
  confirmedBlocks: z.array(clarificationBlockSchema),
  skippedBlocks: z.array(skippedClarificationBlockSchema),
  finalSummaryConfirmed: z.boolean(),
});

export const frontendExperienceLevelSchema = z.enum([
  "none",
  "technical_demo",
  "usable_internal_product",
  "polished_product",
]);

export const frontendTargetSelectionModeSchema = z.enum([
  "query_and_select",
  "direct_id_lookup",
  "preselected_context",
  "not_applicable",
]);

export const frontendActionEntryPointSchema = z.enum([
  "result_row_action",
  "detail_button",
  "form_submit",
  "bulk_action",
  "inline_action",
  "navigation_entry",
]);

export const frontendResultObservationModeSchema = z.enum([
  "list_refresh",
  "detail_refresh",
  "inline_status_update",
  "response_message",
  "not_applicable",
]);

export const frontendInteractionStateSchema = z.enum([
  "loading",
  "success",
  "error",
  "empty",
  "business_blocking",
]);

export const frontendSearchCriterionSchema = z.object({
  criterionId: z.string().min(1),
  label: z.string().min(1),
  fieldRef: z.string().min(1).optional(),
  reason: z.string().min(1),
  sourceRefs: stringArraySchema,
});

export const frontendDataViewSchema = z.object({
  viewId: z.string().min(1),
  name: z.string().min(1),
  purpose: z.string().min(1),
  targetObject: z.string().min(1).optional(),
  selectionMode: frontendTargetSelectionModeSchema,
  paginationRequired: z.boolean(),
  defaultLoadsFirstPage: z.boolean(),
  searchCriteria: z.array(frontendSearchCriterionSchema).optional(),
  criteriaUnclearNote: z.string().min(1).optional(),
  sourceRefs: stringArraySchema,
});

export const frontendActionPathSchema = z.object({
  actionId: z.string().min(1),
  label: z.string().min(1),
  targetObject: z.string().min(1).optional(),
  entryPoint: frontendActionEntryPointSchema,
  inputFields: stringArraySchema.optional(),
  resultObservation: z.array(frontendResultObservationModeSchema),
  refreshPolicy: z.enum([
    "refresh_current_query",
    "refresh_detail",
    "update_inline_state",
    "show_message_only",
    "not_applicable",
  ]),
  successFeedback: stringArraySchema,
  blockingOrErrorFeedback: stringArraySchema,
  sourceRefs: stringArraySchema,
});

export const frontendOperationPathSchema = z.object({
  pathId: z.string().min(1),
  name: z.string().min(1),
  userGoal: z.string().min(1),
  surfaceRef: z.string().min(1).optional(),
  workflowRef: z.string().min(1).optional(),
  targetObject: z.string().min(1).optional(),
  selectionMode: frontendTargetSelectionModeSchema,
  selectionSummary: z.string().min(1),
  dataViewRefs: stringArraySchema,
  actionRefs: stringArraySchema,
  requiredStates: z.array(frontendInteractionStateSchema),
  sourceRefs: stringArraySchema,
});

export const confirmedFrontendExperienceTargetSchema = z.object({
  required: z.boolean(),
  kind: z.string().min(1),
  experienceLevel: frontendExperienceLevelSchema,
  audiences: z.array(z.object({
    audienceId: z.string().min(1),
    name: z.string().min(1),
    primaryJobs: stringArraySchema,
  })).optional(),
  surfaces: z.array(z.object({
    surfaceId: z.string().min(1),
    name: z.string().min(1),
    audienceRefs: stringArraySchema,
    primaryJobs: stringArraySchema,
  })).optional(),
  dataViews: z.array(frontendDataViewSchema).optional(),
  actions: z.array(frontendActionPathSchema).optional(),
  operationPaths: z.array(frontendOperationPathSchema).optional(),
  mustNot: stringArraySchema,
  confirmationSummary: z.string().min(1).optional(),
});

export const conceptGroundingRefsSchema = z.object({
  deliveryConceptGlossaryRef: z.string().min(1).nullable().optional(),
  phaseConceptGroundingRef: z.string().min(1),
});

export const frontendExperienceRefsSchema = z.object({
  confirmedFrontendExperienceRef: z.string().min(1).nullable().optional(),
  currentFrontendExperienceRef: z.string().min(1).nullable().optional(),
});

export const brainstormCandidateSchema = z.object({
  schemaVersion: z.literal("1.0"),
  candidateId: z.string().min(1),
  brainstormRunId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1).optional(),
  status: brainstormCandidateStatusSchema,
  requestSummary: z.object({
    title: z.string().min(1),
    oneLine: z.string().min(1),
    businessGoal: z.string().min(1).optional(),
    complexity: z.enum(["small", "medium", "large", "unknown"]),
  }),
  sources: z.array(requirementSourceSchema).optional(),
  scope: brainstormCandidateScopeSchema,
  roadmap: z.object({
    required: z.boolean(),
    currentPhaseId: z.string().min(1),
    phases: z.array(brainstormCandidatePhaseSchema).min(1),
  }),
  phasePlan: phasePlanSchema,
  acceptance: z.array(acceptanceCandidateSchema),
  domainModel: domainModelSchema.optional(),
  userConfirmation: z.object({
    confirmed: z.boolean(),
    confirmedAt: z.string().datetime().optional(),
    confirmationSummary: z.string().min(1),
    confirmationBasis: confirmationBasisSchema.optional(),
  }),
  conceptGrounding: conceptGroundingSchema.optional(),
  conceptConfirmation: conceptConfirmationSchema.optional(),
  clarificationProgress: clarificationProgressSchema.optional(),
  frontendExperience: confirmedFrontendExperienceTargetSchema.optional(),
  handoff: z.object({
    ready: z.boolean(),
    nextNode: z.enum(["technical_baseline_generation", "brainstorm_clarification", "blocked"]),
    blockingReasons: stringArraySchema.optional(),
  }),
});

export type BrainstormCandidate = z.infer<typeof brainstormCandidateSchema>;

export const clarificationQuestionSchema = z.object({
  questionId: z.string().min(1),
  turnId: z.string().min(1),
  type: z.enum(["free_text", "open_choice", "confirm_patch", "blocking_decision"]),
  severity: z.enum(["blocking", "important", "optional"]),
  question: z.string().min(1),
  whyAsked: z.string().min(1),
  suggestedOptions: z.array(z.object({
    optionId: z.string().min(1),
    label: z.string().min(1),
    description: z.string().optional(),
    recommended: z.boolean().optional(),
  })),
  allowFreeform: z.boolean(),
  freeformHint: z.string().optional(),
  defaultIfSkipped: z.object({
    optionId: z.string().min(1),
    assumptionText: z.string().min(1),
  }).optional(),
  status: z.enum(["pending", "answered", "skipped"]),
});

export const clarificationAnswerSchema = z.object({
  answerId: z.string().min(1),
  turnId: z.string().min(1),
  questionId: z.string().min(1),
  answeredAt: z.string().datetime(),
  answerText: z.string().min(1),
  selectedOptionIds: stringArraySchema,
  freeformText: z.string(),
  source: z.literal("user"),
});

export const contractPatchSchema = z.object({
  patchId: z.string().min(1),
  turnId: z.string().min(1),
  answerId: z.string().min(1).nullable(),
  target: z.enum(["brainstorm_contract", "scope", "roadmap"]),
  status: z.enum(["needs_confirmation", "confirmed", "applied", "rejected"]),
  summary: z.string().min(1),
  operations: z.array(z.object({
    op: z.enum(["add", "replace", "remove"]),
    path: z.string().min(1),
    value: z.unknown().optional(),
  })),
  confidence: z.enum(["low", "medium", "high"]),
  needsUserConfirmation: z.boolean(),
});

export const confirmationRequestSchema = z.object({
  confirmationId: z.string().min(1),
  turnId: z.string().min(1),
  patchIds: stringArraySchema,
  message: z.string().min(1),
  confirmOptions: z.array(z.object({
    id: z.enum(["yes", "revise"]),
    label: z.string().min(1),
  })),
  allowFreeformRevision: z.boolean(),
  status: z.enum(["pending", "confirmed", "revise"]),
});

export const clarificationTurnSchema = z.object({
  turnId: z.string().min(1),
  startedAt: z.string().datetime(),
  completedAt: z.string().datetime().nullable(),
  reason: z.string().min(1),
  questions: stringArraySchema,
  answers: stringArraySchema,
  patches: stringArraySchema,
  confirmations: stringArraySchema,
  status: z.enum(["needs_answer", "answer_received", "needs_confirmation", "confirmed", "blocked"]),
});

export const clarificationStateSchema = z.object({
  status: z.enum([
    "not_needed",
    "needs_answer",
    "answer_received",
    "needs_confirmation",
    "confirmed",
    "blocked",
  ]),
  turns: z.array(clarificationTurnSchema),
  questions: z.array(clarificationQuestionSchema),
  answers: z.array(clarificationAnswerSchema),
  patches: z.array(contractPatchSchema),
  confirmations: z.array(confirmationRequestSchema),
  pendingQuestionIds: stringArraySchema,
  pendingConfirmationIds: stringArraySchema,
});

export const brainstormContractSchema = z.object({
  schemaVersion: z.literal("1.0"),
  contractId: z.string().min(1),
  brainstormRunId: z.string().min(1),
  status: brainstormStatusSchema,
  sources: z.array(requirementSourceSchema),
  summary: brainstormSummarySchema,
  domainModel: domainModelSchema,
  scope: brainstormScopeSchema,
  acceptance: brainstormAcceptanceSchema,
  userConfirmation: z.object({
    confirmed: z.boolean(),
    confirmedAt: z.string().datetime().optional(),
    confirmationSummary: z.string().min(1),
    confirmationBasis: confirmationBasisSchema.optional(),
  }).nullable().optional(),
  deliveryStrategy: deliveryStrategySchema,
  deliveryContext: deliveryContextSchema,
  conceptGrounding: conceptGroundingSchema.optional(),
  conceptConfirmation: conceptConfirmationSchema.optional(),
  clarificationProgress: clarificationProgressSchema.optional(),
  conceptGroundingRefs: conceptGroundingRefsSchema.optional(),
  frontendExperience: confirmedFrontendExperienceTargetSchema.optional(),
  frontendExperienceRefs: frontendExperienceRefsSchema.optional(),
  clarification: clarificationStateSchema,
  roadmap: roadmapSchema.nullable(),
  phasePlan: phasePlanSchema,
  handoff: z.object({
    ready: z.boolean(),
    nextNode: z.literal("planning_generation_contract"),
    blockingReasons: stringArraySchema,
    confirmedAt: z.string().datetime().nullable(),
  }),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type BrainstormContract = z.infer<typeof brainstormContractSchema>;
export type ClarificationAnswer = z.infer<typeof clarificationAnswerSchema>;
