import { z } from "zod";

const stringArraySchema = z.array(z.string());
const agentActionContractSchema = z.object({
  actionKind: z.string().min(1),
  instruction: z.string().min(1),
  actionRequired: z.record(z.unknown()).optional(),
  finalResponseGuard: z.record(z.unknown()).optional(),
  read: z.record(z.unknown()),
  write: z.record(z.unknown()),
  submit: z.record(z.unknown()),
  schema: z.record(z.unknown()),
  stopConditions: stringArraySchema,
});

const referencedArtifactReadGuideSchema = z.array(z.object({
  refKey: z.string().min(1),
  refPath: z.string().min(1).nullable(),
  artifactType: z.string().min(1),
  purpose: z.string().min(1),
  requiredSelectors: stringArraySchema,
  optionalSelectors: stringArraySchema.optional(),
  usageRule: z.string().min(1).optional(),
  doNotGuessAlternateRoots: z.literal(true),
  nullSelectorHandling: z.string().min(1),
}));

export const contractIssueSeveritySchema = z.enum(["blocking"]);
export const contractIssueRepairabilitySchema = z.enum([
  "agent_repairable",
  "requires_user_decision",
  "blocked",
]);

const contractSchemaErrorSchema = z.object({
  source: z.literal("zod"),
  code: z.string().min(1),
  path: z.string().min(1),
  message: z.string().min(1),
  allowedValues: stringArraySchema.optional(),
  expected: z.string().optional(),
  received: z.string().optional(),
});

export const contractIssueSchema = z.object({
  issueId: z.string().min(1),
  code: z.string().min(1),
  severity: contractIssueSeveritySchema,
  path: z.string().min(1),
  message: z.string().min(1),
  repairability: contractIssueRepairabilitySchema,
  repairHint: z.string().min(1),
  schemaError: contractSchemaErrorSchema.optional(),
});

export type ContractIssue = z.infer<typeof contractIssueSchema>;

export const projectKindSchema = z.enum(["greenfield", "existing_project", "unknown"]);

export const repoSignalSetSchema = z.object({
  schemaVersion: z.literal("1.0"),
  signalSetId: z.string().min(1),
  projectKind: projectKindSchema,
  signals: z.object({
    manifests: stringArraySchema,
    packageManagers: stringArraySchema,
    languages: stringArraySchema,
    frameworkHints: stringArraySchema,
    testHints: stringArraySchema,
    sourceSamples: stringArraySchema,
    scripts: z.record(z.string()),
  }),
  conflicts: stringArraySchema,
  confidenceHints: z.record(z.string()),
  createdAt: z.string().datetime(),
});

export type RepoSignalSet = z.infer<typeof repoSignalSetSchema>;

export const technicalBaselineStatusSchema = z.enum([
  "draft",
  "needs_user_confirmation",
  "auto_accepted",
  "confirmed",
  "blocked",
  "superseded",
]);

export const technicalBaselineSourceSchema = z.enum([
  "user_specified",
  "user_confirmed",
  "detected_from_repo",
  "agent_inferred_from_repo_signals",
  "agent_recommended_for_greenfield",
]);

export const technicalBaselineApprovalSchema = z.object({
  type: z.enum(["user_confirmed", "policy_auto_accept", "manual_override", "none"]),
  confirmedAt: z.string().datetime().optional(),
  reason: z.string().optional(),
});

export const technicalBaselineSchema = z.object({
  schemaVersion: z.literal("1.0"),
  technicalBaselineId: z.string().min(1),
  status: technicalBaselineStatusSchema,
  source: technicalBaselineSourceSchema,
  projectKind: projectKindSchema,
  scope: z.enum(["project", "roadmap", "phase_override"]),
  stack: z.record(z.unknown()),
  constraints: stringArraySchema,
  evidence: z.array(z.object({
    path: z.string().min(1).optional(),
    reason: z.string().min(1),
  })),
  approval: technicalBaselineApprovalSchema,
  confidence: z.enum(["low", "medium", "high"]),
  requiresUserConfirmation: z.boolean().optional(),
  reasoningSummary: stringArraySchema.optional(),
  alternatives: z.array(z.object({
    name: z.string().min(1),
    tradeoff: z.string().min(1),
  })).optional(),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type TechnicalBaseline = z.infer<typeof technicalBaselineSchema>;

export const technicalBaselineRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  agentAction: agentActionContractSchema.optional(),
  deliveryId: z.string().min(1).optional(),
  phaseId: z.string().min(1).optional(),
  operation: z.enum(["recommend_greenfield_baseline", "infer_existing_project_baseline"]),
  projectKind: projectKindSchema,
  scope: z.enum(["project", "roadmap", "phase_override"]),
  inputs: z.record(z.unknown()),
  contextRefs: z.object({
    brainstormContractRef: z.string().min(1),
    latestRepositoryContextRef: z.string().min(1).optional(),
    repoSignalSetRef: z.string().min(1).optional(),
    previousTechnicalBaselineRef: z.string().min(1).optional(),
  }).optional(),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  currentPhaseLens: z.object({
    phaseId: z.string().min(1),
    title: z.string().min(1),
    includedScopeRefs: stringArraySchema,
    excludedScopeRefs: stringArraySchema,
    deferredScopeRefs: stringArraySchema,
    acceptanceRefs: stringArraySchema,
  }).optional(),
  reusePolicy: z.object({
    previousTechnicalBaseline: z.enum(["reuse_stable_stack_only", "none"]),
    currentPhaseScopeAuthority: z.literal("brainstorm_contract"),
    repositoryContextAuthority: z.literal("current_code_facts").optional(),
    repoSignalSetAuthority: z.literal("current_repo_signals").optional(),
    baselineConflictRule: z.string().min(1).optional(),
  }).optional(),
  selectionGuidance: z.record(z.unknown()).optional(),
  decisionNeeds: stringArraySchema,
  constraints: z.object({
    mustUse: stringArraySchema,
    mustAvoid: stringArraySchema,
    userPreferences: stringArraySchema,
    deploymentPreference: z.enum(["local_first", "deploy_requested", "unknown"]),
  }),
  generationProtocol: z.record(z.unknown()).optional(),
  enumRefs: z.record(z.array(z.string())).optional(),
  outputContract: z.object({
    candidateFile: z.string().min(1),
    schemaRef: z.literal("technical-baseline-v1"),
    schemaShape: z.record(z.unknown()).optional(),
  }),
  submitCommand: z.record(z.unknown()),
  blockedOutput: z.record(z.unknown()).optional(),
  createdAt: z.string().datetime(),
});

export type TechnicalBaselineRequest = z.infer<typeof technicalBaselineRequestSchema>;

export const planningGenerationContractStatusSchema = z.enum(["draft", "ready", "blocked", "superseded"]);

export const pgcScopeItemSchema = z.object({
  scopeId: z.string().min(1),
  label: z.string().min(1),
  items: stringArraySchema.optional(),
  source: z.string().min(1).optional(),
});

export const pgcAcceptanceCandidateSchema = z.object({
  id: z.string().min(1),
  statement: z.string().min(1),
  capabilityRefs: stringArraySchema.optional(),
  sourceRefs: stringArraySchema.optional(),
  priority: z.enum(["must", "should", "could"]),
});

export const requirementDetailKindSchema = z.enum([
  "business_scenario",
  "scope_boundary",
  "object_field_set",
  "object_operation",
  "business_flow",
  "validation_rule",
  "blocking_rule",
  "state_transition",
  "frontend_operation_path",
  "acceptance_outcome",
  "assumption",
  "deferred_or_excluded_boundary",
]);

export const requirementDetailImpactTagSchema = z.enum([
  "scope",
  "data_model",
  "business_flow",
  "frontend",
  "interface",
  "acceptance",
  "runtime",
]);

export const requirementDetailLifecycleStageSchema = z.enum([
  "create",
  "query_select",
  "view",
  "update",
  "approve_or_process",
  "state_change",
  "terminate_or_cancel",
  "blocking_or_exception",
  "not_applicable",
]);

export const requirementDetailQualitySchema = z.enum(["thin", "usable", "rich"]);

export const requirementDetailItemSchema = z.object({
  detailId: z.string().min(1),
  kind: requirementDetailKindSchema,
  title: z.string().min(1),
  summary: z.string().min(1),
  requiredForCurrentPhase: z.boolean(),
  priority: z.enum(["must", "should", "could"]),
  sourceFieldRefs: stringArraySchema,
  sourceRefs: stringArraySchema,
  scopeRefs: stringArraySchema,
  acceptanceRefs: stringArraySchema,
  conceptRefs: stringArraySchema,
  frontendRefs: stringArraySchema,
  impactTags: z.array(requirementDetailImpactTagSchema),
  lifecycleStage: requirementDetailLifecycleStageSchema,
  quality: requirementDetailQualitySchema,
  unresolvedNote: z.string().min(1).nullable(),
});

