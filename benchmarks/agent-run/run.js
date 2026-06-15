#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const BENCH_DIR = __dirname;
const LOOM_ROOT = path.resolve(BENCH_DIR, "../..");
const CASES_DIR = path.join(BENCH_DIR, "cases");
const CLI_PATH = path.join(LOOM_ROOT, "dist", "cli.js");

main();

function main() {
  const { command, options } = parseArgs(process.argv.slice(2));
  if (command === "prepare") {
    prepareRuns(options);
  } else if (command === "record") {
    recordResult(options);
  } else if (command === "summarize") {
    summarizeResults(options);
  } else {
    printHelp();
    process.exit(command === "help" ? 0 : 1);
  }
}

function parseArgs(args) {
  const command = args[0] && !args[0].startsWith("--") ? args[0] : "help";
  const rest = command === "help" ? args : args.slice(1);
  const options = {
    caseIds: [],
    skipBuild: false,
    outDir: "",
    runDir: "",
    runDirs: [],
    variantDir: "",
    markdownOut: "",
    json: false,
    repeat: 1,
    agentProfile: "codex",
    status: "",
    turns: null,
    repairLoops: null,
    tests: "",
    verificationCommand: "",
    tokensUsed: null,
    verificationStatus: "",
    changedFiles: [],
    successCriteriaMet: null,
    successCriteriaTotal: null,
    compactReads: [],
    rawEvidenceOpened: [],
    flowStep: "",
    notes: "",
    agent: "codex",
  };

  for (let index = 0; index < rest.length; index += 1) {
    const arg = rest[index];
    if (arg === "--case") {
      options.caseIds.push(requireValue(rest, ++index, "--case"));
    } else if (arg === "--skip-build") {
      options.skipBuild = true;
    } else if (arg === "--out-dir") {
      options.outDir = path.resolve(requireValue(rest, ++index, "--out-dir"));
    } else if (arg === "--run-dir") {
      options.runDir = path.resolve(requireValue(rest, ++index, "--run-dir"));
      options.runDirs.push(options.runDir);
    } else if (arg === "--variant-dir") {
      options.variantDir = path.resolve(requireValue(rest, ++index, "--variant-dir"));
    } else if (arg === "--markdown-out") {
      options.markdownOut = path.resolve(requireValue(rest, ++index, "--markdown-out"));
    } else if (arg === "--json") {
      options.json = true;
    } else if (arg === "--repeat") {
      options.repeat = parsePositiveInteger(requireValue(rest, ++index, "--repeat"), "--repeat");
    } else if (arg === "--agent-profile") {
      options.agentProfile = requireValue(rest, ++index, "--agent-profile");
    } else if (arg === "--agent") {
      options.agent = requireValue(rest, ++index, "--agent");
    } else if (arg === "--status") {
      options.status = requireValue(rest, ++index, "--status");
    } else if (arg === "--turns") {
      options.turns = parseNonNegativeInteger(requireValue(rest, ++index, "--turns"), "--turns");
    } else if (arg === "--repair-loops") {
      options.repairLoops = parseNonNegativeInteger(requireValue(rest, ++index, "--repair-loops"), "--repair-loops");
    } else if (arg === "--tests") {
      options.tests = requireValue(rest, ++index, "--tests");
    } else if (arg === "--verification-command") {
      options.verificationCommand = requireValue(rest, ++index, "--verification-command");
    } else if (arg === "--tokens-used") {
      options.tokensUsed = parseNonNegativeInteger(requireValue(rest, ++index, "--tokens-used"), "--tokens-used");
    } else if (arg === "--verification-status") {
      options.verificationStatus = requireValue(rest, ++index, "--verification-status");
    } else if (arg === "--changed-file") {
      options.changedFiles.push(requireValue(rest, ++index, "--changed-file"));
    } else if (arg === "--success-criteria-met") {
      options.successCriteriaMet = parseNonNegativeInteger(requireValue(rest, ++index, "--success-criteria-met"), "--success-criteria-met");
    } else if (arg === "--success-criteria-total") {
      options.successCriteriaTotal = parseNonNegativeInteger(requireValue(rest, ++index, "--success-criteria-total"), "--success-criteria-total");
    } else if (arg === "--compact-read") {
      options.compactReads.push(requireValue(rest, ++index, "--compact-read"));
    } else if (arg === "--raw-evidence-opened") {
      options.rawEvidenceOpened.push(requireValue(rest, ++index, "--raw-evidence-opened"));
    } else if (arg === "--flow-step") {
      options.flowStep = requireValue(rest, ++index, "--flow-step");
    } else if (arg === "--notes") {
      options.notes = requireValue(rest, ++index, "--notes");
    } else if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return { command, options };
}

function printHelp() {
  process.stdout.write(`Agent Run Benchmark

Usage:
  node benchmarks/agent-run/run.js prepare [options]
  node benchmarks/agent-run/run.js record --variant-dir <path> --status <status> [options]
  node benchmarks/agent-run/run.js summarize --run-dir <path> [options]

Prepare options:
  --case <id>              Prepare one case. May be repeated.
  --skip-build             Use the existing dist/cli.js.
  --out-dir <path>         Parent output directory. Default: /tmp/loom-agent-run-benchmark.
  --repeat <n>             Prepare N paired attempts for each case. Default: 1.
  --agent-profile <id>     LOOM_AGENT_PROFILE for Loom plan creation. Default: codex.

Record options:
  --variant-dir <path>     Variant directory created by prepare.
  --agent <name>           Agent name. Default: codex.
  --status <status>        passed | failed | partial | blocked
  --turns <n>              Agent/user turns counted for the run.
  --repair-loops <n>       Repair loops or retries needed.
  --tests <status>         passed | failed | not_run | partial
  --verification-command <cmd>
  --verification-status <status>
  --tokens-used <n>        Total tokens reported by the agent surface, if available.
  --changed-file <path>    Changed file path, relative to the variant dir. May be repeated.
  --success-criteria-met <n>
  --success-criteria-total <n>
  --compact-read <label>   Compact Loom read used, e.g. loom-plan.fields. May be repeated.
  --raw-evidence-opened <path>
  --flow-step <label>      Flow state reached, e.g. brainstorm_prompt_confirmed.
  --notes <text>

Summarize options:
  --run-dir <path>         Prepared run directory. May be repeated for aggregate summaries.
  --markdown-out <path>    Write a Markdown summary table.
  --json                   Print JSON.
`);
}

function prepareRuns(options) {
  if (!options.skipBuild) {
    run("npm", ["run", "build"], { cwd: LOOM_ROOT, label: "build" });
  }

  const cases = loadCases(options.caseIds);
  const outDir = options.outDir || path.join(os.tmpdir(), "loom-agent-run-benchmark");
  const runId = `run-${timestamp()}`;
  const runDir = path.join(outDir, runId);
  fs.mkdirSync(runDir, { recursive: true });

  const preparedCases = [];
  for (let attempt = 1; attempt <= options.repeat; attempt += 1) {
    for (const benchCase of cases) {
      preparedCases.push(prepareCase(runDir, benchCase, options, attempt, options.repeat));
    }
  }
  const manifest = {
    schemaVersion: "1.0",
    benchmark: "agent-run",
    runId,
    createdAt: new Date().toISOString(),
    runDir,
    repeat: options.repeat,
    cases: preparedCases,
  };
  writeJson(path.join(runDir, "manifest.json"), manifest);

  if (options.json) {
    process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
    return;
  }
  process.stdout.write(`Prepared agent-run benchmark: ${runDir}\n\n`);
  for (const preparedCase of preparedCases) {
    for (const variant of preparedCase.variants) {
      process.stdout.write(`${preparedCase.id} ${variant.variant}: ${variant.promptPath}\n`);
    }
  }
}

function prepareCase(runDir, benchCase, options, attempt, repeat) {
  const caseDir = repeat > 1
    ? path.join(runDir, "cases", benchCase.id, `attempt-${attemptLabel(attempt)}`)
    : path.join(runDir, "cases", benchCase.id);
  const direct = prepareVariant(caseDir, benchCase, "direct", options, attempt, repeat);
  const loom = prepareVariant(caseDir, benchCase, "loom", options, attempt, repeat);
  return {
    id: benchCase.id,
    title: benchCase.title,
    attempt,
    variants: [direct, loom],
  };
}

function prepareVariant(caseDir, benchCase, variant, options, attempt, repeat) {
  const variantDir = path.join(caseDir, variant);
  const workspaceDir = path.join(variantDir, "workspace");
  fs.mkdirSync(workspaceDir, { recursive: true });
  writeSeedFiles(workspaceDir, benchCase.seedFiles || {});
  if (benchCase.seedDir) {
    copySeedDir(path.resolve(benchCase.caseDir, benchCase.seedDir), workspaceDir);
  }

  let loomPlan = null;
  if (variant === "loom") {
    const plan = runLoom([
      "plan",
      "--project-root",
      workspaceDir,
      "--json",
      "--compact",
      "--request",
      benchCase.request,
      ...contextArgs(benchCase.context),
    ], options.agentProfile);
    loomPlan = plan.envelope;
    writeJson(path.join(variantDir, "loom-plan.json"), loomPlan);
  }

  const prompt = variant === "loom"
    ? loomPrompt(benchCase, variantDir, workspaceDir, loomPlan)
    : directPrompt(benchCase, variantDir, workspaceDir);
  const resultTemplate = resultTemplateFor(benchCase, variant, workspaceDir, attempt);
  const metadata = {
    schemaVersion: "1.0",
    caseId: benchCase.id,
    title: benchCase.title,
    variant,
    attempt,
    repeat,
    workspaceDir,
    promptPath: path.join(variantDir, "PROMPT.md"),
    resultPath: path.join(variantDir, "BENCHMARK_RESULT.json"),
    resultTemplatePath: path.join(variantDir, "RESULT_TEMPLATE.json"),
    verificationCommand: benchCase.verificationCommand || null,
    successCriteria: benchCase.successCriteria || [],
    ...(loomPlan ? {
      loom: {
        requestRef: loomPlan.data?.requestPath || loomPlan.instruction?.requestRef || null,
        planPath: path.join(variantDir, "loom-plan.json"),
      },
    } : {}),
  };

  fs.writeFileSync(path.join(variantDir, "PROMPT.md"), prompt, "utf8");
  writeJson(path.join(variantDir, "RESULT_TEMPLATE.json"), resultTemplate);
  writeJson(path.join(variantDir, "metadata.json"), metadata);
  return {
    variant,
    attempt,
    variantDir,
    workspaceDir,
    promptPath: metadata.promptPath,
    resultPath: metadata.resultPath,
    ...(metadata.loom ? { loom: metadata.loom } : {}),
  };
}

function recordResult(options) {
  if (!options.variantDir) {
    throw new Error("record requires --variant-dir.");
  }
  if (!["passed", "failed", "partial", "blocked"].includes(options.status)) {
    throw new Error("record requires --status passed|failed|partial|blocked.");
  }
  const metadata = readJson(path.join(options.variantDir, "metadata.json"));
  const successCriteriaTotal = options.successCriteriaTotal ?? metadata.successCriteria?.length ?? null;
  const successCriteriaMet = options.successCriteriaMet;
  if (successCriteriaMet !== null && successCriteriaTotal !== null && successCriteriaMet > successCriteriaTotal) {
    throw new Error("--success-criteria-met cannot be greater than --success-criteria-total.");
  }
  const result = {
    schemaVersion: "1.0",
    caseId: metadata.caseId,
    title: metadata.title,
    variant: metadata.variant,
    attempt: metadata.attempt ?? null,
    agent: options.agent,
    status: options.status,
    turns: options.turns,
    repairLoops: options.repairLoops,
    tests: options.tests || "not_recorded",
    verification: {
      command: options.verificationCommand || metadata.verificationCommand || null,
      status: options.verificationStatus || "not_recorded",
    },
    tokenUsage: {
      total: options.tokensUsed,
      source: options.tokensUsed === null ? "not_recorded" : "agent_surface_report",
    },
    completion: {
      successCriteriaMet,
      successCriteriaTotal,
      scorePct: successCriteriaMet !== null && successCriteriaTotal > 0
        ? round(successCriteriaMet / successCriteriaTotal * 100, 1)
        : null,
      verificationPassed: options.verificationStatus === "passed",
      flowStep: options.flowStep || null,
    },
    readPolicy: {
      compactReads: options.compactReads,
      rawEvidenceOpened: options.rawEvidenceOpened,
      rawEvidenceCount: options.rawEvidenceOpened.length,
    },
    changedFiles: options.changedFiles,
    workspaceDir: metadata.workspaceDir,
    notes: options.notes || "",
    completedAt: new Date().toISOString(),
  };
  const resultPath = path.join(options.variantDir, "BENCHMARK_RESULT.json");
  writeJson(resultPath, result);
  process.stdout.write(`Recorded result: ${resultPath}\n`);
}

function summarizeResults(options) {
  const runDirs = options.runDirs.length > 0 ? options.runDirs : (options.runDir ? [options.runDir] : []);
  if (runDirs.length === 0) {
    throw new Error("summarize requires --run-dir.");
  }
  const results = runDirs
    .flatMap((runDir) => findFiles(runDir, "BENCHMARK_RESULT.json")
      .map((filePath) => {
        const result = readJson(filePath);
        return {
          runDir,
          filePath,
          result: enrichMissingTokenUsage(runDir, result),
        };
      }))
    .sort((left, right) => resultSortKey(left.result).localeCompare(resultSortKey(right.result)));
  const comparisons = pairedComparisons(results.map((item) => item.result));
  const summary = {
    schemaVersion: "1.0",
    benchmark: "agent-run",
    runDir: runDirs.length === 1 ? runDirs[0] : null,
    runDirs,
    resultCount: results.length,
    results,
    comparisons,
    aggregates: aggregateComparisons(comparisons),
  };
  if (options.markdownOut) {
    fs.mkdirSync(path.dirname(options.markdownOut), { recursive: true });
    fs.writeFileSync(options.markdownOut, renderMarkdown(summary), "utf8");
  }
  if (options.json) {
    process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
    return;
  }
  printSummary(summary);
  printComparisons(summary.comparisons);
  printAggregates(summary.aggregates);
  if (options.markdownOut) {
    process.stdout.write(`\nWrote Markdown summary to ${options.markdownOut}\n`);
  }
}

function directPrompt(benchCase, variantDir, workspaceDir) {
  return `${promptHeader(benchCase, workspaceDir)}

## Mode

Run this as a direct coding-agent delivery. Do not use Loom for this variant.

## Request

${benchCase.request}

${contextBlock(benchCase)}
${successCriteriaBlock(benchCase)}
${verificationBlock(benchCase)}

## Required Closeout

${closeoutBlock("direct", variantDir, benchCase)}
`;
}

function loomPrompt(benchCase, variantDir, workspaceDir, loomPlan) {
  const requestRef = loomPlan?.data?.requestPath || loomPlan?.instruction?.requestRef || "<requestRef>";
  const inspectPrefix = `LOOM_AGENT_PROFILE=codex LOOM_COMPACT_OUTPUT=1 node ${CLI_PATH} inspect --project-root ${workspaceDir} --request ${requestRef} --values-only`;
  return `${promptHeader(benchCase, workspaceDir)}

## Mode

Run this as the \`.loom\` artifact-guided variant. Use only the local \`.loom\` artifacts in this workspace; do not load plugin skills or home-directory delivery launchers.

## Local \`.loom\` Request

- requestRef: \`${requestRef}\`
- Start with the local inspect command template for compact request context:

\`${inspectPrefix} --field originalRequest,requirementContext.normalizedText\`

For more request fields, reuse the command and change only \`--field\`. Do not read \`loom-plan.json\`, the whole \`agentAction\`, \`rules\`, \`outputContract\`, or full \`.loom\` artifacts unless a specific missing field blocks implementation.

Prefer focused source files, tests, and benchmark verifiers before raw logs or bulky evidence.

For this isolated benchmark run, the Brainstorm gate is confirmed. Preserve that flow state, keep compact inspect output as the request authority, and record \`completion.flowStep\` as \`brainstorm_prompt_confirmed\`.

## Acceptance

Use the workspace verifier as the acceptance source. Run \`${benchCase.verificationCommand || "the required verification command"}\` before recording a passing result.

## Required Closeout

${closeoutBlock("loom", variantDir, benchCase)}
`;
}

function closeoutBlock(variant, variantDir, benchCase) {
  const flowStep = variant === "loom" ? "brainstorm_prompt_confirmed" : "direct_delivery";
  const successCriteriaTotal = Array.isArray(benchCase.successCriteria) ? benchCase.successCriteria.length : 0;
  const verificationCommand = benchCase.verificationCommand || "";
  const recordArgs = [
    process.execPath,
    path.join(BENCH_DIR, "run.js"),
    "record",
    "--variant-dir",
    variantDir,
    "--status",
    "passed",
    "--repair-loops",
    "0",
    "--tests",
    "passed",
    "--verification-command",
    verificationCommand,
    "--verification-status",
    "passed",
    "--success-criteria-met",
    String(successCriteriaTotal),
    "--success-criteria-total",
    String(successCriteriaTotal),
    "--flow-step",
    flowStep,
    "--notes",
    "verification passed",
  ];
  return `After the required verification command passes, record the result with:

\`${recordArgs.map(shellQuote).join(" ")}\`

The record command writes \`${path.join(variantDir, "BENCHMARK_RESULT.json")}\`. Do not open \`RESULT_TEMPLATE.json\`, hand-write result JSON, search for result paths, run repeated readbacks, \`git diff\`, \`nl\`, or broad sanity scans after verification passes. Keep the final response to two short sentences and do not include diffs or file excerpts.`;
}

function promptHeader(benchCase, workspaceDir) {
  return `# Agent Run Benchmark: ${benchCase.title}

Workspace: \`${workspaceDir}\`
Case: \`${benchCase.id}\`
`;
}

function contextBlock(benchCase) {
  if (!Array.isArray(benchCase.context) || benchCase.context.length === 0) {
    return "";
  }
  return `## Context

${benchCase.context.map((item, index) => `${index + 1}. ${item}`).join("\n")}
`;
}

function successCriteriaBlock(benchCase) {
  if (!Array.isArray(benchCase.successCriteria) || benchCase.successCriteria.length === 0) {
    return "";
  }
  return `## Success Criteria

${benchCase.successCriteria.map((item, index) => `${index + 1}. ${item}`).join("\n")}
`;
}

function verificationBlock(benchCase) {
  if (!benchCase.verificationCommand) {
    return "";
  }
  return `## Verification

Run \`${benchCase.verificationCommand}\` from the workspace before recording a passing result.
`;
}

function resultTemplateFor(benchCase, variant, workspaceDir, attempt) {
  return {
    schemaVersion: "1.0",
    caseId: benchCase.id,
    title: benchCase.title,
    variant,
    attempt,
    agent: "codex",
    status: "passed|failed|partial|blocked",
    turns: null,
    repairLoops: null,
    tests: "passed|failed|partial|not_run",
    verification: {
      command: benchCase.verificationCommand || null,
      status: "passed|failed|partial|not_run",
    },
    tokenUsage: {
      total: null,
      source: "agent_surface_report|manual|not_recorded",
    },
    completion: {
      successCriteriaMet: null,
      successCriteriaTotal: Array.isArray(benchCase.successCriteria) ? benchCase.successCriteria.length : null,
      scorePct: null,
      verificationPassed: null,
      flowStep: variant === "loom" ? "brainstorm_prompt_confirmed|not_observed|blocked" : "direct_delivery",
    },
    readPolicy: {
      compactReads: [],
      rawEvidenceOpened: [],
      rawEvidenceCount: 0,
    },
    changedFiles: [],
    workspaceDir,
    notes: "",
    completedAt: null,
  };
}

function loadCases(caseIds) {
  const wanted = new Set(caseIds);
  const files = fs.readdirSync(CASES_DIR)
    .filter((file) => file.endsWith(".json"))
    .sort();
  const cases = files.map((file) => {
    const benchCase = readJson(path.join(CASES_DIR, file));
    if (!benchCase.id || !benchCase.title || !benchCase.request) {
      throw new Error(`Invalid benchmark case: ${file}`);
    }
    return {
      ...benchCase,
      caseDir: path.dirname(path.join(CASES_DIR, file)),
    };
  });
  const selected = wanted.size > 0 ? cases.filter((benchCase) => wanted.has(benchCase.id)) : cases;
  const missing = [...wanted].filter((id) => !cases.some((benchCase) => benchCase.id === id));
  if (missing.length > 0) {
    throw new Error(`Unknown benchmark case(s): ${missing.join(", ")}`);
  }
  return selected;
}

function writeSeedFiles(root, seedFiles) {
  for (const [relativePath, content] of Object.entries(seedFiles)) {
    const filePath = path.join(root, relativePath);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, content, "utf8");
  }
}

