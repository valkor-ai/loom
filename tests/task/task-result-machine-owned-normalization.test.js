#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { validateTaskResult } = require("../../dist/core/validators");
const { submitDeployExecutionRepairResult } = require("../../dist/core/operations/repair");
const { getDeploymentRepairPaths } = require("../../dist/core/deployment/paths");

const now = "2026-06-19T00:00:00.000Z";

const task = {
  taskId: "task-runtime-ui",
  groupId: "group-runtime",
  title: "Implement runtime UI closure",
  taskKind: "runtime_delivery_closure",
  implementationActions: ["create_or_update_interface", "wire_reference_in_api_or_ui"],
  objective: "Implement and verify a runtime-backed UI path.",
  dependsOn: [],
  scopeRefs: ["scope-runtime"],
  acceptanceRefs: ["AC-runtime"],
  requirementDetailRefs: ["detail-runtime"],
  writeBoundary: {
    forbiddenPaths: [".loom"],
    artifactRefs: {
      modules: ["module-runtime"],
      entities: [],
      interfaces: ["interface-runtime"],
      userFlows: ["flow-runtime"],
      stateMachines: [],
      decisions: [],
      risks: [],
    },
  },
  verificationIntents: [{
    verificationId: "verify-runtime",
    acceptanceRefs: ["AC-runtime"],
    requirementDetailRefs: ["detail-runtime"],
    behavior: "Verify the runtime-backed UI path.",
    preferredEvidence: ["automated_test"],
    acceptableEvidence: ["automated_test", "runtime_api_check"],
  }],
  conceptRefs: ["concept-runtime"],
  frontendExperienceRequirement: {
    frontendExperienceRef: "frontend:runtime",
    executionGuidance: {
      closureRequirementRefs: ["closure:flow-runtime:submit"],
    },
  },
  runtimeDeliveryRequirement: {
    appliesToThisTask: true,
    reason: "Runtime closure must be verified.",
    runtimeDeliveryRef: "runtime:contract",
    affectedContractFields: ["build.command"],
    requiredCodeLevelChecks: [{
      checkId: "rd-check-build",
      contractField: "build.command",
      objective: "Verify build command is executable.",
      acceptableEvidence: ["automated_test"],
    }],
  },
};

const executionRequest = {
  schemaVersion: "1.0",
  requestId: "exec-runtime-ui",
  requestType: "execute_task",
  generationProtocol: {},
  enumRefs: {},
  source: {
    taskPlanId: "taskplan-runtime",
    taskId: task.taskId,
    technicalBaselineId: "tb-runtime",
    architectureArtifactContractId: "aac-runtime",
    taskPlanRunId: "run-runtime",
  },
  task,
  sourceContext: {
    technicalBaseline: {},
    architectureArtifactProjection: {},
    acceptanceSnapshot: [],
    requirementDetailSnapshot: [{
      detailId: "detail-runtime",
      title: "Runtime detail",
      aacCoverage: { coverageStatus: "covered", artifactRefs: { interfaces: ["interface-runtime"] } },
      verificationIntentRefs: ["verify-runtime"],
    }],
    dependencyResults: [],
  },
  executionRules: {},
  blockedOutput: {},
  outputContract: {},
  submitCommand: {},
  createdAt: now,
};

