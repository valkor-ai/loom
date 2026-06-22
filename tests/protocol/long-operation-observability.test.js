#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
const cli = path.join(repoRoot, "dist", "cli.js");

function run(args, projectRoot, expectOk = true) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, expectOk, `${args.join(" ")} unexpected result: ${output}`);
  return envelope;
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
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

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function writeDeliveryState(root) {
  const deliveryId = "delivery-long-op";
  const phaseId = "phase-1";
  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "planning",
      requestSummary: "Verify long operation observability.",
      activePhaseId: phaseId,
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now(),
    }],
    effectiveNextAction: null,
    phase: "planning",
    current: { requirementId: null, planId: null, taskId: null, reviewId: null, repairId: null, deploymentId: null },
    lastAction: null,
    nextAction: "plan",
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Verify long operation observability.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "planning",
      latestRefs: {},
      nextAction: null,
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
    stack: { runtime: "node", language: "typescript", packageManager: "npm", test: "npm test" },
    constraints: [],
    evidence: [{ reason: "fixture" }],
    approval: { type: "policy_auto_accept", reason: "fixture" },
    confidence: "high",
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-001",
    status: "ready",
    source: {
      brainstormRunId: "bs-001",
      brainstormContractId: "bc-001",
      roadmapId: null,
      phaseId,
      technicalBaselineId: "tb-001",
    },
    phaseScope: {
      phaseName: "Phase 1",
      phaseGoal: "Verify long operation observability.",
      included: [{ scopeId: "scope-001", label: "Core", items: ["core"], source: "fixture" }],
      deferred: [],
      excluded: [],
      acceptanceCandidates: [{ id: "AC-001", statement: "Core status rule is covered.", priority: "must" }],
    },
    technicalBaseline: { technicalBaselineId: "tb-001", status: "confirmed", scope: "project", summary: {}, mustFollow: true },
    planningInputs: { businessGoal: "Verify.", actors: [], capabilityGroups: [], businessFlows: [], sourceRefs: [], contextNotes: [] },
    planningRules: {
      scopeIsolation: { onlyPlanCurrentPhase: true, forbidDeferredScopeImplementation: true, forbidFuturePhaseImplementation: true },
      outputRequirements: { mustCreateArchitectureArtifactContract: true, mustCreateTaskPlan: true, taskPlanMustReferenceAcceptance: true },
      deployment: { defaultEnabled: false, requiresExplicitUserRequest: true },
    },
    qualityGates: { requiresArchitectureBeforeTaskPlan: true, requiresAcceptanceCoverage: true, requiresVerificationEvidence: true },
    handoff: { readyForArchitecture: true, readyForTaskPlan: false, blockingReasons: [], nextNode: "architecture_artifact_contract" },
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-001",
    contractRef: `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`,
    updatedAt: now(),
  });
  return { deliveryId, phaseId };
}

function sectionCandidate(request, section, content) {
  return {
    schemaVersion: "1.0",
    requestId: request.requestId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    section,
    status: "ready",
    content,
    blockedReasons: [],
    createdAt: now(),
  };
}

function taskPlanOutline(request) {
  return {
    schemaVersion: "1.0",
    requestId: request.requestId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    taskPlanId: "taskplan-long-op",
    groups: [
      {
        groupId: "group-core",
        title: "Core",
        objective: "Core work.",
        dependsOn: [],
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        taskIds: ["task-core"],
      },
      {
        groupId: "group-tests",
        title: "Tests",
        objective: "Test work.",
        dependsOn: ["group-core"],
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        taskIds: ["task-tests"],
      },
    ],
    createdAt: now(),
  };
}

function taskPlanGroupCandidate(request, group) {
  return {
    schemaVersion: "1.0",
    requestId: request.requestId,
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    group,
    tasks: [],
    createdAt: now(),
  };
}

