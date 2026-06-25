#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");

const { runCli } = require("../harness/cli");
const { readRepoFile } = require("../harness/root");
const { hydrateRequest, readProjectJson, tempProject } = require("../harness/files");

function assertIncludes(text, needle, message) {
  assert.ok(text.includes(needle), message);
}

const projectRoot = tempProject("loom-user-facing-language-");
runCli(["init"], projectRoot);

const started = runCli([
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
  request.requestReadPlan.groups.some((group) => group.fields.includes("userFacingLanguage")),
  true,
  "Brainstorm requestReadPlan must require reading userFacingLanguage",
);

const phaseScopeCoreGroup = request.requestReadPlan.groups.find((group) => group.groupId === "brainstorm_session_phase_scope_core");
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

const repositoryContextSource = readRepoFile("core/operations/repository-context.ts");
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

const contractsSource = readRepoFile("core/operations/contracts.ts");
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

const tasksSource = readRepoFile("core/operations/tasks.ts");
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

const reviewSource = readRepoFile("core/operations/review.ts");
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
