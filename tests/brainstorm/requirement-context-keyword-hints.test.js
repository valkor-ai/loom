#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
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

const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-requirements-"));
run(["init"], projectRoot);

const requestFile = path.join(projectRoot, "stock-trading-requirements.txt");
fs.writeFileSync(
  requestFile,
  [
    "Build a stock trading system for buy orders, sell orders, account funds, holdings, risk controls, and order cancellation.",
    "The system must distinguish frozen funds, available funds, market orders, limit orders, trading calendar, and settlement records.",
    "Frontend users need an internal operations workflow for order review and account status visibility.",
  ].join("\n"),
);

const data = run([
  "brainstorm",
  "start",
  "--requirement-file",
  requestFile,
  "--request",
  "Start the stock trading delivery with staged clarification.",
], projectRoot);

const request = hydrateRequest(projectRoot, readJson(projectRoot, data.requestPath ?? data.requestRef));
assert.equal(request.requestType, "brainstorm_session");
assert.equal(request.contextRefs.originalRequirementContextRef, request.contextRefs.requirementContextRef, "BrainstormRequest must expose originalRequirementContextRef alias");
assert.ok(request.contextRefs.requirementContextRef, "BrainstormRequest must expose requirementContextRef");
assert.ok(request.contextRefs.normalizedRequirementTextRef, "BrainstormRequest must expose normalizedRequirementTextRef");
assert.ok(request.contextRefs.keywordHintsRef, "BrainstormRequest must expose keywordHintsRef when hints are generated");
assert.equal(
  request.sourceFieldAccessHints.requirementContextInput.sourceItemsSelector,
  ".sourceItems[]",
  "BrainstormRequest must tell Agent how to read RequirementContext sourceItems",
);
assert.equal(
  request.sourceFieldAccessHints.requirementContextInput.itemIdField,
  "itemId",
  "BrainstormRequest must not imply RequirementContext uses sourceId",
);
assert.equal(
  request.sourceFieldAccessHints.candidateOutput.sourceIdField,
  "sourceId",
  "BrainstormRequest must distinguish candidate sources[].sourceId from input sourceItems[].itemId",
);
assert.ok(
  request.sourceFieldAccessHints.mappingRules.some((rule) => rule.includes("sourceItems[]") && rule.includes("itemId/kind")),
  "BrainstormRequest must explain RequirementContext-to-candidate source field mapping",
);
assert.equal(request.keywordHintsPolicy.status, "advisory_only");
assert.equal(request.keywordHintsPolicy.mustNotTreatAsScope, true);
assert.equal(request.keywordHintsPolicy.mustNotTreatAsAcceptance, true);
assert.equal(request.keywordHintsPolicy.mustNotTreatAsConfirmedConcept, true);
const brainstormFieldGroups = request.agentAction.read.fieldGroups;
const phaseScopeCoreGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_phase_scope_core");
const phaseScopeAuthorityGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_phase_scope_authority");
const conceptGroundingGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_concept_grounding_context");
const keywordHintsAdvisoryGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_keyword_hints_advisory");
const candidateWriteControlsGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_write_controls");
const candidateSchemaHandoffGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_schema_handoff");
const candidateFinalSummaryRulesGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_final_summary_rules");
const candidateRequirementSemanticRulesGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_requirement_semantic_rules");
const candidateSelfReviewRulesGroup = brainstormFieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_self_review_rules");
assert.ok(phaseScopeCoreGroup, "BrainstormRequest must include the phase_scope core read group");
assert.ok(phaseScopeAuthorityGroup, "BrainstormRequest must include the phase_scope authority read group");
assert.ok(conceptGroundingGroup, "BrainstormRequest must include the concept grounding read group");
assert.equal(
  phaseScopeAuthorityGroup.fields.includes("keywordHints"),
  false,
  "BrainstormRequest phase_scope authority must not read complete advisory keywordHints",
);
assert.ok(keywordHintsAdvisoryGroup, "BrainstormRequest must expose keywordHints through a separate advisory read group");
assert.deepEqual(keywordHintsAdvisoryGroup.fields, ["keywordHints.compact"], "keywordHints advisory group must read the compact keywordHints projection");
assert.equal(keywordHintsAdvisoryGroup.required, false, "keywordHints advisory group must be optional");
assert.ok(
  keywordHintsAdvisoryGroup.whenToRead.includes("Do not read this group by default for phase_scope"),
  "keywordHints advisory group must prevent default phase_scope reads",
);
assert.ok(candidateWriteControlsGroup, "BrainstormRequest must include delayed candidate write controls group");
assert.ok(candidateSchemaHandoffGroup, "BrainstormRequest must include delayed candidate schema handoff group");
assert.ok(candidateFinalSummaryRulesGroup, "BrainstormRequest must include delayed candidate final summary rules group");
assert.ok(candidateRequirementSemanticRulesGroup, "BrainstormRequest must include delayed candidate requirement semantic rules group");
assert.ok(candidateSelfReviewRulesGroup, "BrainstormRequest must include delayed candidate self-review rules group");
assert.deepEqual(candidateWriteControlsGroup.fields, ["agentAction.write", "agentAction.submit", "agentAction.schema", "outputContract.candidateFile", "outputContract.schemaRef", "generationProtocol", "enumRefs"]);
assert.deepEqual(candidateSchemaHandoffGroup.fields, ["outputContract.schemaShape.handoff"]);
assert.deepEqual(candidateRequirementSemanticRulesGroup.fields, ["rules.requirementSemanticGrounding.compactRules"]);
assert.deepEqual(candidateSelfReviewRulesGroup.fields, ["rules.candidateSelfReview", "rules.nextPhasePreviewGeneration"]);
assert.deepEqual(conceptGroundingGroup.fields, [
  "conceptGroundingRequest",
  "clarificationConversationProtocol.blockExecutionRules",
  "clarificationConversationProtocol.blockConfirmationRules.concept_grounding",
  "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.scopeItemCoverageContract",
  "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.objectOperationContract",
]);
assert.equal(phaseScopeCoreGroup.fields.includes("agentAction"), false, "phase_scope core must not read agentAction");
assert.equal(phaseScopeCoreGroup.fields.includes("clarificationConversationProtocol"), false, "phase_scope core must not read clarificationConversationProtocol");
assert.equal(phaseScopeCoreGroup.fields.includes("requestManifest"), false, "phase_scope core must not read requestManifest");
for (const candidateGroup of [
  candidateWriteControlsGroup,
  candidateSchemaHandoffGroup,
  candidateFinalSummaryRulesGroup,
  candidateRequirementSemanticRulesGroup,
  candidateSelfReviewRulesGroup,
]) {
  assert.ok(
    candidateGroup.whenToRead.includes("final_summary"),
    `${candidateGroup.groupId} must be readable only after final_summary confirmation`,
  );
}
assert.ok(
  candidateWriteControlsGroup.whenToRead.includes("Do not read this group after phase_scope"),
  "BrainstormRequest candidate write controls must not be read after early Brainstorm block confirmations",
);
assert.equal(phaseScopeCoreGroup.fields.includes("outputContract"), false, "phase_scope core must not read outputContract");
assert.equal(phaseScopeCoreGroup.fields.includes("rules"), false, "phase_scope core must not read rules");
assert.equal(phaseScopeCoreGroup.fields.includes("generationProtocol"), false, "phase_scope core must not read generationProtocol");
assert.equal(phaseScopeCoreGroup.fields.includes("enumRefs"), false, "phase_scope core must not read enumRefs");
const brainstormReadFields = brainstormFieldGroups.flatMap((group) => group.fields);
assert.equal(brainstormReadFields.includes("outputContract"), false, "BrainstormRequest must not read full outputContract in fieldGroups");
assert.equal(brainstormReadFields.includes("outputContract.schemaShape"), false, "BrainstormRequest must not read full outputContract.schemaShape in fieldGroups");
assert.equal(brainstormReadFields.includes("outputContract.schemaShape.candidateRules"), false, "BrainstormRequest must not read full candidateRules in fieldGroups");
assert.equal(brainstormReadFields.includes("agentAction"), false, "BrainstormRequest must not read full agentAction in fieldGroups");
assert.equal(brainstormReadFields.includes("clarificationConversationProtocol"), false, "BrainstormRequest must not read full clarificationConversationProtocol in fieldGroups");
assert.equal(brainstormReadFields.includes("requestManifest"), false, "BrainstormRequest must not read full requestManifest in fieldGroups");
assert.equal(brainstormReadFields.includes("rules"), false, "BrainstormRequest must not read full rules in fieldGroups");
assert.deepEqual(
  phaseScopeAuthorityGroup.fields,
  ["requirementContext.sourceItems", "requirementContext.normalizedText"],
  "initial phase_scope authority must read original requirement authority fields only",
);

