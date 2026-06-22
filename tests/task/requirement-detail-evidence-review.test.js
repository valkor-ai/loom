#!/usr/bin/env node

const assert = require("node:assert/strict");
const { validateTaskResult, validateReviewResult } = require("../../dist/core/validators");

const now = "2026-05-24T00:00:00.000Z";

const task = {
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
};

const executionRequest = {
  source: {
    taskPlanId: "taskplan-001",
    taskId: "task-review",
    taskPlanRunId: "run-001",
  },
  task,
  sourceContext: {
    requirementDetailSnapshot: [{
      detailId: "detail-review-lifecycle",
      title: "Review lifecycle",
      aacCoverage: { coverageStatus: "covered", artifactRefs: { interfaces: ["interface-review"] } },
      verificationIntentRefs: ["verify-review"],
    }],
  },
};

function baseTaskResult() {
  return {
    schemaVersion: "1.0",
    taskResultId: "result-review",
    taskId: "task-review",
    taskPlanId: "taskplan-001",
    status: "completed",
    changedFiles: ["src/review.ts"],
    noChangeReason: null,
    verificationResults: [{
      verificationId: "verify-review",
      status: "passed",
      evidenceType: "automated_test",
      summary: "Review workflow verification passed.",
    }],
    selfRepairSummary: null,
    failure: null,
    executionContinuity: {
      taskResultSubmittedAfterVerification: true,
      agentOwnedLongRunningWork: "none",
      notes: [],
    },
    notes: [],
    requirementDetailEvidence: [{
      detailId: "detail-review-lifecycle",
      status: "satisfied",
      verificationIds: ["verify-review"],
      evidenceRefs: ["src/review.ts", "verify-review"],
      summary: "Implemented review lifecycle and verified status readback.",
    }],
    blockedReasons: [],
    createdAt: now,
    updatedAt: now,
  };
}

const validTaskResult = validateTaskResult(baseTaskResult(), executionRequest);
assert.equal(validTaskResult.issues.length, 0, JSON.stringify(validTaskResult.issues, null, 2));

const missingEvidence = baseTaskResult();
delete missingEvidence.requirementDetailEvidence;
assert.ok(validateTaskResult(missingEvidence, executionRequest).issues.some((issue) => issue.code === "TASK_RESULT_DETAIL_EVIDENCE_INVALID"));

const completedButPartial = baseTaskResult();
completedButPartial.requirementDetailEvidence[0].status = "partially_satisfied";
assert.ok(validateTaskResult(completedButPartial, executionRequest).issues.some((issue) => issue.code === "TASK_RESULT_DETAIL_EVIDENCE_INVALID"));

const badVerification = baseTaskResult();
badVerification.requirementDetailEvidence[0].verificationIds = ["missing-verification"];
const normalizedVerificationIds = validateTaskResult(badVerification, executionRequest);
assert.equal(normalizedVerificationIds.issues.length, 0, JSON.stringify(normalizedVerificationIds.issues, null, 2));
assert.deepEqual(normalizedVerificationIds.value.requirementDetailEvidence[0].verificationIds, ["verify-review"]);

const reviewRequest = {
  schemaVersion: "1.0",
  requestId: "review-001",
  requestType: "review_gate",
  source: {
    roadmapId: null,
    phaseId: "phase-1",
    taskPlanId: "taskplan-001",
    taskPlanRunId: "run-001",
    technicalBaselineId: "tb-001",
    architectureArtifactContractId: "aac-001",
  },
  reviewScope: {
    type: "phase_run",
    groupIds: ["group-app"],
    taskIds: ["task-review"],
    acceptanceRefs: ["AC-1"],
    nextPhaseId: null,
    nextPhasePreview: { kind: "none", reason: "No next phase." },
  },
  reviewPacketRef: ".loom/review-packet.json",
  changeContextRef: ".loom/change-context.json",
  enumRefs: {
    readRefType: ["review_packet", "change_context", "diff_ref", "changed_file", "task_result", "verification_evidence"],
    evidenceRefType: ["task_result", "verification_result", "diff_ref", "changed_file", "manual_note"],
  },
  outputContract: {
    reviewSignals: [{
      signalId: "sig-requirement-detail-detail-review-lifecycle",
      kind: "requirement_detail_evidence",
      detailId: "detail-review-lifecycle",
      taskRefs: ["task-review"],
      detailSatisfied: false,
      actualStatus: "missing",
      recommendedNextAction: "execution_repair",
    }],
    allowedRefs: {
      taskIds: ["task-review"],
      groupIds: ["group-app"],
      acceptanceRefs: ["AC-1"],
      taskResultIds: ["result-review"],
      changedFilePaths: ["src/review.ts"],
      verificationEvidenceRefs: ["verify-review", "result-review:verify-review", "task-review:verify-review"],
      readRefs: [".loom/review-packet.json", ".loom/change-context.json", "result-review", "src/review.ts"],
    },
  },
  reviewRules: { commonRules: [], changeSetRules: [] },
  submitCommand: {},
  createdAt: now,
};

function approvedReviewResult() {
  return {
    schemaVersion: "1.0",
    reviewId: "review-001",
    source: {
      requestId: "review-001",
      phaseId: "phase-1",
      taskPlanId: "taskplan-001",
      taskPlanRunId: "run-001",
    },
    decision: "approved",
    findings: [],
    coverageAssessment: {
      mustAcceptance: [{
        acceptanceRef: "AC-1",
        status: "satisfied",
        supportingTaskResults: ["result-review"],
        evidenceStatus: "sufficient",
        notes: [],
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
      reason: "Approved.",
      targetNode: "done",
    },
    createdAt: now,
    updatedAt: now,
  };
}

assert.ok(validateReviewResult(approvedReviewResult(), reviewRequest).issues.some((issue) => issue.code === "REVIEW_RESULT_STATUS_INCONSISTENT"));

const repairReviewResult = approvedReviewResult();
repairReviewResult.decision = "changes_requested";
repairReviewResult.findings = [{
  findingId: "finding-detail-evidence",
  severity: "major",
  severityClass: "blocking",
  evidenceKind: "static",
  failureClass: "insufficient_evidence",
  category: "evidence_insufficient",
  summary: "Requirement detail evidence is missing.",
  evidence: "reviewSignals reports detailSatisfied=false for detail-review-lifecycle.",
  readRefs: [{ type: "review_packet", ref: ".loom/review-packet.json", reason: "Review packet contains task result evidence." }],
  evidenceRefs: [{ type: "task_result", ref: "result-review", reason: "Task result lacks required detail evidence." }],
  groupRefs: ["group-app"],
  taskRefs: ["task-review"],
  acceptanceRefs: ["AC-1"],
  artifactRefs: {},
  location: null,
  taskRelevance: "direct",
  scopeRelation: "within_task_changed_files",
  introducedByCurrentTask: "yes",
  recommendedNextAction: "execution_repair",
}];
repairReviewResult.coverageAssessment.mustAcceptance[0].status = "insufficient_evidence";
repairReviewResult.coverageAssessment.mustAcceptance[0].evidenceStatus = "insufficient";
repairReviewResult.coverageAssessment.summary.satisfied = 0;
repairReviewResult.coverageAssessment.summary.insufficientEvidence = 1;
repairReviewResult.nextAction = {
  type: "execution_repair",
  reason: "Repair missing requirement detail evidence.",
  targetNode: "execution_repair",
  targetTaskIds: ["task-review"],
  findingRefs: ["finding-detail-evidence"],
};
assert.equal(validateReviewResult(repairReviewResult, reviewRequest).issues.length, 0);

console.log("Requirement detail evidence and review verification passed.");
