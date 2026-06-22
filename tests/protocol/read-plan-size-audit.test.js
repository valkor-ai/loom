#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../..");
const cli = path.join(repoRoot, "dist", "cli.js");

let hydrateRequestManifest;

function now() {
  return "2026-06-04T00:00:00.000Z";
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function run(args, projectRoot, options = {}) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: options.compact ? "1" : process.env.LOOM_COMPACT_OUTPUT },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return options.envelope ? envelope : envelope.data;
}

async function loadRequest(projectRoot, requestRef) {
  if (!hydrateRequestManifest) {
    ({ hydrateRequestManifest } = require("../../dist/core/operations/request-manifest"));
  }
  return await hydrateRequestManifest(projectRoot, projectFile(projectRoot, requestRef));
}

async function requestFromCommand(data, projectRoot) {
  if (data.request) {
    return data.request;
  }
  const requestRef = data.requestPath ??
    data.requestRef ??
    data.executionRequestPath ??
    data.instruction?.requestRef;
  assert.ok(requestRef, `command result must expose requestRef/requestPath: ${JSON.stringify(data, null, 2)}`);
  return await loadRequest(projectRoot, requestRef);
}

function fieldCovers(parent, child) {
  return child.startsWith(`${parent}.`);
}

function coveredPairs(fields) {
  const pairs = [];
  for (const parent of fields) {
    for (const child of fields) {
      if (parent !== child && fieldCovers(parent, child)) {
        pairs.push([parent, child]);
      }
    }
  }
  return pairs;
}

function allReadFields(agentAction) {
  return agentAction.read.fieldGroups.flatMap((group) => group.fields);
}

function readGroup(request, groupId) {
  return request.agentAction.read.fieldGroups.find((group) => group.groupId === groupId);
}

function auditReadPlan(label, request, options = {}) {
  const agentAction = request.agentAction;
  assert.ok(agentAction, `${label}: request must expose agentAction`);
  assert.ok(agentAction.read, `${label}: agentAction.read missing`);
  assert.equal(agentAction.read.primaryMethod, "inspect", `${label}: read.primaryMethod must be inspect`);
  assert.equal(agentAction.read.fallbackMethod, "request_manifest_refs", `${label}: read.fallbackMethod must be request_manifest_refs`);
  assert.equal(agentAction.read.fields, undefined, `${label}: legacy read.fields must not be emitted`);
  assert.ok(Array.isArray(agentAction.read.fieldGroups), `${label}: read.fieldGroups missing`);
  assert.ok(agentAction.read.fieldGroups.length > 0, `${label}: read.fieldGroups must not be empty`);
  assert.ok(request.requestReadPlan, `${label}: request root must expose requestReadPlan before sidecar fallback`);
  assert.equal(request.requestReadPlan.authority, "agentAction.read.fieldGroups", `${label}: requestReadPlan must mirror agentAction fieldGroups`);
  assert.ok(Array.isArray(request.requestReadPlan.groups), `${label}: requestReadPlan.groups missing`);
  assert.equal(request.requestReadPlan.groups.length, agentAction.read.fieldGroups.length, `${label}: requestReadPlan groups must mirror fieldGroups`);
  assert.equal(JSON.stringify(request.requestReadPlan).includes("{requestRef}"), false, `${label}: requestReadPlan commands must use the concrete requestRef, not a placeholder`);

  const globalFields = [];
  for (const [index, group] of agentAction.read.fieldGroups.entries()) {
    assert.equal(typeof group.groupId, "string", `${label}: fieldGroup.groupId missing`);
    assert.equal(typeof group.required, "boolean", `${label}.${group.groupId}: fieldGroup.required missing`);
    assert.ok(Array.isArray(group.fields), `${label}.${group.groupId}: fields must be array`);
    assert.ok(group.fields.length > 0, `${label}.${group.groupId}: fields must not be empty`);
    assert.deepEqual(group.fields, [...new Set(group.fields)], `${label}.${group.groupId}: duplicate fields in group`);
    assert.deepEqual(coveredPairs(group.fields), [], `${label}.${group.groupId}: parent/child fields must not share a group`);
    assert.equal(group.readCommand?.name, "inspect", `${label}.${group.groupId}: readCommand must use inspect`);
    assert.deepEqual(
      group.readCommand?.argv,
      ["inspect", "--request", "{requestRef}", "--field", group.fields.join(",")],
      `${label}.${group.groupId}: readCommand argv must match fields exactly`,
    );
    const rootPlanGroup = request.requestReadPlan.groups[index];
    assert.equal(rootPlanGroup.groupId, group.groupId, `${label}.${group.groupId}: requestReadPlan groupId must match agentAction`);
    assert.deepEqual(rootPlanGroup.fields, group.fields, `${label}.${group.groupId}: requestReadPlan fields must match agentAction`);
    assert.deepEqual(
      rootPlanGroup.readCommand?.argv,
      ["inspect", "--request", request.requestReadPlan.requestRef, "--field", group.fields.join(",")],
      `${label}.${group.groupId}: requestReadPlan readCommand must target the concrete requestRef`,
    );
    globalFields.push(...group.fields);
  }

  assert.deepEqual(globalFields, [...new Set(globalFields)], `${label}: fields must not repeat across groups`);
  assert.deepEqual(coveredPairs(globalFields), [], `${label}: parent/child fields must not be split across groups`);

  for (const field of options.requiredFields ?? []) {
    assert.ok(globalFields.includes(field), `${label}: expected read field ${field}`);
  }
  for (const field of options.forbiddenFields ?? []) {
    assert.equal(globalFields.includes(field), false, `${label}: must not read broad field ${field}`);
  }
}

