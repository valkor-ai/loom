export type ReferencedArtifactReadGuideEntry = {
  refKey: string;
  refPath: string | null;
  artifactType: string;
  purpose: string;
  requiredSelectors: string[];
  optionalSelectors?: string[];
  usageRule?: string;
  doNotGuessAlternateRoots: true;
  nullSelectorHandling: string;
};

type RefInput = Record<string, string | null | undefined>;

const ARTIFACT_GUIDES: Record<string, Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath">> = {
  brainstormContractRef: {
    artifactType: "BrainstormContract",
    purpose: "Confirmed requirement scope, current phase plan, acceptance candidates, and next phase preview.",
    requiredSelectors: [
      ".contractId",
      ".brainstormRunId",
      ".status",
      ".summary",
      ".scope",
      ".phasePlan.current",
      ".phasePlan.nextPhasePreview",
      ".roadmap.currentPhaseId",
      ".roadmap.phases",
      ".acceptance.candidates",
      ".handoff",
    ],
    optionalSelectors: [
      ".sources",
      ".domainModel",
      ".deliveryContextRef",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If one of these selectors is null, treat the field as absent in this artifact version; do not try .data, .contract, or other guessed wrapper roots.",
  },
  repoSignalSetRef: {
    artifactType: "RepoSignalSet",
    purpose: "Current repository technology signals for TechnicalBaseline inference in existing projects.",
    requiredSelectors: [
      ".projectKind",
      ".signals.manifests",
      ".signals.packageManagers",
      ".signals.languages",
      ".signals.frameworkHints",
      ".signals.testHints",
      ".signals.scripts",
      ".conflicts",
    ],
    optionalSelectors: [
      ".signals.sourceSamples",
      ".confidenceHints",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is empty, treat that signal family as not detected; do not infer missing technology from unrelated files.",
  },
  latestRepositoryContextRef: repositoryContextGuide(),
  repositoryContextRef: repositoryContextGuide(),
  previousTechnicalBaselineRef: technicalBaselineGuide("Previous technical baseline for stable stack reuse only."),
  technicalBaselineRef: technicalBaselineGuide("Authoritative technology stack, package manager, runtime, constraints, and stack evidence."),
  planningContractRef: planningGenerationContractGuide(),
  planningGenerationContractRef: planningGenerationContractGuide(),
  architectureArtifactRef: architectureArtifactGuide(),
  architectureArtifactContractRef: architectureArtifactGuide(),
  taskPlanRef: taskPlanGuide(),
  taskPlanRunRef: taskPlanRunGuide(),
  reviewResultRef: reviewResultGuide(),
  reviewPacketRef: reviewPacketGuide(),
  changeContextRef: changeContextGuide(),
  originalRequestRef: requestArtifactGuide(),
  requestTextRef: requestTextGuide(),
  originalRequirementContextRef: requirementContextGuide(),
  requirementContextRef: requirementContextGuide(),
  normalizedRequirementTextRef: normalizedRequirementTextGuide(),
  keywordHintsRef: keywordHintsGuide(),
  latestConfirmedRequirementDecisionRef: brainstormDecisionGuide(),
  confirmedRequirementDecisionsIndexRef: brainstormDecisionIndexGuide(),
  deliveryConceptGlossaryRef: conceptGroundingGuide("DeliveryConceptGlossary", "Confirmed delivery-wide business terminology and concept explanations."),
  phaseConceptGroundingRef: conceptGroundingGuide("PhaseConceptGrounding", "Current phase high-risk concepts, priorities, and misunderstanding guards."),
  confirmedFrontendExperienceRef: frontendExperienceTargetGuide("Confirmed frontend experience target for the active phase."),
  currentFrontendExperienceRef: frontendExperienceTargetGuide("Current latest frontend experience target or delta across phases."),
  deploymentFailureRef: deploymentFailureGuide(),
  failureReportRef: deploymentFailureGuide(),
  deploymentSpecRef: deploymentSpecGuide(),
  runtimeDeliveryRef: runtimeDeliveryGuide(),
  previousRuntimeDeliveryRef: runtimeDeliveryGuide("Previous accepted phase RuntimeDeliveryContract; use as stable runtime-shape authority only when the current runtimeDelivery.status is unchanged."),
};

export function referencedArtifactReadGuide(refs: RefInput): ReferencedArtifactReadGuideEntry[] {
  return Object.entries(refs)
    .filter((entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0)
    .map(([refKey, refPath]) => guideEntry(refKey, refPath));
}

export function guideEntry(refKey: string, refPath: string | null): ReferencedArtifactReadGuideEntry {
  const guide = ARTIFACT_GUIDES[refKey] ?? genericArtifactGuide();
  return {
    refKey,
    refPath,
    ...guide,
  };
}

function repositoryContextGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "RepositoryContext",
    purpose: "Current repository code facts only; never current phase scope authority.",
    requiredSelectors: [
      ".repositoryContextId",
      ".status",
      ".source",
      ".requestLens",
      ".repoOverview",
      ".technologySignals",
      ".structureSignals",
      ".implementedCapabilities",
      ".recommendedReadRefs",
      ".roadmapImplications",
    ],
    optionalSelectors: [
      ".riskSignals",
      ".limitations",
      ".notes",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, do not infer phase scope from repository facts and do not probe alternate wrapper roots.",
  };
}

function technicalBaselineGuide(purpose: string): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "TechnicalBaseline",
    purpose,
    requiredSelectors: [
      ".technicalBaselineId",
      ".status",
      ".projectKind",
      ".scope",
      ".stack",
      ".constraints",
      ".evidence",
      ".approval",
      ".confidence",
    ],
    optionalSelectors: [
      ".reasoningSummary",
      ".alternatives",
      ".requiresUserConfirmation",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, use the field as absent rather than probing guessed roots.",
  };
}

function planningGenerationContractGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "PlanningGenerationContract",
    purpose: "Current phase planning authority for scope, acceptance candidates, preserved Brainstorm requirement details, technical baseline summary, and planning rules.",
    requiredSelectors: [
      ".planningContractId",
      ".status",
      ".source.phaseId",
      ".phaseScope",
      ".phaseScope.included[].items",
      ".phaseScope.deferred[].items",
      ".phaseScope.excluded[].items",
      ".phaseScope.acceptanceCandidates[].statement",
      ".phaseScope.acceptanceCandidates[].sourceRefs",
      ".phaseScope.acceptanceCandidates[].capabilityRefs",
      ".contextRefs",
      ".technicalBaseline",
      ".planningInputs",
      ".planningInputs.businessFlows[].summary",
      ".requirementDetails",
      ".planningRules",
      ".qualityGates",
      ".handoff",
    ],
    optionalSelectors: [
      ".contextUsageRules",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, do not try .contract or .data wrappers; the PGC root is the contract object.",
  };
}

function architectureArtifactGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "ArchitectureArtifactContract",
    purpose: "Accepted architecture facts for modules, domain contracts, behavior, frontend experience, runtime delivery, coverage, risks, and handoff.",
    requiredSelectors: [
      ".architectureArtifactContractId",
      ".status",
      ".source",
      ".engineeringBoundary",
      ".modules",
      ".entities",
      ".interfaces",
      ".userFlows",
      ".stateMachines",
      ".acceptanceMatrix",
      ".frontendExperience",
      ".runtimeDelivery",
      ".coverage",
      ".risks",
      ".decisions",
      ".handoff",
    ],
    optionalSelectors: [
      ".metadata",
      ".acceptedSections",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, treat that architecture section as absent or not applicable; do not probe old section paths unless the current request explicitly points to them.",
  };
}

function taskPlanGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "TaskPlan",
    purpose: "Accepted task plan structure, task DAG, groups, scope snapshot, and task requirements.",
    requiredSelectors: [
      ".taskPlanId",
      ".status",
      ".source",
      ".scopeSnapshot",
      ".groups",
      ".tasks",
      ".verificationStrategy",
      ".handoff",
    ],
    optionalSelectors: [
      ".missingDesignSignals",
      ".notes",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, use the request's current outputContract or sourceContext instead of historical task-plan temp files.",
  };
}

function taskPlanRunGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "TaskPlanRun",
    purpose: "Execution state for task groups, task attempts, result ids, and next action.",
    requiredSelectors: [
      ".runId",
      ".taskPlanId",
      ".status",
      ".scheduler",
      ".groupStates",
      ".taskStates",
      ".summary",
      ".nextAction",
    ],
    optionalSelectors: [
      ".notes",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, do not infer execution state from historical run files.",
  };
}

function reviewResultGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "ReviewResult",
    purpose: "Latest accepted review decision, findings, coverage assessment, and next action.",
    requiredSelectors: [
      ".reviewId",
      ".source",
      ".decision",
      ".findings",
      ".coverageAssessment",
      ".limitations",
      ".pendingActions",
      ".nextAction",
    ],
    optionalSelectors: [
      ".notes",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, report absent review evidence instead of probing guessed roots.",
  };
}

function reviewPacketGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "ReviewPacket",
    purpose: "Review-ready summary of task plan, task run, task results, verification evidence, frontend, and runtime signals.",
    requiredSelectors: [
      ".taskPlan",
      ".taskPlanRun",
      ".taskResults",
      ".verificationSummary",
      ".frontendExperience",
      ".runtimeDelivery",
    ],
    optionalSelectors: [
      ".notes",
      ".limitations",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, cite a review limitation rather than guessing from unrelated artifacts.",
  };
}

function changeContextGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "ReviewChangeContext",
    purpose: "Review changed files and diff refs; use per-file diffs before full diff.",
    requiredSelectors: [
      ".mode",
      ".changedFiles",
      ".fullDiffRef",
      ".reviewScope",
    ],
    optionalSelectors: [
      ".warnings",
      ".baseline",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a diff ref is absent, read changed files only when needed or report a review limitation.",
  };
}

function requestArtifactGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "OriginalGenerationRequest",
    purpose: "Original request authority for repair scope, schema, output paths, allowed refs, and submit command.",
    requiredSelectors: [
      ".requestId",
      ".requestType",
      ".agentAction",
      ".generationProtocol",
      ".sourceRefs",
      ".contextRefs",
      ".allowedRefs",
      ".enumRefs",
      ".outputContract",
      ".submitCommand",
    ],
    optionalSelectors: [
      ".fieldAccessHints",
      ".blockedOutput",
      ".validatorPolicy",
      ".generationRules",
      ".repairSubmitRouting",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, use fields that exist in the original request; do not infer from another request version.",
  };
}

function requestTextGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "RequirementText",
    purpose: "Original or extracted requirement text used only when originalRequest.text is insufficient.",
    requiredSelectors: [
      "<entire text file>",
    ],
    optionalSelectors: [],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "This ref is text, not JSON. Read it as text when needed; do not run jq or guess JSON wrapper roots.",
  };
}

function requirementContextGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "RequirementContext",
    purpose: "Normalized index of user requirement sources, extracted text refs, normalized text ref, and advisory keyword hints ref.",
    requiredSelectors: [
      ".schemaVersion",
      ".deliveryId",
      ".sourceItems",
      ".normalizedTextRef",
      ".normalizedTextStatus",
      ".keywordHintsRef",
      ".keywordHintsStatus",
    ],
    optionalSelectors: [
      ".sourceItems[].path",
      ".sourceItems[].textRef",
      ".sourceItems[].extractedTextRef",
      ".sourceItems[].extractionStatus",
      ".keywordHintsReason",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If normalizedTextRef or keywordHintsRef is null, use originalRequest.text and proceed without that optional aid; do not guess alternate requirement paths.",
  };
}

function brainstormDecisionGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "BrainstormPhaseDecision",
    purpose: "Immutable confirmed requirement decision snapshot for one phase. Use it as requirement-decision history; do not treat repository facts as a replacement.",
    requiredSelectors: [
      ".phaseId",
      ".acceptedAt",
      ".sources",
      ".scope",
      ".acceptance",
      ".domainModel",
      ".phasePlan.current",
      ".phasePlan.nextPhasePreview",
      ".userConfirmation",
    ],
    optionalSelectors: [
      ".conceptGrounding",
      ".clarificationProgress",
      ".frontendExperience",
      ".frontendExperienceDelta",
      ".sourceRefs",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If an optional selector is null, treat that topic as not captured for the referenced phase; do not search repository files to invent requirement decisions.",
  };
}

function brainstormDecisionIndexGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "BrainstormPhaseDecisionIndex",
    purpose: "Lightweight index of confirmed phase decision snapshots. Use it to locate a specific prior phase decision when the user refers to earlier confirmed scope or requirement changes.",
    requiredSelectors: [
      ".deliveryId",
      ".latestConfirmedPhaseId",
      ".decisions[].phaseId",
      ".decisions[].decisionRef",
      ".decisions[].title",
      ".decisions[].goal",
      ".decisions[].scopeLabels",
      ".decisions[].acceptanceStatements",
    ],
    optionalSelectors: [
      ".decisions[].nextPhasePreview",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If the needed prior phase is not in the index, ask the user which confirmed phase they mean instead of guessing by keyword.",
  };
}

function normalizedRequirementTextGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "NormalizedRequirementText",
    purpose: "Full extracted requirement text for Brainstorm understanding when originalRequest.text is only a short summary or file name.",
    requiredSelectors: [
      "<entire text file>",
    ],
    optionalSelectors: [],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "This ref is text, not JSON. Read it as text when needed; if absent, use originalRequest.text and RequirementContext source refs.",
  };
}

function keywordHintsGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "RequirementKeywordHints",
    purpose: "Advisory-only local TF-IDF keyword hints that may help formulate clarification and concept-grounding questions.",
    requiredSelectors: [
      ".usage",
      ".status",
      ".globalKeywords[].keyword",
      ".globalKeywords[].score",
      ".globalKeywords[].sampleContexts",
      ".sectionKeywords[].keywords[].keyword",
      ".sectionKeywords[].keywords[].score",
      ".sectionKeywords[].keywords[].sampleContexts",
      ".rules",
    ],
    optionalSelectors: [
      ".languageHints",
      ".extraction",
      ".globalKeywords[]",
      ".sectionKeywords[].keywords",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If keyword hints are absent or empty, ignore them. Never treat keyword hints as confirmed scope, acceptance, or concept facts.",
  };
}

