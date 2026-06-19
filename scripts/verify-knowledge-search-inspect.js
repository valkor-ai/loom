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

function assertScoresDescending(results, message) {
  for (let index = 1; index < results.length; index += 1) {
    assert.equal(results[index - 1].score >= results[index].score, true, message);
  }
}

const fundDoc = writeFixture("knowledge/fund.md", [
  "# Fund account operations",
  "",
  "Withdrawal requires the withdrawal password and available cash.",
  "",
  "If available cash is insufficient, the withdrawal is blocked with a business reason.",
  "",
  "Closing a fund account requires all cash to be withdrawn first.",
].join("\n"));
const fundCloseDoc = writeFixture("knowledge/fund-close.md", [
  "# Fund account close",
  "",
  "Closing a fund account requires all cash to be withdrawn first.",
  "",
  "The fund account is separated from the related securities account after close.",
  "",
  "Trading stays blocked until a usable fund account relation is restored.",
].join("\n"));
const securitiesCloseDoc = writeFixture("knowledge/securities-close.md", [
  "# Securities account close",
  "",
  "A securities account close uses identity documents and the securities account card.",
  "",
  "Close close close close close close close close close close close close close close close close close close close close.",
].join("\n"));
const pageDoc = writeFixture("knowledge/staff-page.md", [
  "# Staff workspace operation path",
  "",
  "A staff workspace page operation path should provide search, pagination, row action entry, form inputs, success feedback, business-blocking feedback, and refresh readback.",
].join("\n"));

run(["knowledge", "add", "--name", "funds-search", fundDoc, fundCloseDoc, securitiesCloseDoc, pageDoc]);
const build = run(["knowledge", "build", "funds-search"]);
const request = build.data.firstRequest;

writeJson(request.outputContract.resultFile, {
  schemaVersion: "1.0",
  buildId: request.buildId,
  packId: request.packId,
  chunkResults: request.chunkPack.chunks.map((chunk) => ({
    chunkId: chunk.chunkId,
    status: "completed",
    ...semanticForChunk(chunk),
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
assertScoresDescending(search.data.results, "knowledge search results must be ordered by displayed score.");
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

const rerankedSearch = run([
  "knowledge",
  "search",
  "--source",
  "funds-search",
  "--query",
  "fund account close cash securities account close close close",
  "--block",
  "concept_grounding",
  "--semantic-focus",
  "object:Fund account",
  "--semantic-focus",
  "operation:Close",
  "--limit",
  "5",
]);
assert.equal(rerankedSearch.data.results.length >= 2, true);
assertScoresDescending(rerankedSearch.data.results, "semantic rerank results must remain ordered by displayed score.");
assert.match(
  rerankedSearch.data.results[0].headingPath.join(" / "),
  /Fund account close/,
  "complete object+operation semantic matches must rank ahead of adjacent-object operation-only matches",
);
assert.deepEqual(
  rerankedSearch.data.results[0].matchedLabels.map((label) => `${label.kind}:${label.text}`).sort(),
  ["object:Fund account", "operation:Close"],
);

const fallbackSearch = run([
  "knowledge",
  "search",
  "--source",
  "funds-search",
  "--semantic-focus",
  "object:Fund account",
  "--semantic-focus",
  "operation:Transfer",
  "--block",
  "concept_grounding",
]);
assert.equal(fallbackSearch.data.results.length > 0, true, "partial semantic focus matches must still be returned as fallback");
assert.equal(
  fallbackSearch.data.results.some((chunk) => chunk.matchedLabels.some((label) => label.kind === "object" && label.text === "Fund account")),
  true,
);

const frontendSearch = run([
  "knowledge",
  "search",
  "--source",
  "funds-search",
  "--query",
  "staff page operation path search pagination action entry success feedback refresh readback",
  "--block",
  "frontend_experience",
  "--semantic-focus",
  "flow:Page operation path",
  "--semantic-focus",
  "operation:Refresh readback",
]);
assert.match(
  frontendSearch.data.results[0].headingPath.join(" / "),
  /Staff workspace operation path/,
  "frontend search must prefer page/flow plus operation-path matches over generic business chunks",
);
assertScoresDescending(frontendSearch.data.results, "frontend search results must be ordered by displayed score.");

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

function semanticForChunk(chunk) {
  const title = chunk.headingPath.join(" / ");
  if (title.includes("Fund account close")) {
    return {
      summary: "Fund account close requires cash withdrawal and relation separation.",
      semanticLabels: [
        {
          kind: "object",
          text: "Fund account",
          normalizedText: "fund account",
          aliases: [],
          confidence: "high",
        },
        {
          kind: "operation",
          text: "Close",
          normalizedText: "close",
          aliases: [],
          confidence: "high",
        },
      ],
      blockAffinity: {
        phaseScope: 0.3,
        conceptGrounding: 0.9,
        frontendExperience: 0.1,
        finalSummary: 0.1,
      },
    };
  }
  if (title.includes("Securities account close")) {
    return {
      summary: "Securities account close is an adjacent account operation.",
      semanticLabels: [
        {
          kind: "object",
          text: "Securities account",
          normalizedText: "securities account",
          aliases: [],
          confidence: "high",
        },
        {
          kind: "operation",
          text: "Close",
          normalizedText: "close",
          aliases: [],
          confidence: "high",
        },
      ],
      blockAffinity: {
        phaseScope: 0.3,
        conceptGrounding: 0.8,
        frontendExperience: 0.1,
        finalSummary: 0.1,
      },
    };
  }
  if (title.includes("Staff workspace operation path")) {
    return {
      summary: "Staff workspace page operation paths include search, action entry, feedback, and refresh readback.",
      semanticLabels: [
        {
          kind: "flow",
          text: "Page operation path",
          normalizedText: "page operation path",
          aliases: [],
          confidence: "high",
        },
        {
          kind: "operation",
          text: "Refresh readback",
          normalizedText: "refresh readback",
          aliases: [],
          confidence: "high",
        },
      ],
      blockAffinity: {
        phaseScope: 0.1,
        conceptGrounding: 0.3,
        frontendExperience: 0.95,
        finalSummary: 0.1,
      },
    };
  }
  return {
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
  };
}
