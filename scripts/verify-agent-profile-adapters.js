#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const cli = path.join(repoRoot, "dist", "cli.js");

function runEnvelope(args, projectRoot, env = {}, allowFailure = false) {
  let output;
  try {
    output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
      cwd: repoRoot,
      env: { ...process.env, ...env },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    output = error.stdout?.toString("utf8") ?? "";
    if (!allowFailure) {
      throw error;
    }
  }
  const envelope = JSON.parse(output);
  if (!allowFailure) {
    assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  }
  return envelope;
}

function read(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function assertExists(relativePath, message) {
  assert.ok(fs.existsSync(path.join(repoRoot, relativePath)), `${relativePath}: ${message}`);
}

function assertIncludes(file, needle, message) {
  assert.ok(read(file).includes(needle), `${file}: ${message}`);
}

function assertNotIncludes(file, needle, message) {
  assert.equal(read(file).includes(needle), false, `${file}: ${message}`);
}

function assertMaxLines(file, maxLines, message) {
  const lineCount = read(file).split(/\r?\n/).length;
  assert.ok(lineCount <= maxLines, `${file}: ${message}; found ${lineCount}, max ${maxLines}`);
}

function runClaudeWorkflowGuard(input, fixture) {
  const guardPath = path.join(repoRoot, "plugins/claude-code/hooks/loom-workflow-guard.js");
  const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-claude-hook-"));
  const transcriptPath = path.join(projectRoot, "transcript.jsonl");
  const loomRoot = path.join(projectRoot, ".loom");
  const loomHome = path.join(projectRoot, ".loom-home");
  const sessionId = fixture.sessionId || "claude-session-1";
  try {
    fs.mkdirSync(loomRoot, { recursive: true });
    fs.writeFileSync(path.join(loomRoot, "status.json"), `${JSON.stringify(fixture.status, null, 2)}\n`);
    fs.writeFileSync(transcriptPath, fixture.transcriptText || "");
    if (fixture.activeLease) {
      const deliveryId = fixture.activeLease.deliveryId || fixture.status?.activeDeliveryId || "delivery-1";
      writeJson(path.join(loomRoot, "deliveries", deliveryId, "operations", "active-lease.json"), fixture.activeLease);
    }
    for (const [relativePath, value] of Object.entries(fixture.files ?? {})) {
      const file = path.join(projectRoot, relativePath);
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, typeof value === "string" ? value : `${JSON.stringify(value, null, 2)}\n`);
    }
    const userPrompts = Array.isArray(fixture.userPrompts)
      ? fixture.userPrompts
      : Object.prototype.hasOwnProperty.call(fixture, "userPrompt")
        ? [fixture.userPrompt]
        : [];
    for (const userPrompt of userPrompts) {
      execFileSync(process.execPath, [guardPath], {
        cwd: repoRoot,
        env: { ...process.env, LOOM_HOME: loomHome },
        input: JSON.stringify({
          cwd: projectRoot,
          session_id: sessionId,
          transcript_path: transcriptPath,
          hook_event_name: "UserPromptSubmit",
          user_prompt: userPrompt,
        }),
        encoding: "utf8",
        stdio: ["pipe", "pipe", "pipe"],
      });
    }
    if (fixture.preToolUse) {
      execFileSync(process.execPath, [guardPath], {
        cwd: repoRoot,
        env: { ...process.env, LOOM_HOME: loomHome },
        input: JSON.stringify({
          cwd: projectRoot,
          session_id: sessionId,
          transcript_path: transcriptPath,
          hook_event_name: "PreToolUse",
          ...fixture.preToolUse,
        }),
        encoding: "utf8",
        stdio: ["pipe", "pipe", "pipe"],
      });
    }
    const output = execFileSync(process.execPath, [guardPath], {
      cwd: repoRoot,
      env: { ...process.env, LOOM_HOME: loomHome },
      input: JSON.stringify({
        cwd: projectRoot,
        session_id: sessionId,
        transcript_path: transcriptPath,
        ...input,
      }),
      encoding: "utf8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    return output.trim();
  } finally {
    fs.rmSync(projectRoot, { recursive: true, force: true });
  }
}

function assertDeployReferencesAligned() {
  const referenceFiles = [
    "bootstrap.md",
    "compose.md",
    "dockerfile.md",
    "dotnet.md",
    "environment.md",
    "external-references.md",
    "go.md",
    "java.md",
    "node.md",
    "php.md",
    "providers.md",
    "python.md",
    "repair.md",
    "ruby.md",
    "static.md",
    "workspaces.md",
  ];
  const sharedRoot = "plugins/shared/loom-deploy/references";
  for (const fileName of referenceFiles) {
    assertExists(`${sharedRoot}/${fileName}`, `shared deploy references must include ${fileName}`);
  }
  for (const adapterRoot of [
    "plugins/codex/skills/loom-deploy/references",
    "plugins/claude-code/skills/loom-deploy/references",
    "plugins/opencode/.opencode/loom-deploy/references",
  ]) {
    assert.equal(
      fs.existsSync(path.join(repoRoot, adapterRoot)),
      false,
      `${adapterRoot}: deploy references must be installed from the shared source, not maintained as adapter-local copies`,
    );
  }
}

function assertBrainstormPhaseScopeGuidanceAligned() {
  const files = [
    "plugins/codex/skills/loom/SKILL.md",
    "plugins/claude-code/skills/loom/SKILL.md",
    "plugins/opencode/.opencode/commands/loom.md",
  ];
  for (const file of files) {
    assertIncludes(
      file,
      "phaseScopeOptionComparison",
      "Brainstorm phase_scope must point agents at request-owned option comparison guidance",
    );
    assertIncludes(
      file,
      "present 2-3 source-grounded scope options by default",
      "Brainstorm phase_scope must default to multiple source-grounded options",
    );
    assertIncludes(
      file,
      "treat `nextPhaseSeed` as a non-binding seed rather than a preselected answer",
      "Brainstorm phase_scope must not treat nextPhaseSeed as a preselected answer",
    );
    assertIncludes(
      file,
      "Use a single scope only when the request rules' atomic-scope exception is satisfied",
      "Brainstorm phase_scope must restrict single-scope confirmation to the atomic exception",
    );
  }
}

function assertCoreInstructionShape(envelope) {
  assert.equal(envelope.ok, true);
  assert.ok(
    envelope.agentProfile?.id === "codex" ||
      envelope.agentProfile?.id === "claude" ||
      envelope.agentProfile?.id === "opencode",
  );
  assert.equal(envelope.instruction.mode, "ask_user");
  assert.equal(envelope.instruction.expectedResponse.kind, "brainstorm_progressive_clarification");
  assert.ok(envelope.instruction.expectedResponse.requestRef);
  assert.equal(envelope.instruction.candidateFile, undefined, "Brainstorm ask_user must not expose candidateFile before final_summary confirmation");
  assert.equal(envelope.instruction.submitCommand, undefined, "Brainstorm ask_user must not expose submitCommand before final_summary confirmation");
  assert.equal(envelope.instruction.expectedResponse.candidateFile, undefined, "Brainstorm expectedResponse must not expose candidateFile before final_summary confirmation");
  assert.equal(envelope.instruction.expectedResponse.submitCommand, undefined, "Brainstorm expectedResponse must not expose submitCommand before final_summary confirmation");
  assert.equal(envelope.actionRequired, undefined, "Brainstorm start should remain user-gated");
}

function assertCommandInvocation(invocation, profile, argv) {
  assert.equal(invocation.kind, "loom_user_launcher");
  assert.equal(invocation.launcherRef, "$HOME/.loom/bin/loom-cli");
  assert.deepEqual(invocation.env, {
    LOOM_AGENT_PROFILE: profile,
    LOOM_COMPACT_OUTPUT: "1",
  });
  assert.deepEqual(invocation.argv, argv);
  assert.deepEqual(invocation.argvWithProjectRoot, [...argv, "--project-root", invocation.projectRoot]);
  assert.ok(path.isAbsolute(invocation.projectRoot));
  assert.equal(invocation.projectRootRequired, true);
  assert.match(invocation.usage, /argvWithProjectRoot exactly/);
  assert.match(invocation.usage, /Do not use bare loom/);
}

function comparableInstruction(envelope) {
  return {
    mode: envelope.instruction.mode,
    autoContinue: envelope.instruction.autoContinue,
    expectedResponseKind: envelope.instruction.expectedResponse.kind,
    hasRequestRef: typeof envelope.instruction.expectedResponse.requestRef === "string",
    hasCandidateFile: Object.prototype.hasOwnProperty.call(envelope.instruction.expectedResponse, "candidateFile"),
    hasSubmitCommand: Object.prototype.hasOwnProperty.call(envelope.instruction.expectedResponse, "submitCommand"),
    hasSubmitCommandInvocation: envelope.instruction.expectedResponse.submitCommand?.commandInvocation?.kind === "loom_user_launcher",
    nextActionType: envelope.instruction.nextAction?.type,
  };
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function installActiveLease(projectRoot, deliveryId, phaseId) {
  const now = new Date();
  writeJson(path.join(projectRoot, ".loom", "deliveries", deliveryId, "operations", "active-lease.json"), {
    schemaVersion: "1.0",
    operationId: "op-agent-profile-resume-command",
    deliveryId,
    phaseId,
    operationType: "task_execution",
    status: "active",
    startedAt: now.toISOString(),
    heartbeatAt: now.toISOString(),
    expiresAt: new Date(now.getTime() + 30 * 60 * 1000).toISOString(),
    refs: {},
  });
}

function assertResumeCommandSurface(projectRoot, env, expectedUserCommand) {
  const initialStatus = runEnvelope(["status"], projectRoot, env);
  installActiveLease(projectRoot, initialStatus.data.activeDeliveryId, initialStatus.data.activePhaseId);
  const status = runEnvelope(["status"], projectRoot, env);
  assert.equal(status.data.activeOperation.resumeCommand.userCommand, expectedUserCommand);
  assert.ok(
    status.data.activeOperation.resumeCommand.guidance.includes(`Run ${expectedUserCommand} in the Agent chat`),
    `${expectedUserCommand} must be the user-visible resume command`,
  );
}

function assertPlanInitializesEmptyProject(profile) {
  const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), `loom-agent-profile-plan-${profile}-`));
  const envelope = runEnvelope(
    ["plan", "--request", `Build a tiny ${profile} adapter smoke app`],
    projectRoot,
    { LOOM_AGENT_PROFILE: profile, LOOM_COMPACT_OUTPUT: "1" },
  );
  assert.equal(envelope.command, "plan");
  assertCoreInstructionShape(envelope);
  assert.ok(
    fs.existsSync(path.join(projectRoot, ".loom", "status.json")),
    `${profile} plan entrypoint must initialize an empty project before Brainstorm`,
  );
}