function conceptGroundingGuide(
  artifactType: string,
  purpose: string,
): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType,
    purpose,
    requiredSelectors: [
      ".mode",
      ".concepts[].conceptId",
      ".concepts[].term",
      ".concepts[].explanation",
      ".concepts[].phaseRelevance",
      ".concepts[].priority",
      ".concepts[].attentionRank",
      ".concepts[].riskFactors",
      ".concepts[].mustNotMisinterpretAs",
    ],
    optionalSelectors: [
      ".reason",
      ".concepts[].scopeRefs",
      ".concepts[].acceptanceRefs",
      ".concepts[].humanReadableReason",
    ],
    usageRule: "For current-phase concept grounding, .concepts[].explanation may carry confirmed business object semantics, key field meanings, object operations, operation inputs, validation/blocking reasons, state changes, visible feedback, and misunderstanding boundaries.",
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If concepts is empty, respect mode/reason. Do not infer missing concepts from unrelated artifacts unless the current request asks you to repair ConceptGrounding.",
  };
}

function frontendExperienceTargetGuide(
  purpose: string,
): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "ConfirmedFrontendExperienceTarget",
    purpose,
    requiredSelectors: [
      ".frontendExperience",
      ".frontendExperienceDelta",
      ".inheritedFrontendExperience",
      ".source",
      ".phaseId",
    ],
    optionalSelectors: [
      ".currentPhaseId",
      ".confirmedFrontendExperienceRef",
      ".frontendExperience.required",
      ".frontendExperience.experienceLevel",
      ".frontendExperience.audiences",
      ".frontendExperience.surfaces",
      ".frontendExperience.mustNot",
      ".frontendExperienceDelta.inheritsPrevious",
      ".frontendExperienceDelta.currentPhaseImpact",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If frontendExperience is null, inspect inheritedFrontendExperience and frontendExperienceDelta together. If all are null, treat frontend as not confirmed for this phase and do not guess a UI target.",
  };
}

function deploymentFailureGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "DeploymentFailureReport",
    purpose: "Deploy failure classification, failed runtime contract field, evidence, routing boundary, and retry guard.",
    requiredSelectors: [
      ".failureId",
      ".failureKind",
      ".failureOwner",
      ".repairRoute",
      ".runtimeDeliveryRef",
      ".sourceRefs",
      ".failedContract",
      ".evidence",
      ".routing",
      ".loopGuard",
    ],
    optionalSelectors: [
      ".diagnostics",
      ".evidence.errorWindow",
      ".evidence.fullLogRef",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, do not route repair beyond the failure report boundary.",
  };
}

function deploymentSpecGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "DeploymentSpec",
    purpose: "Deploy provider, runtime command mapping, generated deployment files, ports, and health probe expectations.",
    requiredSelectors: [
      ".provider",
      ".runtimeContract",
      ".commands",
      ".ports",
      ".healthcheck",
      ".files",
    ],
    optionalSelectors: [
      ".environment",
      ".docker",
      ".compose",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, treat it as absent from the deployment spec; do not repair application code from spec guesses alone.",
  };
}

function runtimeDeliveryGuide(purpose = "Runtime delivery contract selected by AAC or deploy state, including build, start, probes, API, frontend, and environment."): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "RuntimeDeliveryContract",
    purpose,
    requiredSelectors: [
      ".status",
      ".runtimeKind",
      ".build",
      ".start",
      ".runtimeSurfaces",
      ".deliveryMechanics",
      ".httpProbes",
      ".frontend",
      ".api",
      ".environment",
    ],
    optionalSelectors: [
      ".basis",
      ".taskPlanningGuidance",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If this ref uses a JSON pointer suffix such as #/runtimeDelivery, resolve that pointer first and then use these selectors relative to the pointed object.",
  };
}

function genericArtifactGuide(): Omit<ReferencedArtifactReadGuideEntry, "refKey" | "refPath"> {
  return {
    artifactType: "ReferencedArtifact",
    purpose: "Referenced artifact required by this request.",
    requiredSelectors: [
      ".schemaVersion",
      ".status",
      ".source",
    ],
    optionalSelectors: [
      ".summary",
      ".notes",
    ],
    doNotGuessAlternateRoots: true,
    nullSelectorHandling: "If a selector is null, inspect top-level keys only for orientation and do not guess historical wrapper roots.",
  };
}