const guideKeys = new Set(request.referencedArtifactReadGuide.map((entry) => entry.refKey));
assert.ok(guideKeys.has("originalRequirementContextRef"), "read guide must cover originalRequirementContextRef");
assert.ok(guideKeys.has("requirementContextRef"), "read guide must cover requirementContextRef");
assert.ok(guideKeys.has("normalizedRequirementTextRef"), "read guide must cover normalizedRequirementTextRef");
assert.ok(guideKeys.has("keywordHintsRef"), "read guide must cover keywordHintsRef");
const keywordHintsGuide = request.referencedArtifactReadGuide.find((entry) => entry.refKey === "keywordHintsRef");
assert.ok(keywordHintsGuide.requiredSelectors.includes(".compact"), "keyword hints read guide must expose compact projection");
assert.ok(keywordHintsGuide.requiredSelectors.includes(".compact.topKeywords[].keyword"), "keyword hints compact guide must expose keyword field");
assert.ok(keywordHintsGuide.requiredSelectors.includes(".compact.sectionKeywords[].keywords[]"), "keyword hints compact guide must expose section keyword field");
assert.equal(keywordHintsGuide.requiredSelectors.includes(".globalKeywords[].term"), false, "keyword hints read guide must not use stale term field");

const context = readJson(projectRoot, request.contextRefs.requirementContextRef);
assert.equal(context.schemaVersion, "1.0");
assert.equal(context.normalizedTextStatus, "completed");
assert.equal(context.keywordHintsStatus, "completed");
assert.equal(context.sourceItems.length, 2);

const hints = readJson(projectRoot, request.contextRefs.keywordHintsRef);
assert.equal(hints.usage, "advisory_only");
assert.ok(hints.globalKeywords.length >= 3, "keyword hints should include multiple advisory business terms");
assert.ok(typeof hints.globalKeywords[0].keyword === "string", "keyword hints must use keyword field for keyword text");
assert.equal("term" in hints.globalKeywords[0], false, "keyword hints must not expose stale term field");
assert.ok(hints.compact.topKeywords.length > 0, "keyword hints must expose compact topKeywords");
assert.ok(typeof hints.compact.topKeywords[0].keyword === "string", "compact keyword hints must expose keyword text");
assert.equal(hints.rules.mustNotTreatAsScope, true);

const keywordText = hints.globalKeywords.map((item) => item.keyword).join("\n");
assert.match(keywordText, /order|fund|trading|account|订单|资金|交易/, "keyword hints should contain requirement-related terms");

fs.rmSync(projectRoot, { recursive: true, force: true });
console.log("Requirement context and advisory keyword hints verification passed.");