export const requirementDetailWarningSchema = z.object({
  warningId: z.string().min(1),
  detailId: z.string().min(1).nullable(),
  sourceFieldRef: z.string().min(1).nullable(),
  severity: z.enum(["info", "warning"]),
  message: z.string().min(1),
});

export const requirementDetailsIndexSchema = z.object({
  schemaVersion: z.literal("1.0"),
  authority: z.literal("brainstorm_contract"),
  sourceBrainstormContractRef: z.string().min(1),
  items: z.array(requirementDetailItemSchema),
  extractionWarnings: z.array(requirementDetailWarningSchema),
});

export type RequirementDetailItem = z.infer<typeof requirementDetailItemSchema>;
export type RequirementDetailsIndex = z.infer<typeof requirementDetailsIndexSchema>;

export const planningGenerationContractSchema = z.object({
  schemaVersion: z.literal("1.0"),
  planningContractId: z.string().min(1),
  status: planningGenerationContractStatusSchema,
  source: z.object({
    brainstormRunId: z.string().min(1),
    brainstormContractId: z.string().min(1),
    roadmapId: z.string().nullable(),
    phaseId: z.string().min(1),
    technicalBaselineId: z.string().min(1),
  }),
  phaseScope: z.object({
    phaseName: z.string().min(1),
    phaseGoal: z.string().min(1),
    included: z.array(pgcScopeItemSchema),
    deferred: z.array(pgcScopeItemSchema),
    excluded: z.array(pgcScopeItemSchema),
    acceptanceCandidates: z.array(pgcAcceptanceCandidateSchema),
  }),
  contextRefs: z.object({
    brainstormContractRef: z.string().min(1).optional(),
    repositoryContextRef: z.string().min(1).nullable().optional(),
    deliveryConceptGlossaryRef: z.string().min(1).nullable().optional(),
    phaseConceptGroundingRef: z.string().min(1).nullable().optional(),
    confirmedFrontendExperienceRef: z.string().min(1).nullable().optional(),
    currentFrontendExperienceRef: z.string().min(1).nullable().optional(),
  }).optional(),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  contextUsageRules: stringArraySchema.optional(),
  technicalBaseline: z.object({
    technicalBaselineId: z.string().min(1),
    status: technicalBaselineStatusSchema,
    scope: z.enum(["project", "roadmap", "phase_override"]),
    summary: z.record(z.unknown()),
    mustFollow: z.boolean(),
  }),
  planningInputs: z.object({
    businessGoal: z.string().min(1),
    actors: z.array(z.unknown()),
    capabilityGroups: z.array(z.unknown()),
    businessFlows: z.array(z.unknown()),
    frontendExperience: z.record(z.unknown()).nullable().optional(),
    frontendExperienceDelta: z.record(z.unknown()).nullable().optional(),
    sourceRefs: stringArraySchema,
    contextNotes: stringArraySchema,
  }),
  requirementDetails: requirementDetailsIndexSchema.optional(),
  planningRules: z.object({
    scopeIsolation: z.object({
      onlyPlanCurrentPhase: z.boolean(),
      forbidDeferredScopeImplementation: z.boolean(),
      forbidFuturePhaseImplementation: z.boolean(),
    }),
    outputRequirements: z.object({
      mustCreateArchitectureArtifactContract: z.boolean(),
      mustCreateTaskPlan: z.boolean(),
      taskPlanMustReferenceAcceptance: z.boolean(),
    }),
    deployment: z.object({
      defaultEnabled: z.boolean(),
      requiresExplicitUserRequest: z.boolean(),
    }),
  }),
  qualityGates: z.object({
    requiresArchitectureBeforeTaskPlan: z.boolean(),
    requiresAcceptanceCoverage: z.boolean(),
    requiresVerificationEvidence: z.boolean(),
  }),
  handoff: z.object({
    readyForArchitecture: z.boolean(),
    readyForTaskPlan: z.boolean(),
    blockingReasons: z.array(z.union([z.string(), z.record(z.unknown())])),
    nextNode: z.enum(["architecture_artifact_contract", "blocked"]),
  }),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type PlanningGenerationContract = z.infer<typeof planningGenerationContractSchema>;

export const architectureSectionSchema = z.enum([
  "engineeringBoundary",
  "modules",
  "dataModel",
  "interfaces",
  "userFlows",
  "stateMachines",
  "acceptanceMatrix",
  "risksAndDecisions",
]);

export const safePathSchema = z.string().min(1);

export const architectureArtifactStatusSchema = z.enum([
  "draft",
  "ready",
  "needs_candidate_repair",
  "needs_user_decision",
  "blocked",
  "superseded",
]);

const refsSchema = z.array(z.string());

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
  "idle",
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

export const frontendExperienceContractSchema = z.object({
  required: z.boolean(),
  kind: z.string().min(1),
  experienceLevel: frontendExperienceLevelSchema,
  sourceRefs: z.record(z.string()).optional(),
  sourceAuthority: stringArraySchema.optional(),
  detection: z.object({
    source: z.string().min(1),
    signals: stringArraySchema,
    confidence: z.enum(["low", "medium", "high"]),
  }).optional(),
  surfaces: z.array(z.object({
    surfaceId: z.string().min(1),
    name: z.string().min(1),
    purpose: z.string().min(1),
    userRoleRefs: refsSchema,
    workflowRefs: refsSchema,
    moduleRefs: refsSchema,
  })),
  dataViews: z.array(frontendDataViewSchema).optional(),
  actions: z.array(frontendActionPathSchema).optional(),
  operationPaths: z.array(frontendOperationPathSchema).optional(),
  navigation: z.object({
    required: z.boolean(),
    pattern: z.string().min(1),
    items: z.array(z.object({
      label: z.string().min(1),
      targetSurfaceRef: z.string().min(1),
    })),
  }),
  interactionStates: z.array(frontendInteractionStateSchema),
  mustNot: stringArraySchema,
  notes: stringArraySchema,
});

export const interfaceRoleSchema = z.enum([
  "read_model",
  "command",
  "readback",
  "component_binding",
  "external_contract",
  "unknown",
]);

export const runtimeDeliveryStatusSchema = z.enum(["modified", "unchanged", "not_applicable"]);
export const runtimeDeliveryCodegenRequiredSchema = z.enum(["yes", "no", "if_applicable"]);
export const runtimeDeliveryVerificationBoundarySchema = z.literal("code_level_only");

const runtimeDeliveryCodeLevelExpectationsSchema = stringArraySchema;

export const runtimeDeliveryContractSchema = z.object({
  status: runtimeDeliveryStatusSchema,
  contractVersion: z.string().min(1),
  runtimeKind: z.string().min(1),
  basis: z.object({
    technicalBaselineRef: z.string().min(1).optional(),
    repositoryContextRef: z.string().min(1).nullable().optional(),
    planningGenerationContractRef: z.string().min(1).optional(),
    previousRuntimeDeliveryRef: z.string().min(1).nullable().optional(),
    reason: z.string().min(1).optional(),
  }),
  build: z.object({
    command: z.string().min(1),
    workingDirectory: z.string().min(1),
    outputs: refsSchema,
    codeLevelExpectations: runtimeDeliveryCodeLevelExpectationsSchema.optional(),
  }).optional(),
  start: z.object({
    command: z.string().min(1),
    workingDirectory: z.string().min(1),
    entry: z.string().min(1).optional(),
    host: z.string().min(1).optional(),
    port: z.number().int().positive().optional(),
    portEnv: z.string().min(1).optional(),
    codeLevelExpectations: runtimeDeliveryCodeLevelExpectationsSchema.optional(),
  }).optional(),
  runtimeSurfaces: z.array(z.object({
    surfaceId: z.string().min(1),
    kind: z.string().min(1),
    probe: z.object({
      type: z.enum(["http_path", "command", "import_check", "none"]),
      target: z.string().min(1).nullable().optional(),
      expected: z.string().min(1).optional(),
    }),
  })).optional(),
  deliveryMechanics: z.object({
    staticAssets: z.object({
      required: z.boolean(),
      source: z.string().min(1).nullable().optional(),
      output: z.string().min(1).nullable().optional(),
      servedBy: z.string().min(1).optional(),
    }).optional(),
    api: z.object({
      required: z.boolean(),
      entry: z.string().min(1).nullable().optional(),
      basePath: z.string().min(1).nullable().optional(),
      probePaths: refsSchema,
    }).optional(),
    codegen: z.object({
      required: runtimeDeliveryCodegenRequiredSchema,
      commands: refsSchema,
      codeLevelExpectations: runtimeDeliveryCodeLevelExpectationsSchema.optional(),
    }).optional(),
  }).optional(),
  taskPlanningGuidance: z.object({
    requireRuntimeDeliveryRequirementWhenTaskTouches: refsSchema,
    doNotRequireForTaskKinds: refsSchema,
    verificationBoundary: runtimeDeliveryVerificationBoundarySchema,
    doNotRequireCleanInstallOrContainerBuild: z.boolean(),
  }).optional(),
  httpProbes: z.object({
    previewPath: z.string().min(1),
    healthPath: z.string().min(1).optional(),
    apiPaths: refsSchema,
    expectedStatus: z.literal("2xx_or_3xx"),
  }).optional(),
  frontend: z.object({
    required: z.boolean(),
    kind: z.string().min(1).optional(),
    buildCommand: z.string().min(1).optional(),
    sourceRoot: z.string().min(1).optional(),
    outputDir: z.string().min(1).optional(),
    servedBy: z.string().min(1).optional(),
    servedByRef: z.string().min(1).optional(),
    codeLevelExpectations: runtimeDeliveryCodeLevelExpectationsSchema.optional(),
  }).optional(),
  api: z.object({
    required: z.boolean(),
    kind: z.string().min(1).optional(),
    buildCommand: z.string().min(1).optional(),
    entry: z.string().min(1).optional(),
    basePath: z.string().min(1).optional(),
    probePaths: refsSchema,
    codeLevelExpectations: runtimeDeliveryCodeLevelExpectationsSchema.optional(),
  }).optional(),
  environment: z.object({
    required: refsSchema,
    optional: refsSchema,
  }).optional(),
  deployability: z.object({
    localDocker: z.enum(["supported", "not_supported", "unknown"]),
    notes: stringArraySchema,
  }).optional(),
});

export type FieldShape = {
  fieldId: string;
  name: string;
  type: string;
  semanticType?: string;
  required: boolean;
  enumValues?: string[];
  items?: FieldShape;
  fields?: FieldShape[];
};

export const fieldShapeSchema: z.ZodType<FieldShape> = z.lazy(() =>
  z.object({
    fieldId: z.string().min(1),
    name: z.string().min(1),
    type: z.string().min(1),
    semanticType: z.string().optional(),
    required: z.boolean(),
    enumValues: stringArraySchema.optional(),
    items: fieldShapeSchema.optional(),
    fields: z.array(fieldShapeSchema).optional(),
  }),
);

export const architectureArtifactContractSchema = z.object({
  schemaVersion: z.literal("1.0"),
  architectureArtifactContractId: z.string().min(1),
  status: architectureArtifactStatusSchema,
  source: z.object({
    planningGenerationContractId: z.string().min(1),
    technicalBaselineId: z.string().min(1),
    brainstormContractId: z.string().min(1).optional(),
    roadmapId: z.string().nullable().optional(),
    phaseId: z.string().min(1),
  }),
  engineeringBoundary: z.object({
    projectKind: projectKindSchema,
    strategy: z.enum([
      "create_minimal_phase_structure",
      "follow_existing_structure",
      "extend_existing_modules",
      "unknown",
    ]),
    applications: z.array(z.object({
      appId: z.string().min(1),
      type: z.string().min(1),
      root: safePathSchema,
    })),
    modules: z.array(z.object({
      moduleId: z.string().min(1),
      appId: z.string().optional(),
      paths: stringArraySchema,
      responsibility: z.string().min(1),
      layerMappings: z.array(z.object({
        layer: z.string().min(1),
        paths: stringArraySchema,
        artifactIntent: z.string().optional(),
      })).optional(),
    })),
    creationPolicy: z.object({
      createOnlyCurrentPhasePaths: z.boolean(),
      avoidFuturePhaseScaffolding: z.boolean(),
    }).optional(),
  }),
  modules: z.array(z.object({
    moduleId: z.string().min(1),
    name: z.string().min(1),
    responsibility: z.string().min(1),
    dependsOn: refsSchema,
    scopeRefs: refsSchema,
    acceptanceRefs: refsSchema,
  })),
  dataModel: z.object({
    entities: z.array(z.object({
      entityId: z.string().min(1),
      name: z.string().min(1),
      type: z.enum(["internal", "external", "derived", "value_object"]),
      implementationIntent: z.enum(["full", "reference_only", "read_only_projection", "external_dependency"]),
      moduleRefs: refsSchema,
      scopeRefs: refsSchema,
      acceptanceRefs: refsSchema,
      fields: z.array(fieldShapeSchema),
      constraints: z.array(z.object({
        constraintId: z.string().min(1),
        type: z.string().min(1),
        description: z.string().min(1),
      })),
    })),
    relationships: z.array(z.object({
      relationshipId: z.string().min(1),
      type: z.string().min(1),
      fromEntityRef: z.string().min(1),
      toEntityRef: z.string().min(1),
      moduleRefs: refsSchema,
      scopeRefs: refsSchema,
      acceptanceRefs: refsSchema,
      description: z.string().min(1),
    })),
    constraints: z.array(z.object({
      constraintId: z.string().min(1),
      type: z.string().min(1),
      description: z.string().min(1),
      entityRefs: refsSchema,
      acceptanceRefs: refsSchema,
    })),
  }),
  interfaces: z.array(z.object({
    interfaceId: z.string().min(1),
    name: z.string().min(1),
    type: z.enum(["http_api", "service_method", "component", "cli_command", "event", "job", "external_adapter"]),
    role: interfaceRoleSchema.optional(),
    moduleRefs: refsSchema,
    entityRefs: refsSchema,
    scopeRefs: refsSchema,
    acceptanceRefs: refsSchema,
    method: z.enum(["GET", "POST", "PUT", "PATCH", "DELETE"]).optional(),
    path: z.string().optional(),
    requestSchema: z.array(fieldShapeSchema).optional(),
    responseSchema: z.array(fieldShapeSchema).optional(),
    errorSchema: z.array(fieldShapeSchema).optional(),
  })),
  userFlows: z.array(z.object({
    flowId: z.string().min(1),
    name: z.string().min(1),
    kind: z.enum(["user_interaction", "service_flow", "cli_flow", "scheduled_flow", "system_flow"]),
    moduleRefs: refsSchema,
    interfaceRefs: refsSchema,
    entityRefs: refsSchema,
    scopeRefs: refsSchema,
    acceptanceRefs: refsSchema,
    entry: z.object({
      type: z.enum(["page", "api", "command", "event", "job", "manual", "unknown"]),
      ref: z.string().nullable(),
      label: z.string().min(1).optional(),
    }),
    steps: z.array(z.object({
      stepId: z.string().min(1),
      actor: z.string().min(1).optional(),
      action: z.string().min(1),
      systemResponse: z.string().min(1).optional(),
      interfaceRefs: refsSchema,
      stateMachineRefs: refsSchema,
    })),
    outcomes: z.array(z.object({
      type: z.enum(["success", "error", "partial", "async_pending"]),
      description: z.string().min(1),
      errorCode: z.string().optional(),
    })),
  })),
  stateMachines: z.array(z.object({
    stateMachineId: z.string().min(1),
    name: z.string().min(1),
    entityRef: z.string().nullable(),
    entityRefs: refsSchema,
    moduleRefs: refsSchema,
    scopeRefs: refsSchema,
    acceptanceRefs: refsSchema,
    states: z.array(z.object({
      stateId: z.string().min(1),
      name: z.string().min(1),
      terminal: z.boolean(),
    })),
    initialState: z.string().min(1),
    events: z.array(z.object({
      eventId: z.string().min(1),
      name: z.string().min(1),
    })),
    transitions: z.array(z.object({
      transitionId: z.string().min(1),
      from: z.string().min(1),
      to: z.string().min(1),
      event: z.string().min(1),
      guards: stringArraySchema,
      effects: stringArraySchema,
    })),
    rules: z.array(z.object({
      ruleId: z.string().min(1),
      description: z.string().min(1),
      acceptanceRefs: refsSchema,
    })),
  })),
  frontendExperience: frontendExperienceContractSchema.optional(),
  runtimeDelivery: runtimeDeliveryContractSchema.optional(),
  acceptanceMatrix: z.array(z.object({
    acceptanceId: z.string().min(1),
    priority: z.enum(["must", "should", "could"]),
    statement: z.string().min(1),
    coverageStatus: z.enum(["covered", "partial", "not_applicable", "deferred", "uncovered"]),
    reason: z.string().optional(),
    reasonCategory: z.enum([
      "candidate_incomplete",
      "reference_error",
      "scope_conflict",
      "baseline_conflict",
      "user_tradeoff",
      "upstream_invalid",
    ]).optional(),
    repairability: contractIssueRepairabilitySchema.optional(),
    coverage: z.array(z.object({
      type: z.enum([
        "module",
        "data_entity",
        "data_constraint",
        "relationship",
        "interface",
        "user_flow",
        "state_machine",
        "state_rule",
        "decision",
        "risk",
      ]),
      refs: refsSchema,
      description: z.string().min(1),
    })),
    verificationHints: z.array(z.object({
      kind: z.enum(["unit", "integration", "e2e", "manual", "static", "contract"]),
      description: z.string().min(1),
    })),
  })),
  risksAndDecisions: z.object({
    decisions: z.array(z.object({
      decisionId: z.string().min(1),
      type: z.enum(["architecture", "scope", "baseline", "delivery", "validation", "implementation"]),
      title: z.string().min(1),
      decision: z.string().nullable(),
      rationale: z.string().min(1),
      scopeRefs: refsSchema,
      acceptanceRefs: refsSchema,
      status: z.enum(["proposed", "accepted", "needs_user_decision", "superseded"]),
      decisionCategory: z.enum([
        "scope_change",
        "baseline_change",
        "architecture_tradeoff",
        "acceptance_conflict",
        "defer_or_include",
      ]).optional(),
      decisionQuestion: z.string().min(1).optional(),
      options: z.array(z.object({
        optionId: z.string().min(1),
        label: z.string().min(1),
      })).optional(),
      allowFreeform: z.boolean().optional(),
      impact: z.object({
        requiresScopeRevision: z.boolean(),
        requiresBaselineRevision: z.boolean(),
        requiresPgcRegeneration: z.boolean(),
        requiresAacRegeneration: z.boolean(),
      }).optional(),
    })),
    risks: z.array(z.object({
      riskId: z.string().min(1),
      type: z.enum(["architecture", "implementation", "data", "integration", "delivery", "validation"]),
      title: z.string().min(1),
      description: z.string().min(1),
      severity: z.enum(["low", "medium", "high", "blocking"]),
      mitigation: z.string().min(1).optional(),
      scopeRefs: refsSchema,
      acceptanceRefs: refsSchema,
      status: z.enum(["open", "mitigated", "accepted", "closed"]),
    })),
    assumptions: z.array(z.object({
      assumptionId: z.string().min(1),
      statement: z.string().min(1),
      scopeRefs: refsSchema,
      entityRefs: refsSchema,
      status: z.enum(["active", "resolved", "superseded"]),
    })),
    deferredNotes: z.array(z.object({
      deferredRef: z.string().min(1),
      reason: z.string().min(1),
      impactOnCurrentPhase: z.string().min(1),
    })),
  }),
  handoff: z.object({
    readyForTaskPlan: z.boolean(),
    blockingReasons: z.array(z.union([z.string(), z.record(z.unknown())])),
    nextNode: z.enum(["task_plan", "architecture_artifact_repair", "needs_user_decision", "blocked"]),
  }),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type ArchitectureArtifactContract = z.infer<typeof architectureArtifactContractSchema>;

export const architectureSectionGroupSchema = z.enum([
  "foundation",
  "domain_contract",
  "behavior",
  "frontend_experience",
  "runtime_delivery",
  "coverage",
]);

export type ArchitectureSectionGroup = z.infer<typeof architectureSectionGroupSchema>;

export const architectureSectionCandidateSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  section: architectureSectionGroupSchema,
  status: z.enum(["ready", "blocked"]),
  content: z.record(z.unknown()),
  blockedReasons: z.array(z.object({
    code: z.string().min(1),
    message: z.string().min(1),
    nextNode: z.enum(["planning_contract_create", "technical_baseline_request", "needs_user_decision", "blocked"]),
  })).optional(),
  createdAt: z.string().datetime(),
});

export type ArchitectureSectionCandidate = z.infer<typeof architectureSectionCandidateSchema>;

const architectureSectionOutputSchema = z.object({
  section: architectureSectionGroupSchema,
  candidateFile: z.string().min(1),
  schemaRef: z.string().min(1),
  schemaShape: z.record(z.unknown()),
  enumRefs: z.record(z.array(z.string())).optional(),
  generationRules: stringArraySchema.optional(),
});

export const architectureSectionsGenerationRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  requestType: z.literal("architecture_sections_generation"),
  agentAction: agentActionContractSchema.optional(),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  generationProtocol: z.record(z.unknown()),
  requestOptimization: z.object({
    profile: z.string().min(1),
    version: z.number().int().positive(),
    intent: z.string().min(1).optional(),
  }).optional(),
  sourceRefs: z.record(z.string()),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  fieldAccessHints: z.record(z.unknown()).optional(),
  contextProjection: z.record(z.unknown()),
  frontendExperienceSource: z.record(z.unknown()).optional(),
  allowedRefs: z.record(z.unknown()),
  rules: z.record(z.unknown()),
  enumRefs: z.record(z.array(z.string())),
  blockedOutput: z.object({
    schemaRef: z.string().min(1),
    candidateFile: z.string().min(1),
    schemaShape: z.record(z.unknown()),
  }),
  submitCommand: z.object({
    name: z.string().min(1),
    argv: z.array(z.string()),
  }),
  outputContract: z.object({
    format: z.string().min(1),
    schema: z.string().min(1),
    sectionOutputs: z.array(architectureSectionOutputSchema),
    allowedRefsUsage: z.record(z.unknown()).optional(),
    blockedOutput: z.record(z.unknown()).optional(),
    pathAuthority: z.record(z.unknown()).optional(),
  }),
  validatorPolicy: z.record(z.unknown()),
  createdAt: z.string().datetime(),
});

