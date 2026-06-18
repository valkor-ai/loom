#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-fixture-"));
const loomHome = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-home-"));
const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-knowledge-project-"));

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
  return {
    stdout: result.stdout,
    stderr: result.stderr,
    envelope: parseJsonOrNull(result.stdout),
  };
}

function parseJsonOrNull(value) {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
}

function writeFixture(relativePath, content) {
  const target = path.join(fixtureRoot, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content, "utf8");
  return target;
}

const mainDoc = writeFixture("domain/funds.md", "# Funds account\n开户、存款、取款、密码、挂失、销户。\n");
const secondDoc = writeFixture("domain/rules.txt", "取款需要取款密码，销户前必须取空现金。\n");
const unsupportedDoc = writeFixture("domain/raw.bin", "not indexed\n");
const explicitUnsupportedDoc = writeFixture("explicit.bin", "not supported\n");
const jsonDoc = writeFixture("extra/account.json", "{\"name\":\"funds\"}\n");

const missingName = runFailure(["knowledge", "add", mainDoc]);
assert.match(
  missingName.stderr,
  /required option '--name <name>' not specified/,
  "knowledge add must require --name before queueing changes",
);

const explicitUnsupported = runFailure(["knowledge", "add", "--name", "bad-source", explicitUnsupportedDoc]);
assert.equal(explicitUnsupported.envelope?.ok, false);
assert.equal(explicitUnsupported.envelope?.error.code, "INVALID_ARGUMENT");
assert.equal(explicitUnsupported.envelope?.error.details.reason, "unsupported_file_type");

const add = run(["knowledge", "add", "--name", "funds-domain", mainDoc, path.dirname(secondDoc)]);
assert.equal(add.command, "knowledge.add");
assert.equal(add.data.name, "funds-domain");
assert.equal(add.data.pending.createNew, true);
assert.equal(add.data.pending.operations.length, 1);
assert.equal(add.data.validation.acceptedFiles, 1);
assert.equal(add.data.validation.acceptedDirectories, 1);
assert.ok(add.data.validation.supportedFiles >= 2);
assert.ok(
  add.data.validation.skippedFiles.some((warning) => warning.path === unsupportedDoc),
  "directory scan should warn about unsupported files without rejecting the directory",
);
assert.equal(fs.existsSync(path.join(loomHome, "knowledge", "sources")), true);
assert.equal(
  fs.existsSync(path.join(loomHome, "knowledge", "sources", "funds-domain")),
  false,
  "registration must not build source indexes",
);

const listAfterAdd = run(["knowledge", "list"]);
assert.equal(typeof listAfterAdd.data.timeZone, "string");
assert.equal(listAfterAdd.data.sources.length, 1);
assert.equal(listAfterAdd.data.sources[0].name, "funds-domain");
assert.equal(listAfterAdd.data.sources[0].status, "pending");
assert.equal(listAfterAdd.data.sources[0].docs, null);
assert.equal(listAfterAdd.data.sources[0].lastBuild, null);
assert.equal(listAfterAdd.data.sources[0].lastBuildLocal, null);
assert.equal(listAfterAdd.data.sources[0].pendingOperations, 1);
assert.match(listAfterAdd.data.sources[0].updatedAtLocal, /^20\d\d-\d\d-\d\d \d\d:\d\d:\d\d UTC[+-]\d\d:\d\d$/);

const pending = run(["knowledge", "pending", "funds-domain"]);
assert.equal(pending.command, "knowledge.pending");
assert.equal(typeof pending.data.timeZone, "string");
assert.equal(pending.data.pending.length, 1);
assert.equal(pending.data.pending[0].operations[0].type, "add_paths");
assert.match(pending.data.pending[0].updatedAtLocal, /^20\d\d-\d\d-\d\d \d\d:\d\d:\d\d UTC[+-]\d\d:\d\d$/);

const update = run(["knowledge", "update", "funds-domain", "--add-path", jsonDoc]);
assert.equal(update.command, "knowledge.update");
assert.equal(update.data.operation.type, "add_paths");
assert.equal(update.data.pending.operations.length, 2);
assert.equal(update.data.validation.acceptedFiles, 1);

const removePath = run(["knowledge", "update", "funds-domain", "--remove-path", mainDoc]);
assert.equal(removePath.data.operation.type, "remove_paths");
assert.deepEqual(removePath.data.validation.acceptedPaths, []);
assert.equal(removePath.data.pending.operations.length, 3);

const discard = run(["knowledge", "discard", "funds-domain"]);
assert.equal(discard.data.discarded, true);
assert.equal(fs.existsSync(mainDoc), true, "discard must not delete source files");

const listAfterDiscard = run(["knowledge", "list"]);
assert.deepEqual(listAfterDiscard.data.sources, []);

const updateMissing = runFailure(["knowledge", "update", "funds-domain", "--add-path", mainDoc]);
assert.equal(updateMissing.envelope?.ok, false);
assert.equal(updateMissing.envelope?.error.code, "INVALID_ARGUMENT");
assert.match(updateMissing.envelope?.error.message ?? "", /does not exist/);

run(["knowledge", "add", "--name", "funds-domain", mainDoc]);
const status = run(["knowledge", "status", "funds-domain"]);
assert.equal(typeof status.data.timeZone, "string");
assert.equal(status.data.source, null);
assert.equal(status.data.pending.name, "funds-domain");
assert.match(status.data.pending.createdAtLocal, /^20\d\d-\d\d-\d\d \d\d:\d\d:\d\d UTC[+-]\d\d:\d\d$/);

const remove = run(["knowledge", "remove", "funds-domain"]);
assert.equal(remove.data.removedSource, false);
assert.equal(remove.data.removedPending, true);
assert.equal(fs.existsSync(mainDoc), true, "remove must not delete source documents");

fs.rmSync(fixtureRoot, { recursive: true, force: true });
fs.rmSync(loomHome, { recursive: true, force: true });
fs.rmSync(projectRoot, { recursive: true, force: true });

console.log("Knowledge registration verification passed.");
