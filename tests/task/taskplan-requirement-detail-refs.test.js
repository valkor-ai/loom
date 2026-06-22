#!/usr/bin/env node

const assert = require("node:assert/strict");
const { validateTaskPlanCandidate } = require("../../dist/core/validators");

const now = "2026-05-24T00:00:00.000Z";

const baseline = {
  schemaVersion: "1.0",
  technicalBaselineId: "tb-001",
  status: "confirmed",
  source: "user_confirmed",
  projectKind: "greenfield",
  scope: "project",
  stack: { languages: ["typescript"] },
  constraints: [],
  evidence: [{ reason: "test fixture" }],
  approval: { type: "user_confirmed", confirmedAt: now, reason: "test fixture" },
  confidence: "high",
  createdAt: now,
  updatedAt: now,
};

const pgc = {
  schemaVersion: "1.0",
  planningContractId: "pgc-phase-1",
  status: "ready",
  source: {
    brainstormRunId: "bs-001",
    brainstormContractId: "bc-001",
    roadmapId: null,
    phaseId: "phase-1",
    technicalBaselineId: "tb-001",
  },
  phaseScope: {
    phaseName: "Phase 1",
    phaseGoal: "Review applications.",
    included: [{ scopeId: "scope-app", label: "Applications", items: ["Review application lifecycle."], source: "user_confirmed" }],
    deferred: [],
    excluded: [],
    acceptanceCandidates: [{
      id: "AC-1",
      statement: "Staff can review an application and see updated status.",
      capabilityRefs: ["cap-app"],
      sourceRefs: ["src-1"],
      priority: "must",
    }],
  },
  contextRefs: { brainstormContractRef: ".loom/deliveries/d/brainstorms/contract.json" },
  technicalBaseline: {
    technicalBaselineId: "tb-001",
    status: "confirmed",
    scope: "project",
    summary: {},
    mustFollow: true,
  },
  planningInputs: {
    businessGoal: "Review applications.",
    actors: [],
    capabilityGroups: [],
    businessFlows: [],
    sourceRefs: ["src-1"],
    contextNotes: [],
  },
  requirementDetails: {
    schemaVersion: "1.0",
    authority: "brainstorm_contract",
    sourceBrainstormContractRef: ".loom/deliveries/d/brainstorms/contract.json",
    items: [
      {
        detailId: "detail-review-lifecycle",
        kind: "business_flow",
        title: "Review lifecycle",
        summary: "Staff query, view, approve, and see updated status.",
        requiredForCurrentPhase: true,
        priority: "must",
        sourceFieldRefs: ["brainstorm.domainModel.businessFlows[0].summary"],
        sourceRefs: ["src-1"],
        scopeRefs: ["scope-app"],
        acceptanceRefs: ["AC-1"],
        conceptRefs: [],
        frontendRefs: [],
        impactTags: ["business_flow", "acceptance"],
        lifecycleStage: "approve_or_process",
        quality: "usable",
        unresolvedNote: null,
      },
      {
        detailId: "detail-future-reporting",
        kind: "scope_item",
        title: "Future reporting",
        summary: "Reporting is known but not implemented in this phase.",
        requiredForCurrentPhase: true,
        priority: "should",
        sourceFieldRefs: ["brainstorm.scope.deferred[0]"],
        sourceRefs: ["src-2"],
        scopeRefs: ["scope-app"],
        acceptanceRefs: [],
        conceptRefs: [],
        frontendRefs: [],
        impactTags: ["scope"],
        lifecycleStage: "not_applicable",
        quality: "thin",
        unresolvedNote: "Deferred by current phase boundary.",
      },
    ],
    extractionWarnings: [],
  },
  planningRules: {
    scopeIsolation: { onlyPlanCurrentPhase: true, forbidDeferredScopeImplementation: true, forbidFuturePhaseImplementation: true },
    outputRequirements: { mustCreateArchitectureArtifactContract: true, mustCreateTaskPlan: true, taskPlanMustReferenceAcceptance: true },
    deployment: { defaultEnabled: false, requiresExplicitUserRequest: true },
  },
  qualityGates: { requiresArchitectureBeforeTaskPlan: true, requiresAcceptanceCoverage: true, requiresVerificationEvidence: true },
  handoff: { readyForArchitecture: true, readyForTaskPlan: false, blockingReasons: [], nextNode: "architecture_artifact_contract" },
  createdAt: now,
  updatedAt: now,
};

