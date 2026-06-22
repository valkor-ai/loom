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
    stdio: ["ignore", "pipe", "pipe"],
  });
  const envelope = JSON.parse(output);
  assert.equal(envelope.ok, true, `${args.join(" ")} failed: ${output}`);
  return envelope.data;
}

function git(args, cwd) {
  return execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
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

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function writeReviewReadyState(root) {
  const deliveryId = "delivery-review-scope";
  const phaseId = "phase-1";
  const taskPlanId = "taskplan-review-scope";
  const runId = "run-review-scope";
  const taskId = "task-review-scope";
  const resultId = "result-review-scope";

  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "reviewing",
      requestSummary: "Verify review scope ignores unrelated worktree files.",
      activePhaseId: phaseId,
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now(),
    }],
    effectiveNextAction: {
      type: "review",
      source: "task_plan_run",
      deliveryId,
      phaseId,
      reason: "TASKPLAN_RUN_COMPLETED",
      targetNode: "review",
    },
    phase: "reviewing",
    current: {
      requirementId: null,
      planId: null,
      taskId: null,
      reviewId: null,
      repairId: null,
      deploymentId: null,
    },
    lastAction: null,
    nextAction: "review",
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "reviewing",
    requestSummary: "Verify review scope ignores unrelated worktree files.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "reviewing",
      latestRefs: {
        taskPlan: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/${taskPlanId}.json`,
        taskPlanRun: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/${runId}.json`,
      },
      nextAction: {
        type: "review",
        source: "task_plan_run",
        deliveryId,
        phaseId,
        reason: "TASKPLAN_RUN_COMPLETED",
        targetNode: "review",
      },
    }],
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-review-scope",
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
    planningContractId: "pgc-review-scope",
    status: "ready",
    source: {
      brainstormRunId: "brainstorm-review-scope",
      brainstormContractId: "brainstorm-contract-review-scope",
      roadmapId: null,
      phaseId,
      technicalBaselineId: "tb-review-scope",
    },
    phaseScope: {
      phaseName: "Phase 1",
      phaseGoal: "Verify review scope.",
      included: [{ scopeId: "scope-review", label: "Review declared files", items: ["declared changed file"], source: "test" }],
      deferred: [],
      excluded: [],
      acceptanceCandidates: [{ id: "AC-review", statement: "Review only declared changed files.", priority: "must" }],
    },
    technicalBaseline: {
      technicalBaselineId: "tb-review-scope",
      status: "confirmed",
      scope: "project",
      summary: { languages: ["TypeScript"] },
      mustFollow: true,
    },
    planningInputs: {
      businessGoal: "Verify review scope.",
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
    planningContractId: "pgc-review-scope",
    contractRef: `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`,
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-review-scope",
    status: "ready",
    source: {
      planningGenerationContractId: "pgc-review-scope",
      technicalBaselineId: "tb-review-scope",
      brainstormContractId: "brainstorm-contract-review-scope",
      roadmapId: null,
      phaseId,
    },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-main", type: "library", root: "." }],
      modules: [{ moduleId: "module-review", appId: "app-main", paths: ["src"], responsibility: "Review scope fixture." }],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [{
      moduleId: "module-review",
      name: "Review Scope",
      responsibility: "Review declared changed files.",
      dependsOn: [],
      scopeRefs: ["scope-review"],
      acceptanceRefs: ["AC-review"],
    }],
    dataModel: { entities: [], relationships: [], constraints: [] },
    interfaces: [],
    userFlows: [],
    stateMachines: [],
    acceptanceMatrix: [{
      acceptanceId: "AC-review",
      priority: "must",
      statement: "Review only declared changed files.",
      coverageStatus: "covered",
      coverage: [{ type: "module", refs: ["module-review"], description: "Fixture module." }],
      verificationHints: [{ kind: "static", description: "Review request scopes changed files." }],
    }],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/latest.json`), {
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-review-scope",
    contractRef: `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`,
    planningContractId: "pgc-review-scope",
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/${taskPlanId}.json`), {
    schemaVersion: "1.0",
    taskPlanId,
    version: 1,
    status: "ready",
    source: {
      roadmapId: null,
      phaseId,
      planningGenerationContractId: "pgc-review-scope",
      architectureArtifactContractId: "aac-review-scope",
      technicalBaselineId: "tb-review-scope",
    },
    scopeSnapshot: {
      includedScopeRefs: ["scope-review"],
      excludedScopeRefs: [],
      deferredScopeRefs: [],
      acceptanceRefs: ["AC-review"],
    },
    planningPolicy: {
      taskGranularity: "engineering_increment",
      groupGranularity: "engineering_capability",
      allowTaskSplitDuringRepair: true,
      allowTaskMergeDuringRepair: true,
    },
    groups: [{
      groupId: "group-review",
      title: "Review declared files",
      objective: "Produce one declared changed file.",
      dependsOn: [],
      scopeRefs: ["scope-review"],
      acceptanceRefs: ["AC-review"],
      taskIds: [taskId],
    }],
    tasks: [{
      taskId,
      groupId: "group-review",
      title: "Modify declared file",
      taskKind: "feature_increment",
      implementationActions: ["create_or_update_interface"],
      objective: "Modify only the declared file for review scope.",
      dependsOn: [],
      scopeRefs: ["scope-review"],
      acceptanceRefs: ["AC-review"],
      writeBoundary: {
        forbiddenPaths: [".loom"],
        artifactRefs: {
          modules: ["module-review"],
          entities: [],
          interfaces: [],
          userFlows: [],
          stateMachines: [],
          decisions: [],
          risks: [],
        },
      },
      verificationIntents: [{
        verificationId: "VI-review",
        acceptanceRefs: ["AC-review"],
        behavior: "Declared file is reviewed.",
        preferredEvidence: ["static_check"],
        acceptableEvidence: ["static_check", "agent_review_explanation"],
      }],
    }],
    handoff: { readyForExecution: true, nextNode: "task_execution", blockedReasons: [] },
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/latest.json`), {
    schemaVersion: "1.0",
    taskPlanId,
    taskPlanRef: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/${taskPlanId}.json`,
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/${runId}.json`), {
    schemaVersion: "1.0",
    runId,
    taskPlanId,
    status: "completed",
    scheduler: { mode: "group_dag", startedAt: now(), finishedAt: now() },
    groupStates: [{
      groupId: "group-review",
      status: "completed",
      startedAt: now(),
      finishedAt: now(),
      dependsOn: [],
      taskIds: [taskId],
    }],
    taskStates: [{
      taskId,
      groupId: "group-review",
      status: "completed",
      resultId,
      startedAt: now(),
      finishedAt: now(),
      dependsOn: [],
      attempts: [{ attempt: 1, resultId, status: "completed" }],
    }],
    summary: { total: 1, completed: 1, completedWithNotes: 0, blocked: 0, failed: 0, pending: 0, running: 0 },
    nextAction: { type: "review", reason: "TASKPLAN_RUN_COMPLETED", targetNode: "review" },
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/latest.json`), {
    schemaVersion: "1.0",
    taskPlanRunId: runId,
    runRef: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/${runId}.json`,
    taskPlanId,
    updatedAt: now(),
  });
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/results/${runId}/${taskId}/${resultId}.json`), {
    schemaVersion: "1.0",
    taskResultId: resultId,
    taskId,
    taskPlanId,
    status: "completed",
    changedFiles: ["src/declared.ts"],
    noChangeReason: null,
    verificationResults: [{
      verificationId: "VI-review",
      status: "passed",
      evidenceType: "static_check",
      summary: "Declared file changed.",
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
  });
  return { deliveryId, phaseId };
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-review-scope-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2));
    fs.mkdirSync(projectFile(root, "src"), { recursive: true });
    fs.writeFileSync(projectFile(root, "src/declared.ts"), "export const declared = 'before';\n");
    git(["init"], root);
    git(["config", "user.name", "Loom Test"], root);
    git(["config", "user.email", "loom-test@example.com"], root);
    git(["add", "package.json", "src/declared.ts"], root);
    git(["commit", "-m", "initial"], root);
    fs.writeFileSync(projectFile(root, "src/declared.ts"), "export const declared = 'after';\n");
    fs.writeFileSync(projectFile(root, "src/not-declared.ts"), "export const extra = true;\n");

    run(["init"], root);
    const { deliveryId, phaseId } = writeReviewReadyState(root);
    const review = run(["review", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const reviewRequest = hydrateRequest(root, readJson(projectFile(root, review.requestPath ?? review.requestRef)));
    const changeContext = readJson(projectFile(root, reviewRequest.changeContextRef));
    assert.equal(changeContext.mode, "git_diff_ref");
    assert.deepEqual(changeContext.changedFiles.map((file) => file.path), ["src/declared.ts"]);
    assert.equal(JSON.stringify(changeContext).includes("not-declared"), false);

    const diffRef = changeContext.changedFiles[0].diffRef;
    assert.equal(typeof diffRef, "string");
    const fileDiff = fs.readFileSync(projectFile(root, diffRef), "utf8");
    assert.match(fileDiff, /declared = 'after'/);
    assert.equal(fileDiff.includes("not-declared"), false);

    const fullDiff = fs.readFileSync(path.join(path.dirname(projectFile(root, diffRef)), "full.diff"), "utf8");
    assert.match(fullDiff, /src\/declared\.ts/);
    assert.equal(fullDiff.includes("not-declared"), false);

    const resultFile = reviewRequest.outputContract.resultFile;
    writeJson(projectFile(root, resultFile), {
      schemaVersion: "1.0",
      reviewId: reviewRequest.requestId,
      source: {
        requestId: reviewRequest.requestId,
        phaseId,
        taskPlanId: reviewRequest.source.taskPlanId,
        taskPlanRunId: reviewRequest.source.taskPlanRunId,
      },
      decision: "approved_with_notes",
      findings: [{
        findingId: "finding-review-ref-normalization",
        severity: "minor",
        severityClass: "warning",
        evidenceKind: "static",
        failureClass: "subjective_quality",
        category: "functional_correctness",
        summary: "Review ref normalization fixture.",
        evidence: "Uses equivalent current changed-file and verification refs.",
        readRefs: [
          {
            type: "changed_file",
            ref: "changed_file:./src/declared.ts",
            reason: "Equivalent form of the current changed file path.",
          },
          {
            type: "verification_evidence",
            ref: "VI-review",
            reason: "Verification id from the current TaskResult.",
          },
        ],
        evidenceRefs: [
          {
            type: "changed_file",
            ref: "./src/declared.ts",
            reason: "Equivalent changed-file evidence path.",
          },
          {
            type: "verification_result",
            ref: "result-review-scope:VI-review",
            reason: "TaskResult verification evidence.",
          },
        ],
        groupRefs: ["group-review"],
        taskRefs: ["task-review-scope"],
        acceptanceRefs: ["AC-review"],
        artifactRefs: {
          modules: [],
          entities: [],
          interfaces: [],
          userFlows: [],
          stateMachines: [],
          decisions: [],
          risks: [],
        },
        location: { file: "src/declared.ts", line: null, diffRef },
        taskRelevance: "direct",
        scopeRelation: "within_task_changed_files",
        introducedByCurrentTask: "unknown",
        recommendedNextAction: "continue_to_next_phase",
      }],
      coverageAssessment: {
        mustAcceptance: [{
          acceptanceRef: "AC-review",
          status: "satisfied",
          supportingTaskResults: ["result-review-scope"],
          evidenceStatus: "sufficient",
          notes: ["Review refs normalized."],
        }],
        summary: {
          totalMust: 1,
          satisfied: 1,
          insufficientEvidence: 0,
          notSatisfied: 0,
          notReviewed: 0,
        },
      },
      limitations: [],
      pendingActions: [],
      nextAction: {
        type: "done",
        reason: "No further phases in this fixture.",
        targetNode: "done",
        targetPhaseId: null,
        targetTaskIds: [],
        findingRefs: [],
        userVisibleState: null,
      },
      notes: ["Review ref normalization accepted."],
      createdAt: now(),
      updatedAt: now(),
    });
    const acceptedReview = run(["review", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--result-file", resultFile], root);
    assert.equal(acceptedReview.accepted, true, JSON.stringify(acceptedReview.issues, null, 2));

    console.log("review scope declared changed-files verification passed");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

main();
