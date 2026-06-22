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
      title: "Greenfield technical baseline fixture",
      oneLine: "Build a greenfield management tool.",
      businessGoal: "Verify greenfield technology baseline confirmation.",
      complexity: "small",
    },
    domainModel: { actors: [], capabilityGroups: [], businessFlows: [] },
    scope: {
      included: [{ id: "scope-001", label: "Greenfield app", items: ["management tool"], source: "user_confirmed" }],
      deferred: [],
      excluded: [],
      assumptions: [],
    },
    acceptance: {
      candidates: [{ id: "AC-001", statement: "Technology baseline is confirmed before planning.", capabilityRefs: [], sourceRefs: ["src-001"], priority: "must" }],
      coverageNotes: [],
    },
    deliveryStrategy: { mode: "single_phase", reason: "fixture", recommendedCurrentPhaseId: phaseId },
    deliveryContext: {
      originalRequest: {
        text: "Build a small greenfield management tool.",
        inputRefs: ["src-001"],
      },
      initialSummary: {
        title: "Greenfield technical baseline fixture",
        oneLine: "Verify greenfield baseline confirmation.",
        businessGoal: "Verify greenfield technology baseline confirmation.",
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
        goal: "Verify greenfield technology baseline behavior.",
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
        goal: "Verify greenfield technology baseline behavior.",
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

function makeDeliveryIndex(deliveryId, phaseId) {
  return {
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Greenfield technical baseline fixture",
    roadmapId: `rm-${deliveryId}`,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Current phase",
      status: "scope_confirmed",
      latestRefs: {
        brainstormRunId: `bs-${deliveryId}`,
        brainstormContract: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
      },
      nextAction: null,
    }],
    createdAt: NOW,
    updatedAt: NOW,
  };
}

function writeDelivery(root, deliveryId, phaseId) {
  const brainstormRunId = `bs-${deliveryId}`;
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), makeDeliveryIndex(deliveryId, phaseId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/contract.json`), makeBrainstormContract(deliveryId, phaseId, brainstormRunId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/latest.json`), {
    schemaVersion: "1.0",
    brainstormRunId,
    contractRef: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
    updatedAt: NOW,
  });
  const status = readJson(projectFile(root, ".loom/status.json"));
  status.activeDeliveryId = deliveryId;
  status.deliveries = [{
    deliveryId,
    status: "planning",
    requestSummary: "Greenfield technical baseline fixture",
    activePhaseId: phaseId,
    indexRef: `.loom/deliveries/${deliveryId}/index.json`,
    updatedAt: NOW,
  }];
  status.phase = "planning";
  status.nextAction = "plan";
  status.effectiveNextAction = null;
  status.updatedAt = NOW;
  writeJson(projectFile(root, ".loom/status.json"), status);
}

function stackTracks(overrides = {}) {
  return {
    web: { status: "selected", selection: "Next.js + TypeScript", source: "agent_recommended_user_confirmed", rationale: "Fits a small greenfield management tool.", ...overrides.web },
    app: { status: "not_needed", selection: "No App client", source: "not_applicable", rationale: "The confirmed requirement only needs a web surface.", ...overrides.app },
    backend: { status: "selected", selection: "Next.js server capabilities", source: "agent_recommended_user_confirmed", rationale: "Keeps the small greenfield stack integrated.", ...overrides.backend },
    persistence: { status: "selected", selection: "SQLite", source: "agent_recommended_user_confirmed", rationale: "Local-first persistence is enough for this fixture.", ...overrides.persistence },
    dataAccess: { status: "selected", selection: "Prisma", source: "agent_recommended_user_confirmed", rationale: "Matches the selected TypeScript stack.", ...overrides.dataAccess },
    externalServices: { status: "not_needed", selection: "None", source: "not_applicable", rationale: "No external service is required by the fixture.", ...overrides.externalServices },
  };
}

function greenfieldCandidate(request, overrides = {}) {
  return {
    schemaVersion: "1.0",
    technicalBaselineId: overrides.technicalBaselineId ?? "tb-greenfield",
    status: overrides.status ?? "confirmed",
    source: overrides.source ?? "agent_recommended_for_greenfield",
    projectKind: "greenfield",
    scope: "roadmap",
    stack: overrides.stack ?? {
      tracks: stackTracks(),
      derivedLater: ["testing", "build", "local run", "deployment preparation"],
    },
    constraints: [],
    evidence: [{ reason: "Final technical baseline confirmed by user in the fixture." }],
    approval: overrides.approval ?? { type: "user_confirmed", confirmedAt: NOW, reason: "User confirmed the final greenfield technology baseline." },
    confidence: "medium",
    ...(overrides.requiresUserConfirmation === undefined ? {} : { requiresUserConfirmation: overrides.requiresUserConfirmation }),
    reasoningSummary: overrides.reasoningSummary ?? ["User accepted the recommended greenfield technical baseline."],
    alternatives: [{ name: "Custom stack", tradeoff: "Not selected in this fixture." }],
    createdAt: request.createdAt ?? NOW,
    updatedAt: NOW,
  };
}

