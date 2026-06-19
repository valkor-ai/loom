#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-brainstorm-knowledge-fixture-"));
const loomHome = fs.mkdtempSync(path.join(os.tmpdir(), "loom-brainstorm-knowledge-home-"));
const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-brainstorm-knowledge-project-"));

function run(args) {
  const output = execFileSync(process.execPath, [
    cli,
    ...args,
    "--project-root",
    projectRoot,
    "--json",
  ], {
    cwd: repoRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      LOOM_AGENT_PROFILE: "codex",
      LOOM_HOME: loomHome,
    },
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, output);
  return envelope;
}

function runFailure(args) {
  const result = spawnSync(process.execPath, [
    cli,
    ...args,
    "--project-root",
    projectRoot,
    "--json",
  ], {
    cwd: repoRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      LOOM_AGENT_PROFILE: "codex",
      LOOM_HOME: loomHome,
    },
  });
  assert.notEqual(result.status, 0, `Expected command to fail: ${args.join(" ")}\n${result.stdout}\n${result.stderr}`);
  return JSON.parse(result.stdout);
}

function writeFixture(relativePath, content) {
  const target = path.join(fixtureRoot, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content, "utf8");
  return target;
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function readProjectJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(projectRoot, relativePath), "utf8"));
}

function hydrateRequest(request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readProjectJson(value);
  }
  return hydrated;
}

function includes(text, needle, message) {
  assert.ok(String(text).includes(needle), message);
}

const doc = writeFixture("knowledge/operations.md", [
  "# Operations knowledge",
  "",
  "Internal operators should clarify object lifecycle rules before implementation.",
  "",
  "## Scope boundary",
  "",
  "The current phase boundary should be checked against dependency order and closed capability coverage.",
  "",
  "## Object lifecycle",
  "",
  "A lifecycle flow should keep the object, operations, states, and blocking rules together.",
  "",
  "## Business rules",
  "",
  "Business rules should identify validation, blocking result, success outcome, and state change.",
  "",
  "## Page operation path",
  "",
  "A page operation path should identify target discovery, action entry, success feedback, blocking feedback, and refresh behavior.",
  "",
  "## Readback",
  "",
  "After an operation succeeds, the page should show the latest state rather than only a transient message.",
  "",
  "## Adjacent modules",
  "",
  "Adjacent downstream capabilities can explain dependency order without becoming current scope.",
].join("\n"));

run(["init"]);
run(["knowledge", "add", "--name", "ops-context", doc]);
const build = run(["knowledge", "build", "ops-context"]);
const request = build.data.firstRequest;
assert.equal(
  request.chunkPack.chunks.length >= 5,
  true,
  "Knowledge brainstorm context fixture must create enough chunks to verify single-source recall coverage.",
);

writeJson(request.outputContract.resultFile, {
  schemaVersion: "1.0",
  buildId: request.buildId,
  packId: request.packId,
  chunkResults: request.chunkPack.chunks.map((chunk) => ({
    chunkId: chunk.chunkId,
    status: "completed",
    summary: "Operators clarify lifecycle rules and page operation path feedback.",
    semanticLabels: [
      {
        kind: "operation",
        text: "clarify lifecycle rules",
        normalizedText: "clarify lifecycle rules",
        aliases: ["lifecycle clarification"],
        confidence: "high",
      },
      {
        kind: "flow",
        text: "page operation path",
        normalizedText: "page operation path",
        aliases: ["operation workflow"],
        confidence: "high",
      },
    ],
    blockAffinity: {
      phaseScope: 0.3,
      conceptGrounding: 0.8,
      frontendExperience: 0.9,
      finalSummary: 0,
    },
  })),
});

run([
  "knowledge",
  "semantic",
  "submit",
  "--request",
  build.data.firstRequestPath,
  "--result-file",
  request.outputContract.resultFile,
]);

const queryFile = path.join(fixtureRoot, "match-query.json");
writeJson(queryFile, {
  naturalLanguageQuery: "operator lifecycle rules and page operation feedback",
  brainstormBlock: "frontend_experience",
  semanticFocus: [
    { kind: "flow", text: "page operation path" },
    { kind: "operation", text: "clarify lifecycle rules" },
  ],
  sourceLimit: 2,
  chunkLimitPerSource: 5,
});

