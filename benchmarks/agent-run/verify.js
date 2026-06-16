#!/usr/bin/env node

const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "loom-agent-run-verify-"));
const prepareOutput = execFileSync(process.execPath, [
  path.join(root, "benchmarks", "agent-run", "run.js"),
  "prepare",
  "--skip-build",
  "--case",
  "backend-readiness-continuation",
  "--case",
  "billing-entitlements-continuation",
  "--case",
  "compliance-evidence-continuation",
  "--case",
  "customer-onboarding-continuation",
  "--case",
  "fulfillment-operations-continuation",
  "--case",
  "incident-review-continuation",
  "--case",
  "feature-flags-continuation",
  "--case",
  "analytics-funnel-continuation",
  "--case",
  "workspace-permissions-continuation",
  "--case",
  "release-readiness-continuation",
  "--case",
  "support-sla-continuation",
  "--out-dir",
  outDir,
  "--json",
], {
  cwd: root,
  encoding: "utf8",
});

const manifest = JSON.parse(prepareOutput);
assert(manifest.runDir && fs.existsSync(manifest.runDir), "prepare must create runDir");
const preparedCase = manifest.cases.find((item) => item.id === "backend-readiness-continuation");
assert(preparedCase, "prepare must include backend-readiness-continuation");
const direct = preparedCase.variants.find((variant) => variant.variant === "direct");
const loom = preparedCase.variants.find((variant) => variant.variant === "loom");
assert(direct && fs.existsSync(direct.promptPath), "prepare must create direct prompt");
assert(loom && fs.existsSync(loom.promptPath), "prepare must create loom prompt");
assert(fs.existsSync(loom.loom.planPath), "prepare must save loom plan output");
const loomPrompt = fs.readFileSync(loom.promptPath, "utf8");
assert(
  loomPrompt.includes("Preserve that flow state"),
  "loom prompt must preserve Brainstorm flow instead of cancelling it"
);
assert(
  !loomPrompt.includes("plan output"),
  "loom prompt must not ask agents to read loom-plan.json by default"
);
assert(
  loomPrompt.includes("Start with the local inspect command template"),
  "loom prompt must start with compact inspect"
);
assert(
  loomPrompt.includes("--values-only"),
  "loom prompt must use values-only inspect for benchmark request reads"
);
assert(
  !loomPrompt.includes("\n## Request\n"),
  "loom prompt must not duplicate the full direct request block"
);
assert(
  loomPrompt.includes("compact inspect output as the request authority"),
  "loom prompt must route request context through compact inspect"
);
assert(
  loomPrompt.includes("run repeated readbacks"),
  "loom prompt must discourage repeated closeout reads"
);
assert(
  loomPrompt.includes("record --variant-dir"),
  "loom prompt must use the record helper for closeout"
);
assert(
  loomPrompt.includes(`writes \`${loom.resultPath}\``),
  "loom prompt must name the canonical result path"
);
assert(
  loomPrompt.includes("Do not open `RESULT_TEMPLATE.json`, hand-write result JSON"),
  "loom prompt must avoid manual result JSON closeout"
);

