#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-search-fixture-"));
const loomHome = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-search-home-"));
const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-search-project-"));

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

const doc = writeFixture("knowledge/fund.md", [
  "# Fund account operations",
  "",
  "Withdrawal requires the withdrawal password and available cash.",
  "",
  "If available cash is insufficient, the withdrawal is blocked with a business reason.",
  "",
  "Closing a fund account requires all cash to be withdrawn first.",
].join("\n"));

run(["knowledge", "add", "--name", "funds-search", doc]);
const build = run(["knowledge", "build", "funds-search"]);
const request = build.data.firstRequest;

writeJson(request.outputContract.resultFile, {
  schemaVersion: "1.0",
  buildId: request.buildId,
  packId: request.packId,
  chunkResults: request.chunkPack.chunks.map((chunk) => ({
    chunkId: chunk.chunkId,
    status: "completed",
    summary: "Withdrawal rules require password and available cash checks.",
    semanticLabels: [{
      kind: "operation",
      text: "Withdrawal",
      normalizedText: "withdrawal",
      aliases: ["cash out"],
      confidence: "high",
    }],
    blockAffinity: {
      phaseScope: 0.2,
      conceptGrounding: 0.9,
      frontendExperience: 0.4,
      finalSummary: 0.1,
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

const search = run([
  "knowledge",
  "search",
  "--source",
  "funds-search",
  "--query",
  "withdrawal password",
  "--block",
  "concept_grounding",
  "--semantic-focus",
  "operation:Withdrawal",
  "--limit",
  "5",
]);

assert.equal(search.command, "knowledge.search");
assert.equal(search.data.results.length > 0, true);
const top = search.data.results[0];
assert.equal(top.sourceName, "funds-search");
assert.equal(top.matchedLabels[0].kind, "operation");
assert.equal(top.matchedLabels[0].matchSource, "text");
assert.equal("text" in top, false, "knowledge search must not return chunk body text");
assert.deepEqual(top.inspectCommand.argv.slice(0, 4), ["knowledge", "inspect", "--source", "funds-search"]);

const inspect = run(top.inspectCommand.argv);
assert.equal(inspect.command, "knowledge.inspect");
assert.equal(inspect.data.chunkId, top.chunkId);
assert.match(inspect.data.text, /^Document:/);
assert.match(inspect.data.text, /Withdrawal requires the withdrawal password/);

const aliasSearch = run([
  "knowledge",
  "search",
  "--source",
  "funds-search",
  "--semantic-focus",
  "operation:cash out",
  "--block",
  "concept_grounding",
]);
assert.equal(aliasSearch.data.results.length > 0, true);
assert.equal(aliasSearch.data.results[0].matchedLabels[0].matchSource, "alias");

const finalSummarySearch = runFailure([
  "knowledge",
  "search",
  "--source",
  "funds-search",
  "--query",
  "withdrawal",
  "--block",
  "final_summary",
]);
assert.equal(finalSummarySearch.ok, false);
assert.equal(finalSummarySearch.error.code, "INVALID_ARGUMENT");

run(["knowledge", "disable", "funds-search"]);
const disabledSearch = run([
  "knowledge",
  "search",
  "--source",
  "funds-search",
  "--query",
  "withdrawal",
]);
assert.deepEqual(disabledSearch.data.results, [], "disabled sources should not participate in search");

fs.rmSync(fixtureRoot, { recursive: true, force: true });
fs.rmSync(loomHome, { recursive: true, force: true });
fs.rmSync(projectRoot, { recursive: true, force: true });

console.log("Knowledge search and inspect verification passed.");
