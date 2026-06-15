#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

function main() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-deploy-active-op-"));
  try {
    verifyActiveOperationBlocksMutations(path.join(root, "active"));
    verifyReadOnlyCommandsObserveActiveOperation(path.join(root, "readonly"));
    verifyStaleOperationIsArchived(path.join(root, "stale"));
    verifyAdapterGuidance();
    console.log(`deploy active operation boundary verification passed in ${root}`);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function verifyActiveOperationBlocksMutations(projectRoot) {
  writeActiveOperation(projectRoot, {
    operationId: "deploy-op-live-block",
    command: "deploy.run",
    phase: "building",
    pid: process.pid,
  });

  for (const args of [
    ["deploy", "run"],
    ["deploy", "prepare"],
    ["deploy", "up"],
    ["deploy", "down"],
    ["deploy", "bootstrap", "--confirm"],
    ["deploy", "validate"],
    ["deploy", "repair"],
  ]) {
    const envelope = runCli(args, projectRoot, true);
    assert.equal(envelope.ok, false, `${args.join(" ")} should be blocked.`);
    assert.equal(envelope.error.code, "DEPLOY_OPERATION_ACTIVE");
    const details = envelope.error.details;
    assert.equal(details.activeOperation.operationActive, true);
    assert.equal(details.activeOperation.operationId, "deploy-op-live-block");
    assert.equal(details.activeOperation.command, "deploy.run");
    assert.equal(details.activeOperation.phase, "building");
    assert.equal(details.activeOperation.logRef, ".loom/deployment/logs/local.log");
    assert.deepEqual(details.activeOperation.allowedCommands, ["deploy status", "deploy inspect", "deploy logs"]);
    assert.deepEqual(details.activeOperation.forbiddenActions, [
      "deploy run",
      "deploy up",
      "deploy down",
      "raw docker compose",
      "kill process",
    ]);
    assert.equal(details.agentFacingBlocker?.mode, "observe_active_operation");
    assert.equal(details.agentFacingBlocker?.retryPolicy?.autoRetry, false);
    assert.ok(
      details.agentFacingBlocker?.allowedActions?.some((action) => /deploy status, deploy inspect, or deploy logs/i.test(action)),
      "active operation blocker must allow only observation commands.",
    );
    assert.ok(
      details.agentFacingBlocker?.forbiddenActions?.some((action) => /raw Docker/i.test(action)),
      "active operation blocker must forbid raw Docker.",
    );
    assert.ok(
      details.agentFacingBlocker?.forbiddenActions?.some((action) => /kill/i.test(action)),
      "active operation blocker must forbid killing processes.",
    );
  }
}

function verifyReadOnlyCommandsObserveActiveOperation(projectRoot) {
  writeActiveOperation(projectRoot, {
    operationId: "deploy-op-live-observe",
    command: "deploy.up",
    phase: "starting",
    pid: process.pid,
  });
  writeFile(path.join(projectRoot, ".loom/deployment/logs/local.log"), "first line\nsecond line\n");

  const status = runCli(["deploy", "status"], projectRoot, false);
  assert.equal(status.ok, true);
  assertActiveObservationInstruction(status, "deploy.status");
  assert.equal(status.data.operationActive, true);
  assert.equal(status.data.activeOperation.operationId, "deploy-op-live-observe");
  assert.equal(status.data.activeOperation.command, "deploy.up");
  assert.equal(status.data.activeOperation.phase, "starting");

  const inspect = runCli(["deploy", "inspect", "--refresh"], projectRoot, false);
  assert.equal(inspect.ok, true);
  assertActiveObservationInstruction(inspect, "deploy.inspect");
  assert.equal(inspect.data.operationActive, true);
  assert.equal(inspect.data.refreshed, false);
  assert.equal(inspect.data.activeOperation.operationId, "deploy-op-live-observe");

  const logs = runCli(["deploy", "logs"], projectRoot, false);
  assert.equal(logs.ok, true);
  assertActiveObservationInstruction(logs, "deploy.logs");
  assert.equal(logs.data.operationActive, true);
  assert.deepEqual(logs.data.lines, ["first line", "second line"]);
  assert.equal(logs.data.fullLogRef, ".loom/deployment/logs/local.log");
}

function assertActiveObservationInstruction(envelope, sourceCommand) {
  assert.equal(envelope.instruction?.mode, "observe_active_deploy_operation");
  assert.equal(envelope.instruction?.autoContinue, false);
  assert.equal(envelope.actionRequired, undefined, "active observation must not create an infinite auto-poll action.");
  assert.equal(envelope.instruction?.nextAction?.reason, "DEPLOY_OPERATION_ACTIVE");
  assert.equal(envelope.instruction?.nextAction?.refs?.operationId, "deploy-op-live-observe");
  assert.equal(envelope.instruction?.nextAction?.refs?.command, "deploy.up");
  assert.equal(envelope.instruction?.nextAction?.refs?.phase, "starting");
  assert.match(
    envelope.instruction?.routingRule ?? "",
    /Do not infer failure from an unchanged log tail/i,
    "active observation instruction must prevent log-stall failure inference.",
  );
  assert.match(
    envelope.instruction?.routingRule ?? "",
    /Do not start another deploy command/i,
    "active observation instruction must forbid competing deploy commands.",
  );
  assert.ok(
    envelope.instruction?.instructions?.some((item) => /deploy status, deploy inspect, or deploy logs/i.test(item)),
    "active observation instruction must restrict allowed observation commands.",
  );
  assert.ok(
    envelope.instruction?.instructions?.some((item) => /Do not run deploy run, deploy up, deploy down, raw docker/i.test(item)),
    "active observation instruction must forbid mutating/raw docker actions.",
  );
  assert.match(
    envelope.instruction?.finalResponseGuard?.invalidFinalResponseWhen ?? "",
    /still running/i,
    "active observation instruction must guard against premature final responses.",
  );
  assert.equal(envelope.instruction?.advisories?.[0]?.sourceCommand, sourceCommand);
}

function verifyStaleOperationIsArchived(projectRoot) {
  writeStaticProject(projectRoot);
  writeActiveOperation(projectRoot, {
    operationId: "deploy-op-stale",
    command: "deploy.prepare",
    phase: "preparing",
    pid: 999999,
  });

  const envelope = runCli(["deploy", "prepare", "--healthcheck-disabled"], projectRoot, false);
  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.prepared, true);
  assert.equal(fs.existsSync(activeOperationPath(projectRoot)), false);

  const stale = JSON.parse(fs.readFileSync(staleOperationPath(projectRoot), "utf8"));
  assert.equal(stale.operationId, "deploy-op-stale");
  assert.equal(stale.status, "stale");
}

