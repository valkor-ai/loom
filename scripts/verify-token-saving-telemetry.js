#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function captureStdout(fn) {
  const originalWrite = process.stdout.write;
  let output = "";
  process.stdout.write = function write(chunk, encoding, callback) {
    output += Buffer.isBuffer(chunk) ? chunk.toString("utf8") : String(chunk);
    if (typeof encoding === "function") {
      encoding();
    } else if (typeof callback === "function") {
      callback();
    }
    return true;
  };
  try {
    fn();
  } finally {
    process.stdout.write = originalWrite;
  }
  return output;
}

function runJson(args, projectRoot, options = {}) {
  const cliArgs = [cli, ...args, "--project-root", projectRoot, "--json"];
  if (options.compact) {
    cliArgs.push("--compact");
  }
  const output = execFileSync(process.execPath, cliArgs, {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return envelope;
}

function runStatus(projectRoot) {
  const envelope = runJson(["status"], projectRoot, { compact: true });
  return envelope.data;
}

function assertSourceTotals(telemetry, source, expectedCount) {
  const totals = telemetry.totals.bySource[source];
  assert.ok(totals, `missing source totals for ${source}`);
  assert.equal(totals.eventCount, expectedCount, `${source}: event count`);
  assert.ok(totals.bytesAvoided > 0, `${source}: expected positive bytes avoided`);
  assert.ok(totals.estimatedTokensSaved > 0, `${source}: expected positive token estimate`);
}

async function main() {
  const { initProject } = require("../dist/core/operations/init-project");
  const { writeRequestManifestAtomic } = require("../dist/core/operations/request-manifest");
  const { readTokenSavingSummary } = require("../dist/core/operations/token-saving-telemetry");
  const { printEnvelope } = require("../dist/commands/output");

  const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-token-saving-verify-"));
  const sentinel = "TOKEN_SAVING_VERIFY_SENTINEL_USER_CONTENT";
  await initProject({ projectRoot });

  const compactOutput = captureStdout(() => {
    printEnvelope({
      ok: true,
      command: "verify-token-saving",
      version: "0.0.0",
      projectRoot,
      data: {
        requestRef: ".loom/tmp/request.json",
        largeIgnoredPayload: Array.from({ length: 100 }, (_, index) => ({
          index,
          value: `${sentinel}-${index}-${"x".repeat(60)}`,
        })),
      },
      summary: "verify token-saving telemetry",
    }, { compact: true });
  });
  assert.doesNotThrow(() => JSON.parse(compactOutput), "compact envelope output must stay valid JSON");

  const requestFile = projectFile(projectRoot, ".loom/tmp/request.json");
  await writeRequestManifestAtomic(projectRoot, requestFile, {
    schemaVersion: "1.0",
    requestId: "verify-token-saving",
    agentAction: {
      actionKind: "execute_task",
      read: {
        required: ["this request"],
        displayPolicy: "compact",
      },
    },
    outputContract: {
      schemaShape: {
        fields: Array.from({ length: 100 }, (_, index) => ({
          name: `field${index}`,
          type: "string",
          description: `${sentinel}-schema-${index}`,
        })),
      },
    },
    rules: Array.from({ length: 100 }, (_, index) => `${sentinel}-rule-${index}-${"x".repeat(40)}`),
  });

  runJson(["inspect", "--request", ".loom/tmp/request.json", "--field", "outputContract.schemaShape"], projectRoot);

  const telemetryFile = projectFile(projectRoot, ".loom/metrics/token-saving.json");
  const telemetry = readJson(telemetryFile);
  assert.equal(telemetry.schemaVersion, "1.0", "telemetry schema version");
  assert.equal(telemetry.totals.eventCount, 3, "expected compact envelope, request manifest, and inspect selector events");
  assertSourceTotals(telemetry, "compact_envelope", 1);
  assertSourceTotals(telemetry, "request_manifest_refs", 1);
  assertSourceTotals(telemetry, "inspect_selectors", 1);
  assert.equal(JSON.stringify(telemetry).includes(sentinel), false, "telemetry must not store prompt, request, or artifact bodies");
  assert.ok(
    telemetry.recentEvents.every((event) => event.fullBytes > event.compactBytes),
    "every event must record a real byte reduction",
  );

  const status = runStatus(projectRoot);
  assert.equal(status.tokenSaving.telemetryRef, ".loom/metrics/token-saving.json", "status telemetry ref");
  assert.equal(status.tokenSaving.eventCount, 3, "status must expose the current telemetry total");
  assert.deepEqual(
    Object.keys(status.tokenSaving.bySource).sort(),
    ["compact_envelope", "inspect_selectors", "request_manifest_refs"],
    "status must expose source totals",
  );

  const metrics = runJson(["metrics", "token-saving"], projectRoot, { compact: true }).data.tokenSaving;
  assert.equal(metrics.telemetryRef, ".loom/metrics/token-saving.json", "metrics telemetry ref");
  assert.equal(metrics.eventCount, 3, "metrics command must expose the current telemetry total");
  assert.ok(metrics.recentEvents.some((event) => event.source === "inspect_selectors"), "metrics command must expose inspect selector events");
  assert.equal(JSON.stringify(metrics).includes(sentinel), false, "metrics command must not expose prompt, request, or artifact bodies");

  const afterReadCommands = await readTokenSavingSummary(projectRoot);
  assert.equal(afterReadCommands.totals.eventCount, 3, "status and metrics must not mutate token-saving telemetry while displaying it");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