export type ArchitectureSectionsGenerationRequest = z.infer<typeof architectureSectionsGenerationRequestSchema>;

export const architectureArtifactRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  operation: z.literal("create_architecture_artifact_candidate"),
  instruction: z.object({
    goal: z.string().min(1),
    mustNot: stringArraySchema,
    output: z.object({
      format: z.literal("json"),
      schemaRef: z.literal("architecture-artifact-contract-v1"),
      candidateFile: z.string().min(1),
    }),
  }),
  inputs: z.object({
    planningContractPath: z.string().min(1),
    technicalBaselinePath: z.string().min(1),
    brainstormContractPath: z.string().min(1).optional(),
  }),
  requiredSections: z.array(architectureSectionSchema),
  sectionSpecs: z.record(z.unknown()),
  rules: z.record(z.unknown()),
  createdAt: z.string().datetime(),
});

export type ArchitectureArtifactRequest = z.infer<typeof architectureArtifactRequestSchema>;

export const architectureAcceptResultSchema = z.object({
  accepted: z.boolean(),
  status: architectureArtifactStatusSchema,
  architectureArtifactContractId: z.string().nullable(),
  issues: z.array(contractIssueSchema),
  contractPath: z.string().nullable(),
  repairRequest: z.record(z.unknown()).nullable(),
  instruction: z.record(z.unknown()).optional(),
  repairInstruction: z.record(z.unknown()).nullable().optional(),
  postRepairSubmitRouting: z.record(z.unknown()).optional(),
});

