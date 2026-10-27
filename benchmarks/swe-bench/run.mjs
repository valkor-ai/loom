#!/usr/bin/env node

import { execFileSync, spawnSync } from "node:child_process";
import { appendFileSync, copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const BENCHMARK_ROOT = path.dirname(fileURLToPath(import.meta.url));
const REPOS_ROOT = path.join(BENCHMARK_ROOT, "repos");
const RUNS_ROOT = path.join(BENCHMARK_ROOT, "runs");
const DATASETS = {
  lite: {
    source: "princeton-nlp/SWE-bench_Lite",
    cacheName: "swe-bench-lite.json",
  },
  verified: {
    source: "princeton-nlp/SWE-bench_Verified",
    cacheName: "swe-bench-verified.json",
  },
};

function fail(message) {
  process.stderr.write(`swe-bench runner: ${message}\n`);
  process.exit(1);
}

function usage() {
  process.stdout.write(`Usage:
  node benchmarks/swe-bench/run.mjs prepare --instance <id> [--instance <id> ...] [--dataset lite|verified] [--run <id>] [--model <id>] [--provider <name>] [--reasoning <setting>] [--max-turns <n>] [--max-elapsed-ms <n>]
  node benchmarks/swe-bench/run.mjs record --run <id> --instance <id> --variant direct|loom --status <status> [--tokens <n>] [--turns <n>] [--repairs <n>] [--elapsed-ms <n>] [--note <text>]
  node benchmarks/swe-bench/run.mjs evaluate --run <id> --instance <id> --variant direct|loom --command <command> [--setup <command>] [--exclude-agent-path <path> ...]
  node benchmarks/swe-bench/run.mjs official --run <id> --instance <id> --variant direct|loom --status resolved|unresolved --report <path> [--note <text>]
  node benchmarks/swe-bench/run.mjs summarize --run <id>

The first prepare downloads and caches the selected SWE-bench dataset metadata under benchmarks/swe-bench/data/.
Codex is run manually in the generated direct/ and loom/ workspaces. The dataset test patch is applied only by evaluate; this is local evidence, not an official SWE-bench score.
`);
}

function parseArgs(values) {
  const options = { instance: [], "exclude-agent-path": [] };
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (!value.startsWith("--")) {
      fail(`unexpected argument: ${value}`);
    }
    const key = value.slice(2);
    const next = values[index + 1];
    if (!next || next.startsWith("--")) {
      fail(`${value} requires a value`);
    }
    if (key === "instance") {
      options.instance.push(next);
    } else if (key === "exclude-agent-path") {
      options["exclude-agent-path"].push(next);
    } else {
      options[key] = next;
    }
    index += 1;
  }
  return options;
}

function requireOption(options, name) {
  if (!options[name]) {
    fail(`--${name} is required`);
  }
  return options[name];
}

function command(program, args, cwd) {
  try {
    return execFileSync(program, args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  } catch (error) {
    const detail = [error.stdout, error.stderr].filter(Boolean).join("\n").trim();
    fail(`${program} ${args.join(" ")} failed${detail ? `:\n${detail}` : ""}`);
  }
}

function writeJson(target, value) {
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, `${JSON.stringify(value, null, 2)}\n`);
}

function readJson(target) {
  if (!existsSync(target)) {
    fail(`missing ${target}`);
  }
  return JSON.parse(readFileSync(target, "utf8"));
}

function safeName(value) {
  return value.replaceAll(/[^a-zA-Z0-9._-]/g, "_");
}

function timestamp() {
  return new Date().toISOString().replaceAll(/[-:.TZ]/g, "").slice(0, 14);
}

async function loadDataset(name = "lite") {
  const dataset = typeof name === "string" ? datasetFor(name) : name;
  const dataPath = path.join(BENCHMARK_ROOT, "data", dataset.cacheName);
  if (existsSync(dataPath)) {
    return readJson(dataPath).instances;
  }

  process.stdout.write(`Downloading ${dataset.source} metadata...\n`);
  const instances = [];
  let offset = 0;
  const pageSize = 100;
  while (true) {
    const url = `https://datasets-server.huggingface.co/rows?dataset=${encodeURIComponent(dataset.source)}&config=default&split=test&offset=${offset}&length=${pageSize}`;
    const response = await fetch(url);
    if (!response.ok) {
      fail(`dataset download failed: ${response.status} ${response.statusText}`);
    }
    const payload = await response.json();
    const rows = payload.rows ?? [];
    instances.push(...rows.map((entry) => entry.row));
    if (rows.length === 0 || instances.length >= payload.num_rows_total) {
      break;
    }
    offset += rows.length;
  }
  writeJson(dataPath, {
    source: dataset.source,
    downloadedAt: new Date().toISOString(),
    instances,
  });
  return instances;
}

