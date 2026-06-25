#!/usr/bin/env node

const assert = require("node:assert/strict");
const { validateArchitectureArtifactCandidate } = require("../../../../src/ts/reference/dist/core/validators");

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
    items: [{
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
    }],
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

function baseAac() {
  return {
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
    detailCoverage: [{
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
      reason: null,
    }],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    createdAt: now,
    updatedAt: now,
  };
}

function validate(candidate) {
  return validateArchitectureArtifactCandidate(candidate, pgc, baseline);
}

const valid = validate(baseAac());
assert.equal(valid.status, "ready", JSON.stringify(valid.issues, null, 2));
assert.equal(valid.issues.length, 0, JSON.stringify(valid.issues, null, 2));

const missing = baseAac();
missing.detailCoverage = [];
assert.ok(validate(missing).issues.some((issue) => issue.code === "DETAIL_COVERAGE_INVALID"));

const badDetail = baseAac();
badDetail.detailCoverage[0].detailId = "detail-not-from-pgc";
assert.ok(validate(badDetail).issues.some((issue) => issue.code === "DETAIL_REF_INVALID"));

const noRefs = baseAac();
noRefs.detailCoverage[0].artifactRefs = {
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
};
assert.ok(validate(noRefs).issues.some((issue) => issue.code === "DETAIL_COVERAGE_INVALID"));

const partialWithoutReason = baseAac();
partialWithoutReason.detailCoverage[0].coverageStatus = "partial";
assert.ok(validate(partialWithoutReason).issues.some((issue) => issue.code === "DETAIL_COVERAGE_INVALID"));

const badArtifactRef = baseAac();
badArtifactRef.detailCoverage[0].artifactRefs.interfaces = ["missing-interface"];
assert.ok(validate(badArtifactRef).issues.some((issue) => issue.code === "UNKNOWN_ARTIFACT_REF"));

console.log("AAC detail coverage verification passed.");
