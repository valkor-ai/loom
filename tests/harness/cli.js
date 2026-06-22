const assert = require("node:assert/strict");
const { execFileSync, spawnSync } = require("node:child_process");

const { cliPath, repoRoot } = require("./root");

function cliArgs(args, projectRoot, options = {}) {
  const result = [cliPath, ...args];
  if (projectRoot) {
    result.push("--project-root", projectRoot);
  }
  if (options.json !== false) {
    result.push("--json");
  }
  if (options.compact) {
    result.push("--compact");
  }
  return result;
}

function runEnvelope(args, projectRoot, options = {}) {
  const output = runText(args, projectRoot, options);
  const envelope = JSON.parse(output);
  if (options.assertOk !== false) {
    assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  }
  return options.raw ? { output, envelope } : envelope;
}

function runText(args, projectRoot, options = {}) {
  return execFileSync(process.execPath, cliArgs(args, projectRoot, options), {
    cwd: repoRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      LOOM_AGENT_PROFILE: "codex",
      ...(options.compactOutput ? { LOOM_COMPACT_OUTPUT: "1" } : {}),
      ...(options.env ?? {}),
    },
    stdio: options.stdio ?? ["ignore", "pipe", "pipe"],
  });
}

function runCli(args, projectRoot, options = {}) {
  const envelope = runEnvelope(args, projectRoot, options);
  return options.returnEnvelope ? envelope : envelope.data;
}

function runFailure(args, projectRoot, options = {}) {
  const result = spawnSync(process.execPath, cliArgs(args, projectRoot, options), {
    cwd: repoRoot,
    encoding: "utf8",
    env: {
      ...process.env,
      LOOM_AGENT_PROFILE: "codex",
      ...(options.compactOutput ? { LOOM_COMPACT_OUTPUT: "1" } : {}),
      ...(options.env ?? {}),
    },
  });
  assert.notEqual(result.status, 0, `Expected command to fail: ${args.join(" ")}\n${result.stdout}\n${result.stderr}`);
  return JSON.parse(result.stdout);
}

module.exports = {
  runCli,
  runEnvelope,
  runFailure,
  runText,
};
