#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
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

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function projectFile(root, relativePath) {
  return path.join(root, relativePath);
}

function requestFromCommand(data, root) {
  return data.request ?? hydrateRequest(root, readJson(projectFile(root, data.requestPath ?? data.requestRef)));
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

function createPhase1Candidate(request) {
  return {
    schemaVersion: "1.0",
    candidateId: "bcand-phase1",
    brainstormRunId: request.brainstormRunId,
    deliveryId: request.deliveryId,
    phaseId: "phase-1",
    status: "confirmed",
    requestSummary: {
      title: "Stock Trading System",
      oneLine: "Build a phased stock trading system.",
      businessGoal: "Deliver a trading system by roadmap phases.",
      complexity: "large",
    },
    sources: [{ sourceId: "src-001", type: "text", title: "test request" }],
    scope: {
      included: [{ id: "scope-phase-1", label: "Phase 1 core", items: ["core loop"], source: "user_confirmed" }],
      excluded: [{ id: "scope-no-deploy", label: "No deploy", items: ["deployment"], source: "user_confirmed" }],
      deferred: [{ id: "scope-phase-2", label: "Phase 2 expansion", items: ["market info", "security"], source: "user_confirmed" }],
      assumptions: [],
    },
    roadmap: {
      required: true,
      currentPhaseId: "phase-1",
      phases: [
        {
          phaseId: "phase-1",
          title: "Core loop",
          status: "scope_confirmed",
          goal: "Deliver core loop.",
          scopeRefs: ["scope-phase-1"],
          acceptanceRefs: ["AC-001"],
          dependsOn: [],
        },
        {
          phaseId: "phase-2",
          title: "Capability expansion",
          status: "proposed",
          goal: "Expand capabilities.",
          scopeRefs: ["scope-phase-2"],
          acceptanceRefs: [],
          dependsOn: ["phase-1"],
        },
      ],
    },
    phasePlan: {
      current: {
        phaseId: "phase-1",
        title: "Core loop",
        goal: "Deliver core loop.",
        scopeRefs: ["scope-phase-1"],
        acceptanceRefs: ["AC-001"],
        status: "scope_confirmed",
      },
      nextPhasePreview: {
        kind: "candidate",
        suggestedPhaseId: "phase-2",
        title: "Capability expansion",
        goal: "Expand capabilities.",
        scopePreview: ["market info", "security"],
        reason: "Phase 2 remains after the core loop.",
      },
    },
    domainModel: { actors: [], capabilityGroups: [], businessFlows: [] },
    acceptance: [{ id: "AC-001", statement: "Core loop works.", capabilityRefs: [], sourceRefs: ["src-001"], priority: "must" }],
    userConfirmation: {
      confirmed: true,
      confirmedAt: "2026-05-24T00:00:00.000Z",
      confirmationSummary: "Confirmed.",
    },
    handoff: { ready: true, nextNode: "technical_baseline_generation", blockingReasons: [] },
  };
}

function forcePhase2State(root, deliveryId) {
  const indexPath = projectFile(root, `.loom/deliveries/${deliveryId}/index.json`);
  const index = readJson(indexPath);
  index.activePhaseId = "phase-2";
  if (!index.phases.some((phase) => phase.phaseId === "phase-2")) {
    index.phases.push({
      phaseId: "phase-2",
      name: "Capability expansion",
      status: "scope_confirmed",
      latestRefs: {
        brainstormContract: `.loom/deliveries/${deliveryId}/brainstorms/contract.json`,
      },
      nextAction: null,
    });
  }
  for (const phase of index.phases) {
    if (phase.phaseId === "phase-1") phase.status = "completed";
    if (phase.phaseId === "phase-2") phase.status = "scope_confirmed";
  }
  writeJson(indexPath, index);

  const contractPath = projectFile(root, `.loom/deliveries/${deliveryId}/brainstorms/contract.json`);
  const contract = readJson(contractPath);
  contract.roadmap.currentPhaseId = "phase-2";
  contract.roadmap.recommendedPhaseId = "phase-2";
  for (const phase of contract.roadmap.phases) {
    if (phase.phaseId === "phase-1") {
      phase.status = "delivered";
      phase.handoff.readyForPlanning = false;
    }
    if (phase.phaseId === "phase-2") {
      phase.status = "scope_confirmed";
      phase.scope = {
        includedRefs: ["scope-phase-2"],
        excludedRefs: ["scope-no-deploy"],
        deferredRefs: [],
      };
      phase.acceptanceRefs = ["AC-201"];
      phase.handoff.readyForPlanning = true;
      phase.handoff.planningContractId = "pgc-phase-2";
    }
  }
  contract.scope.included = [{ id: "scope-phase-2", label: "Phase 2 expansion", items: ["market info", "security"], source: "user_confirmed" }];
  contract.scope.excluded = [{ id: "scope-no-deploy", label: "No deploy", items: ["deployment"], source: "user_confirmed" }];
  contract.scope.deferred = [];
  contract.phasePlan = {
    current: {
      phaseId: "phase-2",
      title: "Capability expansion",
      goal: "Expand capabilities.",
      scopeRefs: ["scope-phase-2"],
      acceptanceRefs: ["AC-201"],
      status: "scope_confirmed",
    },
    nextPhasePreview: {
      kind: "none",
      reason: "Phase 2 fixture has no further phase.",
    },
  };
  contract.acceptance.candidates.push({
    id: "AC-201",
    statement: "Phase 2 expands capabilities.",
    capabilityRefs: [],
    sourceRefs: ["src-001"],
    priority: "must",
  });
  writeJson(contractPath, contract);
}

function writeTechnicalBaseline(root, deliveryId) {
  writeJson(projectFile(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
    schemaVersion: "1.0",
    technicalBaselineId: "tb-001",
    status: "auto_accepted",
    source: "detected_from_repo",
    projectKind: "existing_project",
    scope: "project",
    stack: {
      languages: ["typescript"],
      frameworks: [],
      packageManagers: ["npm"],
      runtime: "node",
      testFrameworks: ["vitest"],
      buildTools: [],
    },
    constraints: [],
    evidence: [{ path: "package.json", reason: "package metadata" }],
    approval: { type: "policy_auto_accept", reason: "test fixture" },
    confidence: "high",
    createdAt: "2026-05-24T00:00:00.000Z",
    updatedAt: "2026-05-24T00:00:00.000Z",
  });
}

function createRepositoryContextCandidate(requestData, root) {
  const request = requestFromCommand(requestData, root);
  return {
    schemaVersion: "1.0",
    repositoryContextId: "repoctx-001",
    deliveryId: request.deliveryId,
    phaseId: request.phaseId,
    status: "ready",
    source: {
      requestRef: requestData.requestRef,
      brainstormContractRef: request.source.brainstormContractRef,
      technicalBaselineRef: request.source.technicalBaselineRef,
    },
    requestLens: {
      projectKind: "existing_project",
      scanPurpose: "phase_start_repository_snapshot",
      primaryConsumer: "phase_brainstorm",
      laterConsumers: ["PGC", "AAC", "TaskPlan"],
    },
    repoOverview: {
      summary: "Existing TypeScript project.",
      repositoryShape: "single_package",
      primaryApplications: [{ applicationId: "app-main", name: "Main", kind: "library", rootPath: "." }],
    },
    technologySignals: { primaryLanguages: ["typescript"], frameworks: [], packageManagers: ["npm"], buildCommands: [], testCommands: [], notes: [] },
    structureSignals: {
      rootPaths: [{ path: "src", role: "source_root" }],
      entryPoints: [{ path: "src/index.ts", kind: "module" }],
      configurationFiles: ["package.json"],
    },
    existingCapabilities: [{ capabilityId: "cap-core", name: "Core", status: "implemented", summary: "Core exists.", surfaceRefs: ["surface-index"], confidence: "medium" }],
    relevantSurfaces: [{ surfaceId: "surface-index", kind: "module", path: "src/index.ts", summary: "Index.", relevance: "implemented_capability", suggestedUse: "inspect_or_extend" }],
    recommendedReadRefs: [{ path: "src/index.ts", reason: "implemented_capability", priority: "high", surfaceRefs: ["surface-index"] }],
    roadmapImplications: [],
    contextQuality: { coverage: "focused", confidence: "medium", warnings: [] },
    warnings: [],
    createdAt: "2026-05-24T00:00:00.000Z",
    updatedAt: "2026-05-24T00:00:00.000Z",
  };
}

function writeArchitectureSections(root, archRequest, planningContract) {
  const base = {
    schemaVersion: "1.0",
    requestId: archRequest.requestId,
    deliveryId: archRequest.deliveryId,
    phaseId: archRequest.phaseId,
    status: "ready",
    createdAt: "2026-05-24T00:00:00.000Z",
  };
  const bySection = {
    foundation: {
      source: {
        planningGenerationContractId: planningContract.planningContractId,
        technicalBaselineId: planningContract.source.technicalBaselineId,
        brainstormContractId: planningContract.source.brainstormContractId,
        roadmapId: planningContract.source.roadmapId,
        phaseId: planningContract.source.phaseId,
      },
      engineeringBoundary: {
        projectKind: "existing_project",
        strategy: "extend_existing_modules",
        applications: [{ appId: "app-main", type: "library", root: "." }],
        modules: [{ moduleId: "module-capabilities", appId: "app-main", paths: ["src"], responsibility: "Phase 2 capability expansion." }],
        creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
      },
      modules: [{ moduleId: "module-capabilities", name: "Capabilities", responsibility: "Expand phase 2 capabilities.", dependsOn: [], scopeRefs: ["scope-phase-2"], acceptanceRefs: ["AC-201"] }],
    },
    domain_contract: {
      dataModel: {
        entities: [{
          entityId: "capability",
          name: "Capability",
          type: "internal",
          implementationIntent: "full",
          moduleRefs: ["module-capabilities"],
          scopeRefs: ["scope-phase-2"],
          acceptanceRefs: ["AC-201"],
          fields: [{ fieldId: "id", name: "id", type: "string", required: true }],
          constraints: [],
        }],
        relationships: [],
        constraints: [],
      },
      interfaces: [{
        interfaceId: "capability-service",
        name: "CapabilityService",
        type: "service_method",
        moduleRefs: ["module-capabilities"],
        entityRefs: ["capability"],
        scopeRefs: ["scope-phase-2"],
        acceptanceRefs: ["AC-201"],
        requestSchema: [],
        responseSchema: [],
        errorSchema: [],
      }],
    },
    behavior: {
      userFlows: [{
        flowId: "expand-capability-flow",
        name: "Expand capability",
        kind: "service_flow",
        moduleRefs: ["module-capabilities"],
        interfaceRefs: ["capability-service"],
        entityRefs: ["capability"],
        scopeRefs: ["scope-phase-2"],
        acceptanceRefs: ["AC-201"],
        entry: { type: "manual", ref: "capability-service" },
        steps: [{ stepId: "step-1", action: "Expand capability.", interfaceRefs: ["capability-service"], stateMachineRefs: [] }],
        outcomes: [{ type: "success", description: "Capability expanded." }],
      }],
      stateMachines: [],
    },
    frontend_experience: {
      frontendExperience: {
        required: false,
        kind: "none",
        experienceLevel: "none",
        surfaces: [],
        navigation: { required: false, pattern: "none", items: [] },
        interactionStates: [],
        mustNot: [],
        notes: ["No frontend in this phase 2 repository context fixture."],
      },
    },
    runtime_delivery: {
      runtimeDelivery: {
        status: "not_applicable",
        contractVersion: "phase-2-v1",
        runtimeKind: "api_only",
        basis: {
          previousRuntimeDeliveryRef: null,
          reason: "Repository context fixture does not exercise deployment.",
        },
      },
    },
    coverage: {
      acceptanceMatrix: [{
        acceptanceId: "AC-201",
        priority: "must",
        statement: "Phase 2 expands capabilities.",
        coverageStatus: "covered",
        coverage: [{ type: "module", refs: ["module-capabilities"], description: "Module covers phase 2 expansion." }],
        verificationHints: [{ kind: "unit", description: "Verify phase 2 capability behavior." }],
      }],
      detailCoverage: (planningContract.requirementDetails?.items ?? [])
        .filter((item) => item.requiredForCurrentPhase)
        .map((item) => ({
          detailId: item.detailId,
          coverageStatus: "covered",
          artifactRefs: {
            modules: ["module-capabilities"],
            entities: ["capability"],
            fields: ["id"],
            constraints: [],
            interfaces: ["capability-service"],
            userFlows: ["expand-capability-flow"],
            stateMachines: [],
            frontendDataViews: [],
            frontendActions: [],
            frontendOperationPaths: [],
            acceptanceMatrix: ["AC-201"],
          },
        })),
      risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
      handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    },
  };

  for (const output of archRequest.outputContract.sectionOutputs) {
    writeJson(projectFile(root, output.candidateFile), {
      ...base,
      section: output.section,
      content: bySection[output.section],
      blockedReasons: [],
    });
  }
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });

  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-phase2-repoctx-"));
  try {
    fs.writeFileSync(projectFile(root, "package.json"), JSON.stringify({ type: "module", scripts: { test: "vitest" } }, null, 2));
    fs.mkdirSync(projectFile(root, "src"), { recursive: true });
    fs.writeFileSync(projectFile(root, "src/index.ts"), "export const ok = true;\n");

    run(["init"], root);
    const started = run(["brainstorm", "start", "--request", "Build a phased stock trading system."], root);
    const startedRequest = requestFromCommand(started, root);
    writeJson(projectFile(root, startedRequest.outputContract.candidateFile), createPhase1Candidate(startedRequest));
    run([
      "brainstorm", "accept",
      "--delivery-id", startedRequest.deliveryId,
      "--phase-id", "phase-1",
      "--run-id", startedRequest.brainstormRunId,
      "--request-id", startedRequest.requestId,
      "--candidate-file", startedRequest.outputContract.candidateFile,
    ], root);

    const deliveryId = startedRequest.deliveryId;
    forcePhase2State(root, deliveryId);
    writeTechnicalBaseline(root, deliveryId);

    const repoRequest = run(["repository-context", "request", "--delivery-id", deliveryId, "--phase-id", "phase-2"], root);
    const repoRequestBody = requestFromCommand(repoRequest, root);
    assert.equal(repoRequest.request, undefined);
    assert.equal(Object.hasOwn(repoRequestBody, "priorPhaseContext"), false);
    assert.equal(Object.hasOwn(repoRequestBody, "currentPhaseLens"), false);
    assert.equal(repoRequestBody.purpose, "generate_phase_start_repository_snapshot");
    assert.equal(repoRequestBody.scanPurpose.type, "phase_start_repository_snapshot");
    assert.deepEqual(repoRequestBody.scanPurpose.completedPhases.map((phase) => phase.phaseId), ["phase-1"]);

    writeJson(projectFile(root, repoRequest.candidateFile), createRepositoryContextCandidate(repoRequest, root));
    run([
      "repository-context", "accept",
      "--delivery-id", deliveryId,
      "--phase-id", "phase-2",
      "--request-id", repoRequest.requestId,
      "--candidate-file", repoRequest.candidateFile,
    ], root);

    const baselineAccepted = run([
      "technical-baseline", "accept",
      "--delivery-id", deliveryId,
      "--phase-id", "phase-2",
      "--candidate-file", `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`,
    ], root);
    assert.equal(baselineAccepted.nextAction.type, "planning_contract_create");
    assert.equal(baselineAccepted.nextAction.reason, "TECHNICAL_BASELINE_READY_WITH_REPOSITORY_CONTEXT");
    assert.equal(baselineAccepted.instruction.command.name, "planning_contract_create");

    const pgc = run(["planning-contract", "create", "--delivery-id", deliveryId, "--phase-id", "phase-2"], root);
    assert.deepEqual(pgc.contract.phaseScope.included.map((item) => item.scopeId), ["scope-phase-2"]);
    assert.deepEqual(pgc.contract.phaseScope.excluded.map((item) => item.scopeId), ["scope-no-deploy"]);
    assert.deepEqual(pgc.contract.phaseScope.deferred.map((item) => item.scopeId), []);
    assert.ok(pgc.contract.contextRefs.repositoryContextRef.endsWith("/workspace/phase-2/repository-context.json"));

    const arch = run(["architecture", "request", "--delivery-id", deliveryId, "--phase-id", "phase-2"], root);
    const archRequestBody = requestFromCommand(arch, root);
    assert.equal(arch.request, undefined);
    assert.equal(archRequestBody.sourceRefs.repositoryContextRef, pgc.contract.contextRefs.repositoryContextRef);
    writeArchitectureSections(root, archRequestBody, pgc.contract);
    const archAccepted = run(["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", "phase-2", "--request-id", archRequestBody.requestId], root);
    assert.equal(archAccepted.accepted, true, JSON.stringify(archAccepted.issues ?? [], null, 2));

    const taskPlanGate = run(["continue"], root);
    assert.equal(taskPlanGate.nextAction.type, "taskplan_generation");
    const taskPlan = run(["task-plan", "request", "--delivery-id", deliveryId, "--phase-id", "phase-2"], root);
    const taskPlanRequestBody = requestFromCommand(taskPlan, root);
    assert.equal(taskPlan.request, undefined);
    assert.equal(taskPlanRequestBody.sourceRefs.repositoryContextRef, pgc.contract.contextRefs.repositoryContextRef);
    assert.equal(taskPlanRequestBody.sourceRefs.phaseConceptGroundingRef, pgc.contract.contextRefs.phaseConceptGroundingRef);
    assert.equal(taskPlanRequestBody.conceptGroundingSource.phaseConceptGroundingRef, pgc.contract.contextRefs.phaseConceptGroundingRef ?? null);
    assert.ok(taskPlanRequestBody.conceptGroundingSource.missingRefPolicy.includes("omit task conceptRefs"));
    assert.ok(taskPlanRequestBody.fieldAccessHints.phaseConceptGroundingRef.includes("sourceRefs.phaseConceptGroundingRef"));

    console.log("phase2 repository-context flow verification passed");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

main();