export type ArchitectureAcceptResult = z.infer<typeof architectureAcceptResultSchema>;

export const taskKindSchema = z.enum([
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
]);

export const implementationActionSchema = z.enum([
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
]);

export const verificationEvidenceSchema = z.enum([
  "automated_test",
  "manual_command_output",
  "runtime_api_check",
  "static_check",
  "agent_review_explanation",
]);

export const runtimeDeliveryRequirementSchema = z.object({
  appliesToThisTask: z.boolean(),
  reason: z.string().min(1),
  runtimeDeliveryRef: z.string().min(1).optional(),
  affectedContractFields: refsSchema.optional(),
  requiredCodeLevelChecks: z.array(z.object({
    checkId: z.string().min(1),
    contractField: z.string().min(1),
    objective: z.string().min(1),
    acceptableEvidence: z.array(verificationEvidenceSchema),
  })).optional(),
  evidenceExpectedInTaskResult: stringArraySchema.optional(),
  forbiddenActions: stringArraySchema.optional(),
  source: z.string().min(1).optional(),
  deploymentFailureRef: z.string().min(1).optional(),
});

export const runtimeDeliveryEvidenceSchema = z.object({
  requirementRef: z.string().min(1).optional(),
  checkedFields: refsSchema.optional(),
  codeLevelChecks: z.array(z.object({
    checkId: z.string().min(1),
    contractField: z.string().min(1).optional(),
    status: z.enum(["passed", "failed", "blocked", "not_applicable"]),
    evidence: z.string().min(1).optional(),
    reason: z.string().min(1).optional(),
  })).optional(),
  commandsRun: z.array(z.object({
    command: z.string().min(1),
    status: z.enum(["passed", "failed", "not_run"]),
    environment: z.enum(["local_warm", "project_workspace", "unknown"]).optional(),
    summary: z.string().min(1).optional(),
  })).optional(),
  unverifiedItems: z.array(z.object({
    item: z.string().min(1),
    reason: z.string().min(1),
  })).optional(),
  runtimeProbeCleanup: z.object({
    temporaryRuntimeStarted: z.boolean(),
    attempted: z.boolean(),
    status: z.enum(["not_needed", "succeeded", "failed", "unknown", "not_safe_to_cleanup"]),
    targets: z.array(z.object({
      kind: z.enum(["process", "port", "container", "dev_server", "other"]),
      pid: z.number().int().positive().nullable().optional(),
      port: z.number().int().positive().nullable().optional(),
      command: z.string().min(1).nullable().optional(),
      summary: z.string().min(1),
    })).optional(),
    summary: z.string().min(1),
  }).optional(),
  source: z.string().min(1).optional(),
  deploymentFailureRef: z.string().min(1).optional(),
  addressedFailedContractFields: refsSchema.optional(),
});

