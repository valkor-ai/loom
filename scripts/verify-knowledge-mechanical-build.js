#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-build-fixture-"));
const loomHome = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-build-home-"));
const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-build-project-"));

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

function writeFixture(relativePath, content) {
  const target = path.join(fixtureRoot, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content, "utf8");
  return target;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

const knowledgeDir = path.join(fixtureRoot, "knowledge");
const fundRules = writeFixture("knowledge/fund-rules.md", [
  "# Fund account rules",
  "",
  "Workers open fund accounts and bind existing securities accounts.",
  "",
  "## Withdrawal",
  "",
  "Withdrawal requires the withdrawal password. If available cash is insufficient, the operation is blocked.",
  "",
  "## Closing",
  "",
  "Before closing a fund account, all cash must be withdrawn and the securities-account relation must be separated.",
].join("\n"));
const workflow = writeFixture("knowledge/workflow.txt", [
  "The staff console must show success feedback, business blockers, and refreshed account state.",
  "Loss replacement creates a new fund account and keeps or restores the original securities-account relation.",
].join("\n\n"));
const structured = writeFixture("knowledge/fields.yaml", [
  "fields:",
  "  - fundAccountNo",
  "  - securitiesAccountNo",
  "  - withdrawalPassword",
].join("\n"));
const skipped = writeFixture("knowledge/raw.bin", "not indexed");

run(["knowledge", "add", "--name", "funds-build", knowledgeDir]);
const build = run(["knowledge", "build", "funds-build"]);

assert.equal(build.command, "knowledge.build");
assert.equal(build.data.status, "mechanical_ready");
assert.equal(build.data.name, "funds-build");
assert.match(build.data.sourceId, /^ksrc_funds-build_/);
assert.match(build.data.buildId, /^kbld_/);
assert.equal(build.data.roots.length, 1);
assert.equal(build.data.roots[0].type, "directory");
assert.equal(build.data.documentCount, 3);
assert.ok(build.data.chunkCount >= 3, "mechanical build should create searchable chunks");
assert.ok(
  build.data.skippedFiles.some((warning) => warning.path === skipped && warning.reason === "unsupported_file_type"),
  "build should preserve skipped file warnings for unsupported files under directories",
);

for (const filePath of [
  build.data.buildRunPath,
  build.data.chunksPath,
  build.data.snapshotPath,
  build.data.lexicalIndexPath,
]) {
  assert.equal(fs.existsSync(filePath), true, `${filePath} should exist`);
}

const buildRun = readJson(build.data.buildRunPath);
assert.equal(buildRun.status, "mechanical_ready");
assert.equal(buildRun.sourceId, build.data.sourceId);
assert.equal(buildRun.pendingOperations.length, 1);
assert.equal(buildRun.documents.length, 3);
assert.equal(buildRun.chunks.length, build.data.chunkCount);
assert.ok(buildRun.refs.chunks.endsWith("chunks.json"));
assert.ok(buildRun.refs.snapshot.endsWith("snapshot.json"));
assert.ok(buildRun.refs.lexicalIndex.endsWith("lexical-index.json"));

const chunksPayload = readJson(build.data.chunksPath);
assert.equal(chunksPayload.chunks.length, build.data.chunkCount);
for (const chunk of chunksPayload.chunks) {
  assert.ok(chunk.chunkId.startsWith("kchunk_"));
  assert.ok(chunk.documentId.startsWith("kdoc_"));
  assert.ok(chunk.tokenEstimate <= 1800, "chunk must respect hard token limit");
  assert.deepEqual(chunk.semanticLabels, [], "mechanical build must not invent semantic labels");
  assert.deepEqual(chunk.blockAffinity, {
    phaseScope: 0,
    conceptGrounding: 0,
    frontendExperience: 0,
    finalSummary: 0,
  });
  const bodyPath = path.join(path.dirname(build.data.chunksPath), chunk.textRef);
  const body = fs.readFileSync(bodyPath, "utf8");
  assert.match(body, /^Document:/);
  assert.match(body, /Section:/);
}

const snapshot = readJson(build.data.snapshotPath);
assert.equal(snapshot.files.length, 3);
const snapshotPaths = snapshot.files.map((entry) => entry.path).sort();
assert.deepEqual(snapshotPaths, [fundRules, structured, workflow].sort());
for (const file of snapshot.files) {
  assert.match(file.contentHash, /^sha256:[a-f0-9]{64}$/);
  assert.equal(typeof file.mtimeMs, "number");
}

const lexical = readJson(build.data.lexicalIndexPath);
assert.equal(lexical.chunkCount, build.data.chunkCount);
assert.equal(lexical.fieldWeights.title, 5);
assert.ok(lexical.terms.withdrawal, "lexical index should include deterministic Latin tokens");

const registry = readJson(path.join(loomHome, "knowledge", "registry.json"));
assert.deepEqual(registry.sources, [], "mechanical build must not publish a usable source before semantic enrichment");

const pending = run(["knowledge", "pending", "funds-build"]);
assert.equal(pending.data.pending.length, 1);
assert.equal(pending.data.pending[0].sourceId, build.data.sourceId);

const list = run(["knowledge", "list"]);
assert.deepEqual(list.data.sources, [{
  name: "funds-build",
  status: "pending",
  docs: null,
  lastBuild: null,
  pendingOperations: 1,
}]);

fs.rmSync(fixtureRoot, { recursive: true, force: true });
fs.rmSync(loomHome, { recursive: true, force: true });
fs.rmSync(projectRoot, { recursive: true, force: true });

console.log("Knowledge mechanical build verification passed.");