const contextEnvelope = run(["knowledge", "brainstorm-context", "--query-file", queryFile]);
const context = contextEnvelope.data.context;
assert.equal(context.status, "available");
assert.equal(context.block, "frontend_experience");
assert.match(context.matchQuery.naturalLanguageQuery, /target discovery/);
assert.match(context.matchQuery.naturalLanguageQuery, /页面办理路径/);
assert.equal(context.matchedSources.length <= 2, true);
assert.equal(context.readPlan.mode, "inspect_all_listed_chunks");
assert.equal(context.readPlan.chunks.length > 0, true);
assert.equal(context.readPlan.chunks.length, 5, "Brainstorm context should be able to return the full five-chunk block budget from a single source.");
assert.equal(context.matchedSources[0].topChunks.length <= 5, true);
assert.equal("text" in context.matchedSources[0].topChunks[0], false, "Brainstorm knowledge context must not inline chunk body text.");
assert.deepEqual(
  context.readPlan.chunks[0].inspectCommand.argv.slice(0, 4),
  ["knowledge", "inspect", "--source", "ops-context"],
);
assert.equal(context.matchedSources[0].matchedFocus.length > 0, true);
assert.equal(context.matchedSources[0].scoreBreakdown.bestChunkScore > 0, true);

const finalSummaryFailure = runFailure([
  "knowledge",
  "brainstorm-context",
  "--query",
  "operator lifecycle",
  "--block",
  "final_summary",
]);
assert.equal(finalSummaryFailure.ok, false);
assert.equal(finalSummaryFailure.error.code, "INVALID_ARGUMENT");

const help = execFileSync(process.execPath, [cli, "knowledge", "brainstorm-context", "--help"], {
  cwd: repoRoot,
  encoding: "utf8",
});
assert.equal(/--source\s+<name>/.test(help), false, "Brainstorm knowledge context command must not allow user-selected sources.");

