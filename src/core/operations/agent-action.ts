export type AgentActionContract = {
  actionKind:
    | "brainstorm_session"
    | "generate_candidate"
    | "generate_sections"
    | "generate_taskplan_grouped"
    | "execute_task"
    | "review_gate"
    | "repository_context";
  instruction: string;
  actionRequired?: Record<string, unknown>;
  finalResponseGuard?: Record<string, unknown>;
  read: {
    requestRef?: string;
    required: string[];
    optional?: string[];
    primaryMethod?: "inspect";
    fallbackMethod?: "request_manifest_refs";
    fieldGroups?: AgentActionReadGroup[];
    fields?: AgentActionReadField[];
    fallbackRule?: string;
    displayPolicy: "compact";
  };
  write: {
    currentTarget?: Record<string, unknown>;
    candidateFile?: string;
    resultFile?: string;
    sectionOutputs?: Array<{ section: string; candidateFile: string }>;
    outlineFile?: string;
    groupFilePattern?: string;
    blockedFile?: string;
    requiredTopLevelFields?: string[];
    requiredTopLevelFieldRule?: string;
    requiredRuntimeEvidence?: Record<string, unknown>;
    rules: string[];
  };
  submit: {
    command: {
      name: string;
      argv: string[];
    };
    requiredArgs: string[];
    placeholders: Record<string, string>;
    runAfter: string;
  };
  schema: {
    primary: string;
    shapeLocation: string;
    enumLocation: string;
    allowedRefsLocation?: string;
  };
  stopConditions: string[];
};

export type AgentActionReadField = {
  field: string;
  required: boolean;
  purpose: string;
  whenToRead: string;
  readCommand: {
    name: "inspect";
    argv: string[];
  };
  fallback: {
    method: "request_manifest_refs";
    refKey: string | null;
    selector: string;
    source: "request_root" | "request_manifest_ref";
  };
};

export type AgentActionReadGroup = {
  groupId: string;
  required: boolean;
  purpose: string;
  whenToRead: string;
  fields: string[];
  readCommand: {
    name: "inspect";
    argv: string[];
  };
  fallbackRule: string;
};

export function brainstormSessionAgentActionContract(input: {
  candidateFile: string;
  blockedFile?: string | null;
  contextKind?: "initial" | "phase_continuation";
  submitCommand: {
    name: string;
    argv: string[];
  };
}): AgentActionContract {
  const baseOptional = [
    "clarificationGuidance",
    "riskGuidance",
    "confirmationRules",
    "contextRefs.requirementContextRef",
    "contextRefs.originalRequirementContextRef",
    "contextRefs.normalizedRequirementTextRef",
    "contextRefs.keywordHintsRef",
    "requirementContext.sourceItems",
    "requirementContext.normalizedText",
  ];
  const phaseContinuationOptional = input.contextKind === "phase_continuation"
    ? [
        "phaseContinuationContext",
        "latestConfirmedRequirementDecision",
        "confirmedRequirementDecisionsIndex",
        "contextRefs.deliveryContextRef",
        "contextRefs.latestRepositoryContextRef",
        "contextRefs.latestConfirmedRequirementDecisionRef",
        "contextRefs.confirmedRequirementDecisionsIndexRef",
        "contextRefs.deliveryConceptGlossaryRef",
        "contextRefs.phaseConceptGroundingRef",
        "contextRefs.currentFrontendExperienceRef",
        "deliveryContext.sources",
        "latestRepositoryContext",
        "deliveryConceptGlossary",
        "phaseConceptGrounding",
        "currentFrontendExperience",
      ]
    : [];
  return agentActionContract({
    actionKind: "brainstorm_session",
    instruction: "Manage the progressive Brainstorm clarification conversation. Before presenting any confirmation summary, read the request through root requestReadPlan groups when present; otherwise use agentAction.read.fieldGroups inspect commands. Continue through phase_scope, concept_grounding, frontend_experience, and final_summary in chat. Write and submit BrainstormCandidate only after the user explicitly confirms the dedicated final_summary block. Do not infer scope, output paths, sources, concepts, frontend target, or rules from guessed legacy root fields.",
    read: {
      required: [
        "this request",
        "agentAction",
        "requestManifest",
        "originalRequest",
        "userFacingLanguage",
        "contextRefs",
        "sourceFieldAccessHints",
        "knowledgeContextProtocol",
        "knowledgeQueryPlan",
        "firstClarificationGate",
        "clarificationConversationProtocol",
        "conceptGroundingRequest",
        "outputContract",
        "rules",
        "generationProtocol",
        "enumRefs",
      ],
      optional: [
        ...baseOptional,
        ...phaseContinuationOptional,
      ],
      displayPolicy: "compact",
    },
    write: {
      candidateFile: input.candidateFile,
      blockedFile: input.blockedFile ?? undefined,
      rules: [
        "Do not write outputContract.candidateFile during phase_scope, concept_grounding, or frontend_experience confirmation.",
        "Write only outputContract.candidateFile for a confirmed BrainstormCandidate after final_summary has been presented and explicitly confirmed by the user.",
        "If blocked, write only blockedOutput.candidateFile when present.",
        "Do not write accepted Brainstorm contract files directly.",
        "Do not base BrainstormCandidate fields on null values returned from guessed selectors; return to agentAction.read.fieldGroups and sourceFieldAccessHints.",
      ],
    },
    submit: {
      command: input.submitCommand,
      requiredArgs: ["--delivery-id", "--phase-id", "--request-id", "--run-id", "--candidate-file"],
      placeholders: { "{candidateFile}": input.candidateFile },
      runAfter: "the dedicated final_summary block has been explicitly confirmed, candidateFile exists, and the candidate validates against outputContract.schemaShape",
    },
    schema: {
      primary: "BrainstormCandidate",
      shapeLocation: "outputContract.schemaShape",
      enumLocation: "enumRefs",
    },
    stopConditions: [
      "after presenting the next required Brainstorm block, the user has not confirmed or corrected it yet",
      "blockedOutput is required",
      "submitCommand returns non-repairable failure",
    ],
  });
}

export function agentActionContract(input: AgentActionContract): AgentActionContract {
  return {
    ...input,
    read: normalizeReadPlan(input.actionKind, input.read),
  };
}

export function normalizeAgentActionForFieldGroups(value: unknown): unknown {
  if (!isRecord(value)) {
    return value;
  }
  const actionKind = typeof value.actionKind === "string" ? value.actionKind : "unknown";
  const read = isRecord(value.read) ? value.read as AgentActionContract["read"] : null;
  if (!read) {
    return value;
  }
  return {
    ...value,
    read: normalizeReadPlan(actionKind, read),
  };
}

