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
  "A page operation path should identify target discovery, action entry, success feedback, blocking feedback, and refresh behavior.",
].join("\n"));

run(["init"]);
run(["knowledge", "add", "--name", "ops-context", doc]);
const build = run(["knowledge", "build", "ops-context"]);
const request = build.data.firstRequest;

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
  chunkLimitPerSource: 3,
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
assert.equal(context.readPlan.chunks.length <= 5, true);
assert.equal(context.matchedSources[0].topChunks.length <= 3, true);
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
assert.deepEqual(brainstormRequest.knowledgeContextProtocol.appliesToBlocks, [
  "phase_scope",
  "concept_grounding",
  "frontend_experience",
]);
assert.deepEqual(brainstormRequest.knowledgeContextProtocol.excludedBlocks, ["final_summary"]);
assert.equal(
  brainstormRequest.knowledgeContextProtocol.blockQueryGuidance.frontend_experience.naturalLanguageQueryMustCombine.length,
  2,
);
assert.deepEqual(
  brainstormRequest.knowledgeContextProtocol.blockQueryGuidance.frontend_experience.semanticFocusPriorityKinds.slice(0, 2),
  ["page", "flow"],
);
includes(
  brainstormRequest.knowledgeContextProtocol.blockQueryGuidance.concept_grounding.semanticFocusRules.join("\n"),
  "Pair object focus with the most relevant operation",
  "Concept knowledge query guidance must tell agents to build semantically complete focus anchors.",
);
includes(
  brainstormRequest.knowledgeContextProtocol.blockQueryGuidance.frontend_experience.semanticFocusRules.join("\n"),
  "page or flow focus",
  "Frontend knowledge query guidance must prefer page or flow focus anchors.",
);
includes(
  brainstormRequest.knowledgeContextProtocol.blockQueryGuidance.frontend_experience.retrievalIntent,
  "target discovery",
  "Frontend knowledge query guidance must target page-operation paths.",
);
includes(
  brainstormRequest.knowledgeContextProtocol.blockQueryGuidance.frontend_experience.mustNotDo.join("\n"),
  "Do not require users to register a separate frontend knowledge source",
  "Frontend knowledge query guidance must not prescribe knowledge source organization.",
);
assert.equal(brainstormRequest.knowledgeContextProtocol.perBlockLimits.maxSources, 2);
assert.equal(brainstormRequest.knowledgeContextProtocol.perBlockLimits.maxChunks, 5);
assert.equal(brainstormRequest.knowledgeContextProtocol.perBlockLimits.maxChunksPerSource, 3);
assert.deepEqual(
  brainstormRequest.knowledgeContextProtocol.command.argv,
  ["knowledge", "brainstorm-context", "--query-file", "{queryFile}"],
);

const readPlanGroups = brainstormRequest.requestReadPlan.groups;
assert.ok(
  readPlanGroups.some((group) => group.groupId === "brainstorm_session_knowledge_context_protocol"),
  "Brainstorm requestReadPlan must expose knowledge context protocol as a grouped read.",
);
const protocolRules = brainstormRequest.knowledgeContextProtocol.blockRules.join("\n");
includes(protocolRules, "Do not run knowledge brainstorm-context for final_summary", "final_summary must not run knowledge recall.");
includes(protocolRules, "inspect every chunk listed in context.readPlan.chunks", "available knowledge context must be inspected.");
includes(protocolRules, "Do not ask the user to choose or name a knowledge source", "Brainstorm must not support manual knowledge source selection.");
includes(protocolRules, "semanticFocus should include them explicitly", "Brainstorm must instruct agents to provide concrete semanticFocus anchors when available.");
includes(protocolRules, "instead of inventing labels", "Brainstorm must forbid invented semanticFocus labels.");

const blockRules = brainstormRequest.clarificationConversationProtocol.blockExecutionRules.join("\n");
includes(blockRules, "follow knowledgeContextProtocol before presenting the block", "Brainstorm blocks must invoke knowledge protocol before presentation.");
includes(blockRules, "Do not run or use knowledge context for final_summary", "Brainstorm execution rules must exclude final_summary.");
includes(blockRules, "Knowledge context is reference material only", "Knowledge must not become direct scope/rule/page authority.");

const candidateRules = brainstormRequest.outputContract.schemaShape.candidateRules.join("\n");
includes(candidateRules, "Do not add knowledge source ids", "BrainstormCandidate must not accept knowledge refs as formal sources.");
includes(candidateRules, "candidate must preserve only the user's confirmed conclusion", "Candidate must keep confirmed conclusions instead of knowledge metadata.");
assert.equal("knowledgeContext" in brainstormRequest.outputContract.schemaShape, false);
assert.equal("knowledgeContextProtocol" in brainstormRequest.outputContract.schemaShape, false);

fs.rmSync(fixtureRoot, { recursive: true, force: true });
fs.rmSync(loomHome, { recursive: true, force: true });
fs.rmSync(projectRoot, { recursive: true, force: true });

console.log("Brainstorm knowledge context protocol verification passed.");