function copySeedDir(sourceDir, targetDir) {
  if (!fs.existsSync(sourceDir)) {
    throw new Error(`Missing seedDir: ${sourceDir}`);
  }
  for (const entry of fs.readdirSync(sourceDir, { withFileTypes: true })) {
    const sourcePath = path.join(sourceDir, entry.name);
    const targetPath = path.join(targetDir, entry.name);
    if (entry.isDirectory()) {
      fs.mkdirSync(targetPath, { recursive: true });
      copySeedDir(sourcePath, targetPath);
    } else if (entry.isFile()) {
      fs.mkdirSync(path.dirname(targetPath), { recursive: true });
      fs.copyFileSync(sourcePath, targetPath);
    }
  }
}

function contextArgs(context) {
  if (!Array.isArray(context)) return [];
  return context.flatMap((item) => ["--context", item]);
}

function runLoom(args, agentProfile) {
  const result = run(process.execPath, [CLI_PATH, ...args], {
    cwd: LOOM_ROOT,
    label: `loom ${args[0]}`,
    env: {
      ...process.env,
      LOOM_AGENT_PROFILE: agentProfile,
      LOOM_COMPACT_OUTPUT: "1",
    },
  });
  return {
    stdout: result.stdout,
    envelope: JSON.parse(result.stdout),
  };
}