const aac = {
  schemaVersion: "1.0",
  architectureArtifactContractId: "aac-001",
  status: "ready",
  source: {
    planningGenerationContractId: "pgc-phase-1",
    technicalBaselineId: "tb-001",
    brainstormContractId: "bc-001",
    roadmapId: null,
    phaseId: "phase-1",
  },
  engineeringBoundary: {
    projectKind: "greenfield",
    strategy: "create_minimal_phase_structure",
    applications: [{ appId: "app-main", type: "web", root: "." }],
    modules: [{ moduleId: "module-app", appId: "app-main", paths: ["src"], responsibility: "Review application workflow." }],
    creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
  },
  modules: [{ moduleId: "module-app", name: "Application module", responsibility: "Review application workflow.", dependsOn: [], scopeRefs: ["scope-app"], acceptanceRefs: ["AC-1"] }],
  dataModel: {
    entities: [{
      entityId: "entity-application",
      name: "Application",
      type: "internal",
      implementationIntent: "full",
      moduleRefs: ["module-app"],
      scopeRefs: ["scope-app"],
      acceptanceRefs: ["AC-1"],
      fields: [{ fieldId: "field-status", name: "status", type: "string", required: true }],
      constraints: [{ constraintId: "constraint-status", type: "business_rule", description: "Status changes after review." }],
    }],
    relationships: [],
    constraints: [],
  },
  interfaces: [{
    interfaceId: "interface-review",
    name: "reviewApplication",
    type: "service_method",
    role: "command",
    moduleRefs: ["module-app"],
    entityRefs: ["entity-application"],
    scopeRefs: ["scope-app"],
    acceptanceRefs: ["AC-1"],
    requestSchema: [{ fieldId: "request-decision", name: "decision", type: "string", required: true }],
    responseSchema: [{ fieldId: "response-status", name: "status", type: "string", required: true }],
    errorSchema: [],
  }],
  userFlows: [{
    flowId: "flow-review",
    name: "Review application",
    kind: "user_interaction",
    moduleRefs: ["module-app"],
    interfaceRefs: ["interface-review"],
    entityRefs: ["entity-application"],
    scopeRefs: ["scope-app"],
    acceptanceRefs: ["AC-1"],
    entry: { type: "page", ref: "/applications", label: "Applications" },
    steps: [{ stepId: "step-review", actor: "staff", action: "Review application.", interfaceRefs: ["interface-review"], stateMachineRefs: [] }],
    outcomes: [{ type: "success", description: "Status updated." }],
  }],
  stateMachines: [],
  acceptanceMatrix: [{
    acceptanceId: "AC-1",
    priority: "must",
    statement: "Staff can review an application and see updated status.",
    coverageStatus: "covered",
    coverage: [{ type: "user_flow", refs: ["flow-review"], description: "Review flow covers the acceptance." }],
    verificationHints: [{ kind: "integration", description: "Verify review flow." }],
  }],
  detailCoverage: [
    {
      detailId: "detail-review-lifecycle",
      coverageStatus: "covered",
      artifactRefs: {
        modules: ["module-app"],
        entities: ["entity-application"],
        fields: ["field-status", "request-decision", "response-status"],
        constraints: ["constraint-status"],
        interfaces: ["interface-review"],
        userFlows: ["flow-review"],
        stateMachines: [],
        frontendDataViews: [],
        frontendActions: [],
        frontendOperationPaths: [],
        acceptanceMatrix: ["AC-1"],
      },
    },
    {
      detailId: "detail-future-reporting",
      coverageStatus: "not_applicable",
      artifactRefs: {
        modules: [],
        entities: [],
        fields: [],
        constraints: [],
        interfaces: [],
        userFlows: [],
        stateMachines: [],
        frontendDataViews: [],
        frontendActions: [],
        frontendOperationPaths: [],
        acceptanceMatrix: [],
      },
      reason: "Deferred by current phase boundary.",
    },
  ],
  risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
  handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
  createdAt: now,
  updatedAt: now,
};