function baseResult() {
  return {
    schemaVersion: "0.0",
    taskResultId: "",
    taskId: "wrong-task",
    taskPlanId: "wrong-plan",
    status: "completed",
    changedFiles: ["src/runtime-ui.ts"],
    verificationResults: [{
      verificationId: "wrong-verification",
      status: "passed",
      evidenceType: "automated_test",
      summary: "Runtime UI verification passed.",
    }],
    selfRepairSummary: null,
    failure: { code: "UNKNOWN_EXECUTION_FAILURE", summary: "This stale failure must be cleared for completed results." },
    executionContinuity: {
      taskResultSubmittedAfterVerification: true,
      agentOwnedLongRunningWork: "none",
      notes: [],
    },
    runtimeDeliveryEvidence: {
      requirementRef: "wrong-runtime-ref",
      codeLevelChecks: [{
        checkId: "generic-build",
        contractField: "build.command",
        status: "passed",
        evidence: "Build command was verified.",
      }],
      commandsRun: [],
      unverifiedItems: [],
    },
    requirementDetailEvidence: [{
      status: "satisfied",
      verificationIds: ["wrong-verification"],
      evidenceRefs: ["src/runtime-ui.ts"],
      summary: "Runtime detail implemented and verified.",
    }],
    conceptEvidence: [{
      evidenceType: "code",
      refs: ["src/runtime-ui.ts"],
      summary: "Runtime concept is implemented by the UI path.",
    }],
    frontendExperienceSelfCheck: {
      requirementRef: "wrong-frontend-ref",
      status: "satisfied",
      dataBinding: {
        mode: "wired",
        closureRequirementIds: ["wrong-closure"],
      },
      knownGaps: [],
    },
  };
}

const normalized = validateTaskResult(baseResult(), executionRequest);
assert.equal(normalized.issues.length, 0, JSON.stringify(normalized.issues, null, 2));
assert.equal(normalized.value.taskId, "task-runtime-ui");
assert.equal(normalized.value.taskPlanId, "taskplan-runtime");
assert.equal(normalized.value.verificationResults[0].verificationId, "verify-runtime");
assert.equal(normalized.value.runtimeDeliveryEvidence.requirementRef, "runtime:contract");
assert.deepEqual(normalized.value.runtimeDeliveryEvidence.checkedFields, ["build.command"]);
assert.equal(normalized.value.runtimeDeliveryEvidence.codeLevelChecks[0].checkId, "rd-check-build");
assert.equal(normalized.value.requirementDetailEvidence[0].detailId, "detail-runtime");
assert.deepEqual(normalized.value.requirementDetailEvidence[0].verificationIds, ["verify-runtime"]);
assert.equal(normalized.value.conceptEvidence[0].conceptRef, "concept-runtime");
assert.equal(normalized.value.frontendExperienceSelfCheck.requirementRef, "frontend:runtime");
assert.deepEqual(normalized.value.frontendExperienceSelfCheck.closureRequirementIds, ["closure:flow-runtime:submit"]);
assert.deepEqual(normalized.value.frontendExperienceSelfCheck.dataBinding.closureRequirementIds, ["closure:flow-runtime:submit"]);

const missingVerification = baseResult();
delete missingVerification.verificationResults;
const missingVerificationValidation = validateTaskResult(missingVerification, executionRequest);
assert.ok(
  missingVerificationValidation.issues.some((issue) => issue.code === "TASK_RESULT_STATUS_INCONSISTENT"),
  JSON.stringify(missingVerificationValidation.issues, null, 2),
);

const missingDetailEvidence = baseResult();
delete missingDetailEvidence.requirementDetailEvidence;
const missingDetailValidation = validateTaskResult(missingDetailEvidence, executionRequest);
assert.ok(
  missingDetailValidation.issues.some((issue) => issue.code === "TASK_RESULT_DETAIL_EVIDENCE_INVALID"),
  JSON.stringify(missingDetailValidation.issues, null, 2),
);

const missingRuntimeCheck = baseResult();
missingRuntimeCheck.runtimeDeliveryEvidence = {
  codeLevelChecks: [],
  commandsRun: [],
  unverifiedItems: [],
};
const missingRuntimeValidation = validateTaskResult(missingRuntimeCheck, executionRequest);
assert.ok(
  missingRuntimeValidation.issues.some((issue) => issue.code === "TASK_RESULT_RUNTIME_CHECK_ID_INVALID"),
  JSON.stringify(missingRuntimeValidation.issues, null, 2),
);

