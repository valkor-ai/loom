#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-semantic-fixture-"));
const loomHome = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-semantic-home-"));
const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-semantic-project-"));

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
      TZ: "Asia/Shanghai",
      LOOM_AGENT_PROFILE: "codex",
      LOOM_COMPACT_OUTPUT: "1",
      LOOM_HOME: loomHome,
    },
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, output);
  return envelope;
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

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

const doc = writeFixture("knowledge/fund.md", [
  "# Fund account",
  "",
  "Withdrawal requires a withdrawal password.",
  "",
  "Loss replacement creates a new fund account, inherits key information, and restores the securities-account relation.",
  "",
  ...Array.from({ length: 36 }, (_, index) => [
    `## Operation rule ${index + 1}`,
    "",
    Array.from({ length: 18 }, () => (
      "Fund account staff must verify the account state, password category, available cash, relation continuity, blocking reason, refresh feedback, and lifecycle result before confirming the counter operation."
    )).join(" "),
  ].join("\n")),
].join("\n"));

run(["knowledge", "add", "--name", "funds-semantic", doc]);
const staleBuild = run(["knowledge", "build", "funds-semantic"]);
const staleResume = run(["knowledge", "resume", "funds-semantic"]);
assert.equal(staleResume.data.status, "semantic_pending");
assert.equal(staleResume.instruction.requestRef, staleBuild.data.firstRequestPath);

const build = run(["knowledge", "build", "funds-semantic"]);
let request = build.data.firstRequest;
assert.equal(build.compact, true, "semantic build must remain executable in compact output mode");
assert.equal(build.data.packCount > 1, true, "fixture must exercise multi-pack semantic continuation");
assert.equal(build.instruction.mode, "generate_knowledge_semantics");
assert.equal(build.actionRequired.mode, "generate_knowledge_semantics");
assert.equal(build.actionRequired.autoContinue, true);
assert.equal(build.actionRequired.mustRunImmediately, true);
assert.equal(build.instruction.requestRef, build.data.firstRequestPath);
assert.equal(build.instruction.resultFile, request.outputContract.resultFile);
assertResultTemplate(request);
assert.deepEqual(request.generationRules.statusValues, ["completed", "low_signal", "unreadable"]);
assert.deepEqual(request.generationRules.blockAffinityFields, ["phaseScope", "conceptGrounding", "frontendExperience", "finalSummary"]);
assert.match(
  request.generationRules.resultTemplateRule,
  /outputContract\.resultTemplate/,
  "Semantic build request must make resultTemplate the result shape authority.",
);
assert.match(
  request.generationRules.resultTemplateRule,
  /Do not infer the schema from Loom source files/,
  "Semantic build request must forbid source-code schema guessing.",
);
assert.match(
  request.generationRules.blockAffinityGuidance.frontendExperience,
  /target discovery/,
  "Semantic build request must explain frontendExperience affinity using page-operation signals.",
);
assert.deepEqual(build.instruction.knowledgeSemantic, {
  sourceName: "funds-semantic",
  buildId: request.buildId,
  packId: request.packId,
  packIndex: request.packIndex,
  packCount: request.packCount,
  chunkCount: request.chunkPack.chunks.length,
  mustReadChunkText: true,
});
assert.deepEqual(build.instruction.submitCommand.argv, [
  "knowledge",
  "semantic",
  "submit",
  "--request",
  build.data.firstRequestPath,
  "--result-file",
  request.outputContract.resultFile,
]);

const legacyRequest = readJson(build.data.firstRequestPath);
legacyRequest.chunkPack.chunks = legacyRequest.chunkPack.chunks.map((chunk) => ({
  ...chunk,
  readCommand: {
    argv: ["knowledge", "inspect", "--build-id", legacyRequest.buildId, "--chunk", chunk.chunkId],
  },
}));
delete legacyRequest.outputContract.resultTemplate;
delete legacyRequest.generationRules.statusValues;
delete legacyRequest.generationRules.resultTemplateRule;
delete legacyRequest.generationRules.semanticLabelFieldRules;
delete legacyRequest.generationRules.blockAffinityFields;
delete legacyRequest.generationRules.blockAffinityValueRule;
writeJson(build.data.firstRequestPath, legacyRequest);