function assertPlanRequirementFilePreserved(profile) {
  const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), `loom-agent-profile-file-${profile}-`));
  const requirementFile = path.join(projectRoot, "requirements.txt");
  fs.writeFileSync(
    requirementFile,
    [
      "证券账户开户、资金账户开户、账户关联、挂失、销户、资金查询。",
      "交易客户端需要区分证券账户、资金账户、股票买入、股票卖出和交易结果。",
    ].join("\n"),
  );
  const envelope = runEnvelope(
    [
      "plan",
      "--request",
      "按需求文件启动最小阶段澄清",
      "--requirement-file",
      requirementFile,
    ],
    projectRoot,
    { LOOM_AGENT_PROFILE: profile, LOOM_COMPACT_OUTPUT: "1" },
  );
  assertCoreInstructionShape(envelope);
  const requestRef = envelope.instruction.expectedResponse.requestRef;
  const request = JSON.parse(fs.readFileSync(path.join(projectRoot, requestRef), "utf8"));
  const contextRef = request.contextRefs?.requirementContextRef;
  const keywordHintsRef = request.contextRefs?.keywordHintsRef;
  assert.ok(contextRef, `${profile} plan request must expose requirementContextRef for file requirements`);
  assert.ok(keywordHintsRef, `${profile} plan request must expose keywordHintsRef for file requirements`);
  const context = JSON.parse(fs.readFileSync(path.join(projectRoot, contextRef), "utf8"));
  assert.ok(
    context.sourceItems.some((item) => item.kind === "file" && item.path === requirementFile && item.extractionStatus === "completed"),
    `${profile} plan must preserve requirement files as sourceItems instead of plain request text`,
  );
}

function assertAutoRunnableRecoveryHintNotExposed(profile, adapter, commandSurface) {
  const { withAgentProfile } = require(path.join(repoRoot, "dist/commands/agent-profile.js"));
  const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), `loom-stop-recovery-${profile}-`));
  try {
    const envelope = withAgentProfile({
      ok: true,
      command: "continue",
      version: "0.1.0",
      projectRoot,
      actionRequired: {
        mode: "generate_candidate",
        autoContinue: true,
        mustRunImmediately: true,
        mustNotReportProgress: true,
        mustNotAskBeforeExecuting: true,
        requestRef: ".loom/request.json",
        targetCandidateFile: ".loom/candidate.json",
        summary: "ACTION REQUIRED: generate_candidate is auto-runnable. Do not summarize progress or ask whether to continue.",
      },
      instruction: {
        mode: "generate_candidate",
        autoContinue: true,
        mustRunImmediately: true,
        requestRef: ".loom/request.json",
        targetCandidateFile: ".loom/candidate.json",
      },
      data: {
        instruction: {
          mode: "generate_candidate",
          autoContinue: true,
          mustRunImmediately: true,
          requestRef: ".loom/request.json",
          targetCandidateFile: ".loom/candidate.json",
        },
      },
      summary: "ACTION REQUIRED: generate_candidate is auto-runnable. Do not summarize progress or ask whether to continue.",
    }, { id: profile, adapter, commandSurface });
    const expectedCommand = `${commandSurface} continue`;
    assert.equal(envelope.instruction.stopRecoveryInstruction, undefined);
    assert.equal(envelope.actionRequired.stopRecoveryInstruction, undefined);
    assert.equal(envelope.data.instruction.stopRecoveryInstruction, undefined);
    assert.equal(envelope.summary.includes(expectedCommand), false, "recovery command must not be mixed into the primary summary");
    assert.equal(JSON.stringify(envelope).includes(`Run ${expectedCommand} to resume this active loom operation.`), false, "auto-runnable envelopes must not expose recovery as a final-answer sentence");
  } finally {
    fs.rmSync(projectRoot, { recursive: true, force: true });
  }
}