export const taskPlanStatusSchema = z.enum([
  "draft",
  "ready",
  "needs_candidate_repair",
  "blocked",
  "superseded",
]);

export const taskArtifactRefsSchema = z.object({
  modules: refsSchema,
  entities: refsSchema,
  interfaces: refsSchema,
  userFlows: refsSchema,
  stateMachines: refsSchema,
  decisions: refsSchema,
  risks: refsSchema,
});

export const verificationIntentSchema = z.object({
  verificationId: z.string().min(1),
  acceptanceRefs: refsSchema,
  behavior: z.string().min(1),
  preferredEvidence: z.array(verificationEvidenceSchema),
  acceptableEvidence: z.array(verificationEvidenceSchema),
});

export const conceptEvidenceTypeSchema = z.enum([
  "code",
  "test",
  "api",
  "ui",
  "runtime",
  "documentation",
]);

export const conceptResponsibilitySchema = z.object({
  conceptRef: z.string().min(1),
  responsibility: z.string().min(1),
});

export const conceptVerificationIntentSchema = z.object({
  conceptRef: z.string().min(1),
  evidenceType: conceptEvidenceTypeSchema,
  intent: z.string().min(1),
});

export const taskSchema = z.object({
  taskId: z.string().min(1),
  groupId: z.string().min(1),
  title: z.string().min(1),
  taskKind: taskKindSchema,
  implementationActions: z.array(implementationActionSchema),
  objective: z.string().min(1),
  dependsOn: refsSchema,
  scopeRefs: refsSchema,
  acceptanceRefs: refsSchema,
  writeBoundary: z.object({
    forbiddenPaths: refsSchema,
    artifactRefs: taskArtifactRefsSchema,
  }),
  verificationIntents: z.array(verificationIntentSchema),
  conceptRefs: refsSchema.optional(),
  conceptResponsibilities: z.array(conceptResponsibilitySchema).optional(),
  conceptVerificationIntents: z.array(conceptVerificationIntentSchema).optional(),
  frontendExperienceRequirement: z.record(z.unknown()).optional(),
  runtimeDeliveryRequirement: runtimeDeliveryRequirementSchema.optional(),
});

export type Task = z.infer<typeof taskSchema>;

export const taskPlanGroupSchema = z.object({
  groupId: z.string().min(1),
  title: z.string().min(1),
  objective: z.string().min(1),
  dependsOn: refsSchema,
  scopeRefs: refsSchema,
  acceptanceRefs: refsSchema,
  taskIds: refsSchema,
});

export type TaskPlanGroup = z.infer<typeof taskPlanGroupSchema>;

export const taskPlanSchema = z.object({
  schemaVersion: z.literal("1.0"),
  taskPlanId: z.string().min(1),
  version: z.literal(1),
  status: taskPlanStatusSchema,
  source: z.object({
    roadmapId: z.string().nullable(),
    phaseId: z.string().min(1),
    planningGenerationContractId: z.string().min(1),
    architectureArtifactContractId: z.string().min(1),
    technicalBaselineId: z.string().min(1),
  }),
  scopeSnapshot: z.object({
    includedScopeRefs: refsSchema,
    excludedScopeRefs: refsSchema,
    deferredScopeRefs: refsSchema,
    acceptanceRefs: refsSchema,
  }),
  planningPolicy: z.object({
    taskGranularity: z.literal("engineering_increment"),
    groupGranularity: z.literal("engineering_capability"),
    allowTaskSplitDuringRepair: z.boolean(),
    allowTaskMergeDuringRepair: z.boolean(),
  }),
  groups: z.array(taskPlanGroupSchema),
  tasks: z.array(taskSchema),
  handoff: z.object({
    readyForExecution: z.boolean(),
    nextNode: z.enum(["task_execution", "architecture_artifact_repair", "blocked"]),
    blockedReasons: z.array(z.union([z.string(), z.record(z.unknown())])),
  }),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type TaskPlan = z.infer<typeof taskPlanSchema>;

export const taskPlanOutlineSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  status: z.enum(["ready", "blocked"]),
  taskPlanId: z.string().min(1),
  groups: z.array(taskPlanGroupSchema),
  blockedReasons: z.array(z.object({
    code: z.string().min(1),
    message: z.string().min(1),
    nextNode: z.enum(["architecture_artifact_repair", "planning_contract_create", "needs_user_decision", "blocked"]),
  })).optional(),
  createdAt: z.string().datetime(),
});

export type TaskPlanOutline = z.infer<typeof taskPlanOutlineSchema>;

export const taskPlanGroupCandidateSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  status: z.enum(["ready", "blocked"]),
  group: taskPlanGroupSchema,
  tasks: z.array(taskSchema),
  blockedReasons: z.array(z.object({
    code: z.string().min(1),
    message: z.string().min(1),
    nextNode: z.enum(["architecture_artifact_repair", "planning_contract_create", "needs_user_decision", "blocked"]),
  })).optional(),
  createdAt: z.string().datetime(),
});

export type TaskPlanGroupCandidate = z.infer<typeof taskPlanGroupCandidateSchema>;

export const missingDesignSignalSchema = z.object({
  signalId: z.string().min(1),
  aacSections: z.array(architectureSectionSchema),
  scopeRefs: refsSchema,
  acceptanceRefs: refsSchema,
  description: z.string().min(1),
});

export const taskPlanGenerationRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  requestType: z.enum(["generate", "taskplan_grouped_generation"]),
  agentAction: agentActionContractSchema.optional(),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  sourceContracts: z.object({
    technicalBaseline: technicalBaselineSchema,
    planningGenerationContract: planningGenerationContractSchema,
    architectureArtifactContract: architectureArtifactContractSchema,
  }).optional(),
  sourceRefs: z.record(z.string()).optional(),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  fieldAccessHints: z.record(z.unknown()).optional(),
  contextProjection: z.record(z.unknown()).optional(),
  conceptGroundingSource: z.record(z.unknown()).optional(),
  allowedRefs: z.object({
    scopeRefs: refsSchema,
    acceptanceRefs: refsSchema,
    moduleRefs: refsSchema,
    entityRefs: refsSchema,
    interfaceRefs: refsSchema,
    userFlowRefs: refsSchema,
    stateMachineRefs: refsSchema,
    decisionRefs: refsSchema,
    riskRefs: refsSchema,
  }),
  generationRules: z.record(z.unknown()),
  validatorRulesSummary: z.record(z.unknown()),
  generationProtocol: z.record(z.unknown()).optional(),
  enumRefs: z.record(z.array(z.string())).optional(),
  outputContract: z.record(z.unknown()),
  submitCommand: z.record(z.unknown()),
  blockedOutput: z.record(z.unknown()).optional(),
  createdAt: z.string().datetime(),
});

export type TaskPlanGenerationRequest = z.infer<typeof taskPlanGenerationRequestSchema>;

