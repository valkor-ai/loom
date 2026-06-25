#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const { runCli } = require("../harness/cli");
const { projectFile, readJson, tempProject, writeJson } = require("../harness/files");

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function writeTechnicalBaseline(root, deliveryId) {
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-source-consistency",
    status: "auto_accepted",
    source: "detected_from_repo",
    projectKind: "existing_project",
    scope: "project",
    stack: { runtime: "node", language: "typescript", packageManager: "npm", test: "npm test" },
    constraints: [],
    evidence: [{ path: "package.json", reason: "fixture package manifest" }],
    approval: { type: "policy_auto_accept", reason: "test fixture" },
    confidence: "high",
    createdAt: now(),
    updatedAt: now(),
  });
}

function writeRepositoryContext(root, deliveryId, phaseId) {
  const ref = `.loom/deliveries/${deliveryId}/workspace/${phaseId}/repository-context.json`;
  writeJson(projectFile(root, ref), {
    schemaVersion: "1.0",
    repositoryContextId: `repoctx-${phaseId}`,
    deliveryId,
    phaseId,
    status: "ready",
    source: {
      requestRef: `.loom/deliveries/${deliveryId}/workspace/${phaseId}/repository-context-requests/repoctx-${phaseId}.json`,
      brainstormContractRef: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
      technicalBaselineRef: `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`,
    },
    requestLens: {
      projectKind: "existing_project",
      baselineProjectKind: "existing_project",
      repositoryMode: "existing_project",
      phaseDevelopmentMode: "initial_delivery",
      scanPurpose: "phase_start_repository_snapshot",
      primaryConsumer: "phase_brainstorm",
      laterConsumers: ["PGC", "AAC", "TaskPlan"],
    },
    repoOverview: {
      summary: "Existing repository fixture.",
      repositoryShape: "single_package",
      primaryApplications: [{ applicationId: "app-main", name: "Main", kind: "service", rootPath: "." }],
    },
    technologySignals: { primaryLanguages: ["typescript"], frameworks: [], packageManagers: ["npm"], buildCommands: [], testCommands: [], notes: [] },
    structureSignals: { rootPaths: [{ path: "src", role: "source_root" }], entryPoints: [], configurationFiles: ["package.json"] },
    existingCapabilities: [],
    relevantSurfaces: [],
    recommendedReadRefs: [],
    roadmapImplications: [],
    contextQuality: { coverage: "focused", confidence: "high", warnings: [] },
    warnings: [],
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/workspace/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    repositoryContextRef: ref,
    updatedAt: now(),
  });
}

