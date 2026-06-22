#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const NOW = "2026-05-24T00:00:00.000Z";

const { runCli, runEnvelope } = require("../harness/cli");
const { buildDist } = require("../harness/root");
const { projectFile, readJson, requestFromCommand, tempProject, writeJson } = require("../harness/files");

function allReadFields(request) {
  return (request.agentAction?.read?.fieldGroups ?? []).flatMap((group) => group.fields ?? []);
}

function makeBrainstormContract(deliveryId, phaseId, brainstormRunId) {
  return {
    schemaVersion: "1.0",
    contractId: `bc-${deliveryId}`,
    brainstormRunId,
    status: "confirmed",
    sources: [{ sourceId: "src-001", type: "text", title: "fixture" }],
    summary: {
      title: "Technical baseline fixture",
      oneLine: "Verify existing project baseline reuse.",
      businessGoal: "Verify technical baseline source handling.",
      complexity: "small",
    },
    domainModel: { actors: [], capabilityGroups: [], businessFlows: [] },
    scope: {
      included: [{ id: "scope-001", label: "Current phase", items: ["baseline behavior"], source: "user_confirmed" }],
      deferred: [],
      excluded: [],
      assumptions: [],
    },
    acceptance: {
      candidates: [{ id: "AC-001", statement: "Technical baseline request uses repository evidence.", capabilityRefs: [], sourceRefs: ["src-001"], priority: "must" }],
      coverageNotes: [],
    },
    deliveryStrategy: { mode: "single_phase", reason: "fixture", recommendedCurrentPhaseId: phaseId },
    deliveryContext: {
      originalRequest: {
        text: "Verify existing project baseline behavior.",
        inputRefs: ["src-001"],
      },
      initialSummary: {
        title: "Technical baseline fixture",
        oneLine: "Verify baseline behavior.",
        businessGoal: "Verify baseline source handling.",
        complexity: "small",
      },
    },
    clarification: { status: "confirmed", turns: [], questions: [], answers: [], patches: [], confirmations: [], pendingQuestionIds: [], pendingConfirmationIds: [] },
    roadmap: {
      roadmapId: `rm-${deliveryId}`,
      status: "active",
      strategy: "multi_phase",
      reason: "fixture",
      currentPhaseId: phaseId,
      recommendedPhaseId: phaseId,
      phases: [{
        phaseId,
        name: "Current phase",
        status: "scope_confirmed",
        goal: "Verify technical baseline behavior.",
        scope: { includedRefs: ["scope-001"], deferredRefs: [], excludedRefs: [] },
        acceptanceRefs: ["AC-001"],
        dependsOn: [],
        handoff: { readyForPlanning: true, planningContractId: null, planId: null },
        confirmation: { confirmedBy: "user", confirmedAt: NOW, sourcePatchIds: [] },
        nextActions: [],
      }],
      nextActions: [],
    },
    phasePlan: {
      current: {
        phaseId,
        title: "Current phase",
        goal: "Verify technical baseline behavior.",
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview: { kind: "none", reason: "Fixture has no further phase." },
    },
    handoff: { ready: true, nextNode: "planning_generation_contract", blockingReasons: [], confirmedAt: NOW },
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function makeDeliveryIndex(deliveryId, phaseId, status = "planning") {
  return {
    schemaVersion: "1.0",
    deliveryId,
    status,
    requestSummary: "Technical baseline fixture",
    roadmapId: `rm-${deliveryId}`,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Current phase",
      status: status === "completed" ? "completed" : "scope_confirmed",
      latestRefs: {
        brainstormRunId: `bs-${deliveryId}`,
        brainstormContract: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
      },
      nextAction: status === "completed" ? { type: "done", reason: "fixture" } : null,
    }],
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function writeDelivery(root, deliveryId, phaseId, status = "planning") {
  const brainstormRunId = `bs-${deliveryId}`;
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), makeDeliveryIndex(deliveryId, phaseId, status));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/contract.json`), makeBrainstormContract(deliveryId, phaseId, brainstormRunId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/latest.json`), {
    schemaVersion: "1.0",
    brainstormRunId,
    contractRef: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
    updatedAt: NOW,
  });
}