const started = run([
  "brainstorm",
  "start",
  "--request",
  "Build an internal workflow where staff clarify lifecycle rules and page operation feedback before implementation.",
]);
const brainstormRequest = hydrateRequest(readProjectJson(started.data.requestPath ?? started.data.requestRef));
assert.equal(brainstormRequest.requestType, "brainstorm_session");
assert.equal(brainstormRequest.knowledgeContextProtocol.status, "enabled");
assert.equal(brainstormRequest.knowledgeQueryPlan.status, "enabled");
assert.equal(
  brainstormRequest.knowledgeQueryPlan.blocks.phase_scope.executionOrder.length,
  2,
  "Phase scope knowledge query plan must require dependency and capability-closure steps.",
);
assert.deepEqual(
  brainstormRequest.knowledgeQueryPlan.blocks.phase_scope.executionOrder.map((step) => step.queryKind),
  ["dependency_order", "capability_closure"],
  "Phase scope knowledge query plan must separate dependency order from capability closure.",
);
assert.equal(
  brainstormRequest.knowledgeQueryPlan.blocks.phase_scope.executionOrder[1].repeat,
  "Run once per candidate capability unit that could define a phase option or recommended phase.",
  "Capability closure query must run per candidate capability unit.",
);
includes(
  brainstormRequest.knowledgeQueryPlan.blocks.phase_scope.executionOrder[1].querySubjectRule,
  "exactly one candidate capability unit",
  "Capability closure query must have a single-subject rule.",
);
includes(
  brainstormRequest.knowledgeQueryPlan.blocks.phase_scope.executionOrder[1].queryConstructionRules.join("\n"),
  "Do not include sibling, downstream, or next-phase capability units in semanticFocus",
  "Capability closure query must not mix adjacent capability units.",
);
includes(
  brainstormRequest.knowledgeQueryPlan.sharedRules.join("\n"),
  "Do not combine sibling or downstream capability units",
  "Knowledge query plan must forbid mixed capability queries.",
);
includes(
  brainstormRequest.knowledgeQueryPlan.sharedRules.join("\n"),
  "one semanticFocus entry per concrete",
  "Knowledge query plan must forbid compound semanticFocus entries.",
);
includes(
  brainstormRequest.knowledgeQueryPlan.sharedRules.join("\n"),
  "self-contained",
  "Knowledge query plan must align semanticFocus granularity with semantic labels.",
);
includes(
  brainstormRequest.knowledgeQueryPlan.sharedRules.join("\n"),
  "Do not rely on a separate object focus entry to qualify a bare operation word",
  "Knowledge query plan must require object-qualified operation focus text.",
);
assert.deepEqual(
  brainstormRequest.knowledgeQueryPlan.blocks.concept_grounding.executionOrder.map((step) => step.queryKind),
  ["scope_item_grounding"],
  "Concept grounding must query confirmed scope items rather than the whole system.",
);
assert.deepEqual(
  brainstormRequest.knowledgeQueryPlan.blocks.frontend_experience.executionOrder.map((step) => step.queryKind),
  ["page_operation_path"],
  "Frontend experience must query page-operation paths.",
);
assert.deepEqual(brainstormRequest.knowledgeContextProtocol.appliesToBlocks, [
  "phase_scope",
  "concept_grounding",
  "frontend_experience",
]);
assert.deepEqual(brainstormRequest.knowledgeContextProtocol.excludedBlocks, ["final_summary"]);
const phaseClosureRules = brainstormRequest.knowledgeQueryPlan.blocks.phase_scope.executionOrder[1].queryConstructionRules.join("\n");
const conceptGroundingRules = brainstormRequest.knowledgeQueryPlan.blocks.concept_grounding.executionOrder[0].queryConstructionRules.join("\n");
const frontendPathRules = brainstormRequest.knowledgeQueryPlan.blocks.frontend_experience.executionOrder[0].queryConstructionRules.join("\n");
includes(
  conceptGroundingRules,
  "pairing object focus with relevant operation",
  "Concept knowledge query plan must tell agents to build semantically complete focus anchors.",
);
includes(
  phaseClosureRules,
  "candidate phase capability units",
  "Phase scope knowledge query plan must tell agents to derive candidate phase anchors before composing options.",
);
includes(
  phaseClosureRules,
  "closed phase",
  "Phase scope knowledge query plan must target current-phase closure coverage.",
);
includes(
  phaseClosureRules,
  "self-contained",
  "Phase scope capability closure must use self-contained focus anchors.",
);
includes(
  phaseClosureRules,
  "Do not rely on a separate object focus entry to qualify a bare operation word",
  "Phase scope capability closure must not use bare operation focus text when the subject qualifier is available.",
);
includes(
  phaseClosureRules,
  "each identifiable component operation as separate operation focus entries",
  "Phase scope knowledge query plan must split connected processes into component operation focus anchors.",
);
includes(
  phaseClosureRules,
  "downstream execution operations",
  "Phase scope semanticFocus must avoid being diluted by downstream modules unless they are competing current-phase options.",
);
includes(
  conceptGroundingRules,
  "every confirmed current-phase included item",
  "Concept knowledge query plan must cover all confirmed current-phase scope items.",
);
includes(
  conceptGroundingRules,
  "self-contained",
  "Concept grounding must use self-contained focus anchors.",
);
includes(
  conceptGroundingRules,
  "single compound operation focus",
  "Concept knowledge query plan must not collapse lifecycle processes into one compound operation focus.",
);
includes(
  frontendPathRules,
  "no explicit page labels",
  "Frontend knowledge query plan must handle knowledge sources without page labels.",
);
includes(
  frontendPathRules,
  "page or flow focus",
  "Frontend knowledge query plan must prefer page or flow focus anchors.",
);
includes(
  frontendPathRules,
  "self-contained",
  "Frontend knowledge query plan must use self-contained focus anchors.",
);
includes(
  frontendPathRules,
  "do not rely on a compound operation focus alone",
  "Frontend knowledge query plan must not rely on a compound operation focus for workflows.",
);
includes(
  frontendPathRules,
  "target discovery",
  "Frontend knowledge query plan must target page-operation paths.",
);
includes(
  frontendPathRules,
  "Do not query only with business object names",
  "Frontend knowledge query plan must not prescribe object-only matching.",
);
assert.equal(brainstormRequest.knowledgeContextProtocol.perBlockLimits.maxSources, 2);
assert.equal(brainstormRequest.knowledgeContextProtocol.perBlockLimits.maxChunks, 5);
assert.equal(brainstormRequest.knowledgeContextProtocol.perBlockLimits.maxChunksPerSource, 5);
assert.deepEqual(
  brainstormRequest.knowledgeContextProtocol.command.argv,
  ["knowledge", "brainstorm-context", "--query-file", "{queryFile}"],
);
assert.equal(
  brainstormRequest.knowledgeContextProtocol.queryWorkspace.scope,
  "current_brainstorm_request",
  "Brainstorm knowledge query workspace must be scoped to the current request.",
);
assert.equal(
  brainstormRequest.knowledgeContextProtocol.queryWorkspace.requiredForCommand,
  true,
  "Brainstorm knowledge context must require query files from the request workspace.",
);
assert.match(
  brainstormRequest.knowledgeContextProtocol.queryWorkspace.directory,
  /^\.loom\/deliveries\/[^/]+\/tmp\/brainstorm\/phase-1\/brainstorm-session-[^/]+\/knowledge-queries$/,
  "Brainstorm knowledge query workspace must live under the current delivery/phase/request tmp directory.",
);
includes(
  brainstormRequest.knowledgeContextProtocol.command.placeholderRules["{queryFile}"],
  "queryWorkspace.directory",
  "Brainstorm queryFile placeholder must point agents to the request-scoped query workspace.",
);
includes(
  brainstormRequest.knowledgeContextProtocol.command.placeholderRules["{queryFile}"],
  "project-root tmp/loom",
  "Brainstorm queryFile placeholder must forbid old project tmp/loom query files.",
);
includes(
  brainstormRequest.knowledgeContextProtocol.queryWorkspace.rules.join("\n"),
  "another requestId",
  "Brainstorm query workspace rules must forbid previous request query files.",
);