function writeFixture(root) {
  const deliveryId = "delivery-source-consistency";
  const phaseId = "phase-3";
  runCli(["init"], root);
  fs.mkdirSync(projectFile(root, "src"), { recursive: true });
  fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ scripts: { test: "node -e true" } }, null, 2));

  const contract = {
    schemaVersion: "1.0",
    contractId: "bc-source-consistency",
    brainstormRunId: "bs-phase-2-stale",
    status: "confirmed",
    sources: [{ sourceId: "src-001", type: "text", title: "fixture" }],
    summary: {
      title: "Phase 3 announcements",
      oneLine: "Deliver announcements.",
      businessGoal: "Deliver announcements for the current phase.",
      complexity: "medium",
    },
    domainModel: { actors: [], capabilityGroups: [], businessFlows: [] },
    scope: {
      included: [{ id: "scope-phase-3", label: "Phase 3", items: ["announcements"], source: "user_confirmed" }],
      deferred: [{ id: "scope-phase-4", label: "Phase 4", items: ["operations"], source: "user_confirmed" }],
      excluded: [{ id: "scope-no-prod", label: "No production", items: ["real broker"], source: "user_confirmed" }],
      assumptions: [],
    },
    acceptance: {
      candidates: [{ id: "AC-301", statement: "Announcements are available.", capabilityRefs: [], sourceRefs: ["src-001"], priority: "must" }],
      coverageNotes: [],
    },
    deliveryStrategy: { mode: "roadmap", reason: "fixture", recommendedCurrentPhaseId: phaseId },
    deliveryContext: {
      originalRequest: {
        text: "Source consistency fixture",
        inputRefs: ["src-001"],
      },
      initialSummary: {
        title: "Source consistency fixture",
        oneLine: "Verify source consistency.",
        businessGoal: "Verify phase-aware source consistency.",
        complexity: "medium",
      },
    },
    clarification: { status: "confirmed", turns: [], questions: [], answers: [], patches: [], confirmations: [], pendingQuestionIds: [], pendingConfirmationIds: [] },
    roadmap: {
      roadmapId: "rm-source-consistency",
      status: "active",
      strategy: "multi_phase",
      reason: "fixture",
      currentPhaseId: phaseId,
      recommendedPhaseId: phaseId,
      phases: [{
        phaseId,
        name: "Phase 3",
        status: "scope_confirmed",
        goal: "Deliver announcements.",
        scope: { includedRefs: ["scope-phase-3"], deferredRefs: ["scope-phase-4"], excludedRefs: ["scope-no-prod"] },
        acceptanceRefs: ["AC-301"],
        dependsOn: ["phase-2"],
        handoff: { readyForPlanning: true, planningContractId: "pgc-phase-3", planId: null },
        confirmation: { confirmedBy: "user", confirmedAt: now(), sourcePatchIds: [] },
        nextActions: [],
      }],
      nextActions: [],
    },
    phasePlan: {
      current: {
        phaseId,
        title: "Phase 3",
        goal: "Deliver announcements.",
        scopeRefs: ["scope-phase-3"],
        acceptanceRefs: ["AC-301"],
        status: "scope_confirmed",
      },
      nextPhasePreview: { kind: "candidate", suggestedPhaseId: "phase-4", title: "Phase 4", goal: "Operations.", scopePreview: ["operations"], reason: "Remaining scope." },
    },
    handoff: { ready: true, nextNode: "planning_generation_contract", blockingReasons: [], confirmedAt: now() },
    createdAt: now(),
    updatedAt: now(),
  };

  const index = {
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Source consistency fixture",
    roadmapId: "rm-source-consistency",
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 3",
      status: "scope_confirmed",
      latestRefs: {
        brainstormRunId: "bs-phase-3-latest",
        brainstormContract: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
        repositoryContextRef: `.loom/deliveries/${deliveryId}/workspace/${phaseId}/repository-context.json`,
      },
      nextAction: null,
    }],
    createdAt: now(),
    updatedAt: now(),
  };

  const status = readJson(projectFile(root, ".loom/status.json"));
  status.activeDeliveryId = deliveryId;
  status.deliveries = [{ deliveryId, status: "planning", requestSummary: "Source consistency fixture", activePhaseId: phaseId, indexRef: `.loom/deliveries/${deliveryId}/index.json`, updatedAt: now() }];
  status.phase = "planning";
  status.nextAction = "plan";
  status.updatedAt = now();

  writeJson(projectFile(root, ".loom/status.json"), status);
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), index);
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/contract.json`), contract);
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/latest.json`), {
    schemaVersion: "1.0",
    brainstormRunId: "bs-phase-3-latest",
    contractRef: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
    updatedAt: now(),
  });
  writeTechnicalBaseline(root, deliveryId);
  writeRepositoryContext(root, deliveryId, phaseId);
  return { deliveryId, phaseId };
}

const root = tempProject("loom-source-consistency-");
try {
  const { deliveryId, phaseId } = writeFixture(root);

  const baselineRequestData = runCli(["technical-baseline", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
  const baselineRequest = readJson(projectFile(root, baselineRequestData.requestPath));
  assert.equal(baselineRequest.deliveryId, deliveryId);
  assert.equal(baselineRequest.phaseId, phaseId);
  assert.equal(baselineRequest.inputs.brainstormRunId, "bs-phase-3-latest");
  assert.equal(baselineRequest.contextRefs.brainstormContractRef, `.loom/deliveries/${deliveryId}/brainstorms/contract.json`);
  assert.equal(baselineRequest.contextRefs.latestRepositoryContextRef, `.loom/deliveries/${deliveryId}/workspace/${phaseId}/repository-context.json`);
  assert.deepEqual(baselineRequest.currentPhaseLens.includedScopeRefs, ["scope-phase-3"]);
  assert.deepEqual(baselineRequest.currentPhaseLens.deferredScopeRefs, ["scope-phase-4"]);
  assert.equal(baselineRequest.reusePolicy.previousTechnicalBaseline, "reuse_stable_stack_only");
  assert.equal(baselineRequest.reusePolicy.currentPhaseScopeAuthority, "brainstorm_contract");

  const pgcData = runCli(["planning-contract", "create", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
  assert.equal(pgcData.status, "ready");
  assert.equal(pgcData.contract.source.brainstormRunId, "bs-phase-3-latest");
  assert.equal(pgcData.contract.source.phaseId, phaseId);
  assert.equal(pgcData.contract.contextRefs.normalizedBrainstormRunIdFrom, undefined);
  assert.deepEqual(pgcData.contract.phaseScope.included.map((item) => item.scopeId), ["scope-phase-3"]);
  assert.deepEqual(pgcData.contract.phaseScope.deferred.map((item) => item.scopeId), ["scope-phase-4"]);

  console.log("phase-aware-source-consistency-ok");
} finally {
  fs.rmSync(root, { recursive: true, force: true });
}