export const taskPlanAcceptResultSchema = z.object({
  accepted: z.boolean(),
  status: taskPlanStatusSchema,
  taskPlanId: z.string().nullable(),
  issues: z.array(contractIssueSchema),
  taskPlanPath: z.string().nullable(),
  repairRequest: z.record(z.unknown()).nullable(),
  run: z.record(z.unknown()).nullable(),
  instruction: z.record(z.unknown()).optional(),
  repairInstruction: z.record(z.unknown()).nullable().optional(),
  postRepairSubmitRouting: z.record(z.unknown()).optional(),
});

export type TaskPlanAcceptResult = z.infer<typeof taskPlanAcceptResultSchema>;

export const taskRunStatusSchema = z.enum([
  "pending",
  "running",
  "completed",
  "completed_with_notes",
  "blocked",
  "failed",
]);

export const taskPlanRunStatusSchema = z.enum([
  "not_started",
  "running",
  "completed",
  "completed_with_notes",
  "blocked",
  "failed",
]);

export const taskPlanNextActionSchema = z.object({
  type: z.enum([
    "continue_execution",
    "architecture_artifact_repair",
    "taskplan_repair",
    "task_result_repair",
    "execution_repair",
    "wait_dependency",
    "review",
  ]),
  reason: z.string().min(1),
  sourceTaskId: z.string().optional(),
  targetNode: z.string().min(1),
});

export const taskPlanRunSchema = z.object({
  schemaVersion: z.literal("1.0"),
  runId: z.string().min(1),
  taskPlanId: z.string().min(1),
  status: taskPlanRunStatusSchema,
  scheduler: z.object({
    mode: z.enum(["single_task_sequential", "group_dag"]),
    startedAt: z.string().datetime().nullable(),
    finishedAt: z.string().datetime().nullable(),
  }),
  groupStates: z.array(z.object({
    groupId: z.string().min(1),
    status: taskRunStatusSchema,
    startedAt: z.string().datetime().nullable(),
    finishedAt: z.string().datetime().nullable(),
    dependsOn: refsSchema,
    taskIds: refsSchema,
  })),
  taskStates: z.array(z.object({
    taskId: z.string().min(1),
    groupId: z.string().min(1).optional(),
    status: taskRunStatusSchema,
    resultId: z.string().nullable(),
    startedAt: z.string().datetime().nullable(),
    finishedAt: z.string().datetime().nullable(),
    dependsOn: refsSchema,
    attempts: z.array(z.object({
      attempt: z.number().int().positive(),
      resultId: z.string().min(1),
      status: z.enum(["completed", "completed_with_notes", "blocked", "failed"]),
    })),
  })),
  summary: z.object({
    total: z.number().int().nonnegative(),
    completed: z.number().int().nonnegative(),
    completedWithNotes: z.number().int().nonnegative(),
    blocked: z.number().int().nonnegative(),
    failed: z.number().int().nonnegative(),
    pending: z.number().int().nonnegative(),
    running: z.number().int().nonnegative(),
  }),
  nextAction: taskPlanNextActionSchema.nullable(),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type TaskPlanRun = z.infer<typeof taskPlanRunSchema>;

export const taskExecutionRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  requestType: z.literal("execute_task"),
  agentAction: agentActionContractSchema.optional(),
  generationProtocol: z.record(z.unknown()),
  enumRefs: z.record(z.array(z.string())),
  source: z.object({
    taskPlanId: z.string().min(1),
    taskId: z.string().min(1),
    groupId: z.string().min(1).optional(),
    technicalBaselineId: z.string().min(1),
    architectureArtifactContractId: z.string().min(1),
    taskPlanRunId: z.string().min(1),
  }),
  sourceRefs: z.record(z.string()).optional(),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  task: taskSchema,
  sourceContext: z.object({
    technicalBaseline: z.record(z.unknown()),
    architectureArtifactProjection: z.record(z.unknown()),
    acceptanceSnapshot: z.array(z.record(z.unknown())),
    dependencyResults: z.array(z.record(z.unknown())),
  }),
  executionRules: z.record(z.unknown()),
  taskConceptGrounding: z.record(z.unknown()).optional(),
  blockedOutput: z.record(z.unknown()),
  outputContract: z.record(z.unknown()),
  submitCommand: z.record(z.unknown()),
  postSubmitRouting: z.record(z.unknown()).optional(),
  operation: z.record(z.unknown()).optional(),
  createdAt: z.string().datetime(),
});

export type TaskExecutionRequest = z.infer<typeof taskExecutionRequestSchema>;

export const taskResultStatusSchema = z.enum([
  "completed",
  "completed_with_notes",
  "blocked",
  "failed",
]);

export const verificationResultSchema = z.object({
  verificationId: z.string().min(1),
  status: z.enum(["passed", "not_run", "failed", "inconclusive"]),
  evidenceType: verificationEvidenceSchema.optional(),
  summary: z.string().min(1),
});

export const selfRepairSummarySchema = z.object({
  attempted: z.boolean(),
  attemptCount: z.number().int().nonnegative(),
  stopReason: z.enum([
    "not_attempted",
    "verification_passed",
    "blocked_condition_detected",
    "same_failure_repeated_without_progress",
    "hard_attempt_limit_reached",
    "repair_requires_contract_change",
    "repair_requires_scope_expansion",
  ]),
  progressObserved: z.boolean(),
});

export const executionContinuitySchema = z.object({
  taskResultSubmittedAfterVerification: z.boolean(),
  agentOwnedLongRunningWork: z.enum(["none", "started_and_released", "unknown"]),
  notes: refsSchema,
});

