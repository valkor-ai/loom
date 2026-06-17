#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

function run(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return envelope.data;
}

function runEnvelope(args, projectRoot, extraEnv = {}) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", ...extraEnv },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return envelope;
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function requestFromCommand(data, root) {
  return data.request ?? hydrateRequest(root, readJson(projectFile(root, data.requestPath ?? data.requestRef)));
}

function hydrateRequest(root, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readJson(projectFile(root, value));
  }
  return hydrated;
}

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function writeTechnicalBaseline(root, deliveryId) {
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-preview",
    status: "auto_accepted",
    source: "agent_recommended_for_greenfield",
    projectKind: "greenfield",
    scope: "roadmap",
    stack: {
      languages: ["typescript"],
      frameworks: [],
      packageManagers: ["npm"],
      runtime: "node",
      testFrameworks: [],
      buildTools: [],
    },
    constraints: [],
    evidence: [],
    approval: { type: "policy_auto_accept", reason: "test fixture" },
    confidence: "medium",
    createdAt: now(),
    updatedAt: now(),
  });
}

function createRepositoryContextCandidate(requestData, root) {
  const request = requestFromCommand(requestData, root);
  return {
    schemaVersion: "1.0",
    repositoryContextId: "repoctx-preview",
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    source: {
      requestRef: requestData.requestRef,
      brainstormContractRef: request.source.brainstormContractRef,
      technicalBaselineRef: request.source.technicalBaselineRef,
    },
    requestLens: {
      projectKind: request.projectKind,
      scanPurpose: "phase_start_repository_snapshot",
      primaryConsumer: "phase_brainstorm",
      laterConsumers: ["PGC", "AAC", "TaskPlan"],
    },
    repoOverview: {
      summary: "Current repository after previous phase.",
      repositoryShape: "single_package",
      primaryApplications: [{ applicationId: "app-main", name: "Main", kind: "library", rootPath: "." }],
    },
    technologySignals: { primaryLanguages: ["typescript"], frameworks: [], packageManagers: ["npm"], buildCommands: [], testCommands: [], notes: [] },
    structureSignals: { rootPaths: [{ path: "src", role: "source_root" }], entryPoints: [], configurationFiles: [] },
    existingCapabilities: [{ capabilityId: "cap-phase-1", name: "Phase 1 capability", status: "implemented", summary: "Phase 1 exists in code.", surfaceRefs: ["surface-src"], confidence: "medium" }],
    relevantSurfaces: [{ surfaceId: "surface-src", kind: "module", path: "src/index.ts", summary: "Existing source.", relevance: "implemented_capability", suggestedUse: "inspect_or_extend" }],
    recommendedReadRefs: [{ path: "src/index.ts", reason: "implemented_capability", priority: "high", surfaceRefs: ["surface-src"] }],
    roadmapImplications: [],
    contextQuality: { coverage: "focused", confidence: "medium", warnings: [] },
    warnings: [],
    createdAt: now(),
    updatedAt: now(),
  };
}