function run(command, args, options) {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env || process.env,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`${options.label} failed with exit ${result.status}\n${result.stdout || ""}${result.stderr || ""}`);
  }
  return {
    stdout: result.stdout,
    stderr: result.stderr,
  };
}

function findFiles(root, filename) {
  if (!fs.existsSync(root)) {
    return [];
  }
  const output = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const filePath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      output.push(...findFiles(filePath, filename));
    } else if (entry.isFile() && entry.name === filename) {
      output.push(filePath);
    }
  }
  return output;
}

function printSummary(summary) {
  const rows = summary.results.map(({ result }) => ({
    Case: result.caseId,
    Attempt: result.attempt ?? "",
    Variant: result.variant,
    Status: result.status,
    Turns: result.turns ?? "",
    Repairs: result.repairLoops ?? "",
    Tests: result.tests ?? "",
    Completion: completionLabel(result),
    Tokens: result.tokenUsage?.total ?? "",
    "Raw Evidence": result.readPolicy?.rawEvidenceCount ?? "",
  }));
  printTable(rows);
}

function renderMarkdown(summary) {
  const headers = ["Case", "Attempt", "Variant", "Status", "Turns", "Repairs", "Tests", "Completion", "Tokens", "Raw Evidence", "Notes"];
  const rows = summary.results.map(({ result }) => [
    result.caseId,
    result.attempt ?? "",
    result.variant,
    result.status,
    result.turns ?? "",
    result.repairLoops ?? "",
    result.tests ?? "",
    completionLabel(result),
    result.tokenUsage?.total ?? "",
    result.readPolicy?.rawEvidenceCount ?? "",
    (result.notes || "").replace(/\|/g, "\\|"),
  ]);
  return [
    "# Loom Agent Run Benchmark",
    "",
    "| " + headers.join(" | ") + " |",
    "| " + headers.map(() => "---").join(" | ") + " |",
    ...rows.map((row) => "| " + row.join(" | ") + " |"),
    "",
    ...renderComparisonMarkdown(summary.comparisons),
    ...renderAggregateMarkdown(summary.aggregates),
  ].join("\n");
}