const initialResume = run(["knowledge", "resume", "funds-semantic"]);
assert.equal(initialResume.command, "knowledge.resume");
assert.equal(initialResume.data.status, "semantic_pending");
assert.equal(initialResume.instruction.mode, "generate_knowledge_semantics");
assert.equal(initialResume.instruction.requestRef, build.data.firstRequestPath);
assert.equal(initialResume.instruction.resultFile, request.outputContract.resultFile);
assert.equal(initialResume.data.nextRequest.packId, request.packId);
request = initialResume.data.nextRequest;
assert.deepEqual(
  request.chunkPack.chunks[0].readCommand.argv.slice(0, 6),
  ["knowledge", "inspect", "--source", "funds-semantic", "--build-id", request.buildId],
);
assert.deepEqual(
  readJson(build.data.firstRequestPath).chunkPack.chunks[0].readCommand.argv.slice(0, 6),
  ["knowledge", "inspect", "--source", "funds-semantic", "--build-id", request.buildId],
);
assertResultTemplate(readJson(build.data.firstRequestPath));

const firstChunkText = run(request.chunkPack.chunks[0].readCommand.argv);
assert.equal(firstChunkText.command, "knowledge.inspect");
assert.equal(firstChunkText.data.sourceName, "funds-semantic");
assert.equal(firstChunkText.data.buildId, request.buildId);
assert.equal(firstChunkText.data.chunkId, request.chunkPack.chunks[0].chunkId);
assert.match(firstChunkText.data.text, /Withdrawal requires a withdrawal password/);

const legacySubmitRequest = readJson(build.data.firstRequestPath);
delete legacySubmitRequest.outputContract.resultTemplate;
delete legacySubmitRequest.generationRules.statusValues;
delete legacySubmitRequest.generationRules.resultTemplateRule;
delete legacySubmitRequest.generationRules.semanticLabelFieldRules;
delete legacySubmitRequest.generationRules.blockAffinityFields;
delete legacySubmitRequest.generationRules.blockAffinityValueRule;
writeJson(build.data.firstRequestPath, legacySubmitRequest);
writeJson(request.outputContract.resultFile, {
  schemaVersion: "1.0",
  buildId: request.buildId,
  packId: request.packId,
  chunkResults: [],
});
const repair = run([
  "knowledge",
  "semantic",
  "submit",
  "--request",
  build.data.firstRequestPath,
  "--result-file",
  request.outputContract.resultFile,
]);
assert.equal(repair.data.status, "needs_repair");
assert.equal(repair.data.packId, request.packId);
assert.ok(repair.data.repairRequest.issues.some((issue) => issue.code === "missing_chunk_result"));
assert.equal(repair.instruction.mode, "generate_knowledge_semantics");
assert.equal(repair.actionRequired.autoContinue, true);
assert.equal(repair.instruction.requestRef, build.data.firstRequestPath);
assert.equal(repair.instruction.resultFile, request.outputContract.resultFile);
assert.match(
  repair.instruction.instructions.join("\n"),
  /outputContract\.resultTemplate/,
  "Semantic repair instruction must tell the agent to use resultTemplate.",
);
assertResultTemplate(readJson(build.data.firstRequestPath));
assert.equal(repair.instruction.repairRequestPath, repair.data.repairRequestPath);
assert.equal(repair.instruction.issues.some((issue) => issue.code === "missing_chunk_result"), true);
assert.equal(readJson(path.join(loomHome, "knowledge", "registry.json")).sources.length, 0);

