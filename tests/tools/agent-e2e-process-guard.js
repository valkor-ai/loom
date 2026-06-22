#!/usr/bin/env node
"use strict";

const { execFileSync } = require("node:child_process");
const path = require("node:path");

const projectRootArg = process.argv[2];

if (!projectRootArg) {
  console.error("Usage: node tests/tools/agent-e2e-process-guard.js /abs/e2e/project-root");
  process.exit(2);
}

const projectRoot = path.resolve(projectRootArg);
const currentPid = process.pid;
const parentPid = process.ppid;
const psOutput = execFileSync("ps", ["-axo", "pid=,command="], { encoding: "utf8" });

const matches = psOutput
  .split("\n")
  .map((line) => {
    const trimmed = line.trim();
    const match = trimmed.match(/^(\d+)\s+(.+)$/);
    if (!match) return null;
    return { pid: Number(match[1]), command: match[2] };
  })
  .filter(Boolean)
  .filter((item) => item.pid !== currentPid && item.pid !== parentPid)
  .filter((item) => isAgentCommand(item.command))
  .map((item) => ({ ...item, cwd: processCwd(item.pid) }))
  .filter((item) => item.command.includes(projectRoot) || isInsideProjectRoot(item.cwd));

if (matches.length > 0) {
  console.error(`Refusing to start or resume agent E2E while ${matches.length} agent process(es) target this project root:`);
  for (const item of matches) {
    const cwd = item.cwd ? ` cwd=${item.cwd}` : "";
    console.error(`- pid ${item.pid}:${cwd} ${item.command}`);
  }
  console.error("Stop the stale process(es), then rerun this guard before starting a new E2E agent session.");
  process.exit(1);
}

console.log(`No Claude/Codex agent processes are targeting ${projectRoot}.`);

function processCwd(pid) {
  try {
    const output = execFileSync("lsof", ["-a", "-p", String(pid), "-d", "cwd", "-Fn"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    const line = output.split("\n").find((item) => item.startsWith("n"));
    return line ? line.slice(1) : null;
  } catch {
    return null;
  }
}

function isAgentCommand(command) {
  const firstToken = command.trim().split(/\s+/, 1)[0] ?? "";
  const executable = path.basename(firstToken);
  return executable === "claude" || executable === "codex";
}

function isInsideProjectRoot(candidate) {
  if (!candidate) return false;
  return candidate === projectRoot || candidate.startsWith(`${projectRoot}${path.sep}`);
}