const readPlanGroups = brainstormRequest.requestReadPlan.groups;
assert.ok(
  readPlanGroups.some((group) => group.groupId === "brainstorm_session_knowledge_context_protocol"),
  "Brainstorm requestReadPlan must expose knowledge context protocol as a grouped read.",
);
const retiredKnowledgeGuidanceField = ["knowledgeContextProtocol", "block" + "QueryGuidance"].join(".");
assert.equal(
  readPlanGroups
    .find((group) => group.groupId === "brainstorm_session_knowledge_context_protocol")
    .fields.includes(retiredKnowledgeGuidanceField),
  false,
  "Brainstorm requestReadPlan must not expose retired knowledge query guidance fields.",
);
assert.ok(
  readPlanGroups
    .find((group) => group.groupId === "brainstorm_session_knowledge_context_protocol")
    .fields.includes("knowledgeQueryPlan"),
  "Brainstorm requestReadPlan must explicitly expose knowledgeQueryPlan.",
);
const protocolRules = brainstormRequest.knowledgeContextProtocol.blockRules.join("\n");
includes(protocolRules, "follow knowledgeQueryPlan", "Brainstorm must make knowledgeQueryPlan the execution sequence authority.");
includes(protocolRules, "queryWorkspace.directory", "Brainstorm protocol must require request-scoped query files for every knowledge step.");
includes(protocolRules, "CLI rejects", "Brainstorm protocol must tell agents the CLI enforces query workspace isolation.");
includes(protocolRules, "Do not run knowledge brainstorm-context for final_summary", "final_summary must not run knowledge recall.");
includes(protocolRules, "inspect every chunk listed in context.readPlan.chunks", "available knowledge context must be inspected.");
includes(protocolRules, "Self-check each returned context", "Brainstorm must self-check knowledge coverage before presenting the block.");
includes(protocolRules, "self-contained", "Brainstorm protocol must carry the shared semantic anchor specificity rule.");
includes(protocolRules, "Do not ask the user to choose or name a knowledge source", "Brainstorm must not support manual knowledge source selection.");
includes(protocolRules, "semanticFocus should include them explicitly", "Brainstorm must instruct agents to provide concrete semanticFocus anchors when available.");
includes(protocolRules, "do not collapse it into a single compound operation semanticFocus", "Brainstorm must split connected-process semanticFocus anchors when possible.");
includes(protocolRules, "do not move lifecycle, replacement, recovery, or relationship focus to an adjacent business object", "Brainstorm must preserve object boundaries when creating semanticFocus.");
includes(protocolRules, "instead of inventing labels", "Brainstorm must forbid invented semanticFocus labels.");