function writePlanningFixture(root) {
  const deliveryId = "delivery-read-plan-audit";
  const phaseId = "phase-1";
  const aacRef = `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`;
  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "planning",
      requestSummary: "Audit read plan size.",
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
    requestSummary: "Audit read plan size.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "planning",
      latestRefs: {
        architectureArtifactContract: aacRef,
      },
      nextAction: null,
    }],
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), technicalBaseline());
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`), planningContract(deliveryId, phaseId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-read-plan",
    contractRef: `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`,
    updatedAt: now(),
  });
  writeJson(projectFile(root, aacRef), architectureContract(deliveryId, phaseId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-read-plan",
    contractRef: aacRef,
    planningContractId: "pgc-read-plan",
    updatedAt: now(),
  });
  return { deliveryId, phaseId };
}

function writeExecutionFixture(root) {
  const deliveryId = "delivery-read-plan-execution";
  const phaseId = "phase-1";
  const aacRef = `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`;
  const taskPlanRef = `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/taskplan-read-plan.json`;
  const runRef = `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/run-read-plan.json`;
  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "executing",
      requestSummary: "Audit execution read plan.",
      activePhaseId: phaseId,
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now(),
    }],
    effectiveNextAction: { type: "continue_execution", deliveryId, phaseId, reason: "TASKPLAN_READY", targetNode: "task_execution" },
    phase: "building",
    current: { requirementId: null, planId: null, taskId: null, reviewId: null, repairId: null, deploymentId: null },
    lastAction: null,
    nextAction: "next-task",
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "executing",
    requestSummary: "Audit execution read plan.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "ready_for_execution",
      latestRefs: {
        architectureArtifactContract: aacRef,
        taskPlan: taskPlanRef,
        taskPlanRun: runRef,
      },
      nextAction: { type: "continue_execution", deliveryId, phaseId, reason: "TASKPLAN_READY", targetNode: "task_execution" },
    }],
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), technicalBaseline());
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`), planningContract(deliveryId, phaseId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-read-plan",
    contractRef: `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`,
    updatedAt: now(),
  });
  writeJson(projectFile(root, aacRef), architectureContract(deliveryId, phaseId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-read-plan",
    contractRef: aacRef,
    planningContractId: "pgc-read-plan",
    updatedAt: now(),
  });
  writeJson(projectFile(root, taskPlanRef), taskPlan(deliveryId, phaseId));
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/latest.json`), {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-read-plan",
    taskPlanRef,
    updatedAt: now(),
  });
  writeJson(projectFile(root, runRef), taskPlanRun());
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/latest.json`), {
    schemaVersion: "1.0",
    taskPlanRunId: "run-read-plan",
    runRef,
    taskPlanId: "taskplan-read-plan",
    updatedAt: now(),
  });
  return { deliveryId, phaseId };
}

function technicalBaseline() {
  return {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-read-plan",
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
  };
}

function planningContract(deliveryId, phaseId) {
  return {
    schemaVersion: "1.0",
    planningContractId: "pgc-read-plan",
    status: "ready",
    source: {
      brainstormRunId: "brainstorm-read-plan",
      brainstormContractId: "brainstorm-contract-read-plan",
      roadmapId: null,
      phaseId,
      technicalBaselineId: "tb-read-plan",
    },
    phaseScope: {
      phaseName: "Phase 1",
      phaseGoal: "Deliver a small status workflow.",
      included: [{ scopeId: "scope-status", label: "Status workflow", items: ["status workflow"], source: "fixture" }],
      deferred: [],
      excluded: [],
      acceptanceCandidates: [{ id: "AC-status", statement: "The status workflow is implemented and verifiable.", priority: "must" }],
    },
    technicalBaseline: { technicalBaselineId: "tb-read-plan", status: "confirmed", scope: "project", summary: {}, mustFollow: true },
    planningInputs: { businessGoal: "Deliver a small status workflow.", actors: [], capabilityGroups: [], businessFlows: [], sourceRefs: [], contextNotes: [] },
    requirementDetails: {
      schemaVersion: "1.0",
      authority: "brainstorm_contract",
      sourceBrainstormContractRef: `.loom/deliveries/${deliveryId}/brainstorm/contract.json`,
      items: [{
        detailId: "detail-status-workflow",
        kind: "business_flow",
        title: "Status workflow",
        summary: "The current phase status workflow is implemented and verifiable.",
        requiredForCurrentPhase: true,
        priority: "must",
        sourceFieldRefs: ["brainstorm.acceptance.candidates[0]"],
        sourceRefs: [],
        scopeRefs: ["scope-status"],
        acceptanceRefs: ["AC-status"],
        conceptRefs: [],
        frontendRefs: [],
        impactTags: ["business_flow", "acceptance"],
        lifecycleStage: "not_applicable",
        quality: "usable",
        unresolvedNote: null,
      }],
      extractionWarnings: [],
    },
    planningRules: {
      scopeIsolation: { onlyPlanCurrentPhase: true, forbidDeferredScopeImplementation: true, forbidFuturePhaseImplementation: true },
      outputRequirements: { mustCreateArchitectureArtifactContract: true, mustCreateTaskPlan: true, taskPlanMustReferenceAcceptance: true },
      deployment: { defaultEnabled: false, requiresExplicitUserRequest: true },
    },
    qualityGates: { requiresArchitectureBeforeTaskPlan: true, requiresAcceptanceCoverage: true, requiresVerificationEvidence: true },
    handoff: { readyForArchitecture: true, readyForTaskPlan: true, blockingReasons: [], nextNode: "architecture_artifact_contract" },
    createdAt: now(),
    updatedAt: now(),
  };
}