function datasetFor(name = "lite") {
  const dataset = DATASETS[name];
  if (!dataset) {
    fail(`unknown dataset: ${name}; use ${Object.keys(DATASETS).join(" or ")}`);
  }
  return dataset;
}

function datasetForManifest(manifest) {
  return Object.values(DATASETS).find((dataset) => dataset.source === manifest.dataset)
    ?? fail(`unknown dataset in run manifest: ${manifest.dataset}`);
}

function publicTask(task) {
  return {
    instanceId: task.instance_id,
    repository: task.repo,
    baseCommit: task.base_commit,
    problemStatement: task.problem_statement,
    hintsText: task.hints_text,
    failToPass: task.FAIL_TO_PASS,
    passToPass: task.PASS_TO_PASS,
    environmentSetupCommit: task.environment_setup_commit,
    datasetVersion: task.version,
  };
}

function baseRepoPath(task) {
  return path.join(REPOS_ROOT, safeName(task.repo));
}

function ensureBaseRepo(task) {
  const repo = baseRepoPath(task);
  if (!existsSync(path.join(repo, ".git"))) {
    mkdirSync(REPOS_ROOT, { recursive: true });
    process.stdout.write(`Cloning ${task.repo}...\n`);
    command("git", ["clone", "https://github.com/" + task.repo + ".git", repo]);
  }
  const shallow = command("git", ["rev-parse", "--is-shallow-repository"], repo).trim() === "true";
  if (shallow) {
    process.stdout.write(`Fetching full history for ${task.repo}...\n`);
    command("git", ["fetch", "--unshallow", "--tags", "origin"], repo);
  } else {
    command("git", ["fetch", "--tags", "origin"], repo);
  }
  command("git", ["fetch", "origin", task.base_commit], repo);
  command("git", ["checkout", "--detach", task.base_commit], repo);
  command("git", ["reset", "--hard", task.base_commit], repo);
  command("git", ["clean", "-fdx"], repo);
  return repo;
}

function cloneWorkspace(source, destination) {
  rmSync(destination, { recursive: true, force: true });
  // Keep future upstream commits out of the agent workspace. The cached source
  // may have full history so the runner can locate base commits, but agents
  // must not be able to discover a later fix through local Git history.
  command("git", ["clone", "--no-local", "--depth", "1", pathToFileURL(source).href, destination]);
  command("git", ["checkout", "--detach", "HEAD"], destination);
  command("git", ["remote", "remove", "origin"], destination);
  appendFileSync(path.join(destination, ".git", "info", "exclude"), "\nPROMPT.md\nrun-agent.sh\n.benchmark-zsh/\n.venv/\n");
}

function writeAgentLauncher(workspace) {
  mkdirSync(path.join(workspace, ".benchmark-zsh"), { recursive: true });
  writeFileSync(path.join(workspace, "run-agent.sh"), `#!/bin/sh
set -eu

workspace=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export ZDOTDIR="$workspace/.benchmark-zsh"
export PYTHONNOUSERSITE=1

if [ -d "$workspace/.venv/bin" ]; then
  export VIRTUAL_ENV="$workspace/.venv"
  export PATH="$VIRTUAL_ENV/bin:$PATH"
fi

exec "$@"
`, { mode: 0o755 });
}

function directPrompt(task, dataset) {
  return `# ${dataset.source}: ${task.instance_id}\n\nResolve this issue in the current repository:\n\n> ${task.problem_statement.trim()}\n\nWork only in this repository. Do not read or apply any SWE-bench gold patch or test patch. Implement the fix, add or update focused regression coverage, and run the relevant checks. Finish only when the work is complete.\n`;
}