const backendCase = manifest.cases.find((item) => item.id === "backend-readiness-continuation");
assert(backendCase, "prepare must include backend-readiness-continuation");
const backendDirect = backendCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(backendDirect.workspaceDir, "docs", "phase-1-readiness.md")),
  "prepare must copy backend readiness docs"
);
const billingCase = manifest.cases.find((item) => item.id === "billing-entitlements-continuation");
assert(billingCase, "prepare must include billing-entitlements-continuation");
const billingDirect = billingCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(billingDirect.workspaceDir, "docs", "phase-1-billing.md")),
  "prepare must copy billing docs"
);
const complianceCase = manifest.cases.find((item) => item.id === "compliance-evidence-continuation");
assert(complianceCase, "prepare must include compliance-evidence-continuation");
const complianceDirect = complianceCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(complianceDirect.workspaceDir, "docs", "phase-1-compliance.md")),
  "prepare must copy compliance docs"
);
const customerCase = manifest.cases.find((item) => item.id === "customer-onboarding-continuation");
assert(customerCase, "prepare must include customer-onboarding-continuation");
const customerDirect = customerCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(customerDirect.workspaceDir, "docs", "phase-1-onboarding.md")),
  "prepare must copy customer onboarding docs"
);
const fulfillmentCase = manifest.cases.find((item) => item.id === "fulfillment-operations-continuation");
assert(fulfillmentCase, "prepare must include fulfillment-operations-continuation");
const fulfillmentDirect = fulfillmentCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(fulfillmentDirect.workspaceDir, "docs", "phase-1-fulfillment.md")),
  "prepare must copy fulfillment docs"
);
const incidentCase = manifest.cases.find((item) => item.id === "incident-review-continuation");
assert(incidentCase, "prepare must include incident-review-continuation");
const incidentDirect = incidentCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(incidentDirect.workspaceDir, "docs", "phase-1-incidents.md")),
  "prepare must copy incident docs"
);
const flagsCase = manifest.cases.find((item) => item.id === "feature-flags-continuation");
assert(flagsCase, "prepare must include feature-flags-continuation");
const flagsDirect = flagsCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(flagsDirect.workspaceDir, "docs", "phase-1-flags.md")),
  "prepare must copy feature flag docs"
);
const analyticsCase = manifest.cases.find((item) => item.id === "analytics-funnel-continuation");
assert(analyticsCase, "prepare must include analytics-funnel-continuation");
const analyticsDirect = analyticsCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(analyticsDirect.workspaceDir, "docs", "phase-1-analytics.md")),
  "prepare must copy analytics docs"
);
const workspaceCase = manifest.cases.find((item) => item.id === "workspace-permissions-continuation");
assert(workspaceCase, "prepare must include workspace-permissions-continuation");
const workspaceDirect = workspaceCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(workspaceDirect.workspaceDir, "docs", "phase-1-workspace.md")),
  "prepare must copy workspace docs"
);

const continuationCase = manifest.cases.find((item) => item.id === "release-readiness-continuation");
assert(continuationCase, "prepare must include release-readiness-continuation");
const continuationDirect = continuationCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(continuationDirect.workspaceDir, "docs", "phase-1-delivery.md")),
  "prepare must copy continuation seed docs"
);
const supportCase = manifest.cases.find((item) => item.id === "support-sla-continuation");
assert(supportCase, "prepare must include support-sla-continuation");
const supportDirect = supportCase.variants.find((variant) => variant.variant === "direct");
assert(
  fs.existsSync(path.join(supportDirect.workspaceDir, "docs", "phase-1-support.md")),
  "prepare must copy support continuation docs"
);

const repeatOutput = execFileSync(process.execPath, [
  path.join(root, "benchmarks", "agent-run", "run.js"),
  "prepare",
  "--skip-build",
  "--case",
  "support-sla-continuation",
  "--repeat",
  "2",
  "--out-dir",
  outDir,
  "--json",
], {
  cwd: root,
  encoding: "utf8",
});
const repeatManifest = JSON.parse(repeatOutput);
assert(repeatManifest.repeat === 2, "prepare must record repeat count");
assert(repeatManifest.cases.length === 2, "repeat prepare must create one case entry per attempt");
assert(repeatManifest.cases[0].attempt === 1, "repeat prepare must record first attempt");
assert(repeatManifest.cases[1].attempt === 2, "repeat prepare must record second attempt");
assert(
  repeatManifest.cases[0].variants[0].variantDir.includes("attempt-01"),
  "repeat prepare must isolate attempt directories"
);

recordSyntheticResult(repeatManifest.runDir, "support-sla-continuation", 1, "direct", 1000, "direct_delivery");
recordSyntheticResult(repeatManifest.runDir, "support-sla-continuation", 1, "loom", 700, "brainstorm_prompt_confirmed");
recordSyntheticResult(repeatManifest.runDir, "support-sla-continuation", 2, "direct", 1200, "direct_delivery");
recordSyntheticResult(repeatManifest.runDir, "support-sla-continuation", 2, "loom", 900, "brainstorm_prompt_confirmed");