export function normalizeAgentActionForRequest(value: unknown, request: Record<string, unknown>): unknown {
  const normalized = normalizeAgentActionForFieldGroups(value);
  if (!isRecord(normalized) || !isRecord(normalized.read) || !Array.isArray(normalized.read.fieldGroups)) {
    return normalized;
  }
  const contextRefs = isRecord(request.contextRefs) ? request.contextRefs : {};
  const fieldGroups = normalized.read.fieldGroups
    .filter((group): group is Record<string, unknown> => isRecord(group))
    .map((group) => {
      const required = typeof group.required === "boolean" ? group.required : true;
      const fields = Array.isArray(group.fields)
        ? group.fields.filter((field): field is string => typeof field === "string" && field.trim().length > 0)
        : [];
      const availableFields = fields.filter((field) => {
        if (required) {
          return requestFieldAvailability(request, field) !== "missing";
        }
        const contextRefKey = contextRefKeyForReadField(field);
        return (!contextRefKey || contextRefKey in contextRefs) &&
          requestFieldAvailability(request, field) !== "missing";
      });
      return {
        ...group,
        fields: availableFields,
        readCommand: inspectReadCommand(availableFields),
      };
    })
    .filter((group) => group.fields.length > 0);
  return {
    ...normalized,
    read: {
      ...normalized.read,
      fieldGroups,
    },
  };
}

function normalizeReadPlan(actionKind: string, read: AgentActionContract["read"]): AgentActionContract["read"] {
  const requiredLabels = readLabels(read.required);
  const optionalLabels = readLabels(read.optional);
  const supplementalRequiredFields = requiredLabels
    .map((label) => readFieldFromLabel(label, true)?.field)
    .filter((field): field is string => Boolean(field));
  const supplementalOptionalFields = optionalLabels
    .map((label) => readFieldFromLabel(label, false)?.field)
    .filter((field): field is string => Boolean(field));
  const existingGroups = normalizeExistingFieldGroups(
    actionKind,
    read.fieldGroups,
    supplementalRequiredFields,
    supplementalOptionalFields,
  );
  if (existingGroups.length > 0) {
    const { fields: _legacyFields, ...rest } = read;
    return {
      ...rest,
      primaryMethod: "inspect",
      fallbackMethod: "request_manifest_refs",
      fieldGroups: existingGroups,
      fallbackRule: "Read each fieldGroup with its inspect readCommand first. If inspect fails, use requestManifest refs for the fields in that group; if that also fails, read requestRef/requestManifest refs directly as a correctness fallback and keep chat output compact.",
    };
  }

  const fields = [
    ...requiredLabels.map((label) => readFieldFromLabel(label, true)),
    ...optionalLabels.map((label) => readFieldFromLabel(label, false)),
  ]
    .filter((field): field is AgentActionReadField => field !== null);
  const legacyFields = normalizeLegacyReadFields(read.fields);
  const normalizedFields = fields.length > 0 ? fields : legacyFields;
  const fieldGroups = buildFieldGroups(actionKind, normalizedFields);
  const { fields: _legacyFields, ...rest } = read;
  return {
    ...rest,
    primaryMethod: "inspect",
    fallbackMethod: "request_manifest_refs",
    fieldGroups,
    fallbackRule: "Read each fieldGroup with its inspect readCommand first. If inspect fails, use requestManifest refs for the fields in that group; if that also fails, read requestRef/requestManifest refs directly as a correctness fallback and keep chat output compact.",
  };
}

function readLabels(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.trim().length > 0)
    : [];
}

function normalizeExistingFieldGroups(
  actionKind: string,
  value: unknown,
  supplementalRequiredFields: string[] = [],
  supplementalOptionalFields: string[] = [],
): AgentActionReadGroup[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const groups = value
    .filter((group): group is Record<string, unknown> => isRecord(group))
    .map((group) => {
      const rawFields = Array.isArray(group.fields)
        ? group.fields.filter((field): field is string => typeof field === "string" && field.trim().length > 0)
        : [];
      const fields = actionKind === "execute_task"
        ? uniqueFieldNames(rawFields)
        : dedupeCoveredFieldPaths(rawFields);
      if (fields.length === 0) {
        return null;
      }
      const groupId = typeof group.groupId === "string" && group.groupId.trim().length > 0
        ? group.groupId
        : groupIdFromFields(fields);
      return {
        groupId,
        required: typeof group.required === "boolean" ? group.required : true,
        purpose: typeof group.purpose === "string" && group.purpose.length > 0
          ? group.purpose
          : purposeForFieldGroup(groupId, fields),
        whenToRead: typeof group.whenToRead === "string" && group.whenToRead.length > 0
          ? group.whenToRead
          : whenToReadForFieldGroup(groupId, fields),
        fields,
        readCommand: inspectReadCommand(fields),
        fallbackRule: typeof group.fallbackRule === "string" && group.fallbackRule.length > 0
          ? group.fallbackRule
          : "If this grouped inspect read fails, read each listed field through requestManifest refs as a targeted fallback.",
      };
    })
    .filter((group): group is AgentActionReadGroup => group !== null);
  if (actionKind === "execute_task") {
    return buildTaskExecutionFieldGroups(
      actionKind,
      [
        ...groups.filter((group) => group.required).flatMap((group) => group.fields),
        ...supplementalRequiredFields,
      ],
      [
        ...groups.filter((group) => !group.required).flatMap((group) => group.fields),
        ...supplementalOptionalFields,
      ],
    );
  }
  const requiredFields = dedupeCoveredFieldPaths(groups
    .filter((group) => group.required)
    .flatMap((group) => group.fields));
  const seen = new Set<string>();
  const normalizedGroups = groups
    .map((group) => {
      const candidateFields = group.required
        ? group.fields.filter((field) => requiredFields.includes(field))
        : removeFieldsCoveredBy(group.fields, requiredFields);
      const fields = candidateFields.filter((field) => {
        if (seen.has(field)) {
          return false;
        }
        seen.add(field);
        return true;
      });
      return {
        ...group,
        fields,
        readCommand: inspectReadCommand(fields),
      };
    })
    .filter((group) => group.fields.length > 0);
  if (actionKind === "brainstorm_session") {
    return rewriteBrainstormFieldGroups(normalizedGroups);
  }
  return normalizedGroups;
}

function rewriteBrainstormFieldGroups(groups: AgentActionReadGroup[]): AgentActionReadGroup[] {
  const fields = groups.flatMap((group) => group.fields);
  return buildBrainstormFieldGroups("brainstorm_session", fields, [], fields);
}

function buildFieldGroups(actionKind: string, fields: AgentActionReadField[]): AgentActionReadGroup[] {
  const rawRequiredFields = uniqueFieldNames(fields.filter((field) => field.required).map((field) => field.field));
  const rawOptionalFieldsForAction = uniqueFieldNames(fields.filter((field) => !field.required).map((field) => field.field));
  const requiredFields = dedupeCoveredFieldPaths(rawRequiredFields);
  const rawOptionalFields = dedupeCoveredFieldPaths(rawOptionalFieldsForAction);
  const optionalFields = removeFieldsCoveredBy(
    rawOptionalFields,
    requiredFields,
  );
  if (actionKind === "brainstorm_session") {
    return buildBrainstormFieldGroups(actionKind, requiredFields, optionalFields, rawOptionalFields);
  }
  if (actionKind === "execute_task") {
    return buildTaskExecutionFieldGroups(actionKind, rawRequiredFields, rawOptionalFieldsForAction);
  }
  if (actionKind === "generate_sections") {
    return buildArchitectureSectionFieldGroups(actionKind, requiredFields, optionalFields);
  }
  if (actionKind === "repository_context") {
    return buildRepositoryContextFieldGroups(actionKind, requiredFields, optionalFields);
  }
  if (actionKind === "review_gate") {
    return buildReviewGateFieldGroups(actionKind, requiredFields, optionalFields);
  }
  if (actionKind === "generate_candidate" && requiredFields.includes("repairRules")) {
    return buildRepairCandidateFieldGroups(actionKind, requiredFields, optionalFields);
  }
  const groups: AgentActionReadGroup[] = [];
  if (requiredFields.length > 0) {
    groups.push(fieldGroupFor(actionKind, "core", true, requiredFields));
  }
  if (optionalFields.length > 0) {
    groups.push(fieldGroupFor(actionKind, "optional_context", false, optionalFields));
  }
  return groups;
}