function verifyAdapterGuidance() {
  const files = [
    "plugins/codex/skills/loom-deploy/SKILL.md",
    "plugins/claude-code/skills/loom-deploy/SKILL.md",
    "plugins/opencode/.opencode/commands/loom-deploy.md",
  ];
  for (const relative of files) {
    const content = fs.readFileSync(path.join(repoRoot, relative), "utf8");
    assert.match(content, /DEPLOY_OPERATION_ACTIVE/, `${relative} must mention active operation blocker.`);
    assert.match(content, /deploy status.*deploy inspect.*deploy logs/s, `${relative} must limit observation commands.`);
    assert.match(content, /raw `docker compose`/, `${relative} must forbid raw docker compose.`);
    assert.match(content, /kill, `pkill`, or stop deploy/, `${relative} must forbid killing deploy processes.`);
  }
}

function runCli(args, projectRoot, allowFailure) {
  let stdout;
  try {
    stdout = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
      cwd: repoRoot,
      env: {
        ...process.env,
        LOOM_AGENT_PROFILE: "codex",
        LOOM_COMPACT_OUTPUT: "1",
      },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    stdout = error.stdout?.toString("utf8") ?? "";
    if (!allowFailure) {
      throw error;
    }
  }
  assert.ok(stdout.trim(), `${args.join(" ")} must return a JSON envelope.`);
  return JSON.parse(stdout);
}

function writeActiveOperation(projectRoot, patch) {
  const now = new Date().toISOString();
  writeFile(activeOperationPath(projectRoot), `${JSON.stringify({
    schemaVersion: 1,
    operationId: patch.operationId,
    command: patch.command,
    phase: patch.phase,
    pid: patch.pid,
    projectRoot,
    startedAt: now,
    updatedAt: now,
    logRef: ".loom/deployment/logs/local.log",
    specRef: null,
    status: "running",
  }, null, 2)}\n`);
}

function writeStaticProject(projectRoot) {
  writeFile(path.join(projectRoot, "index.html"), "<!doctype html><h1>loom deploy verifier</h1>\n");
}

function activeOperationPath(projectRoot) {
  return path.join(projectRoot, ".loom/deployment/state/active-operation.json");
}

function staleOperationPath(projectRoot) {
  return path.join(projectRoot, ".loom/deployment/state/last-stale-operation.json");
}

function writeFile(file, content) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content, "utf8");
}

main();