const blockRules = brainstormRequest.clarificationConversationProtocol.blockExecutionRules.join("\n");
includes(blockRules, "follow knowledgeQueryPlan before presenting the block", "Brainstorm blocks must invoke the planned knowledge query sequence before presentation.");
includes(blockRules, "Do not run or use knowledge context for final_summary", "Brainstorm execution rules must exclude final_summary.");
includes(blockRules, "Knowledge context is reference material only", "Knowledge must not become direct scope/rule/page authority.");

const candidateRules = brainstormRequest.outputContract.schemaShape.candidateRules.join("\n");
includes(candidateRules, "Do not add knowledge source ids", "BrainstormCandidate must not accept knowledge refs as formal sources.");
includes(candidateRules, "candidate must preserve only the user's confirmed conclusion", "Candidate must keep confirmed conclusions instead of knowledge metadata.");
assert.equal("knowledgeContext" in brainstormRequest.outputContract.schemaShape, false);
assert.equal("knowledgeContextProtocol" in brainstormRequest.outputContract.schemaShape, false);

const staleQueryFile = path.join(projectRoot, "tmp", "loom", "phase-scope-stale.json");
writeJson(staleQueryFile, {
  naturalLanguageQuery: "stale query outside the current request workspace",
  brainstormBlock: "phase_scope",
  semanticFocus: [{ kind: "object", text: "stale object" }],
  sourceLimit: 2,
  chunkLimitPerSource: 5,
});
const staleFailure = runFailure(["knowledge", "brainstorm-context", "--query-file", staleQueryFile]);
assert.equal(staleFailure.ok, false);
assert.equal(staleFailure.error.code, "INVALID_ARGUMENT");
assert.match(
  staleFailure.error.message,
  /current Brainstorm request query workspace/,
  "Brainstorm knowledge context must reject stale project tmp query files while a Brainstorm session is active.",
);
assert.match(
  staleFailure.error.details.allowedDirectory,
  /knowledge-queries$/,
  "Brainstorm knowledge query guard must report the allowed request workspace.",
);

const allowedQueryDir = path.join(projectRoot, brainstormRequest.knowledgeContextProtocol.queryWorkspace.directory);
const allowedQueryFile = path.join(allowedQueryDir, "phase_scope-phase_scope_dependency_order-1.json");
const allowedQueryRef = path.join(
  brainstormRequest.knowledgeContextProtocol.queryWorkspace.directory,
  "phase_scope-phase_scope_dependency_order-1.json",
);
writeJson(allowedQueryFile, {
  naturalLanguageQuery: "operator lifecycle dependency order and phase boundary",
  brainstormBlock: "phase_scope",
  semanticFocus: [{ kind: "operation", text: "clarify lifecycle rules" }],
  sourceLimit: 2,
  chunkLimitPerSource: 5,
});
const allowedContext = run(["knowledge", "brainstorm-context", "--query-file", allowedQueryRef]).data.context;
assert.equal(allowedContext.block, "phase_scope");

fs.rmSync(fixtureRoot, { recursive: true, force: true });
fs.rmSync(loomHome, { recursive: true, force: true });
fs.rmSync(projectRoot, { recursive: true, force: true });

console.log("Brainstorm knowledge context protocol verification passed.");