function architectureContract(deliveryId, phaseId) {
  return {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-read-plan",
    status: "ready",
    source: {
      planningGenerationContractId: "pgc-read-plan",
      technicalBaselineId: "tb-read-plan",
      brainstormContractId: "brainstorm-contract-read-plan",
      roadmapId: null,
      phaseId,
    },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-main", type: "library", root: "." }],
      modules: [{ moduleId: "module-status", appId: "app-main", paths: ["src"], responsibility: "Status workflow." }],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [{
      moduleId: "module-status",
      name: "Status Workflow",
      responsibility: "Implement status workflow.",
      dependsOn: [],
      scopeRefs: ["scope-status"],
      acceptanceRefs: ["AC-status"],
    }],
    dataModel: { entities: [], relationships: [], constraints: [] },
    interfaces: [],
    userFlows: [],
    stateMachines: [],
    frontendExperience: {
      required: false,
      kind: "none",
      experienceLevel: "none",
      surfaces: [],
      navigation: { required: false, pattern: "none", items: [] },
      interactionStates: [],
      mustNot: [],
      notes: ["Fixture does not require frontend."],
    },
    runtimeDelivery: {
      status: "not_applicable",
      contractVersion: "phase-1-v1",
      runtimeKind: "code_level_only",
      basis: { technicalBaselineRef: `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`, repositoryContextRef: null, previousRuntimeDeliveryRef: null },
      verificationBoundary: "code_level_only",
      runtimeSurfaces: [],
      taskPlanningGuidance: {
        requireRuntimeDeliveryRequirementWhenTaskTouches: [],
        doNotRequireForTaskKinds: ["implementation", "verification_increment"],
        verificationBoundary: "code_level_only",
        doNotRequireCleanInstallOrContainerBuild: true,
      },
      api: { required: false, probePaths: [] },
      environment: { required: [], optional: [] },
      deployability: { localDocker: "unknown", notes: [] },
    },
    acceptanceMatrix: [{
      acceptanceId: "AC-status",
      priority: "must",
      statement: "The status workflow is implemented and verifiable.",
      coverageStatus: "covered",
      coverage: [{ type: "module", refs: ["module-status"], description: "Status workflow module." }],
      verificationHints: [{ kind: "static", description: "Static verification is sufficient." }],
    }],
    detailCoverage: [{
      detailId: "detail-status-workflow",
      coverageStatus: "covered",
      artifactRefs: {
        modules: ["module-status"],
        entities: [],
        fields: [],
        constraints: [],
        interfaces: [],
        userFlows: [],
        stateMachines: [],
        frontendDataViews: [],
        frontendActions: [],
        frontendOperationPaths: [],
        acceptanceMatrix: ["AC-status"],
      },
    }],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    createdAt: now(),
    updatedAt: now(),
  };
}