function runFromDeletedCwd(args, env = {}, allowFailure = false) {
  const deletedCwd = fs.mkdtempSync(path.join(os.tmpdir(), "loom-deleted-cwd-"));
  const projectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-project-root-authority-"));
  const command = [
    `cd ${shellQuote(deletedCwd)}`,
    `rmdir ${shellQuote(deletedCwd)}`,
    `${shellQuote(process.execPath)} ${shellQuote(cli)} ${args.map(shellQuote).join(" ")} --project-root ${shellQuote(projectRoot)} --json`,
  ].join(" && ");
  let output = "";
  try {
    output = execFileSync("sh", ["-c", command], {
      cwd: repoRoot,
      env: { ...process.env, ...env },
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    output = error.stdout?.toString("utf8") ?? "";
    if (!allowFailure) {
      throw error;
    }
  }
  return {
    envelope: JSON.parse(output),
    projectRoot,
  };
}

function assertProjectRootAuthorityWhenCwdIsUnavailable() {
  const initResult = runFromDeletedCwd(["init"], { LOOM_AGENT_PROFILE: "codex" });
  assert.equal(initResult.envelope.ok, true, "absolute --project-root must allow init when shell cwd is unavailable");
  assert.equal(initResult.envelope.projectRoot, initResult.projectRoot);
  assert.ok(
    fs.existsSync(path.join(initResult.projectRoot, ".loom", "status.json")),
    "init from deleted cwd must write state under --project-root",
  );

  const statusResult = runFromDeletedCwd(["status"], { LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" }, true);
  assert.equal(statusResult.envelope.ok, false);
  assert.equal(statusResult.envelope.error.code, "STATE_NOT_INITIALIZED");
  assert.equal(statusResult.envelope.projectRoot, statusResult.projectRoot);
  assert.equal(
    fs.existsSync(path.join(statusResult.projectRoot, ".loom")),
    false,
    "status from deleted cwd must remain read-only and must not initialize .loom",
  );
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}

execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
assertProjectRootAuthorityWhenCwdIsUnavailable();

const missingProfileRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-agent-profile-missing-"));
const missing = runEnvelope(
  ["brainstorm", "start", "--request", "Build a tiny notes app"],
  missingProfileRoot,
  {},
  true,
);
assert.equal(missing.ok, false);
assert.equal(missing.error.code, "AGENT_PROFILE_REQUIRED");
assert.equal(
  fs.existsSync(path.join(missingProfileRoot, ".loom")),
  false,
  "agent-facing command without profile must not mutate project state before failing",
);
assert.match(missing.error.details.repairInstruction, /rerun the exact same loom command/i);

const invalidProfile = runEnvelope(
  ["brainstorm", "start", "--request", "Build a tiny notes app"],
  fs.mkdtempSync(path.join(os.tmpdir(), "loom-agent-profile-invalid-")),
  { LOOM_AGENT_PROFILE: "generic" },
  true,
);
assert.equal(invalidProfile.ok, false);
assert.equal(invalidProfile.error.code, "INVALID_AGENT_PROFILE");
assert.deepEqual(invalidProfile.error.details.supportedProfiles, ["codex", "claude", "opencode"]);

const codexRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-agent-profile-codex-"));
runEnvelope(["init"], codexRoot);
const codex = runEnvelope(
  ["brainstorm", "start", "--request", "Build a tiny notes app"],
  codexRoot,
  { LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" },
);
assert.equal(codex.agentProfile.id, "codex");
assert.equal(codex.agentProfile.adapter, "codex_plugin");
assert.equal(codex.agentProfile.commandSurface, "@loom");
assertCoreInstructionShape(codex);
assertResumeCommandSurface(codexRoot, { LOOM_AGENT_PROFILE: "codex" }, "@loom continue");
assertAutoRunnableRecoveryHintNotExposed("codex", "codex_plugin", "@loom");
assertPlanInitializesEmptyProject("codex");
assertPlanRequirementFilePreserved("codex");

const claudeRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-agent-profile-claude-"));
runEnvelope(["init"], claudeRoot);
const claude = runEnvelope(
  ["brainstorm", "start", "--request", "Build a tiny notes app"],
  claudeRoot,
  { LOOM_AGENT_PROFILE: "claude", LOOM_COMPACT_OUTPUT: "1" },
);
assert.equal(claude.agentProfile.id, "claude");
assert.equal(claude.agentProfile.adapter, "claude_code");
assert.equal(claude.agentProfile.commandSurface, "/loom");
assertCoreInstructionShape(claude);
assertResumeCommandSurface(claudeRoot, { LOOM_AGENT_PROFILE: "claude" }, "/loom continue");
assertAutoRunnableRecoveryHintNotExposed("claude", "claude_code", "/loom");
assertPlanInitializesEmptyProject("claude");
assertPlanRequirementFilePreserved("claude");
assert.deepEqual(
  comparableInstruction(codex),
  comparableInstruction(claude),
  "Codex and Claude adapters must preserve the same core instruction shape",
);

const opencodeRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-agent-profile-opencode-"));
runEnvelope(["init"], opencodeRoot);
const opencode = runEnvelope(
  ["brainstorm", "start", "--request", "Build a tiny notes app"],
  opencodeRoot,
  { LOOM_AGENT_PROFILE: "opencode", LOOM_COMPACT_OUTPUT: "1" },
);
assert.equal(opencode.agentProfile.id, "opencode");
assert.equal(opencode.agentProfile.adapter, "opencode");
assert.equal(opencode.agentProfile.commandSurface, "/loom");
assertCoreInstructionShape(opencode);
assertResumeCommandSurface(opencodeRoot, { LOOM_AGENT_PROFILE: "opencode" }, "/loom continue");
assertAutoRunnableRecoveryHintNotExposed("opencode", "opencode", "/loom");
assertPlanInitializesEmptyProject("opencode");
assertPlanRequirementFilePreserved("opencode");
assert.deepEqual(
  comparableInstruction(codex),
  comparableInstruction(opencode),
  "Codex and opencode adapters must preserve the same core instruction shape",
);

assertIncludes(
  "src/commands/agent-profile.ts",
  "export type AgentProfileId = \"codex\" | \"claude\" | \"opencode\"",
  "registered public profiles must be real adapters only",
);
assertNotIncludes(
  "src/commands/agent-profile.ts",
  "\"generic\"",
  "generic must not be a public profile",
);
assertExists(
  "plugins/codex/.codex-plugin/plugin.json",
  "Codex adapter must be packaged under plugins/codex, not at the repository root",
);
assertExists(
  "plugins/codex/skills/loom/SKILL.md",
  "Codex adapter must expose the main loom skill through its plugin package",
);
assertExists(
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "Codex adapter must expose deploy as an independent plugin skill",
);
assertExists(
  "plugins/codex/skills/loom/references/uix/core.md",
  "Codex main skill must keep frontend UIX detail in a reference file",
);
assertExists(
  "plugins/codex/skills/loom/references/uix/mobile.md",
  "Codex main skill must have a mobile UIX reference",
);
for (const referenceName of ["interaction", "system", "content", "data"]) {
  assertExists(
    `plugins/codex/skills/loom/references/uix/${referenceName}.md`,
    `Codex main skill must have a ${referenceName} UIX reference`,
  );
}
assertDeployReferencesAligned();
assertBrainstormPhaseScopeGuidanceAligned();

const codexManifest = JSON.parse(read("plugins/codex/.codex-plugin/plugin.json"));
assert.equal(codexManifest.name, "loom");
assert.match(codexManifest.description, /Codex adapter/i);

assertIncludes(
  "plugins/codex/skills/loom/SKILL.md",
  "LOOM_AGENT_PROFILE=codex",
  "Codex skill must inject Codex profile",
);
assertIncludes(
  "plugins/codex/skills/loom/SKILL.md",
  "$HOME/.loom/bin/loom-cli",
  "Codex skill must use the shared loom launcher",
);
assertIncludes(
  "plugins/codex/skills/loom/SKILL.md",
  "references/uix/core.md",
  "Codex skill must route frontend UIX work to modular references",
);
assertIncludes(
  "plugins/codex/skills/loom/SKILL.md",
  "references/uix/system.md",
  "Codex skill must route design-system UIX work to modular references",
);
assert.ok(
  read("plugins/codex/skills/loom/SKILL.md").length <= 26000,
  "Codex main skill must stay concise enough for long auto-runnable delivery turns",
);
assert.ok(
  read("plugins/claude-code/skills/loom/SKILL.md").length <= 19000,
  "Claude main skill must stay concise enough for long auto-runnable delivery turns",
);
assert.ok(
  read("plugins/opencode/.opencode/commands/loom.md").length <= 19000,
  "opencode main command must stay concise enough for long auto-runnable delivery turns",
);
assertIncludes(
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "LOOM_AGENT_PROFILE=codex",
  "Codex deploy skill must inject Codex profile",
);

assertExists(
  "plugins/opencode/.opencode/commands/loom.md",
  "opencode adapter must expose the main loom command through its official commands layout",
);
assertExists(
  "plugins/opencode/.opencode/commands/loom-deploy.md",
  "opencode adapter must expose deploy through its official commands layout",
);
assertExists(
  "plugins/opencode/.opencode/plugins/loom.js",
  "opencode adapter must include an official local plugin module",
);
assertExists(
  "plugins/opencode/.opencode/references/loom/uix/core.md",
  "opencode adapter must package frontend UIX references",
);
for (const referenceName of ["interaction", "system", "content", "data"]) {
  assertExists(
    `plugins/opencode/.opencode/references/loom/uix/${referenceName}.md`,
    `opencode adapter must package ${referenceName} UIX references`,
  );
}
assert.equal(
  fs.existsSync(path.join(repoRoot, "plugins", "opencode", ".opencode", "command")),
  false,
  "opencode adapter must not use the obsolete singular .opencode/command layout",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "LOOM_AGENT_PROFILE=opencode",
  "opencode main command must inject opencode profile",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "A bare `/loom <request>` is the normal new-delivery entrypoint",
  "opencode main command must route bare requests like Codex @loom requests",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "plan --project-root",
  "opencode main command must use the plan entrypoint so empty projects initialize before Brainstorm",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "--requirement-file",
  "opencode main command must route local requirement files through CLI extraction instead of plain request text",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "Current invocation facts from opencode",
  "opencode main command must expose concrete slash-command arguments before routing",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "First token: `$1`",
  "opencode main command must make the first argument token explicit",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "If `First token` is exactly `deploy`, this is an explicit deployment command and this command file must handle it directly",
  "opencode main command must dispatch /loom deploy before delivery-state routing",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "For `First token = deploy`, do not run `loom continue`, `loom brainstorm start`, `loom plan`, `loom status`, or any phase/delivery command before the deploy command",
  "opencode /loom deploy must not fall through to phase planning or Brainstorm",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "If `First token = deploy` and `Second token` is empty, run exactly `LOOM_AGENT_PROFILE=opencode LOOM_COMPACT_OUTPUT=1 \"$HOME/.loom/bin/loom-cli\" deploy run --project-root /abs/project`",
  "opencode /loom deploy must map to deploy run exactly",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "Do not ask the user to rerun `/loom-deploy`",
  "opencode /loom deploy must be self-contained rather than depending on the deploy alias",
);
assertNotIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "use `/loom-deploy` for deploy-specific boundaries",
  "opencode /loom deploy must not point users at a separate slash command for normal deploy routing",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "Do not run bare `loom` or depend on shell `PATH`",
  "opencode main command must use the user launcher instead of PATH lookup",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "../references/loom/uix/core.md",
  "opencode main command must route frontend UIX work to modular references",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "../references/loom/uix/system.md",
  "opencode main command must route design-system UIX work to modular references",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "Supported instruction modes",
  "opencode main command must document complete instruction mode handling",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "`repair_candidate`",
  "opencode main command must document candidate repair instructions",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom.md",
  "Avoid multi-line shell scripts for read-only inspection",
  "opencode main command must avoid read-only heredoc/script prompts",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom-deploy.md",
  "LOOM_AGENT_PROFILE=opencode",
  "opencode deploy command must inject opencode profile",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom-deploy.md",
  "$HOME/.loom/bin/loom-cli",
  "opencode deploy command must use the shared loom launcher",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom-deploy.md",
  "execution-repair",
  "opencode deploy command must document deploy-to-execution repair routing",
);
assertIncludes(
  "plugins/opencode/.opencode/commands/loom-deploy.md",
  "$HOME/.config/opencode/loom-deploy/references/",
  "opencode deploy command must expose installed deploy reference location",
);
assertIncludes(
  "plugins/opencode/.opencode/plugins/loom.js",
  "export const LoomPlugin",
  "opencode plugin module must export an official plugin function",
);
assertIncludes(
  "plugins/opencode/.opencode/plugins/loom.js",
  "command.execute.before",
  "opencode plugin must mark explicit /loom command sessions as guarded before command execution",
);
assertIncludes(
  "plugins/opencode/.opencode/plugins/loom.js",
  "tool.execute.after",
  "opencode plugin must inspect loom tool output for auto-runnable continuations",
);
assertIncludes(
  "plugins/opencode/.opencode/plugins/loom.js",
  "session.idle",
  "opencode plugin must recover when opencode idles before following an auto-runnable loom instruction",
);
assertIncludes(
  "plugins/opencode/.opencode/plugins/loom.js",
  "client.session.promptAsync",
  "opencode plugin must be able to trigger one bounded auto-continue prompt after an idle loop boundary",
);
assertIncludes(
  "plugins/opencode/.opencode/plugins/loom.js",
  "LOOM_AUTORUN_REQUIRED",
  "opencode plugin must make auto-runnable tool results unambiguous",
);
assertNotIncludes(
  "plugins/opencode/.opencode/plugins/loom.js",
  "experimental.chat.system.transform",
  "opencode plugin must not inject Loom instructions into ordinary non-Loom chats",
);
execFileSync(
  process.execPath,
  [
    "--input-type=module",
    "-e",
    `
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const repoRoot = ${JSON.stringify(repoRoot)};
const pluginPath = path.join(repoRoot, "plugins/opencode/.opencode/plugins/loom.js");
const source = fs.readFileSync(pluginPath, "utf8");
const moduleUrl = "data:text/javascript;base64," + Buffer.from(source).toString("base64");
const { LoomPlugin } = await import(moduleUrl);
let promptPayload = null;
let promptCount = 0;
const hooks = await LoomPlugin({
  directory: "/tmp/loom-opencode-project",
  client: { session: { promptAsync: async (payload) => { promptPayload = payload; promptCount += 1; } } },
});
const envelope = {
  ok: true,
  command: "technical-baseline.accept",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  actionRequired: { mode: "run_cli", autoContinue: true, mustRunImmediately: true },
  instruction: {
    mode: "run_cli",
    autoContinue: true,
    mustRunImmediately: true,
    commandInvocation: {
      argvWithProjectRoot: ["planning-contract", "create", "--delivery-id", "delivery-1", "--phase-id", "phase-1", "--project-root", "/tmp/loom-opencode-project"],
      projectRoot: "/tmp/loom-opencode-project",
    },
  },
};
const output = { title: "tool", output: JSON.stringify(envelope, null, 2), metadata: {} };
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-1", callID: "call-1" }, output);
assert.match(output.output, /LOOM_AUTORUN_REQUIRED/);
assert.match(output.output, /planning-contract create/);
assert.equal(output.metadata.loomAutoRunnable, true);
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-1" } } });
assert.ok(promptPayload, "idle hook should prompt opencode to continue one bounded auto-runnable step");
assert.equal(promptPayload.body.parts[0].synthetic, undefined, "opencode auto-continue prompt must not use a synthetic text part");
assert.match(promptPayload.body.system, /plain text acknowledgment.*invalid/);
assert.match(promptPayload.body.parts[0].text, /opencode became idle/);
assert.match(promptPayload.body.parts[0].text, /planning-contract create/);
const userDecisionRepairEnvelope = {
  ok: true,
  command: "technical-baseline.accept",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  actionRequired: { mode: "repair_candidate", autoContinue: true, mustRunImmediately: true },
  instruction: {
    mode: "repair_candidate",
    autoContinue: true,
    mustRunImmediately: true,
    candidateFile: ".loom/deliveries/delivery-1/technical-baseline/request/candidate.json",
    issues: [{ code: "GREENFIELD_BASELINE_CONFIRMATION_REQUIRED", repairability: "requires_user_decision" }],
  },
  data: {
    accepted: false,
    nextAction: { type: "needs_user_decision", reason: "TECHNICAL_BASELINE_REQUIRES_USER_CONFIRMATION" },
    repairInstruction: {
      mode: "repair_candidate",
      issues: [{ code: "GREENFIELD_BASELINE_CONFIRMATION_REQUIRED", repairability: "requires_user_decision" }],
    },
  },
};
const userDecisionOutput = { title: "tool", output: JSON.stringify(userDecisionRepairEnvelope, null, 2), metadata: {} };
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-user-decision", callID: "call-user-decision" }, userDecisionOutput);
assert.doesNotMatch(userDecisionOutput.output, /LOOM_AUTORUN_REQUIRED/, "opencode hook must not auto-continue requires_user_decision repair envelopes");
assert.equal(userDecisionOutput.metadata.loomAutoRunnable, undefined, "requires_user_decision envelopes must not be marked auto-runnable");
promptPayload = null;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-user-decision" } } });
assert.equal(promptPayload, null, "requires_user_decision envelopes must not schedule idle repair prompts");
const executeTaskEnvelope = {
  ok: true,
  command: "next-task",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  actionRequired: { mode: "execute_task", autoContinue: true, mustRunImmediately: true },
  instruction: {
    mode: "execute_task",
    autoContinue: true,
    mustRunImmediately: true,
    requestRef: ".loom/tasks/phase-1/execution-requests/exec-task.json",
    resultFile: ".loom/tmp/phase-1/task-results/exec-task/result.json",
    submitCommand: {
      commandInvocation: {
        argvWithProjectRoot: ["record-result", "--input-file", "{resultFile}", "--project-root", "/tmp/loom-opencode-project"],
        projectRoot: "/tmp/loom-opencode-project",
      },
    },
  },
};
const executeTaskOutput = { title: "tool", output: JSON.stringify(executeTaskEnvelope, null, 2), metadata: {} };
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-2", callID: "call-2" }, executeTaskOutput);
assert.match(executeTaskOutput.output, /LOOM_AUTORUN_REQUIRED/);
assert.match(executeTaskOutput.output, /requestRef: \\.loom\\/tasks\\/phase-1\\/execution-requests\\/exec-task\\.json/);
assert.match(executeTaskOutput.output.split("\\n\\n")[0], /submitCommand: .*record-result/);
promptPayload = null;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-2" } } });
assert.ok(promptPayload, "execute_task idle recovery must first read requestReadPlan so the read plan is known");
assert.match(promptPayload.body.parts[0].text, /requestReadPlan first/);
assert.match(promptPayload.body.parts[0].text, /--field requestReadPlan/);
const requestReadPlanInspectEnvelope = {
  ok: true,
  command: "inspect",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  data: {
    requestRef: ".loom/tasks/phase-1/execution-requests/exec-task.json",
    requestedFields: ["requestReadPlan"],
    fields: {
      requestReadPlan: {
        status: "resolved",
        value: {
          groups: [
            {
              groupId: "execute_task_core",
              required: true,
              fields: ["task", "sourceContext.acceptanceSnapshot"],
              readCommand: {
                name: "inspect",
                argv: ["inspect", "--request", ".loom/tasks/phase-1/execution-requests/exec-task.json", "--field", "task,sourceContext.acceptanceSnapshot"],
              },
            },
            {
              groupId: "execute_task_optional_context",
              required: false,
              fields: ["sourceRefs"],
              readCommand: {
                name: "inspect",
                argv: ["inspect", "--request", ".loom/tasks/phase-1/execution-requests/exec-task.json", "--field", "sourceRefs"],
              },
            },
          ],
        },
      },
    },
  },
};
const requestReadPlanInspectOutput = { title: "tool", output: JSON.stringify(requestReadPlanInspectEnvelope, null, 2), metadata: {} };
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-2", callID: "call-request-read-plan" }, requestReadPlanInspectOutput);
assert.match(requestReadPlanInspectOutput.output, /LOOM_NEXT_ACTION/, "requestReadPlan inspect output must inline the next read action instead of waiting for idle recovery");
assert.match(requestReadPlanInspectOutput.output, /next required TaskExecutionRequest field now: task/);
assert.match(requestReadPlanInspectOutput.output, /--field '?task,sourceContext\\.acceptanceSnapshot'?/);
promptPayload = null;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-2" } } });
assert.ok(promptPayload, "after requestReadPlan inspect, adapter must prompt the next required request field");
assert.match(promptPayload.body.parts[0].text, /next required TaskExecutionRequest field now: task/);
assert.match(promptPayload.body.parts[0].text, /--field '?task,sourceContext\\.acceptanceSnapshot'?/);
const inspectRequiredFieldsEnvelope = {
  ok: true,
  command: "inspect",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  data: {
    requestRef: ".loom/tasks/phase-1/execution-requests/exec-task.json",
    requestedFields: ["task", "sourceContext.acceptanceSnapshot"],
    fields: {
      task: {
        status: "resolved",
        value: { taskId: "task-1" },
      },
      "sourceContext.acceptanceSnapshot": {
        status: "resolved",
        value: [{ acceptanceId: "AC-1", statement: "Deliver the task." }],
      },
    },
  },
};
const inspectOutput = { title: "tool", output: JSON.stringify(inspectRequiredFieldsEnvelope, null, 2), metadata: {} };
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-2", callID: "call-3" }, inspectOutput);
assert.match(inspectOutput.output, /LOOM_NEXT_ACTION/, "required field inspect output must inline the next task execution action");
assert.match(inspectOutput.output, /modify\\/verify the project/);
assert.match(inspectOutput.output, /record-result/);
promptPayload = null;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-2" } } });
assert.ok(promptPayload, "inspect output must not clear pending execute_task auto-continue state");
assert.equal(promptPayload.body.parts[0].synthetic, undefined, "execute_task recovery prompt must be a normal promptAsync text part");
assert.match(promptPayload.body.system, /required result file is still missing/i);
assert.match(promptPayload.body.parts[0].text, /already inspected/);
assert.match(promptPayload.body.parts[0].text, /modify\\/verify the project/);
assert.match(promptPayload.body.parts[0].text, /record-result/);
assert.match(promptPayload.body.parts[0].text, /exec-task\\.json/);
assert.doesNotMatch(promptPayload.body.parts[0].text, /--field agentAction/);
promptPayload = null;
const promptsBeforeImmediateIdle = promptCount;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-2" } } });
assert.equal(promptPayload, null, "a quick repeated idle for the same stage must not enqueue another promptAsync immediately");
assert.equal(promptCount, promptsBeforeImmediateIdle);
const generateCandidateEnvelope = {
  ok: true,
  command: "architecture.request",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  actionRequired: { mode: "generate_candidate", autoContinue: true, mustRunImmediately: true },
  instruction: {
    mode: "generate_candidate",
    autoContinue: true,
    mustRunImmediately: true,
    requestRef: ".loom/architecture/requests/arch-req.json",
    candidateKind: "ArchitectureSections",
    sectionGenerationMode: "single_section",
    targetSection: "runtime_delivery",
    targetCandidateFile: ".loom/tmp/architecture/runtime-delivery.json",
    completionBarrier: {
      followUpCommand: {
        commandInvocation: {
          argvWithProjectRoot: ["continue", "--project-root", "/tmp/loom-opencode-project"],
          projectRoot: "/tmp/loom-opencode-project",
        },
      },
    },
  },
};
const generateOutput = { title: "tool", output: JSON.stringify(generateCandidateEnvelope, null, 2), metadata: {} };
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-3", callID: "call-4" }, generateOutput);
assert.match(generateOutput.output, /mode: generate_candidate/);
assert.match(generateOutput.output, /targetCandidateFile: \\.loom\\/tmp\\/architecture\\/runtime-delivery\\.json/);
promptPayload = null;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-3" } } });
assert.ok(promptPayload, "generate_candidate auto-runnable instructions must also be guarded");
assert.match(promptPayload.body.parts[0].text, /requestReadPlan first/);
assert.match(promptPayload.body.parts[0].text, /--field requestReadPlan/);
const generateRequestReadPlanInspectEnvelope = {
  ok: true,
  command: "inspect",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  data: {
    requestRef: ".loom/architecture/requests/arch-req.json",
    requestedFields: ["requestReadPlan"],
    fields: {
      requestReadPlan: {
        status: "resolved",
        value: { groups: [] },
      },
    },
  },
};
const generateRequestReadPlanInspectOutput = { title: "tool", output: JSON.stringify(generateRequestReadPlanInspectEnvelope, null, 2), metadata: {} };
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-3", callID: "call-generate-request-read-plan" }, generateRequestReadPlanInspectOutput);
assert.match(generateRequestReadPlanInspectOutput.output, /LOOM_NEXT_ACTION/, "generate_candidate inspect output must inline candidate generation action");
assert.match(generateRequestReadPlanInspectOutput.output, /Generate\\/write/);
assert.match(generateRequestReadPlanInspectOutput.output, /runtime-delivery\\.json/);
promptPayload = null;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-3" } } });
assert.ok(promptPayload, "generate_candidate should prompt writing only after required read plan fields are resolved");
assert.match(promptPayload.body.parts[0].text, /Generate\\/write/);
assert.match(promptPayload.body.parts[0].text, /runtime-delivery\\.json/);
assert.match(promptPayload.body.parts[0].text, /follow-up command/);
const repairEnvelope = {
  ok: true,
  command: "task-result.repair",
  projectRoot: "/tmp/loom-opencode-project",
  agentProfile: { id: "opencode", adapter: "opencode", commandSurface: "/loom" },
  actionRequired: { mode: "repair_result_contract", autoContinue: true, mustRunImmediately: true },
  instruction: {
    mode: "repair_result_contract",
    autoContinue: true,
    mustRunImmediately: true,
    requestRef: ".loom/tasks/phase-1/repair/repair-result.json",
    resultFile: ".loom/tmp/phase-1/task-results/exec-task/result.json",
    submitCommand: {
      commandInvocation: {
        argvWithProjectRoot: ["record-result", "--input-file", "{resultFile}", "--project-root", "/tmp/loom-opencode-project"],
        projectRoot: "/tmp/loom-opencode-project",
      },
    },
  },
};
await hooks["tool.execute.after"]({ tool: "bash", sessionID: "session-repair", callID: "call-repair" }, { title: "tool", output: JSON.stringify(repairEnvelope, null, 2), metadata: {} });
promptPayload = null;
await hooks.event({ event: { type: "session.idle", properties: { sessionID: "session-repair" } } });
assert.ok(promptPayload, "repair auto-runnable instructions must first read repair requestReadPlan");
assert.match(promptPayload.body.parts[0].text, /repair request requestReadPlan first/);
assert.match(promptPayload.body.parts[0].text, /--field requestReadPlan/);
const resetHooks = await LoomPlugin({
  directory: "/tmp/loom-opencode-project",
  client: { session: { promptAsync: async (payload) => { promptPayload = payload; promptCount += 1; } } },
});
const resetOutput = { title: "tool", output: JSON.stringify(executeTaskEnvelope, null, 2), metadata: {} };
await resetHooks["tool.execute.after"]({ tool: "bash", sessionID: "session-reset", callID: "call-reset-1" }, resetOutput);
const promptCountBeforeExhaustion = promptCount;
for (let index = 0; index < 3; index += 1) {
  await resetHooks.event({ event: { type: "session.idle", properties: { sessionID: "session-reset" } } });
}
assert.equal(promptCount, promptCountBeforeExhaustion + 1, "idle recovery must coalesce rapid repeated idle events for the same stage");
await resetHooks.event({ event: { type: "session.idle", properties: { sessionID: "session-reset" } } });
assert.equal(promptCount, promptCountBeforeExhaustion + 1, "idle recovery must stay bounded when the agent repeatedly ignores it");
await resetHooks["tool.execute.after"]({ tool: "bash", sessionID: "session-reset", callID: "call-reset-2" }, resetOutput);
await resetHooks.event({ event: { type: "session.idle", properties: { sessionID: "session-reset" } } });
assert.equal(promptCount, promptCountBeforeExhaustion + 2, "a fresh manual /loom continue output must reset idle prompt coalescing for the same signature");
const activeProjectRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-opencode-active-"));
fs.mkdirSync(path.join(activeProjectRoot, ".loom", "deliveries", "delivery-active", "operations"), { recursive: true });
fs.writeFileSync(path.join(activeProjectRoot, ".loom", "status.json"), JSON.stringify({
  activeDeliveryId: "delivery-active",
  activePhaseId: "phase-1",
}, null, 2));
fs.writeFileSync(path.join(activeProjectRoot, ".loom", "deliveries", "delivery-active", "operations", "active-lease.json"), JSON.stringify({
  schemaVersion: "1.0",
  operationId: "op-active",
  deliveryId: "delivery-active",
  phaseId: "phase-1",
  operationType: "task_execution",
  status: "active",
  refs: {
    executionRequestRef: ".loom/tasks/phase-1/execution-requests/exec-task.json",
    resultFile: ".loom/tmp/phase-1/task-results/exec-task/result.json",
  },
}, null, 2));
const activeHooks = await LoomPlugin({
  directory: activeProjectRoot,
  client: { session: { promptAsync: async (payload) => { promptPayload = payload; promptCount += 1; } } },
});
const activeEnvelope = structuredClone(executeTaskEnvelope);
activeEnvelope.projectRoot = activeProjectRoot;
activeEnvelope.instruction.submitCommand.commandInvocation.argvWithProjectRoot = ["record-result", "--input-file", "{resultFile}", "--project-root", activeProjectRoot];
activeEnvelope.instruction.submitCommand.commandInvocation.projectRoot = activeProjectRoot;
const activeOutput = { title: "tool", output: JSON.stringify(activeEnvelope, null, 2), metadata: {} };
await activeHooks["tool.execute.after"]({ tool: "bash", sessionID: "session-active", callID: "call-active" }, activeOutput);
const promptCountBeforeActiveExhaustion = promptCount;
for (let index = 0; index < 4; index += 1) {
  promptPayload = null;
  await activeHooks.event({ event: { type: "session.idle", properties: { sessionID: "session-active" } } });
  if (index < 3) {
    await activeHooks["tool.execute.after"](
      { tool: "read", sessionID: "session-active", callID: "call-active-progress-" + index },
      { title: "read", output: "non-loom tool output", metadata: {} },
    );
  }
}
assert.equal(promptCount, promptCountBeforeActiveExhaustion + 4, "active operation with missing output must get a visible recovery prompt after stage exhaustion");
assert.match(promptPayload.body.parts[0].text, /auto-continue stage was prompted repeatedly/);
assert.match(promptPayload.body.parts[0].text, /active operation recovery command/);
assert.match(promptPayload.body.parts[0].text, /continue --project-root/);
console.log("opencode plugin auto-continue behavior passed");
`,
  ],
  { cwd: repoRoot, stdio: "inherit" },
);
assertExists(
  "scripts/refresh-local-opencode-plugin.js",
  "opencode adapter must have a local refresh script",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "\"commands\"",
  "opencode refresh must install command entrypoints to the official commands directory",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "\"plugins\"",
  "opencode refresh must install plugin modules to the official plugins directory",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "copyReferences",
  "opencode refresh must install reference files used by command entrypoints",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "removeLegacyCommand(\"loomline.md\")",
  "opencode refresh must clean stale pre-rename singular command installs",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "removeLegacyOpencodeArtifacts",
  "opencode refresh must clean stale pre-rename commands, plugins, references, and stamps",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "loomline.js",
  "opencode refresh must remove the stale pre-rename plugin module",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "deployReferenceInstallRoot",
  "opencode refresh must install deploy stack references outside the commands directory",
);
assertIncludes(
  "scripts/refresh-local-codex-plugin.js",
  "sharedDeployReferenceSourceRoot",
  "Codex refresh must install deploy references from the shared source",
);
assertIncludes(
  "scripts/refresh-local-claude-plugin.js",
  "sharedDeployReferenceSourceRoot",
  "Claude refresh must install deploy references from the shared source",
);
assertIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "plugins\", \"shared\", \"loom-deploy\", \"references",
  "OpenCode refresh must install deploy references from the shared source",
);
assertNotIncludes(
  "scripts/refresh-local-opencode-plugin.js",
  "const commandInstallRoot = path.join(opencodeConfigRoot, \"command\")",
  "opencode refresh must not install into the obsolete singular command directory",
);

assertExists(
  "plugins/claude-code/.claude-plugin/plugin.json",
  "Claude adapter must be packaged as a Claude Code plugin, not as a per-project command file",
);
assertExists(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Claude adapter must expose the main loom skill through the plugin skills layout",
);
assertExists(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "Claude adapter must expose deploy as an independent plugin skill",
);
assertExists(
  "plugins/claude-code/commands/loom.md",
  "Claude adapter must expose a global /loom command prompt so status/continue route before prose",
);
assertExists(
  "plugins/claude-code/commands/loom-deploy.md",
  "Claude adapter must expose a global /loom-deploy command prompt for direct deploy routing",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "Your first assistant action must be exactly one Bash tool call",
  "Claude /loom command must force a CLI call before prose",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]",
  "Claude /loom command must keep full workflow tools available after the routing CLI call",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1",
  "Claude /loom command must use the profiled compact launcher",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "status --project-root",
  "Claude /loom command must route status directly to the status CLI command",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "plan --project-root",
  "Claude /loom command must use the plan entrypoint so empty projects initialize before Brainstorm",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "--requirement-file",
  "Claude /loom command must route local requirement files through CLI extraction instead of plain request text",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "installed Loom adapter protocol",
  "Claude /loom command must hand non-status routes back to the full Loom adapter protocol",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "read `~/.claude/skills/loom/skills/loom/SKILL.md`",
  "Claude /loom command must explicitly load the full skill protocol for non-status routes",
);
assertIncludes(
  "plugins/claude-code/commands/loom.md",
  "Brainstorm block order",
  "Claude /loom command must preserve Brainstorm clarification protocol for new requests",
);
assertIncludes(
  "plugins/claude-code/commands/loom-deploy.md",
  "deploy run --project-root",
  "Claude /loom-deploy command must route empty deploy command to deploy run",
);
assertIncludes(
  "plugins/claude-code/commands/loom-deploy.md",
  "allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]",
  "Claude /loom-deploy command must keep repair workflow tools available after the routing CLI call",
);
assertIncludes(
  "plugins/claude-code/commands/loom-deploy.md",
  "installed Loom deploy protocol",
  "Claude /loom-deploy command must hand deploy repair routes back to the full deploy protocol",
);
assertIncludes(
  "plugins/claude-code/commands/loom-deploy.md",
  "read `~/.claude/skills/loom/skills/loom-deploy/SKILL.md`",
  "Claude /loom-deploy command must explicitly load the full deploy skill protocol after the first deploy command",
);
assertExists(
  "plugins/claude-code/skills/loom/references/uix/core.md",
  "Claude main skill must keep frontend UIX detail in a reference file",
);
assertExists(
  "plugins/claude-code/skills/loom/references/uix/mobile.md",
  "Claude main skill must have a mobile UIX reference",
);
for (const referenceName of ["interaction", "system", "content", "data"]) {
  assertExists(
    `plugins/claude-code/skills/loom/references/uix/${referenceName}.md`,
    `Claude main skill must have a ${referenceName} UIX reference`,
  );
}

const claudeManifest = JSON.parse(read("plugins/claude-code/.claude-plugin/plugin.json"));
assert.equal(claudeManifest.name, "loom");
assert.match(claudeManifest.description, /Claude Code adapter/i);
assert.equal(
  claudeManifest.hooks,
  "./hooks/hooks.json",
  "Claude adapter must register plugin-level hooks for workflow guardrails",
);
assertExists(
  "plugins/claude-code/hooks/hooks.json",
  "Claude adapter must package workflow guard hooks",
);
assertExists(
  "plugins/claude-code/hooks/loom-workflow-guard.js",
  "Claude adapter must package the workflow guard hook script",
);
assertIncludes(
  "plugins/claude-code/hooks/hooks.json",
  "Bash|EnterPlanMode|ExitPlanMode",
  "Claude workflow guard must intercept Loom CLI commands plus Plan Mode tools",
);
assertNotIncludes(
  "plugins/claude-code/hooks/hooks.json",
  "Agent|TaskCreate|TaskUpdate|TaskList|TaskGet|TaskOutput|TaskStop",
  "Claude workflow guard must not intercept subagent/internal task tools because they are allowed as implementation aids",
);
for (const allowedClaudeTool of ["Agent", "TaskCreate", "TaskUpdate", "TaskList", "TaskGet", "TaskOutput", "TaskStop"]) {
  assertNotIncludes(
    "plugins/claude-code/hooks/hooks.json",
    allowedClaudeTool,
    `Claude workflow guard must not register PreToolUse interception for ${allowedClaudeTool}`,
  );
}
assertIncludes(
  "plugins/claude-code/hooks/hooks.json",
  "\"Stop\"",
  "Claude workflow guard must include a Stop hook for incomplete active task execution",
);
assertIncludes(
  "plugins/claude-code/hooks/hooks.json",
  "\"UserPromptSubmit\"",
  "Claude workflow guard must scope itself to the current user prompt/session instead of stale .loom state",
);
assertIncludes(
  "plugins/claude-code/hooks/loom-workflow-guard.js",
  "LOOM_AGENT_PROFILE=claude",
  "Claude workflow guard must send Claude back through the profiled launcher",
);
assertIncludes(
  "plugins/claude-code/hooks/loom-workflow-guard.js",
  "readSessionState",
  "Claude workflow guard must require session-scoped activation before blocking tools",
);
assertIncludes(
  "plugins/claude-code/hooks/loom-workflow-guard.js",
  "loom_user_gate_reply",
  "Claude workflow guard must preserve activation through multi-turn Loom user gates",
);
assertIncludes(
  "plugins/claude-code/hooks/loom-workflow-guard.js",
  "loom_cli_command",
  "Claude workflow guard must reactivate when the agent runs a Loom CLI command",
);
assertIncludes(
  "plugins/claude-code/hooks/loom-workflow-guard.js",
  "permissionDecision",
  "Claude workflow guard must be able to deny incompatible platform tools",
);
assertIncludes(
  "scripts/refresh-local-claude-plugin.js",
  "commandsRoot",
  "Claude refresh must install global slash commands, not only skills",
);
assertIncludes(
  "scripts/refresh-local-claude-plugin.js",
  "loom.md",
  "Claude refresh must install the /loom global command",
);
assertIncludes(
  "scripts/refresh-local-claude-plugin.js",
  "loom-deploy.md",
  "Claude refresh must install the /loom-deploy global command",
);
assertIncludes(
  "scripts/refresh-local-claude-plugin.js",
  "hooks\", \"hooks.json",
  "Claude refresh must assert hook configuration is copied into the installed plugin",
);
assertIncludes(
  "scripts/refresh-local-claude-plugin.js",
  "hooks\", \"loom-workflow-guard.js",
  "Claude refresh must assert the workflow guard script is copied into the installed plugin",
);
{
  const activeTaskStatus = {
    activeDeliveryId: "delivery-1",
    deliveries: [{ deliveryId: "delivery-1", status: "executing" }],
    effectiveNextAction: {
      targetNode: "task_execution",
      refs: {
        resultFile: ".loom/tmp/result.json",
      },
    },
  };
  const userGateStatus = {
    activeDeliveryId: "delivery-1",
    deliveries: [{ deliveryId: "delivery-1", status: "planning" }],
    effectiveNextAction: {
      type: "brainstorm_clarification",
      targetNode: "brainstorm_clarification",
    },
  };
  const loomTranscript = "LOOM_AGENT_PROFILE=claude $HOME/.loom/bin/loom-cli continue\n";
  const denied = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "PreToolUse", tool_name: "ExitPlanMode" },
      { status: activeTaskStatus, transcriptText: loomTranscript, userPrompt: "/loom continue" },
    ),
  );
  assert.equal(
    denied.hookSpecificOutput.hookEventName,
    "PreToolUse",
    "Claude workflow guard PreToolUse output must include hookEventName for Claude hook schema validation",
  );
  assert.equal(
    denied.hookSpecificOutput.permissionDecision,
    "deny",
    "Claude workflow guard must deny Plan Mode tools during active loom execution",
  );
  assert.match(
    denied.hookSpecificOutput.permissionDecisionReason,
    /Loom workflow is active/,
    "Claude workflow guard PreToolUse denial must include a schema-valid reason",
  );
  assert.match(
    denied.systemMessage,
    /TaskExecutionRequest/,
    "Claude workflow guard denial must tell Claude to continue the Loom execution request",
  );

  for (const toolName of ["TaskCreate", "TaskUpdate", "TaskList", "TaskGet", "TaskOutput", "Agent"]) {
    const allowedInternalToolOutput = runClaudeWorkflowGuard(
      { hook_event_name: "PreToolUse", tool_name: toolName },
      { status: activeTaskStatus, transcriptText: loomTranscript, userPrompt: "/loom continue" },
    );
    assert.equal(
      allowedInternalToolOutput,
      "",
      `Claude workflow guard must allow ${toolName} as an implementation aid while Loom state remains authoritative`,
    );
  }

  const blockedStop = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "Stop" },
      { status: activeTaskStatus, transcriptText: loomTranscript, userPrompt: "/loom continue" },
    ),
  );
  assert.equal(
    blockedStop.decision,
    "block",
    "Claude workflow guard must block incomplete stops while a task result is missing",
  );
  assert.match(
    blockedStop.reason,
    /loom-cli" continue/,
    "Claude workflow guard stop block must route back through the shared launcher",
  );
  assert.match(
    blockedStop.reason,
    /TaskExecutionRequest/,
    "Claude workflow guard stop block must describe the active task operation",
  );

  const userDecisionStopOutput = runClaudeWorkflowGuard(
    { hook_event_name: "Stop" },
    {
      status: {
        activeDeliveryId: "delivery-user-decision",
        deliveries: [{ deliveryId: "delivery-user-decision", status: "waiting_user" }],
        effectiveNextAction: { type: "needs_user_decision", targetNode: "technical_baseline_request" },
      },
      activeLease: {
        schemaVersion: "1.0",
        operationId: "op-user-decision",
        deliveryId: "delivery-user-decision",
        phaseId: "phase-1",
        operationType: "task_execution",
        status: "active",
        refs: { resultFile: ".loom/tmp/result.json" },
      },
      transcriptText: loomTranscript,
      userPrompt: "/loom continue",
    },
  );
  assert.equal(
    userDecisionStopOutput,
    "",
    "Claude workflow guard must not block Stop while the active Loom route is user-gated",
  );

  const activeTechnicalBaselineStopOutput = runClaudeWorkflowGuard(
    { hook_event_name: "Stop" },
    {
      status: {
        activeDeliveryId: "delivery-tb",
        deliveries: [{ deliveryId: "delivery-tb", status: "planning" }],
      },
      activeLease: {
        schemaVersion: "1.0",
        operationId: "op-tb",
        deliveryId: "delivery-tb",
        phaseId: "phase-1",
        operationType: "technical_baseline_generation",
        status: "active",
        refs: {
          requestRef: ".loom/deliveries/delivery-tb/technical-baseline/phase-1/request.json",
          candidateFile: ".loom/deliveries/delivery-tb/technical-baseline/phase-1/candidate.json",
        },
      },
      files: {
        ".loom/deliveries/delivery-tb/technical-baseline/phase-1/request.json": {
          projectKind: "greenfield",
          outputContract: {
            candidateFile: ".loom/deliveries/delivery-tb/technical-baseline/phase-1/candidate.json",
          },
        },
        ".loom/deliveries/delivery-tb/technical-baseline/phase-1/candidate.json": {
          schemaVersion: "1.0",
          technicalBaselineId: "tb-unconfirmed",
          status: "auto_accepted",
          projectKind: "greenfield",
          approval: { type: "policy_auto_accept" },
        },
      },
      transcriptText: loomTranscript,
      userPrompt: "/loom continue",
    },
  );
  assert.equal(
    activeTechnicalBaselineStopOutput,
    "",
    "Claude workflow guard must not block Stop for a greenfield TechnicalBaseline candidate that still requires user confirmation",
  );

  const activeArchitectureStatus = {
    activeDeliveryId: "delivery-arch",
    deliveries: [{ deliveryId: "delivery-arch", status: "planning" }],
  };
  const activeArchitectureLease = {
    schemaVersion: "1.0",
    operationId: "op-arch",
    deliveryId: "delivery-arch",
    phaseId: "phase-3",
    operationType: "architecture_generation",
    status: "active",
    refs: {
      requestRef: ".loom/deliveries/delivery-arch/artifacts/architecture/phase-3/requests/arch-req.json",
      sectionOutputs: [
        { section: "foundation", candidateFile: ".loom/tmp/arch/sections/foundation.json" },
        { section: "coverage", candidateFile: ".loom/tmp/arch/sections/coverage.json" },
      ],
    },
  };
  const blockedArchitectureStop = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "Stop" },
      {
        status: activeArchitectureStatus,
        activeLease: activeArchitectureLease,
        files: {
          ".loom/tmp/arch/sections/foundation.json": { status: "ready" },
        },
        transcriptText: loomTranscript,
        userPrompt: "/loom continue",
      },
    ),
  );
  assert.equal(
    blockedArchitectureStop.decision,
    "block",
    "Claude workflow guard must block stops while ArchitectureSections generation is missing section files",
  );
  assert.match(
    blockedArchitectureStop.reason,
    /architecture_generation|ArchitectureSections/,
    "Claude workflow guard stop block must identify architecture generation",
  );
  assert.match(
    blockedArchitectureStop.reason,
    /coverage/,
    "Claude workflow guard stop block must identify the missing architecture section",
  );

  const activeTaskPlanStatus = {
    activeDeliveryId: "delivery-taskplan",
    deliveries: [{ deliveryId: "delivery-taskplan", status: "planning" }],
  };
  const activeTaskPlanLease = {
    schemaVersion: "1.0",
    operationId: "op-taskplan",
    deliveryId: "delivery-taskplan",
    phaseId: "phase-1",
    operationType: "taskplan_generation",
    status: "active",
    refs: {
      requestRef: ".loom/deliveries/delivery-taskplan/tasks/phase-1/requests/taskplan-gen.json",
    },
  };
  const taskPlanRequestRef = ".loom/deliveries/delivery-taskplan/tasks/phase-1/requests/taskplan-gen.json";
  const taskPlanOutlineRef = ".loom/deliveries/delivery-taskplan/tmp/phase-1/task-plan/taskplan-gen/outline.json";
  const taskPlanGroupPattern = ".loom/deliveries/delivery-taskplan/tmp/phase-1/task-plan/taskplan-gen/groups/{groupId}.json";
  const blockedTaskPlanStop = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "Stop" },
      {
        status: activeTaskPlanStatus,
        activeLease: activeTaskPlanLease,
        files: {
          [taskPlanRequestRef]: {
            outputContract: {
              outlineFile: taskPlanOutlineRef,
              groupFilePattern: taskPlanGroupPattern,
            },
          },
          [taskPlanOutlineRef]: {
            groups: [{ groupId: "group-core", taskIds: ["task-core"] }],
          },
        },
        transcriptText: loomTranscript,
        userPrompt: "/loom continue",
      },
    ),
  );
  assert.equal(
    blockedTaskPlanStop.decision,
    "block",
    "Claude workflow guard must block stops while TaskPlan grouped outputs are missing",
  );
  assert.match(
    blockedTaskPlanStop.reason,
    /TaskPlan|taskplan_generation/,
    "Claude workflow guard stop block must identify taskplan generation",
  );
  assert.match(
    blockedTaskPlanStop.reason,
    /group-core/,
    "Claude workflow guard stop block must identify the missing TaskPlan group file",
  );

  const activeReviewStatus = {
    activeDeliveryId: "delivery-review",
    deliveries: [{ deliveryId: "delivery-review", status: "reviewing" }],
  };
  const activeReviewLease = {
    schemaVersion: "1.0",
    operationId: "op-review",
    deliveryId: "delivery-review",
    phaseId: "phase-1",
    operationType: "review_generation",
    status: "active",
    refs: {
      resultFile: ".loom/tmp/review/result.json",
    },
  };
  const blockedReviewStop = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "Stop" },
      {
        status: activeReviewStatus,
        activeLease: activeReviewLease,
        transcriptText: loomTranscript,
        userPrompt: "/loom continue",
      },
    ),
  );
  assert.equal(
    blockedReviewStop.decision,
    "block",
    "Claude workflow guard must block stops while ReviewResult is missing",
  );
  assert.match(
    blockedReviewStop.reason,
    /ReviewRequest|review_generation/,
    "Claude workflow guard stop block must identify review generation",
  );

  const activeRepairStatus = {
    activeDeliveryId: "delivery-repair",
    deliveries: [{ deliveryId: "delivery-repair", status: "repairing" }],
  };
  const activeRepairLease = {
    schemaVersion: "1.0",
    operationId: "op-repair",
    deliveryId: "delivery-repair",
    phaseId: "phase-1",
    operationType: "architecture_artifact_repair",
    status: "active",
    refs: {
      candidateFile: ".loom/tmp/repair/architecture-repair.json",
    },
  };
  const blockedRepairStop = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "Stop" },
      {
        status: activeRepairStatus,
        activeLease: activeRepairLease,
        transcriptText: loomTranscript,
        userPrompt: "/loom continue",
      },
    ),
  );
  assert.equal(
    blockedRepairStop.decision,
    "block",
    "Claude workflow guard must block stops while repair candidate output is missing",
  );
  assert.match(
    blockedRepairStop.reason,
    /repair|architecture_artifact_repair/i,
    "Claude workflow guard stop block must identify repair generation",
  );

  const completedOutput = runClaudeWorkflowGuard(
    { hook_event_name: "PreToolUse", tool_name: "ExitPlanMode" },
    {
      status: {
        activeDeliveryId: "delivery-1",
        deliveries: [{ deliveryId: "delivery-1", status: "completed" }],
      },
      transcriptText: loomTranscript,
      userPrompt: "/loom continue",
    },
  );
  assert.equal(
    completedOutput,
    "",
    "Claude workflow guard must not block non-active or completed Loom deliveries",
  );

  const unrelatedTranscriptOutput = runClaudeWorkflowGuard(
    { hook_event_name: "PreToolUse", tool_name: "ExitPlanMode" },
    { status: activeTaskStatus, transcriptText: loomTranscript, userPrompt: "please inspect this code normally" },
  );
  assert.equal(
    unrelatedTranscriptOutput,
    "",
    "Claude workflow guard must not hijack ordinary Claude prompts even when stale .loom state and old transcript signals exist",
  );

  const taskStopOutput = runClaudeWorkflowGuard(
    {
      hook_event_name: "PreToolUse",
      tool_name: "TaskStop",
      tool_input: { shell_id: "bg-runtime", task_id: "bg-runtime" },
    },
    { status: activeTaskStatus, transcriptText: loomTranscript, userPrompt: "/loom continue" },
  );
  assert.equal(
    taskStopOutput,
    "",
    "Claude workflow guard must allow TaskStop because Claude uses it to stop task-owned background Bash/runtime probes",
  );

  const clarificationReplyDenied = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "PreToolUse", tool_name: "ExitPlanMode" },
      {
        status: userGateStatus,
        transcriptText: loomTranscript,
        userPrompts: ["/loom 帮我做一个小系统", "确认，按这个范围继续"],
      },
    ),
  );
  assert.equal(
    clarificationReplyDenied.hookSpecificOutput.permissionDecision,
    "deny",
    "Claude workflow guard must stay active through multi-turn Brainstorm clarification replies without /loom",
  );

  const reactivatedByCli = JSON.parse(
    runClaudeWorkflowGuard(
      { hook_event_name: "PreToolUse", tool_name: "ExitPlanMode" },
      {
        status: activeTaskStatus,
        transcriptText: "ordinary Claude coding session\n",
        userPrompt: "普通编码问题",
        preToolUse: {
          tool_name: "Bash",
          tool_input: {
            command: "LOOM_AGENT_PROFILE=claude LOOM_COMPACT_OUTPUT=1 \"$HOME/.loom/bin/loom-cli\" brainstorm accept --project-root /tmp/demo",
          },
        },
      },
    ),
  );
  assert.equal(
    reactivatedByCli.hookSpecificOutput.permissionDecision,
    "deny",
    "Claude workflow guard must reactivate after the agent runs a profiled Loom CLI command",
  );
}

assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "name: loom",
  "Claude main skill must be named loom",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "LOOM_AGENT_PROFILE=claude",
  "Claude main skill must inject Claude profile",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "$HOME/.loom/bin/loom-cli",
  "Claude main skill must use the shared loom launcher",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "references/uix/core.md",
  "Claude main skill must route frontend UIX work to modular references",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "references/uix/system.md",
  "Claude main skill must route design-system UIX work to modular references",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "For `/loom continue`, `/loom status`, `/loom deploy`, or `/loom deploy <subcommand>`, your first assistant action must be the matching Bash tool call",
  "Claude main skill must force direct CLI execution for routing slash commands",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Do not answer in prose, recap state, read files, or inspect `.loom/` before that first CLI call",
  "Claude main skill must forbid no-op prose before first routing CLI call",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "internal task/todo tools and subagents may be used as implementation aids",
  "Claude main skill must allow Claude internal task/subagent tools as implementation aids",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "TaskStop` is allowed only to stop a task-owned background Bash/runtime",
  "Claude main skill must allow TaskStop for bounded runtime cleanup",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Do not use Claude Code Plan Mode, `ExitPlanMode`, or `.claude/plans/*`",
  "Claude main skill must forbid Claude internal Plan Mode handoff during loom execution",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Loom state under `.loom/`, the CLI JSON envelope, and returned `instruction` / `actionRequired` fields are the only task source of truth",
  "Claude main skill must declare loom state as the only task authority",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "If Claude shows stale internal task reminders, ignore them for loom routing",
  "Claude main skill must ignore stale Claude task reminders during loom routing",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "instruction.continuationContract.kind = \"auto_runnable_transition\"",
  "Claude main skill must follow the agent-neutral continuation contract",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "use `inputRefs`, produce `outputRefs`, run the listed command/submit command",
  "Claude main skill must explain continuation inputs and outputs",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "argument-hint: \"<request> | plan <request> | continue | deploy [subcommand] | status\"",
  "Claude main skill must expose bare /loom <request> as the primary new-delivery entrypoint",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "A bare `/loom <request>` is the normal new-delivery entrypoint",
  "Claude main skill must route bare requests like Codex @loom requests",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "--requirement-file",
  "Claude main skill must preserve document/PDF requirement input through CLI extraction",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Supported instruction modes",
  "Claude main skill must document complete instruction mode handling",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "`repair_candidate`",
  "Claude main skill must document candidate repair instructions",
);
for (const file of [
  "plugins/codex/skills/loom/SKILL.md",
  "plugins/claude-code/skills/loom/SKILL.md",
  "plugins/opencode/.opencode/commands/loom.md",
]) {
  assertIncludes(
    file,
    "STATE_NOT_INITIALIZED",
    `${file}: main adapter must treat uninitialized status as a read-only smoke result`,
  );
  assertIncludes(
    file,
    "Do not run manual `init`",
    `${file}: main adapter must not auto-initialize for status/continue`,
  );
  assertIncludes(
    file,
    "--requirement-file",
    `${file}: main adapter must pass local requirement documents through --requirement-file`,
  );
  assertIncludes(
    file,
    "PDF",
    `${file}: main adapter must explicitly cover PDF requirement documents`,
  );
  assertIncludes(
    file,
    "For Brainstorm `ask_user` gates",
    `${file}: main adapter must route Brainstorm ask_user through inspect read fields`,
  );
  assertIncludes(
    file,
    "request-ready/path-only",
    `${file}: main adapter must not let Brainstorm ask_user stop at a request path recap`,
  );
  assertIncludes(
    file,
    ".objective",
    `${file}: main adapter must forbid guessed legacy Brainstorm root fields`,
  );
  assertIncludes(
    file,
    "executionRules.sourceEditPreparationContract",
    `${file}: main adapter must follow the agent-neutral source edit preparation contract`,
  );
  assertIncludes(
    file,
    "malformed",
    `${file}: main adapter must recover through the source edit contract after malformed write calls`,
  );
  assertIncludes(
    file,
    "After an auto-runnable command response, your next action must be a tool call or file operation that follows `instruction`",
    `${file}: main adapter must forbid recap-only stops after auto-runnable responses`,
  );
  assertIncludes(
    file,
    "Before sending any final/progress response during an auto-runnable loom route",
    `${file}: main adapter must enforce the final response guard before prose`,
  );
  assertIncludes(
    file,
    "actionRequired.finalResponseGuard",
    `${file}: main adapter must recognize the top-level final response guard`,
  );
  assertIncludes(
    file,
    "requestReadPlan",
    `${file}: main adapter must use requestReadPlan for request reads`,
  );
}
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]",
  "Claude main skill must preserve Read capability for normal coding work",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Use Claude Code's native file tools normally",
  "Claude main skill must give positive file-tool guidance instead of over-constraining Read",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Treat that as a tool-call retry, not as a loom protocol blocker",
  "Claude main skill must not turn file-read tool mistakes into protocol blockers",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "do not repeat the malformed tool call",
  "Claude main skill must recover from malformed Write/Edit/MultiEdit tool calls",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "Avoid multi-line shell scripts for read-only inspection",
  "Claude main skill must avoid read-only heredoc/script prompts that trigger command safety confirmations",
);
assertNotIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "node -",
  "Claude main skill must not include node heredoc-style read examples",
);
assertNotIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "pages",
  "Claude main skill must not anchor file-tool calls on optional pagination parameters",
);
assertIncludes(
  "plugins/claude-code/skills/loom/SKILL.md",
  "loom-deploy",
  "Claude main skill must route deploy through deploy-specific skill guidance",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "name: loom-deploy",
  "Claude deploy skill must be named loom-deploy",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "LOOM_AGENT_PROFILE=claude",
  "Claude deploy skill must inject Claude profile",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "$HOME/.loom/bin/loom-cli",
  "Claude deploy skill must use the shared loom launcher",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "internal task/todo tools and subagents may be used as implementation aids",
  "Claude deploy skill must allow Claude internal task/subagent tools as implementation aids",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "TaskStop` is allowed only to stop a task-owned background Bash/runtime",
  "Claude deploy skill must allow TaskStop for bounded runtime cleanup",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "Do not use Claude Code Plan Mode, `ExitPlanMode`, or `.claude/plans/*`",
  "Claude deploy skill must forbid Claude internal Plan Mode handoff during deploy execution",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "Loom deploy state, the CLI JSON envelope, and returned `instruction` / `actionRequired` fields are the only task source of truth",
  "Claude deploy skill must declare loom deploy state as the only task authority",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "fullLogRef",
  "Claude deploy skill must preserve deploy repair evidence guidance",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "execution-repair",
  "Claude deploy skill must document deploy-to-execution repair routing",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "## Knowledge Layout",
  "Claude deploy skill must expose deploy reference loading guidance",
);
for (const file of [
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "plugins/opencode/.opencode/commands/loom-deploy.md",
]) {
  assertIncludes(
    file,
    "$HOME/.loom/bin/loom-cli",
    `${file}: deploy adapter must use the shared loom launcher`,
  );
  assertIncludes(
    file,
    "LOOM_COMPACT_OUTPUT=1",
    `${file}: deploy adapter must request compact CLI envelopes`,
  );
  assertIncludes(
    file,
    "agentAction.read.fieldGroups",
    `${file}: deploy adapter must use request inspect read groups before artifact fallbacks`,
  );
  assertIncludes(
    file,
    "DEPLOY_OPERATION_ACTIVE",
    `${file}: deploy adapter must preserve active deploy operation blocker handling`,
  );
  assertIncludes(
    file,
    "observe_active_deploy_operation",
    `${file}: deploy adapter must preserve active deploy observation instruction handling`,
  );
  assertIncludes(
    file,
    "instruction.observationPolicy",
    `${file}: deploy adapter must obey CLI-provided active deploy observation policy`,
  );
  assertIncludes(
    file,
    "first 120 seconds",
    `${file}: deploy adapter must keep an initial quiet window for long deploy runs`,
  );
  assertIncludes(
    file,
    "once every 60 seconds",
    `${file}: deploy adapter must limit active deploy observation cadence`,
  );
  assertIncludes(
    file,
    "operationActive=true",
    `${file}: deploy adapter must not stop while deploy is still active`,
  );
  assertIncludes(
    file,
    "DEPLOY_SOURCE_INSUFFICIENT",
    `${file}: deploy adapter must preserve structured source blocker handling`,
  );
  assertIncludes(
    file,
    "DEPLOY_CONFLICT",
    `${file}: deploy adapter must preserve structured conflict blocker handling`,
  );
  assertIncludes(
    file,
    "Do not run raw `docker compose`",
    `${file}: deploy adapter must forbid raw Docker takeover`,
  );
  assertIncludes(
    file,
    "providers.md",
    `${file}: deploy adapter must expose provider reference guidance`,
  );
  assertIncludes(
    file,
    "executionRules.sourceEditPreparationContract",
    `${file}: deploy adapter must follow the agent-neutral source edit preparation contract`,
  );
  assertIncludes(
    file,
    "malformed",
    `${file}: deploy adapter must recover through the source edit contract after malformed write calls`,
  );
}
assertMaxLines(
  "plugins/codex/skills/loom-deploy/SKILL.md",
  170,
  "Codex deploy skill must stay adapter-facing and not grow back into deploy engine documentation",
);
assertNotIncludes(
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "## Target Architecture",
  "Codex deploy skill must not contain deploy engine architecture documentation",
);
assertNotIncludes(
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "## References",
  "Codex deploy skill must use Knowledge Layout instead of a second references catalog",
);
assertNotIncludes(
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "Strategy resolver chooses",
  "Codex deploy skill must not duplicate deploy provider selection logic",
);
assertNotIncludes(
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "Validator should run",
  "Codex deploy skill must not duplicate deploy validation internals",
);
for (const file of [
  "plugins/codex/skills/loom/SKILL.md",
  "plugins/claude-code/skills/loom/SKILL.md",
  "plugins/claude-code/commands/loom.md",
  "plugins/claude-code/commands/loom-deploy.md",
  "plugins/opencode/.opencode/commands/loom.md",
]) {
  assertIncludes(
    file,
    "observe_active_deploy_operation",
    `${file}: main deploy entrypoint must recognize active deploy observation instructions`,
  );
  assertIncludes(
    file,
    "instruction.observationPolicy",
    `${file}: main deploy entrypoint must obey active deploy observation policy`,
  );
  assertIncludes(
    file,
    "first 120 seconds",
    `${file}: main deploy entrypoint must preserve the initial quiet deploy window`,
  );
  assertIncludes(
    file,
    "once every 60 seconds",
    `${file}: main deploy entrypoint must limit deploy observation cadence`,
  );
  assertIncludes(
    file,
    "operationActive=true",
    `${file}: main deploy entrypoint must forbid final responses while deploy is active`,
  );
}
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "allowed-tools: [Read, Glob, Grep, Bash, Edit, MultiEdit, Write]",
  "Claude deploy skill must preserve Read capability for normal coding work",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "short, single-purpose shell selectors",
  "Claude deploy skill must avoid heavyweight read-only shell scripts",
);
assertIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "do not repeat the malformed tool call",
  "Claude deploy skill must recover from malformed Write/Edit/MultiEdit tool calls",
);
assertNotIncludes(
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "pages",
  "Claude deploy skill must not anchor file-tool calls on optional pagination parameters",
);
assert.equal(
  fs.existsSync(path.join(repoRoot, ".claude", "commands", "loom.md")),
  false,
  "Claude adapter must not rely on a per-project .claude/commands/loom.md file",
);
assert.equal(
  fs.existsSync(path.join(repoRoot, ".codex-plugin")),
  false,
  "Codex adapter source must not live at repository root .codex-plugin",
);
assert.equal(
  fs.existsSync(path.join(repoRoot, "skills")),
  false,
  "Codex adapter source must not live at repository root skills",
);