function printTable(rows) {
  if (rows.length === 0) {
    process.stdout.write("No recorded benchmark results found.\n");
    return;
  }
  const headers = Object.keys(rows[0]);
  const widths = Object.fromEntries(headers.map((header) => [
    header,
    Math.max(header.length, ...rows.map((row) => String(row[header]).length)),
  ]));
  const line = headers.map((header) => "-".repeat(widths[header])).join("  ");
  process.stdout.write(`${headers.map((header) => pad(header, widths[header])).join("  ")}\n`);
  process.stdout.write(`${line}\n`);
  for (const row of rows) {
    process.stdout.write(`${headers.map((header) => pad(String(row[header]), widths[header])).join("  ")}\n`);
  }
}

function pairedComparisons(results) {
  const byCase = new Map();
  for (const result of results) {
    const key = `${result.caseId}::${result.attempt ?? "single"}`;
    if (!byCase.has(key)) {
      byCase.set(key, {
        caseId: result.caseId,
        attempt: result.attempt ?? null,
        variants: {},
      });
    }
    byCase.get(key).variants[result.variant] = result;
  }

  return [...byCase.values()]
    .sort((left, right) => `${left.caseId}:${left.attempt ?? ""}`.localeCompare(`${right.caseId}:${right.attempt ?? ""}`))
    .map(({ caseId, attempt, variants }) => {
      const directTokens = numericTokens(variants.direct);
      const loomTokens = numericTokens(variants.loom);
      const tokenDelta = directTokens !== null && loomTokens !== null
        ? directTokens - loomTokens
        : null;
      const directCompletion = numericCompletion(variants.direct);
      const loomCompletion = numericCompletion(variants.loom);
      return {
        caseId,
        attempt,
        directStatus: variants.direct?.status || null,
        loomStatus: variants.loom?.status || null,
        directCompletion,
        loomCompletion,
        completionDelta: directCompletion !== null && loomCompletion !== null
          ? round(loomCompletion - directCompletion, 1)
          : null,
        directTokens,
        loomTokens,
        tokensSavedByLoom: tokenDelta,
        tokensSavedPct: tokenDelta !== null && directTokens > 0
          ? round(tokenDelta / directTokens * 100, 1)
          : null,
      };
    });
}

