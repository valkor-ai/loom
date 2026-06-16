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

function readProjectJson(projectRoot, relativePath) {
  return JSON.parse(fs.readFileSync(path.join(projectRoot, relativePath), "utf8"));
}

function hydrateRequest(projectRoot, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readProjectJson(projectRoot, value);
  }
  return hydrated;
}

function readRepo(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function assertIncludes(text, needle, message) {
  assert.ok(text.includes(needle), message);
}

const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-brainstorm-frontend-path-"));
run(["init"], projectRoot);

const started = run([
  "brainstorm",
  "start",
  "--request",
  "Build an internal operations console where staff query existing applications, select one record, approve or reject it, and see the updated result.",
], projectRoot);

const request = hydrateRequest(projectRoot, readProjectJson(projectRoot, started.requestPath ?? started.requestRef));
assert.equal(request.requestType, "brainstorm_session");

assert.deepEqual(
  request.enumRefs.frontendTargetSelectionMode,
  ["query_and_select", "direct_id_lookup", "preselected_context", "not_applicable"],
  "BrainstormRequest must expose internal target selection enum values for candidate writing",
);
assert.ok(
  request.enumRefs.frontendInteractionState.includes("business_blocking"),
  "BrainstormRequest must let candidates represent business-blocking UI feedback",
);

const blockRules = request.clarificationConversationProtocol.blockExecutionRules.join("\n");
assertIncludes(
  blockRules,
  "The frontend_experience block must clarify page operation paths before final_summary",
  "frontend_experience must own operation-path clarification before final summary",
);
assertIncludes(
  blockRules,
  "do not use a hardcoded industry field list",
  "query criteria guidance must be source-grounded, not test-scenario hardcoded",
);
assertIncludes(
  blockRules,
  "Do not show internal enum values like query_and_select",
  "user-facing clarification must hide internal enum names",
);
assertIncludes(
  request.clarificationConversationProtocol.blockConfirmationRules.concept_grounding,
  "inputs or fields",
  "concept_grounding confirmation must include applicable inputs or fields",
);
assertIncludes(
  request.clarificationConversationProtocol.blockConfirmationRules.concept_grounding,
  "actions or behaviors",
  "concept_grounding confirmation must include applicable actions or behaviors",
);
assertIncludes(
  request.clarificationConversationProtocol.blockConfirmationRules.frontend_experience,
  "how users find or receive target objects",
  "frontend_experience confirmation must include target discovery/selection path",
);
assert.ok(
  request.firstClarificationGate.mustPresentBeforeAccept.includes("businessObjectOperationSummary"),
  "first clarification gate must require business object/operation summary before accept",
);
assertIncludes(
  blockRules,
  "map every confirmed scope.included item",
  "concept_grounding block must own generic scope item coverage",
);

const semanticContract = request.rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract;
assert.equal(
  semanticContract.confirmedBlockDetailRetentionContract.sourceOfTruth,
  "all_confirmed_brainstorm_blocks_plus_final_summary_corrections",
  "candidate writing must use every confirmed Brainstorm block, not final_summary alone",
);
assert.ok(
  semanticContract.confirmedBlockDetailRetentionContract.candidateFields.includes("frontendExperience.dataViews/actions/operationPaths"),
  "confirmed frontend details must map to structured frontendExperience fields",
);
assert.ok(
  semanticContract.confirmedBlockDetailRetentionContract.rules.some((rule) => rule.includes("not from final_summary alone")),
  "confirmed block retention rules must forbid final_summary-only candidate writing",
);
assert.ok(
  semanticContract.requiredUserVisibleTopicsWhenApplicable.includes("frontend_experience block confirmed or skipped reason"),
  "final_summary contract must only summarize frontend_experience status",
);
assert.ok(
  semanticContract.frontendOperationPathContract.candidateFields.includes("frontendExperience.dataViews"),
  "operation-path contract must map to frontendExperience.dataViews",
);
assert.ok(
  semanticContract.frontendOperationPathContract.candidateFields.includes("frontendExperience.operationPaths"),
  "operation-path contract must map to frontendExperience.operationPaths",
);
assert.ok(
  semanticContract.frontendOperationPathContract.rules.some((rule) => rule.includes("input fields")),
  "frontend operation-path contract must preserve applicable input fields",
);
assert.ok(
  semanticContract.objectOperationContract.candidateFields.includes("domainModel.businessFlows[].summary"),
  "object-operation contract must map details to existing businessFlows summaries",
);

const frontendShape = request.outputContract.schemaShape.frontendExperience;
assert.ok(Array.isArray(frontendShape.dataViews), "schemaShape.frontendExperience must include dataViews");
assert.ok(Array.isArray(frontendShape.actions), "schemaShape.frontendExperience must include actions");
assert.ok(Array.isArray(frontendShape.operationPaths), "schemaShape.frontendExperience must include operationPaths");
assertIncludes(
  request.outputContract.schemaShape.candidateRules.join("\n"),
  "Write page operation path details into frontendExperience.dataViews/actions/operationPaths",
  "candidateRules must require operation-path details in structured frontend fields",
);
assertIncludes(
  request.outputContract.schemaShape.candidateRules.join("\n"),
  "A concise final_summary does not make earlier phase_scope, concept_grounding, or frontend_experience details optional",
  "candidateRules must keep earlier confirmed block details even when final_summary is concise",
);
assertIncludes(
  request.outputContract.schemaShape.candidateRules.join("\n"),
  "when the user confirmed query criteria in frontend_experience, preserve them in dataViews[].searchCriteria",
  "candidateRules must require confirmed query criteria to land in structured searchCriteria fields",
);
assertIncludes(
  request.outputContract.schemaShape.candidateRules.join("\n"),
  "Store confirmed object-operation details in existing BrainstormCandidate fields",
  "candidateRules must require object-operation details in existing BrainstormCandidate fields",
);

const repositoryContextSource = readRepo("src/core/operations/repository-context.ts");
assertIncludes(
  repositoryContextSource,
  "frontendOperationPathClarificationRules",
  "phase-continuation Brainstorm requests must reuse frontend operation path clarification rules",
);
assertIncludes(
  repositoryContextSource,
  "frontendExperienceDelta.*Deltas",
  "phase-continuation Brainstorm requests must map frontend deltas to structured fields",
);

const architectureSource = readRepo("src/core/operations/contracts.ts");
assertIncludes(
  architectureSource,
  "operationPaths",
  "AAC frontend_experience section shape must be able to carry operation paths",
);
assertIncludes(
  architectureSource,
  "target discovery/selection",
  "AAC generation rules must preserve target discovery and selection expectations",
);

console.log("Brainstorm frontend operation-path protocol verification passed.");