assertExists(
  "scripts/lib/loom-user-install.js",
  "local refresh must share one Loom user launcher installer",
);
assertExists(
  "scripts/uninstall-local-adapter.js",
  "local adapter uninstall helper must exist",
);
assertIncludes(
  "scripts/uninstall-local-adapter.js",
  "Project-local .loom/ delivery state is not removed.",
  "uninstall helper must preserve project delivery state",
);
assertIncludes(
  "scripts/uninstall-local-adapter.js",
  "plugin:uninstall",
  "uninstall helper usage should align with npm uninstall scripts",
);
const packageJson = JSON.parse(read("package.json"));
for (const scriptName of [
  "plugin:uninstall-codex",
  "plugin:uninstall-claude",
  "plugin:uninstall-opencode",
  "plugin:uninstall-adapters",
]) {
  assert.ok(packageJson.scripts?.[scriptName], `package.json must define ${scriptName}`);
  assert.match(packageJson.scripts[scriptName], /scripts\/uninstall-local-adapter\.js/, `${scriptName} must use the shared uninstall helper`);
}
assertIncludes(
  "scripts/lib/loom-user-install.js",
  "/bin/pwd",
  "shared Loom launcher must probe whether the startup cwd is readable",
);
assertIncludes(
  "scripts/lib/loom-user-install.js",
  "TMPDIR:-/tmp",
  "shared Loom launcher must fall back to a stable cwd before starting Node",
);
for (const file of [
  "scripts/refresh-local-codex-plugin.js",
  "scripts/refresh-local-claude-plugin.js",
  "scripts/refresh-local-opencode-plugin.js",
]) {
  assertIncludes(file, "ensureLoomUserInstall", `${file} must install the shared Loom user launcher`);
  assertNotIncludes(file, ".local\", \"bin\"", `${file} must not use ~/.local/bin as the adapter execution contract`);
  assertNotIncludes(file, "findExecutable(\"loom\")", `${file} must not depend on PATH lookup for adapter execution`);
}
assertIncludes(
  "scripts/refresh-local-codex-plugin.js",
  "personalMarketplacePath",
  "Codex refresh must install through the standard personal marketplace flow",
);
assertIncludes(
  "scripts/refresh-local-codex-plugin.js",
  "personalPluginRoot",
  "Codex refresh must generate the standard ~/plugins/loom plugin source",
);
assertIncludes(
  "scripts/refresh-local-codex-plugin.js",
  "codex\", [\"plugin\", \"add\"",
  "Codex refresh must reinstall through codex plugin add instead of hand-writing cache",
);
assertIncludes(
  "scripts/refresh-local-codex-plugin.js",
  "removeLegacyCodexArtifacts",
  "Codex refresh must remove stale pre-rename local plugin sources and caches",
);
assertIncludes(
  "scripts/refresh-local-codex-plugin.js",
  "plugin?.name !== legacyPluginName",
  "Codex refresh must remove the stale pre-rename marketplace entry",
);
assertIncludes(
  "scripts/refresh-local-claude-plugin.js",
  "removeLegacyClaudeArtifacts",
  "Claude refresh must remove stale pre-rename global commands and skill packages",
);
assertNotIncludes(
  "scripts/refresh-local-codex-plugin.js",
  "activeLocalCacheRoot",
  "Codex refresh must not maintain a parallel hand-written active local cache",
);

for (const file of [
  "src/core/operations/brainstorm.ts",
  "src/core/operations/contracts.ts",
  "src/core/operations/repository-context.ts",
  "src/core/operations/tasks.ts",
  "src/core/operations/review.ts",
  "src/core/operations/repair.ts",
  "src/core/operations/continue.ts",
  "src/core/deployment/operations.ts",
]) {
  assertNotIncludes(file, "LOOM_AGENT_PROFILE", `${file} must not read adapter profile`);
  assertNotIncludes(file, "agentProfile", `${file} must not branch on adapter profile`);
  assertNotIncludes(file, "claude", `${file} must not contain Claude-specific routing`);
  assertNotIncludes(file, "codex_plugin", `${file} must not contain Codex-specific routing`);
  assertNotIncludes(file, "claude_code", `${file} must not contain Claude-specific routing`);
  assertNotIncludes(file, "opencode", `${file} must not contain opencode-specific routing`);
}

console.log("Agent profile adapter verification passed.");
