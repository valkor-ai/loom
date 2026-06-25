#!/usr/bin/env node

const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "../../../../src/ts/reference");
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

function runEnvelope(args, projectRoot) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  return JSON.parse(output);
}

function runEnvelopeWithEnv(args, projectRoot, env) {
  const output = execFileSync(process.execPath, [cli, ...args, "--project-root", projectRoot, "--json"], {
    cwd: repoRoot,
    env: { ...process.env, LOOM_AGENT_PROFILE: "codex", ...env },
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return JSON.parse(output);
}

function assertNoDeepKey(value, forbiddenKey, pathName = "$") {
  if (!value || typeof value !== "object") {
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertNoDeepKey(item, forbiddenKey, `${pathName}[${index}]`));
    return;
  }
  for (const [key, child] of Object.entries(value)) {
    assert.notEqual(key, forbiddenKey, `compact output must not include ${forbiddenKey} at ${pathName}.${key}`);
    assertNoDeepKey(child, forbiddenKey, `${pathName}.${key}`);
  }
}

function assertCompactRequestEnvelope(envelope) {
  assert.equal(envelope.ok, true);
  assert.equal(envelope.compact, true);
  assert.equal(Object.prototype.hasOwnProperty.call(envelope.data ?? {}, "request"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(envelope.data ?? {}, "requestSummary"), false);
  for (const forbiddenKey of ["schemaShape", "reviewRules", "allowedRefs", "enumRefs", "generationRules", "executionRules"]) {
    assertNoDeepKey(envelope, forbiddenKey);
  }
}

function file(root, relativePath) {
  return path.join(root, relativePath);
}

function writeJson(target, value) {
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, `${JSON.stringify(value, null, 2)}\n`);
}

function readJson(target) {
  return JSON.parse(fs.readFileSync(target, "utf8"));
}

function hydrateRequest(root, request) {
  const hydrated = { ...request };
  for (const [key, value] of Object.entries(request)) {
    if (!key.endsWith("Ref") || typeof value !== "string" || key === "requestRef") continue;
    const targetKey = key.slice(0, -"Ref".length);
    if (targetKey in hydrated) continue;
    hydrated[targetKey] = readJson(file(root, value));
  }
  return hydrated;
}

function latestFile(root, globParts) {
  const dir = file(root, globParts.dir);
  const entries = fs.readdirSync(dir).filter((entry) => globParts.pattern.test(entry)).sort();
  assert.ok(entries.length > 0, `No file matched ${globParts.pattern} in ${globParts.dir}`);
  return path.join(dir, entries.at(-1));
}

function now() {
  return "2026-05-24T00:00:00.000Z";
}

function seedDelivery(root) {
  const deliveryId = "delivery-fe-rd";
  const phaseId = "phase-1";
  const confirmedFrontendExperienceRef = `.loom/deliveries/${deliveryId}/frontend-experience/${phaseId}/confirmed-target.json`;
  const currentFrontendExperienceRef = `.loom/deliveries/${deliveryId}/frontend-experience/current.json`;
  writeJson(file(root, ".loom/status.json"), {
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "planning",
      requestSummary: "Verify frontend/runtime/severity protocol.",
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
  writeJson(file(root, `.loom/deliveries/${deliveryId}/index.json`), {
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Verify frontend/runtime/severity protocol.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{ phaseId, name: "Phase 1", status: "planning", latestRefs: {}, nextAction: null }],
    createdAt: now(),
    updatedAt: now(),
  });
  writeJson(file(root, `.loom/deliveries/${deliveryId}/contracts/technical-baseline.json`), {
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
  const frontendTarget = {
    schemaVersion: "1.0",
    deliveryId,
    phaseId,
    updatedAt: now(),
    frontendExperience: {
      required: true,
      kind: "business_application",
      experienceLevel: "usable_internal_product",
      audiences: [{ audienceId: "user", name: "User", primaryJobs: ["Open and operate the app."] }],
      surfaces: [{ surfaceId: "surface-home", name: "Home", audienceRefs: ["user"], primaryJobs: ["Open the app."] }],
      mustNot: ["Do not build a naked form stack."],
      confirmationSummary: "User confirmed a usable internal product UI.",
    },
    source: "brainstorm_user_confirmed",
  };
  writeJson(file(root, confirmedFrontendExperienceRef), frontendTarget);
  writeJson(file(root, currentFrontendExperienceRef), {
    ...frontendTarget,
    currentPhaseId: phaseId,
    confirmedFrontendExperienceRef,
  });
  writeJson(file(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/pgc.json`), {
    schemaVersion: "1.0",
    planningContractId: "pgc-001",
    status: "ready",
    source: { brainstormRunId: "bs-001", brainstormContractId: "bc-001", roadmapId: null, phaseId, technicalBaselineId: "tb-001" },
    contextRefs: {
      confirmedFrontendExperienceRef,
      currentFrontendExperienceRef,
    },
    phaseScope: {
      phaseName: "Phase 1",
      phaseGoal: "Verify runnable frontend app.",
      included: [{ scopeId: "scope-001", label: "Core app", items: ["core app"], source: "fixture" }],
      deferred: [],
      excluded: [],
      acceptanceCandidates: [{ id: "AC-001", statement: "The app can be built, started, and viewed.", priority: "must" }],
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
  writeJson(file(root, `.loom/deliveries/${deliveryId}/contracts/planning/${phaseId}/latest.json`), {
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

function writeArchitectureSections(root, request) {
  const sectionOutputs = request.outputContract.sectionOutputs;
  const outputs = Object.fromEntries(sectionOutputs.map((output) => [output.section, output.candidateFile]));
  writeJson(file(root, outputs.foundation), sectionCandidate(request, "foundation", {
    source: { planningGenerationContractId: "pgc-001", technicalBaselineId: "tb-001", brainstormContractId: "bc-001", roadmapId: null, phaseId: "phase-1" },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-main", type: "web_app", root: "." }],
      modules: [{ moduleId: "module-app", appId: "app-main", paths: ["src"], responsibility: "Runnable app.", layerMappings: [] }],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [{ moduleId: "module-app", name: "App", responsibility: "Runnable app.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"] }],
  }));
  writeJson(file(root, outputs.domain_contract), sectionCandidate(request, "domain_contract", {
    dataModel: { entities: [], relationships: [], constraints: [] },
    interfaces: [{ interfaceId: "iface-root", name: "root", type: "http_api", moduleRefs: ["module-app"], entityRefs: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], method: "GET", path: "/" }],
  }));
  writeJson(file(root, outputs.behavior), sectionCandidate(request, "behavior", {
    userFlows: [{ flowId: "flow-root", name: "Open app", kind: "user_interaction", moduleRefs: ["module-app"], interfaceRefs: ["iface-root"], entityRefs: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], entry: { type: "page", ref: "iface-root", label: "Home" }, steps: [{ stepId: "step-open", action: "Open root page.", interfaceRefs: ["iface-root"], stateMachineRefs: [] }], outcomes: [{ type: "success", description: "App renders." }] }],
    stateMachines: [],
  }));
  writeJson(file(root, outputs.frontend_experience), sectionCandidate(request, "frontend_experience", {
    frontendExperience: {
      required: true,
      kind: "business_application",
      experienceLevel: "usable_internal_product",
      sourceRefs: {
        brainstormFrontendExperienceRef: request.sourceRefs.confirmedFrontendExperienceRef ?? request.sourceRefs.currentFrontendExperienceRef,
        technicalBaselineRef: request.sourceRefs.technicalBaselineRef,
      },
      sourceAuthority: [
        "Brainstorm frontendExperience is the user-confirmed product target.",
      ],
      surfaces: [{ surfaceId: "surface-home", name: "Home", purpose: "Operate the app.", userRoleRefs: ["user"], workflowRefs: ["flow-root"], moduleRefs: ["module-app"] }],
      navigation: { required: false, pattern: "none", items: [] },
      interactionStates: ["idle", "loading", "success", "error"],
      mustNot: ["Do not build a naked form stack."],
      notes: ["Fixture frontend contract."],
    },
  }));
  writeJson(file(root, outputs.runtime_delivery), sectionCandidate(request, "runtime_delivery", {
    runtimeDelivery: {
      status: "modified",
      contractVersion: "phase-1-v1",
      runtimeKind: "node_express_serves_vite_static",
      basis: {
        technicalBaselineRef: "contracts/technical-baseline.json",
        repositoryContextRef: "workspace/repository-context.json",
        planningGenerationContractRef: "contracts/planning/phase-1/pgc.json",
        previousRuntimeDeliveryRef: null,
        reason: "Fixture runtime follows the accepted technical baseline.",
      },
      build: { command: "npm run build", workingDirectory: ".", outputs: ["dist/server", "dist/web"], codeLevelExpectations: ["Build produces frontend and server deliverables."] },
      start: { command: "npm run start", workingDirectory: ".", entry: "dist/server/src/interfaces/http/server.js", host: "0.0.0.0", port: 4173, portEnv: "PORT", codeLevelExpectations: ["Start serves API and frontend static assets."] },
      runtimeSurfaces: [{ surfaceId: "preview-root", kind: "http", probe: { type: "http_path", target: "/", expected: "2xx_or_3xx" } }],
      deliveryMechanics: {
        staticAssets: { required: true, source: "src/ui", output: "dist/web", servedBy: "express_static" },
        api: { required: true, entry: "dist/server/src/interfaces/http/server.js", basePath: "/api", probePaths: ["/api/accounts"] },
        codegen: { required: "no", commands: [], codeLevelExpectations: [] },
      },
      httpProbes: { previewPath: "/", healthPath: "/health", apiPaths: ["/api/accounts"], expectedStatus: "2xx_or_3xx" },
      frontend: { required: true, kind: "vite_react", buildCommand: "npm run build:web", sourceRoot: "src/ui", outputDir: "dist/web", servedBy: "express_static", servedByRef: "src/interfaces/http/server.ts", codeLevelExpectations: ["Frontend output is served by Express static middleware."] },
      api: { required: true, kind: "express", buildCommand: "npm run build:api", entry: "dist/server/src/interfaces/http/server.js", basePath: "/api", probePaths: ["/api/accounts"], codeLevelExpectations: ["API route probes are mounted under /api."] },
      environment: { required: [], optional: ["PORT"] },
      taskPlanningGuidance: {
        requireRuntimeDeliveryRequirementWhenTaskTouches: ["build_or_packaging", "runtime_entry", "serving_or_routing", "configuration_or_environment", "generated_artifacts", "runtime_surface"],
        doNotRequireForTaskKinds: ["domain_only_validation", "copy_only_documentation", "pure_unit_test_additions"],
        verificationBoundary: "code_level_only",
        doNotRequireCleanInstallOrContainerBuild: true,
      },
      deployability: { localDocker: "supported", notes: [] },
    },
  }));
  writeJson(file(root, outputs.coverage), sectionCandidate(request, "coverage", {
    acceptanceMatrix: [{ acceptanceId: "AC-001", priority: "must", statement: "The app can be built, started, and viewed.", coverageStatus: "covered", coverage: [{ type: "module", refs: ["module-app"], description: "App module covers runtime." }, { type: "interface", refs: ["iface-root"], description: "Root page covers preview." }, { type: "user_flow", refs: ["flow-root"], description: "Open app flow covers preview." }], verificationHints: [{ kind: "contract", description: "RuntimeDeliveryContract must close build/start/preview." }] }],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
  }));
}

function main() {
  execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "loom-fe-rd-"));
  try {
    fs.writeFileSync(file(root, "package.json"), JSON.stringify({ type: "module", scripts: { build: "echo build", start: "node server.js" } }, null, 2));
    run(["init"], root);
    const { deliveryId, phaseId } = seedDelivery(root);

    const arch = run(["architecture", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const request = arch.request ?? hydrateRequest(root, readJson(file(root, arch.requestPath ?? arch.requestRef)));
    const sectionOutputs = request.outputContract.sectionOutputs;
    assert.equal(Object.prototype.hasOwnProperty.call(request, "inputRefs"), false);
    assert.equal(Object.prototype.hasOwnProperty.call(request, "sectionOutputs"), false);
    assert.ok(request.sourceRefs.planningContractRef);
    assert.ok(request.sourceRefs.confirmedFrontendExperienceRef);
    assert.equal(request.frontendExperienceSource.confirmedFrontendExperienceRef, request.sourceRefs.confirmedFrontendExperienceRef);
    assert.ok(
      request.requestReadPlan.groups.flatMap((group) => group.fields ?? []).some((field) => field.includes("frontendExperienceSource")),
      "Architecture requestReadPlan must expose frontendExperienceSource fields.",
    );
    assert.ok(request.fieldAccessHints.compactJqExamples.some((example) => example.includes(".outputContract.sectionOutputs")));
    assert.equal(request.outputContract.allowedRefsUsage.exactOnly, true);
    assert.ok(request.agentAction.write.rules.some((rule) => rule.includes("allowedRefs.acceptanceRefs")));
    assert.ok(request.agentAction.write.rules.some((rule) => rule.includes("inventing AC ids")));
    assert.ok(sectionOutputs.length > 0);
    const archCompact = runEnvelopeWithEnv(["architecture", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root, { LOOM_COMPACT_OUTPUT: "1" });
    assertCompactRequestEnvelope(archCompact);
    assert.equal(archCompact.instruction.requestRef, arch.requestPath);
    assert.equal(archCompact.data.requestPath, arch.requestPath);
    const compactRequest = hydrateRequest(root, readJson(file(root, archCompact.instruction.requestRef)));
    assert.ok(compactRequest.fieldAccessHints.compactJqExamples.some((example) => example.includes(".sourceRefs")));
    assert.equal(compactRequest.outputContract.sectionOutputs[0].schemaRef, sectionOutputs[0].schemaRef);
    assert.ok(request.enumRefs.section.includes("frontend_experience"));
    assert.ok(request.enumRefs.section.includes("runtime_delivery"));
    const domainOutput = sectionOutputs.find((output) => output.section === "domain_contract");
    assert.ok(domainOutput.generationRules?.some((rule) => rule.includes("earlier phase") && rule.includes("reference_only")));
    assert.ok(sectionOutputs.some((output) => output.section === "frontend_experience"));
    const frontendOutput = sectionOutputs.find((output) => output.section === "frontend_experience");
    assert.ok(frontendOutput.generationRules.some((rule) => rule.includes("frontendExperienceSource")));
    assert.ok(frontendOutput.generationRules.some((rule) => rule.includes("brainstormFrontendExperienceRef")));
    assert.ok(sectionOutputs.some((output) => output.section === "runtime_delivery"));
    const runtimeOutput = sectionOutputs.find((output) => output.section === "runtime_delivery");
    assert.ok(runtimeOutput.enumRefs?.verificationBoundary?.includes("code_level_only"));
    assert.ok(runtimeOutput.schemaShape.content.fieldPresenceMatrix.omitWhenNotApplicableNeverNull.includes("api"));
    assert.ok(runtimeOutput.schemaShape.content.fieldPresenceMatrix.apiRules.some((rule) => rule.includes("Omit the entire object")));
    assert.ok(runtimeOutput.generationRules.some((rule) => rule.includes("fieldPresenceMatrix")));
    assert.ok(runtimeOutput.generationRules?.some((rule) => rule.includes("technicalBaselineRef")));
    assert.ok(runtimeOutput.schemaShape?.content?.runtimeDelivery?.taskPlanningGuidance);
    writeArchitectureSections(root, request);
    const accepted = run(["architecture", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", request.requestId], root);
    assert.equal(accepted.accepted, true);

    const taskPlanRequestData = run(["task-plan", "request", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const taskPlanRequest = taskPlanRequestData.request ?? hydrateRequest(root, readJson(file(root, taskPlanRequestData.requestPath ?? taskPlanRequestData.requestRef)));
    assert.equal(taskPlanRequest.generationRules.frontendExperienceRules.required, true);
    assert.equal(taskPlanRequest.generationRules.runtimeDeliveryRules.status, "modified");
    assert.equal(taskPlanRequest.outputContract.runtimeDeliveryProjection.startCommand, "npm run start");
    assert.ok(taskPlanRequest.outputContract.groupSchemaShape.tasks[0].frontendExperienceRequirement);
    assert.ok(taskPlanRequest.outputContract.groupSchemaShape.tasks[0].runtimeDeliveryRequirement);
    assert.ok(taskPlanRequest.generationRules.runtimeDeliveryRules.rules.some((rule) => /exactly one runtime_delivery_closure/.test(rule)));
    assert.ok(taskPlanRequest.generationRules.runtimeDeliveryRules.rules.some((rule) => /requiredClosureGroupShape/.test(rule)));
    assert.deepEqual(
      taskPlanRequest.generationRules.runtimeDeliveryRules.closureRequirement,
      taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement,
    );
    assert.ok(taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredContractFields.includes("httpProbes"));
    assert.ok(taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredContractFields.includes("deliveryMechanics.staticAssets"));
    assert.equal(taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredClosureGroupShape.position, "final_group");
    assert.equal(taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredClosureGroupShape.allowedExtraTasks, false);
    assert.equal(taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredClosureTaskShape.taskKind, "runtime_delivery_closure");
    assert.deepEqual(
      taskPlanRequest.outputContract.runtimeDeliveryClosureTaskTemplate.runtimeDeliveryRequirement.requiredCodeLevelChecks,
      taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredCodeLevelChecks,
    );
    assert.deepEqual(
      taskPlanRequest.generationRules.runtimeDeliveryRules.closureTaskTemplate.runtimeDeliveryRequirement,
      taskPlanRequest.outputContract.runtimeDeliveryClosureTaskTemplate.runtimeDeliveryRequirement,
    );
    assert.ok(
      taskPlanRequest.agentAction.write.rules.some((rule) => rule.includes("runtimeDeliveryClosureTaskTemplate") && rule.includes("exactly")),
    );
    assert.ok(taskPlanRequest.outputContract.outlineSchemaShape.groups.some((group) => group.shapeAuthority === "outputContract.runtimeDeliveryClosureRequirement.requiredClosureGroupShape"));

    assert.deepEqual(taskPlanRequest.outputContract.submitCommand, undefined);

    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-missing-group-file",
      groups: [{ groupId: "group-missing-file", title: "Missing group file", objective: "Assembly failure fixture.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-missing-file"] }],
      createdAt: now(),
    });
    const missingGroupFileAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(missingGroupFileAccept.ok, true);
    assert.equal(missingGroupFileAccept.data.accepted, false);
    assert.deepEqual(
      missingGroupFileAccept.data.repairRequest.outputContract.runtimeDeliveryClosureRequirement,
      taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement,
    );

    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-mismatched-group-file",
      groups: [{ groupId: "group-mismatch", title: "Mismatched group file", objective: "Assembly mismatch fixture.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-mismatch"] }],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-mismatch")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-mismatch", title: "Mismatched group file", objective: "Assembly mismatch fixture.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-other"] },
      tasks: [],
      createdAt: now(),
    });
    const mismatchedGroupAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(mismatchedGroupAccept.ok, true);
    assert.equal(mismatchedGroupAccept.data.accepted, false);
    assert.ok(mismatchedGroupAccept.data.issues.some((issue) => issue.path === "/groups/group-mismatch/group"));
    assert.deepEqual(
      mismatchedGroupAccept.data.repairRequest.outputContract.runtimeDeliveryClosureRequirement,
      taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement,
    );

    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-missing-runtime-closure",
      groups: [{ groupId: "group-runtime-missing", title: "Runtime missing closure", objective: "Missing closure fixture.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-missing"] }],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-missing")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-missing", title: "Runtime missing closure", objective: "Missing closure fixture.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-missing"] },
      tasks: [{
        taskId: "task-runtime-missing",
        groupId: "group-runtime-missing",
        title: "Runtime task without closure",
        taskKind: "runtime_delivery",
        implementationActions: ["implement_runtime_delivery_contract"],
        objective: "Fixture intentionally omits runtime_delivery_closure.",
        dependsOn: [],
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        writeBoundary: { forbiddenPaths: [".loom"], artifactRefs: { modules: ["module-app"], entities: [], interfaces: ["iface-root"], userFlows: ["flow-root"], stateMachines: [], decisions: [], risks: [] } },
        verificationIntents: [{ verificationId: "VI-runtime-missing", acceptanceRefs: ["AC-001"], behavior: "Runtime has evidence.", preferredEvidence: ["static_check"], acceptableEvidence: ["static_check"] }],
        runtimeDeliveryRequirement: {
          appliesToThisTask: true,
          reason: "Fixture runtime task is not a closure task.",
          runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
          affectedContractFields: ["build.command"],
          requiredCodeLevelChecks: [{ checkId: "rd-check-build-only", contractField: "build.command", objective: "Partial runtime check.", acceptableEvidence: ["static_check"] }],
          evidenceExpectedInTaskResult: ["Partial evidence."],
          forbiddenActions: ["do_not_require_clean_install_or_container_build_for_this_task"],
        },
      }],
      createdAt: now(),
    });
    const missingClosureAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(missingClosureAccept.ok, true);
    assert.equal(missingClosureAccept.data.accepted, false);
    assert.equal(missingClosureAccept.instruction?.mode, "repair_candidate");
    assert.equal(missingClosureAccept.actionRequired?.mode, "repair_candidate");
    assert.deepEqual(
      missingClosureAccept.data.repairRequest.outputContract.runtimeDeliveryClosureRequirement,
      taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement,
    );
    assert.ok(missingClosureAccept.data.issues.some((issue) => issue.path === "/tasks/runtimeDeliveryClosure"));

    const closureFields = taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredContractFields;
    const closureChecks = taskPlanRequest.outputContract.runtimeDeliveryClosureRequirement.requiredCodeLevelChecks;
    const runtimeTask = {
      taskId: "task-runtime-app",
      groupId: "group-runtime-app",
      title: "Implement runtime app wiring",
      taskKind: "runtime_delivery",
      implementationActions: ["implement_runtime_delivery_contract"],
      objective: "Fixture runtime-affecting task.",
      dependsOn: [],
      scopeRefs: ["scope-001"],
      acceptanceRefs: ["AC-001"],
      writeBoundary: { forbiddenPaths: [".loom"], artifactRefs: { modules: ["module-app"], entities: [], interfaces: ["iface-root"], userFlows: ["flow-root"], stateMachines: [], decisions: [], risks: [] } },
      verificationIntents: [{ verificationId: "VI-runtime-app", acceptanceRefs: ["AC-001"], behavior: "Runtime app wiring exists.", preferredEvidence: ["static_check"], acceptableEvidence: ["static_check"] }],
      runtimeDeliveryRequirement: {
        appliesToThisTask: true,
        reason: "Fixture task affects runtime delivery.",
        runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
        affectedContractFields: ["build.command"],
        requiredCodeLevelChecks: [{ checkId: "rd-check-runtime-app-build", contractField: "build.command", objective: "Confirm build command is represented.", acceptableEvidence: ["static_check"] }],
        evidenceExpectedInTaskResult: ["Runtime app build wiring is checked."],
        forbiddenActions: ["do_not_require_clean_install_or_container_build_for_this_task"],
      },
    };
    const closureTask = {
      taskId: "task-runtime-closure",
      groupId: "group-runtime-closure",
      title: "Close runtime delivery",
      taskKind: "runtime_delivery_closure",
      implementationActions: ["implement_runtime_delivery_contract"],
      objective: "Close runtime delivery fields after runtime-affecting groups.",
      dependsOn: [],
      scopeRefs: ["scope-001"],
      acceptanceRefs: ["AC-001"],
      writeBoundary: { forbiddenPaths: [".loom"], artifactRefs: { modules: ["module-app"], entities: [], interfaces: ["iface-root"], userFlows: ["flow-root"], stateMachines: [], decisions: [], risks: [] } },
      verificationIntents: [{ verificationId: "VI-runtime-closure", acceptanceRefs: ["AC-001"], behavior: "Runtime closure has code-level evidence.", preferredEvidence: ["static_check"], acceptableEvidence: ["static_check"] }],
      frontendExperienceRequirement: {
        frontendExperienceRef: "architectureArtifactContractRef#/frontendExperience",
        experienceLevel: "usable_internal_product",
        mustSatisfy: ["Fixture also covers required frontend experience presence."],
      },
      runtimeDeliveryRequirement: {
        appliesToThisTask: true,
        reason: "Closure covers every RuntimeDeliveryContract field required by the TaskPlan request.",
        runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
        affectedContractFields: closureFields,
        requiredCodeLevelChecks: closureChecks,
        evidenceExpectedInTaskResult: ["Runtime delivery closure fields are checked."],
        forbiddenActions: ["do_not_create_or_edit_deploy_generated_files", "do_not_require_clean_install_or_container_build_for_this_task"],
      },
    };
    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-runtime-group-deps",
      groups: [
        { groupId: "group-runtime-app", title: "Runtime app", objective: "Runtime-affecting implementation.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-app"] },
        { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      ],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-app")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-app", title: "Runtime app", objective: "Runtime-affecting implementation.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-app"] },
      tasks: [runtimeTask],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-closure")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      tasks: [closureTask],
      createdAt: now(),
    });
    const missingGroupDependencyAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(missingGroupDependencyAccept.ok, true);
    assert.equal(missingGroupDependencyAccept.data.accepted, false);
    assert.ok(missingGroupDependencyAccept.data.issues.some((issue) => issue.path === "/groups/group-runtime-closure/dependsOn/group-runtime-app"));

    const crossGroupClosureTask = {
      ...closureTask,
      dependsOn: ["task-runtime-app"],
    };
    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-runtime-group-deps",
      groups: [
        { groupId: "group-runtime-app", title: "Runtime app", objective: "Runtime-affecting implementation.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-app"] },
        { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      ],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-closure")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      tasks: [crossGroupClosureTask],
      createdAt: now(),
    });
    const crossGroupTaskDependencyAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(crossGroupTaskDependencyAccept.ok, true);
    assert.equal(crossGroupTaskDependencyAccept.data.accepted, false);
    assert.ok(crossGroupTaskDependencyAccept.data.issues.some((issue) => issue.path === "/tasks/task-runtime-closure/dependsOn/task-runtime-app"));

    const badCheckIdClosureTask = {
      ...closureTask,
      runtimeDeliveryRequirement: {
        ...closureTask.runtimeDeliveryRequirement,
        requiredCodeLevelChecks: [
          { ...closureChecks[0], checkId: "rd-check-wrong-build" },
          ...closureChecks.slice(1),
        ],
      },
    };
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-closure")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      tasks: [badCheckIdClosureTask],
      createdAt: now(),
    });
    const badCheckIdAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(badCheckIdAccept.ok, true);
    assert.equal(badCheckIdAccept.data.accepted, false);
    assert.ok(badCheckIdAccept.data.issues.some((issue) => issue.path === `/tasks/task-runtime-closure/runtimeDeliveryRequirement/requiredCodeLevelChecks/${closureChecks[0].checkId}`));

    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-runtime-closure-not-final",
      groups: [
        { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
        { groupId: "group-runtime-app", title: "Runtime app", objective: "Runtime-affecting implementation.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-app"] },
      ],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-closure")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      tasks: [closureTask],
      createdAt: now(),
    });
    const closureNotFinalAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(closureNotFinalAccept.ok, true);
    assert.equal(closureNotFinalAccept.data.accepted, false);
    assert.ok(closureNotFinalAccept.data.issues.some((issue) => issue.path === "/groups/group-runtime-closure/position"));

    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-runtime-group-deps",
      groups: [
        { groupId: "group-runtime-app", title: "Runtime app", objective: "Runtime-affecting implementation.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-app"] },
        { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure", "task-extra"] },
      ],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-closure")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure", "task-extra"] },
      tasks: [closureTask, { ...runtimeTask, taskId: "task-extra", groupId: "group-runtime-closure", dependsOn: [] }],
      createdAt: now(),
    });
    const extraClosureGroupTaskAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(extraClosureGroupTaskAccept.ok, true);
    assert.equal(extraClosureGroupTaskAccept.data.accepted, false);
    assert.ok(extraClosureGroupTaskAccept.data.issues.some((issue) => issue.path === "/groups/group-runtime-closure/taskIds"));

    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: "taskplan-runtime-group-deps",
      groups: [
        { groupId: "group-runtime-app", title: "Runtime app", objective: "Runtime-affecting implementation.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-app"] },
        { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      ],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-runtime-closure")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-runtime-closure", title: "Runtime closure", objective: "Runtime delivery closure.", dependsOn: ["group-runtime-app"], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-runtime-closure"] },
      tasks: [closureTask],
      createdAt: now(),
    });
    const groupedRuntimeAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(groupedRuntimeAccept.ok, true);
    assert.equal(groupedRuntimeAccept.data.accepted, true, JSON.stringify(groupedRuntimeAccept.data.issues, null, 2));

    const acceptedAacPath = file(root, `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`);
    const originalAcceptedAac = readJson(acceptedAacPath);
    const sectionOutputPaths = Object.fromEntries(sectionOutputs.map((output) => [output.section, output.candidateFile]));
    writeJson(file(root, sectionOutputPaths.runtime_delivery), sectionCandidate(request, "runtime_delivery", {
      runtimeDelivery: {
        status: "modified",
        contractVersion: "phase-1-api-v1",
        runtimeKind: "generic_http_api",
        basis: {
          technicalBaselineRef: "contracts/technical-baseline.json",
          repositoryContextRef: "workspace/repository-context.json",
          planningGenerationContractRef: "contracts/planning/phase-1/pgc.json",
          previousRuntimeDeliveryRef: null,
          reason: "Fixture runtime follows an API-only stack.",
        },
        build: { command: "make build", workingDirectory: ".", outputs: [], codeLevelExpectations: ["Build prepares the API binary."] },
        start: { command: "./app serve", workingDirectory: ".", entry: "cmd/server", host: "0.0.0.0", port: 8080, portEnv: "PORT", codeLevelExpectations: ["Start runs the API server."] },
        runtimeSurfaces: [{ surfaceId: "api-health", kind: "http", probe: { type: "http_path", target: "/health", expected: "2xx_or_3xx" } }],
        deliveryMechanics: {
          api: { required: true, entry: "cmd/server", basePath: "/api", probePaths: ["/health"] },
          codegen: { required: "no", commands: [], codeLevelExpectations: [] },
        },
        httpProbes: { previewPath: "/health", apiPaths: ["/health"], expectedStatus: "2xx_or_3xx" },
        api: { required: true, kind: "generic_http", entry: "cmd/server", basePath: "/api", probePaths: ["/health"], codeLevelExpectations: ["Health probe is routed by the API server."] },
        environment: { required: [], optional: ["PORT"] },
        taskPlanningGuidance: {
          requireRuntimeDeliveryRequirementWhenTaskTouches: ["build_or_packaging", "runtime_entry", "serving_or_routing", "configuration_or_environment", "generated_artifacts", "runtime_surface"],
          doNotRequireForTaskKinds: ["domain_only_validation", "copy_only_documentation", "pure_unit_test_additions"],
          verificationBoundary: "code_level_only",
          doNotRequireCleanInstallOrContainerBuild: true,
        },
        deployability: { localDocker: "unknown", notes: [] },
      },
    }));
    writeJson(acceptedAacPath, {
      ...originalAcceptedAac,
      frontendExperience: { ...originalAcceptedAac.frontendExperience, required: false },
      runtimeDelivery: readJson(file(root, sectionOutputPaths.runtime_delivery)).content.runtimeDelivery,
      updatedAt: now(),
    });
    const apiOnlyTaskPlanId = "taskplan-api-only-runtime";
    writeJson(file(root, taskPlanRequest.outputContract.outlineFile), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      taskPlanId: apiOnlyTaskPlanId,
      groups: [{ groupId: "group-api-runtime", title: "API runtime", objective: "API runtime closure fixture.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-api-runtime-closure"] }],
      createdAt: now(),
    });
    writeJson(file(root, taskPlanRequest.outputContract.groupFilePattern.replace("{groupId}", "group-api-runtime")), {
      schemaVersion: "1.0",
      requestId: taskPlanRequest.requestId,
      deliveryId,
      phaseId,
      status: "ready",
      group: { groupId: "group-api-runtime", title: "API runtime", objective: "API runtime closure fixture.", dependsOn: [], scopeRefs: ["scope-001"], acceptanceRefs: ["AC-001"], taskIds: ["task-api-runtime-closure"] },
      tasks: [{
        taskId: "task-api-runtime-closure",
        groupId: "group-api-runtime",
        title: "Close API runtime delivery",
        taskKind: "runtime_delivery_closure",
        implementationActions: ["implement_runtime_delivery_contract"],
        objective: "Verify API runtime delivery fields without frontend/static requirements.",
        dependsOn: [],
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        writeBoundary: { forbiddenPaths: [".loom"], artifactRefs: { modules: ["module-app"], entities: [], interfaces: ["iface-root"], userFlows: ["flow-root"], stateMachines: [], decisions: [], risks: [] } },
        verificationIntents: [{ verificationId: "VI-api-runtime", acceptanceRefs: ["AC-001"], behavior: "API runtime has code-level evidence.", preferredEvidence: ["static_check"], acceptableEvidence: ["static_check"] }],
        runtimeDeliveryRequirement: {
          appliesToThisTask: true,
          reason: "API-only runtime closure covers the contract fields present in the API runtime contract.",
          runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
          affectedContractFields: ["build.command", "start.command", "runtimeSurfaces", "httpProbes", "deliveryMechanics.api", "api", "environment"],
          requiredCodeLevelChecks: [
            { checkId: "rd-closure-build-command", contractField: "build.command", objective: "Confirm API build script is wired.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-closure-start-command", contractField: "start.command", objective: "Confirm API start command is wired.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-closure-runtimesurfaces", contractField: "runtimeSurfaces", objective: "Confirm API runtime surface is routed.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-closure-httpprobes", contractField: "httpProbes", objective: "Confirm API probe path is routed.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-closure-deliverymechanics-api", contractField: "deliveryMechanics.api", objective: "Confirm API mechanics are represented.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-closure-api", contractField: "api", objective: "Confirm API contract fields are represented.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-closure-environment", contractField: "environment", objective: "Confirm runtime env handling is represented.", acceptableEvidence: ["static_check"] },
          ],
          evidenceExpectedInTaskResult: ["API runtime fields are checked."],
          forbiddenActions: ["do_not_create_or_edit_deploy_generated_files", "do_not_require_clean_install_or_container_build_for_this_task"],
        },
      }],
      createdAt: now(),
    });
    const apiOnlyAccept = runEnvelope(["task-plan", "accept", "--delivery-id", deliveryId, "--phase-id", phaseId, "--request-id", taskPlanRequest.requestId], root);
    assert.equal(apiOnlyAccept.ok, true);
    assert.equal(apiOnlyAccept.data.accepted, true);

    writeJson(acceptedAacPath, originalAcceptedAac);
    writeArchitectureSections(root, request);

    const runId = "run-fe-rd";
    const taskPlanId = "taskplan-fe-rd";
    const runtimeTaskId = "task-runtime-delivery";
    writeJson(file(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/${taskPlanId}.json`), {
      schemaVersion: "1.0",
      taskPlanId,
      version: 1,
      status: "ready",
      source: {
        roadmapId: null,
        phaseId,
        planningGenerationContractId: "pgc-001",
        architectureArtifactContractId: "aac-001",
        technicalBaselineId: "tb-001",
      },
      scopeSnapshot: { includedScopeRefs: ["scope-001"], excludedScopeRefs: [], deferredScopeRefs: [], acceptanceRefs: ["AC-001"] },
      planningPolicy: { taskGranularity: "engineering_increment", groupGranularity: "engineering_capability", allowTaskSplitDuringRepair: true, allowTaskMergeDuringRepair: false },
      groups: [{ groupId: "group-runtime", title: "Runtime", objective: "Verify runtime.", acceptanceRefs: ["AC-001"], scopeRefs: ["scope-001"], taskIds: [runtimeTaskId], dependsOn: [] }],
      tasks: [{
        taskId: runtimeTaskId,
        groupId: "group-runtime",
        title: "Verify runtime delivery",
        taskKind: "runtime_delivery_closure",
        implementationActions: ["implement_runtime_delivery_contract"],
        objective: "Verify runtime delivery.",
        dependsOn: [],
        scopeRefs: ["scope-001"],
        acceptanceRefs: ["AC-001"],
        writeBoundary: { forbiddenPaths: [".loom"], artifactRefs: { modules: ["module-app"], entities: [], interfaces: ["iface-root"], userFlows: ["flow-root"], stateMachines: [], decisions: [], risks: [] } },
        verificationIntents: [{ verificationId: "VI-runtime", acceptanceRefs: ["AC-001"], behavior: "Runtime can build and serve.", preferredEvidence: ["automated_test"], acceptableEvidence: ["automated_test", "runtime_api_check"] }],
        runtimeDeliveryRequirement: {
          appliesToThisTask: true,
          reason: "Runtime delivery fixture task checks build and preview wiring.",
          runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
          affectedContractFields: ["build.command", "start.command", "runtimeSurfaces", "httpProbes", "deliveryMechanics.staticAssets", "deliveryMechanics.api", "frontend", "api", "environment"],
          requiredCodeLevelChecks: [
            { checkId: "rd-check-build", contractField: "build.command", objective: "Confirm build command still covers runtime deliverables.", acceptableEvidence: ["manual_command_output", "static_check"] },
            { checkId: "rd-check-start", contractField: "start.command", objective: "Confirm start entry and command are wired.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-check-surfaces", contractField: "runtimeSurfaces", objective: "Confirm runtime surfaces have code-level routes or handlers.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-check-http", contractField: "httpProbes", objective: "Confirm HTTP probe targets are represented in code/config.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-check-static", contractField: "deliveryMechanics.staticAssets", objective: "Confirm static assets output is served by runtime.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-check-api-mechanics", contractField: "deliveryMechanics.api", objective: "Confirm API entry/base paths are mounted.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-check-frontend", contractField: "frontend", objective: "Confirm frontend source/output/serving contract is wired.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-check-api", contractField: "api", objective: "Confirm API contract is wired.", acceptableEvidence: ["static_check"] },
            { checkId: "rd-check-env", contractField: "environment", objective: "Confirm required runtime env has code/config handling.", acceptableEvidence: ["static_check"] },
          ],
          evidenceExpectedInTaskResult: ["runtimeDeliveryEvidence.checkedFields includes build.command."],
          forbiddenActions: ["do_not_create_or_edit_deploy_generated_files", "do_not_require_clean_install_or_container_build_for_this_task"],
        },
      }],
      handoff: { readyForExecution: true, nextNode: "task_execution", blockedReasons: [] },
      createdAt: now(),
      updatedAt: now(),
    });
    writeJson(file(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/latest.json`), {
      schemaVersion: "1.0",
      taskPlanId,
      taskPlanRef: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/taskplans/${taskPlanId}.json`,
      updatedAt: now(),
    });
    writeJson(file(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/${runId}.json`), {
      schemaVersion: "1.0",
      runId,
      taskPlanId,
      status: "completed",
      scheduler: { mode: "group_dag", startedAt: now(), finishedAt: now() },
      groupStates: [{ groupId: "group-runtime", status: "completed", startedAt: now(), finishedAt: now(), dependsOn: [], taskIds: [runtimeTaskId] }],
      taskStates: [{ taskId: runtimeTaskId, groupId: "group-runtime", status: "completed", resultId: `result-${runtimeTaskId}`, startedAt: now(), finishedAt: now(), dependsOn: [], attempts: [{ attempt: 1, resultId: `result-${runtimeTaskId}`, status: "completed" }] }],
      summary: { total: 1, completed: 1, completedWithNotes: 0, failed: 0, blocked: 0, pending: 0, running: 0 },
      nextAction: { type: "review", reason: "TASK_PLAN_RUN_COMPLETED", targetNode: "review" },
      createdAt: now(),
      updatedAt: now(),
    });
    writeJson(file(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/latest.json`), {
      schemaVersion: "1.0",
      taskPlanRunId: runId,
      taskPlanRunRef: `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/${runId}.json`,
      updatedAt: now(),
    });
    fs.rmSync(file(root, `.loom/deliveries/${deliveryId}/operations/active-lease.json`), { force: true });

    writeJson(file(root, `.loom/deliveries/${deliveryId}/tasks/${phaseId}/runs/${runId}.json`), {
      schemaVersion: "1.0",
      runId,
      taskPlanId,
      status: "not_started",
      scheduler: { mode: "group_dag", startedAt: null, finishedAt: null },
      groupStates: [{ groupId: "group-runtime", status: "pending", startedAt: null, finishedAt: null, dependsOn: [], taskIds: [runtimeTaskId] }],
      taskStates: [{ taskId: runtimeTaskId, groupId: "group-runtime", status: "pending", resultId: null, startedAt: null, finishedAt: null, dependsOn: [], attempts: [] }],
      summary: { total: 1, completed: 0, completedWithNotes: 0, failed: 0, blocked: 0, pending: 1, running: 0 },
      nextAction: { type: "continue_execution", reason: "TASKPLAN_READY", targetNode: "task_execution" },
      createdAt: now(),
      updatedAt: now(),
    });
    const nextTask = run(["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const executionRequest = nextTask.request ?? hydrateRequest(root, readJson(file(root, nextTask.executionRequestPath)));
    assert.ok(executionRequest.outputContract.requiredTopLevelFields.includes("blockedReasons"));
    assert.ok(executionRequest.outputContract.requiredTopLevelFields.includes("createdAt"));
    assert.ok(executionRequest.outputContract.requiredTopLevelFields.includes("updatedAt"));
    assert.deepEqual(executionRequest.agentAction.write.requiredTopLevelFields, executionRequest.outputContract.requiredTopLevelFields);
    assert.equal(executionRequest.executionRules.runtimeDeliveryExecutionRules.mustRecordRuntimeProbeCleanupWhenTemporaryRuntimeStarted, true);
    assert.equal(executionRequest.executionRules.runtimeDeliveryExecutionRules.runtimeProbeCleanupFailureSeverity, "completed_with_notes_only");
    assert.equal(executionRequest.executionRules.runtimeDeliveryExecutionRules.foregroundRuntimeCommandsForbidden, true);
    assert.equal(executionRequest.executionRules.runtimeDeliveryExecutionRules.controlledRuntimeProbeRequired, true);
    assert.ok(executionRequest.executionRules.runtimeDeliveryExecutionRules.runtimeCommandGuardRules.some((rule) => /any technology stack command/.test(rule)));
    assert.ok(executionRequest.outputContract.resultRules.some((rule) => /runtimeProbeCleanup/.test(rule)));
    assert.ok(executionRequest.outputContract.resultRules.some((rule) => /requiredCheckIds/.test(rule)));
    assert.deepEqual(
      executionRequest.agentAction.write.requiredRuntimeEvidence.requiredCheckIds,
      [
        "rd-check-build",
        "rd-check-start",
        "rd-check-surfaces",
        "rd-check-http",
        "rd-check-static",
        "rd-check-api-mechanics",
        "rd-check-frontend",
        "rd-check-api",
        "rd-check-env",
      ],
    );
    assert.deepEqual(
      executionRequest.outputContract.requiredRuntimeEvidence.requiredCheckIds,
      executionRequest.agentAction.write.requiredRuntimeEvidence.requiredCheckIds,
    );
    assert.ok(executionRequest.agentAction.write.rules.some((rule) => /Copy every exact checkId/.test(rule)));
    assert.deepEqual(
      executionRequest.outputContract.schemaShape.runtimeDeliveryEvidence.requiredCheckIds,
      [
        "rd-check-build",
        "rd-check-start",
        "rd-check-surfaces",
        "rd-check-http",
        "rd-check-static",
        "rd-check-api-mechanics",
        "rd-check-frontend",
        "rd-check-api",
        "rd-check-env",
      ],
    );
    assert.equal(executionRequest.outputContract.schemaShape.runtimeDeliveryEvidence.allowedCodeLevelChecks[0].checkId, "rd-check-build");
    assert.equal(Object.prototype.hasOwnProperty.call(nextTask.instruction, "executionSteps"), false);
    assert.equal(nextTask.instruction.runtimeCommandGuard, undefined);
    assert.equal(nextTask.instruction.verificationCommandSchedulingRules, undefined);
    assert.ok(nextTask.instruction.requestReadProtocol.readRule.includes("requestReadPlan.groups"));
    assert.ok(executionRequest.executionRules.rules.some((rule) => /temporary local runtime, dev server, preview server, container, or probe process/.test(rule)));
    assert.ok(executionRequest.executionRules.rules.some((rule) => /write-producing verification commands/.test(rule)));
    assert.ok(executionRequest.executionRules.rules.some((rule) => /Never run long-lived runtime\/server commands/.test(rule)));
    assert.ok(executionRequest.outputContract.schemaShape.runtimeDeliveryEvidence.codeLevelCheckRules.some((rule) => /foreground verification commands/.test(rule)));
    const executionRulesRef = executionRequest.requestManifest.refs.executionRules.ref;
    const staleExecutionRules = readJson(file(root, executionRulesRef));
    delete staleExecutionRules.runtimeDeliveryExecutionRules.foregroundRuntimeCommandsForbidden;
    delete staleExecutionRules.runtimeDeliveryExecutionRules.controlledRuntimeProbeRequired;
    staleExecutionRules.rules = staleExecutionRules.rules.filter((rule) => !/Never run long-lived runtime\/server commands/.test(rule));
    writeJson(file(root, executionRulesRef), staleExecutionRules);
    const recoveredRuntimeTask = run(["continue"], root);
    assert.equal(recoveredRuntimeTask.instruction.mode, "execute_task");
    assert.equal(recoveredRuntimeTask.instruction.runtimeCommandGuard, undefined);
    assert.equal(recoveredRuntimeTask.instruction.verificationCommandSchedulingRules, undefined);
    const refreshedExecutionRequest = hydrateRequest(root, readJson(file(root, nextTask.executionRequestPath)));
    assert.equal(refreshedExecutionRequest.executionRules.runtimeDeliveryExecutionRules.foregroundRuntimeCommandsForbidden, true);
    assert.ok(refreshedExecutionRequest.executionRules.rules.some((rule) => /Never run long-lived runtime\/server commands/.test(rule)));

    const runtimeResultFile = executionRequest.outputContract.resultFile;
    const invalidRuntimeResultFile = runtimeResultFile.replace(/\.json$/, "-invalid-check-id.json");
    writeJson(file(root, invalidRuntimeResultFile), {
      schemaVersion: "1.0",
      taskResultId: `result-${runtimeTaskId}-invalid-check-id`,
      taskId: runtimeTaskId,
      taskPlanId,
      status: "completed_with_notes",
      changedFiles: ["package.json"],
      noChangeReason: null,
      verificationResults: [{ verificationId: "VI-runtime", status: "passed", evidenceType: "automated_test", summary: "Local warm build passed." }],
      selfRepairSummary: null,
      failure: null,
      executionContinuity: {
        taskResultSubmittedAfterVerification: true,
        agentOwnedLongRunningWork: "none",
        notes: [],
      },
      notes: ["Invalid check id fixture."],
      runtimeDeliveryEvidence: {
        requirementRef: "architectureArtifactContractRef#/runtimeDelivery",
        checkedFields: ["build.command", "start.command", "runtimeSurfaces", "httpProbes", "deliveryMechanics.staticAssets", "deliveryMechanics.api", "frontend", "api", "environment"],
        codeLevelChecks: [
          { checkId: "rd-check-not-in-request", contractField: "build.command", status: "passed", evidence: "Invalid fixture." },
        ],
        commandsRun: [],
        unverifiedItems: [],
      },
      blockedReasons: [],
      createdAt: now(),
      updatedAt: now(),
    });
    const invalidRecord = runEnvelope(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", invalidRuntimeResultFile], root);
    assert.equal(invalidRecord.ok, true);
    assert.equal(invalidRecord.data.recorded, false);
    assert.ok(invalidRecord.data.issues.some((issue) => issue.code === "TASK_RESULT_RUNTIME_CHECK_ID_INVALID"));
    assert.equal(invalidRecord.data.repairInstruction.allowedRuntimeCodeLevelChecks, undefined);
    assert.ok(
      invalidRecord.data.repairInstruction.inspectRepairContractCommand.argv.join(" ").includes("outputContract.schemaShape.runtimeDeliveryEvidence"),
      "runtime TaskResult repair must expose runtime contract through on-demand inspect instead of inline allowedRuntimeCodeLevelChecks",
    );

    writeJson(file(root, runtimeResultFile), {
      schemaVersion: "1.0",
      taskResultId: `result-${runtimeTaskId}`,
      taskId: runtimeTaskId,
      taskPlanId,
      status: "completed_with_notes",
      changedFiles: ["package.json"],
      noChangeReason: null,
      verificationResults: [{ verificationId: "VI-runtime", status: "passed", evidenceType: "automated_test", summary: "Local warm build passed." }],
      selfRepairSummary: null,
      failure: null,
      executionContinuity: {
        taskResultSubmittedAfterVerification: true,
        agentOwnedLongRunningWork: "unknown",
        notes: ["Runtime probe cleanup did not confirm process shutdown."],
      },
      notes: ["Runtime probe cleanup did not confirm process shutdown; this is a non-blocking cleanup note."],
      runtimeDeliveryEvidence: {
        requirementRef: "architectureArtifactContractRef#/runtimeDelivery",
        checkedFields: ["build.command", "start.command", "runtimeSurfaces", "httpProbes", "deliveryMechanics.staticAssets", "deliveryMechanics.api", "frontend", "api", "environment"],
        codeLevelChecks: [
          { checkId: "rd-check-build", contractField: "build.command", status: "passed", evidence: "Build script covers runtime deliverables." },
          { checkId: "rd-check-start", contractField: "start.command", status: "passed", evidence: "Start script entry exists." },
          { checkId: "rd-check-surfaces", contractField: "runtimeSurfaces", status: "passed", evidence: "Root route is declared." },
          { checkId: "rd-check-http", contractField: "httpProbes", status: "passed", evidence: "Probe paths map to handlers." },
          { checkId: "rd-check-static", contractField: "deliveryMechanics.staticAssets", status: "passed", evidence: "Static output is served." },
          { checkId: "rd-check-api-mechanics", contractField: "deliveryMechanics.api", status: "passed", evidence: "API mechanics are mounted." },
          { checkId: "rd-check-frontend", contractField: "frontend", status: "passed", evidence: "Frontend contract paths are coherent." },
          { checkId: "rd-check-api", contractField: "api", status: "passed", evidence: "API entry and base path are coherent." },
          { checkId: "rd-check-env", contractField: "environment", status: "passed", evidence: "PORT env is optional/defaulted." },
        ],
        commandsRun: [{
          command: "npm run build",
          status: "passed",
          environment: "local_warm",
          summary: "Build passed with existing local dependencies.",
        }],
        unverifiedItems: [],
        runtimeProbeCleanup: {
          temporaryRuntimeStarted: true,
          attempted: true,
          status: "failed",
          targets: [{
            kind: "port",
            port: 3100,
            command: "node dist/server/index.js",
            summary: "Local warm runtime probe was started by this task.",
          }],
          summary: "Cleanup did not confirm the probe server stopped; recorded as a non-blocking note.",
        },
      },
      blockedReasons: [],
      createdAt: now(),
      updatedAt: now(),
    });
    const record = run(["record-result", "--delivery-id", deliveryId, "--phase-id", phaseId, "--input-file", runtimeResultFile], root);
    assert.equal(record.recorded, true);
    assert.equal(record.run.status, "completed_with_notes");

    const review = run(["review", "--delivery-id", deliveryId, "--phase-id", phaseId], root);
    const reviewRequest = review.request ?? hydrateRequest(root, readJson(latestFile(root, {
      dir: `.loom/deliveries/${deliveryId}/reviews/${phaseId}/requests`,
      pattern: /^review-.*\.json$/,
    })));
    const reviewPacket = readJson(file(root, reviewRequest.reviewPacketRef));
    const runtimeTaskResult = reviewPacket.taskResults.find((result) => result.taskId === runtimeTaskId);
    assert.equal(runtimeTaskResult.runtimeDeliveryEvidence.commandsRun[0].environment, "local_warm");
    assert.equal(runtimeTaskResult.runtimeDeliveryEvidence.runtimeProbeCleanup.status, "failed");
    assert.ok(reviewRequest.outputContract.runtimeDeliveryReview.reviewGuidance.some((rule) => /code-level consistency/.test(rule)));
    assert.ok(reviewRequest.outputContract.runtimeDeliveryReview.reviewGuidance.some((rule) => /runtimeProbeCleanup/.test(rule)));
    assert.equal(reviewRequest.outputContract.runtimeDeliveryReview.reviewGuidance.some((rule) => /container-like/.test(rule)), false);
    assert.ok(reviewRequest.outputContract.severityPolicy.doNotBlockOn.some((rule) => /runtime\/probe cleanup/.test(rule)));
    assert.ok(reviewRequest.outputContract.reviewSignals.some((signal) => (
      signal.kind === "task_result_evidence_presence" &&
      signal.evidenceType === "runtime_delivery" &&
      signal.isClosureTaskEvidence === true &&
      signal.checkedFieldCount === 9 &&
      signal.codeLevelCheckCount === 9 &&
      signal.codeLevelCheckIds.includes("rd-check-env")
    )));
    console.log("Frontend/runtime/severity protocol verification passed");
  } finally {
    if (process.env.KEEP_VERIFY_TMP === "1") {
      console.error(`Keeping verification fixture at ${root}`);
    } else {
      fs.rmSync(root, { recursive: true, force: true });
    }
  }
}

main();