function writeStatus(root, activeDeliveryId, activePhaseId, deliveries, lastCompletedDeliveryId = null) {
  const status = readJson(projectFile(root, ".loom/status.json"));
  status.activeDeliveryId = activeDeliveryId;
  status.lastCompletedDeliveryId = lastCompletedDeliveryId;
  status.deliveries = deliveries.map((delivery) => ({
    deliveryId: delivery.deliveryId,
    status: delivery.status,
    requestSummary: "Technical baseline fixture",
    activePhaseId: delivery.activePhaseId,
    indexRef: `.loom/deliveries/${delivery.deliveryId}/index.json`,
    updatedAt: delivery.updatedAt ?? NOW,
  }));
  status.phase = activeDeliveryId ? "planning" : "completed";
  status.nextAction = "plan";
  status.effectiveNextAction = null;
  status.updatedAt = NOW;
  writeJson(projectFile(root, ".loom/status.json"), status);
}

function writeNodeProject(root) {
  writeJson(projectFile(root, "package.json"), {
    scripts: { build: "tsc", test: "vitest" },
    dependencies: { react: "^19.0.0" },
    devDependencies: { typescript: "^5.0.0", vitest: "^2.0.0" },
  });
  writeJson(projectFile(root, "package-lock.json"), { lockfileVersion: 3 });
  fs.mkdirSync(projectFile(root, "src"), { recursive: true });
  fs.writeFileSync(projectFile(root, "src/index.ts"), "export const ok = true;\n");
}

function writeJavaProject(root) {
  fs.writeFileSync(projectFile(root, "pom.xml"), "<project><modelVersion>4.0.0</modelVersion></project>\n");
  fs.mkdirSync(projectFile(root, "src/main/java/app"), { recursive: true });
  fs.writeFileSync(projectFile(root, "src/main/java/app/App.java"), "package app; public class App {}\n");
}