let currentRequestPath = build.data.firstRequestPath;
let currentRequest = request;
let accepted;
for (let packNumber = 1; packNumber <= build.data.packCount; packNumber += 1) {
  const chunkText = run(currentRequest.chunkPack.chunks[0].readCommand.argv);
  assert.equal(chunkText.data.sourceName, "funds-semantic");
  assert.equal(chunkText.data.buildId, currentRequest.buildId);
  assert.equal(chunkText.data.chunkId, currentRequest.chunkPack.chunks[0].chunkId);

  writeSemanticResult(currentRequest);
  const response = run([
    "knowledge",
    "semantic",
    "submit",
    "--request",
    currentRequestPath,
    "--result-file",
    currentRequest.outputContract.resultFile,
  ]);
  assert.equal(response.data.status, "accepted");
  assert.equal(response.data.acceptedPackIds.length, packNumber);
  if (packNumber < build.data.packCount) {
    assert.equal(response.instruction.mode, "generate_knowledge_semantics");
    assert.equal(response.actionRequired.autoContinue, true);
    assert.equal(response.instruction.requestRef, response.data.nextRequestPath);
    assert.equal(response.instruction.resultFile, response.data.nextRequest.outputContract.resultFile);
    assert.equal(response.data.nextRequest.packIndex, packNumber + 1);
    const resumed = run(["knowledge", "resume", "funds-semantic"]);
    assert.equal(resumed.data.status, "semantic_pending");
    assert.equal(resumed.instruction.mode, "generate_knowledge_semantics");
    assert.equal(resumed.instruction.requestRef, response.data.nextRequestPath);
    assert.equal(resumed.data.nextRequest.packId, response.data.nextRequest.packId);
    currentRequestPath = response.data.nextRequestPath;
    currentRequest = response.data.nextRequest;
    continue;
  }
  accepted = response;
}

assert.ok(accepted, "final semantic submit response must be captured");
assert.equal(accepted.data.published.name, "funds-semantic");
assert.equal(accepted.data.published.sourceId, build.data.sourceId);
assert.equal(accepted.data.acceptedPackIds.length, build.data.packCount);
assert.equal("nextRequest" in accepted.data, false);
assert.equal(accepted.instruction, null, "published semantic build must not return another auto-runnable instruction");
assert.equal("actionRequired" in accepted, false, "published semantic build must not require another action");
assert.equal(fs.existsSync(staleBuild.data.buildRunPath), false, "publishing a newer build must remove stale pending build-runs for the same knowledge source");

const publishedResume = run(["knowledge", "resume", "funds-semantic"]);
assert.equal(publishedResume.data.status, "already_published");
assert.equal(publishedResume.instruction, null);
assert.equal("actionRequired" in publishedResume, false);

const registry = readJson(path.join(loomHome, "knowledge", "registry.json"));
assert.equal(registry.sources.length, 1);
assert.equal(registry.sources[0].name, "funds-semantic");
assert.equal(registry.sources[0].status, "enabled");
assert.equal(registry.sources[0].index.version, 1);
assert.equal(registry.sources[0].index.currentBuildId, build.data.buildId);
assert.equal(registry.sources[0].index.documentCount, 1);
assert.equal(registry.sources[0].index.chunkCount, build.data.chunkCount);

const list = run(["knowledge", "list"]);
assert.equal(list.data.timeZone, "Asia/Shanghai");
assert.equal(list.data.sources.length, 1);
assert.equal(list.data.sources[0].lastBuild, registry.sources[0].index.lastBuiltAt);
assert.equal(list.data.sources[0].lastBuildLocal, formatShanghai(registry.sources[0].index.lastBuiltAt));
assert.equal(list.data.sources[0].updatedAtLocal, formatShanghai(registry.sources[0].updatedAt));

const status = run(["knowledge", "status", "funds-semantic"]);
assert.equal(status.data.timeZone, "Asia/Shanghai");
assert.equal(status.data.source.index.lastBuiltAt, registry.sources[0].index.lastBuiltAt);
assert.equal(status.data.source.index.lastBuiltAtLocal, formatShanghai(registry.sources[0].index.lastBuiltAt));
assert.equal(status.data.source.updatedAtLocal, formatShanghai(registry.sources[0].updatedAt));

