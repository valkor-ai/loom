#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const {
  validateReviewResult,
  validateTaskPlanCandidate,
  validateTaskResult,
} = require("../../dist/core/validators");
const { createTaskPlanRequest } = require("../../dist/core/operations/tasks");
const { buildWorkflowClosureRequirements } = require("../../dist/core/workflow-closure");

function now() {
  return new Date().toISOString();
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

const baseline = {
  schemaVersion: "1.0",
  technicalBaselineId: "tb-workflow-closure",
  status: "confirmed",
  source: "detected_from_repo",
  projectKind: "existing_project",
  scope: "project",
  stack: { runtime: "node", language: "typescript", packageManager: "npm" },
  constraints: [],
  evidence: [{ path: "package.json", reason: "fixture" }],
  approval: { type: "policy_auto_accept", reason: "fixture" },
  confidence: "high",
  createdAt: now(),
  updatedAt: now(),
};

const pgc = {
  schemaVersion: "1.0",
  planningContractId: "pgc-phase-1",
  status: "ready",
  source: {
    brainstormRunId: "brainstorm-closure",
    brainstormContractId: "brainstorm-contract-closure",
    roadmapId: null,
    phaseId: "phase-1",
    technicalBaselineId: baseline.technicalBaselineId,
  },
  phaseScope: {
    phaseName: "Phase 1",
    phaseGoal: "Deliver the first usable workflow.",
    included: [{ scopeId: "scope-neutral", label: "Neutral workflow", items: ["field A", "field B", "result state"], source: "fixture" }],
    deferred: [],
    excluded: [],
    acceptanceCandidates: [{
      id: "AC-neutral",
      statement: "The screen reflects the declared result after the current interaction.",
      priority: "must",
      capabilityRefs: [],
      sourceRefs: ["fixture#neutral"],
    }],
  },
  technicalBaseline: {
    technicalBaselineId: baseline.technicalBaselineId,
    status: "confirmed",
    scope: "project",
    summary: { runtime: "node" },
    mustFollow: true,
  },
  planningInputs: {
    businessGoal: "Deliver the first usable workflow.",
    actors: [],
    capabilityGroups: [],
    businessFlows: [{
      flowId: "business-flow-neutral",
      name: "Neutral flow",
      steps: ["enter fields", "invoke boundary", "show result"],
    }],
    sourceRefs: ["fixture#neutral"],
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
    deployment: { defaultEnabled: false, requiresExplicitUserRequest: true },
  },
  qualityGates: {
    requiresArchitectureBeforeTaskPlan: true,
    requiresAcceptanceCoverage: true,
    requiresVerificationEvidence: true,
  },
  handoff: { readyForArchitecture: true, readyForTaskPlan: true, blockingReasons: [], nextNode: "architecture_artifact_contract" },
  createdAt: now(),
  updatedAt: now(),
};

const aac = {
  schemaVersion: "1.0",
  architectureArtifactContractId: "aac-workflow-closure",
  status: "ready",
  source: {
    planningGenerationContractId: pgc.planningContractId,
    technicalBaselineId: baseline.technicalBaselineId,
    brainstormContractId: "brainstorm-contract-closure",
    roadmapId: null,
    phaseId: "phase-1",
  },
  engineeringBoundary: {
    projectKind: "existing_project",
    strategy: "extend_existing_modules",
    applications: [{ appId: "app-web", type: "web", root: "." }],
    modules: [{ moduleId: "module-neutral", appId: "app-web", paths: ["src"], responsibility: "Neutral workflow." }],
    creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
  },
  modules: [{
    moduleId: "module-neutral",
    name: "Neutral module",
    responsibility: "Own the neutral workflow.",
    dependsOn: [],
    scopeRefs: ["scope-neutral"],
    acceptanceRefs: ["AC-neutral"],
  }],
  dataModel: {
    entities: [{
      entityId: "entity-neutral",
      name: "NeutralEntity",
      type: "internal",
      implementationIntent: "full",
      moduleRefs: ["module-neutral"],
      scopeRefs: ["scope-neutral"],
      acceptanceRefs: ["AC-neutral"],
      fields: [{ fieldId: "field-a", name: "fieldA", type: "string", required: true }],
      constraints: [{ constraintId: "constraint-a-required", type: "validation", description: "fieldA is required." }],
    }],
    relationships: [],
    constraints: [],
  },
  interfaces: [{
    interfaceId: "if-neutral-view",
    name: "Neutral view",
    type: "component",
    moduleRefs: ["module-neutral"],
    entityRefs: ["entity-neutral"],
    scopeRefs: ["scope-neutral"],
    acceptanceRefs: ["AC-neutral"],
  }, {
    interfaceId: "if-neutral-boundary",
    name: "Neutral boundary",
    type: "service_method",
    moduleRefs: ["module-neutral"],
    entityRefs: ["entity-neutral"],
    scopeRefs: ["scope-neutral"],
    acceptanceRefs: ["AC-neutral"],
    requestSchema: [{ fieldId: "field-a", name: "fieldA", type: "string", required: true }],
    responseSchema: [{ fieldId: "field-result", name: "result", type: "string", required: true }],
    errorSchema: [{ fieldId: "field-error", name: "error", type: "string", required: true }],
  }],
  userFlows: [{
    flowId: "flow-neutral",
    name: "Neutral interaction",
    kind: "user_interaction",
    moduleRefs: ["module-neutral"],
    interfaceRefs: ["if-neutral-view", "if-neutral-boundary"],
    entityRefs: ["entity-neutral"],
    scopeRefs: ["scope-neutral"],
    acceptanceRefs: ["AC-neutral"],
    entry: { type: "page", ref: "surface-neutral", label: "Neutral" },
    steps: [{
      stepId: "step-neutral",
      actor: "user",
      action: "Provide field values and continue",
      systemResponse: "Declared result is visible",
      interfaceRefs: ["if-neutral-boundary"],
      stateMachineRefs: ["sm-neutral"],
    }],
    outcomes: [{ type: "success", description: "Result is persisted or reflected in state." }],
  }],
  stateMachines: [{
    stateMachineId: "sm-neutral",
    name: "Neutral state",
    entityRef: "entity-neutral",
    entityRefs: ["entity-neutral"],
    moduleRefs: ["module-neutral"],
    scopeRefs: ["scope-neutral"],
    acceptanceRefs: ["AC-neutral"],
    states: [{ stateId: "draft", name: "Draft", terminal: false }, { stateId: "done", name: "Done", terminal: true }],
    initialState: "draft",
    events: [{ eventId: "continue", name: "Continue" }],
    transitions: [{ transitionId: "tr-neutral", from: "draft", to: "done", event: "continue", guards: ["fieldA exists"], effects: ["result visible"] }],
    rules: [{ ruleId: "rule-neutral", description: "Cannot continue without fieldA.", acceptanceRefs: ["AC-neutral"] }],
  }],
  frontendExperience: {
    required: true,
    kind: "web_app",
    experienceLevel: "usable_internal_product",
    surfaces: [{
      surfaceId: "surface-neutral",
      name: "Neutral",
      purpose: "Collect fields and show the declared result.",
      userRoleRefs: ["user"],
      workflowRefs: ["flow-neutral"],
      moduleRefs: ["module-neutral"],
    }],
    navigation: {
      required: true,
      pattern: "single_page_primary_action",
      items: [{ label: "Neutral", targetSurfaceRef: "surface-neutral" }],
    },
    interactionStates: ["idle", "loading", "success", "error"],
    mustNot: [],
    notes: [],
  },
  acceptanceMatrix: [{
    acceptanceId: "AC-neutral",
    priority: "must",
    statement: "The screen reflects the declared result after the current interaction.",
    coverageStatus: "covered",
    coverage: [{ type: "user_flow", refs: ["flow-neutral"], description: "Neutral flow." }],
    verificationHints: [{ kind: "integration", description: "Exercise the boundary through the UI." }],
  }],
  risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
  handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
  createdAt: now(),
  updatedAt: now(),
};

const [closureRequirement] = buildWorkflowClosureRequirements(aac);
assert.ok(closureRequirement, "AAC structure must derive a workflow closure requirement.");
assert.equal(closureRequirement.closureId, "closure:flow-neutral:step-neutral");
assert.deepEqual(closureRequirement.interfaceRefs, ["if-neutral-boundary"]);
assert.equal(closureRequirement.derivation.source, "aac_frontend_surface_userflow_interface");

async function verifyTaskPlanRequestProjection() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-workflow-closure-"));
  const deliveryId = "delivery-workflow-closure";
  const phaseId = "phase-1";
  const pgcRef = `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`;
  const aacRef = `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`;
  try {
    writeJson(projectFile(root, ".loom/config.json"), {
      schemaVersion: 1,
      project: {
        name: "workflow-closure-fixture",
        createdAtRoot: root,
      },
      defaults: {
        language: "auto",
        mode: "build",
        verificationLevel: "standard",
      },
      git: { policy: "local" },
      features: {
        plan: true,
        build: true,
        review: true,
        repair: true,
        deploy: false,
      },
      createdAt: now(),
      updatedAt: now(),
    });
    writeJson(projectFile(root, ".loom/status.json"), {
      schemaVersion: 1,
      activeDeliveryId: deliveryId,
      lastCompletedDeliveryId: null,
      deliveries: [{
        deliveryId,
        status: "planning",
        requestSummary: "Workflow closure fixture.",
        activePhaseId: phaseId,
        indexRef: `.loom/deliveries/${deliveryId}/index.json`,
        updatedAt: now(),
      }],
      effectiveNextAction: {
        type: "taskplan_generation",
        deliveryId,
        phaseId,
        reason: "AAC_READY",
        targetNode: "task_plan",
      },
      phase: "planning",
      current: {
        requirementId: null,
        planId: null,
        taskId: null,
        reviewId: null,
        repairId: null,
        deploymentId: null,
      },
      lastAction: null,
      nextAction: "plan",
      updatedAt: now(),
    });
    writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/index.json`), {
      schemaVersion: "1.0",
      deliveryId,
      status: "planning",
      requestSummary: "Workflow closure fixture.",
      roadmapId: null,
      activePhaseId: phaseId,
      phases: [{
        phaseId,
        name: "Phase 1",
        status: "planning",
        latestRefs: {
          planningContract: pgcRef,
          architectureArtifact: aacRef,
        },
        nextAction: {
          type: "taskplan_generation",
          deliveryId,
          phaseId,
          reason: "AAC_READY",
          targetNode: "task_plan",
        },
      }],
      createdAt: now(),
      updatedAt: now(),
    });
    writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), baseline);
    writeJson(projectFile(root, pgcRef), pgc);
    writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/latest.json`), {
      schemaVersion: "1.0",
      planningContractId: pgc.planningContractId,
      contractRef: pgcRef,
      updatedAt: now(),
    });
    writeJson(projectFile(root, aacRef), aac);
    writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/latest.json`), {
      schemaVersion: "1.0",
      architectureArtifactContractId: aac.architectureArtifactContractId,
      contractRef: aacRef,
      planningContractId: pgc.planningContractId,
      updatedAt: now(),
    });

    const { request } = await createTaskPlanRequest({ projectRoot: root, deliveryId, phaseId });
    const transferRequirements = request.contextProjection.requirementDetailTransfer.workflowClosureRequirements;
    assert.equal(transferRequirements.length, 1, "TaskPlan request must expose workflow closure requirements in requirementDetailTransfer.");
    assert.equal(transferRequirements[0].closureId, closureRequirement.closureId);
    assert.equal(request.generationRules.workflowClosureRules.requirements, undefined, "TaskPlan generationRules must not duplicate full workflow closure requirements.");
    assert.deepEqual(request.generationRules.workflowClosureRules.requirementSource.closureRequirementIds, [closureRequirement.closureId], "TaskPlan generationRules must expose workflow closure requirement refs.");
    assert.equal(request.generationRules.workflowClosureRules.derivationAuthority, "AAC structure only. No acceptance text, phase title, or business keyword scanning is used.");
    assert.equal(request.outputContract.workflowClosureRequirements, undefined, "TaskPlan outputContract must not duplicate full workflow closure requirements.");
    assert.deepEqual(request.outputContract.workflowClosureRequirementRefs.closureRequirementIds, [closureRequirement.closureId], "TaskPlan outputContract must expose workflow closure requirement refs.");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function baseTaskPlan(task) {
  return {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-workflow-closure",
    version: 1,
    status: "ready",
    source: {
      roadmapId: null,
      phaseId: "phase-1",
      planningGenerationContractId: pgc.planningContractId,
      architectureArtifactContractId: aac.architectureArtifactContractId,
      technicalBaselineId: baseline.technicalBaselineId,
    },
    scopeSnapshot: {
      includedScopeRefs: ["scope-neutral"],
      excludedScopeRefs: [],
      deferredScopeRefs: [],
      acceptanceRefs: ["AC-neutral"],
    },
    planningPolicy: {
      taskGranularity: "engineering_increment",
      groupGranularity: "engineering_capability",
      allowTaskSplitDuringRepair: true,
      allowTaskMergeDuringRepair: true,
    },
    groups: [{
      groupId: "group-neutral",
      title: "Neutral workflow",
      objective: "Deliver the neutral workflow.",
      dependsOn: [],
      scopeRefs: ["scope-neutral"],
      acceptanceRefs: ["AC-neutral"],
      taskIds: [task.taskId],
    }],
    tasks: [task],
    handoff: { readyForExecution: true, nextNode: "task_execution", blockedReasons: [] },
    createdAt: now(),
    updatedAt: now(),
  };
}

function taskBase(overrides = {}) {
  return {
    taskId: "task-neutral",
    groupId: "group-neutral",
    title: "Implement neutral workflow",
    taskKind: "ui_flow_increment",
    implementationActions: ["create_or_update_ui_flow"],
    objective: "Implement the neutral workflow.",
    dependsOn: [],
    scopeRefs: ["scope-neutral"],
    acceptanceRefs: ["AC-neutral"],
    writeBoundary: {
      forbiddenPaths: [".loom"],
      artifactRefs: {
        modules: ["module-neutral"],
        entities: ["entity-neutral"],
        interfaces: ["if-neutral-view"],
        userFlows: ["flow-neutral"],
        stateMachines: ["sm-neutral"],
        decisions: [],
        risks: [],
      },
    },
    verificationIntents: [{
      verificationId: "verify-neutral",
      acceptanceRefs: ["AC-neutral"],
      behavior: "Verify the neutral workflow.",
      preferredEvidence: ["manual_command_output"],
      acceptableEvidence: ["manual_command_output", "static_check"],
    }],
    frontendExperienceRequirement: {
      frontendExperienceRef: "architectureArtifactContractRef#/frontendExperience",
      experienceLevel: "usable_internal_product",
      mustSatisfy: ["surface fits frontendExperience.surfaces"],
    },
    ...overrides,
  };
}

async function main() {
await verifyTaskPlanRequestProjection();

const invalidTaskPlan = baseTaskPlan(taskBase());
const invalidPlanResult = validateTaskPlanCandidate(invalidTaskPlan, pgc, aac, baseline);
assert.ok(
  invalidPlanResult.issues.some((issue) => issue.code === "WORKFLOW_CLOSURE_NOT_ASSIGNED"),
  "TaskPlan validator must reject a static/UI-only task that does not structurally own workflow closure.",
);

const closureTask = taskBase({
  implementationActions: ["create_or_update_ui_flow", "wire_reference_in_api_or_ui", "add_or_update_tests"],
  writeBoundary: {
    forbiddenPaths: [".loom"],
    artifactRefs: {
      modules: ["module-neutral"],
      entities: ["entity-neutral"],
      interfaces: ["if-neutral-boundary"],
      userFlows: ["flow-neutral"],
      stateMachines: ["sm-neutral"],
      decisions: [],
      risks: [],
    },
  },
  verificationIntents: [{
    verificationId: "verify-neutral",
    acceptanceRefs: ["AC-neutral"],
    behavior: "Verify the user action invokes the declared interface and shows the result.",
    preferredEvidence: ["automated_test"],
    acceptableEvidence: ["automated_test", "runtime_api_check", "manual_command_output"],
  }],
  frontendExperienceRequirement: {
    frontendExperienceRef: "architectureArtifactContractRef#/frontendExperience",
    experienceLevel: "usable_internal_product",
    mustSatisfy: ["workflow closure uses wired data binding"],
    executionGuidance: {
      closureRequirementRefs: [closureRequirement],
    },
  },
});
const validTaskPlan = baseTaskPlan(closureTask);
const validPlanResult = validateTaskPlanCandidate(validTaskPlan, pgc, aac, baseline);
assert.equal(
  validPlanResult.issues.some((issue) => issue.code === "WORKFLOW_CLOSURE_NOT_ASSIGNED"),
  false,
  "TaskPlan validator must accept a task that structurally owns workflow closure.",
);

const taskExecutionRequest = {
  requestId: "exec-neutral",
  source: {
    taskId: closureTask.taskId,
    taskPlanId: validTaskPlan.taskPlanId,
  },
  task: closureTask,
};

const staticSatisfiedResult = {
  schemaVersion: "1.0",
  taskResultId: "result-neutral",
  taskId: closureTask.taskId,
  taskPlanId: validTaskPlan.taskPlanId,
  status: "completed_with_notes",
  changedFiles: ["src/neutral.tsx"],
  noChangeReason: null,
  verificationResults: [{
    verificationId: "verify-neutral",
    status: "passed",
    evidenceType: "automated_test",
    summary: "Fixture verification passed.",
  }],
  selfRepairSummary: { attempted: false, attemptCount: 0, stopReason: "not_attempted", progressObserved: false },
  failure: null,
  executionContinuity: { taskResultSubmittedAfterVerification: true, agentOwnedLongRunningWork: "none", notes: [] },
  frontendExperienceSelfCheck: {
    status: "satisfied",
    surfacesTouched: ["surface-neutral"],
    workflowsCovered: [{ workflowRef: "flow-neutral", coverage: "static_only", evidenceRefs: [], summary: "Static page only." }],
    userActionsImplemented: ["Provide field values and continue"],
    interactionStatesCovered: ["idle", "success"],
    dataBinding: {
      mode: "static_only_with_reason",
      interfaceRefs: [],
      evidenceRefs: [],
      notes: ["Boundary not wired."],
    },
    knownGaps: ["Boundary wiring remains."],
    notes: ["Fixture intentionally violates closure."],
  },
  notes: ["Fixture intentionally violates closure."],
  blockedReasons: [],
  createdAt: now(),
  updatedAt: now(),
};
const taskResultValidation = validateTaskResult(staticSatisfiedResult, taskExecutionRequest);
assert.ok(
  taskResultValidation.issues.some((issue) => issue.code === "TASK_RESULT_WORKFLOW_CLOSURE_INVALID"),
  "TaskResult validator must reject satisfied static-only self-checks for closure-required tasks.",
);

const reviewRequest = {
  requestId: "review-neutral",
  source: {
    phaseId: "phase-1",
    taskPlanId: validTaskPlan.taskPlanId,
    taskPlanRunId: "run-neutral",
  },
  reviewScope: {
    type: "phase_run",
    acceptanceRefs: ["AC-neutral"],
    nextPhasePreview: { kind: "none", reason: "No next phase in fixture." },
  },
  outputContract: {
    changeContextMode: "current_file_content",
    allowedRefs: {
      taskIds: [closureTask.taskId],
      groupIds: ["group-neutral"],
      acceptanceRefs: ["AC-neutral"],
      taskResultIds: ["result-neutral"],
      changedFilePaths: ["src/neutral.tsx"],
      readRefs: ["review-packet", "change-context", "result-neutral", "src/neutral.tsx"],
      verificationEvidenceRefs: ["verify-neutral", "result-neutral:verify-neutral", "task-neutral:verify-neutral"],
    },
    reviewSignals: [{
      kind: "frontend_workflow_closure",
      closureId: closureRequirement.closureId,
      closureSatisfied: false,
      taskRefs: [closureTask.taskId],
      taskResultId: "result-neutral",
      recommendedNextAction: "execution_repair",
    }],
  },
};

const approvedReviewResult = {
  schemaVersion: "1.0",
  reviewId: "review-neutral",
  source: {
    requestId: "review-neutral",
    phaseId: "phase-1",
    taskPlanId: validTaskPlan.taskPlanId,
    taskPlanRunId: "run-neutral",
  },
  decision: "approved",
  findings: [],
  coverageAssessment: {
    mustAcceptance: [{
      acceptanceRef: "AC-neutral",
      status: "satisfied",
      supportingTaskResults: ["result-neutral"],
      evidenceStatus: "sufficient",
      notes: [],
    }],
    summary: { totalMust: 1, satisfied: 1, insufficientEvidence: 0, notSatisfied: 0, notReviewed: 0 },
  },
  limitations: [],
  pendingActions: [],
  nextAction: { type: "done", reason: "Fixture approved." },
  createdAt: now(),
  updatedAt: now(),
};
const reviewValidation = validateReviewResult(approvedReviewResult, reviewRequest);
assert.ok(
  reviewValidation.issues.some((issue) => issue.code === "REVIEW_RESULT_STATUS_INCONSISTENT" && issue.path === "/decision"),
  "Review validator must reject approved results when a workflow closure signal is unsatisfied.",
);

console.log("workflow closure protocol verification passed");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