function createDeferredScopeNonePreviewCandidate(request) {
  const conceptId = "concept-phase-2-current";
  return {
    schemaVersion: "1.0",
    candidateId: "brainstorm-candidate-deferred-none",
    brainstormRunId: request.brainstormRunId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "confirmed",
    requestSummary: {
      title: "Phase 2 current scope",
      oneLine: "Confirm the current phase and defer one later capability.",
      businessGoal: "Verify deferred scope cannot be silently dropped at phase handoff.",
      complexity: "medium",
    },
    sources: [{ sourceId: "src-001", type: "user_text", title: "preview fixture", extracted: true }],
    scope: {
      included: [{ id: "scope-current", label: "Current phase scope", items: ["Current phase capability"], source: "user_confirmed" }],
      deferred: [{ id: "scope-deferred-next", label: "Deferred next capability", items: ["Next capability"], source: "user_confirmed" }],
      excluded: [],
      assumptions: [{ id: "assumption-001", text: "The deferred capability remains in the same delivery.", requiresConfirmation: false }],
    },
    roadmap: {
      required: false,
      currentPhaseId: request.phaseId,
      phases: [
        {
          phaseId: "phase-1",
          title: "Phase 1",
          status: "delivered",
          goal: "Completed phase.",
          scopeRefs: [],
          acceptanceRefs: [],
          dependsOn: [],
        },
        {
          phaseId: request.phaseId,
          title: "Phase 2",
          status: "scope_confirmed",
          goal: "Deliver the current phase scope.",
          scopeRefs: ["scope-current"],
          acceptanceRefs: ["AC-current"],
          dependsOn: ["phase-1"],
        },
      ],
    },
    phasePlan: {
      current: {
        phaseId: request.phaseId,
        title: "Phase 2",
        goal: "Deliver the current phase scope.",
        scopeRefs: ["scope-current"],
        acceptanceRefs: ["AC-current"],
        status: "scope_confirmed",
      },
      nextPhasePreview: {
        kind: "none",
        reason: "No next phase remains.",
      },
    },
    domainModel: {
      actors: [{ id: "actor-user", name: "User", description: "Uses the current phase capability." }],
      capabilityGroups: [{ id: "cap-current", name: "Current Capability", description: "Current phase capability group." }],
      businessFlows: [{ id: "flow-current", name: "Current Flow", actors: ["actor-user"], capabilityRefs: ["cap-current"], summary: "User completes the current phase flow." }],
    },
    acceptance: [{ id: "AC-current", statement: "The current phase capability is delivered.", capabilityRefs: ["cap-current"], sourceRefs: ["src-001"], priority: "must" }],
    userConfirmation: {
      confirmed: true,
      confirmedAt: now(),
      confirmationSummary: "User confirmed current scope while leaving one capability deferred.",
      confirmationBasis: {
        initialRequestOnly: false,
        summaryPresentedToUser: true,
        confirmedAfterSummary: true,
        presentedItems: [
          "currentPhaseScopeSummary",
          "includedDeferredExcludedBoundary",
          "nextPhasePreview",
          "conceptSummary",
          "businessObjectOperationSummary",
        ],
      },
    },
    conceptGrounding: {
      phaseConceptGrounding: {
        mode: "concepts_present",
        concepts: [{
          conceptId,
          term: "Current phase boundary",
          normalizedName: "current_phase_boundary",
          explanation: "The current phase includes one capability and explicitly defers another.",
          mustNotMisinterpretAs: ["All remaining delivery work is complete"],
          phaseRelevance: "current",
          priority: "must_understand",
          attentionRank: 1,
          riskFactors: ["scope_confusion_risk"],
          scopeRefs: ["scope-current", "scope-deferred-next"],
          acceptanceRefs: ["AC-current"],
          humanReadableReason: "Misreading this boundary can make Loom end while deferred work remains.",
        }],
      },
      glossaryUpdates: [],
    },
    conceptConfirmation: {
      shownToUser: true,
      confirmedConceptRefs: [conceptId],
      confirmationSummary: "User confirmed the current phase boundary concept.",
    },
    clarificationProgress: {
      mode: "progressive_blocks",
      confirmedBlocks: [
        { block: "phase_scope", summary: "Confirmed included and deferred scope.", confirmedByUser: true },
        { block: "concept_grounding", summary: "Confirmed scope boundary concept.", confirmedByUser: true },
        { block: "final_summary", summary: "Confirmed final Brainstorm summary.", confirmedByUser: true },
      ],
      skippedBlocks: [{ block: "frontend_experience", reason: "This routing fixture has no frontend target." }],
      finalSummaryConfirmed: true,
    },
    handoff: { ready: true, nextNode: "technical_baseline_generation", blockingReasons: [] },
  };
}

function createDeferredScopeCandidatePreviewCandidate(request) {
  const candidate = createDeferredScopeNonePreviewCandidate(request);
  return {
    ...candidate,
    candidateId: "brainstorm-candidate-deferred-candidate-preview",
    phasePlan: {
      ...candidate.phasePlan,
      nextPhasePreview: {
        kind: "candidate",
        suggestedPhaseId: "phase-3",
        title: "Deferred next capability",
        goal: "Confirm and deliver the deferred next capability.",
        scopePreview: ["Next capability"],
        reason: "The current confirmed phase explicitly defers this capability.",
      },
    },
  };
}