function buildTaskExecutionFieldGroups(actionKind: string, requiredFields: string[], optionalFields: string[]): AgentActionReadGroup[] {
  const availableFields = uniqueFieldNames([...requiredFields, ...optionalFields]);
  const used = new Set<string>();
  const hasField = (field: string): boolean => fieldAvailable(field, availableFields);
  const hasExplicitFrontendRequirement = availableFields.some((field) =>
    field === "task.frontendExperienceRequirement" ||
    field.startsWith("task.frontendExperienceRequirement.")
  );

  const groups: AgentActionReadGroup[] = [];
  pushTakenAvailableGroup(groups, used, actionKind, "task_core", true, hasField, [
    "task.taskId",
    "task.groupId",
    "task.title",
    "task.taskKind",
    "task.implementationActions",
    "task.objective",
    "task.dependsOn",
    "task.scopeRefs",
    "task.acceptanceRefs",
    "task.writeBoundary",
    "task.verificationIntents",
    "task.conceptRefs",
    "task.conceptResponsibilities",
    "task.conceptVerificationIntents",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "acceptance_and_concepts", true, hasField, [
    "sourceContext.acceptanceSnapshot",
    "sourceContext.userFacingLanguage",
    "taskConceptGrounding",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "architecture_context", true, hasField, [
    "sourceContext.architectureArtifactProjection.profile",
    "sourceContext.architectureArtifactProjection.projectionCompleteness",
    "sourceContext.architectureArtifactProjection.readFullArchitectureArtifactRefWhenNeeded",
    "sourceContext.architectureArtifactProjection.engineeringBoundary",
    "sourceContext.architectureArtifactProjection.modules",
    "sourceContext.architectureArtifactProjection.artifactRefs",
    "sourceContext.architectureArtifactProjection.entityDetails",
    "sourceContext.architectureArtifactProjection.dataConstraints",
    "sourceContext.architectureArtifactProjection.relationships",
    "sourceContext.architectureArtifactProjection.interfaceContracts",
    "sourceContext.architectureArtifactProjection.userFlowDetails",
    "sourceContext.architectureArtifactProjection.stateMachineDetails",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "frontend_guidance", true, (field) => hasExplicitFrontendRequirement && hasField(field), [
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
    "task.frontendExperienceRequirement.executionGuidance.mustNot",
    "task.frontendExperienceRequirement.executionGuidance.dataBindingExpectation",
    "task.frontendExperienceRequirement.executionGuidance.bindingProjectionRules",
    "task.frontendExperienceRequirement.executionGuidance.bindingProjectionSummary",
    "task.frontendExperienceRequirement.executionGuidance.taskResultEvidenceGuide",
    "task.frontendExperienceRequirement.executionGuidance.guidanceWarnings",
    "task.frontendExperienceRequirement.executionGuidance.unresolvedBindingInputs",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "frontend_closure_details", false, (field) => hasExplicitFrontendRequirement && hasField(field), [
    "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
    "task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "execution_rules", true, hasField, [
    "executionRules.completionBarrier",
    "executionRules.completionContinuityRequirement",
    "executionRules.sourceEditPreparationContract",
    "executionRules.interactiveVerificationProbePolicy",
    "executionRules.frontendExperienceExecutionRules",
    "executionRules.runtimeDeliveryExecutionRules",
    "executionRules.environmentPreparation",
    "executionRules.selfRepairPolicy",
    "executionRules.rules",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "result_contract", true, hasField, [
    "enumRefs",
    "outputContract.resultFile",
    "outputContract.requiredTopLevelFields",
    "outputContract.requiredTopLevelFieldRule",
    "outputContract.requiredRuntimeEvidence",
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
    "outputContract.schemaShape.conceptEvidence",
    "outputContract.schemaShape.notes",
    "outputContract.schemaShape.blockedReasons",
    "outputContract.schemaShape.createdAt",
    "outputContract.schemaShape.updatedAt",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "result_frontend_self_check_contract", false, hasField, [
    "outputContract.schemaShape.frontendExperienceSelfCheck.requirementRef",
    "outputContract.schemaShape.frontendExperienceSelfCheck.status",
    "outputContract.schemaShape.frontendExperienceSelfCheck.surfacesTouched",
    "outputContract.schemaShape.frontendExperienceSelfCheck.workflowsCovered",
    "outputContract.schemaShape.frontendExperienceSelfCheck.userActionsImplemented",
    "outputContract.schemaShape.frontendExperienceSelfCheck.interactionStatesCovered",
    "outputContract.schemaShape.frontendExperienceSelfCheck.dataBinding",
    "outputContract.schemaShape.frontendExperienceSelfCheck.knownGaps",
    "outputContract.schemaShape.frontendExperienceSelfCheck.notes",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "result_frontend_closure_contract", false, hasField, [
    "outputContract.schemaShape.frontendExperienceSelfCheck.closureRequirementIds",
    "outputContract.schemaShape.frontendExperienceSelfCheck.workflowClosureEvidenceRule",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "runtime_result_contract", false, hasField, [
    "outputContract.schemaShape.runtimeDeliveryEvidence",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "frontend_binding_details", false, (field) => hasExplicitFrontendRequirement && hasField(field), [
    "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "runtime_guidance", false, hasField, [
    "task.runtimeDeliveryRequirement",
  ]);
  pushTakenAvailableGroup(groups, used, actionKind, "dependency_context", false, hasField, [
    "sourceRefs",
    "sourceContext.dependencyResults",
  ]);

  const remainingRequiredFields = requiredFields.filter((field) => !fieldCoveredByUsed(field, used));
  const remainingOptionalFields = optionalFields.filter((field) => !fieldCoveredByUsed(field, used));
  pushRemainingGroup(groups, used, actionKind, "required_context", true, remainingRequiredFields);
  pushRemainingGroup(groups, used, actionKind, "optional_context", false, remainingOptionalFields);
  return groups;
}

function buildArchitectureSectionFieldGroups(actionKind: string, requiredFields: string[], optionalFields: string[]): AgentActionReadGroup[] {
  const groups: AgentActionReadGroup[] = [];
  const used = new Set<string>();
  pushTakenGroup(groups, used, actionKind, "current_target", true, requiredFields, [
    "agentAction.write.currentTarget",
  ]);
  pushTakenGroup(groups, used, actionKind, "section_authority", true, requiredFields, [
    "sourceRefs.planningContractRef",
    "sourceRefs.technicalBaselineRef",
    "contextProjection.phaseId",
    "contextProjection.planningContractId",
    "contextProjection.phaseScope",
    "contextProjection.requirementDetailTransfer",
    "allowedRefs",
  ]);
  pushTakenGroup(groups, used, actionKind, "section_contract", true, requiredFields, [
    "generationProtocol",
    "rules.requirementDetailTransfer",
    "rules.allowedRefsAuthority",
    "outputContract.allowedRefsUsage",
    "frontendExperienceSource",
  ]);
  pushRemainingGroup(groups, used, actionKind, "required_context", true, requiredFields);
  pushRemainingGroup(groups, used, actionKind, "optional_context", false, optionalFields);
  return groups;
}

function buildRepositoryContextFieldGroups(actionKind: string, requiredFields: string[], optionalFields: string[]): AgentActionReadGroup[] {
  const groups: AgentActionReadGroup[] = [];
  const used = new Set<string>();
  pushTakenGroup(groups, used, actionKind, "scan_scope", true, requiredFields, [
    "scanPurpose",
  ]);
  pushTakenGroup(groups, used, actionKind, "generation_rules", true, requiredFields, [
    "generationRules",
    "enumRefs",
  ]);
  pushTakenGroup(groups, used, actionKind, "candidate_contract", true, requiredFields, [
    "outputContract.schemaShape",
    "outputContract.referenceRules",
  ]);
  pushRemainingGroup(groups, used, actionKind, "required_context", true, requiredFields);
  pushRemainingGroup(groups, used, actionKind, "optional_context", false, optionalFields);
  return groups;
}

function buildReviewGateFieldGroups(actionKind: string, requiredFields: string[], optionalFields: string[]): AgentActionReadGroup[] {
  const groups: AgentActionReadGroup[] = [];
  const used = new Set<string>();
  pushTakenGroup(groups, used, actionKind, "evidence_core", true, requiredFields, [
    "reviewPacketRef",
    "changeContextRef",
    "conceptReviewMatrix",
  ]);
  pushTakenGroup(groups, used, actionKind, "policy", true, requiredFields, [
    "reviewRules.commonRules",
    "reviewRules.changeSetRules",
    "outputContract.severityPolicy",
    "outputContract.routingRules",
    "outputContract.conceptReviewRules",
    "outputContract.frontendExperienceReview",
    "outputContract.reviewSignals",
  ]);
  pushTakenGroup(groups, used, actionKind, "result_contract", true, requiredFields, [
    "enumRefs",
    "outputContract.schemaShape",
    "outputContract.allowedRefs",
  ]);
  pushRemainingGroup(groups, used, actionKind, "required_context", true, requiredFields);
  pushRemainingGroup(groups, used, actionKind, "optional_context", false, optionalFields);
  return groups;
}

function buildRepairCandidateFieldGroups(actionKind: string, requiredFields: string[], optionalFields: string[]): AgentActionReadGroup[] {
  const groups: AgentActionReadGroup[] = [];
  const used = new Set<string>();
  pushTakenGroup(groups, used, actionKind, "repair_scope", true, requiredFields, [
    "inputs",
    "repairRules",
    "resumePolicy",
  ]);
  pushTakenGroup(groups, used, actionKind, "candidate_contract", true, requiredFields, [
    "enumRefs",
    "outputContract",
  ]);
  pushRemainingGroup(groups, used, actionKind, "required_context", true, requiredFields);
  pushRemainingGroup(groups, used, actionKind, "optional_context", false, optionalFields);
  return groups;
}

function buildBrainstormFieldGroups(actionKind: string, requiredFields: string[], optionalFields: string[], rawOptionalFields: string[]): AgentActionReadGroup[] {
  const availableFields = uniqueFieldNames([...requiredFields, ...optionalFields, ...rawOptionalFields]);
  const hasField = (field: string): boolean => fieldAvailable(field, availableFields);
  const hasExactField = (field: string): boolean => availableFields.includes(field);
  const hasFieldOrDescendant = (field: string): boolean => availableFields.some((available) =>
    field === available || fieldIsDescendantOf(available, field)
  );
  const hasLatestRepositoryContext = hasFieldOrDescendant("latestRepositoryContext") || hasExactField("contextRefs.latestRepositoryContextRef");
  const hasKeywordHintsRef = hasExactField("contextRefs.keywordHintsRef");
  const hasKeywordHintsField = hasFieldOrDescendant("keywordHints");

  const phaseScopeCoreFields = filterAvailable([
    "agentAction.instruction",
    "agentAction.stopConditions",
    "originalRequest",
    "userFacingLanguage",
    "contextRefs",
    "sourceFieldAccessHints",
    "firstClarificationGate",
    "clarificationConversationProtocol.mode",
    "clarificationConversationProtocol.oneTopicPerTurn",
    "clarificationConversationProtocol.maxOptionsPerQuestion",
    "clarificationConversationProtocol.avoidSchemaLanguageToUser",
    "clarificationConversationProtocol.requiredBlocks",
    "clarificationConversationProtocol.blockConfirmationRules.phase_scope",
    "rules.phaseScopeOptionComparison",
    "clarificationGuidance",
    "riskGuidance",
    "confirmationRules",
  ], hasField);
  const knowledgeContextProtocolFields = filterAvailable([
    "knowledgeContextProtocol.status",
    "knowledgeContextProtocol.purpose",
    "knowledgeContextProtocol.appliesToBlocks",
    "knowledgeContextProtocol.excludedBlocks",
    "knowledgeContextProtocol.command",
    "knowledgeContextProtocol.matchQueryShape",
    "knowledgeContextProtocol.perBlockLimits",
    "knowledgeContextProtocol.blockRules",
    "knowledgeContextProtocol.candidateBoundaryRules",
    "knowledgeQueryPlan",
  ], hasField);
  const phaseScopeAuthorityFields = [
    ...filterAvailable([
      "phaseContinuationContext",
      "requirementContext.sourceItems",
      "requirementContext.normalizedText",
      "deliveryContext.sources",
      "latestConfirmedRequirementDecision",
      "confirmedRequirementDecisionsIndex",
    ], hasField),
    ...(hasLatestRepositoryContext
      ? [
          "latestRepositoryContext.requestLens",
          "latestRepositoryContext.repoOverview",
          "latestRepositoryContext.existingCapabilities",
          "latestRepositoryContext.roadmapImplications",
          "latestRepositoryContext.warnings",
          "latestRepositoryContext.contextQuality",
        ]
      : []),
  ];
  const conceptGroundingFields = filterAvailable([
    "conceptGroundingRequest",
    "clarificationConversationProtocol.blockExecutionRules",
    "clarificationConversationProtocol.blockConfirmationRules.concept_grounding",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.scopeItemCoverageContract",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.objectOperationContract",
    "deliveryConceptGlossary",
    "phaseConceptGrounding",
  ], hasField);
  const frontendExperienceFields = filterAvailable([
    "clarificationConversationProtocol.blockConfirmationRules.frontend_experience",
    "clarificationConversationProtocol.frontendBlockRequiredWhen",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.frontendOperationPathContract",
    "currentFrontendExperience",
  ], hasField);
  const keywordHintFields = hasKeywordHintsRef || hasKeywordHintsField ? ["keywordHints.compact"] : [];
  const candidateWriteControlFields = filterAvailable([
    "agentAction.write",
    "agentAction.submit",
    "agentAction.schema",
    "outputContract.candidateFile",
    "outputContract.schemaRef",
    "generationProtocol",
    "enumRefs",
  ], hasField);
  const candidateSchemaIdentityFields = filterAvailable([
    "outputContract.schemaShape.schemaVersion",
    "outputContract.schemaShape.candidateId",
    "outputContract.schemaShape.brainstormRunId",
    "outputContract.schemaShape.deliveryId",
    "outputContract.schemaShape.phaseId",
    "outputContract.schemaShape.status",
    "outputContract.schemaShape.requestSummary",
    "outputContract.schemaShape.sources",
  ], hasField);
  const candidateSchemaScopeFields = filterAvailable([
    "outputContract.schemaShape.scope",
    "outputContract.schemaShape.roadmap",
    "outputContract.schemaShape.phasePlan",
    "outputContract.schemaShape.acceptance",
  ], hasField);
  const candidateSchemaConceptFields = filterAvailable([
    "outputContract.schemaShape.domainModel",
    "outputContract.schemaShape.conceptGrounding",
    "outputContract.schemaShape.conceptConfirmation",
    "outputContract.schemaShape.clarificationProgress",
  ], hasField);
  const candidateSchemaFrontendFields = filterAvailable([
    "outputContract.schemaShape.frontendExperience",
  ], hasField);
  const candidateSchemaHandoffFields = filterAvailable([
    "outputContract.schemaShape.handoff",
  ], hasField);
  const candidateFinalSummaryRuleFields = filterAvailable([
    "clarificationConversationProtocol.blockConfirmationRules.final_summary",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.appliesWhenAgentFinds",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.requiredUserVisibleTopicsWhenApplicable",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.notApplicableRule",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.candidateFieldMapping",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.finalSummaryReviewContract",
  ], hasField);
  const candidateRequirementSemanticRuleFields = filterAvailable([
    "rules.requirementSemanticGrounding.compactRules",
  ], hasField);
  const candidateSelfReviewRuleFields = filterAvailable([
    "rules.candidateSelfReview",
    "rules.nextPhasePreviewGeneration",
  ], hasField);

  const groups: AgentActionReadGroup[] = [];
  pushGroup(groups, actionKind, "phase_scope_core", true, phaseScopeCoreFields);
  pushGroup(groups, actionKind, "phase_scope_authority", true, phaseScopeAuthorityFields);
  pushGroup(groups, actionKind, "knowledge_context_protocol", true, knowledgeContextProtocolFields);
  pushGroup(groups, actionKind, "concept_grounding_context", false, conceptGroundingFields);
  pushGroup(groups, actionKind, "frontend_experience_context", false, frontendExperienceFields);
  pushGroup(groups, actionKind, "keyword_hints_advisory", false, keywordHintFields);
  pushGroup(groups, actionKind, "candidate_write_controls", false, candidateWriteControlFields);
  pushGroup(groups, actionKind, "candidate_schema_identity", false, candidateSchemaIdentityFields);
  pushGroup(groups, actionKind, "candidate_schema_scope", false, candidateSchemaScopeFields);
  pushGroup(groups, actionKind, "candidate_schema_concepts", false, candidateSchemaConceptFields);
  pushGroup(groups, actionKind, "candidate_schema_frontend", false, candidateSchemaFrontendFields);
  pushGroup(groups, actionKind, "candidate_schema_handoff", false, candidateSchemaHandoffFields);
  pushGroup(groups, actionKind, "candidate_final_summary_rules", false, candidateFinalSummaryRuleFields);
  pushGroup(groups, actionKind, "candidate_requirement_semantic_rules", false, candidateRequirementSemanticRuleFields);
  pushGroup(groups, actionKind, "candidate_self_review_rules", false, candidateSelfReviewRuleFields);

  const groupedFields = new Set(groups.flatMap((group) => group.fields));
  const remainingFields = availableFields.filter((field) =>
    !fieldCoveredByUsed(field, groupedFields) &&
    !field.startsWith("contextRefs.") &&
    field !== "requestManifest" &&
    field !== "latestRepositoryContext" &&
    field !== "keywordHints"
  );
  pushGroup(groups, actionKind, "additional_context", false, remainingFields);
  return groups;
}

function pushTakenGroup(
  groups: AgentActionReadGroup[],
  used: Set<string>,
  actionKind: string,
  suffix: string,
  required: boolean,
  availableFields: string[],
  wantedFields: string[],
): void {
  const fields = wantedFields.filter((field) => availableFields.includes(field) && !used.has(field));
  for (const field of fields) {
    used.add(field);
  }
  pushGroup(groups, actionKind, suffix, required, fields);
}

function pushTakenAvailableGroup(
  groups: AgentActionReadGroup[],
  used: Set<string>,
  actionKind: string,
  suffix: string,
  required: boolean,
  hasField: (field: string) => boolean,
  wantedFields: string[],
): void {
  const fields = wantedFields.filter((field) => hasField(field) && !fieldCoveredByUsed(field, used));
  for (const field of fields) {
    used.add(field);
  }
  pushGroup(groups, actionKind, suffix, required, fields);
}

function pushRemainingGroup(
  groups: AgentActionReadGroup[],
  used: Set<string>,
  actionKind: string,
  suffix: string,
  required: boolean,
  fields: string[],
): void {
  const remaining = fields.filter((field) => !used.has(field));
  for (const field of remaining) {
    used.add(field);
  }
  pushGroup(groups, actionKind, suffix, required, remaining);
}

function filterAvailable(fields: string[], hasField: (field: string) => boolean): string[] {
  return fields.filter((field) => hasField(field));
}

function fieldAvailable(field: string, availableFields: string[]): boolean {
  return availableFields.some((available) =>
    field === available ||
    fieldIsDescendantOf(field, available) ||
    fieldIsDescendantOf(available, field)
  );
}

function fieldCoveredByUsed(field: string, used: Set<string>): boolean {
  for (const usedField of used) {
    if (
      field === usedField ||
      fieldIsDescendantOf(field, usedField) ||
      fieldIsDescendantOf(usedField, field)
    ) {
      return true;
    }
  }
  return false;
}

function pushGroup(groups: AgentActionReadGroup[], actionKind: string, suffix: string, required: boolean, fields: string[]): void {
  const normalizedFields = dedupeCoveredFieldPaths(fields);
  if (normalizedFields.length === 0) {
    return;
  }
  groups.push(fieldGroupFor(actionKind, suffix, required, normalizedFields));
}

function fieldGroupFor(actionKind: string, suffix: string, required: boolean, fields: string[]): AgentActionReadGroup {
  const groupId = `${readableActionKind(actionKind)}_${suffix}`;
  return {
    groupId,
    required,
    purpose: purposeForFieldGroup(groupId, fields),
    whenToRead: whenToReadForFieldGroup(groupId, fields),
    fields,
    readCommand: inspectReadCommand(fields),
    fallbackRule: "Fallback: requestManifest refs; compact output.",
  };
}

function inspectReadCommand(fields: string[]): AgentActionReadGroup["readCommand"] {
  return {
    name: "inspect",
    argv: ["inspect", "--request", "{requestRef}", "--field", uniqueFieldNames(fields).join(",")],
  };
}

function groupIdFromFields(fields: string[]): string {
  if (fields.length === 1) {
    return `${fields[0].replace(/[^A-Za-z0-9]+/g, "_").replace(/^_+|_+$/g, "")}_field`;
  }
  return "request_fields";
}

function readableActionKind(actionKind: string): string {
  return actionKind.replace(/[^A-Za-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "request";
}

function uniqueFieldNames(fields: string[]): string[] {
  return [...new Set(fields.map((field) => field.trim()).filter((field) => field.length > 0))];
}

function dedupeCoveredFieldPaths(fields: string[]): string[] {
  const unique = uniqueFieldNames(fields);
  return unique.filter((field) => !unique.some((candidate) => candidate !== field && fieldIsDescendantOf(field, candidate)));
}

function removeFieldsCoveredBy(fields: string[], coveringFields: string[]): string[] {
  const covering = uniqueFieldNames(coveringFields);
  return uniqueFieldNames(fields).filter((field) => !covering.some((candidate) =>
    fieldIsDescendantOf(field, candidate) ||
    fieldIsDescendantOf(candidate, field) ||
    field === candidate
  ));
}

function fieldIsDescendantOf(field: string, candidateParent: string): boolean {
  return field.startsWith(`${candidateParent}.`);
}

function contextRefKeyForReadField(field: string): string | null {
  if (field.startsWith("contextRefs.")) {
    const refKey = field.slice("contextRefs.".length).split(".")[0];
    return refKey.length > 0 ? refKey : null;
  }
  if (field.startsWith("requirementContext.")) return "requirementContextRef";
  if (field.startsWith("originalRequirementContext.")) return "originalRequirementContextRef";
  if (field === "keywordHints" || field.startsWith("keywordHints.")) return "keywordHintsRef";
  if (field.startsWith("deliveryContext.")) return "deliveryContextRef";
  if (field === "latestRepositoryContext" || field.startsWith("latestRepositoryContext.")) return "latestRepositoryContextRef";
  if (field === "latestConfirmedRequirementDecision" || field.startsWith("latestConfirmedRequirementDecision.")) return "latestConfirmedRequirementDecisionRef";
  if (field === "confirmedRequirementDecisionsIndex" || field.startsWith("confirmedRequirementDecisionsIndex.")) return "confirmedRequirementDecisionsIndexRef";
  if (field === "deliveryConceptGlossary" || field.startsWith("deliveryConceptGlossary.")) return "deliveryConceptGlossaryRef";
  if (field === "phaseConceptGrounding" || field.startsWith("phaseConceptGrounding.")) return "phaseConceptGroundingRef";
  if (field === "currentFrontendExperience" || field.startsWith("currentFrontendExperience.")) return "currentFrontendExperienceRef";
  return null;
}

function requestFieldAvailability(request: Record<string, unknown>, field: string): "present" | "missing" | "unknown" {
  const parts = field.split(".").filter(Boolean);
  if (parts.length === 0) {
    return "missing";
  }
  const [rootKey, ...rest] = parts;
  if (rootKey === "requestManifest") {
    return "unknown";
  }
  const contextRefs = isRecord(request.contextRefs) ? request.contextRefs : {};
  const contextRefKey = contextRefKeyForReadField(field);
  if (contextRefKey && contextRefKey in contextRefs) {
    return "unknown";
  }
  if (!(rootKey in request)) {
    const rootRefKey = `${rootKey}Ref`;
    const manifestRefs = isRecord(request.requestManifest) && isRecord(request.requestManifest.refs)
      ? request.requestManifest.refs
      : {};
    return rootRefKey in request || rootKey in manifestRefs ? "unknown" : "missing";
  }
  let current: unknown = request[rootKey];
  for (const part of rest) {
    if (Array.isArray(current)) {
      return "unknown";
    }
    if (!isRecord(current)) {
      return "missing";
    }
    if (!(part in current)) {
      return "missing";
    }
    current = current[part];
  }
  return "present";
}

function normalizeLegacyReadFields(value: unknown): AgentActionReadField[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((field): field is Record<string, unknown> => isReadField(field))
    .map((field) => ({
      ...field,
      field: field.field,
      required: typeof field.required === "boolean" ? field.required : true,
    })) as AgentActionReadField[];
}

function isReadField(value: unknown): value is Record<string, unknown> & { field: string } {
  return isRecord(value) && typeof value.field === "string" && value.field.trim().length > 0;
}

function purposeForFieldGroup(groupId: string, fields: string[]): string {
  if (groupId === "brainstorm_session_phase_scope_core") return "Brainstorm phase_scope gate controls, request refs, conversation rules, and root user request fields needed before asking the current phase scope question.";
  if (groupId === "brainstorm_session_phase_scope_authority") return "Source-grounded phase_scope authority fields: original requirements, prior confirmed requirement decisions, current phase seed, and narrow repository fact selectors.";
  if (groupId === "brainstorm_session_knowledge_context_protocol") return "Block-scoped knowledge matching protocol for Brainstorm clarification; reference-only context that must not be written as candidate sources.";
  if (groupId === "brainstorm_session_concept_grounding_context") return "Brainstorm concept-grounding context to read only when presenting or writing the concept confirmation block.";
  if (groupId === "brainstorm_session_frontend_experience_context") return "Brainstorm frontend-experience context to read only when presenting or writing the frontend experience block.";
  if (groupId === "brainstorm_session_keyword_hints_advisory") return "Advisory keyword hints for Brainstorm concept discovery and clarification only; never use this group as scope or acceptance authority.";
  if (groupId === "brainstorm_session_candidate_write_controls") return "Candidate write, submit, schema, enum controls.";
  if (groupId === "brainstorm_session_candidate_schema_identity") return "BrainstormCandidate identity/source schema selectors.";
  if (groupId === "brainstorm_session_candidate_schema_scope") return "BrainstormCandidate scope/roadmap/acceptance schema.";
  if (groupId === "brainstorm_session_candidate_schema_concepts") return "BrainstormCandidate domain/concept schema.";
  if (groupId === "brainstorm_session_candidate_schema_frontend") return "BrainstormCandidate frontend schema.";
  if (groupId === "brainstorm_session_candidate_schema_handoff") return "BrainstormCandidate handoff schema.";
  if (groupId === "brainstorm_session_candidate_final_summary_rules") return "Final summary review and business-detail rule selectors.";
  if (groupId === "brainstorm_session_candidate_requirement_semantic_rules") return "Requirement semantic grounding rule list.";
  if (groupId === "brainstorm_session_candidate_self_review_rules") return "Candidate self-review and next-phase rules.";
  if (groupId === "execute_task_task_core") return "Task identity, objective, write boundary, acceptance refs, concept refs, and verification intents needed before editing project files.";
  if (groupId === "execute_task_acceptance_and_concepts") return "Task-scoped acceptance and concept grounding authority for implementation coverage.";
  if (groupId === "execute_task_architecture_context") return "Task-scoped AAC projection fields needed for source edits without reading the entire architecture projection at once.";
  if (groupId === "execute_task_frontend_guidance") return "Task-scoped frontend surfaces, workflows, states, and data-binding expectations. This excludes detailed closure and binding payloads until needed.";
  if (groupId === "execute_task_frontend_closure_details") return "Detailed workflow closure requirements needed before claiming required frontend closure as satisfied.";
  if (groupId === "execute_task_execution_rules") return "Task execution, verification, runtime probe, self-repair, and source-edit rules split from larger request context.";
  if (groupId === "execute_task_result_contract") return "TaskResult writing contract and schema subfields needed before writing resultFile, without reading the entire schemaShape object.";
  if (groupId === "execute_task_result_frontend_self_check_contract") return "Frontend self-check result schema subfields needed before writing frontendExperienceSelfCheck.";
  if (groupId === "execute_task_result_frontend_closure_contract") return "Frontend closure evidence schema details needed only before writing satisfied closure evidence.";
  if (groupId === "execute_task_runtime_result_contract") return "RuntimeDeliveryEvidence result schema needed only when runtime delivery evidence applies to this task.";
  if (groupId === "execute_task_frontend_binding_details") return "Detailed frontend/backend binding projection for cases where architecture context and frontend guidance are insufficient. This is guidance, not an API allowlist.";
  if (groupId === "execute_task_runtime_guidance") return "Runtime delivery requirement details to read when this task needs runtime evidence beyond agentAction.write.requiredRuntimeEvidence.";
  if (groupId === "execute_task_dependency_context") return "Source refs and dependency task result summaries for targeted fallback reads.";
  if (groupId.includes("brainstorm")) return "Grouped Brainstorm request fields needed to present the current clarification gate and write the candidate.";
  if (groupId.includes("execute_task")) return "Grouped TaskExecutionRequest fields needed before editing project files, verifying work, and writing TaskResult.";
  if (groupId.includes("generate_taskplan")) return "Grouped TaskPlan generation fields needed to write the outline and grouped task candidates.";
  if (groupId.includes("generate") || groupId.includes("sections")) return "Grouped candidate-generation fields needed to write the requested artifact.";
  if (groupId.includes("review")) return "Grouped review fields needed to evaluate implementation evidence and route the next action.";
  if (groupId.includes("repair")) return "Grouped repair fields needed to repair the requested artifact or execution result.";
  if (fields.length === 1) return `Single request field: ${fields[0]}.`;
  return "Grouped request fields that should be read together before acting.";
}

function whenToReadForFieldGroup(groupId: string, fields: string[]): string {
  if (groupId === "brainstorm_session_phase_scope_core") return "First read before asking the phase_scope question.";
  if (groupId === "brainstorm_session_phase_scope_authority") return "Read with phase_scope_core for source-grounded scope options.";
  if (groupId === "brainstorm_session_knowledge_context_protocol") return "Read before each phase_scope, concept_grounding, or frontend_experience block; do not read or run knowledge context for final_summary.";
  if (groupId === "brainstorm_session_concept_grounding_context") return "Only for concept_grounding or candidate concept fields.";
  if (groupId === "brainstorm_session_frontend_experience_context") return "Only for frontend_experience or frontend candidate fields.";
  if (groupId === "brainstorm_session_keyword_hints_advisory") return "Only when concept extraction, terminology ambiguity, or candidate discovery needs advisory hints. Do not read this group by default for phase_scope, and never use keywordHints as scope or acceptance authority.";
  if (groupId === "brainstorm_session_candidate_write_controls") return "Only after the user has explicitly confirmed the dedicated final_summary block, immediately before writing BrainstormCandidate or running submitCommand. Do not read this group after phase_scope, concept_grounding, or frontend_experience confirmations.";
  if (groupId === "brainstorm_session_candidate_schema_identity") return "Only after final_summary confirmation, before writing identity, summary, or source metadata.";
  if (groupId === "brainstorm_session_candidate_schema_scope") return "Only after final_summary confirmation, before writing scope, roadmap, phasePlan, or acceptance.";
  if (groupId === "brainstorm_session_candidate_schema_concepts") return "Only after final_summary confirmation, when writing concept or clarification progress fields.";
  if (groupId === "brainstorm_session_candidate_schema_frontend") return "Only after final_summary confirmation, when frontendExperience applies.";
  if (groupId === "brainstorm_session_candidate_schema_handoff") return "Only after final_summary confirmation, before setting handoff routing fields.";
  if (groupId === "brainstorm_session_candidate_final_summary_rules") return "Read before presenting the final_summary block; read again after final_summary confirmation before writing business-detail candidate fields.";
  if (groupId === "brainstorm_session_candidate_requirement_semantic_rules") return "Only after final_summary confirmation, when detailed requirement semantic grounding rules are needed.";
  if (groupId === "brainstorm_session_candidate_self_review_rules") return "Only after final_summary confirmation, before final self-review or submitCommand.";
  if (groupId === "execute_task_task_core") return "First TaskExecution read, before deciding source edits or verification approach.";
  if (groupId === "execute_task_acceptance_and_concepts") return "Read before editing to preserve acceptance and concept coverage.";
  if (groupId === "execute_task_architecture_context") return "Read before editing project files; use sourceRefs.architectureArtifactContractRef as targeted fallback only when this projection lacks a required detail.";
  if (groupId === "execute_task_frontend_guidance") return "Read before implementing or verifying a frontend workflow, UI state, or data-binding responsibility.";
  if (groupId === "execute_task_frontend_closure_details") return "Read before implementing required workflow closure details or before claiming frontendExperienceSelfCheck.status=satisfied.";
  if (groupId === "execute_task_execution_rules") return "Read before editing, running verification, starting runtime probes, or attempting self-repair.";
  if (groupId === "execute_task_result_contract") return "Read after implementation/verification and before writing TaskResult.";
  if (groupId === "execute_task_result_frontend_self_check_contract") return "Read after implementation/verification and before writing frontendExperienceSelfCheck.";
  if (groupId === "execute_task_result_frontend_closure_contract") return "Read before writing satisfied frontend closure evidence or repairing workflow closure self-check issues.";
  if (groupId === "execute_task_runtime_result_contract") return "Read before writing runtimeDeliveryEvidence when runtime delivery evidence applies.";
  if (groupId === "execute_task_frontend_binding_details") return "Read only when the frontend task needs explicit user-action-to-interface binding details beyond architecture context and frontend guidance.";
  if (groupId === "execute_task_runtime_guidance") return "Read when runtimeDeliveryEvidence is required or when runtimeDeliveryExecutionRules point to the task runtime requirement.";
  if (groupId === "execute_task_dependency_context") return "Read only when a dependency result or source authority ref is needed for this task.";
  if (groupId.endsWith("_optional_context")) return "After required groups, when the task needs additional context or a required field points to this context.";
  if (fields.some((field) => field === "task" || field.startsWith("sourceContext") || field === "executionRules")) return "Before editing project files, running verification, or writing TaskResult.";
  if (fields.some((field) => field === "firstClarificationGate" || field === "clarificationConversationProtocol")) return "Before presenting a Brainstorm confirmation block or writing a BrainstormCandidate.";
  if (fields.some((field) => field.startsWith("outputContract") || field === "enumRefs")) return "Before writing the candidate/result artifact or deciding submit behavior.";
  return "Before using these fields for generation, execution, review, repair, or submit decisions.";
}

function readFieldFromLabel(label: string, required: boolean): AgentActionReadField | null {
  const field = normalizeFieldLabel(label);
  if (!field || isSelfRequestLabel(field) || field === "referencedArtifactReadGuide" || !isStructuredFieldLabel(field)) {
    return null;
  }
  const fallback = fallbackForField(field);
  return {
    field,
    required,
    purpose: purposeForField(field),
    whenToRead: whenToReadForField(field),
    readCommand: {
      name: "inspect",
      argv: ["inspect", "--request", "{requestRef}", "--field", field],
    },
    fallback,
  };
}

function isSelfRequestLabel(field: string): boolean {
  return field.toLowerCase() === "this request" || /^this\s+[A-Za-z]+Request$/.test(field);
}

function isStructuredFieldLabel(field: string): boolean {
  return /^[A-Za-z_$][A-Za-z0-9_$]*(\.[A-Za-z_$][A-Za-z0-9_$]*)*$/.test(field);
}

function normalizeFieldLabel(label: string): string {
  return label
    .replace(/\s+when\s+.+$/i, "")
    .replace(/\s+if\s+.+$/i, "")
    .trim();
}

function fallbackForField(field: string): AgentActionReadField["fallback"] {
  const [refKey, ...rest] = field.split(".");
  const selector = rest.length > 0 ? `.${rest.join(".")}` : "$";
  return {
    method: "request_manifest_refs",
    refKey: refKey.length > 0 ? refKey : null,
    selector,
    source: "request_manifest_ref",
  };
}

function purposeForField(field: string): string {
  if (field === "agentAction") return "Request-owned action plan, including required inspect read fields, write target, schema location, and submit command.";
  if (field === "requestManifest") return "Ref-first authority map for request sidecars; use it instead of probing unlisted paths.";
  if (field === "originalRequest") return "User-provided requirement text and input refs for Brainstorm clarification.";
  if (field === "userFacingLanguage") return "Delivery-level constraint for user-visible copy language; it applies to labels, messages, navigation, visible status/results, and feedback, not code identifiers or internal ids.";
  if (field === "contextRefs") return "External context refs available to this request, such as requirement, delivery, repository, glossary, frontend, and keyword refs.";
  if (field === "sourceFieldAccessHints") return "Selector and field-name rules for reading source facts and writing BrainstormCandidate.sources without guessing schemas.";
  if (field === "knowledgeContextProtocol") return "Block-scoped knowledge matching protocol for Brainstorm; reference-only and never a BrainstormCandidate source.";
  if (field === "knowledgeQueryPlan") return "Execution sequence for Brainstorm knowledge queries; follow it before presenting phase_scope, concept_grounding, or frontend_experience.";
  if (field === "firstClarificationGate") return "Required Brainstorm confirmation gate before a candidate may be submitted.";
  if (field === "clarificationConversationProtocol") return "Required progressive clarification block order and confirmation rules.";
  if (field === "conceptGroundingRequest") return "Business concept extraction, presentation, and confirmation requirements for Brainstorm.";
  if (field === "phaseContinuationContext") return "Current phase continuation seed and boundaries for follow-up Brainstorm.";
  if (field === "clarificationGuidance" || field === "riskGuidance" || field === "confirmationRules") return "Brainstorm conversation and confirmation rules.";
  if (field === "requirementContext.sourceItems") return "Normalized requirement source records for initial-delivery Brainstorm.";
  if (field === "requirementContext.normalizedText") return "Normalized full requirement text for Brainstorm when root originalRequest text is insufficient.";
  if (field === "keywordHints.compact") return "Compact advisory keyword hints for concept grounding and clarification questions; never a scope or acceptance authority.";
  if (field === "keywordHints") return "Full advisory keyword hints for concept grounding and clarification questions; never a scope or acceptance authority.";
  if (field === "deliveryContext.sources") return "Existing delivery source facts for phase-continuation Brainstorm.";
  if (field.startsWith("latestRepositoryContext.")) return "Narrow current repository fact selector for phase-continuation Brainstorm; never current phase scope authority by itself.";
  if (field === "latestRepositoryContext") return "Latest repository fact snapshot for phase-continuation Brainstorm.";
  if (field === "latestConfirmedRequirementDecision") return "Most recent confirmed phase requirement-decision snapshot, including confirmed scope, acceptance, business semantics, concepts, frontend target, and next phase seed.";
  if (field === "confirmedRequirementDecisionsIndex") return "Index of confirmed phase decision snapshots for resolving explicit references to earlier confirmed phase scope or requirement changes.";
  if (field === "deliveryConceptGlossary" || field === "phaseConceptGrounding") return "Previously confirmed concept grounding context for phase-continuation Brainstorm.";
  if (field === "currentFrontendExperience") return "Previously confirmed frontend target context for phase-continuation Brainstorm.";
  if (field === "task") return "Current task objective, refs, write boundary, frontend/runtime responsibilities, and verification intents.";
  if (field === "sourceContext.architectureArtifactProjection") return "Task-scoped AAC projection with relevant entity fields/constraints, interfaces, user flow steps/outcomes, and state machine rules needed for implementation.";
  if (field.startsWith("sourceContext.acceptanceSnapshot")) return "Canonical acceptance statements scoped to this task.";
  if (field === "taskConceptGrounding") return "Task-scoped concept meanings, guardrails, responsibilities, and evidence expectations.";
  if (field.startsWith("outputContract")) return "Output paths, schema fields, enum constraints, and result writing rules.";
  if (field === "enumRefs") return "Allowed enum values for generated candidate/result fields.";
  if (field === "executionRules") return "Execution boundaries, verification closeout, self-repair, runtime probe, and completion rules.";
  if (field === "sourceRefs") return "Authoritative source artifact refs for the current request.";
  if (field === "allowedRefs") return "Allowed scope, acceptance, concept, surface, and reference ids for generated output.";
  if (field === "generationRules" || field === "reviewRules" || field === "repairRules") return "Rules required to generate or repair the requested artifact.";
  return `Field required by this ${field.includes(".") ? "request sub-contract" : "request"}.`;
}

function whenToReadForField(field: string): string {
  if (
    field === "agentAction" ||
    field === "requestManifest" ||
    field === "outputContract" ||
    field === "rules" ||
    field === "generationProtocol" ||
    field === "enumRefs"
  ) return "Before presenting a Brainstorm summary, writing a candidate/result artifact, or deciding submit behavior.";
  if (
    field === "originalRequest" ||
    field === "userFacingLanguage" ||
    field === "contextRefs" ||
    field === "sourceFieldAccessHints" ||
    field === "phaseContinuationContext" ||
    field.startsWith("requirementContext.") ||
    field.startsWith("deliveryContext.") ||
    field.startsWith("latestRepositoryContext.") ||
    field === "latestRepositoryContext" ||
    field === "latestConfirmedRequirementDecision" ||
    field === "confirmedRequirementDecisionsIndex" ||
    field === "keywordHints" ||
    field.startsWith("keywordHints.") ||
    field === "deliveryConceptGlossary" ||
    field === "phaseConceptGrounding" ||
    field === "currentFrontendExperience"
  ) return "Before presenting Brainstorm scope, source, concept, frontend, or final-summary confirmation.";
  if (
    field === "firstClarificationGate" ||
    field === "clarificationConversationProtocol" ||
    field === "conceptGroundingRequest" ||
    field === "clarificationGuidance" ||
    field === "riskGuidance" ||
    field === "confirmationRules"
  ) return "Before deciding which Brainstorm block to present or whether user confirmation is sufficient.";
  if (field === "task" || field.startsWith("sourceContext") || field === "taskConceptGrounding") return "Before editing project files or deciding implementation coverage.";
  if (field.startsWith("outputContract") || field === "enumRefs") return "Before writing or repairing the candidate/result artifact.";
  if (field === "executionRules") return "Before running verification or deciding task closeout.";
  return "Before using this field for generation, repair, review, or submit decisions.";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