function previousNodeBaseline() {
  return {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-previous-node",
    status: "auto_accepted",
    source: "detected_from_repo",
    projectKind: "existing_project",
    scope: "project",
    stack: {
      runtime: "node",
      language: "typescript",
      framework: "react",
      packageManager: "npm",
      build: "npm run build",
      test: "npm test",
    },
    constraints: [],
    evidence: [{ path: "package.json", reason: "previous delivery fixture" }],
    approval: { type: "policy_auto_accept", reason: "previous delivery fixture" },
    confidence: "high",
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function candidateFromRequest(request, overrides = {}) {
  return {
    schemaVersion: "1.0",
    technicalBaselineId: overrides.technicalBaselineId ?? "tb-candidate",
    status: overrides.status ?? "auto_accepted",
    source: overrides.source ?? "agent_inferred_from_repo_signals",
    projectKind: "existing_project",
    scope: "project",
    stack: overrides.stack ?? {
      runtime: "node",
      language: "typescript",
      framework: "react",
      packageManager: "npm",
      build: "npm run build",
      test: "npm test",
    },
    constraints: [],
    evidence: overrides.evidence ?? [{ path: "package.json", reason: "candidate fixture" }],
    approval: overrides.approval ?? { type: "policy_auto_accept", reason: "candidate fixture" },
    confidence: overrides.confidence ?? "high",
    ...(overrides.requiresUserConfirmation === undefined ? {} : { requiresUserConfirmation: overrides.requiresUserConfirmation }),
    reasoningSummary: overrides.reasoningSummary ?? ["Generated from fixture request."],
    alternatives: [],
    createdAt: request.createdAt ?? NOW,
    updatedAt: NOW,
  };
}

function setupRoot() {
  const root = tempProject("loom-technical-baseline-");
  runCli(["init"], root);
  return root;
}

function verifyFirstExistingProjectUsesRepoSignals() {
  const root = setupRoot();
  try {
    writeNodeProject(root);
    writeDelivery(root, "delivery-first", "phase-1");
    writeStatus(root, "delivery-first", "phase-1", [
      { deliveryId: "delivery-first", status: "planning", activePhaseId: "phase-1" },
    ]);

    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-first", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    const fields = allReadFields(request);
    assert.equal(request.projectKind, "existing_project");
    assert.equal(request.operation, "infer_existing_project_baseline");
    assert.ok(request.contextRefs.repoSignalSetRef, "existing project request must include repoSignalSetRef");
    assert.equal(request.contextRefs.previousTechnicalBaselineRef, undefined, "first use must not invent previous baseline");
    assert.ok(fields.includes("contextRefs.repoSignalSetRef"), "repoSignalSetRef must be inspect-readable");
    assert.equal(fields.includes("contextRefs.previousTechnicalBaselineRef"), false, "absent previous baseline must not be read");
    assert.ok(request.referencedArtifactReadGuide.some((entry) => entry.refKey === "repoSignalSetRef"), "read guide must describe repoSignalSetRef");

    const signals = readJson(projectFile(root, request.contextRefs.repoSignalSetRef));
    assert.deepEqual(signals.projectKind, "existing_project");
    assert.ok(signals.signals.manifests.includes("package.json"));
    assert.ok(signals.signals.packageManagers.includes("npm"));
    assert.ok(signals.signals.languages.includes("TypeScript"));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyNewDeliveryReadsHistoricalBaseline() {
  const root = setupRoot();
  try {
    writeNodeProject(root);
    writeDelivery(root, "delivery-previous", "phase-1", "completed");
    writeDelivery(root, "delivery-current", "phase-1");
    writeJson(projectFile(root, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json"), previousNodeBaseline());
    writeStatus(root, "delivery-current", "phase-1", [
      { deliveryId: "delivery-previous", status: "completed", activePhaseId: "phase-1" },
      { deliveryId: "delivery-current", status: "planning", activePhaseId: "phase-1", updatedAt: "2026-05-25T00:00:00.000Z" },
    ], "delivery-previous");

    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-current", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    const fields = allReadFields(request);
    assert.equal(request.contextRefs.previousTechnicalBaselineRef, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json");
    assert.ok(fields.includes("contextRefs.previousTechnicalBaselineRef"), "historical baseline must be required inspect context");
    assert.ok(fields.includes("contextRefs.repoSignalSetRef"), "current repo signals must still be required");
    assert.equal(request.reusePolicy.previousTechnicalBaseline, "reuse_stable_stack_only");
    assert.equal(request.reusePolicy.repoSignalSetAuthority, "current_repo_signals");
    assert.ok(request.reusePolicy.baselineConflictRule.includes("do not silently continue"));
    assert.ok(request.generationProtocol.technicalBaselineSourceRules.some((rule) => rule.includes("Reuse previous TechnicalBaseline stable stack")));
    assert.ok(
      request.generationProtocol.technicalBaselineSourceRules.some((rule) => rule.includes("Do not rewrite TechnicalBaseline only because RepositoryContext or RepoSignalSet contains more precise implementation facts")),
      "previous baseline request must not reopen confirmation only for implementation facts",
    );
    assert.deepEqual(request.decisionNeeds, [
      "whether the current confirmed scope explicitly adds a new technology surface",
      "whether the current confirmed scope explicitly replaces a previous technology baseline element",
      "otherwise reuse the previous TechnicalBaseline unchanged for normal bugfix, repair, optimization, or feature work inside the existing stack",
    ]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyUnchangedPreviousBaselineCanContinue() {
  const root = setupRoot();
  try {
    writeNodeProject(root);
    writeDelivery(root, "delivery-previous", "phase-1", "completed");
    writeDelivery(root, "delivery-current", "phase-1");
    writeJson(projectFile(root, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json"), previousNodeBaseline());
    writeStatus(root, "delivery-current", "phase-1", [
      { deliveryId: "delivery-previous", status: "completed", activePhaseId: "phase-1" },
      { deliveryId: "delivery-current", status: "planning", activePhaseId: "phase-1", updatedAt: "2026-05-25T00:00:00.000Z" },
    ], "delivery-previous");

    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-current", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    writeJson(projectFile(root, request.outputContract.candidateFile), candidateFromRequest(request));

    const accepted = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-current",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(accepted.accepted, true);
    assert.equal(accepted.nextAction.type, "repository_context_request");
    assert.equal(accepted.instruction.autoContinue, true);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyUserConfirmedStackChangeCannotBypassGate() {
  const root = setupRoot();
  try {
    writeNodeProject(root);
    writeDelivery(root, "delivery-previous", "phase-1", "completed");
    writeDelivery(root, "delivery-current", "phase-1");
    writeJson(projectFile(root, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json"), previousNodeBaseline());
    writeStatus(root, "delivery-current", "phase-1", [
      { deliveryId: "delivery-previous", status: "completed", activePhaseId: "phase-1" },
      { deliveryId: "delivery-current", status: "planning", activePhaseId: "phase-1", updatedAt: "2026-05-25T00:00:00.000Z" },
    ], "delivery-previous");

    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-current", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    const additiveCandidate = candidateFromRequest(request, {
      technicalBaselineId: "tb-user-confirmed-additive-change",
      source: "user_confirmed",
      approval: { type: "user_confirmed", confirmedAt: NOW, reason: "Requirement scope was confirmed by user." },
      stack: {
        runtime: "node",
        language: "typescript",
        framework: "react",
        frontendFramework: "next",
        packageManager: "npm",
        build: "npm run build",
        test: "npm test",
      },
      reasoningSummary: ["Adds a new frontend framework while preserving the previous Node/npm baseline."],
    });
    writeJson(projectFile(root, request.outputContract.candidateFile), additiveCandidate);

    const rejected = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-current",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(rejected.accepted, false);
    assert.ok(rejected.issues.some((issue) => issue.code === "BASELINE_CONFLICT_REQUIRES_USER_CONFIRMATION"));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyDerivedCommandChangesDoNotRequireUserConfirmation() {
  const root = setupRoot();
  try {
    writeNodeProject(root);
    writeDelivery(root, "delivery-previous", "phase-1", "completed");
    writeDelivery(root, "delivery-current", "phase-2");
    writeJson(projectFile(root, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json"), previousNodeBaseline());
    writeStatus(root, "delivery-current", "phase-2", [
      { deliveryId: "delivery-previous", status: "completed", activePhaseId: "phase-1" },
      { deliveryId: "delivery-current", status: "planning", activePhaseId: "phase-2", updatedAt: "2026-05-25T00:00:00.000Z" },
    ], "delivery-previous");

    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-current", "--phase-id", "phase-2"], root);
    const request = requestFromCommand(data, root);
    const commandOnlyCandidate = candidateFromRequest(request, {
      technicalBaselineId: "tb-derived-command-update",
      stack: {
        runtime: "node",
        language: "typescript",
        framework: "react",
        packageManager: "npm",
        build: "npm run build --workspace app",
        start: "npm run start --workspace app",
        test: "npm run test --workspace account-service",
      },
      reasoningSummary: ["Current repo has more precise build/start/test commands, but the technology baseline is unchanged."],
    });
    writeJson(projectFile(root, request.outputContract.candidateFile), commandOnlyCandidate);

    const accepted = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-current",
      "--phase-id", "phase-2",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(accepted.accepted, true);
    assert.notEqual(accepted.nextAction.type, "needs_user_decision");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyTrackAdditiveChangeCannotBypassGate() {
  const root = setupRoot();
  try {
    writeNodeProject(root);
    writeDelivery(root, "delivery-previous", "phase-1", "completed");
    writeDelivery(root, "delivery-current", "phase-1");
    writeJson(projectFile(root, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json"), previousNodeBaseline());
    writeStatus(root, "delivery-current", "phase-1", [
      { deliveryId: "delivery-previous", status: "completed", activePhaseId: "phase-1" },
      { deliveryId: "delivery-current", status: "planning", activePhaseId: "phase-1", updatedAt: "2026-05-25T00:00:00.000Z" },
    ], "delivery-previous");

    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-current", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    const additiveCandidate = candidateFromRequest(request, {
      technicalBaselineId: "tb-track-additive-change",
      stack: {
        runtime: "node",
        language: "typescript",
        framework: "react",
        packageManager: "npm",
        build: "npm run build",
        test: "npm test",
        tracks: {
          web: { status: "selected", selection: "Next.js", source: "agent_recommended_user_confirmed", rationale: "Adds a new Web surface." },
          app: { status: "not_needed", selection: "No App client", source: "not_applicable", rationale: "No app surface." },
          backend: { status: "selected", selection: "Next.js server capabilities", source: "agent_recommended_user_confirmed", rationale: "Adds server-side capabilities." },
          persistence: { status: "selected", selection: "SQLite", source: "agent_recommended_user_confirmed", rationale: "Adds persistence." },
          dataAccess: { status: "selected", selection: "Prisma", source: "agent_recommended_user_confirmed", rationale: "Adds data access." },
          externalServices: { status: "not_needed", selection: "None", source: "not_applicable", rationale: "No external service." },
        },
      },
      reasoningSummary: ["Adds stack.tracks while preserving the old flat Node/npm fields."],
    });
    writeJson(projectFile(root, request.outputContract.candidateFile), additiveCandidate);

    const rejected = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-current",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(rejected.accepted, false);
    assert.ok(rejected.issues.some((issue) => issue.code === "BASELINE_CONFLICT_REQUIRES_USER_CONFIRMATION"));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifySignalConflictRequiresUserConfirmation() {
  const root = setupRoot();
  try {
    writeJavaProject(root);
    writeDelivery(root, "delivery-previous", "phase-1", "completed");
    writeDelivery(root, "delivery-current", "phase-1");
    writeJson(projectFile(root, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json"), previousNodeBaseline());
    writeStatus(root, "delivery-current", "phase-1", [
      { deliveryId: "delivery-previous", status: "completed", activePhaseId: "phase-1" },
      { deliveryId: "delivery-current", status: "planning", activePhaseId: "phase-1", updatedAt: "2026-05-25T00:00:00.000Z" },
    ], "delivery-previous");

    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-current", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    assert.equal(request.contextRefs.previousTechnicalBaselineRef, ".loom/deliveries/delivery-previous/contracts/technical-baseline.json");

    const silentSwitchCandidate = candidateFromRequest(request, {
      technicalBaselineId: "tb-silent-switch",
      stack: {
        runtime: "jvm",
        language: "java",
        framework: "spring",
        packageManager: "maven",
        build: "mvn package",
        test: "mvn test",
      },
      evidence: [{ path: "pom.xml", reason: "current repo fixture" }],
    });
    writeJson(projectFile(root, request.outputContract.candidateFile), silentSwitchCandidate);

    const rejectedEnvelope = runEnvelope([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-current",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    const rejected = rejectedEnvelope.data;
    assert.equal(rejected.accepted, false);
    assert.ok(rejected.issues.some((issue) => issue.code === "BASELINE_CONFLICT_REQUIRES_USER_CONFIRMATION"));
    assert.equal(rejected.nextAction.type, "needs_user_decision");
    assert.equal(rejected.instruction.mode, "ask_user");
    assert.equal(rejected.repairInstruction, undefined);
    assert.equal(rejectedEnvelope.actionRequired, undefined);

    const surfacedCandidate = candidateFromRequest(request, {
      technicalBaselineId: "tb-conflict-surfaced",
      status: "needs_user_confirmation",
      stack: silentSwitchCandidate.stack,
      evidence: [{ path: "pom.xml", reason: "current repo conflicts with previous Node baseline and needs user confirmation" }],
      approval: { type: "none", reason: "Repo signals conflict with previous baseline." },
      requiresUserConfirmation: true,
      confidence: "medium",
      reasoningSummary: ["Previous baseline is Node/npm; current repo signals are Maven/Java."],
    });
    writeJson(projectFile(root, request.outputContract.candidateFile), surfacedCandidate);

    const accepted = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-current",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(accepted.accepted, true);
    assert.equal(accepted.status, "needs_user_confirmation");
    assert.equal(accepted.nextAction.type, "needs_user_decision");
    assert.equal(accepted.instruction.mode, "ask_user");
    assert.equal(accepted.instruction.autoContinue, false);

    const status = readJson(projectFile(root, ".loom/status.json"));
    assert.equal(status.effectiveNextAction.type, "needs_user_decision");
    assert.equal(status.effectiveNextAction.reason, "TECHNICAL_BASELINE_REQUIRES_USER_CONFIRMATION");
    assert.equal(status.deliveries.find((delivery) => delivery.deliveryId === "delivery-current").status, "waiting_user");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function main() {
  buildDist();
  verifyFirstExistingProjectUsesRepoSignals();
  verifyNewDeliveryReadsHistoricalBaseline();
  verifyUnchangedPreviousBaselineCanContinue();
  verifyUserConfirmedStackChangeCannotBypassGate();
  verifyDerivedCommandChangesDoNotRequireUserConfirmation();
  verifyTrackAdditiveChangeCannotBypassGate();
  verifySignalConflictRequiresUserConfirmation();
  console.log("technical-baseline-reuse-signals verification passed");
}

main();
