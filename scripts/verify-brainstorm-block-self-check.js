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
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, output);
  return envelope.data;
}

function runEnvelope(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" },
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, output);
  return envelope;
}

function readJson(projectRoot, relativePath) {
  return JSON.parse(fs.readFileSync(path.join(projectRoot, relativePath), "utf8"));
}

function hydrateRequest(projectRoot, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readJson(projectRoot, value);
  }
  return hydrated;
}

function includes(text, needle, message) {
  assert.ok(String(text).includes(needle), message);
}

function writeJson(projectRoot, relativePath, value) {
  const target = path.join(projectRoot, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, `${JSON.stringify(value, null, 2)}\n`);
}

function phaseScopeOnlyCandidate(request) {
  return {
    schemaVersion: "1.0",
    candidateId: "brainstorm-candidate-phase-scope-only",
    brainstormRunId: request.brainstormRunId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "confirmed",
    requestSummary: {
      title: "Operations console",
      oneLine: "Confirm only the current phase scope.",
      businessGoal: "Only the phase_scope block has been confirmed so far.",
      complexity: "medium",
    },
    sources: [{ sourceId: "req-001", type: "user_text", title: "test request", extracted: true }],
    scope: {
      included: [{
        id: "scope-001",
        label: "Operations console scope",
        items: ["Create applications and review them."],
        source: "user_confirmed",
      }],
      excluded: [],
      deferred: [],
      assumptions: [],
    },
    roadmap: {
      required: false,
      currentPhaseId: request.phaseId,
      phases: [{
        phaseId: request.phaseId,
        title: "Operations console scope",
        status: "scope_confirmed",
        goal: "Confirm the scope only.",
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        dependsOn: [],
      }],
    },
    phasePlan: {
      current: {
        phaseId: request.phaseId,
        title: "Operations console scope",
        goal: "Confirm the scope only.",
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview: { kind: "none", reason: "No next phase has been confirmed in this fixture." },
    },
    domainModel: {
      actors: [{ id: "actor-staff", name: "Staff", description: "Uses the operations console." }],
      capabilityGroups: [{ id: "cap-applications", name: "Applications", description: "Application handling." }],
      businessFlows: [],
    },
    acceptance: [{
      id: "AC-001",
      statement: "Scope has been confirmed, but later Brainstorm blocks have not been confirmed yet.",
      capabilityRefs: ["cap-applications"],
      sourceRefs: ["req-001"],
      priority: "must",
    }],
    userConfirmation: {
      confirmed: true,
      confirmedAt: "2026-06-16T00:00:00.000Z",
      confirmationSummary: "The user confirmed the phase_scope option only.",
      confirmationBasis: {
        initialRequestOnly: false,
        summaryPresentedToUser: true,
        confirmedAfterSummary: true,
        presentedItems: [
          "currentPhaseScopeSummary",
          "includedDeferredExcludedBoundary",
          "nextPhasePreview",
        ],
      },
    },
    conceptGrounding: {
      phaseConceptGrounding: {
        mode: "concepts_present",
        concepts: [],
      },
      glossaryUpdates: [],
    },
    conceptConfirmation: {
      shownToUser: false,
      confirmedConceptRefs: [],
      confirmationSummary: "Concept grounding has not been confirmed.",
    },
    clarificationProgress: {
      mode: "progressive_blocks",
      confirmedBlocks: [
        { block: "phase_scope", summary: "The user confirmed phase_scope only.", confirmedByUser: true },
      ],
      skippedBlocks: [],
      finalSummaryConfirmed: false,
    },
    handoff: {
      ready: false,
      nextNode: "brainstorm_clarification",
      blockingReasons: ["Later Brainstorm blocks are not confirmed."],
    },
  };
}

const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-brainstorm-block-self-check-"));
run(["init"], projectRoot);

const started = run([
  "brainstorm",
  "start",
  "--request",
  "Build an operations console for staff to create applications, review them, block invalid requests with reasons, update status, and query records before operating on them.",
], projectRoot);

const request = hydrateRequest(projectRoot, readJson(projectRoot, started.requestPath ?? started.requestRef));
assert.equal(request.requestType, "brainstorm_session");

const blockRules = request.clarificationConversationProtocol.blockExecutionRules.join("\n");
includes(blockRules, "phase_scope self-check", "phase_scope must have a block-level self-check before confirmation.");
includes(blockRules, "present 2-3 source-grounded phase scope options", "phase_scope must present option comparison by default.");
includes(blockRules, "consistent compact structure", "phase_scope options must use a consistent compact structure.");
includes(blockRules, "nextPhaseSeed", "phase_scope must treat nextPhaseSeed as non-binding.");
includes(blockRules, "must preserve the current phase's source-grounded core outcome", "phase_scope recommendation must not shrink the current phase core outcome.");
includes(blockRules, "do not recommend it when it defers explicit nextPhaseSeed.scopePreview items", "phase_scope must not recommend narrower cuts that defer explicit seed items.");
includes(blockRules, "scope reduction", "phase_scope must mark seed-item reductions as scope reductions.");
includes(blockRules, "atomic single-scope exception", "phase_scope must restrict single-scope confirmation to atomic scope.");
includes(blockRules, "single preselected phase_scope", "phase_scope self-check must reject preselected single-scope output when the phase is not atomic.");
includes(blockRules, "concept_grounding self-check", "concept_grounding must have a block-level self-check before confirmation.");
includes(blockRules, "frontend_experience self-check", "frontend_experience must have a block-level self-check before confirmation.");
includes(blockRules, "business scenario confirmation", "concept_grounding must include business scenario confirmation.");
includes(blockRules, "decision impact ordering", "concept_grounding must include decision impact ordering.");
includes(blockRules, "lifecycle scan", "concept_grounding must include lifecycle scan.");
includes(blockRules, "final_summary block is a review gate", "final_summary must be a review gate, not first-detail discovery.");
includes(blockRules, "Do not use the Chinese word 反讲", "user-facing scenario confirmation must avoid obscure wording.");

const confirmationRules = request.clarificationConversationProtocol.blockConfirmationRules;
includes(confirmationRules.phase_scope, "phase_scope self-check", "phase_scope confirmation must depend on self-check.");
includes(confirmationRules.phase_scope, "recommended option", "phase_scope confirmation must include recommended option confirmation.");
includes(confirmationRules.concept_grounding, "business scenario confirmation", "concept confirmation must mention scenario confirmation.");
includes(confirmationRules.concept_grounding, "decision impact ordering", "concept confirmation must mention decision impact ordering.");
includes(confirmationRules.concept_grounding, "lifecycle scan", "concept confirmation must mention lifecycle scan.");
includes(confirmationRules.frontend_experience, "frontend_experience self-check", "frontend confirmation must depend on self-check.");
includes(confirmationRules.final_summary, "already-confirmed business scenario", "final summary must review already-confirmed details.");

const semantic = request.rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract;
assert.ok(
  semantic.requiredUserVisibleTopicsWhenApplicable.includes("current business scenario confirmation"),
  "semantic contract must list business scenario confirmation as a required visible topic.",
);
assert.ok(
  semantic.requiredUserVisibleTopicsWhenApplicable.includes("decision impact ordering"),
  "semantic contract must list decision impact ordering as a required visible topic.",
);
assert.ok(
  semantic.requiredUserVisibleTopicsWhenApplicable.includes("business object lifecycle scan"),
  "semantic contract must list lifecycle scan as a required visible topic.",
);
assert.ok(semantic.blockSelfCheckContract.phase_scope.rules.some((rule) => rule.includes("phase_scope self-check")));
assert.ok(semantic.blockSelfCheckContract.phase_scope.rules.some((rule) => rule.includes("2-3 source-grounded phase scope options")));
assert.ok(semantic.blockSelfCheckContract.phase_scope.rules.some((rule) => rule.includes("single preselected phase_scope")));
assert.ok(semantic.blockSelfCheckContract.concept_grounding.rules.some((rule) => rule.includes("business scenario confirmation")));
assert.ok(semantic.blockSelfCheckContract.concept_grounding.rules.some((rule) => rule.includes("decision impact")));
assert.ok(semantic.blockSelfCheckContract.concept_grounding.rules.some((rule) => rule.includes("Lifecycle actions")));
assert.ok(semantic.blockSelfCheckContract.frontend_experience.rules.some((rule) => rule.includes("frontend_experience self-check")));
assert.ok(semantic.blockSelfCheckContract.final_summary.rules.some((rule) => rule.includes("review gate")));

const candidateShape = request.outputContract.schemaShape;
includes(
  candidateShape.scope.included[0].items.join("\n"),
  "Business scenario, decision impact, or lifecycle action detail",
  "scope.included[].items shape must tell agents where scenario/decision/lifecycle details can land.",
);
includes(
  candidateShape.domainModel.businessFlows[0].summary,
  "current business scenario confirmation",
  "businessFlows summary shape must carry scenario confirmation.",
);
includes(
  candidateShape.domainModel.businessFlows[0].summary,
  "decision impacts ordered by downstream effect",
  "businessFlows summary shape must carry decision impact ordering.",
);
includes(
  candidateShape.domainModel.businessFlows[0].summary,
  "relevant lifecycle actions",
  "businessFlows summary shape must carry lifecycle scan.",
);
includes(
  candidateShape.conceptGrounding.phaseConceptGrounding.concepts[0].explanation,
  "decision impact",
  "concept explanation shape must carry decision impact.",
);
includes(
  candidateShape.conceptGrounding.phaseConceptGrounding.concepts[0].explanation,
  "lifecycle semantics",
  "concept explanation shape must carry lifecycle semantics.",
);

writeJson(projectRoot, request.outputContract.candidateFile, phaseScopeOnlyCandidate(request));
const earlyAccept = runEnvelope([
  "brainstorm",
  "accept",
  "--delivery-id",
  request.deliveryId,
  "--phase-id",
  request.phaseId,
  "--request-id",
  request.requestId,
  "--run-id",
  request.brainstormRunId,
  "--candidate-file",
  request.outputContract.candidateFile,
], projectRoot);
assert.equal(earlyAccept.data.accepted, false, "phase_scope-only candidate must not be accepted.");
assert.equal(earlyAccept.instruction?.mode, "ask_user", "phase_scope-only accept must route back to Brainstorm clarification.");
assert.equal(earlyAccept.instruction?.expectedResponse?.kind, "brainstorm_progressive_clarification");
assert.equal(earlyAccept.actionRequired, undefined, "phase_scope-only accept must not become auto-runnable repair_candidate.");
assert.equal(earlyAccept.instruction?.submitCommand, undefined, "phase_scope-only clarification must not expose submitCommand.");
assert.equal(earlyAccept.instruction?.candidateFile, undefined, "phase_scope-only clarification must not expose candidateFile.");

console.log("Brainstorm block self-check protocol verification passed.");