function writeBaseState(root, nextPhasePreview) {
  const deliveryId = nextPhasePreview.kind === "candidate" ? "delivery-preview-candidate" : "delivery-preview-none";
  const phaseId = "phase-1";
  run(["init"], root);
  const status = readJson(projectFile(root, ".loom/status.json"));
  status.activeDeliveryId = deliveryId;
  status.deliveries = [{
    deliveryId,
    status: "planning",
    requestSummary: "Preview routing fixture",
    activePhaseId: phaseId,
    indexRef: `.loom/deliveries/${deliveryId}/index.json`,
    updatedAt: now(),
  }];
  status.phase = "planning";
  status.nextAction = "plan";
  status.effectiveNextAction = {
    type: "continue_to_next_phase",
    source: "review_result",
    deliveryId,
    phaseId,
    reason: "REVIEW_APPROVED",
    targetNode: "continue_to_next_phase",
  };
  status.updatedAt = now();
  writeJson(projectFile(root, ".loom/status.json"), status);

  const requirementContextRef = `.loom/deliveries/${deliveryId}/requirements/context.json`;
  const normalizedRequirementTextRef = `.loom/deliveries/${deliveryId}/requirements/normalized.txt`;
  const keywordHintsRef = `.loom/deliveries/${deliveryId}/requirements/keyword-hints.json`;
  fs.mkdirSync(projectFile(root, `.loom/deliveries/${deliveryId}/requirements`), { recursive: true });
  fs.writeFileSync(projectFile(root, normalizedRequirementTextRef), "Verify nextPhasePreview routing.\n");
  writeJson(projectFile(root, requirementContextRef), {
    schemaVersion: "1.0",
    deliveryId,
    createdAt: now(),
    sourceItems: [{
      itemId: "req-001",
      kind: "text",
      origin: "user_message",
      textRef: normalizedRequirementTextRef,
      extractionStatus: "completed",
      digest: "sha256:preview",
      characterCount: 33,
    }],
    normalizedTextRef: normalizedRequirementTextRef,
    normalizedTextStatus: "completed",
    keywordHintsRef,
    keywordHintsStatus: "empty",
    keywordHintsReason: "Fixture has no keyword hints.",
  });
  writeJson(projectFile(root, keywordHintsRef), {
    schemaVersion: "1.0",
    deliveryId,
    usage: "advisory_only",
    status: "empty",
    globalKeywords: [],
    sectionKeywords: [],
    rules: { mustNotTreatAsScope: true, mustNotTreatAsAcceptance: true, mustNotTreatAsConfirmedConcept: true },
    generatedAt: now(),
  });

  const brainstormDecisionRef = `.loom/deliveries/${deliveryId}/brainstorms/decisions/${phaseId}.json`;
  const brainstormDecisionsIndexRef = `.loom/deliveries/${deliveryId}/brainstorms/decisions/index.json`;
  writeJson(projectFile(root, brainstormDecisionRef), {
    schemaVersion: "1.0",
    artifactType: "brainstorm_phase_decision",
    deliveryId,
    phaseId,
    brainstormRunId: "brainstorm-preview-run",
    contractId: "brainstorm-preview",
    acceptedAt: now(),
    sources: [{ sourceId: "src-001", type: "text", title: "preview fixture" }],
    sourceRefs: {
      originalRequirementContextRef: requirementContextRef,
      requirementContextRef,
      normalizedRequirementTextRef,
      keywordHintsRef,
    },
    summary: {
      title: "Preview routing",
      oneLine: "Verify nextPhasePreview routing.",
      businessGoal: "Route from review approval using nextPhasePreview.",
      complexity: "large",
    },
    scope: {
      included: [{ id: "scope-phase-1", label: "Phase 1", items: ["done"], source: "user_confirmed" }],
      deferred: [],
      excluded: [],
      assumptions: [],
    },
    acceptance: {
      candidates: [{ id: "AC-001", statement: "Phase 1 complete.", capabilityRefs: [], sourceRefs: ["src-001"], priority: "must" }],
      coverageNotes: [],
    },
    domainModel: { actors: [], capabilityGroups: [], businessFlows: [] },
    phasePlan: {
      current: {
        phaseId,
        title: "Phase 1",
        goal: "Completed phase.",
        scopeRefs: ["scope-phase-1"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview,
    },
    userConfirmation: {
      confirmed: true,
      confirmedAt: now(),
      confirmationSummary: "Fixture confirmed phase 1.",
    },
  });
  writeJson(projectFile(root, brainstormDecisionsIndexRef), {
    schemaVersion: "1.0",
    artifactType: "brainstorm_phase_decisions_index",
    deliveryId,
    latestConfirmedPhaseId: phaseId,
    updatedAt: now(),
    decisions: [{
      phaseId,
      decisionRef: brainstormDecisionRef,
      brainstormRunId: "brainstorm-preview-run",
      contractId: "brainstorm-preview",
      acceptedAt: now(),
      title: "Phase 1",
      goal: "Completed phase.",
      scopeLabels: ["Phase 1"],
      acceptanceStatements: ["Phase 1 complete."],
      nextPhasePreview,
    }],
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Preview routing fixture",
    roadmapId: "roadmap-preview",
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "completed",
      latestRefs: {
        brainstormContract: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
        originalRequirementContextRef: requirementContextRef,
        requirementContextRef,
        normalizedRequirementTextRef,
        keywordHintsRef,
        brainstormDecision: brainstormDecisionRef,
        brainstormDecisionsIndex: brainstormDecisionsIndexRef,
      },
      nextAction: status.effectiveNextAction,
    }],
    createdAt: now(),
    updatedAt: now(),
  });

  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/contract.json`), {
    schemaVersion: "1.0",
    contractId: "brainstorm-preview",
    brainstormRunId: "brainstorm-preview-run",
    status: "confirmed",
    sources: [{ sourceId: "src-001", type: "text", title: "preview fixture" }],
    summary: {
      title: "Preview routing",
      oneLine: "Verify nextPhasePreview routing.",
      businessGoal: "Route from review approval using nextPhasePreview.",
      complexity: "large",
    },
    domainModel: { actors: [], capabilityGroups: [], businessFlows: [] },
    scope: {
      included: [{ id: "scope-phase-1", label: "Phase 1", items: ["done"], source: "user_confirmed" }],
      deferred: [],
      excluded: [],
      assumptions: [],
    },
    acceptance: {
      candidates: [{ id: "AC-001", statement: "Phase 1 complete.", capabilityRefs: [], sourceRefs: ["src-001"], priority: "must" }],
      coverageNotes: [],
    },
    deliveryStrategy: {
      mode: "roadmap",
      reason: "Preview routing fixture.",
      recommendedCurrentPhaseId: phaseId,
    },
    deliveryContext: {
      originalRequest: {
        text: "Verify nextPhasePreview routing.",
        inputRefs: ["src-001"],
      },
      initialSummary: {
        title: "Preview routing",
        oneLine: "Verify nextPhasePreview routing.",
        businessGoal: "Route from review approval using nextPhasePreview.",
        complexity: "large",
      },
    },
    clarification: {
      status: "confirmed",
      turns: [],
      questions: [],
      answers: [],
      patches: [],
      confirmations: [],
      pendingQuestionIds: [],
      pendingConfirmationIds: [],
    },
    roadmap: {
      roadmapId: "roadmap-preview",
      status: "active",
      strategy: "multi_phase",
      reason: "Phase index only.",
      currentPhaseId: phaseId,
      recommendedPhaseId: phaseId,
      phases: [{
        phaseId,
        name: "Phase 1",
        status: "delivered",
        goal: "Completed phase.",
        scope: { includedRefs: ["scope-phase-1"], deferredRefs: [], excludedRefs: [] },
        acceptanceRefs: ["AC-001"],
        dependsOn: [],
        handoff: { readyForPlanning: false, planningContractId: null, planId: null },
        confirmation: { confirmedBy: "user", confirmedAt: now(), sourcePatchIds: [] },
        nextActions: [],
      }],
      nextActions: [],
    },
    phasePlan: {
      current: {
        phaseId,
        title: "Phase 1",
        goal: "Completed phase.",
        scopeRefs: ["scope-phase-1"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview,
    },
    handoff: {
      ready: true,
      nextNode: "planning_generation_contract",
      blockingReasons: [],
      confirmedAt: now(),
    },
    createdAt: now(),
    updatedAt: now(),
  });
  return deliveryId;
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });

  const candidateRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-preview-candidate-"));
  try {
    fs.mkdirSync(projectFile(candidateRoot, "src"), { recursive: true });
    fs.writeFileSync(projectFile(candidateRoot, "src/index.ts"), "export const phase1 = true;\n");
    writeBaseState(candidateRoot, {
      kind: "candidate",
      suggestedPhaseId: "phase-2",
      title: "Phase 2",
      goal: "Confirm next phase.",
      scopePreview: ["next capability"],
      reason: "A next phase remains.",
    });
    const decision = run(["continue"], candidateRoot);
    assert.equal(decision.transition.type, "phase_activated");
    assert.equal(decision.phaseId, "phase-2");
    assert.equal(decision.nextAction.type, "repository_context_request");
    assert.equal(decision.instruction.mode, "run_cli");
    assert.deepEqual(decision.instruction.command.argv.slice(0, 2), ["repository-context", "request"]);

    writeTechnicalBaseline(candidateRoot, "delivery-preview-candidate");
    const repoRequest = run(["repository-context", "request", "--delivery-id", "delivery-preview-candidate", "--phase-id", "phase-2"], candidateRoot);
    const repoRequestBody = requestFromCommand(repoRequest, candidateRoot);
    assert.equal(repoRequest.request, undefined);
    assert.equal(repoRequestBody.projectKind, "greenfield");
    assert.equal(Object.hasOwn(repoRequestBody, "currentPhaseLens"), false);
    assert.equal(Object.hasOwn(repoRequestBody, "nextPhasePreview"), false);
    assert.equal(repoRequestBody.scanPurpose.activePhase.phaseId, "phase-2");
    writeJson(projectFile(candidateRoot, repoRequest.candidateFile), createRepositoryContextCandidate(repoRequest, candidateRoot));
    const accepted = run([
      "repository-context", "accept",
      "--delivery-id", "delivery-preview-candidate",
      "--phase-id", "phase-2",
      "--request-id", repoRequest.requestId,
      "--candidate-file", repoRequest.candidateFile,
    ], candidateRoot);
    assert.equal(accepted.instruction.mode, "ask_user");
    assert.equal(accepted.instruction.nextAction.type, "brainstorm_confirmation");
    const brainstormRequest = hydrateRequest(candidateRoot, readJson(projectFile(candidateRoot, accepted.instruction.expectedResponse.requestRef)));
    writeJson(projectFile(candidateRoot, brainstormRequest.outputContract.candidateFile), createDeferredScopeNonePreviewCandidate(brainstormRequest));
    const rejectedBrainstorm = run([
      "brainstorm", "accept",
      "--delivery-id", "delivery-preview-candidate",
      "--phase-id", "phase-2",
      "--run-id", brainstormRequest.brainstormRunId,
      "--request-id", brainstormRequest.requestId,
      "--candidate-file", brainstormRequest.outputContract.candidateFile,
    ], candidateRoot);
    assert.equal(rejectedBrainstorm.accepted, false);
    assert.equal(
      rejectedBrainstorm.issues.some((issue) => issue.code === "NEXT_PHASE_PREVIEW_REQUIRED_FOR_DEFERRED_SCOPE"),
      true,
      JSON.stringify(rejectedBrainstorm.issues, null, 2),
    );
    assert.equal(
      rejectedBrainstorm.repairInstruction.instructions.some((instruction) =>
        instruction.includes("scope.deferred") && instruction.includes("nextPhasePreview.kind must be candidate"),
      ),
      true,
      JSON.stringify(rejectedBrainstorm.repairInstruction, null, 2),
    );
    const compactAccepted = runEnvelope([
      "repository-context", "accept",
      "--delivery-id", "delivery-preview-candidate",
      "--phase-id", "phase-2",
      "--request-id", repoRequest.requestId,
      "--candidate-file", repoRequest.candidateFile,
    ], candidateRoot, { LOOM_COMPACT_OUTPUT: "1" });
    assert.equal(compactAccepted.instruction.mode, "ask_user");
    assert.equal(compactAccepted.actionRequired, undefined);
    assert.ok(compactAccepted.instruction.expectedResponse.requestRef);
    assert.equal(compactAccepted.instruction.expectedResponse.candidateFile, undefined);
    assert.equal(compactAccepted.instruction.expectedResponse.submitCommand, undefined);
    assert.equal(brainstormRequest.contextRefs.latestRepositoryContextRef, ".loom/deliveries/delivery-preview-candidate/workspace/phase-2/repository-context.json");
    assert.equal(brainstormRequest.phaseContinuationContext.nextPhaseSeed.kind, "candidate");
    assert.equal(Object.hasOwn(brainstormRequest.phaseContinuationContext, "completedPhases"), false);
    assert.equal(
      brainstormRequest.clarificationGuidance.phaseScopeOptionComparison.requiredByDefault,
      true,
      "Phase continuation Brainstorm must require phase scope option comparison by default.",
    );
    assert.equal(
      brainstormRequest.clarificationGuidance.phaseScopeOptionComparison.nextPhaseSeedIsPreselectedAnswer,
      false,
      "Phase continuation Brainstorm must not treat nextPhaseSeed as a preselected answer.",
    );
    assert.equal(
      brainstormRequest.clarificationGuidance.phaseScopeOptionComparison.recommendationMustPreserveCoreCurrentPhaseOutcome,
      true,
      "Phase continuation Brainstorm must require the recommended option to preserve the core current-phase outcome.",
    );
    assert.equal(
      brainstormRequest.clarificationGuidance.phaseScopeOptionComparison.narrowerOptionCannotBeRecommendedWhenDefersExplicitSeedItems,
      true,
      "Phase continuation Brainstorm must not recommend a narrower option that defers explicit seed items.",
    );
    assert.equal(
      brainstormRequest.clarificationGuidance.phaseScopeOptionComparison.scopeReductionRequiresExplicitUserConfirmation,
      true,
      "Phase continuation Brainstorm must require explicit user confirmation for seed-item scope reductions.",
    );
    assert.equal(
      brainstormRequest.clarificationGuidance.phaseScopeOptionComparison.atomicSingleScopeException.disallowedWhenMultipleScopePreviewItemsOrLifecycleActionsExist,
      true,
      "Phase continuation Brainstorm must not use single-scope confirmation for multi-item nextPhaseSeed.",
    );
    assert.ok(
      brainstormRequest.rules.phaseScopeOptionComparison.rules.some((rule) =>
        rule.includes("multiple nextPhaseSeed.scopePreview items") && rule.includes("not atomic"),
      ),
      "Phase continuation Brainstorm rules must force multi-item nextPhaseSeed into option comparison.",
    );
    assert.ok(
      brainstormRequest.rules.phaseScopeOptionComparison.rules.some((rule) =>
        rule.includes("do not recommend it when it defers explicit nextPhaseSeed.scopePreview items"),
      ),
      "Phase continuation Brainstorm rules must keep narrower alternatives from becoming the recommendation.",
    );
    const compactBrainstormRequest = hydrateRequest(candidateRoot, readJson(projectFile(candidateRoot, compactAccepted.instruction.expectedResponse.requestRef)));
    writeJson(projectFile(candidateRoot, compactBrainstormRequest.outputContract.candidateFile), createDeferredScopeCandidatePreviewCandidate(compactBrainstormRequest));
    const acceptedBrainstorm = run([
      "brainstorm", "accept",
      "--delivery-id", "delivery-preview-candidate",
      "--phase-id", "phase-2",
      "--run-id", compactBrainstormRequest.brainstormRunId,
      "--request-id", compactBrainstormRequest.requestId,
      "--candidate-file", compactBrainstormRequest.outputContract.candidateFile,
    ], candidateRoot);
    assert.equal(acceptedBrainstorm.accepted, true, JSON.stringify(acceptedBrainstorm, null, 2));
    const acceptedContract = readJson(projectFile(candidateRoot, acceptedBrainstorm.contractPath));
    assert.equal(acceptedContract.deliveryStrategy.mode, "roadmap");
    assert.equal(acceptedContract.phasePlan.nextPhasePreview.kind, "candidate");
    assert.deepEqual(
      acceptedContract.roadmap.phases.map((phase) => phase.phaseId),
      ["phase-1", "phase-2"],
      "Only prior phases and the active confirmed phase should be preserved; generic future placeholders must not be reintroduced.",
    );
  } finally {
    fs.rmSync(candidateRoot, { recursive: true, force: true });
  }

  const noneRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-preview-none-"));
  try {
    writeBaseState(noneRoot, {
      kind: "none",
      reason: "No next phase remains.",
    });
    const decision = run(["continue"], noneRoot);
    assert.equal(decision.nextAction.type, "done");
    assert.equal(decision.instruction.mode, "report_done");
    const status = readJson(projectFile(noneRoot, ".loom/status.json"));
    assert.equal(status.activeDeliveryId, null);
  } finally {
    fs.rmSync(noneRoot, { recursive: true, force: true });
  }

  console.log("next phase preview routing verification passed");
}

main();
