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
includes(blockRules, "concept_grounding self-check", "concept_grounding must have a block-level self-check before confirmation.");
includes(blockRules, "frontend_experience self-check", "frontend_experience must have a block-level self-check before confirmation.");
includes(blockRules, "business scenario confirmation", "concept_grounding must include business scenario confirmation.");
includes(blockRules, "decision impact ordering", "concept_grounding must include decision impact ordering.");
includes(blockRules, "lifecycle scan", "concept_grounding must include lifecycle scan.");
includes(blockRules, "final_summary block is a review gate", "final_summary must be a review gate, not first-detail discovery.");
includes(blockRules, "Do not use the Chinese word 反讲", "user-facing scenario confirmation must avoid obscure wording.");

const confirmationRules = request.clarificationConversationProtocol.blockConfirmationRules;
includes(confirmationRules.phase_scope, "phase_scope self-check", "phase_scope confirmation must depend on self-check.");
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

console.log("Brainstorm block self-check protocol verification passed.");