function loomPrompt(task, dataset) {
  return `# ${dataset.source}: ${task.instance_id}\n\nResolve this issue in the current repository:\n\n> ${task.problem_statement.trim()}\n\nUse Loom for the entire delivery. Start with \`@loom plan\` and follow every returned Loom MCP action until the delivery is terminal. You may make conservative, issue-grounded decisions at Loom confirmation gates; do not stop for intermediate approval. Work only in this repository. Do not read or apply any SWE-bench gold patch or test patch.\n\nImplement the fix, add or update focused regression coverage, run the relevant checks, and complete the Loom review, repair, verification, and handoff path. Do not finish with a progress update while Loom has an auto-runnable action.\n`;
}

async function prepare(options) {
  const instanceIds = options.instance;
  if (instanceIds.length === 0) {
    fail("prepare requires at least one --instance");
  }
  const runId = options.run ?? `run-${timestamp()}`;
  const runRoot = path.join(RUNS_ROOT, safeName(runId));
  if (existsSync(runRoot)) {
    fail(`run already exists: ${runRoot}`);
  }
  const datasetSpec = datasetFor(options.dataset);
  const dataset = await loadDataset(options.dataset);
  const tasks = instanceIds.map((instanceId) => {
    const task = dataset.find((candidate) => candidate.instance_id === instanceId);
    if (!task) {
      fail(`instance not found in ${datasetSpec.source}: ${instanceId}`);
    }
    return task;
  });

  const manifest = {
    schemaVersion: 2,
    runId,
    createdAt: new Date().toISOString(),
    dataset: datasetSpec.source,
    agentConfiguration: {
      model: options.model,
      provider: options.provider,
      reasoning: options.reasoning,
      maxTurns: numericOption(options, "max-turns"),
      maxElapsedMs: numericOption(options, "max-elapsed-ms"),
    },
    instances: tasks.map(publicTask),
  };
  writeJson(path.join(runRoot, "RUN.json"), manifest);
  for (const task of tasks) {
    const taskRoot = path.join(runRoot, task.instance_id);
    const source = ensureBaseRepo(task);
    for (const variant of ["direct", "loom"]) {
      const workspace = path.join(taskRoot, variant);
      cloneWorkspace(source, workspace);
      writeFileSync(path.join(workspace, "PROMPT.md"), variant === "loom" ? loomPrompt(task, datasetSpec) : directPrompt(task, datasetSpec));
      writeAgentLauncher(workspace);
    }
    writeJson(path.join(taskRoot, "RESULT.json"), {
      schemaVersion: 1,
      instance: publicTask(task),
      variants: Object.fromEntries(["direct", "loom"].map((variant) => [variant, {
        status: "prepared",
        workspace: variant,
      }])),
    });
  }
  process.stdout.write(`Prepared run: ${runRoot}\n`);
}

function taskRoot(options) {
  if (options.instance.length !== 1) {
    fail("record and evaluate require exactly one --instance");
  }
  return path.join(RUNS_ROOT, safeName(requireOption(options, "run")), options.instance[0]);
}

function resultFor(options) {
  const root = taskRoot(options);
  const resultPath = path.join(root, "RESULT.json");
  const result = readJson(resultPath);
  const variant = requireOption(options, "variant");
  if (!result.variants[variant]) {
    fail(`unknown variant: ${variant}`);
  }
  return { root, resultPath, result, variant };
}

function numericOption(options, name) {
  if (options[name] === undefined) {
    return undefined;
  }
  const value = Number(options[name]);
  if (!Number.isFinite(value) || value < 0) {
    fail(`--${name} must be a non-negative number`);
  }
  return value;
}

function record(options) {
  const { root, resultPath, result, variant } = resultFor(options);
  const workspace = path.join(root, variant);
  const baseCommit = result.instance.baseCommit;
  // Include agent-created files in the saved patch without staging their content.
  command("git", ["add", "--intent-to-add", "--", "."], workspace);
  const patch = command("git", ["diff", "--binary", baseCommit], workspace);
  const patchPath = path.join(root, "artifacts", `${variant}.patch`);
  mkdirSync(path.dirname(patchPath), { recursive: true });
  writeFileSync(patchPath, patch);
  result.variants[variant] = {
    ...result.variants[variant],
    status: requireOption(options, "status"),
    recordedAt: new Date().toISOString(),
    tokens: numericOption(options, "tokens"),
    turns: numericOption(options, "turns"),
    repairs: numericOption(options, "repairs"),
    elapsedMs: numericOption(options, "elapsed-ms"),
    note: options.note,
    patch: path.relative(root, patchPath),
    changedFiles: patch ? command("git", ["diff", "--name-only", baseCommit], workspace).trim().split("\n").filter(Boolean).length : 0,
  };
  writeJson(resultPath, result);
  process.stdout.write(`Recorded ${result.instance.instanceId} ${variant}.\n`);
}