function numericTokens(result) {
  const value = result?.tokenUsage?.total;
  return Number.isFinite(value) ? value : null;
}

function enrichMissingTokenUsage(runDir, result) {
  if (numericTokens(result) !== null) {
    return result;
  }
  const tokens = inferTokensFromAgentLog(runDir, result);
  if (tokens === null) {
    return result;
  }
  return {
    ...result,
    tokenUsage: {
      ...(result.tokenUsage || {}),
      total: tokens,
      source: "agent_log_fallback",
    },
  };
}

function inferTokensFromAgentLog(runDir, result) {
  if (!result?.caseId || !result?.variant) {
    return null;
  }
  const logPath = path.join(runDir, "agent-logs", `${result.caseId}_${result.variant}.log`);
  if (!fs.existsSync(logPath)) {
    return null;
  }
  const log = fs.readFileSync(logPath, "utf8");
  const matches = [...log.matchAll(/tokens used\s*:?\s*\n?\s*([0-9][0-9,]*)/gi)];
  if (matches.length === 0) {
    return null;
  }
  const lastMatch = matches[matches.length - 1];
  const parsed = Number(lastMatch[1].replace(/,/g, ""));
  return Number.isFinite(parsed) ? parsed : null;
}

function numericCompletion(result) {
  const value = result?.completion?.scorePct;
  return Number.isFinite(value) ? value : null;
}

