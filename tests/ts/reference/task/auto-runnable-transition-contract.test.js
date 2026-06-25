#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../../../../src/ts/reference");
const cli = path.join(repoRoot, "dist", "cli.js");

function run(args, projectRoot, options = {}) {
  const cliArgs = [cli, ...args, "--project-root", projectRoot, "--json"];
  if (options.compact) {
    cliArgs.push("--compact");
  }
  const output = execFileSync(process.execPath, cliArgs, {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  if (options.allowRecordedFalse) {
    assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
    return options.returnEnvelope ? envelope : envelope.data;
  }
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return options.returnEnvelope ? envelope : envelope.data;
}

function runCompact(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  return JSON.parse(output);
}

function runCompactRaw(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  return {
    output,
    envelope: JSON.parse(output),
  };
}

function runDefaultOutput(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  return JSON.parse(output);
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function hydrateRequest(root, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readJson(projectFile(root, value));
  }
  return hydrated;
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function assertExecuteTaskInstruction(data, expectedTaskId, label) {
  assert.equal(data.instruction?.mode, "execute_task", `${label}: expected execute_task instruction`);
  assert.equal(data.instruction?.autoContinue, true, `${label}: expected autoContinue`);
  assert.equal(data.instruction?.mustStartImmediately, true, `${label}: expected mustStartImmediately`);
  assert.equal(data.instruction?.mustNotReportProgressBeforeExecuting, true, `${label}: expected progress summaries to be forbidden before execution`);
  assert.equal(data.instruction?.groupBoundaryIsNotStopCondition, undefined, `${label}: group scheduling details must stay CLI-internal`);
  assert.doesNotMatch(data.instruction?.routingRule ?? "", /group boundaries|task group/i, `${label}: routing rule must not expose group scheduling details`);
  assert.equal(data.instruction?.requestRef, data.executionRequestPath, `${label}: instruction requestRef must match executionRequestPath`);
  assert.equal(data.instruction?.resultFile, data.executionRequest?.resultFile, `${label}: instruction resultFile must match execution summary`);
  assert.deepEqual(data.instruction?.submitCommand, data.executionRequest?.submitCommand, `${label}: instruction submitCommand must match execution summary`);
  assert.equal(data.instruction?.task?.taskId, expectedTaskId, `${label}: instruction task id`);
  assert.equal(data.instruction?.routingRule?.includes("@loom continue"), false, `${label}: routingRule must not expose recovery as primary action`);
  assert.equal(data.instruction?.userMessage?.includes("@loom continue"), false, `${label}: userMessage must not expose recovery as primary action`);
  assert.equal(data.instruction?.stopRecoveryInstruction, undefined, `${label}: auto-runnable instruction must not expose recovery hint as a competing action`);
  assert.ok(data.instruction?.finalResponseGuard?.invalidFinalResponseWhen, `${label}: instruction must carry final response guard`);
  assertAutoRunnableContinuationContract(data.instruction, label);
}

function assertAutoRunnableContinuationContract(instruction, label) {
  const contract = instruction?.continuationContract;
  assert.equal(contract?.kind, "auto_runnable_transition", `${label}: missing auto-runnable continuationContract`);
  assert.equal(contract?.agentObligation?.mustBeginWithoutUserInput, true, `${label}: continuation must begin without user input`);
  assert.equal(contract?.agentObligation?.mustFollowReturnedInstruction, true, `${label}: continuation must follow returned instruction`);
  if (contract?.agentObligation?.requiredSteps !== undefined) {
    assert.ok(Array.isArray(contract.agentObligation.requiredSteps), `${label}: continuation requiredSteps must be an array when present`);
    assert.ok(contract.agentObligation.requiredSteps.length > 0, `${label}: continuation requiredSteps empty`);
  }
  if (contract?.agentObligation?.forbiddenStops !== undefined) {
    assert.ok(Array.isArray(contract.agentObligation.forbiddenStops), `${label}: continuation forbiddenStops must be an array when present`);
  }
  if (contract?.agentObligation?.stopOnlyWhen !== undefined) {
    assert.ok(Array.isArray(contract.agentObligation.stopOnlyWhen), `${label}: continuation stopOnlyWhen must be an array when present`);
  }
  assert.equal(contract?.userVisibleMeaning?.notAStoppingPoint, true, `${label}: continuation must mark transition as not a stopping point`);
  assert.equal(contract?.next?.instructionMode, instruction?.mode, `${label}: continuation next mode must match instruction mode`);
  if (instruction?.requestRef) {
    assert.ok(contract.agentObligation.inputRefs.includes(instruction.requestRef), `${label}: continuation inputRefs must include requestRef`);
  }
  if (instruction?.resultFile) {
    assert.ok(contract.agentObligation.outputRefs.includes(instruction.resultFile), `${label}: continuation outputRefs must include resultFile`);
  }
  if (instruction?.submitCommand) {
    assert.deepEqual(contract.next.submitCommand?.argv, instruction.submitCommand.argv, `${label}: continuation next submitCommand argv must match instruction`);
  }
}

function assertTopLevelActionRequired(envelope, expectedTaskId, label) {
  assert.equal(envelope.actionRequired?.mode, "execute_task", `${label}: expected top-level execute_task action`);
  assert.equal(envelope.actionRequired?.autoContinue, true, `${label}: expected top-level autoContinue`);
  assert.equal(envelope.actionRequired?.mustRunImmediately, true, `${label}: expected top-level mustRunImmediately`);
  assert.equal(envelope.actionRequired?.mustNotReportProgress, true, `${label}: expected top-level progress summaries to be forbidden`);
  assert.equal(envelope.actionRequired?.mustNotAskBeforeExecuting, true, `${label}: expected top-level asking to be forbidden`);
  assert.equal(envelope.actionRequired?.requestRef, envelope.instruction?.requestRef, `${label}: top-level action must expose requestRef directly`);
  assert.equal(envelope.actionRequired?.resultFile, envelope.instruction?.resultFile, `${label}: top-level action must expose resultFile directly`);
  assert.equal(envelope.actionRequired?.submitCommand, undefined, `${label}: actionRequired must not duplicate instruction.submitCommand`);
  assert.equal(envelope.actionRequired?.requestReadProtocol, undefined, `${label}: actionRequired must not duplicate instruction.requestReadProtocol`);
  assert.ok(envelope.instruction?.submitCommand?.commandInvocation, `${label}: instruction submitCommand must expose launcher invocation`);
  assert.ok(envelope.instruction?.requestReadProtocol?.readRule?.includes("requestReadPlan.groups"), `${label}: instruction must explain requestReadPlan-first reads`);
  assert.equal(envelope.actionRequired?.continuationContract, undefined, `${label}: top-level action must not duplicate the full continuation contract`);
  assert.equal(envelope.actionRequired?.agentObligation, undefined, `${label}: top-level action must not duplicate the full agent obligation`);
  assert.ok(envelope.actionRequired?.requiredSteps?.some((step) => String(step).includes("requestReadPlan")), `${label}: top-level action requiredSteps must mention requestReadPlan`);
  assert.ok(envelope.actionRequired?.forbiddenStops?.some((step) => String(step).includes("progress summary")), `${label}: top-level action must expose forbidden progress stops`);
  assert.ok(envelope.actionRequired?.completionBarrier?.resultFile, `${label}: top-level action must expose completion barrier resultFile`);
  assert.equal(envelope.actionRequired?.completionBarrier?.submitCommand, undefined, `${label}: completion barrier must not duplicate top-level submitCommand`);
  assert.ok(envelope.actionRequired?.finalResponseGuard?.invalidFinalResponseWhen, `${label}: top-level action must expose final response guard`);
  assert.equal(envelope.actionRequired?.stopRecoveryInstruction, undefined, `${label}: top-level action must not expose recovery hint as a competing action`);
  assert.ok(
    envelope.actionRequired?.finalResponseGuard?.requiredActionBeforeFinalResponse?.includes("run submitCommand"),
    `${label}: final response guard must force submit before final response`,
  );
  assert.doesNotMatch(envelope.actionRequired?.summary ?? "", /group/i, `${label}: top-level summary must not expose group scheduling details`);
  assert.equal(envelope.actionRequired?.summary?.includes("@loom continue"), false, `${label}: top-level summary must not expose recovery as primary action`);
  assert.equal(envelope.instruction?.mode, "execute_task", `${label}: expected top-level instruction`);
  assert.equal(envelope.instruction?.stopRecoveryInstruction, undefined, `${label}: top-level instruction must not expose recovery hint as a competing action`);
  assert.equal(envelope.data?.instruction?.stopRecoveryInstruction, undefined, `${label}: data.instruction must not expose recovery hint as a competing action`);
  assert.equal(envelope.instruction?.task?.taskId, expectedTaskId, `${label}: top-level instruction task id`);
  assert.equal(envelope.instruction?.requestRef, envelope.data.instruction?.requestRef, `${label}: top-level instruction must mirror data.instruction`);
}

function assertCompactEnvelopeIsSmall(envelope, label) {
  const output = JSON.stringify(envelope, null, 2);
  assert.ok(Buffer.byteLength(output, "utf8") < 16000, `${label}: compact envelope is too large (${Buffer.byteLength(output, "utf8")} bytes)`);
  assert.equal(envelope.actionRequired?.continuationContract, undefined, `${label}: actionRequired must not duplicate continuationContract`);
  assert.equal(envelope.actionRequired?.agentObligation, undefined, `${label}: actionRequired must not duplicate agentObligation`);
  assert.equal(envelope.actionRequired?.submitCommand, undefined, `${label}: actionRequired must not duplicate submitCommand`);
  assert.equal(envelope.actionRequired?.requestReadProtocol, undefined, `${label}: actionRequired must not duplicate requestReadProtocol`);
  assert.equal(envelope.instruction?.communicationRules, undefined, `${label}: instruction must not print communicationRules`);
  assert.equal(envelope.instruction?.chatOutputPolicy, undefined, `${label}: instruction must not print chatOutputPolicy`);
  assert.equal(envelope.instruction?.runtimeCommandGuard, undefined, `${label}: instruction must not print runtimeCommandGuard`);
  assert.equal(envelope.instruction?.completionBarrier?.rules, undefined, `${label}: completionBarrier rules must stay in request refs`);
  assert.equal(envelope.instruction?.completionBarrier?.submitCommand, undefined, `${label}: completionBarrier must not duplicate submitCommand`);
  assert.equal(envelope.actionRequired?.stopRecoveryInstruction, undefined, `${label}: compact actionRequired must not expose recovery hint`);
  assert.equal(envelope.instruction?.stopRecoveryInstruction, undefined, `${label}: compact instruction must not expose recovery hint`);
  assert.equal(envelope.data?.instruction?.stopRecoveryInstruction, undefined, `${label}: compact data.instruction must not expose recovery hint`);
  assert.equal(envelope.instruction?.continuationContract?.agentObligation?.requiredSteps, undefined, `${label}: continuation requiredSteps must not be duplicated inside instruction`);
  assert.equal(envelope.instruction?.continuationContract?.agentObligation?.forbiddenStops, undefined, `${label}: continuation forbiddenStops must not be duplicated inside instruction`);
  assert.equal(envelope.instruction?.continuationContract?.agentObligation?.stopOnlyWhen, undefined, `${label}: continuation stopOnlyWhen must not be duplicated inside instruction`);
  assert.equal(envelope.data?.instruction, undefined, `${label}: compact data must not duplicate instruction`);
}

function assertTopLevelAutoRunnableAction(envelope, expectedMode, label) {
  assert.equal(envelope.actionRequired?.mode, expectedMode, `${label}: expected top-level ${expectedMode} action`);
  assert.equal(envelope.actionRequired?.autoContinue, true, `${label}: expected top-level autoContinue`);
  assert.equal(envelope.actionRequired?.mustRunImmediately, true, `${label}: expected top-level mustRunImmediately`);
  assert.equal(envelope.actionRequired?.mustNotReportProgress, true, `${label}: expected top-level progress summaries to be forbidden`);
  assert.equal(envelope.actionRequired?.mustNotAskBeforeExecuting, true, `${label}: expected top-level asking to be forbidden`);
  assert.ok(envelope.actionRequired?.finalResponseGuard?.invalidFinalResponseWhen, `${label}: top-level action must expose final response guard`);
  assert.equal(envelope.actionRequired?.stopRecoveryInstruction, undefined, `${label}: top-level action must not expose recovery hint as a competing action`);
  assert.ok(envelope.actionRequired?.finalResponseGuard?.requiredActionBeforeFinalResponse, `${label}: final response guard must describe the required action`);
  assert.ok(
    envelope.actionRequired?.requiredSteps?.length > 0,
    `${label}: top-level action must expose requiredSteps for compact auto-runnable routing`,
  );
  assert.ok(
    envelope.actionRequired?.forbiddenStops?.some((step) => String(step).includes("progress summary")),
    `${label}: top-level action must expose forbiddenStops for compact auto-runnable routing`,
  );
  assert.ok(
    envelope.actionRequired?.stopOnlyWhen?.some((step) => String(step).includes("ask_user")),
    `${label}: top-level action must expose stopOnlyWhen for compact auto-runnable routing`,
  );
  assert.ok(
    envelope.actionRequired?.summary?.includes("Do not send a recap or progress summary"),
    `${label}: summary must forbid recap-only stops`,
  );
  assert.equal(envelope.instruction?.mode, expectedMode, `${label}: expected top-level instruction`);
  assert.equal(envelope.instruction?.stopRecoveryInstruction, undefined, `${label}: instruction must not expose recovery hint as a competing action`);
  assert.equal(envelope.data?.instruction?.stopRecoveryInstruction, undefined, `${label}: data.instruction must not expose recovery hint as a competing action`);
  assert.equal(JSON.stringify(envelope).includes("Run @loom continue to resume this active loom operation."), false, `${label}: auto-runnable envelope must not contain a final-answer recovery sentence`);
  assertAutoRunnableContinuationContract(envelope.instruction, label);
}

function assertTaskResultRepairInstructionProtocol(repairInstruction, label) {
  assert.equal(repairInstruction?.chatOutputPolicy?.writeArtifactToFileOnly, true, `${label}: missing artifact-only chat policy`);
  assert.equal(repairInstruction?.chatOutputPolicy?.doNotPasteArtifactJson, true, `${label}: missing JSON paste guard`);
  assert.equal(repairInstruction?.chatOutputPolicy?.doNotPasteDiff, true, `${label}: missing diff paste guard`);
  assert.equal(repairInstruction?.schemaShape, undefined, `${label}: repairInstruction must not inline full TaskResult schemaShape`);
  assert.equal(repairInstruction?.repairContractProfile, "minimal_task_result_repair", `${label}: repairInstruction must use minimal repair profile`);
  assert.ok(Array.isArray(repairInstruction?.issueConflicts), `${label}: repairInstruction must include issue conflicts.`);
  assert.ok(Array.isArray(repairInstruction?.minimalRepairRules), `${label}: repairInstruction must include minimal repair rules.`);
  assert.ok(repairInstruction?.inspectRepairContractCommand, `${label}: repairInstruction must include an on-demand inspect command.`);
  assert.ok(
    repairInstruction?.resultRules?.some((rule) => rule.includes("verificationResults[].status records verification outcome")),
    `${label}: repairInstruction must distinguish verification outcome from self-repair stop reason`,
  );
  assert.ok(
    repairInstruction?.minimalRepairRules?.some((rule) => rule.includes("Never combine selfRepairSummary.attempted=false with stopReason verification_passed")),
    `${label}: minimal repair rules must forbid attempted=false plus verification_passed`,
  );
}

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function writeMinimalDeliveryState(root) {
  const deliveryId = "delivery-task-result-repair";
  const phaseId = "phase-1";
  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "executing",
      requestSummary: "Verify TaskResult repair auto-continue.",
      activePhaseId: phaseId,
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now(),
    }],
    effectiveNextAction: {
      type: "continue_execution",
      deliveryId,
      phaseId,
      reason: "TASKPLAN_READY",
      targetNode: "task_execution",
    },
    phase: "building",
    current: {
      requirementId: null,
      planId: null,
      taskId: null,
      reviewId: null,
      repairId: null,
      deploymentId: null,
    },
    lastAction: null,
    nextAction: "next-task",
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "executing",
    requestSummary: "Verify TaskResult repair auto-continue.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "ready_for_execution",
      latestRefs: {
        taskPlan: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/taskplan-001.json`,
        taskPlanRun: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/run-001.json`,
      },
      nextAction: {
        type: "continue_execution",
        deliveryId,
        phaseId,
        reason: "TASKPLAN_READY",
        targetNode: "task_execution",
      },
    }],
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-001",
    status: "confirmed",
    source: "detected_from_repo",
    projectKind: "existing_project",
    scope: "project",
    stack: { languages: ["TypeScript"], packageManagers: ["npm"], runtime: "node" },
    constraints: [],
    evidence: [{ path: "package.json", reason: "test fixture" }],
    approval: { type: "policy_auto_accept", reason: "test fixture" },
    confidence: "high",
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-001",
    status: "ready",
    source: {
      brainstormRunId: "brainstorm-001",
      brainstormContractId: "brainstorm-contract-001",
      roadmapId: null,
      phaseId,
      technicalBaselineId: "tb-001",
    },
    phaseScope: {
      phaseName: "Phase 1",
      phaseGoal: "Verify TaskResult repair flow.",
      included: [{ scopeId: "scope-001", label: "TaskResult repair", items: ["repair flow"], source: "test" }],
      deferred: [],
      excluded: [],
      acceptanceCandidates: [{ id: "AC-001", statement: "TaskResult repair resumes execution.", priority: "must" }],
    },
    technicalBaseline: {
      technicalBaselineId: "tb-001",
      status: "confirmed",
      scope: "project",
      summary: { languages: ["TypeScript"] },
      mustFollow: true,
    },
    planningInputs: {
      businessGoal: "Verify TaskResult repair flow.",
      actors: [],
      capabilityGroups: [],
      businessFlows: [],
      sourceRefs: [],
      contextNotes: [],
    },
    planningRules: {
      scopeIsolation: {
        onlyPlanCurrentPhase: true,
        forbidDeferredScopeImplementation: true,
        forbidFuturePhaseImplementation: true,
      },
      outputRequirements: {
        mustCreateArchitectureArtifactContract: true,
        mustCreateTaskPlan: true,
        taskPlanMustReferenceAcceptance: true,
      },
      deployment: {
        defaultEnabled: false,
        requiresExplicitUserRequest: true,
      },
    },
    qualityGates: {
      requiresArchitectureBeforeTaskPlan: true,
      requiresAcceptanceCoverage: true,
      requiresVerificationEvidence: true,
    },
    handoff: {
      readyForArchitecture: true,
      readyForTaskPlan: true,
      blockingReasons: [],
      nextNode: "architecture_artifact_contract",
    },
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-001",
    contractRef: `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`,
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-001",
    status: "ready",
    source: {
      planningGenerationContractId: "pgc-001",
      technicalBaselineId: "tb-001",
      brainstormContractId: "brainstorm-contract-001",
      roadmapId: null,
      phaseId,
    },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-main", type: "library", root: "." }],
      modules: [{ moduleId: "module-001", appId: "app-main", paths: ["src"], responsibility: "Test module." }],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [{
      moduleId: "module-001",
      name: "Task Result Flow",
      responsibility: "Verify repair flow.",
      dependsOn: [],
      scopeRefs: ["scope-001"],
      acceptanceRefs: ["AC-001"],
    }],
    dataModel: { entities: [], relationships: [], constraints: [] },
    interfaces: [],
    userFlows: [],
    stateMachines: [],
    acceptanceMatrix: [{
      acceptanceId: "AC-001",
      priority: "must",
      statement: "TaskResult repair resumes execution.",
      coverageStatus: "covered",
      coverage: [{ type: "module", refs: ["module-001"], description: "Task covers repair flow." }],
      verificationHints: [{ kind: "static", description: "Record successful result." }],
    }],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-001",
    contractRef: `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`,
    planningContractId: "pgc-001",
    updatedAt: now(),
  });
  const taskPlan = {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-001",
    version: 1,
    status: "ready",
    source: {
      roadmapId: null,
      phaseId,
      planningGenerationContractId: "pgc-001",
      architectureArtifactContractId: "aac-001",
      technicalBaselineId: "tb-001",
    },
    scopeSnapshot: {
      includedScopeRefs: ["scope-001"],
      excludedScopeRefs: [],
      deferredScopeRefs: [],
      acceptanceRefs: ["AC-001"],
    },
    planningPolicy: {
      taskGranularity: "engineering_increment",
      groupGranularity: "engineering_capability",
      allowTaskSplitDuringRepair: true,
      allowTaskMergeDuringRepair: true,
    },
    groups: [{
      groupId: "group-001",
      title: "First internal group",
      objective: "Verify the first task can complete.",
      dependsOn: [],
      scopeRefs: ["scope-001"],
      acceptanceRefs: ["AC-001"],
      taskIds: ["task-001"],
    }, {
      groupId: "group-002",
      title: "Second internal group",
      objective: "Verify CLI can advance across internal groups without changing the Agent-facing execute_task protocol.",
      dependsOn: ["group-001"],
      scopeRefs: ["scope-001"],
      acceptanceRefs: ["AC-001"],
      taskIds: ["task-002"],
    }],
    tasks: [
      {
        taskId: "task-001",
        groupId: "group-001",
        title: "First task",
        taskKind: "verification_increment",
        implementationActions: ["add_or_update_tests"],
        objective: "Complete first task.",
        dependsOn: [],
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        writeBoundary: {
          forbiddenPaths: [".loom"],
          artifactRefs: {
            modules: ["module-001"],
            entities: [],
            interfaces: [],
            userFlows: [],
            stateMachines: [],
            decisions: [],
            risks: [],
          },
        },
        verificationIntents: [{
          verificationId: "VI-001",
          acceptanceRefs: ["AC-001"],
          behavior: "First task result is accepted.",
          preferredEvidence: ["static_check"],
          acceptableEvidence: ["static_check", "agent_review_explanation"],
        }],
      },
      {
        taskId: "task-002",
        groupId: "group-002",
        title: "Second task",
        taskKind: "verification_increment",
        implementationActions: ["add_or_update_tests"],
        objective: "Remain pending after first task.",
        dependsOn: ["task-001"],
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        writeBoundary: {
          forbiddenPaths: [".loom"],
          artifactRefs: {
            modules: ["module-001"],
            entities: [],
            interfaces: [],
            userFlows: [],
            stateMachines: [],
            decisions: [],
            risks: [],
          },
        },
        verificationIntents: [{
          verificationId: "VI-002",
          acceptanceRefs: ["AC-001"],
          behavior: "Second task can be reached.",
          preferredEvidence: ["static_check"],
          acceptableEvidence: ["static_check", "agent_review_explanation"],
        }],
        frontendExperienceRequirement: {
          frontendExperienceRef: "architectureArtifactContractRef#/frontendExperience",
          experienceLevel: "usable_internal_product",
          mustSatisfy: ["verify current UI surface when relevant"],
        },
        runtimeDeliveryRequirement: {
          appliesToThisTask: true,
          reason: "Fixture task is runtime-relevant so active task recovery can expose possible runtime foreground stall guidance.",
          affectedContractFields: ["runtimeSurfaces"],
          requiredCodeLevelChecks: [{
            checkId: "rd-fixture-runtime-surface",
            contractField: "runtimeSurfaces",
            objective: "Confirm runtime surface remains probeable.",
            acceptableEvidence: ["static_check", "runtime_api_check"],
          }],
        },
      },
    ],
    handoff: { readyForExecution: true, nextNode: "task_execution", blockedReasons: [] },
    createdAt: now(),
    updatedAt: now(),
  };
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/taskplan-001.json`), taskPlan);
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/latest.json`), {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-001",
    taskPlanRef: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/taskplan-001.json`,
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/run-001.json`), {
    schemaVersion: "1.0",
    runId: "run-001",
    taskPlanId: "taskplan-001",
    status: "not_started",
    scheduler: { mode: "group_dag", startedAt: null, finishedAt: null },
    groupStates: [{
      groupId: "group-001",
      status: "pending",
      startedAt: null,
      finishedAt: null,
      dependsOn: [],
      taskIds: ["task-001"],
    }, {
      groupId: "group-002",
      status: "pending",
      startedAt: null,
      finishedAt: null,
      dependsOn: ["group-001"],
      taskIds: ["task-002"],
    }],
    taskStates: [
      { taskId: "task-001", groupId: "group-001", status: "pending", resultId: null, startedAt: null, finishedAt: null, dependsOn: [], attempts: [] },
      { taskId: "task-002", groupId: "group-002", status: "pending", resultId: null, startedAt: null, finishedAt: null, dependsOn: ["task-001"], attempts: [] },
    ],
    summary: { total: 2, completed: 0, completedWithNotes: 0, blocked: 0, failed: 0, pending: 2, running: 0 },
    nextAction: { type: "continue_execution", reason: "TASKPLAN_READY", targetNode: "task_execution" },
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/latest.json`), {
    schemaVersion: "1.0",
    taskPlanRunId: "run-001",
    runRef: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/run-001.json`,
    taskPlanId: "taskplan-001",
    updatedAt: now(),
  });
  return { deliveryId, phaseId };
}