function official(options) {
  const { root, resultPath, result, variant } = resultFor(options);
  const status = requireOption(options, "status");
  if (!new Set(["resolved", "unresolved"]).has(status)) {
    fail("--status must be resolved or unresolved");
  }
  const report = requireOption(options, "report");
  if (!existsSync(report)) {
    fail(`official report does not exist: ${report}`);
  }
  const reportPath = path.join(root, "artifacts", `${variant}.official-report.json`);
  mkdirSync(path.dirname(reportPath), { recursive: true });
  copyFileSync(report, reportPath);
  result.variants[variant] = {
    ...result.variants[variant],
    officialEvaluation: {
      status,
      recordedAt: new Date().toISOString(),
      report: path.relative(root, reportPath),
      note: options.note,
    },
  };
  writeJson(resultPath, result);
  process.stdout.write(`Recorded official evaluation for ${result.instance.instanceId} ${variant}.\n`);
}

async function evaluate(options) {
  const { root, resultPath, result, variant } = resultFor(options);
  const testCommand = requireOption(options, "command");
  const manifest = readJson(path.join(RUNS_ROOT, safeName(requireOption(options, "run")), "RUN.json"));
  const dataset = await loadDataset(datasetForManifest(manifest));
  const task = dataset.find((candidate) => candidate.instance_id === result.instance.instanceId);
  if (!task) {
    fail(`instance is no longer available in the dataset cache: ${result.instance.instanceId}`);
  }
  const fullPatch = path.join(root, result.variants[variant].patch ?? path.join("artifacts", `${variant}.patch`));
  if (!existsSync(fullPatch)) {
    fail(`missing recorded agent patch for ${variant}; run record first`);
  }
  const excludedPaths = options["exclude-agent-path"];
  const workspace = path.join(root, variant);
  let patch = fullPatch;
  let evaluationMode = "full-agent-patch";
  let filteredPatch;
  if (excludedPaths.length > 0) {
    if (!existsSync(path.join(workspace, ".git"))) {
      fail(`missing agent workspace for ${variant}; cannot create a filtered patch`);
    }
    const pathspecs = [".", ...excludedPaths.map((value) => `:(exclude)${value}`)];
    const filtered = command("git", ["diff", "--binary", result.instance.baseCommit, "--", ...pathspecs], workspace);
    filteredPatch = path.join(root, "artifacts", `${variant}.filtered.patch`);
    writeFileSync(filteredPatch, filtered);
    patch = filteredPatch;
    evaluationMode = "filtered-agent-patch";
  }
  const evaluationRoot = path.join(root, "evaluation", variant);
  const source = ensureBaseRepo(task);
  cloneWorkspace(source, evaluationRoot);
  const testPatch = path.join(root, "evaluation", "official-test.patch");
  writeFileSync(testPatch, task.test_patch);

  const logPath = path.join(root, "evaluation", `${variant}.log`);
  const runShellCommand = (value) => spawnSync(value, {
    cwd: evaluationRoot,
    shell: true,
    encoding: "utf8",
    maxBuffer: 20 * 1024 * 1024,
  });
  const applyPatch = (patchPath) => spawnSync("git", ["apply", "--whitespace=nowarn", patchPath], {
    cwd: evaluationRoot,
    encoding: "utf8",
  });
  const recordPatchFailure = (stage, output) => {
    writeFileSync(logPath, `$ git apply --whitespace=nowarn ${stage === "agent_patch" ? patch : testPatch}\n${output.stdout ?? ""}${output.stderr ?? ""}`);
    result.variants[variant].evaluation = {
      completedAt: new Date().toISOString(),
      passed: false,
      failedAt: stage,
      command: testCommand,
      exitCode: output.status,
      signal: output.signal,
      log: path.relative(root, logPath),
      officialTestPatch: path.relative(root, testPatch),
      mode: evaluationMode,
      agentPatch: path.relative(root, patch),
      excludedAgentPaths: excludedPaths.length > 0 ? excludedPaths : undefined,
    };
    writeJson(resultPath, result);
    process.stdout.write(`Evaluation failed while applying ${stage === "agent_patch" ? "the agent patch" : "the official test patch"}: ${result.instance.instanceId} ${variant}\n`);
    process.exitCode = 1;
  };
  if (evaluationMode === "filtered-agent-patch") {
    const officialTestPatch = applyPatch(testPatch);
    if (officialTestPatch.status !== 0) {
      recordPatchFailure("official_test_patch", officialTestPatch);
      return;
    }
    const agentPatch = applyPatch(patch);
    if (agentPatch.status !== 0) {
      recordPatchFailure("agent_patch", agentPatch);
      return;
    }
  } else {
    const agentPatch = applyPatch(patch);
    if (agentPatch.status !== 0) {
      recordPatchFailure("agent_patch", agentPatch);
      return;
    }
    const officialTestPatch = applyPatch(testPatch);
    if (officialTestPatch.status !== 0) {
      recordPatchFailure("official_test_patch", officialTestPatch);
      return;
    }
  }
  const startedAt = Date.now();
  const setup = options.setup ? runShellCommand(options.setup) : undefined;
  const output = setup && setup.status !== 0 ? setup : runShellCommand(testCommand);
  const elapsedMs = Date.now() - startedAt;
  writeFileSync(logPath, [
    options.setup ? `$ ${options.setup}\n${setup.stdout ?? ""}${setup.stderr ?? ""}` : "",
    `$ ${testCommand}\n${output === setup ? "not run because setup failed" : `${output.stdout ?? ""}${output.stderr ?? ""}`}`,
  ].filter(Boolean).join("\n\n"));
  result.variants[variant].evaluation = {
    completedAt: new Date().toISOString(),
    setup: options.setup ? {
      command: options.setup,
      exitCode: setup.status,
      signal: setup.signal,
    } : undefined,
    command: testCommand,
    exitCode: output.status,
    signal: output.signal,
    elapsedMs,
    passed: output.status === 0,
    log: path.relative(root, logPath),
    officialTestPatch: path.relative(root, testPatch),
    mode: evaluationMode,
    agentPatch: path.relative(root, patch),
    excludedAgentPaths: excludedPaths.length > 0 ? excludedPaths : undefined,
  };
  writeJson(resultPath, result);
  process.stdout.write(`Evaluation ${output.status === 0 ? "passed" : "failed"}: ${result.instance.instanceId} ${variant}\n`);
  process.exitCode = output.status === 0 ? 0 : 1;
}

