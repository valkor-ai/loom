#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
const cli = path.join(repoRoot, "dist", "cli.js");

function run(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, output);
  return envelope.data;
}

function readProjectJson(projectRoot, relativePath) {
  return JSON.parse(fs.readFileSync(path.join(projectRoot, relativePath), "utf8"));
}

function readRepo(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function hydrateRequest(projectRoot, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readProjectJson(projectRoot, value);
  }
  return hydrated;
}

function assertIncludes(text, needle, message) {
  assert.ok(text.includes(needle), message);
}

const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-user-facing-language-"));
run(["init"], projectRoot);

const started = run([
  "brainstorm",
  "start",
  "--request",
  "请做一个工作人员操作台，支持账户分页查询、开户、取款密码校验、业务阻断提示和成功结果回读。",
], projectRoot);

const request = hydrateRequest(projectRoot, readProjectJson(projectRoot, started.requestPath ?? started.requestRef));
assert.equal(request.requestType, "brainstorm_session");
assert.equal(request.userFacingLanguage.defaultLocale, "zh-CN", "Chinese requirement text must infer zh-CN user-facing language");
assert.equal(request.userFacingLanguage.source, "requirement_primary_language");
assert.ok(
  request.userFacingLanguage.appliesTo.includes("button and action labels"),
  "language constraint must define visible UI copy coverage",
);
assert.ok(
  request.userFacingLanguage.doesNotApplyTo.includes("API paths and payload field names"),
  "language constraint must not force technical identifiers to be translated",
);
assert.equal(
  request.agentAction.read.required.includes("userFacingLanguage"),
  true,
  "Brainstorm agentAction must require reading userFacingLanguage",
);

const phaseScopeCoreGroup = request.agentAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_phase_scope_core");
assert.ok(phaseScopeCoreGroup, "Brainstorm read plan must include phase_scope core group");
assert.equal(
  phaseScopeCoreGroup.fields.includes("userFacingLanguage"),
  true,
  "phase_scope core group must expose userFacingLanguage",
);
assert.equal(
  request.rules.requirementSemanticGrounding.userFacingLanguage.defaultLocale,
  "zh-CN",
  "requirement semantic grounding must carry user-facing language",
);
assertIncludes(
  request.outputContract.schemaShape.candidateRules.join("\n"),
  "User-facing UI copy must default to Chinese",
  "BrainstormCandidate writing rules must instruct Chinese user-visible UI copy",
);

const contract = readProjectJson(projectRoot, `.loom/deliveries/${started.deliveryId}/brainstorms/contract.json`);
assert.equal(
  contract.deliveryContext.userFacingLanguage.defaultLocale,
  "zh-CN",
  "BrainstormContract deliveryContext must preserve user-facing language",
);

const repositoryContextSource = readRepo("src/core/operations/repository-context.ts");
assertIncludes(
  repositoryContextSource,
  "userFacingLanguage = input.contract.deliveryContext.userFacingLanguage",
  "phase continuation Brainstorm requests must reuse deliveryContext user-facing language",
);
assertIncludes(
  repositoryContextSource,
  "inferUserFacingLanguageFromText",
  "phase continuation Brainstorm requests must infer a fallback for older contracts",
);

const contractsSource = readRepo("src/core/operations/contracts.ts");
assertIncludes(
  contractsSource,
  "userFacingLanguage: brainstorm.deliveryContext.userFacingLanguage ?? null",
  "PGC must mechanically preserve Brainstorm user-facing language",
);
assertIncludes(
  contractsSource,
  "userFacingLanguage: pgc.planningInputs.userFacingLanguage ?? null",
  "AAC requirement-detail transfer must carry user-facing language",
);

const tasksSource = readRepo("src/core/operations/tasks.ts");
assertIncludes(
  tasksSource,
  "sourceContext.userFacingLanguage",
  "TaskExecutionRequest read plan must require user-facing language",
);
assertIncludes(
  tasksSource,
  "frontendExperienceExecutionRules",
  "TaskExecutionRequest must carry frontend execution rules",
);
assertIncludes(
  tasksSource,
  "userFacingLanguageRule(userFacingLanguage)",
  "TaskExecutionRequest rules must include the user-facing language rule",
);

const reviewSource = readRepo("src/core/operations/review.ts");
assertIncludes(
  reviewSource,
  "Obvious language drift is a frontend_experience finding",
  "ReviewRequest must require checking visible UI language drift",
);
assertIncludes(
  reviewSource,
  "buildFrontendExperienceReview(aac, pgc)",
  "Frontend review contract must receive PGC user-facing language",
);

fs.rmSync(projectRoot, { recursive: true, force: true });
console.log("User-facing language contract verification passed.");