const pending = run(["knowledge", "pending", "funds-semantic"]);
assert.deepEqual(pending.data.pending, [], "published semantic build must clear pending changes");

const buildRun = readJson(build.data.buildRunPath);
assert.equal(buildRun.status, "published");
assert.ok(buildRun.refs.semanticIndex.endsWith("semantic-index.json"));

const chunks = readJson(build.data.chunksPath).chunks;
assert.equal(chunks[0].retrievalFields.summary, `Summary for ${chunks[0].chunkId}`);
assert.equal(chunks[0].semanticLabels[0].normalizedText, "withdrawal");
assert.equal(chunks[0].blockAffinity.conceptGrounding, 0.8);

const semanticIndex = readJson(path.join(path.dirname(build.data.buildRunPath), "semantic-index.json"));
assert.equal(semanticIndex.labels.withdrawal.postings[0].source, "label");
assert.equal(semanticIndex.labels["cash out"].postings[0].source, "alias");

const lexicalIndex = readJson(build.data.lexicalIndexPath);
assert.ok(lexicalIndex.terms.summary, "published lexical index should include semantic summaries");

fs.rmSync(fixtureRoot, { recursive: true, force: true });
fs.rmSync(loomHome, { recursive: true, force: true });
fs.rmSync(projectRoot, { recursive: true, force: true });

console.log("Knowledge semantic build verification passed.");

function formatShanghai(isoTimestamp) {
  const shifted = new Date(new Date(isoTimestamp).getTime() + 8 * 60 * 60 * 1000);
  return [
    `${shifted.getUTCFullYear()}-${pad(shifted.getUTCMonth() + 1)}-${pad(shifted.getUTCDate())}`,
    `${pad(shifted.getUTCHours())}:${pad(shifted.getUTCMinutes())}:${pad(shifted.getUTCSeconds())}`,
    "UTC+08:00",
  ].join(" ");
}

function pad(value) {
  return String(value).padStart(2, "0");
}

function assertResultTemplate(packRequest) {
  const template = packRequest.outputContract.resultTemplate;
  assert.equal(template.schemaVersion, "1.0");
  assert.equal(template.buildId, packRequest.buildId);
  assert.equal(template.packId, packRequest.packId);
  assert.equal(template.chunkResults.length, packRequest.chunkPack.chunks.length);
  assert.deepEqual(
    template.chunkResults.map((chunk) => chunk.chunkId),
    packRequest.chunkPack.chunks.map((chunk) => chunk.chunkId),
  );
  for (const chunkResult of template.chunkResults) {
    assert.equal(chunkResult.status, "completed");
    assert.equal(chunkResult.summary, "");
    assert.deepEqual(chunkResult.semanticLabels, []);
    assert.deepEqual(chunkResult.blockAffinity, {
      phaseScope: 0,
      conceptGrounding: 0,
      frontendExperience: 0,
      finalSummary: 0,
    });
    assert.deepEqual(chunkResult.notes, []);
  }
}

function writeSemanticResult(packRequest) {
  assertResultTemplate(packRequest);
  const result = structuredClone(packRequest.outputContract.resultTemplate);
  result.chunkResults = result.chunkResults.map((chunkResult, index) => ({
    ...chunkResult,
    status: packRequest.packIndex === 1 && index === 0 ? "completed" : "low_signal",
    summary: `Summary for ${chunkResult.chunkId}`,
    semanticLabels: packRequest.packIndex === 1 && index === 0 ? [{
      kind: "operation",
      text: "Withdrawal",
      normalizedText: "withdrawal",
      aliases: ["cash out"],
      confidence: "high",
    }] : [],
    blockAffinity: {
      phaseScope: 0.2,
      conceptGrounding: 0.8,
      frontendExperience: 0.3,
      finalSummary: 0.4,
    },
  }));
  writeJson(packRequest.outputContract.resultFile, result);
}