export const taskResultSchema = z.object({
  schemaVersion: z.literal("1.0"),
  taskResultId: z.string().min(1),
  taskId: z.string().min(1),
  taskPlanId: z.string().min(1),
  status: taskResultStatusSchema,
  changedFiles: refsSchema,
  noChangeReason: z.object({
    code: z.enum(["NO_CODE_CHANGE_REQUIRED", "VERIFICATION_ONLY_TASK", "ENVIRONMENT_CHECK_ONLY"]),
    summary: z.string().min(1),
  }).nullable(),
  verificationResults: z.array(verificationResultSchema),
  selfRepairSummary: selfRepairSummarySchema.nullable(),
  failure: z.object({
    code: z.enum([
      "IMPLEMENTATION_NOT_COMPLETED",
      "VERIFICATION_FAILED",
      "COMMAND_FAILED",
      "DEPENDENCY_INSTALL_FAILED",
      "ENVIRONMENT_ERROR",
      "UNKNOWN_EXECUTION_FAILURE",
    ]),
    summary: z.string().min(1),
  }).nullable(),
  executionContinuity: executionContinuitySchema,
  notes: refsSchema,
  frontendExperienceSelfCheck: z.record(z.unknown()).optional(),
  runtimeDeliveryEvidence: runtimeDeliveryEvidenceSchema.optional(),
  conceptEvidence: z.array(z.object({
    conceptRef: z.string().min(1),
    evidenceType: conceptEvidenceTypeSchema,
    refs: refsSchema,
    summary: z.string().min(1),
  })).optional(),
  blockedReasons: z.array(z.object({
    code: z.enum(["DESIGN_INSUFFICIENT", "TASKPLAN_INVALID", "DEPENDENCY_NOT_READY"]),
    nextNode: z.enum(["architecture_artifact_repair", "taskplan_repair", "wait_dependency"]),
    message: z.string().min(1),
    details: z.record(z.unknown()),
  })),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type TaskResult = z.infer<typeof taskResultSchema>;

export const reviewDecisionSchema = z.enum([
  "approved",
  "approved_with_notes",
  "changes_requested",
  "blocked",
  "needs_user_decision",
]);

export const reviewNextActionTypeSchema = z.enum([
  "done",
  "continue_to_next_phase",
  "execution_repair",
  "taskplan_repair",
  "architecture_artifact_repair",
  "needs_user_decision",
  "manual_review",
]);

export const reviewFindingCategorySchema = z.enum([
  "functional_correctness",
  "integration_risk",
  "test_gap",
  "evidence_insufficient",
  "acceptance_not_satisfied",
  "reference_only_boundary_violation",
  "technical_baseline_violation",
  "forbidden_path_violation",
  "contract_modified",
  "architecture_design_gap",
  "acceptance_design_mismatch",
  "state_model_gap",
  "interface_contract_gap",
  "data_model_gap",
  "task_scope_mismatch",
  "task_dependency_issue",
  "task_verification_mapping_issue",
  "task_artifact_mapping_issue",
  "scope_decision_required",
  "baseline_decision_required",
  "acceptance_tradeoff_required",
  "product_behavior_decision_required",
  "environment_or_dependency",
  "review_limitation",
  "external_system_unavailable",
  "frontend_experience",
  "concept_grounding",
  "runtime_delivery",
  "deployability",
]);

export const reviewFindingSeverityClassSchema = z.enum(["blocking", "manual_review", "warning"]);

export const reviewEvidenceKindSchema = z.enum([
  "static",
  "command",
  "runtime_probe",
  "browser",
  "environment",
  "user_observation",
]);

export const reviewFailureClassSchema = z.enum([
  "product_defect",
  "contract_violation",
  "environment_blocker",
  "insufficient_evidence",
  "subjective_quality",
]);

export const reviewRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  requestType: z.literal("review_gate"),
  agentAction: agentActionContractSchema.optional(),
  source: z.object({
    roadmapId: z.string().nullable(),
    phaseId: z.string().min(1),
    taskPlanId: z.string().min(1),
    taskPlanRunId: z.string().min(1),
    technicalBaselineId: z.string().min(1),
    architectureArtifactContractId: z.string().min(1),
  }),
  reviewScope: z.object({
    type: z.literal("phase_run"),
    groupIds: refsSchema.optional(),
    taskIds: refsSchema.optional(),
    acceptanceRefs: refsSchema,
    nextPhaseId: z.string().nullable().optional(),
    nextPhasePreview: z.discriminatedUnion("kind", [
      z.object({
        kind: z.literal("candidate"),
        suggestedPhaseId: z.string().min(1),
        title: z.string().min(1),
        goal: z.string().min(1),
        scopePreview: refsSchema,
        reason: z.string().min(1),
      }),
      z.object({
        kind: z.literal("none"),
        reason: z.string().min(1),
      }),
    ]).optional(),
  }),
  sourceRefs: z.record(z.string()).optional(),
  reviewPacketRef: z.string().min(1).optional(),
  changeContextRef: z.string().min(1).optional(),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  generationProtocol: z.record(z.unknown()).optional(),
  enumRefs: z.record(z.array(z.string())).optional(),
  sourceContracts: z.object({
    technicalBaseline: technicalBaselineSchema,
    planningGenerationContract: planningGenerationContractSchema,
    architectureArtifactProjection: z.record(z.unknown()),
    taskPlan: taskPlanSchema,
  }).optional(),
  executionArtifacts: z.object({
    taskPlanRun: taskPlanRunSchema,
    taskResults: z.array(taskResultSchema),
    verificationSummary: z.object({
      total: z.number().int().nonnegative(),
      passed: z.number().int().nonnegative(),
      notRun: z.number().int().nonnegative(),
      failed: z.number().int().nonnegative(),
      inconclusive: z.number().int().nonnegative(),
    }),
  }).optional(),
  conceptReviewMatrix: z.array(z.object({
    conceptRef: z.string().min(1),
    priority: z.string().min(1),
    expectedEvidenceTypes: refsSchema,
    taskRefs: refsSchema,
    evidenceRefs: refsSchema,
    mustNotMisinterpretAs: refsSchema,
  })).optional(),
  changeSet: z.record(z.unknown()).optional(),
  reviewRules: z.object({
    commonRules: refsSchema,
    changeSetRules: refsSchema,
  }),
  outputContract: z.record(z.unknown()),
  submitCommand: z.record(z.unknown()),
  createdAt: z.string().datetime(),
});

export type ReviewRequest = z.infer<typeof reviewRequestSchema>;

export const reviewReadRefSchema = z.object({
  type: z.enum(["review_packet", "change_context", "diff_ref", "changed_file", "task_result", "verification_evidence"]),
  ref: z.string().min(1),
  reason: z.string().min(1),
});

export const reviewEvidenceRefSchema = z.object({
  type: z.enum(["task_result", "verification_result", "diff_ref", "changed_file", "manual_note"]),
  ref: z.string().min(1),
  reason: z.string().min(1),
});

export const reviewFindingSchema = z.object({
  findingId: z.string().min(1),
  findingType: z.enum([
    "concept_missing",
    "concept_misunderstanding",
    "concept_evidence_missing",
    "deferred_concept_leakage",
    "surface_only_concept_implementation",
  ]).optional(),
  conceptRef: z.string().min(1).optional(),
  severity: z.enum(["critical", "major", "minor", "note"]),
  severityClass: reviewFindingSeverityClassSchema.optional(),
  evidenceKind: reviewEvidenceKindSchema.optional(),
  failureClass: reviewFailureClassSchema.optional(),
  category: reviewFindingCategorySchema,
  summary: z.string().min(1),
  evidence: z.string().min(1),
  readRefs: z.array(reviewReadRefSchema).min(1),
  evidenceRefs: z.array(reviewEvidenceRefSchema).optional(),
  groupRefs: refsSchema.optional(),
  taskRefs: refsSchema,
  acceptanceRefs: refsSchema,
  artifactRefs: taskArtifactRefsSchema.partial(),
  location: z.object({
    file: z.string().min(1).nullable(),
    line: z.number().int().positive().nullable(),
    diffRef: z.string().min(1).nullable(),
  }).nullable(),
  taskRelevance: z.enum(["direct", "indirect", "unrelated", "unknown"]),
  scopeRelation: z.enum(["within_task_changed_files", "outside_changed_files", "unknown"]),
  introducedByCurrentTask: z.enum(["yes", "no", "unknown"]),
  recommendedNextAction: reviewNextActionTypeSchema,
});

export const reviewResultSchema = z.object({
  schemaVersion: z.literal("1.0"),
  reviewId: z.string().min(1),
  source: z.object({
    requestId: z.string().min(1),
    phaseId: z.string().min(1),
    taskPlanId: z.string().min(1),
    taskPlanRunId: z.string().min(1),
  }),
  decision: reviewDecisionSchema,
  findings: z.array(reviewFindingSchema),
  coverageAssessment: z.object({
    mustAcceptance: z.array(z.object({
      acceptanceRef: z.string().min(1),
      status: z.enum(["satisfied", "not_satisfied", "insufficient_evidence", "not_reviewed"]),
      supportingTaskResults: refsSchema,
      evidenceStatus: z.enum(["sufficient", "insufficient", "missing", "not_applicable"]),
      notes: refsSchema,
    })),
    summary: z.object({
      totalMust: z.number().int().nonnegative(),
      satisfied: z.number().int().nonnegative(),
      insufficientEvidence: z.number().int().nonnegative(),
      notSatisfied: z.number().int().nonnegative(),
      notReviewed: z.number().int().nonnegative(),
    }),
  }),
  limitations: z.array(z.object({
    code: z.enum([
      "DIFF_REF_UNREADABLE",
      "NO_GIT_DIFF",
      "FILE_CONTENT_UNREADABLE",
      "INSUFFICIENT_CONTEXT",
      "REVIEW_RESULT_INVALID",
    ]),
    summary: z.string().min(1),
    impact: z.string().min(1),
  })),
  pendingActions: z.array(z.object({
    type: reviewNextActionTypeSchema,
    findingRefs: refsSchema,
    reason: z.string().min(1),
  })),
  nextAction: z.object({
    type: reviewNextActionTypeSchema,
    reason: z.string().min(1),
    targetNode: z.string().min(1).optional(),
    targetPhaseId: z.string().optional(),
    targetTaskIds: refsSchema.optional(),
    findingRefs: refsSchema.optional(),
    userVisibleState: z.string().optional(),
  }),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type ReviewResult = z.infer<typeof reviewResultSchema>;

export const manualReviewResolutionSchema = z.object({
  schemaVersion: z.literal("1.0"),
  manualReviewResolutionId: z.string().min(1),
  manualReviewRequestId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  userAnswer: z.object({
    text: z.string().min(1),
    selectedShortReply: z.string().min(1).optional(),
  }),
  decision: z.enum(["approve_override", "request_changes"]),
  changeRequest: z.object({
    summary: z.string().min(1),
    route: z.enum(["execution_repair", "taskplan_repair", "architecture_artifact_repair", "needs_user_decision"]),
    reason: z.string().min(1),
    details: z.record(z.unknown()),
  }).nullable(),
  nextAction: z.object({
    type: reviewNextActionTypeSchema,
    targetNode: z.string().min(1),
    reason: z.string().min(1),
  }),
  createdAt: z.string().datetime(),
});

export type ManualReviewResolution = z.infer<typeof manualReviewResolutionSchema>;

export const repositoryContextRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  requestId: z.string().min(1),
  agentAction: agentActionContractSchema.optional(),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  status: z.literal("pending"),
  purpose: z.literal("generate_phase_start_repository_snapshot"),
  projectKind: projectKindSchema,
  source: z.record(z.unknown()),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  scanPurpose: z.record(z.unknown()),
  generationProtocol: z.record(z.unknown()).optional(),
  enumRefs: z.record(z.array(z.string())).optional(),
  generationRules: stringArraySchema,
  outputContract: z.object({
    schema: z.literal("RepositoryContext"),
    candidateFile: z.string().min(1),
    enumRefs: z.record(z.array(z.string())).optional(),
    referenceRules: z.record(z.unknown()).optional(),
    schemaShape: z.record(z.unknown()).optional(),
  }),
  submitCommand: z.record(z.unknown()),
  blockedOutput: z.record(z.unknown()).optional(),
  validatorPolicy: z.record(z.unknown()),
  failureRecovery: z.record(z.unknown()),
  createdAt: z.string().datetime(),
});

