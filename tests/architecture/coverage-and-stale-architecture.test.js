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

function requestFromCommand(data, root) {
  return data.request ?? hydrateRequest(root, readJson(projectFile(root, data.requestPath ?? data.requestRef)));
}

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function writeDeliveryState(root) {
  const deliveryId = "delivery-aac-coverage";
  const phaseId = "phase-1";
  writeJson(projectFile(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "planning",
      requestSummary: "Verify AAC coverage protocol.",
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
    requestSummary: "Verify AAC coverage protocol.",
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
      phaseGoal: "Verify AAC coverage protocol.",
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

function writeSections(root, request, coverageType) {
  const bySection = Object.fromEntries(request.outputContract.sectionOutputs.map((output) => [output.section, output.candidateFile]));
  writeJson(projectFile(root, bySection.foundation), sectionCandidate(request, "foundation", {
    source: {
      planningGenerationContractId: "pgc-001",
      technicalBaselineId: "tb-001",
      brainstormContractId: "bc-001",
      roadmapId: null,
      phaseId: "phase-1",
    },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-main", type: "library", root: "." }],
      modules: [{ moduleId: "module-core", appId: "app-main", paths: ["src"], responsibility: "Core.", layerMappings: [] }],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [{ moduleId: "module-core", name: "Core", responsibility: "Core.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"] }],
  }));
  writeJson(projectFile(root, bySection.domain_contract), sectionCandidate(request, "domain_contract", {
    dataModel: { entities: [], relationships: [], constraints: [] },
    interfaces: [],
  }));
  writeJson(projectFile(root, bySection.behavior), sectionCandidate(request, "behavior", {
    userFlows: [{
      flowId: "flow-open",
      name: "Open core flow",
      kind: "user_interaction",
      moduleRefs: ["module-core"],
      interfaceRefs: [],
      entityRefs: [],
      scopeRefs: ["scope-001"],
      acceptanceRefs: ["AC-001"],
      entry: { type: "manual", ref: null, label: "Open core flow" },
      steps: [{
        stepId: "step-open",
        action: "Open the current phase core behavior.",
        interfaceRefs: [],
      }],
      outcomes: [{ type: "success", description: "The core flow is available." }],
    }],
    stateMachines: [{
      stateMachineId: "sm-order",
      name: "Order",
      entityRef: null,
      entityRefs: [],
      moduleRefs: ["module-core"],
      scopeRefs: ["scope-001"],
      acceptanceRefs: ["AC-001"],
      states: [{ stateId: "open", name: "Open", terminal: false }],
      initialState: "open",
      events: [{ eventId: "submit", name: "Submit" }],
      transitions: [],
      rules: [{ ruleId: "rule-halted-no-order", description: "No order while halted.", acceptanceRefs: ["AC-001"] }],
    }],
  }));
  writeJson(projectFile(root, bySection.frontend_experience), sectionCandidate(request, "frontend_experience", {
    frontendExperience: {
      required: false,
      kind: "none",
      experienceLevel: "none",
      surfaces: [],
      navigation: { required: false, pattern: "none", items: [] },
      interactionStates: [],
      mustNot: [],
      notes: ["No frontend in this fixture."],
    },
  }));
  writeJson(projectFile(root, bySection.runtime_delivery), sectionCandidate(request, "runtime_delivery", {
    runtimeDelivery: {
      status: "not_applicable",
      contractVersion: "phase-1-v1",
      runtimeKind: "api_only",
      basis: {
        technicalBaselineRef: "contracts/technical-baseline.json",
        repositoryContextRef: null,
        planningGenerationContractRef: "contracts/planning/phase-1/pgc.json",
        previousRuntimeDeliveryRef: null,
        reason: "AAC coverage fixture does not exercise a runnable app.",
      },
    },
  }));
  writeJson(projectFile(root, bySection.coverage), sectionCandidate(request, "coverage", {
    acceptanceMatrix: [{
      acceptanceId: "AC-001",
      priority: "must",
      statement: "Core status rule is covered.",
      coverageStatus: "covered",
      coverage: [{ type: coverageType, refs: ["rule-halted-no-order"], description: "Status rule covers AC-001." }],
      verificationHints: [{ kind: "static", description: "Inspect AAC coverage." }],
    }],
    risksAndDecisions: {
      decisions: [{
        decisionId: "decision-optional-empty-values",
        type: "architecture",
        title: "Use existing core module",
        decision: "Extend the existing core module.",
        rationale: "The current phase only needs a small rule.",
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        status: "accepted",
        decisionQuestion: null,
        options: null,
        allowFreeform: null,
        impact: null,
      }],
      risks: [{
        riskId: "risk-optional-empty-values",
        type: "implementation",
        title: "Small fixture risk",
        description: "Fixture risk with empty optional mitigation.",
        severity: "low",
        mitigation: "",
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        status: "accepted",
      }],
      assumptions: [],
      deferredNotes: [],
    },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
  }));
}

async function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-aac-coverage-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module" }, null, 2));
    run(["init"], root);
    const { deliveryId, phaseId } = writeDeliveryState(root);
    const arch = run(["architecture", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const archRequestBody = requestFromCommand(arch, root);
    assert.equal(arch.request, undefined);
    const sectionOutputs = archRequestBody.outputContract.sectionOutputs;
    const coverageShape = sectionOutputs.find((output) => output.section === "coverage").schemaShape;
    assert.ok(JSON.stringify(coverageShape).includes("coverage[].type must match the real artifact kind"));
    assert.ok(sectionOutputs.some((output) => output.section === "frontend_experience"));
    assert.ok(sectionOutputs.some((output) => output.section === "runtime_delivery"));
    const runtimeOutput = sectionOutputs.find((output) => output.section === "runtime_delivery");
    assert.ok(runtimeOutput.enumRefs?.verificationBoundary?.includes("code_level_only"));
    assert.ok(runtimeOutput.generationRules?.some((rule) => rule.includes("technicalBaselineRef")));
    assert.ok(runtimeOutput.generationRules?.some((rule) => rule.includes("RepositoryContext")));
    assert.ok(runtimeOutput.schemaShape?.content?.runtimeDelivery?.basis?.technicalBaselineRef.includes("sourceRefs.technicalBaselineRef"));
    assert.ok(runtimeOutput.schemaShape?.content?.runtimeDelivery?.taskPlanningGuidance);
    assert.ok(runtimeOutput.schemaShape?.content?.runtimeDelivery?.deliveryMechanics?.codegen?.codeLevelExpectations);

    writeSections(root, archRequestBody, "state_rule");
    const domainOutput = sectionOutputs.find((output) => output.section === "domain_contract");
    const invalidDomainSection = readJson(projectFile(root, domainOutput.candidateFile));
    invalidDomainSection.blockedReasons = [{
      code: "PGC_INSUFFICIENT",
      message: "Use only when status=blocked.",
      nextNode: "planning_contract_create | technical_baseline_request | needs_user_decision | blocked",
    }];
    writeJson(projectFile(root, domainOutput.candidateFile), invalidDomainSection);
    const sectionSchemaRejected = run(["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", archRequestBody.requestId], root);
    assert.equal(sectionSchemaRejected.accepted, false);
    assert.equal(sectionSchemaRejected.status, "needs_candidate_repair");
    assert.equal(
      sectionSchemaRejected.issues.some((issue) =>
        issue.code === "SCHEMA_INVALID" &&
        issue.path === "/sections/domain_contract/blockedReasons/0/nextNode" &&
        issue.message.includes("Allowed values"),
      ),
      true,
      JSON.stringify(sectionSchemaRejected.issues, null, 2),
    );
    assert.equal(sectionSchemaRejected.repairInstruction.targetSection, "domain_contract");
    assert.equal(sectionSchemaRejected.repairInstruction.targetCandidateFile.endsWith("/sections/domain_contract.json"), true);

    writeSections(root, archRequestBody, "state_rule");
    const invalidRuntimeSection = readJson(projectFile(root, runtimeOutput.candidateFile));
    invalidRuntimeSection.content.runtimeDelivery.status = "modified | unchanged | not_applicable";
    writeJson(projectFile(root, runtimeOutput.candidateFile), invalidRuntimeSection);
    const runtimeSchemaRejected = run(["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", archRequestBody.requestId], root);
    assert.equal(runtimeSchemaRejected.accepted, false);
    assert.equal(runtimeSchemaRejected.status, "needs_candidate_repair");
    const runtimeStatusIssue = runtimeSchemaRejected.issues.find((issue) =>
      issue.code === "SCHEMA_INVALID" && issue.path === "/runtimeDelivery/status"
    );
    assert.ok(runtimeStatusIssue, JSON.stringify(runtimeSchemaRejected.issues, null, 2));
    assert.ok(runtimeStatusIssue.message.includes("Allowed values: modified, unchanged, not_applicable"), runtimeStatusIssue.message);
    assert.ok(runtimeStatusIssue.message.includes("Received: modified | unchanged | not_applicable"), runtimeStatusIssue.message);
    assert.deepEqual(runtimeStatusIssue.schemaError.allowedValues, ["modified", "unchanged", "not_applicable"]);
    assert.equal(runtimeStatusIssue.schemaError.received, "modified | unchanged | not_applicable");
    assert.ok(runtimeStatusIssue.repairHint.includes("Use exactly one allowed value"), runtimeStatusIssue.repairHint);
    assert.ok(runtimeStatusIssue.repairHint.includes("do not choose arbitrarily just to satisfy schema"), runtimeStatusIssue.repairHint);
    assert.ok(runtimeStatusIssue.repairHint.includes("sourceRefs.previousRuntimeDeliveryRef exists"), runtimeStatusIssue.repairHint);
    assert.ok(runtimeStatusIssue.repairHint.includes("Do not copy pipe-joined examples"), runtimeStatusIssue.repairHint);
    assert.equal(runtimeSchemaRejected.repairInstruction.targetSection, "runtime_delivery");
    const compactRuntimeIssue = runtimeSchemaRejected.repairRequest.issues.find((issue) =>
      issue.code === "SCHEMA_INVALID" && issue.path === "/runtimeDelivery/status"
    );
    assert.deepEqual(compactRuntimeIssue.schemaError.allowedValues, ["modified", "unchanged", "not_applicable"]);

    writeSections(root, archRequestBody, "data_constraint");
    const rejected = run(["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", archRequestBody.requestId], root);
    assert.equal(rejected.accepted, false);
    assert.equal(
      rejected.issues.some((issue) => issue.code === "AAC_COVERAGE_TYPE_MISMATCH"),
      true,
      JSON.stringify(rejected.issues, null, 2),
    );
    assert.equal(rejected.issues.some((issue) => issue.code === "SCHEMA_INVALID" && issue.path.includes("decisionQuestion")), false);
    assert.equal(rejected.repairInstruction.chatOutputPolicy?.writeArtifactToFileOnly, true);
    assert.equal(rejected.repairInstruction.chatOutputPolicy?.doNotPasteArtifactJson, true);
    assert.equal(rejected.repairInstruction.chatOutputPolicy?.doNotPasteDiff, true);
    assert.equal(rejected.repairInstruction.targetSection, "coverage");
    assert.equal(rejected.repairInstruction.targetCandidateFile.endsWith("/sections/coverage.json"), true);
    assert.equal(rejected.repairInstruction.sectionOutputs, undefined);
    assert.equal(rejected.repairRequest.targetSection, "coverage");
    assert.equal(rejected.repairRequest.sectionOutputs, undefined);

    writeSections(root, archRequestBody, "state_rule");
    const accepted = run(["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", archRequestBody.requestId], root);
    assert.equal(accepted.accepted, true);
    const acceptedAac = readJson(projectFile(root, accepted.contractPath));
    assert.deepEqual(acceptedAac.userFlows[0].steps[0].stateMachineRefs, []);
    assert.equal(acceptedAac.runtimeDelivery.basis.repositoryContextRef ?? null, null);

    const contracts = await import(path.join(repoRoot, "dist", "core", "operations", "contracts.js"));
    assert.deepEqual(
      contracts.inferArchitectureRepairSections([{ code: "SOURCE_NOT_READY", path: "/runtimeDelivery/basis/previousRuntimeDeliveryRef" }]),
      ["runtime_delivery"],
    );

    const indexPath = projectFile(root, `.loom/deliveries/${deliveryId}/index.json`);
    const statusPath = projectFile(root, ".loom/status.json");
    const index = readJson(indexPath);
    index.activePhaseId = "phase-2";
    index.phases[0].status = "completed";
    index.phases.push({
      phaseId: "phase-2",
      name: "Phase 2",
      status: "planning",
      latestRefs: {},
      nextAction: null,
    });
    writeJson(indexPath, index);
    const status = readJson(statusPath);
    status.deliveries[0].activePhaseId = "phase-2";
    writeJson(statusPath, status);
    const pgc2 = readJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`));
    pgc2.planningContractId = "pgc-002";
    pgc2.source.phaseId = "phase-2";
    pgc2.phaseScope.phaseName = "Phase 2";
    pgc2.phaseScope.phaseGoal = "Verify previous runtime ref.";
    writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/phase-2/pgc.json`), pgc2);
    writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/planning/phase-2/latest.json`), {
      schemaVersion: "1.0",
      planningContractId: "pgc-002",
      contractRef: `.loom/deliveries/${deliveryId}/contracts/planning/phase-2/pgc.json`,
      updatedAt: now(),
    });
    const arch2 = run(["architecture", "request", "--delivery-id", deliveryId, "--phase-id", "phase-2"], root);
    const arch2RequestBody = requestFromCommand(arch2, root);
    assert.equal(arch2RequestBody.sourceRefs.previousRuntimeDeliveryRef, `${accepted.contractPath}#/runtimeDelivery`);
    assert.ok(
      arch2RequestBody.referencedArtifactReadGuide.some((entry) => entry.refKey === "previousRuntimeDeliveryRef"),
      "phase-2 architecture request must explain previousRuntimeDeliveryRef",
    );

    writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/operations/active-lease.json`), {
      schemaVersion: "1.0",
      operationId: "op-task-execution",
      deliveryId,
      phaseId: "phase-2",
      operationType: "task_execution",
      status: "active",
      startedAt: now(),
      heartbeatAt: now(),
      expiresAt: "2026-05-24T01:00:00.000Z",
      refs: {
        requestRef: `.loom/deliveries/${deliveryId}/tasks/phase-2/execution-requests/exec-task-001.json`,
        resultFile: `.loom/deliveries/${deliveryId}/tmp/phase-2/task-results/exec-task-001/result.json`,
        taskId: "task-001",
        taskPlanRunId: "run-001",
      },
    });
    const stale = run(["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", "phase-2", "--request-id", arch2RequestBody.requestId], root);
    assert.equal(stale.accepted, false);
    assert.equal(stale.issues[0].code, "STALE_INSTRUCTION");
    assert.equal(stale.instruction.mode, "run_cli");
    assert.equal(stale.instruction.command.name, "continue");

    console.log("AAC coverage and stale architecture verification passed");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