function taskPlan(deliveryId, phaseId) {
  return {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-read-plan",
    version: 1,
    status: "ready",
    source: {
      roadmapId: null,
      phaseId,
      planningGenerationContractId: "pgc-read-plan",
      architectureArtifactContractId: "aac-read-plan",
      technicalBaselineId: "tb-read-plan",
    },
    scopeSnapshot: {
      includedScopeRefs: ["scope-status"],
      excludedScopeRefs: [],
      deferredScopeRefs: [],
      acceptanceRefs: ["AC-status"],
    },
    planningPolicy: {
      taskGranularity: "engineering_increment",
      groupGranularity: "engineering_capability",
      allowTaskSplitDuringRepair: true,
      allowTaskMergeDuringRepair: true,
    },
    groups: [{
      groupId: "group-status",
      title: "Status workflow",
      objective: "Implement and verify the status workflow.",
      dependsOn: [],
      scopeRefs: ["scope-status"],
      acceptanceRefs: ["AC-status"],
      taskIds: ["task-status"],
    }],
    tasks: [{
      taskId: "task-status",
      groupId: "group-status",
      title: "Implement status workflow",
      taskKind: "feature_increment",
      implementationActions: ["add_or_update_tests"],
      objective: "Implement the current phase status workflow.",
      dependsOn: [],
      scopeRefs: ["scope-status"],
      acceptanceRefs: ["AC-status"],
      requirementDetailRefs: ["detail-status-workflow"],
      writeBoundary: {
        forbiddenPaths: [".loom"],
        artifactRefs: {
          modules: ["module-status"],
          entities: [],
          interfaces: [],
          userFlows: [],
          stateMachines: [],
          decisions: [],
          risks: [],
        },
      },
      verificationIntents: [{
        verificationId: "VI-status",
        acceptanceRefs: ["AC-status"],
        requirementDetailRefs: ["detail-status-workflow"],
        behavior: "Status workflow can be verified.",
        preferredEvidence: ["static_check"],
        acceptableEvidence: ["static_check", "agent_review_explanation"],
      }],
    }],
    handoff: { readyForExecution: true, nextNode: "task_execution", blockedReasons: [] },
    createdAt: now(),
    updatedAt: now(),
  };
}

function taskPlanRun() {
  return {
    schemaVersion: "1.0",
    runId: "run-read-plan",
    taskPlanId: "taskplan-read-plan",
    status: "not_started",
    scheduler: { mode: "group_dag", startedAt: null, finishedAt: null },
    groupStates: [{ groupId: "group-status", status: "pending", startedAt: null, finishedAt: null, dependsOn: [], taskIds: ["task-status"] }],
    taskStates: [{ taskId: "task-status", groupId: "group-status", status: "pending", resultId: null, startedAt: null, finishedAt: null, dependsOn: [], attempts: [] }],
    summary: { total: 1, completed: 0, completedWithNotes: 0, blocked: 0, failed: 0, pending: 1, running: 0 },
    nextAction: { type: "continue_execution", reason: "TASKPLAN_READY", targetNode: "task_execution" },
    createdAt: now(),
    updatedAt: now(),
  };
}