function writeActiveLease(root, deliveryId, phaseId, operationType, refs = {}, expiresAt = "2999-01-01T00:00:00.000Z") {
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/operations/active-lease.json`), {
    schemaVersion: "1.0",
    operationId: `op-${operationType}`,
    deliveryId,
    phaseId,
    operationType,
    status: "active",
    startedAt: now(),
    heartbeatAt: now(),
    expiresAt,
    refs,
  });
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-long-op-"));
  try {
    run(["init"], root);
    const { deliveryId, phaseId } = writeDeliveryState(root);

    const help = execFileSync(process.execPath, [cli, "--help"], { cwd: repoRoot, encoding: "utf8" });
    assert.equal(help.includes("heartbeat"), false, "Agent-facing heartbeat command must not be registered.");

    const arch = run(["architecture", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root).data;
    assert.equal(arch.lease.heartbeatCommand, undefined);
    assert.equal(arch.lease.progressSignal, "candidate_files");
    assert.equal(arch.request, undefined);
    assert.equal(Object.prototype.hasOwnProperty.call(arch, "requestSummary"), false);
    assert.equal(arch.instruction.requestRef, arch.requestPath);
    const archRequest = hydrateRequest(root, readJson(projectFile(root, arch.requestPath)));
    assert.equal(archRequest.generationProtocol.progressSignal, "candidate_files");
    assert.equal(archRequest.generationProtocol.heartbeatRequired, false);
    assert.equal(archRequest.agentAction.write.currentTarget.section, "foundation");
    assert.equal(archRequest.agentAction.write.currentTarget.candidateFile, archRequest.outputContract.sectionOutputs[0].candidateFile);
    assert.ok(archRequest.agentAction.write.currentTarget.schemaShape, "Current section target must carry its schemaShape.");
    assert.equal(
      archRequest.agentAction.read.fieldGroups.some((group) => group.fields.includes("outputContract.sectionOutputs")),
      false,
      "Single-section architecture read plan must not include the full sectionOutputs schema set.",
    );
    assert.ok(
      archRequest.agentAction.read.fieldGroups.some((group) => group.fields.includes("agentAction.write.currentTarget")),
      "Architecture agentAction read plan must expose the current single-section target as a fieldGroup",
    );
    const initialTargetInspect = run(["inspect", "--request", arch.requestPath, "--field", "agentAction.write.currentTarget"], root).data.fields["agentAction.write.currentTarget"].value;
    assert.equal(initialTargetInspect.section, "foundation");
    assert.equal(initialTargetInspect.candidateFile, archRequest.outputContract.sectionOutputs[0].candidateFile);

    const sectionOutputs = archRequest.outputContract.sectionOutputs;
    const foundationOutput = sectionOutputs.find((output) => output.section === "foundation");
    writeJson(projectFile(root, foundationOutput.candidateFile), sectionCandidate(archRequest, "foundation", {
      source: { planningGenerationContractId: "pgc-001", technicalBaselineId: "tb-001", brainstormContractId: "bc-001", roadmapId: null, phaseId },
      engineeringBoundary: {
        projectKind: "existing_project",
        strategy: "extend_existing_modules",
        applications: [{ appId: "app-main", type: "library", root: "." }],
        modules: [{ moduleId: "module-core", appId: "app-main", paths: ["src"], responsibility: "Core.", layerMappings: [] }],
        creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
      },
      modules: [{ moduleId: "module-core", name: "Core", responsibility: "Core.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"] }],
    }));

    const status = run(["status"], root).data;
    assert.equal(status.activeOperation.progressSignal, "candidate_files");
    assert.deepEqual(status.activeOperation.progress.completedSections, ["foundation"]);
    assert.ok(status.activeOperation.progress.missingSections.includes("coverage"));

    const decisionEnvelope = run(["continue"], root);
    assert.equal(decisionEnvelope.actionRequired?.mode, "generate_candidate");
    assert.equal(decisionEnvelope.actionRequired?.summary.includes("write only the"), true);
    assert.equal(decisionEnvelope.actionRequired?.summary.includes("completionBarrier.followUpCommand.commandInvocation"), true);
    assert.equal(decisionEnvelope.actionRequired?.summary.includes("run loom continue"), false);
    assert.equal(decisionEnvelope.actionRequired?.summary.includes("run submitCommand now"), false);
    const decision = decisionEnvelope.data;
    assert.equal(decision.instruction.mode, "generate_candidate");
    assert.equal(decision.instruction.mustRunImmediately, true);
    assert.equal(decision.instruction.mustNotAskUserBeforeExecuting, true);
    assert.equal(decision.instruction.mustNotReportProgressBeforeExecuting, true);
    assert.ok(decision.instruction.routingRule.includes("completionBarrier.followUpCommand.commandInvocation"));
    assert.equal(Object.prototype.hasOwnProperty.call(decision.instruction, "generationSteps"), false);
    const storedArchRequest = hydrateRequest(root, readJson(projectFile(root, arch.requestPath)));
    assert.equal(storedArchRequest.agentAction.write.currentTarget.section, "domain_contract");
    assert.equal(storedArchRequest.agentAction.write.currentTarget.candidateFile, decision.instruction.targetCandidateFile);
    assert.ok(storedArchRequest.agentAction.write.currentTarget.schemaShape, "Recovered current section target must carry its schemaShape.");
    assert.equal(
      storedArchRequest.agentAction.read.fieldGroups.some((group) => group.fields.includes("outputContract.sectionOutputs")),
      false,
      "Recovered single-section architecture read plan must not include the full sectionOutputs schema set.",
    );
    const resumedTargetInspect = run(["inspect", "--request", arch.requestPath, "--field", "agentAction.write.currentTarget"], root).data.fields["agentAction.write.currentTarget"].value;
    assert.equal(resumedTargetInspect.section, "domain_contract");
    assert.equal(resumedTargetInspect.candidateFile, decision.instruction.targetCandidateFile);
    assert.ok(resumedTargetInspect.schemaShape, "Inspect currentTarget must return the current section schemaShape.");
    assert.equal(
      storedArchRequest.agentAction.write.rules.some((rule) => rule === "Write one candidate per outputContract.sectionOutputs[] entry."),
      false,
    );
    assert.ok(storedArchRequest.agentAction.write.rules.some((rule) => rule.includes("Single-section protocol")));
    assert.ok(storedArchRequest.agentAction.write.rules.some((rule) => rule.includes("completionBarrier.followUpCommand.commandInvocation")));
    assert.equal(decision.instruction.outputSummary, undefined);
    assert.equal(decisionEnvelope.instruction.outputSummary, undefined);
    assert.equal(decisionEnvelope.instruction.submitCommand, undefined);
    assert.equal(decisionEnvelope.actionRequired?.submitCommand, undefined);
    assert.equal(decisionEnvelope.actionRequired?.completionBarrier?.targetCandidateFile, decision.instruction.targetCandidateFile);
    assert.deepEqual(decisionEnvelope.actionRequired?.completionBarrier?.followUpCommand?.argv, ["continue"]);
    assert.equal(decisionEnvelope.actionRequired?.completionBarrier?.followUpCommand?.commandInvocation?.kind, "loom_user_launcher");
    assert.equal(decisionEnvelope.actionRequired?.completionBarrier?.rules, undefined);
    assert.ok(decisionEnvelope.actionRequired?.finalResponseGuard?.invalidFinalResponseWhen?.includes("completionBarrier.followUpCommand"));
    assert.ok(decisionEnvelope.actionRequired?.requiredSteps?.some((step) => step.includes("completionBarrier.followUpCommand")));
    assert.ok(decisionEnvelope.actionRequired?.summary?.includes("completionBarrier.followUpCommand.commandInvocation"));
    assert.equal(decisionEnvelope.actionRequired?.summary?.includes("run loom continue"), false);
    assert.equal(JSON.stringify(decision).includes("heartbeatCommand"), false);

    writeActiveLease(root, deliveryId, phaseId, "architecture_generation", {
      requestRef: arch.requestPath,
      sectionOutputs: sectionOutputs.map((output) => ({
        section: output.section,
        schemaRef: output.schemaRef,
        candidateFile: output.candidateFile,
      })),
    }, "2000-01-01T00:00:00.000Z");
    const expiredEnvelope = run(["continue"], root);
    assert.equal(expiredEnvelope.actionRequired?.mode, "generate_candidate");
    assert.equal(expiredEnvelope.actionRequired?.mustRunImmediately, true);
    const expiredDecision = expiredEnvelope.data;
    assert.equal(expiredDecision.instruction.mode, "generate_candidate");
    assert.equal(expiredDecision.instruction.autoContinue, true);
    assert.equal(expiredDecision.instruction.mustRunImmediately, true);
    assert.equal(expiredDecision.instruction.requestRef, arch.requestPath);
    assert.equal(expiredDecision.instruction.outputSummary, undefined);
    assert.equal(expiredEnvelope.instruction.submitCommand, undefined);
    assert.equal(expiredEnvelope.actionRequired?.submitCommand, undefined);
    assert.deepEqual(expiredEnvelope.actionRequired?.completionBarrier?.followUpCommand?.argv, ["continue"]);
    assert.equal(expiredEnvelope.actionRequired?.completionBarrier?.followUpCommand?.commandInvocation?.kind, "loom_user_launcher");
    assert.ok(expiredEnvelope.actionRequired?.finalResponseGuard?.invalidFinalResponseWhen?.includes("completionBarrier.followUpCommand"));
    assert.equal(expiredDecision.concurrency.staleLeaseRecovered, true);
    assert.equal(expiredDecision.concurrency.recoverableActiveOperation, true);
    const refreshedLease = readJson(projectFile(root, `.loom/deliveries/${deliveryId}/operations/active-lease.json`));
    assert.equal(refreshedLease.status, "active");
    assert.equal(refreshedLease.refs.requestRef, arch.requestPath);
    assert.ok(new Date(refreshedLease.expiresAt).getTime() > Date.now());

    const taskPlanRequestPath = `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplan-requests/taskplan-gen-long-op.json`;
    const taskPlanRequestBody = {
      schemaVersion: "1.0",
      requestId: "taskplan-gen-long-op",
      requestType: "taskplan_grouped_generation",
      deliveryId,
      phaseId,
      outputContract: {
        outlineFile: `.loom/deliveries/${deliveryId}/tmp/${phaseId}/taskplans/taskplan-gen-long-op/outline.json`,
        groupFilePattern: `.loom/deliveries/${deliveryId}/tmp/${phaseId}/taskplans/taskplan-gen-long-op/groups/{groupId}.json`,
      },
      submitCommand: {
        name: "task-plan accept",
        argv: ["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", "taskplan-gen-long-op"],
      },
    };
    writeJson(projectFile(root, taskPlanRequestPath), taskPlanRequestBody);
    writeActiveLease(root, deliveryId, phaseId, "taskplan_generation", {
      requestRef: taskPlanRequestPath,
      outlineFile: taskPlanRequestBody.outputContract.outlineFile,
      groupFilePattern: taskPlanRequestBody.outputContract.groupFilePattern,
    });
    const outline = taskPlanOutline(taskPlanRequestBody);
    writeJson(projectFile(root, taskPlanRequestBody.outputContract.outlineFile), outline);
    writeJson(
      projectFile(root, taskPlanRequestBody.outputContract.groupFilePattern.replace("{groupId}", "group-core")),
      taskPlanGroupCandidate(taskPlanRequestBody, outline.groups[0]),
    );
    const taskPlanStatus = run(["status"], root).data;
    assert.equal(taskPlanStatus.activeOperation.operationType, "taskplan_generation");
    assert.equal(taskPlanStatus.activeOperation.progress.outline.status, "written");
    assert.equal(taskPlanStatus.activeOperation.progress.groupTotal, 2);
    assert.equal(taskPlanStatus.activeOperation.progress.completedGroupCount, 1);
    assert.deepEqual(taskPlanStatus.activeOperation.progress.completedGroups, ["group-core"]);
    assert.deepEqual(taskPlanStatus.activeOperation.progress.missingGroups, ["group-tests"]);
    assert.equal(taskPlanStatus.activeOperation.progress.recommendedAction, "write_missing_groups");
    assert.ok(taskPlanStatus.activeOperation.progress.summary.includes("1/2"));

    const taskPlanContinue = run(["continue"], root).data;
    assert.equal(taskPlanContinue.instruction.mode, "generate_candidate");
    assert.equal(taskPlanContinue.instruction.outputSummary.progress.groupTotal, 2);
    assert.equal(taskPlanContinue.instruction.outputSummary.progress.completedGroupCount, 1);
    assert.deepEqual(taskPlanContinue.instruction.outputSummary.progress.missingGroups, ["group-tests"]);
    assert.equal(taskPlanContinue.instruction.outputSummary.progress.recommendedAction, "write_missing_groups");

    writeJson(
      projectFile(root, taskPlanRequestBody.outputContract.groupFilePattern.replace("{groupId}", "group-tests")),
      taskPlanGroupCandidate(taskPlanRequestBody, outline.groups[1]),
    );
    const completeTaskPlanContinue = run(["continue"], root).data;
    assert.equal(completeTaskPlanContinue.instruction.mode, "submit_existing_candidate");
    assert.equal(completeTaskPlanContinue.instruction.existingOutputs.progress.groupTotal, 2);
    assert.equal(completeTaskPlanContinue.instruction.existingOutputs.progress.completedGroupCount, 2);
    assert.deepEqual(completeTaskPlanContinue.instruction.existingOutputs.progress.missingGroups, []);
    assert.equal(completeTaskPlanContinue.instruction.existingOutputs.progress.recommendedAction, "submit_accept");

    writeActiveLease(root, deliveryId, phaseId, "task_execution", {
      requestRef: ".loom/deliveries/delivery-long-op/tasks/phase-1/execution-requests/exec-001.json",
      resultFile: ".loom/deliveries/delivery-long-op/tmp/phase-1/task-results/exec-001/result.json",
      taskId: "task-001",
      taskPlanRunId: "run-001",
    });
    assert.equal(run(["status"], root).data.activeOperation.progressSignal, "result_file");

    writeActiveLease(root, deliveryId, phaseId, "repository_context_generation", {
      requestRef: ".loom/deliveries/delivery-long-op/workspace/phase-1/repository-context-requests/repo-ctx-001.json",
      candidateFile: ".loom/deliveries/delivery-long-op/tmp/phase-1/repository-context/repo-ctx-001/candidate.json",
    });
    assert.equal(run(["status"], root).data.activeOperation.progressSignal, "candidate_file");

    writeActiveLease(root, deliveryId, phaseId, "execution_repair", {
      requestRef: ".loom/deliveries/delivery-long-op/repairs/phase-1/execution/repair-001.json",
      candidateFile: ".loom/deliveries/delivery-long-op/tmp/phase-1/repairs/execution/repair-001/result.json",
    });
    assert.equal(run(["status"], root).data.activeOperation.progressSignal, "project_files_and_result_file");

    console.log("long operation observability verification passed");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

main();