const repeatSummary = JSON.parse(execFileSync(process.execPath, [
  path.join(root, "benchmarks", "agent-run", "run.js"),
  "summarize",
  "--run-dir",
  repeatManifest.runDir,
  "--json",
], {
  cwd: root,
  encoding: "utf8",
}));
const repeatAggregate = repeatSummary.aggregates.find((item) => item.caseId === "support-sla-continuation");
assert(repeatSummary.comparisons.length === 2, "repeat summarize must keep one comparison per attempt");
assert(repeatAggregate.pairedRuns === 2, "repeat aggregate must count paired attempts");
assert(repeatAggregate.loomTokenWinRuns === 2, "repeat aggregate must count Loom wins across attempts");
assert(repeatAggregate.medianTokensSavedByLoom === 300, "repeat aggregate must calculate median saved tokens");
assert(repeatAggregate.medianTokensSavedPct === 27.5, "repeat aggregate must calculate median saved percent");

const fallbackRunDir = path.join(outDir, "fallback-run");
const fallbackDirectDir = path.join(fallbackRunDir, "cases", "fallback-case", "direct");
const fallbackLoomWorkspaceDir = path.join(fallbackRunDir, "cases", "fallback-case", "loom", "workspace");
fs.mkdirSync(fallbackDirectDir, { recursive: true });
fs.mkdirSync(fallbackLoomWorkspaceDir, { recursive: true });
fs.mkdirSync(path.join(fallbackRunDir, "agent-logs"), { recursive: true });
fs.writeFileSync(path.join(fallbackDirectDir, "BENCHMARK_RESULT.json"), `${JSON.stringify({
  caseId: "fallback-case",
  title: "Fallback Case",
  variant: "direct",
  attempt: 1,
  status: "passed",
  tests: "passed",
  tokenUsage: { total: 9000, source: "agent_surface_report" },
  completion: { scorePct: 100, successCriteriaMet: 5, successCriteriaTotal: 5 },
  readPolicy: { rawEvidenceCount: 0 },
}, null, 2)}\n`);
fs.writeFileSync(path.join(fallbackLoomWorkspaceDir, "BENCHMARK_RESULT.json"), `${JSON.stringify({
  caseId: "fallback-case",
  title: "Fallback Case",
  variant: "loom",
  attempt: 1,
  status: "passed",
  tests: "passed",
  tokenUsage: { total: null, source: "not_recorded" },
  completion: { scorePct: 100, successCriteriaMet: 5, successCriteriaTotal: 5 },
  readPolicy: { rawEvidenceCount: 0 },
}, null, 2)}\n`);
fs.writeFileSync(
  path.join(fallbackRunDir, "agent-logs", "fallback-case_loom.log"),
  "benchmark output\n\ntokens used\n4,567\n",
  "utf8"
);
const fallbackSummary = JSON.parse(execFileSync(process.execPath, [
  path.join(root, "benchmarks", "agent-run", "run.js"),
  "summarize",
  "--run-dir",
  fallbackRunDir,
  "--json",
], {
  cwd: root,
  encoding: "utf8",
}));
const fallbackComparison = fallbackSummary.comparisons.find((item) => item.caseId === "fallback-case");
assert(fallbackComparison.loomTokens === 4567, "summarize must infer missing loom tokens from agent logs");
assert(fallbackComparison.tokensSavedByLoom === 4433, "fallback token inference must feed paired comparison");

execFileSync(process.execPath, [
  path.join(root, "benchmarks", "agent-run", "run.js"),
  "record",
  "--variant-dir",
  direct.variantDir,
  "--status",
  "passed",
  "--turns",
  "2",
  "--repair-loops",
  "0",
  "--tests",
  "passed",
  "--verification-command",
  "npm test",
  "--verification-status",
  "passed",
  "--tokens-used",
  "1000",
  "--changed-file",
  "workspace/src/readiness.js",
  "--success-criteria-met",
  "3",
  "--success-criteria-total",
  "3",
  "--flow-step",
  "direct_delivery",
  "--notes",
  "verify direct smoke",
], {
  cwd: root,
  encoding: "utf8",
});

execFileSync(process.execPath, [
  path.join(root, "benchmarks", "agent-run", "run.js"),
  "record",
  "--variant-dir",
  loom.variantDir,
  "--status",
  "passed",
  "--turns",
  "1",
  "--repair-loops",
  "0",
  "--tests",
  "passed",
  "--verification-command",
  "npm test",
  "--verification-status",
  "passed",
  "--tokens-used",
  "700",
  "--changed-file",
  "workspace/src/readiness.js",
  "--success-criteria-met",
  "3",
  "--success-criteria-total",
  "3",
  "--compact-read",
  "loom-plan.instruction",
  "--compact-read",
  "inspect.originalRequest",
  "--raw-evidence-opened",
  "workspace/logs/example.log",
  "--flow-step",
  "brainstorm_prompt_confirmed",
  "--notes",
  "verify loom smoke",
], {
  cwd: root,
  encoding: "utf8",
});

