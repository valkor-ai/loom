#!/usr/bin/env node

const assert = require("node:assert/strict");
const { access, mkdir, mkdtemp, writeFile } = require("node:fs/promises");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { spawnSync } = require("node:child_process");

async function main() {
  const root = await mkdtemp(join(tmpdir(), "loom-deploy-blockers-"));

  await verifySourceInsufficientBlocker(join(root, "source-insufficient"));
  await verifyConflictBlocker(join(root, "baseline-conflict"));

  console.log(`deploy structured blocker verification passed in ${root}`);
}

async function verifySourceInsufficientBlocker(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.DATABASE_URL)\n", "utf8");

  const envelope = runDeployPrepare(projectRoot, [2]);

  assert.equal(envelope.ok, false);
  assert.equal(envelope.error.code, "DEPLOY_SOURCE_INSUFFICIENT");
  assert.equal(envelope.error.details.code, "DEPLOY_SOURCE_INSUFFICIENT");
  assert.equal(envelope.error.details.status, "blocked");
  assert.equal(typeof envelope.error.details.evidenceRef, "string");
  assert.ok(envelope.error.details.evidenceRef.length > 0);
  assert.ok(envelope.error.details.missingFacts.some((fact) => fact.type === "database_kind"));
  assertNoAgentRunnableRepair(envelope, "source insufficient");
  assert.ok(await fileExists(join(projectRoot, ".loom/deployment/evidence/latest-code-evidence.json")));
  assert.equal(await fileExists(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml")), false);
}

async function verifyConflictBlocker(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
      mysql2: "^3.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.DATABASE_URL)\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    buildCommand: "npm run build",
    startCommand: "npm run start",
    startPort: 4173,
    previewPath: "/",
    healthPath: "/",
    frontendOutputDir: "dist",
  });
  await writeTechnicalBaseline(projectRoot, {
    backend: "Node.js + Express",
    persistence: "PostgreSQL",
    dataAccess: "Raw SQL / lightweight wrapper",
  });

  const envelope = runDeployPrepare(projectRoot, [2]);

  assert.equal(envelope.ok, false);
  assert.equal(envelope.error.code, "DEPLOY_CONFLICT");
  assert.equal(envelope.error.details.code, "DEPLOY_CONFLICT");
  assert.equal(envelope.error.details.status, "blocked");
  assert.equal(typeof envelope.error.details.evidenceRef, "string");
  assert.ok(envelope.error.details.evidenceRef.length > 0);
  assert.ok(envelope.error.details.conflicts.some((conflict) => conflict.type === "technical_baseline_code_conflict"));
  assertNoAgentRunnableRepair(envelope, "baseline conflict");
  assert.ok(await fileExists(join(projectRoot, ".loom/deployment/evidence/latest-code-evidence.json")));
  assert.equal(await fileExists(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml")), false);
}

function assertNoAgentRunnableRepair(envelope, label) {
  assert.equal("instruction" in envelope, false, `${label}: failure envelope must not include instruction.`);
  assert.equal("actionRequired" in envelope, false, `${label}: failure envelope must not include actionRequired.`);
  assert.equal("data" in envelope, false, `${label}: failure envelope must not include success data.`);
  assert.equal("repairInstruction" in envelope, false, `${label}: failure envelope must not include repairInstruction.`);
  assert.equal("repair" in envelope, false, `${label}: failure envelope must not include repair payload.`);

  const details = envelope.error.details ?? {};
  assert.equal("instruction" in details, false, `${label}: error details must not include instruction.`);
  assert.equal("actionRequired" in details, false, `${label}: error details must not include actionRequired.`);
  assert.equal("repairInstruction" in details, false, `${label}: error details must not include repairInstruction.`);
  assert.equal("repairRoute" in details, false, `${label}: error details must not include repairRoute.`);
  assert.equal(details.agentFacingBlocker?.mode, "report_blocker", `${label}: error details must expose report-only blocker protocol.`);
  assert.equal(details.agentFacingBlocker?.retryPolicy?.autoRetry, false, `${label}: blocker protocol must forbid auto retry.`);
  assert.ok(
    details.agentFacingBlocker?.forbiddenActions?.some((action) => /deploy repair/i.test(action)),
    `${label}: blocker protocol must forbid deploy repair.`,
  );
  assert.ok(
    details.agentFacingBlocker?.forbiddenActions?.some((action) => /raw Docker/i.test(action)),
    `${label}: blocker protocol must forbid raw Docker.`,
  );
  assert.ok(
    details.agentFacingBlocker?.forbiddenActions?.some((action) => /Dockerfile|Compose/i.test(action)),
    `${label}: blocker protocol must forbid deployment asset edits from memory.`,
  );
  if (details.retryCommand) {
    assert.notEqual(details.retryCommand.autoContinue, true, `${label}: retryCommand must not be auto-runnable.`);
    assert.notEqual(details.retryCommand.mustRunImmediately, true, `${label}: retryCommand must not be immediate.`);
  }
}

async function writePackage(projectRoot, pkg) {
  await mkdir(projectRoot, { recursive: true });
  await writeFile(join(projectRoot, "package.json"), `${JSON.stringify(pkg, null, 2)}\n`, "utf8");
}

async function writeTechnicalBaseline(projectRoot, input) {
  const deliveryId = "delivery-runtime";
  const now = new Date().toISOString();
  const contractsDir = join(projectRoot, ".loom/deliveries", deliveryId, "contracts");
  await mkdir(contractsDir, { recursive: true });
  await writeFile(join(contractsDir, "technical-baseline.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    technicalBaselineId: "tb-runtime",
    status: "confirmed",
    source: "user_specified",
    projectKind: "greenfield",
    scope: "roadmap",
    stack: {
      tracks: {
        web: track(input.web ?? "No Web client"),
        app: track(input.app ?? "No App client"),
        backend: track(input.backend ?? "No independent backend"),
        persistence: track(input.persistence ?? "No persistence yet"),
        dataAccess: track(input.dataAccess ?? "No ORM"),
        externalServices: track(input.externalServices ?? "None"),
      },
      derivedLater: ["testing", "build", "local run", "deployment preparation"],
    },
    constraints: [],
    evidence: [{ reason: "Deploy structured blocker verifier technical baseline fixture." }],
    approval: {
      type: "user_confirmed",
      confirmedAt: now,
      confirmedBy: "test",
    },
    confidence: "high",
    reasoningSummary: ["Deploy structured blocker verifier baseline."],
    alternatives: [],
    createdAt: now,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
}

function track(selection) {
  const normalized = String(selection).toLowerCase();
  const notNeeded = /no |none|不需要/.test(normalized);
  return {
    status: notNeeded ? "not_needed" : "selected",
    selection,
    source: "user_specified",
    rationale: "Deploy structured blocker verifier fixture.",
  };
}

async function writeAcceptedRuntimeDelivery(projectRoot, input) {
  const deliveryId = "delivery-runtime";
  const phaseId = "phase-1";
  const now = new Date().toISOString();
  const architectureDir = join(projectRoot, ".loom/deliveries", deliveryId, "artifacts/architecture", phaseId);
  await mkdir(architectureDir, { recursive: true });
  await writeFile(join(projectRoot, ".loom/status.json"), `${JSON.stringify({
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "planning",
      requestSummary: "Deploy structured blocker verifier fixture.",
      activePhaseId: phaseId,
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now,
    }],
    phase: "planning",
    current: { requirementId: null, planId: null, taskId: null, reviewId: null, repairId: null, deploymentId: null },
    lastAction: null,
    nextAction: "plan",
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
  await writeFile(join(projectRoot, ".loom/deliveries", deliveryId, "index.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Deploy structured blocker verifier fixture.",
    roadmapId: null,
    activePhaseId: phaseId,
    phases: [{
      phaseId,
      name: "Phase 1",
      status: "planning",
      latestRefs: {
        architectureArtifact: `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`,
      },
      nextAction: null,
    }],
    createdAt: now,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
  await writeFile(join(architectureDir, "latest.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-runtime",
    artifactRef: `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
  await writeFile(join(architectureDir, "aac.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-runtime",
    status: "ready",
    source: {
      planningGenerationContractId: "pgc-runtime",
      technicalBaselineId: "tb-runtime",
      brainstormContractId: "bc-runtime",
      roadmapId: null,
      phaseId,
    },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-main", type: "web_app", root: "." }],
      modules: [],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [],
    dataModel: { entities: [], relationships: [], constraints: [] },
    interfaces: [],
    userFlows: [],
    stateMachines: [],
    runtimeDelivery: {
      status: "modified",
      contractVersion: "phase-1-v1",
      runtimeKind: "node_express_serves_vite_static",
      basis: {
        technicalBaselineRef: "technical-baseline",
        repositoryContextRef: "repository-context",
        planningGenerationContractRef: "planning-contract",
        previousRuntimeDeliveryRef: null,
        reason: "Deploy structured blocker verifier runtime contract.",
      },
      build: {
        command: input.buildCommand,
        workingDirectory: ".",
        outputs: ["dist/server", input.frontendOutputDir],
        codeLevelExpectations: ["Build produces server and frontend outputs."],
      },
      start: {
        command: input.startCommand,
        workingDirectory: ".",
        entry: "dist/server.js",
        host: "0.0.0.0",
        port: input.startPort,
        portEnv: "PORT",
        codeLevelExpectations: ["Start serves the app on the declared port."],
      },
      runtimeSurfaces: [{ surfaceId: "preview-root", kind: "http", probe: { type: "http_path", target: input.previewPath, expected: "2xx_or_3xx" } }],
      deliveryMechanics: {
        staticAssets: { required: true, source: "src/ui", output: input.frontendOutputDir, servedBy: "express_static" },
        api: { required: true, entry: "dist/server.js", basePath: "/api", probePaths: ["/api/health"] },
        codegen: { required: "no", commands: [], codeLevelExpectations: [] },
      },
      httpProbes: { previewPath: input.previewPath, healthPath: input.healthPath, apiPaths: ["/api/health"], expectedStatus: "2xx_or_3xx" },
      frontend: { required: true, kind: "vite_react", buildCommand: "npm run build:web", sourceRoot: "src/ui", outputDir: input.frontendOutputDir, servedBy: "express_static", servedByRef: "src/server.ts", codeLevelExpectations: ["Frontend output is mounted by the server."] },
      api: { required: true, kind: "express", buildCommand: "npm run build:api", entry: "dist/server.js", basePath: "/api", probePaths: ["/api/health"], codeLevelExpectations: ["Health API remains available."] },
      environment: { required: [], optional: ["PORT"] },
      taskPlanningGuidance: {
        requireRuntimeDeliveryRequirementWhenTaskTouches: ["build_or_packaging", "runtime_entry", "serving_or_routing", "configuration_or_environment", "generated_artifacts", "runtime_surface"],
        doNotRequireForTaskKinds: ["domain_only_validation", "copy_only_documentation", "pure_unit_test_additions"],
        verificationBoundary: "code_level_only",
        doNotRequireCleanInstallOrContainerBuild: true,
      },
      deployability: { localDocker: "supported", notes: [] },
    },
    acceptanceMatrix: [],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    createdAt: now,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
}

function runDeployPrepare(projectRoot, expectedStatuses) {
  return runLoom(["deploy", "prepare", "--project-root", projectRoot, "--json"], expectedStatuses);
}

function runLoom(args, expectedStatuses) {
  const result = spawnSync(
    process.execPath,
    ["dist/cli.js", ...args],
    {
      cwd: join(__dirname, ".."),
      encoding: "utf8",
      env: { ...process.env, LOOM_AGENT_PROFILE: "codex", LOOM_COMPACT_OUTPUT: "1" },
    },
  );

  assert.ok(expectedStatuses.includes(result.status), result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

async function fileExists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