function sectionCandidate(request, section, content = {}) {
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

function validTaskResult(request) {
  return {
    schemaVersion: "1.0",
    taskResultId: "result-read-plan",
    taskId: request.task.taskId,
    taskPlanId: request.taskPlanId,
    status: "completed",
    changedFiles: ["src/example.ts"],
    noChangeReason: null,
    verificationResults: [{
      verificationId: "VI-status",
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
    requirementDetailEvidence: [{
      detailId: "detail-status-workflow",
      status: "satisfied",
      verificationIds: ["VI-status"],
      evidenceRefs: ["src/example.ts", "VI-status"],
      summary: "Fixture status workflow detail is verified.",
    }],
    blockedReasons: [],
    createdAt: now(),
    updatedAt: now(),
  };
}

function invalidTaskResult(request) {
  return {
    ...validTaskResult(request),
    taskResultId: "result-read-plan-invalid",
    selfRepairSummary: {
      attempted: false,
      attemptCount: 0,
      stopReason: "verification_passed",
      progressObserved: false,
    },
  };
}

function writeDeployFailureFixture(root) {
  writeJson(projectFile(root, ".loom/deployment/specs/local.json"), { schemaVersion: 1, provider: "dockerfile-template" });
  fs.mkdirSync(projectFile(root, ".loom/deployment/logs"), { recursive: true });
  fs.writeFileSync(projectFile(root, ".loom/deployment/logs/local.log"), "npm run build failed\n", "utf8");
  writeJson(projectFile(root, ".loom/deployment/state/latest-failure.json"), {
    schemaVersion: "1.0",
    failureId: "failure-read-plan",
    source: "deploy",
    createdAt: now(),
    deploymentAttemptId: "attempt-read-plan",
    failureKind: "build_command_failed",
    failureOwner: "application_code",
    repairRoute: "execution_repair",
    runtimeDeliveryRef: null,
    sourceRefs: {
      runtimeDeliveryRef: null,
      taskPlanRef: null,
      taskPlanRunRef: null,
      reviewResultRef: null,
      deploymentSpecRef: ".loom/deployment/specs/local.json",
    },
    failedContract: {
      field: "build.command",
      command: "npm run build",
      workingDirectory: ".",
    },
    evidence: {
      failedAt: "build",
      deployCommand: ["docker", "compose", "up", "--build"],
      exitCode: 1,
      fullLogRef: ".loom/deployment/logs/local.log",
      errorWindow: {
        lines: ["npm run build failed"],
        truncated: false,
        totalLineCount: 1,
        matchedPatterns: ["npm run build"],
      },
      stdoutTail: ["npm run build failed"],
      stderrTail: [],
      logMarkers: ["build"],
      diagnostics: [{
        code: "BUILD_FAILED",
        severity: "error",
        message: "Build command failed.",
        evidence: ["npm run build failed"],
        suggestedAction: "Repair the application build command.",
      }],
    },
    routing: {
      editableBoundary: "application_code_only",
      mustNotEdit: ["Dockerfile", "compose.yaml"],
      nextCommand: {
        name: "repair request",
        argv: ["repair", "request", "--type", "execution", "--source", "deploy", "--failure-ref", ".loom/deployment/state/latest-failure.json"],
      },
    },
    loopGuard: {
      signature: "build.command:npm run build",
      attempt: 1,
      maxAttempts: 3,
    },
  });
}

async function auditBrainstorm() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-read-plan-brainstorm-"));
  try {
    run(["init"], root);
    const started = run(["brainstorm", "start", "--request", "Build a compact status workflow."], root);
    const request = await requestFromCommand(started, root);
    auditReadPlan("BrainstormSessionRequest", request, {
      requiredFields: ["originalRequest", "outputContract.candidateFile", "outputContract.schemaShape.scope", "enumRefs"],
      forbiddenFields: ["agentAction", "clarificationConversationProtocol", "requestManifest", "rules", "outputContract", "outputContract.schemaShape", "outputContract.schemaShape.candidateRules"],
    });
    const scopeCore = readGroup(request, "brainstorm_session_phase_scope_core");
    const scopeAuthority = readGroup(request, "brainstorm_session_phase_scope_authority");
    const conceptContext = readGroup(request, "brainstorm_session_concept_grounding_context");
    const candidateWriteControls = readGroup(request, "brainstorm_session_candidate_write_controls");
    const candidateSchemaIdentity = readGroup(request, "brainstorm_session_candidate_schema_identity");
    const candidateSchemaScope = readGroup(request, "brainstorm_session_candidate_schema_scope");
    const candidateSchemaConcepts = readGroup(request, "brainstorm_session_candidate_schema_concepts");
    const candidateSchemaFrontend = readGroup(request, "brainstorm_session_candidate_schema_frontend");
    const candidateSchemaHandoff = readGroup(request, "brainstorm_session_candidate_schema_handoff");
    const candidateFinalSummaryRules = readGroup(request, "brainstorm_session_candidate_final_summary_rules");
    const candidateRequirementSemanticRules = readGroup(request, "brainstorm_session_candidate_requirement_semantic_rules");
    const candidateSelfReviewRules = readGroup(request, "brainstorm_session_candidate_self_review_rules");
    assert.ok(scopeCore, "BrainstormSessionRequest must expose phase_scope_core group");
    assert.ok(scopeAuthority, "BrainstormSessionRequest must expose phase_scope_authority group");
    assert.ok(conceptContext, "BrainstormSessionRequest must expose concept_grounding_context group");
    assert.ok(candidateWriteControls, "BrainstormSessionRequest must expose candidate_write_controls group");
    assert.ok(candidateSchemaIdentity, "BrainstormSessionRequest must expose candidate_schema_identity group");
    assert.ok(candidateSchemaScope, "BrainstormSessionRequest must expose candidate_schema_scope group");
    assert.ok(candidateSchemaConcepts, "BrainstormSessionRequest must expose candidate_schema_concepts group");
    assert.ok(candidateSchemaFrontend, "BrainstormSessionRequest must expose candidate_schema_frontend group");
    assert.ok(candidateSchemaHandoff, "BrainstormSessionRequest must expose candidate_schema_handoff group");
    assert.ok(candidateFinalSummaryRules, "BrainstormSessionRequest must expose candidate_final_summary_rules group");
    assert.ok(candidateRequirementSemanticRules, "BrainstormSessionRequest must expose candidate_requirement_semantic_rules group");
    assert.ok(candidateSelfReviewRules, "BrainstormSessionRequest must expose candidate_self_review_rules group");
    for (const field of ["agentAction", "clarificationConversationProtocol", "requestManifest", "outputContract", "rules", "generationProtocol", "enumRefs", "conceptGroundingRequest"]) {
      assert.equal(scopeCore.fields.includes(field), false, `phase_scope_core must not read ${field}`);
    }
    assert.deepEqual(candidateWriteControls.fields, ["agentAction.write", "agentAction.submit", "agentAction.schema", "outputContract.candidateFile", "outputContract.schemaRef", "generationProtocol", "enumRefs"], "candidate_write_controls must hold delayed write control fields");
    assert.deepEqual(candidateSchemaIdentity.fields, [
      "outputContract.schemaShape.schemaVersion",
      "outputContract.schemaShape.candidateId",
      "outputContract.schemaShape.brainstormRunId",
      "outputContract.schemaShape.deliveryId",
      "outputContract.schemaShape.phaseId",
      "outputContract.schemaShape.status",
      "outputContract.schemaShape.requestSummary",
      "outputContract.schemaShape.sources",
    ], "candidate_schema_identity must hold identity schema fields");
    assert.deepEqual(candidateSchemaScope.fields, [
      "outputContract.schemaShape.scope",
      "outputContract.schemaShape.roadmap",
      "outputContract.schemaShape.phasePlan",
      "outputContract.schemaShape.acceptance",
    ], "candidate_schema_scope must hold scope schema fields");
    assert.deepEqual(candidateSchemaConcepts.fields, [
      "outputContract.schemaShape.domainModel",
      "outputContract.schemaShape.conceptGrounding",
      "outputContract.schemaShape.conceptConfirmation",
      "outputContract.schemaShape.clarificationProgress",
    ], "candidate_schema_concepts must hold concept schema fields");
    assert.deepEqual(candidateSchemaFrontend.fields, ["outputContract.schemaShape.frontendExperience"], "candidate_schema_frontend must hold frontend schema fields");
    assert.deepEqual(candidateSchemaHandoff.fields, ["outputContract.schemaShape.handoff"], "candidate_schema_handoff must hold delayed handoff fields");
    assert.deepEqual(candidateFinalSummaryRules.fields, [
      "clarificationConversationProtocol.blockConfirmationRules.final_summary",
      "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.appliesWhenAgentFinds",
      "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.requiredUserVisibleTopicsWhenApplicable",
      "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.notApplicableRule",
      "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.candidateFieldMapping",
      "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.finalSummaryReviewContract",
    ], "candidate_final_summary_rules must hold narrow final summary rule fields");
    assert.ok(
      candidateFinalSummaryRules.whenToRead.includes("before presenting the final_summary block"),
      "candidate_final_summary_rules must be readable before final_summary is presented",
    );
    assert.deepEqual(candidateRequirementSemanticRules.fields, ["rules.requirementSemanticGrounding.compactRules"], "candidate_requirement_semantic_rules must hold compact requirement semantic rule list");
    assert.deepEqual(candidateSelfReviewRules.fields, ["rules.candidateSelfReview", "rules.nextPhasePreviewGeneration"], "candidate_self_review_rules must hold narrow self-review rule fields");
    const brainstormFields = request.agentAction.read.fieldGroups.flatMap((group) => group.fields);
    assert.equal(brainstormFields.includes("outputContract"), false, "BrainstormSessionRequest must not read full outputContract in fieldGroups");
    assert.equal(brainstormFields.includes("outputContract.schemaShape"), false, "BrainstormSessionRequest must not read full outputContract.schemaShape in fieldGroups");
    assert.equal(brainstormFields.includes("outputContract.schemaShape.candidateRules"), false, "BrainstormSessionRequest must not read full candidateRules in fieldGroups");
    assert.equal(brainstormFields.includes("agentAction"), false, "BrainstormSessionRequest must not read full agentAction in fieldGroups");
    assert.equal(brainstormFields.includes("clarificationConversationProtocol"), false, "BrainstormSessionRequest must not read full clarificationConversationProtocol in fieldGroups");
    assert.equal(brainstormFields.includes("requestManifest"), false, "BrainstormSessionRequest must not read full requestManifest in fieldGroups");
    assert.equal(brainstormFields.includes("rules"), false, "BrainstormSessionRequest must not read full rules in fieldGroups");
    assert.deepEqual(conceptContext.fields, [
      "conceptGroundingRequest",
      "clarificationConversationProtocol.blockExecutionRules",
      "clarificationConversationProtocol.blockConfirmationRules.concept_grounding",
      "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.scopeItemCoverageContract",
      "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.objectOperationContract",
    ], "concept_grounding_context must hold concept-only fields and narrow concept rules");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function auditArchitecture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-read-plan-architecture-"));
  try {
    run(["init"], root);
    const { deliveryId, phaseId } = writePlanningFixture(root);
    const arch = run(["architecture", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const request = await requestFromCommand(arch, root);
    auditReadPlan("ArchitectureSectionsGenerationRequest", request, {
      requiredFields: [
        "agentAction.write.currentTarget",
        "contextProjection.requirementDetailTransfer",
        "allowedRefs",
        "outputContract.allowedRefsUsage",
      ],
      forbiddenFields: ["contextProjection", "outputContract.sectionOutputs", "enumRefs"],
    });
    assert.ok(readGroup(request, "generate_sections_current_target"), "ArchitectureSectionsGenerationRequest: current target group must exist");
    assert.ok(readGroup(request, "generate_sections_section_authority"), "ArchitectureSectionsGenerationRequest: section authority group must exist");
    assert.ok(readGroup(request, "generate_sections_section_contract"), "ArchitectureSectionsGenerationRequest: section contract group must exist");
    assert.equal(
      request.contextProjection.requirementDetailTransfer.requirementDetails.fullDetailSource,
      "sourceRefs.planningContractRef#/requirementDetails",
      "ArchitectureSectionsGenerationRequest: requirementDetails projection must point to full detail source instead of duplicating full PGC internals",
    );
    assert.equal(
      request.contextProjection.requirementDetailTransfer.requirementDetails.extractionWarnings,
      undefined,
      "ArchitectureSectionsGenerationRequest: compact requirementDetails projection must not inline extractionWarnings",
    );

    const firstTarget = request.outputContract.sectionOutputs[0];
    writeJson(projectFile(root, firstTarget.candidateFile), sectionCandidate(request, firstTarget.section));
    run(["continue"], root);
    const refreshed = await loadRequest(root, arch.requestPath);
    auditReadPlan("ArchitectureSectionsGenerationRequest after continue target refresh", refreshed, {
      requiredFields: [
        "agentAction.write.currentTarget",
        "contextProjection.requirementDetailTransfer",
        "allowedRefs",
        "outputContract.allowedRefsUsage",
      ],
      forbiddenFields: ["contextProjection", "outputContract.sectionOutputs", "enumRefs"],
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function auditTaskPlan() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-read-plan-taskplan-"));
  try {
    run(["init"], root);
    const { deliveryId, phaseId } = writePlanningFixture(root);
    const taskPlanRequest = run(["task-plan", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const request = await requestFromCommand(taskPlanRequest, root);
    auditReadPlan("TaskPlanGenerationRequest", request, {
      requiredFields: [
        "sourceRefs",
        "contextProjection.requirementDetailTransfer",
        "generationRules.requirementDetailTransferRules",
        "generationRules.workflowClosureRules.requirementSource",
        "generationRules.workflowClosureRules.taskAssignmentRule",
        "generationRules.workflowClosureRules.taskCoverageShape",
        "generationRules.runtimeDeliveryRules.status",
        "outputContract.outlineSchemaShape",
        "outputContract.groupSchemaShape",
      ],
      forbiddenFields: [
        "generationRules",
        "generationRules.workflowClosureRules",
        "generationRules.workflowClosureRules.requirements",
        "outputContract",
        "outputContract.workflowClosureRequirements",
      ],
    });
    const coreGroup = readGroup(request, "generate_taskplan_grouped_core");
    assert.ok(coreGroup, "TaskPlanGenerationRequest: core group must exist");
    assert.equal(coreGroup.required, true, "TaskPlanGenerationRequest: core group must be required");
    const rulesGroup = readGroup(request, "generate_taskplan_grouped_rules");
    assert.ok(rulesGroup, "TaskPlanGenerationRequest: rules group must exist");
    assert.equal(rulesGroup.required, true, "TaskPlanGenerationRequest: rules group must be required");
    assert.equal(
      rulesGroup.fields.includes("generationRules.workflowClosureRules.requirements"),
      false,
      "TaskPlanGenerationRequest: rules group must not duplicate workflow closure requirements",
    );
    assert.equal(
      request.generationRules.workflowClosureRules.requirements,
      undefined,
      "TaskPlanGenerationRequest: generationRules must not duplicate workflow closure requirements",
    );
    assert.equal(
      request.outputContract.workflowClosureRequirements,
      undefined,
      "TaskPlanGenerationRequest: outputContract must not duplicate workflow closure requirements",
    );
    assert.equal(
      request.contextProjection.requirementDetailTransfer.architectureDetails.compaction.mode,
      "artifact_summary",
      "TaskPlanGenerationRequest: architectureDetails must use compact artifact summaries",
    );
    assert.equal(
      request.contextProjection.requirementDetailTransfer.architectureDetails.engineeringBoundary,
      undefined,
      "TaskPlanGenerationRequest: architectureDetails must not inline full AAC engineeringBoundary",
    );
    assert.ok(
      request.contextProjection.requirementDetailTransfer.requirementDetailAssignment.items.some((item) =>
        item.detailId === "detail-status-workflow" &&
        item.coverageStatus === "covered" &&
        item.artifactRefs.modules.includes("module-status")
      ),
      "TaskPlanGenerationRequest: detail assignment must retain detailId-to-artifact refs after compaction",
    );
    const optionalGroup = readGroup(request, "generate_taskplan_grouped_optional_context");
    assert.ok(optionalGroup, "TaskPlanGenerationRequest: optional context group must exist");
    assert.equal(optionalGroup.required, false, "TaskPlanGenerationRequest: optional context group must be optional");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function auditTaskExecutionAndReview() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-read-plan-execution-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2), "utf8");
    fs.mkdirSync(projectFile(root, "src"), { recursive: true });
    fs.writeFileSync(projectFile(root, "src/example.ts"), "export const ok = true;\n", "utf8");
    run(["init"], root);
    const { deliveryId, phaseId } = writeExecutionFixture(root);
    const nextTask = run(["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const executionRequest = await requestFromCommand(nextTask, root);
    auditReadPlan("TaskExecutionRequest", executionRequest, {
      requiredFields: [
        "task.taskId",
        "task.objective",
        "task.writeBoundary",
        "sourceContext.acceptanceSnapshot",
        "sourceContext.architectureArtifactProjection.projectionCompleteness",
        "executionRules.sourceEditPreparationContract",
        "executionRules.interactiveVerificationProbePolicy",
        "outputContract.schemaShape.verificationResults",
        "outputContract.schemaShape.allowedVerificationResults",
      ],
      forbiddenFields: [
        "task",
        "task.frontendExperienceRequirement",
        "sourceContext",
        "sourceContext.architectureArtifactProjection",
        "executionRules",
        "outputContract.schemaShape",
      ],
    });
    assert.ok(readGroup(executionRequest, "execute_task_task_core"), "TaskExecutionRequest: task_core group must exist");
    assert.ok(readGroup(executionRequest, "execute_task_architecture_context"), "TaskExecutionRequest: architecture_context group must exist");
    assert.ok(readGroup(executionRequest, "execute_task_execution_rules"), "TaskExecutionRequest: execution_rules group must exist");
    assert.ok(readGroup(executionRequest, "execute_task_result_contract"), "TaskExecutionRequest: result_contract group must exist");

    writeJson(projectFile(root, executionRequest.outputContract.resultFile), validTaskResult(executionRequest));
    run(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", executionRequest.outputContract.resultFile], root);
    const review = run(["review", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const reviewRequest = await requestFromCommand(review, root);
    auditReadPlan("ReviewRequest", reviewRequest, {
      requiredFields: [
        "reviewPacketRef",
        "changeContextRef",
        "reviewRules.commonRules",
        "reviewRules.changeSetRules",
        "enumRefs",
        "outputContract.schemaShape",
        "outputContract.reviewSignals",
      ],
      forbiddenFields: ["outputContract", "reviewRules", "per-file diffRefs from changeContextRef", "task results referenced by reviewPacketRef"],
    });
    assert.ok(readGroup(reviewRequest, "review_gate_evidence_core"), "ReviewRequest: evidence core group must exist");
    assert.ok(readGroup(reviewRequest, "review_gate_policy"), "ReviewRequest: policy group must exist");
    assert.ok(readGroup(reviewRequest, "review_gate_result_contract"), "ReviewRequest: result contract group must exist");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function auditRepair() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-read-plan-repair-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2), "utf8");
    fs.mkdirSync(projectFile(root, "src"), { recursive: true });
    fs.writeFileSync(projectFile(root, "src/example.ts"), "export const ok = true;\n", "utf8");
    run(["init"], root);
    const { deliveryId, phaseId } = writeExecutionFixture(root);
    const nextTask = run(["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const executionRequest = await requestFromCommand(nextTask, root);
    writeJson(projectFile(root, executionRequest.outputContract.resultFile), invalidTaskResult(executionRequest));
    run(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", executionRequest.outputContract.resultFile], root);
    const repair = run(["repair", "request", "--type", "task-result", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const repairRequest = await requestFromCommand(repair, root);
    auditReadPlan("TaskResultRepairRequest", repairRequest, {
      requiredFields: ["inputs", "repairRules", "outputContract"],
      forbiddenFields: ["this RepairRequest"],
    });
    assert.ok(readGroup(repairRequest, "generate_candidate_repair_scope"), "TaskResultRepairRequest: repair scope group must exist");
    assert.ok(readGroup(repairRequest, "generate_candidate_candidate_contract"), "TaskResultRepairRequest: candidate contract group must exist");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function auditDeployRepair() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-read-plan-deploy-repair-"));
  try {
    run(["init"], root);
    writeDeployFailureFixture(root);
    const repair = run([
      "repair",
      "request",
      "--type",
      "execution",
      "--source",
      "deploy",
      "--failure-ref",
      ".loom/deployment/state/latest-failure.json",
    ], root);
    const deployRepairRequest = await loadRequest(root, repair.requestRef);
    auditReadPlan("DeployExecutionRepairRequest", deployRepairRequest, {
      requiredFields: [
        "syntheticTask",
        "deploymentFailureRef",
        "sourceRefs",
        "executionRules.sourceEditPreparationContract",
        "outputContract.resultFile",
        "outputContract.submitCommand",
        "outputContract.schemaShape.status",
      ],
      forbiddenFields: [
        "executionRules",
        "outputContract.schemaShape",
        "sourceRefs.runtimeDeliveryRef",
      ],
    });
    const syntheticExecutionRequest = await loadRequest(root, repair.executionRequestRef);
    auditReadPlan("DeploySourcedTaskExecutionRequest", syntheticExecutionRequest, {
      requiredFields: [
        "task.taskId",
        "task.objective",
        "source",
        "executionRules.sourceEditPreparationContract",
        "outputContract.schemaShape.status",
        "outputContract.schemaShape.runtimeDeliveryEvidence",
        "postSubmitRouting",
      ],
      forbiddenFields: [
        "task",
        "executionRules",
        "outputContract.schemaShape",
        "sourceRefs.runtimeDeliveryRef",
      ],
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
  await auditBrainstorm();
  await auditArchitecture();
  await auditTaskPlan();
  await auditTaskExecutionAndReview();
  await auditRepair();
  await auditDeployRepair();
  console.log("read plan size audit passed");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