function setupRoot() {
  const root = tempProject("loom-greenfield-baseline-");
  runCli(["init"], root);
  writeDelivery(root, "delivery-greenfield", "phase-1");
  return root;
}

function verifyGreenfieldRequestGuidance() {
  const root = setupRoot();
  try {
    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-greenfield", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    const fields = allReadFields(request);

    assert.equal(request.projectKind, "greenfield");
    assert.equal(request.operation, "recommend_greenfield_baseline");
    assert.ok(request.selectionGuidance, "greenfield request must include selectionGuidance");
    assert.ok(fields.includes("selectionGuidance"), "selectionGuidance must be part of the inspect read plan");
    assert.equal(request.selectionGuidance.commonOptions.backend.label, "Backend / service");
    assert.ok(request.selectionGuidance.commonOptions.backend.examples.includes("Java + Spring Boot"));
    assert.ok(request.selectionGuidance.commonOptions.backend.examples.includes("Python + FastAPI"));
    assert.ok(request.selectionGuidance.commonOptions.backend.examples.includes("Python + Django"));
    assert.ok(request.selectionGuidance.commonOptions.dataAccess.examples.includes("MyBatis Plus"));
    assert.ok(request.selectionGuidance.shorthandNormalization.backend.some((rule) => /backend=Java/.test(rule) && /Java \+ Spring Boot/.test(rule)));
    assert.ok(request.selectionGuidance.shorthandNormalization.backend.some((rule) => /backend=Python/.test(rule) && /Python \+ FastAPI/.test(rule)));
    assert.equal(request.selectionGuidance.trackModel.customTechnologyPolicy.includes("not a whitelist"), true);
    assert.match(request.selectionGuidance.recommendationBasis.authority, /complete BrainstormContract/);
    assert.match(request.selectionGuidance.recommendationBasis.currentPhaseLensRole, /first implementation slice only/);
    assert.ok(request.selectionGuidance.recommendationBasis.mustRead.includes("roadmap phases, deferred scope, and known next-phase previews"));
    assert.ok(request.selectionGuidance.userFacingConfirmationProtocol.mandatorySections.some((section) => /Adjustable technology range/.test(section)));
    assert.ok(request.selectionGuidance.userFacingConfirmationProtocol.mandatorySections.some((section) => /Reply format/.test(section)));
    assert.ok(request.selectionGuidance.userFacingConfirmationProtocol.wordingRules.some((rule) => /Do not use db or orm as the primary reply keys/.test(rule)));
    assert.match(request.selectionGuidance.replyProtocolForUser.partialAdjustmentExample, /persistence=PostgreSQL/);
    assert.match(request.selectionGuidance.replyProtocolForUser.partialAdjustmentExample, /dataAccess=Spring Data JPA/);
    assert.doesNotMatch(request.selectionGuidance.replyProtocolForUser.partialAdjustmentExample, /\bdb=/);
    assert.doesNotMatch(request.selectionGuidance.replyProtocolForUser.partialAdjustmentExample, /\borm=/);
    assert.ok(request.generationProtocol.technicalBaselineSourceRules.some((rule) => /complete BrainstormContract product scope/.test(rule)));
    assert.ok(request.generationProtocol.technicalBaselineSourceRules.some((rule) => /user-facing sections/.test(rule)));
    assert.equal(request.decisionNeeds.includes("test strategy"), false);
    assert.equal(request.decisionNeeds.includes("local dev and deploy strategy"), false);
    assert.match(String(request.outputContract.schemaShape.status), /confirmed only after explicit user technical-baseline confirmation/);
    assert.equal(request.outputContract.schemaShape.approval.type, "user_confirmed");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyUnconfirmedGreenfieldCannotContinue() {
  const root = setupRoot();
  try {
    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-greenfield", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    writeJson(projectFile(root, request.outputContract.candidateFile), greenfieldCandidate(request, {
      technicalBaselineId: "tb-greenfield-unconfirmed",
      status: "auto_accepted",
      approval: { type: "policy_auto_accept", reason: "Old behavior should no longer be accepted." },
    }));

    const rejectedEnvelope = runEnvelope([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-greenfield",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    const rejected = rejectedEnvelope.data;
    assert.equal(rejected.accepted, false);
    assert.ok(rejected.issues.some((issue) => issue.code === "GREENFIELD_BASELINE_CONFIRMATION_REQUIRED"));
    assert.equal(rejected.nextAction.type, "needs_user_decision");
    assert.equal(rejected.instruction.mode, "ask_user");
    assert.equal(rejected.instruction.autoContinue, false);
    assert.equal(rejected.repairInstruction, undefined);
    assert.equal(rejectedEnvelope.actionRequired, undefined);
    assert.match(rejectedEnvelope.summary, /requires explicit user confirmation/);

    const status = readJson(projectFile(root, ".loom/status.json"));
    assert.equal(status.effectiveNextAction.type, "needs_user_decision");
    assert.equal(status.deliveries.find((delivery) => delivery.deliveryId === "delivery-greenfield").status, "waiting_user");

    const continueEnvelope = runEnvelope(["continue"], root);
    assert.equal(continueEnvelope.data.instruction.mode, "ask_user");
    assert.equal(continueEnvelope.actionRequired, undefined);
    assert.equal(continueEnvelope.data.nextAction.type, "needs_user_decision");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyIncompleteGreenfieldTracksRejected() {
  const root = setupRoot();
  try {
    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-greenfield", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    writeJson(projectFile(root, request.outputContract.candidateFile), greenfieldCandidate(request, {
      technicalBaselineId: "tb-greenfield-incomplete",
      stack: { tracks: { web: stackTracks().web } },
    }));

    const rejected = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-greenfield",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(rejected.accepted, false);
    assert.ok(rejected.issues.some((issue) => issue.code === "GREENFIELD_BASELINE_TRACKS_INCOMPLETE"));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyConfirmedGreenfieldContinuesToPgc() {
  const root = setupRoot();
  try {
    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-greenfield", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    writeJson(projectFile(root, request.outputContract.candidateFile), greenfieldCandidate(request));

    const accepted = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-greenfield",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(accepted.accepted, true);
    assert.equal(accepted.status, "confirmed");
    assert.equal(accepted.nextAction.type, "planning_contract_create");
    assert.equal(accepted.instruction.autoContinue, true);

    const status = readJson(projectFile(root, ".loom/status.json"));
    assert.equal(status.effectiveNextAction.type, "planning_contract_create");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyCustomGreenfieldTrackAcceptedWhenConfirmed() {
  const root = setupRoot();
  try {
    const data = runCli(["technical-baseline", "request", "--delivery-id", "delivery-greenfield", "--phase-id", "phase-1"], root);
    const request = requestFromCommand(data, root);
    writeJson(projectFile(root, request.outputContract.candidateFile), greenfieldCandidate(request, {
      technicalBaselineId: "tb-greenfield-custom",
      stack: {
        tracks: stackTracks({
          backend: { status: "user_custom", selection: "Java (Spring Boot)", source: "user_specified", rationale: "User replied backend=Java+spring boot and confirmed the final stack." },
          persistence: { status: "selected", selection: "PostgreSQL", source: "user_adjusted", rationale: "User adjusted the database track." },
          dataAccess: { status: "selected", selection: "Spring Data JPA", source: "user_adjusted", rationale: "Matches Java Spring Boot and PostgreSQL." },
        }),
        derivedLater: ["testing", "build", "local run", "deployment preparation"],
      },
      source: "user_specified",
      reasoningSummary: ["User specified backend=Java+spring boot and confirmed the final greenfield technology baseline."],
    }));

    const accepted = runCli([
      "technical-baseline", "accept",
      "--delivery-id", "delivery-greenfield",
      "--phase-id", "phase-1",
      "--candidate-file", request.outputContract.candidateFile,
    ], root);
    assert.equal(accepted.accepted, true);
    assert.equal(accepted.nextAction.type, "planning_contract_create");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function main() {
  buildDist();
  verifyGreenfieldRequestGuidance();
  verifyUnconfirmedGreenfieldCannotContinue();
  verifyIncompleteGreenfieldTracksRejected();
  verifyConfirmedGreenfieldContinuesToPgc();
  verifyCustomGreenfieldTrackAcceptedWhenConfirmed();
  console.log("greenfield technical-baseline confirmation verification passed");
}

main();