export type RepositoryContextRequest = z.infer<typeof repositoryContextRequestSchema>;

const repositoryContextPathRefSchema = z.string().min(1);

export const repositoryContextSchema = z.object({
  schemaVersion: z.literal("1.0"),
  repositoryContextId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  status: z.enum(["ready", "partial", "insufficient"]),
  source: z.object({
    requestRef: z.string().min(1),
    brainstormContractRef: z.string().min(1),
    technicalBaselineRef: z.string().min(1),
  }),
  requestLens: z.object({
    projectKind: projectKindSchema,
    scanPurpose: z.literal("phase_start_repository_snapshot"),
    primaryConsumer: z.literal("phase_brainstorm"),
    laterConsumers: z.array(z.enum(["PGC", "AAC", "TaskPlan"])).default([]),
  }),
  repoOverview: z.object({
    summary: z.string().min(1),
    repositoryShape: z.enum(["single_package", "monorepo", "multi_application", "unknown"]),
    primaryApplications: z.array(z.object({
      applicationId: z.string().min(1),
      name: z.string().min(1),
      kind: z.string().min(1),
      rootPath: repositoryContextPathRefSchema,
    })).default([]),
  }),
  technologySignals: z.object({
    primaryLanguages: stringArraySchema.default([]),
    frameworks: stringArraySchema.default([]),
    packageManagers: stringArraySchema.default([]),
    buildCommands: stringArraySchema.default([]),
    testCommands: stringArraySchema.default([]),
    notes: stringArraySchema.default([]),
  }).default({}),
  structureSignals: z.object({
    rootPaths: z.array(z.object({
      path: repositoryContextPathRefSchema,
      role: z.string().min(1),
    })).default([]),
    entryPoints: z.array(z.object({
      path: repositoryContextPathRefSchema,
      kind: z.string().min(1),
      description: z.string().min(1).optional(),
    })).default([]),
    configurationFiles: z.array(repositoryContextPathRefSchema).default([]),
  }).default({}),
  existingCapabilities: z.array(z.object({
    capabilityId: z.string().min(1),
    name: z.string().min(1),
    status: z.enum(["implemented", "partial", "missing", "unknown"]),
    summary: z.string().min(1),
    surfaceRefs: stringArraySchema,
    confidence: z.enum(["high", "medium", "low", "unknown"]).optional(),
    deliveryRelevance: z.string().min(1).optional(),
  })).default([]),
  relevantSurfaces: z.array(z.object({
    surfaceId: z.string().min(1),
    kind: z.enum(["entrypoint", "module", "service", "controller", "data_access", "ui", "config", "test", "script", "documentation", "unknown"]),
    path: repositoryContextPathRefSchema,
    summary: z.string().min(1),
    relevance: z.enum(["implemented_capability", "architecture_boundary", "extension_point", "validation_surface", "delivery_context", "unrelated"]),
    suggestedUse: z.enum(["inspect_only", "inspect_or_extend", "reuse_existing_pattern", "avoid_modifying"]),
  })).default([]),
  recommendedReadRefs: z.array(z.object({
    path: repositoryContextPathRefSchema,
    reason: z.enum(["implemented_capability", "dependency_context", "integration_boundary", "test_or_validation", "risk_review", "extension_point"]),
    priority: z.enum(["high", "medium", "low"]),
    summary: z.string().min(1).optional(),
    surfaceRefs: stringArraySchema.optional(),
  })).default([]),
  roadmapImplications: z.array(z.object({
    phaseRef: z.string().min(1).optional(),
    implication: z.string().min(1).optional(),
    summary: z.string().min(1).optional(),
    impactType: z.enum(["avoid_structural_dead_end", "preserve_extension_point", "avoid_scope_conflict", "defer_implementation", "unknown"]).optional(),
    type: z.enum(["already_implemented", "needs_scope_adjustment", "future_scope_risk", "none"]).optional(),
    affectedSurfaces: stringArraySchema.optional(),
  })).default([]),
  contextQuality: z.object({
    coverage: z.enum(["focused", "partial", "broad", "insufficient"]),
    confidence: z.enum(["high", "medium", "low", "unknown"]),
    warnings: z.array(z.object({
      code: z.string().min(1),
      message: z.string().min(1),
    })).default([]),
  }),
  warnings: z.array(z.object({
    code: z.string().min(1),
    message: z.string().min(1),
  })).default([]),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
});

export type RepositoryContext = z.infer<typeof repositoryContextSchema>;

export const operationLeaseSchema = z.object({
  schemaVersion: z.literal("1.0"),
  operationId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  operationType: z.string().min(1),
  status: z.enum(["active", "closed", "stale_recovered"]),
  startedAt: z.string().datetime(),
  heartbeatAt: z.string().datetime(),
  expiresAt: z.string().datetime(),
  refs: z.record(z.unknown()),
});

export type OperationLease = z.infer<typeof operationLeaseSchema>;

export const routeDecisionSchema = z.object({
  schemaVersion: z.literal("1.0"),
  decisionId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  status: z.enum(["ready", "waiting_user", "blocked", "done", "state_corrupted"]),
  source: z.object({
    command: z.string().optional(),
    type: z.string().min(1),
    ref: z.string().nullable(),
    validated: z.boolean(),
  }),
  nextAction: z.object({
    type: z.string().min(1),
    targetNode: z.string().optional(),
    reason: z.string().min(1),
    refs: z.record(z.unknown()).optional(),
  }),
  instruction: z.record(z.unknown()),
  materialization: z.object({
    attempted: z.boolean(),
    status: z.enum(["not_applicable", "created", "already_exists", "skipped_user_gated", "skipped_blocked", "failed"]),
    requestRef: z.string().nullable(),
    candidateFile: z.string().nullable(),
    leaseRef: z.string().nullable(),
  }),
  concurrency: z.record(z.unknown()),
  transition: z.record(z.unknown()).optional(),
  possibleRuntimeForegroundStall: z.record(z.unknown()).optional(),
  createdAt: z.string().datetime(),
});

export type RouteDecision = z.infer<typeof routeDecisionSchema>;

export const repairRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  repairRequestId: z.string().min(1),
  agentAction: agentActionContractSchema.optional(),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  repairType: z.enum(["execution_repair", "task_result_repair", "taskplan_repair", "architecture_artifact_repair"]),
  source: z.record(z.unknown()),
  scope: z.record(z.unknown()),
  workspaceContext: z.record(z.unknown()),
  inputs: z.record(z.unknown()),
  referencedArtifactReadGuide: referencedArtifactReadGuideSchema.optional(),
  repairRules: stringArraySchema,
  generationProtocol: z.record(z.unknown()).optional(),
  enumRefs: z.record(z.array(z.string())).optional(),
  outputContract: z.record(z.unknown()),
  resumePolicy: z.record(z.unknown()),
  operation: z.record(z.unknown()),
  createdAt: z.string().datetime(),
});

export type RepairRequest = z.infer<typeof repairRequestSchema>;

export const userDecisionRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  decisionRequestId: z.string().min(1),
  deliveryId: z.string().min(1),
  phaseId: z.string().min(1),
  status: z.literal("waiting_user"),
  source: z.record(z.unknown()),
  question: z.record(z.unknown()),
  decisionContext: z.record(z.unknown()).nullable(),
  options: z.array(z.record(z.unknown())),
  decisionRules: z.record(z.unknown()),
  answerContract: z.record(z.unknown()),
  createdAt: z.string().datetime(),
});

export type UserDecisionRequest = z.infer<typeof userDecisionRequestSchema>;