function summarize(options) {
  const run = safeName(requireOption(options, "run"));
  const runRoot = path.join(RUNS_ROOT, run);
  const manifest = readJson(path.join(runRoot, "RUN.json"));
  const lines = [
    `# ${manifest.dataset} Run`,
    "",
    `Run: \`${manifest.runId}\``,
    "",
    "| Instance | Variant | Status | Official evaluation | Local evaluation | Tokens | Turns | Repairs | Elapsed ms |",
    "| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |",
  ];
  for (const instance of manifest.instances) {
    const result = readJson(path.join(runRoot, instance.instanceId, "RESULT.json"));
    for (const variant of ["direct", "loom"]) {
      const item = result.variants[variant];
      const officialEvaluation = item.officialEvaluation?.status ?? "not recorded";
      const evaluation = item.evaluation
        ? (item.evaluation.passed ? (item.evaluation.mode === "filtered-agent-patch" ? "passed (filtered)" : "passed") : "failed")
        : "not run";
      lines.push(`| ${instance.instanceId} | ${variant} | ${item.status} | ${officialEvaluation} | ${evaluation} | ${item.tokens ?? ""} | ${item.turns ?? ""} | ${item.repairs ?? ""} | ${item.elapsedMs ?? ""} |`);
    }
  }
  const output = `${lines.join("\n")}\n`;
  writeFileSync(path.join(runRoot, "SUMMARY.md"), output);
  process.stdout.write(output);
}

const [action, ...rest] = process.argv.slice(2);
if (!action || action === "--help" || action === "-h") {
  usage();
  process.exit(0);
}
const options = parseArgs(rest);
if (action === "prepare") {
  await prepare(options);
} else if (action === "record") {
  record(options);
} else if (action === "evaluate") {
  await evaluate(options);
} else if (action === "official") {
  official(options);
} else if (action === "summarize") {
  summarize(options);
} else {
  fail(`unknown command: ${action}`);
}