const resultPath = path.join(loom.variantDir, "BENCHMARK_RESULT.json");
assert(fs.existsSync(resultPath), "record must write BENCHMARK_RESULT.json");
const result = JSON.parse(fs.readFileSync(resultPath, "utf8"));
assert(result.status === "passed", "record must preserve status");
assert(result.turns === 1, "record must preserve turns");
assert(result.repairLoops === 0, "record must preserve repairLoops");
assert(result.verification.status === "passed", "record must preserve verification status");
assert(result.tokenUsage.total === 700, "record must preserve token usage");
assert(result.changedFiles.includes("workspace/src/readiness.js"), "record must preserve changed files");
assert(result.completion.successCriteriaMet === 3, "record must preserve completed criteria");
assert(result.completion.successCriteriaTotal === 3, "record must preserve total criteria");
assert(result.completion.scorePct === 100, "record must calculate completion score");
assert(result.completion.flowStep === "brainstorm_prompt_confirmed", "record must preserve flow step");
assert(result.readPolicy.compactReads.length === 2, "record must preserve compact reads");
assert(result.readPolicy.rawEvidenceCount === 1, "record must count raw evidence opens");

const summary = execFileSync(process.execPath, [
  path.join(root, "benchmarks", "agent-run", "run.js"),
  "summarize",
  "--run-dir",
  manifest.runDir,
  "--json",
], {
  cwd: root,
  encoding: "utf8",
});
const parsedSummary = JSON.parse(summary);
assert(
  parsedSummary.results.some((item) => item.result.caseId === "backend-readiness-continuation"),
  "summarize must include recorded case"
);
assert(
  parsedSummary.results.some((item) => item.result.status === "passed"),
  "summarize must include recorded status"
);
const comparison = parsedSummary.comparisons.find((item) => item.caseId === "backend-readiness-continuation");
assert(comparison, "summarize must include paired comparison");
assert(comparison.directTokens === 1000, "comparison must include direct tokens");
assert(comparison.loomTokens === 700, "comparison must include loom tokens");
assert(comparison.tokensSavedByLoom === 300, "comparison must calculate tokens saved");
assert(comparison.tokensSavedPct === 30, "comparison must calculate savings percent");
assert(comparison.directCompletion === 100, "comparison must include direct completion");
assert(comparison.loomCompletion === 100, "comparison must include loom completion");
assert(comparison.completionDelta === 0, "comparison must calculate completion delta");
const aggregate = parsedSummary.aggregates.find((item) => item.caseId === "backend-readiness-continuation");
assert(aggregate, "summarize must include aggregate comparison");
assert(aggregate.pairedRuns === 1, "aggregate must count paired runs");
assert(aggregate.loomTokenWinRuns === 1, "aggregate must count loom token wins");
assert(aggregate.medianTokensSavedByLoom === 300, "aggregate must calculate median saved tokens");
assert(aggregate.medianTokensSavedPct === 30, "aggregate must calculate median savings percent");
assert(aggregate.medianCompletionDelta === 0, "aggregate must calculate median completion delta");

fs.rmSync(outDir, { recursive: true, force: true });
console.log("agent-run benchmark verification passed");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function recordSyntheticResult(runDir, caseId, attempt, variant, tokensUsed, flowStep) {
  execFileSync(process.execPath, [
    path.join(root, "benchmarks", "agent-run", "run.js"),
    "record",
    "--variant-dir",
    path.join(runDir, "cases", caseId, `attempt-${String(attempt).padStart(2, "0")}`, variant),
    "--status",
    "passed",
    "--tests",
    "passed",
    "--verification-status",
    "passed",
    "--tokens-used",
    String(tokensUsed),
    "--success-criteria-met",
    "5",
    "--success-criteria-total",
    "5",
    "--flow-step",
    flowStep,
    "--notes",
    "repeat smoke",
  ], {
    cwd: root,
    encoding: "utf8",
  });
}