function validTaskResult(taskResultId) {
  return {
    schemaVersion: "1.0",
    taskResultId,
    taskId: "task-001",
    taskPlanId: "taskplan-001",
    status: "completed",
    changedFiles: ["src/example.ts"],
    noChangeReason: null,
    verificationResults: [{
      verificationId: "VI-001",
      status: "passed",
      evidenceType: "static_check",
      summary: "Fixture verification passed.",
    }],
    selfRepairSummary: {
      attempted: false,
      attemptCount: 0,
      stopReason: "not_attempted",
      progressObserved: false,
    },
    failure: null,
    executionContinuity: {
      taskResultSubmittedAfterVerification: true,
      agentOwnedLongRunningWork: "none",
      notes: [],
    },
    notes: [],
    blockedReasons: [],
    createdAt: now(),
    updatedAt: now(),
  };
}

function assertCompactArchitectureRequestTransitionOutput() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-architecture-request-compact-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2));

    run(["init"], root);
    const { deliveryId, phaseId } = writeMinimalDeliveryState(root);
    const { output, envelope } = runCompactRaw([
      "architecture",
      "request",
      "--delivery-id",
      deliveryId,
      "--phase-id",
      phaseId,
    ], root);

    assert.ok(
      Buffer.byteLength(output, "utf8") < 18000,
      `compact architecture.request output is too large (${Buffer.byteLength(output, "utf8")} bytes)`,
    );
    assert.equal(envelope.ok, true);
    assert.equal(envelope.compact, true);
    assert.equal(envelope.command, "architecture.request");
    assert.equal(envelope.actionRequired?.mode, "generate_candidate");
    assert.equal(envelope.actionRequired?.autoContinue, true);
    assert.equal(envelope.actionRequired?.mustRunImmediately, true);
    assert.equal(envelope.actionRequired?.targetCandidateFile, envelope.instruction?.targetCandidateFile);
    assert.equal(
      envelope.actionRequired?.submitCommand,
      undefined,
      "architecture.request single-section actionRequired must not expose submitCommand before submit_existing_candidate",
    );
    assert.equal(
      envelope.instruction?.submitCommand,
      undefined,
      "architecture.request single-section instruction must not expose submitCommand before submit_existing_candidate",
    );
    assert.ok(
      envelope.actionRequired?.completionBarrier?.targetCandidateFile,
      "architecture.request action must expose target completion barrier",
    );
    assert.ok(
      envelope.actionRequired?.completionBarrier?.followUpCommand,
      "architecture.request action must expose follow-up continue barrier",
    );
    assert.ok(
      envelope.actionRequired?.requiredSteps?.some((step) => String(step).includes("requestReadPlan")),
      "architecture.request action requiredSteps must mention requestReadPlan-first reads",
    );
    assert.ok(
      envelope.actionRequired?.requiredSteps?.some((step) => String(step).includes("targetCandidateFile")),
      "architecture.request action requiredSteps must mention targetCandidateFile",
    );
    assert.ok(
      envelope.actionRequired?.forbiddenStops?.some((step) => String(step).includes("progress summary")),
      "architecture.request action must forbid progress-summary stops",
    );
    assert.ok(
      envelope.actionRequired?.stopOnlyWhen?.some((step) => String(step).includes("ask_user")),
      "architecture.request action must expose stop-only conditions",
    );
    assert.ok(
      envelope.actionRequired?.finalResponseGuard?.requiredActionBeforeFinalResponse?.includes("targetCandidateFile"),
      "architecture.request final response guard must force section write before final response",
    );
    assert.ok(
      envelope.actionRequired?.summary?.includes("completionBarrier.followUpCommand.commandInvocation"),
      "architecture.request action summary must point to the structured follow-up command invocation",
    );
    assert.equal(
      envelope.actionRequired?.summary?.includes("run loom continue"),
      false,
      "architecture.request action summary must not phrase the follow-up as a user-facing continue command",
    );
    assert.equal(
      envelope.actionRequired?.summary?.includes("This follow-up command is not a user instruction"),
      true,
      "architecture.request action summary must explicitly keep the follow-up as an agent action",
    );
    assert.equal(envelope.data.instruction, undefined, "compact data must not duplicate instruction");
    assert.equal(
      envelope.instruction?.continuationContract?.agentObligation?.requiredSteps,
      undefined,
      "compact instruction must not duplicate requiredSteps",
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function assertCompactRepositoryContextRequestTransitionOutput() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-repoctx-request-compact-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2));

    run(["init"], root);
    const { deliveryId, phaseId } = writeMinimalDeliveryState(root);
    const { output, envelope } = runCompactRaw([
      "repository-context",
      "request",
      "--delivery-id",
      deliveryId,
      "--phase-id",
      phaseId,
    ], root);

    assert.ok(
      Buffer.byteLength(output, "utf8") < 18000,
      `compact repository-context.request output is too large (${Buffer.byteLength(output, "utf8")} bytes)`,
    );
    assert.equal(envelope.ok, true);
    assert.equal(envelope.compact, true);
    assert.equal(envelope.command, "repository-context.request");
    assert.equal(envelope.actionRequired?.mode, "generate_candidate");
    assert.equal(envelope.actionRequired?.autoContinue, true);
    assert.equal(envelope.actionRequired?.mustRunImmediately, true);
    assert.equal(envelope.actionRequired?.candidateFile, envelope.instruction?.candidateFile);
    assert.ok(envelope.instruction?.submitCommand?.commandInvocation, "repository-context.request must expose submit launcher invocation");
    assert.ok(envelope.actionRequired?.finalResponseGuard?.requiredActionBeforeFinalResponse?.includes("run submitCommand"));
    assert.ok(
      envelope.instruction?.requestReadProtocol?.readRule?.includes("requestReadPlan.groups"),
      "repository-context.request instruction must expose requestReadPlan-first request read protocol",
    );
    assert.equal(envelope.actionRequired?.stopRecoveryInstruction, undefined);
    assert.equal(envelope.instruction?.stopRecoveryInstruction, undefined);
    assert.equal(envelope.data?.instruction, undefined);
    assert.equal(
      JSON.stringify(envelope).includes("Run @loom continue to resume this active loom operation."),
      false,
      "repository-context.request must not expose recovery as a final-answer sentence",
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function assertCompactRecordResultTransitionOutput() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-record-result-compact-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2));
    fs.mkdirSync(projectFile(root, "src"), { recursive: true });
    fs.writeFileSync(projectFile(root, "src/example.ts"), "export const ok = true;\n");

    run(["init"], root);
    const { deliveryId, phaseId } = writeMinimalDeliveryState(root);
    const next = run(["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const nextExecutionRequest = hydrateRequest(root, readJson(projectFile(root, next.executionRequestPath)));
    const resultFile = nextExecutionRequest.outputContract.resultFile;
    writeJson(projectFile(root, resultFile), validTaskResult("result-compact"));

    const { output, envelope } = runCompactRaw([
      "record-result",
      "--delivery-id",
      deliveryId,
      "--phase-id",
      phaseId,
      "--input-file",
      resultFile,
    ], root);
    assert.ok(Buffer.byteLength(output, "utf8") < 16000, `compact record-result output is too large (${Buffer.byteLength(output, "utf8")} bytes)`);
    assert.equal(envelope.ok, true);
    assert.equal(envelope.compact, true);
    assert.equal(envelope.data.recorded, true);
    assert.equal(envelope.actionRequired.mode, "execute_task");
    assert.equal(envelope.instruction.mode, "execute_task");
    assert.equal(envelope.instruction.task.taskId, "task-002");
    assert.equal(envelope.data.instruction, undefined);
    assertCompactEnvelopeIsSmall(envelope, "compact record-result direct next task");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
  assertCompactRecordResultTransitionOutput();
  assertCompactArchitectureRequestTransitionOutput();
  assertCompactRepositoryContextRequestTransitionOutput();

  const routeRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-auto-run-route-"));
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-task-result-repair-"));
  try {
    run(["init"], routeRoot);
    const routeState = writeMinimalDeliveryState(routeRoot);
    const architectureCandidate = `.loom/deliveries/${routeState.deliveryId}/artifacts/architecture/${routeState.phaseId}/aac.json`;
    const architectureAccepted = run([
      "architecture",
      "accept",
      "--delivery-id",
      routeState.deliveryId,
      "--phase-id",
      routeState.phaseId,
      "--candidate-file",
      architectureCandidate,
    ], routeRoot, { returnEnvelope: true });
    assertTopLevelAutoRunnableAction(architectureAccepted, "run_cli", "architecture accept to task-plan request");
    assert.equal(architectureAccepted.instruction.commandInvocation.kind, "loom_user_launcher");
    assert.deepEqual(architectureAccepted.instruction.commandInvocation.argv, [
      "task-plan",
      "request",
      "--delivery-id",
      routeState.deliveryId,
      "--phase-id",
      routeState.phaseId,
    ]);
    assert.ok(
      architectureAccepted.actionRequired.finalResponseGuard.requiredActionBeforeFinalResponse.includes("Run instruction.commandInvocation"),
      "architecture accept final response guard must force the next CLI command",
    );

    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2));
    fs.mkdirSync(projectFile(root, "src"), { recursive: true });
    fs.writeFileSync(projectFile(root, "src/example.ts"), "export const ok = true;\n");

    run(["init"], root);
    const { deliveryId, phaseId } = writeMinimalDeliveryState(root);
    const next = run(["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    assert.equal(next.hasTask, true);
    assert.equal(next.task.taskId, "task-001");
    assert.equal(next.executionRequest.requestRef, next.executionRequestPath);
    assertExecuteTaskInstruction(next, "task-001", "first next-task");
    const nextExecutionRequest = hydrateRequest(root, readJson(projectFile(root, next.executionRequestPath)));
    const resultFile = nextExecutionRequest.outputContract.resultFile;
    assert.equal(
      fs.existsSync(path.dirname(projectFile(root, resultFile))),
      true,
      "next-task must precreate TaskResult parent directory so agents can write resultFile directly",
    );
    assert.equal(
      nextExecutionRequest.executionRules.sourceEditPreparationContract?.resultFile,
      resultFile,
      "TaskExecutionRequest must expose sourceEditPreparationContract for source/result writes",
    );
    assert.ok(
      nextExecutionRequest.executionRules.sourceEditPreparationContract?.sequence?.some((step) => step.name === "recover_write_tool_validation_failure"),
      "sourceEditPreparationContract must define malformed write recovery",
    );

    const invalid = validTaskResult("result-invalid");
    invalid.selfRepairSummary = {
      attempted: false,
      attemptCount: 0,
      stopReason: "verification_passed",
      progressObserved: false,
    };
    writeJson(projectFile(root, resultFile), invalid);
    const rejected = run(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", resultFile], root, { allowRecordedFalse: true });
    assert.equal(rejected.recorded, false);
    assert.equal(rejected.status, "invalid_result");
    assert.equal(rejected.nextAction.type, "task_result_repair");
    assert.equal(rejected.repairInstruction.repairSubmitRouting.followReturnedInstructionImmediately, true);
    assertTaskResultRepairInstructionProtocol(rejected.repairInstruction, "invalid TaskResult repair");

    const compactRejected = run(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", resultFile], root, { allowRecordedFalse: true, returnEnvelope: true, compact: true });
    assert.equal(compactRejected.data.recorded, false);
    assert.equal(compactRejected.instruction.mode, "repair_result_contract");
    assert.equal(compactRejected.instruction.resultFile, resultFile);
    assert.equal(compactRejected.instruction.submitCommand.name, "record-result");
    assert.ok(compactRejected.instruction.issues.some((issue) => issue.severity === "blocking"));
    assert.equal(compactRejected.instruction.repairContractProfile, "minimal_task_result_repair");
    assert.ok(Array.isArray(compactRejected.instruction.issueConflicts), "compact repair instruction must expose issue conflicts.");
    assert.ok(Array.isArray(compactRejected.instruction.minimalRepairRules), "compact repair instruction must expose minimal repair rules.");
    assert.ok(compactRejected.instruction.inspectRepairContractCommand, "compact repair instruction must expose on-demand inspect command.");
    assert.ok(compactRejected.instruction.instructions.some((rule) => rule.includes("Never write reason: null")));
    assert.equal(compactRejected.instruction.schemaShape, undefined);
    assert.equal(compactRejected.data.repairInstruction?.fullInstructionLocation, "top-level instruction");
    assert.equal(compactRejected.data.repairInstruction?.issueConflictCount, compactRejected.instruction.issueConflicts.length);
    assert.equal(JSON.stringify(compactRejected).includes("workflowClosureRequirements"), false, "compact generic TaskResult repair must not include large workflow closure schema.");
    const compactRepairEnvelopeSize = Buffer.byteLength(JSON.stringify(compactRejected), "utf8");
    const compactRepairInstructionBreakdown = Object.fromEntries(
      Object.entries(compactRejected.instruction ?? {})
        .map(([key, value]) => [key, Buffer.byteLength(JSON.stringify(value), "utf8")])
        .sort((a, b) => b[1] - a[1])
        .slice(0, 8),
    );
    const compactRepairEnvelopeBreakdown = Object.fromEntries(
      Object.entries(compactRejected)
        .map(([key, value]) => [key, Buffer.byteLength(JSON.stringify(value), "utf8")])
        .sort((a, b) => b[1] - a[1])
    );
    const compactRepairDataBreakdown = Object.fromEntries(
      Object.entries(compactRejected.data ?? {})
        .map(([key, value]) => [key, Buffer.byteLength(JSON.stringify(value), "utf8")])
        .sort((a, b) => b[1] - a[1])
        .slice(0, 8)
    );
    assert.ok(compactRepairEnvelopeSize < 16000, `compact repair_result_contract envelope must stay small, got ${compactRepairEnvelopeSize} bytes; envelope ${JSON.stringify(compactRepairEnvelopeBreakdown)}; data ${JSON.stringify(compactRepairDataBreakdown)}; instruction ${JSON.stringify(compactRepairInstructionBreakdown)}.`);

    writeJson(projectFile(root, resultFile), validTaskResult("result-fixed"));
    const acceptedEnvelope = run(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", resultFile], root, { returnEnvelope: true });
    assertTopLevelActionRequired(acceptedEnvelope, "task-002", "record-result direct next task");
    assert.ok(acceptedEnvelope.actionRequired.summary.includes("TaskResult was recorded"));
    assert.ok(acceptedEnvelope.actionRequired.summary.includes("next TaskExecutionRequest is already created"));
    const accepted = acceptedEnvelope.data;
    assert.equal(accepted.recorded, true);
    assert.equal(accepted.status, "completed");
    assert.equal(accepted.nextAction.type, "continue_execution");
    assert.equal(accepted.materializedNextTask.hasTask, true);
    assert.equal(accepted.materializedNextTask.task.taskId, "task-002");
    assertExecuteTaskInstruction(accepted.materializedNextTask, "task-002", "record-result direct next task");
    assert.equal(accepted.instruction.mode, "execute_task");
    assert.equal(accepted.instruction.continuationContract.source.command, "record-result");
    assert.equal(accepted.instruction.continuationContract.source.succeeded, true);
    assert.equal(accepted.instruction.continuationContract.agentObligation.primaryAction, "execute_materialized_next_task");
    assert.ok(acceptedEnvelope.actionRequired.forbiddenStops.some((rule) => rule.includes("progress summary")));
    assert.equal(accepted.instruction.requestRef, accepted.materializedNextTask.executionRequestPath);
    assert.equal(accepted.postRepairSubmitRouting.repairedSubmitSucceeded, true);
    assert.equal(accepted.postRepairSubmitRouting.followReturnedInstructionImmediately, true);

    const runState = readJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/run-001.json`));
    assert.equal(runState.taskStates[0].status, "completed");
    assert.equal(runState.taskStates[1].status, "running");
    assert.equal(runState.groupStates[0].status, "completed");
    assert.equal(runState.groupStates[1].status, "running");
    assert.equal(runState.nextAction.type, "continue_execution");
    assert.equal(runState.nextAction.sourceTaskId, "task-002");

    const second = accepted.materializedNextTask;
    const secondLeaseBefore = readJson(projectFile(root, `.loom/deliveries/${deliveryId}/operations/active-lease.json`));
    assert.equal(secondLeaseBefore.operationType, "task_execution");
    assert.equal(secondLeaseBefore.refs.taskId, "task-002");
    assert.equal(secondLeaseBefore.refs.taskPlanRunId, "run-001");

    const indexAfterMaterialization = readJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`));
    const phaseAfterMaterialization = indexAfterMaterialization.phases.find((phase) => phase.phaseId === phaseId);
    assert.equal(phaseAfterMaterialization.nextAction.type, "continue_execution");
    assert.equal(phaseAfterMaterialization.nextAction.ref, second.executionRequestPath);
    assert.equal(phaseAfterMaterialization.nextAction.refs.taskId, "task-002");
    assert.equal(phaseAfterMaterialization.nextAction.refs.taskPlanRunId, "run-001");
    assert.equal(phaseAfterMaterialization.nextAction.refs.executionRequestRef, second.executionRequestPath);
    assert.equal(phaseAfterMaterialization.nextAction.refs.resultFile, second.executionRequest.resultFile);
    assert.equal(phaseAfterMaterialization.nextAction.refs.activeOperationType, "task_execution");
    const statusAfterMaterialization = readJson(projectFile(root, ".loom/status.json"));
    assert.equal(statusAfterMaterialization.nextAction, "execute-task");
    assert.equal(statusAfterMaterialization.effectiveNextAction.ref, second.executionRequestPath);
    assert.equal(statusAfterMaterialization.effectiveNextAction.refs.taskId, "task-002");
    assert.equal(statusAfterMaterialization.effectiveNextAction.refs.executionRequestRef, second.executionRequestPath);

    const stale = run(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", resultFile], root, { allowRecordedFalse: true });
    assert.equal(stale.recorded, false);
    assert.equal(stale.status, "invalid_result");
    assert.equal(stale.issues[0].code, "TASK_RESULT_REF_INVALID");
    assert.equal(stale.nextAction.type, "continue_execution");
    assert.equal(stale.nextAction.sourceTaskId, "task-002");
    assert.equal(stale.instruction.mode, "execute_task");
    assert.equal(stale.instruction.requestRef, second.executionRequestPath);

    const secondLeaseAfter = readJson(projectFile(root, `.loom/deliveries/${deliveryId}/operations/active-lease.json`));
    assert.equal(secondLeaseAfter.status, "active");
    assert.equal(secondLeaseAfter.operationType, "task_execution");
    assert.equal(secondLeaseAfter.refs.taskId, "task-002");

    const continued = run(["continue"], root);
    assert.equal(continued.status, "ready");
    assert.equal(continued.nextAction.type, "continue_execution");
    assert.equal(continued.instruction.mode, "execute_task");
    assert.equal(continued.instruction.autoContinue, true);
    assert.equal(continued.instruction.mustStartImmediately, true);
    assert.equal(continued.instruction.mustRunImmediately, true);
    assert.equal(continued.instruction.mustNotAskUserBeforeExecuting, true);
    assert.equal(continued.instruction.requestRef, second.executionRequestPath);
    assert.equal(continued.instruction.resultFile, second.executionRequest.resultFile);
    assert.deepEqual(continued.instruction.submitCommand.argv, second.executionRequest.submitCommand.argv);
    assert.equal(continued.instruction.submitCommand.commandInvocation.kind, "loom_user_launcher");
    assert.equal(continued.instruction.completionBarrier.rules, undefined);
    assert.equal(continued.instruction.userMessage.includes("@loom continue"), false);
    assert.equal(continued.instruction.routingRule.includes("@loom continue"), false);
    assert.equal(continued.instruction.stopRecoveryInstruction, undefined);
    assert.equal(continued.instruction.task.taskId, "task-002");
    assert.equal(continued.instruction.recovery, true);
    assert.equal(continued.possibleRuntimeForegroundStall?.applies, true);
    assert.equal(continued.possibleRuntimeForegroundStall?.evidence?.processScanPerformed, false);
    assert.equal(continued.instruction.possibleRuntimeForegroundStall?.agentInstruction?.primaryAction, "resume_current_task_and_close_runtime_probe");

    const compactContinue = runCompact(["continue"], root);
    assert.equal(compactContinue.ok, true);
    assert.equal(compactContinue.compact, true);
    assertCompactEnvelopeIsSmall(compactContinue, "compact continue recovered active task");
    assert.equal(compactContinue.actionRequired.mode, "execute_task");
    assert.equal(compactContinue.actionRequired.summary.includes("@loom continue"), false);
    assert.equal(compactContinue.actionRequired.completionBarrier.resultFile, second.executionRequest.resultFile);
    assert.ok(compactContinue.actionRequired.finalResponseGuard.invalidFinalResponseWhen.includes(second.executionRequest.resultFile));
    assert.equal(compactContinue.instruction.mode, "execute_task");
    assert.equal(compactContinue.instruction.requestRef, second.executionRequestPath);
    assert.equal(compactContinue.instruction.resultFile, second.executionRequest.resultFile);
    assertAutoRunnableContinuationContract(compactContinue.instruction, "compact continue recovered active task");
    assert.equal(compactContinue.instruction.stopRecoveryInstruction, undefined);
    assert.equal(compactContinue.instruction.possibleRuntimeForegroundStall?.applies, true);
    assert.equal(compactContinue.instruction.possibleRuntimeForegroundStall?.evidence?.processScanPerformed, false);
    assert.equal(compactContinue.instruction.task.taskId, "task-002");
    assert.equal(compactContinue.data.nextAction.type, "continue_execution");
    assert.equal(compactContinue.data.instruction, undefined);
    assert.equal(compactContinue.data.concurrency, undefined);
    assert.equal(compactContinue.instruction.agentAction, undefined);
    assert.equal(compactContinue.instruction.executionRules, undefined);

    const status = run(["status"], root);
    assert.equal(status.activeOperation?.operationType, "task_execution");
    assert.equal(status.activeOperation?.possibleRuntimeForegroundStall?.applies, true);
    assert.equal(status.activeOperation?.possibleRuntimeForegroundStall?.evidence?.processScanPerformed, false);
    assert.ok(
      status.activeOperation?.resumeCommand?.guidance?.includes("not exit by itself"),
      "status must explain possible runtime foreground stall in user-readable recovery guidance",
    );
    const compactStatus = runCompact(["status"], root);
    assert.equal(compactStatus.data.initialized, true);
    assert.equal(compactStatus.data.activeDeliveryId, deliveryId);
    assert.equal(compactStatus.data.activePhaseId, phaseId);
    assert.equal(compactStatus.data.deliveryStatus, "executing");
    assert.equal(compactStatus.data.effectiveNextAction?.type, "continue_execution");
    assert.equal(compactStatus.data.effectiveNextAction?.deliveryId, deliveryId);
    assert.equal(compactStatus.data.effectiveNextAction?.phaseId, phaseId);
    assert.equal(compactStatus.data.effectiveNextAction?.ref, second.executionRequestPath);
    assert.equal(compactStatus.data.activeOperation?.possibleRuntimeForegroundStall?.applies, true);
    assert.equal(compactStatus.data.activeOperation?.possibleRuntimeForegroundStall?.evidence?.processScanPerformed, false);
    assert.ok(
      compactStatus.data.activeOperation?.resumeCommand?.guidance?.includes("not exit by itself"),
      "compact status must preserve runtime-stall recovery guidance",
    );

    const defaultContinue = runDefaultOutput(["continue"], root);
    assert.equal(defaultContinue.ok, true);
    assert.equal(defaultContinue.compact, true);
    assert.equal(defaultContinue.instruction.mode, "execute_task");
    assert.equal(defaultContinue.data.instruction, undefined);

    assertNoLegacyContinuationProtocol();
    console.log("auto-runnable transition contract verification passed");
  } finally {
    fs.rmSync(routeRoot, { recursive: true, force: true });
    fs.rmSync(root, { recursive: true, force: true });
  }
}

main();

function assertNoLegacyContinuationProtocol() {
  const checkedRoots = ["src", "plugins", "scripts"].map((item) => path.join(repoRoot, item));
  const forbidden = [
    "postSubmitContinuation",
    "autoMaterializedBy",
    "record-result succeeded",
    "After any `record-result`",
    "This is not a stopping point",
  ];
  for (const file of listFiles(checkedRoots)) {
    const text = fs.readFileSync(file, "utf8");
    for (const needle of forbidden) {
      assert.equal(text.includes(needle), false, `${path.relative(repoRoot, file)} must not contain legacy continuation protocol ${needle}`);
    }
  }
  const autoFlagMatches = [];
  for (const file of listFiles([path.join(repoRoot, "src")])) {
    const relative = path.relative(repoRoot, file);
    if (relative === "core/operations/routing-instructions.ts") continue;
    const text = fs.readFileSync(file, "utf8");
    if (text.includes("autoContinue: true") || text.includes("mustRunImmediately: true")) {
      autoFlagMatches.push(relative);
    }
  }
  assert.deepEqual(autoFlagMatches, [], "auto-runnable flags must be emitted through routing-instructions builder only");
}

function listFiles(roots) {
  const output = [];
  for (const root of roots) {
    if (!fs.existsSync(root)) continue;
    for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
      const entryPath = path.join(root, entry.name);
      if (entry.name === "dist" || entry.name === "node_modules") continue;
      if (entry.isDirectory()) {
        output.push(...listFiles([entryPath]));
      } else if (entry.isFile() && /\.(ts|js|md|json)$/.test(entry.name)) {
        output.push(entryPath);
      }
    }
  }
  return output;
}