function baseTaskPlan() {
  return {
    schemaVersion: "1.0",
    taskPlanId: "taskplan-001",
    version: 1,
    status: "ready",
    source: {
      roadmapId: null,
      phaseId: "phase-1",
      planningGenerationContractId: "pgc-phase-1",
      architectureArtifactContractId: "aac-001",
      technicalBaselineId: "tb-001",
    },
    scopeSnapshot: {
      includedScopeRefs: ["scope-app"],
      excludedScopeRefs: [],
      deferredScopeRefs: [],
      acceptanceRefs: ["AC-1"],
    },
    planningPolicy: {
      taskGranularity: "engineering_increment",
      groupGranularity: "engineering_capability",
      allowTaskSplitDuringRepair: true,
      allowTaskMergeDuringRepair: false,
    },
    groups: [{
      groupId: "group-app",
      title: "Application review",
      objective: "Deliver application review workflow.",
      dependsOn: [],
      scopeRefs: ["scope-app"],
      acceptanceRefs: ["AC-1"],
      taskIds: ["task-review"],
    }],
    tasks: [{
      taskId: "task-review",
      groupId: "group-app",
      title: "Implement review workflow",
      taskKind: "feature_increment",
      implementationActions: ["create_or_update_interface", "add_or_update_tests"],
      objective: "Implement application review lifecycle.",
      dependsOn: [],
      scopeRefs: ["scope-app"],
      acceptanceRefs: ["AC-1"],
      requirementDetailRefs: ["detail-review-lifecycle"],
      writeBoundary: {
        forbiddenPaths: [".loom"],
        artifactRefs: {
          modules: ["module-app"],
          entities: ["entity-application"],
          interfaces: ["interface-review"],
          userFlows: ["flow-review"],
          stateMachines: [],
          decisions: [],
          risks: [],
        },
      },
      verificationIntents: [{
        verificationId: "verify-review",
        acceptanceRefs: ["AC-1"],
        requirementDetailRefs: ["detail-review-lifecycle"],
        behavior: "Verify review updates status and returns readback.",
        preferredEvidence: ["automated_test"],
        acceptableEvidence: ["automated_test", "manual_command_output"],
      }],
    }],
    handoff: { readyForExecution: true, nextNode: "task_execution", blockedReasons: [] },
    createdAt: now,
    updatedAt: now,
  };
}

function validate(candidate) {
  return validateTaskPlanCandidate(candidate, pgc, aac, baseline);
}

const valid = validate(baseTaskPlan());
assert.equal(valid.status, "ready", JSON.stringify(valid.issues, null, 2));
assert.equal(valid.issues.length, 0, JSON.stringify(valid.issues, null, 2));

const invalidTaskRef = baseTaskPlan();
invalidTaskRef.tasks[0].requirementDetailRefs = ["missing-detail"];
assert.ok(validate(invalidTaskRef).issues.some((issue) => issue.code === "DETAIL_REF_INVALID"));

const invalidVerificationRef = baseTaskPlan();
invalidVerificationRef.tasks[0].verificationIntents[0].requirementDetailRefs = ["missing-detail"];
assert.ok(validate(invalidVerificationRef).issues.some((issue) => issue.code === "DETAIL_REF_INVALID"));

const verificationNotSubset = baseTaskPlan();
verificationNotSubset.tasks[0].requirementDetailRefs = [];
assert.ok(validate(verificationNotSubset).issues.some((issue) => issue.code === "INVALID_VERIFICATION_INTENT"));

const missingTaskAssignment = baseTaskPlan();
delete missingTaskAssignment.tasks[0].requirementDetailRefs;
assert.ok(validate(missingTaskAssignment).issues.some((issue) =>
  issue.code === "DETAIL_TASK_ASSIGNMENT_MISSING" &&
  issue.path.includes("/tasks/requirementDetailRefs/")
));

const missingVerificationAssignment = baseTaskPlan();
delete missingVerificationAssignment.tasks[0].verificationIntents[0].requirementDetailRefs;
assert.ok(validate(missingVerificationAssignment).issues.some((issue) =>
  issue.code === "DETAIL_TASK_ASSIGNMENT_MISSING" &&
  issue.path.includes("/tasks/verificationIntents/requirementDetailRefs/")
));

console.log("TaskPlan requirement detail refs verification passed.");