function completionLabel(result) {
  const completion = result.completion || {};
  if (Number.isFinite(completion.scorePct)) {
    const met = completion.successCriteriaMet ?? "";
    const total = completion.successCriteriaTotal ?? "";
    return `${completion.scorePct}% (${met}/${total})`;
  }
  return "";
}

function printComparisons(comparisons) {
  const comparable = comparisons.filter((item) => item.directTokens !== null && item.loomTokens !== null);
  if (comparable.length === 0) {
    return;
  }
  process.stdout.write("\nPaired token comparison\n");
  printTable(comparable.map((item) => ({
    Case: item.caseId,
    Attempt: item.attempt ?? "",
    Direct: item.directTokens,
    Loom: item.loomTokens,
    "Loom Saved": item.tokensSavedByLoom,
    "Saved %": item.tokensSavedPct === null ? "" : `${item.tokensSavedPct}%`,
    "Completion Δ": item.completionDelta === null ? "" : `${item.completionDelta}%`,
  })));
}

function renderComparisonMarkdown(comparisons) {
  const comparable = comparisons.filter((item) => item.directTokens !== null && item.loomTokens !== null);
  if (comparable.length === 0) {
    return [];
  }
  const headers = ["Case", "Attempt", "Direct Tokens", "Loom Tokens", "Loom Saved", "Saved %", "Completion Delta"];
  const rows = comparable.map((item) => [
    item.caseId,
    item.attempt ?? "",
    item.directTokens,
    item.loomTokens,
    item.tokensSavedByLoom,
    item.tokensSavedPct === null ? "" : `${item.tokensSavedPct}%`,
    item.completionDelta === null ? "" : `${item.completionDelta}%`,
  ]);
  return [
    "## Paired Token Comparison",
    "",
    "| " + headers.join(" | ") + " |",
    "| " + headers.map(() => "---").join(" | ") + " |",
    ...rows.map((row) => "| " + row.join(" | ") + " |"),
    "",
  ];
}