function deployRequest(root, repairId) {
  const paths = getDeploymentRepairPaths(root, repairId);
  return {
    schemaVersion: "1.0",
    repairId,
    repairType: "execution_repair",
    source: "deploy_failure",
    deploymentFailureRef: ".loom/deployment/state/latest-failure.json",
    sourceRefs: {
      runtimeDeliveryRef: "runtime:contract",
      taskPlanRef: null,
      taskPlanRunRef: null,
      reviewResultRef: null,
      deploymentSpecRef: ".loom/deployment/specs/local.json",
    },
    syntheticTask: {
      taskId: "deploy-repair-task",
      taskKind: "runtime_delivery",
      title: "Repair deployment failure",
      objective: "Repair failed runtime field.",
      mutatesOriginalTaskPlan: false,
      relatedTaskIds: [],
      writeBoundary: {
        forbiddenPaths: [".loom"],
      },
      runtimeDeliveryRequirement: {
        appliesToThisTask: true,
        source: "deploy_failure",
        deploymentFailureRef: ".loom/deployment/state/latest-failure.json",
        runtimeDeliveryRef: "runtime:contract",
        affectedContractFields: ["build.command"],
        requiredCodeLevelChecks: [{
          checkId: "deploy-check-build",
          contractField: "build.command",
          objective: "Verify repaired build command.",
          acceptableEvidence: ["manual_command_output"],
        }],
        forbiddenActions: ["Do not edit .loom state."],
      },
    },
    executionRules: {},
    outputContract: {
      format: "json",
      schema: "DeployExecutionRepairTaskResult",
      resultFile: path.relative(root, paths.resultFile),
      schemaShape: {},
      submitCommand: {
        name: "deploy-repair-submit",
        argv: ["deploy", "repair-submit", "--repair-id", repairId, "--result-file", path.relative(root, paths.resultFile)],
      },
    },
    createdAt: now,
  };
}

function deployResult(overrides = {}) {
  return {
    schemaVersion: "0.0",
    repairId: "wrong-repair",
    status: "completed",
    deploymentFailureRef: "wrong-failure",
    changedFiles: ["package.json"],
    runtimeDeliveryEvidence: {
      source: "wrong-source",
      addressedFailedContractFields: [],
      codeLevelChecks: [{
        checkId: "generic-deploy-check",
        status: "passed",
        evidence: "Repaired build command verified.",
      }],
      commandsRun: [],
      unverifiedItems: [],
      ...(overrides.runtimeDeliveryEvidence ?? {}),
    },
    selfRepairSummary: {
      attempted: false,
      attemptCount: 0,
      stopReason: "not_attempted",
      progressObserved: false,
    },
    notes: [],
    ...overrides,
  };
}

async function verifyDeployRepairNormalization() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-machine-fields-"));
  fs.mkdirSync(path.join(root, ".loom"), { recursive: true });
  fs.writeFileSync(path.join(root, ".loom", "config.json"), JSON.stringify({ schemaVersion: "1.0" }, null, 2));

  const repairId = "deploy-repair-001";
  const paths = getDeploymentRepairPaths(root, repairId);
  fs.mkdirSync(paths.repairDir, { recursive: true });
  fs.mkdirSync(path.join(root, ".loom", "deployment", "state"), { recursive: true });
  fs.writeFileSync(path.join(root, ".loom", "deployment", "state", "latest-failure.json"), JSON.stringify({
    schemaVersion: "1.0",
    failureId: "deploy-failure-001",
  }, null, 2));
  const request = deployRequest(root, repairId);
  fs.writeFileSync(paths.requestFile, JSON.stringify(request, null, 2));
  fs.writeFileSync(paths.resultFile, JSON.stringify(deployResult(), null, 2));

  const accepted = await submitDeployExecutionRepairResult({
    projectRoot: root,
    repairId,
    resultFile: path.relative(root, paths.resultFile),
  });
  assert.equal(accepted.accepted, true);

  fs.writeFileSync(paths.resultFile, JSON.stringify(deployResult({
    runtimeDeliveryEvidence: {
      codeLevelChecks: [],
    },
  }), null, 2));
  await assert.rejects(
    () => submitDeployExecutionRepairResult({
      projectRoot: root,
      repairId,
      resultFile: path.relative(root, paths.resultFile),
    }),
    /runtime code-level checks do not match/,
  );
}

verifyDeployRepairNormalization().then(() => {
  console.log("TaskResult and deploy repair machine-owned field normalization verified.");
});
