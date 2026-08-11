#!/usr/bin/env node

import { spawn } from "node:child_process";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = resolve(repoRoot, "src/rust/Cargo.toml");
const cargo = process.env.CARGO ?? "cargo";
const children = new Set();
let interrupted = false;

const releaseTargets = new Map([
  ["release_unit", []],
  ["release_deploy", ["--test-threads=1"]],
  ["release_mcp", ["--test-threads=1"]],
  ["release_workflows", ["--test-threads=1"]],
]);

process.on("SIGINT", () => {
  interrupted = true;
  for (const child of children) {
    child.kill("SIGINT");
  }
});

const compileResult = await runProcess(cargo, [
  "test",
  "--manifest-path",
  manifest,
  "--workspace",
  "--lib",
  "--bins",
  "--no-run",
], { label: "workspace unit and binary compilation" });
printResult(compileResult);
if (compileResult.code !== 0 || interrupted) {
  process.exit(interrupted ? 130 : 1);
}

const integrationArtifacts = await compileReleaseTargets();
const missingTargets = [...releaseTargets.keys()].filter(
  (name) => !integrationArtifacts.some((artifact) => artifact.name === name),
);
if (missingTargets.length > 0) {
  console.error(`Missing release test artifacts: ${missingTargets.join(", ")}`);
  process.exit(1);
}

const integrationResults = [];
const targetBatches = [
  ["release_deploy", "release_mcp"],
  ["release_unit", "release_workflows"],
];
for (const batch of targetBatches) {
  if (interrupted) {
    break;
  }
  const results = process.env.LOOM_RUST_TEST_SERIAL === "1"
    ? await runTargetBatchSerial(batch, integrationArtifacts)
    : await Promise.all(batch.map((name) => runTarget(name, integrationArtifacts)));
  integrationResults.push(...results);
  for (const result of results) {
    printResult(result);
  }
  if (results.some((result) => result.code !== 0)) {
    process.exit(1);
  }
}
if (interrupted) {
  process.exit(130);
}

console.log(
  `Rust release validation passed: workspace compilation and ${integrationResults.length} grouped integration targets. Run npm run rust:test:full for the complete per-crate unit, integration, and doctest matrix.`,
);

async function compileReleaseTargets() {
  const result = await runProcess(cargo, [
    "test",
    "--manifest-path",
    manifest,
    "-p",
    "release-tests",
    "--tests",
    "--no-run",
    "--message-format=json-render-diagnostics",
  ], { label: "grouped integration test compilation", parseArtifacts: true });
  if (result.code !== 0) {
    printProcessFailure(result.label, result);
    process.exit(1);
  }
  return result.artifacts;
}

async function runTarget(name, artifacts) {
  const artifact = artifacts.find((candidate) => candidate.name === name);
  return runProcess(artifact.executable, releaseTargets.get(name), { label: name });
}

async function runTargetBatchSerial(names, artifacts) {
  const results = [];
  for (const name of names) {
    results.push(await runTarget(name, artifacts));
  }
  return results;
}

function runProcess(command, args, options = {}) {
  return new Promise((resolveResult) => {
    const startedAt = Date.now();
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    children.add(child);
    let stdout = "";
    let parseBuffer = "";
    let stderr = "";
    let spawnError;
    const artifacts = [];
    const seenArtifacts = new Set();

    function recordArtifact(line) {
      if (!line.trim()) {
        return;
      }
      try {
        const message = JSON.parse(line);
        if (
          message.reason === "compiler-artifact" &&
          message.profile?.test === true &&
          message.executable &&
          !seenArtifacts.has(message.executable)
        ) {
          seenArtifacts.add(message.executable);
          artifacts.push({
            executable: message.executable,
            name: message.target?.name ?? basename(message.executable),
          });
        }
      } catch {
        process.stderr.write(`${line}\n`);
      }
    }

    child.stdout.on("data", (chunk) => {
      const text = chunk.toString();
      stdout += text;
      if (!options.parseArtifacts) {
        process.stdout.write(text);
        return;
      }
      parseBuffer += text;
      const lines = parseBuffer.split("\n");
      parseBuffer = lines.pop() ?? "";
      for (const line of lines) {
        recordArtifact(line);
      }
    });
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString();
      stderr += text;
      if (!options.parseArtifacts) {
        process.stderr.write(text);
      }
    });
    child.on("error", (error) => {
      spawnError = error;
    });
    child.on("close", (code, signal) => {
      children.delete(child);
      if (options.parseArtifacts) {
        recordArtifact(parseBuffer);
      }
      resolveResult({
        label: options.label,
        code: spawnError ? 1 : code ?? 1,
        signal,
        stdout,
        stderr,
        artifacts,
        elapsedMs: Date.now() - startedAt,
        error: spawnError,
      });
    });
  });
}

function printResult(result) {
  const status = result.code === 0 ? "PASS" : "FAIL";
  const duration = `${(result.elapsedMs / 1000).toFixed(2)}s`;
  console.log(`${status} ${result.label} (${duration})`);
  if (result.code !== 0) {
    printProcessFailure(result.label, result);
  }
}

function printProcessFailure(label, result) {
  console.error(`\n${label} failed${result.signal ? ` with ${result.signal}` : ""}.`);
  if (result.error) {
    console.error(result.error.message);
  }
  if (result.stdout) {
    console.error(result.stdout);
  }
  if (result.stderr) {
    console.error(result.stderr);
  }
}