function aggregateComparisons(comparisons) {
  const comparable = comparisons.filter((item) => item.directTokens !== null && item.loomTokens !== null);
  const byCase = new Map();
  for (const comparison of comparable) {
    if (!byCase.has(comparison.caseId)) {
      byCase.set(comparison.caseId, []);
    }
    byCase.get(comparison.caseId).push(comparison);
  }
  return [...byCase.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([caseId, rows]) => {
      const tokenDeltas = rows.map((row) => row.tokensSavedByLoom).filter(isNumber);
      const tokenPcts = rows.map((row) => row.tokensSavedPct).filter(isNumber);
      const completionDeltas = rows.map((row) => row.completionDelta).filter(isNumber);
      return {
        caseId,
        pairedRuns: rows.length,
        loomTokenWinRuns: rows.filter((row) => isNumber(row.tokensSavedByLoom) && row.tokensSavedByLoom > 0).length,
        directPassRatePct: passRate(rows.map((row) => row.directStatus)),
        loomPassRatePct: passRate(rows.map((row) => row.loomStatus)),
        medianTokensSavedByLoom: median(tokenDeltas),
        meanTokensSavedByLoom: round(mean(tokenDeltas), 1),
        medianTokensSavedPct: median(tokenPcts),
        meanTokensSavedPct: round(mean(tokenPcts), 1),
        medianCompletionDelta: median(completionDeltas),
        meanCompletionDelta: round(mean(completionDeltas), 1),
      };
    });
}

function printAggregates(aggregates) {
  if (!Array.isArray(aggregates) || aggregates.length === 0) {
    return;
  }
  process.stdout.write("\nAggregate comparison\n");
  printTable(aggregates.map((item) => ({
    Case: item.caseId,
    Runs: item.pairedRuns,
    "Loom Wins": item.loomTokenWinRuns,
    "Median Saved": item.medianTokensSavedByLoom ?? "",
    "Median Saved %": item.medianTokensSavedPct === null ? "" : `${item.medianTokensSavedPct}%`,
    "Mean Saved %": item.meanTokensSavedPct === null ? "" : `${item.meanTokensSavedPct}%`,
    "Completion Δ": item.medianCompletionDelta === null ? "" : `${item.medianCompletionDelta}%`,
  })));
}

function renderAggregateMarkdown(aggregates) {
  if (!Array.isArray(aggregates) || aggregates.length === 0) {
    return [];
  }
  const headers = ["Case", "Paired Runs", "Loom Token Wins", "Median Saved", "Median Saved %", "Mean Saved %", "Median Completion Delta"];
  const rows = aggregates.map((item) => [
    item.caseId,
    item.pairedRuns,
    item.loomTokenWinRuns,
    item.medianTokensSavedByLoom ?? "",
    item.medianTokensSavedPct === null ? "" : `${item.medianTokensSavedPct}%`,
    item.meanTokensSavedPct === null ? "" : `${item.meanTokensSavedPct}%`,
    item.medianCompletionDelta === null ? "" : `${item.medianCompletionDelta}%`,
  ]);
  return [
    "## Aggregate Comparison",
    "",
    "| " + headers.join(" | ") + " |",
    "| " + headers.map(() => "---").join(" | ") + " |",
    ...rows.map((row) => "| " + row.join(" | ") + " |"),
    "",
  ];
}

function resultSortKey(result) {
  return `${result.caseId}:${String(result.attempt ?? "").padStart(4, "0")}:${result.variant}`;
}

function attemptLabel(attempt) {
  return String(attempt).padStart(2, "0");
}

function isNumber(value) {
  return Number.isFinite(value);
}

function mean(values) {
  if (!Array.isArray(values) || values.length === 0) {
    return null;
  }
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function median(values) {
  if (!Array.isArray(values) || values.length === 0) {
    return null;
  }
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 1) {
    return sorted[middle];
  }
  return round((sorted[middle - 1] + sorted[middle]) / 2, 1);
}

function passRate(statuses) {
  if (!Array.isArray(statuses) || statuses.length === 0) {
    return null;
  }
  return round(statuses.filter((status) => status === "passed").length / statuses.length * 100, 1);
}

function pad(value, width) {
  return value + " ".repeat(Math.max(0, width - value.length));
}

function shellQuote(value) {
  const text = String(value);
  if (/^[A-Za-z0-9_./:=@+-]+$/.test(text)) {
    return text;
  }
  return `'${text.replace(/'/g, "'\\''")}'`;
}

function requireValue(args, index, flag) {
  const value = args[index];
  if (!value || value.startsWith("--")) {
    throw new Error(`${flag} requires a value.`);
  }
  return value;
}

function parseNonNegativeInteger(value, flag) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`${flag} must be a non-negative integer.`);
  }
  return parsed;
}

function parsePositiveInteger(value, flag) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`${flag} must be a positive integer.`);
  }
  return parsed;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function round(value, digits) {
  if (!Number.isFinite(value)) {
    return null;
  }
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

function timestamp() {
  return new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
}
