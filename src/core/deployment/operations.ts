import { promises as fs } from "node:fs";
import path from "node:path";
import { deployConflict, deployNotRunning, deploySourceInsufficient, deployValidationFailed, dockerUnavailable, LoomError } from "../errors";
import { ensureDir, pathExists } from "../state/fs";
import { toProjectRelative } from "../state/paths";
import { analyzeDeploymentBootstrap } from "./bootstrap";
import {
  assertNoActiveDeploymentOperation,
  readDeploymentOperationView,
  updateDeploymentOperationPhase,
  withDeploymentOperation,
  type DeploymentOperationHandle,
} from "./active-operation";
import {
  applyDeploymentCodeEvidenceToStack,
  buildDeploymentCodeEvidence,
  loadDeploymentTechnicalBaseline,
  writeDeploymentCodeEvidence,
} from "./code-evidence";
import { DEFAULT_DEPLOY_BUILD_TIMEOUT_MS } from "./constants";
import { diagnoseDeploymentFailure } from "./diagnostics";
import {
  analyzeDeploymentEnvironment,
  generatedDependencyEnvironment,
  generatedRuntimeEnvironment,
} from "./env";
import {
  analyzeExistingCompose,
  detectComposePublishedPort,
  findExistingDeploymentFiles,
  selectedComposePort,
} from "./existing";
import { applyHealthcheckInput } from "./healthcheck";
import { createDeploymentSpec, generateComposeForDockerfile, generateDeploymentFiles } from "./generate";
import { getDeploymentPaths } from "./paths";
import {
  applyHealthyPath,
  containerNameFor,
  createRunningState,
  checkDeploymentHealth,
  checkDeploymentPreview,
  dockerCompose,
  dockerComposeExec,
  ensureDockerAvailable,
  findComposeServiceContainer,
  findAvailablePort,
  inspectContainer,
  resolveComposePath,
} from "./runtime";
import { classifyDeploymentRepair, writeDeploymentRepairRequest } from "./repair";
import { applyRuntimeContractToStack, loadDeploymentRuntimeContract } from "./runtime-contract";
import { resolveDeploymentStrategy } from "./strategy";
import { validateStartupLogs } from "./validate";
import { discoverNodeWorkspacePackageJsonPaths, resolveDeploymentWorkspaceForApp } from "./workspace";
import { withAutoRunnableTransition } from "../operations/routing-instructions";
import {
  appendDeploymentLog,
  clearDeploymentFailureArtifacts,
  readDeploymentRepairRequest,
  readDeploymentSpec,
  readDeploymentState,
  writeDeploymentSpec,
  writeDeploymentState,
} from "./state";
import type {
  DeploymentBootstrapRunResult,
  DeploymentErrorWindow,
  DeploymentFailureDiagnostic,
  DeploymentFailureKind,
  DeploymentFailureOwner,
  DeploymentHealth,
  DeploymentHealthcheckInput,
  DeploymentProviderPolicy,
  DeploymentRepairRoute,
  DeploymentSpec,
  DeploymentState,
  DeploymentCodeEvidence,
  DeploymentActiveOperationView,
  DeployProvider,
} from "./types";

export type DeployPrepareResult = {
  prepared: true;
  specPath: string;
  detectedStack: DeploymentSpec["detectedStack"];
  provider: DeploymentSpec["provider"];
  providerReason: string;
  providerPolicy: DeploymentSpec["providerPolicy"];
  providerCandidates: DeploymentSpec["providerCandidates"];
  workspace: DeploymentSpec["workspace"];
  codeEvidence: DeploymentSpec["codeEvidence"] | null;
  environment: DeploymentSpec["environment"];
  bootstrap: DeploymentSpec["bootstrap"];
  compose: DeploymentSpec["compose"];
  files: DeploymentSpec["files"];
  url: string;
  reused: string[];
};

type DeployOperationAwareResult = {
  operationActive?: boolean;
  activeOperation?: DeploymentActiveOperationView | null;
};

export type DeployUpResult = {
  started: boolean;
  url: string | null;
  containerId: string | null;
  serviceName: string;
  appServiceName: string;
  composePath: string;
  health: DeploymentHealth;
  instruction?: Record<string, unknown>;
} & DeployOperationAwareResult;

export type DeployStatusResult = {
  running: boolean;
  url: string | null;
  containerId: string | null;
  serviceName: string | null;
  appServiceName: string | null;
  health: DeploymentHealth | null;
  instruction?: Record<string, unknown>;
} & DeployOperationAwareResult;

export type DeployValidateResult = {
  valid: boolean;
  specPath: string;
  composePath: string;
  config: {
    ok: boolean;
    exitCode: number;
    stderrTail: string[];
  };
  health: DeploymentHealth | null;
  instruction?: Record<string, unknown>;
};

export type DeployLogsResult = {
  lines: string[];
  truncated: boolean;
  fullLogRef: string;
} & DeployOperationAwareResult;

export type DeployDownResult = {
  stopped: boolean;
  removed: boolean;
};

export type DeployRepairResult = {
  hasRepairRequest: boolean;
  repairId: string | null;
  failureKind: string | null;
  provider: DeploymentSpec["provider"] | null;
  command: string[];
  failureOwner: DeploymentFailureOwner | null;
  repairRoute: DeploymentRepairRoute | null;
  failureRef: string | null;
  fullLogRef: string | null;
  errorWindow: DeploymentErrorWindow | null;
  providerCandidates: DeploymentSpec["providerCandidates"];
  environment: DeploymentSpec["environment"] | null;
  bootstrap: DeploymentSpec["bootstrap"] | null;
  diagnostics: DeploymentFailureDiagnostic[] | null;
  suggestedActions: string[];
  editableFiles: string[];
  protectedFiles: string[];
  stdoutTail: string[];
  stderrTail: string[];
  instruction: string | null;
  repairInstruction?: Record<string, unknown>;
  maxAttempts: number;
  attempts: number;
  nextAction: "edit-and-rerun" | "execution-repair" | "request-user-approval" | "none";
};

export type DeployBootstrapResult = {
  tasks: DeploymentSpec["bootstrap"]["tasks"];
  executed: DeploymentBootstrapRunResult[];
  skipped: DeploymentBootstrapRunResult[];
  confirmed: boolean;
  serviceName: string | null;
  composePath: string;
  warnings: string[];
};

export type DeployInspectResult = {
  prepared: boolean;
  refreshed: boolean;
  provider: DeploymentSpec["provider"] | null;
  providerReason: string | null;
  providerPolicy: DeploymentSpec["providerPolicy"] | null;
  providerCandidates: DeploymentSpec["providerCandidates"];
  workspace: DeploymentSpec["workspace"] | null;
  codeEvidence: DeploymentSpec["codeEvidence"] | null;
  codeEvidenceReadGuide: Record<string, unknown> | null;
  detectedStack: DeploymentSpec["detectedStack"] | null;
  files: DeploymentSpec["files"] | null;
  compose: DeploymentSpec["compose"] | null;
  runtime: DeploymentSpec["runtime"] | null;
  environment: {
    missing: DeploymentSpec["environment"]["missing"];
    warnings: string[];
    provided: string[];
  } | null;
  bootstrap: DeploymentSpec["bootstrap"] | null;
  state: DeploymentState | null;
  repair: DeployRepairResult | null;
  summary: {
    appPath: string | null;
    appServiceName: string | null;
    url: string | null;
    running: boolean;
    healthStatus: DeploymentHealth["status"] | null;
    missingEnvCount: number;
    bootstrapTaskCount: number;
    hasRepairRequest: boolean;
  };
} & DeployOperationAwareResult;

export type DeployRunResult = {
  completed: boolean;
  prepared: boolean;
  failedPhase: "prepare" | "up" | "validate" | "status" | null;
  prepare: DeployPrepareResult | null;
  up: DeployUpResult | null;
  validate: DeployValidateResult | null;
  status: DeployStatusResult | null;
  repair: DeployRepairResult | null;
  nextAction: "done" | "edit-and-rerun-up" | "execution_repair" | "request-user-approval" | "fix-docker" | "inspect-error";
  failureOwner?: DeploymentFailureOwner | null;
  repairRoute?: DeploymentRepairRoute | null;
  failureRef?: string | null;
  instruction?: Record<string, unknown>;
  error: {
    code: string;
    message: string;
    details?: unknown;
  } | null;
};

export async function deployPrepare(input: {
  projectRoot: string;
  appPath?: string;
  healthcheck?: DeploymentHealthcheckInput;
  providerPolicy?: Partial<DeploymentProviderPolicy>;
}): Promise<DeployPrepareResult> {
  return withDeploymentOperation(
    { projectRoot: input.projectRoot, command: "deploy.prepare", phase: "preparing" },
    (operation) => deployPrepareInternal(input, operation),
  );
}

async function deployPrepareInternal(input: {
  projectRoot: string;
  appPath?: string;
  healthcheck?: DeploymentHealthcheckInput;
  providerPolicy?: Partial<DeploymentProviderPolicy>;
}, operation?: DeploymentOperationHandle): Promise<DeployPrepareResult> {
  await updateDeploymentOperationPhase(input.projectRoot, operation, "preparing");
  const paths = getDeploymentPaths(input.projectRoot);
  await ensureDir(paths.specsDir);
  await ensureDir(paths.stateDir);
  await ensureDir(paths.logsDir);
  await ensureDir(paths.evidenceDir);
  await ensureDir(paths.generatedDir);

  const workspace = await resolveDeploymentWorkspaceForApp(input.projectRoot, input.appPath ?? null);
  const deploymentRoot = workspace.deploymentRoot;
  const detectedStack = workspace.detectedStack;
  const existing = await findExistingDeploymentFiles(deploymentRoot);
  const strategy = resolveDeploymentStrategy({ detectedStack, existing, policy: input.providerPolicy });
  const generatedBuildContextRoot = buildContextRootFor(input.projectRoot, deploymentRoot, detectedStack, strategy.provider);
  const workspacePackageJsonPaths = detectedStack.kind === "node"
    ? await discoverNodeWorkspacePackageJsonPaths(generatedBuildContextRoot)
    : [];
  const stackForContext = {
    ...detectedStack,
    workingDirectory: path.resolve(generatedBuildContextRoot) === path.resolve(deploymentRoot)
      ? null
      : toProjectRelative(generatedBuildContextRoot, deploymentRoot),
    workspacePackageJsonPaths,
  };
  const runtimeContract = await loadDeploymentRuntimeContract(input.projectRoot, stackForContext);
  const contractStack = applyRuntimeContractToStack(stackForContext, runtimeContract);
  const technicalBaseline = await loadDeploymentTechnicalBaseline(input.projectRoot);
  const rawCodeEvidence = await buildDeploymentCodeEvidence({
    projectRoot: input.projectRoot,
    stack: contractStack,
    technicalBaseline,
  });
  const codeEvidence = await writeDeploymentCodeEvidence(input.projectRoot, rawCodeEvidence);
  assertDeploymentCodeEvidenceReady(input.projectRoot, rawCodeEvidence, codeEvidence.ref);
  const evidenceStack = applyDeploymentCodeEvidenceToStack(contractStack, rawCodeEvidence);
  const hostPort = await findAvailablePort(evidenceStack.port);
  const workspaceMetadata = {
    ...workspace.workspace,
    buildContextPath: toProjectRelative(input.projectRoot, generatedBuildContextRoot) || ".",
  };
  const environment = await analyzeDeploymentEnvironment({
    projectRoot: input.projectRoot,
    deploymentRoot,
    stack: evidenceStack,
    generatedEnvironment: {
      ...generatedRuntimeEnvironment(evidenceStack),
      ...generatedDependencyEnvironment(evidenceStack),
    },
    contractEnvironment: runtimeContract.environment,
  });
  const bootstrap = await analyzeDeploymentBootstrap({
    deploymentRoot,
    stack: evidenceStack,
  });

  if (strategy.provider === "compose-existing" && existing.composePath) {
    const composeInfo = await analyzeExistingCompose(existing.composePath);
    const composePort = selectedComposePort(composeInfo) ?? await detectComposePublishedPort(existing.composePath);
    const reused = [
      toProjectRelative(input.projectRoot, existing.composePath),
      ...(existing.dockerfilePath ? [toProjectRelative(input.projectRoot, existing.dockerfilePath)] : []),
    ];
    const composeStack = composePort
      ? { ...evidenceStack, port: composePort.containerPort }
      : evidenceStack;
    const spec = applyHealthcheckInput(createDeploymentSpec({
      projectRoot: input.projectRoot,
      deploymentRoot,
      buildContextRoot: deploymentRoot,
      workspace: {
        ...workspaceMetadata,
        buildContextPath: toProjectRelative(input.projectRoot, deploymentRoot) || ".",
      },
      provider: strategy.provider,
      providerReason: strategy.reason,
      providerPolicy: strategy.policy,
      providerCandidates: strategy.candidates,
      detectedStack: composeStack,
      environment,
      bootstrap,
      compose: composeInfo,
      codeEvidence,
      dockerfilePath: existing.dockerfilePath ?? paths.dockerfileFile,
      composePath: existing.composePath,
      dockerignorePath: paths.dockerignoreFile,
      generated: false,
      reused,
      hostPort: composePort?.hostPort ?? hostPort,
    }), input.healthcheck);
    applyRuntimeContractHealthDefaults(spec, runtimeContract, input.healthcheck);

    spec.files.dockerfilePath = existing.dockerfilePath
      ? toProjectRelative(input.projectRoot, existing.dockerfilePath)
      : null;
    spec.files.dockerignorePath = null;
    return finalizeDeployPrepare({
      projectRoot: input.projectRoot,
      specFile: paths.specFile,
      spec,
      detectedStack: spec.detectedStack,
      logMessage: `Prepared ${spec.serviceName} deployment by reusing ${spec.files.composePath}.`,
    });
  }

  if (strategy.provider === "dockerfile-existing" && existing.dockerfilePath) {
    const reused = [toProjectRelative(input.projectRoot, existing.dockerfilePath)];
    const spec = applyHealthcheckInput(createDeploymentSpec({
      projectRoot: input.projectRoot,
      deploymentRoot,
      buildContextRoot: deploymentRoot,
      workspace: {
        ...workspaceMetadata,
        buildContextPath: toProjectRelative(input.projectRoot, deploymentRoot) || ".",
      },
      provider: strategy.provider,
      providerReason: strategy.reason,
      providerPolicy: strategy.policy,
      providerCandidates: strategy.candidates,
      detectedStack: evidenceStack,
      environment,
      bootstrap,
      codeEvidence,
      dockerfilePath: existing.dockerfilePath,
      composePath: paths.composeFile,
      dockerignorePath: paths.dockerignoreFile,
      generated: true,
      reused,
      hostPort,
    }), input.healthcheck);
    applyRuntimeContractHealthDefaults(spec, runtimeContract, input.healthcheck);

    await fs.writeFile(paths.composeFile, generateComposeForDockerfile(spec), "utf8");
    return finalizeDeployPrepare({
      projectRoot: input.projectRoot,
      specFile: paths.specFile,
      spec,
      detectedStack: spec.detectedStack,
      logMessage: `Prepared ${spec.serviceName} deployment by reusing ${spec.files.dockerfilePath}.`,
    });
  }

  const spec = applyHealthcheckInput(createDeploymentSpec({
    projectRoot: input.projectRoot,
    deploymentRoot,
    buildContextRoot: generatedBuildContextRoot,
    workspace: workspaceMetadata,
    provider: strategy.provider,
    providerReason: strategy.reason,
    providerPolicy: strategy.policy,
    providerCandidates: strategy.candidates,
    detectedStack: evidenceStack,
    environment,
    bootstrap,
    codeEvidence,
    dockerfilePath: paths.dockerfileFile,
    composePath: paths.composeFile,
    dockerignorePath: paths.dockerignoreFile,
    generated: true,
    reused: [],
    hostPort,
  }), input.healthcheck);
  applyRuntimeContractHealthDefaults(spec, runtimeContract, input.healthcheck);
  const generated = generateDeploymentFiles(spec);

  await fs.writeFile(paths.dockerfileFile, generated.dockerfile, "utf8");
  await fs.writeFile(paths.composeFile, generated.compose, "utf8");
  await fs.writeFile(paths.dockerignoreFile, generated.dockerignore, "utf8");
  return finalizeDeployPrepare({
    projectRoot: input.projectRoot,
    specFile: paths.specFile,
    spec,
    detectedStack: spec.detectedStack,
    logMessage: `Prepared ${spec.serviceName} deployment with ${spec.detectedStack.kind} stack from ${spec.workspace.appPath}.`,
  });
}

export async function deployRun(input: {
  projectRoot: string;
  appPath?: string;
  healthcheck?: DeploymentHealthcheckInput;
  providerPolicy?: Partial<DeploymentProviderPolicy>;
}): Promise<DeployRunResult> {
  return withDeploymentOperation(
    { projectRoot: input.projectRoot, command: "deploy.run", phase: "preparing" },
    (operation) => deployRunInternal(input, operation),
  );
}

async function deployRunInternal(input: {
  projectRoot: string;
  appPath?: string;
  healthcheck?: DeploymentHealthcheckInput;
  providerPolicy?: Partial<DeploymentProviderPolicy>;
}, operation: DeploymentOperationHandle): Promise<DeployRunResult> {
  const paths = getDeploymentPaths(input.projectRoot);
  let failedPhase: DeployRunResult["failedPhase"] = null;
  let prepared = false;
  let prepare: DeployPrepareResult | null = null;
  let up: DeployUpResult | null = null;
  let validate: DeployValidateResult | null = null;
  let status: DeployStatusResult | null = null;

  try {
    if (
      !(await pathExists(paths.specFile)) ||
      await preparedAppDiffers(input.projectRoot, input.appPath) ||
      await preparedRuntimeContractDiffers(input.projectRoot)
    ) {
      failedPhase = "prepare";
      await updateDeploymentOperationPhase(input.projectRoot, operation, "preparing");
      prepare = await deployPrepareInternal({
        projectRoot: input.projectRoot,
        appPath: input.appPath,
        healthcheck: input.healthcheck,
        providerPolicy: input.providerPolicy,
      }, operation);
      prepared = true;
    }

    failedPhase = "up";
    up = await deployUpInternal({
      projectRoot: input.projectRoot,
      appPath: input.appPath,
      healthcheck: input.healthcheck,
      providerPolicy: input.providerPolicy,
    }, operation);

    failedPhase = "validate";
    await updateDeploymentOperationPhase(input.projectRoot, operation, "validating");
    validate = await deployValidate({ projectRoot: input.projectRoot });
    if (!validate.valid) {
      return deployRunFailureResult({
        projectRoot: input.projectRoot,
        prepared,
        failedPhase,
        prepare,
        up,
        validate,
        status,
        error: {
          code: "DEPLOY_VALIDATION_FAILED",
          message: "Deployment validation failed.",
          details: validate,
        },
      });
    }

    failedPhase = "status";
    await updateDeploymentOperationPhase(input.projectRoot, operation, "checking_status");
    status = await deployStatus({ projectRoot: input.projectRoot });
    if (!status.running) {
      return deployRunFailureResult({
        projectRoot: input.projectRoot,
        prepared,
        failedPhase,
        prepare,
        up,
        validate,
        status,
        error: {
          code: "DEPLOY_NOT_RUNNING",
          message: "Deployment completed but no running container was found.",
          details: status,
        },
      });
    }

    return {
      completed: true,
      prepared,
      failedPhase: null,
      prepare,
      up,
      validate,
      status,
      repair: null,
      nextAction: "done",
      instruction: deploySuccessInstruction("deploy.run", status.url ?? up.url),
      error: null,
    };
  } catch (error) {
    return deployRunFailureResult({
      projectRoot: input.projectRoot,
      prepared,
      failedPhase,
      prepare,
      up,
      validate,
      status,
      error: serializeDeployRunError(error),
      nextActionError: error,
    });
  }
}

async function deployRunFailureResult(input: {
  projectRoot: string;
  prepared: boolean;
  failedPhase: DeployRunResult["failedPhase"];
  prepare: DeployPrepareResult | null;
  up: DeployUpResult | null;
  validate: DeployValidateResult | null;
  status: DeployStatusResult | null;
  error: NonNullable<DeployRunResult["error"]>;
  nextActionError?: unknown;
}): Promise<DeployRunResult> {
  const repair = await safeDeployRepair(input.projectRoot);
  return {
    completed: false,
    prepared: input.prepared,
    failedPhase: input.failedPhase,
    prepare: input.prepare,
    up: input.up,
    validate: input.validate,
    status: input.status,
    repair,
    nextAction: deployRunNextAction(input.nextActionError ?? null, repair),
    failureOwner: repair?.failureOwner ?? null,
    repairRoute: repair?.repairRoute ?? null,
    failureRef: repair?.failureRef ?? null,
    instruction: deployRunInstruction(repair),
    error: input.error,
  };
}

export async function deployUp(input: {
  projectRoot: string;
  appPath?: string;
  healthcheck?: DeploymentHealthcheckInput;
  providerPolicy?: Partial<DeploymentProviderPolicy>;
}): Promise<DeployUpResult> {
  return withDeploymentOperation(
    { projectRoot: input.projectRoot, command: "deploy.up", phase: "building" },
    (operation) => deployUpInternal(input, operation),
  );
}

async function deployUpInternal(input: {
  projectRoot: string;
  appPath?: string;
  healthcheck?: DeploymentHealthcheckInput;
  providerPolicy?: Partial<DeploymentProviderPolicy>;
}, operation: DeploymentOperationHandle): Promise<DeployUpResult> {
  const paths = getDeploymentPaths(input.projectRoot);
  const spec = await ensurePrepared(input.projectRoot, input.appPath, input.healthcheck, input.providerPolicy, operation);
  const composePath = resolveComposePath(input.projectRoot, spec);

  await updateDeploymentOperationPhase(input.projectRoot, operation, "building");
  const configResult = await dockerCompose(input.projectRoot, composePath, ["config", "--quiet"], 30_000);
  await appendCommandLog(input.projectRoot, "docker compose config", configResult);
  if (configResult.exitCode !== 0) {
    const diagnostics = diagnoseDeploymentFailure({ spec, stdout: configResult.stdout, stderr: configResult.stderr });
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec,
      failureKind: "compose_config",
      command: composeCommand(spec, ["config", "--quiet"]),
      exitCode: configResult.exitCode,
      stdout: configResult.stdout,
      stderr: configResult.stderr,
      diagnostics,
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw deployValidationFailed(
      "Docker Compose configuration failed.",
      deploymentFailureDetails(spec, configResult),
    );
  }

  try {
    await ensureDockerAvailable(input.projectRoot);
  } catch (error) {
    const details = error instanceof LoomError ? error.details : error;
    const stderr = error instanceof Error ? error.message : "Docker is unavailable.";
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec,
      failureKind: "docker_unavailable",
      command: ["docker", "version", "--format", "{{.Server.Version}}"],
      exitCode: 1,
      stdout: "",
      stderr,
      diagnostics: diagnoseDeploymentFailure({
        spec,
        stdout: "",
        stderr: `${stderr}${details ? `\n${JSON.stringify(details)}` : ""}`,
      }),
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw error;
  }
  const upResult = await dockerCompose(
    input.projectRoot,
    composePath,
    ["up", "-d", "--build"],
    DEFAULT_DEPLOY_BUILD_TIMEOUT_MS,
  );
  await appendCommandLog(input.projectRoot, "docker compose up", upResult);
  if (upResult.exitCode !== 0) {
    const diagnostics = diagnoseDeploymentFailure({ spec, stdout: upResult.stdout, stderr: upResult.stderr });
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec,
      failureKind: classifyUpFailure(spec, upResult.stdout, upResult.stderr),
      command: composeCommand(spec, ["up", "-d", "--build"]),
      exitCode: upResult.exitCode,
      stdout: upResult.stdout,
      stderr: upResult.stderr,
      diagnostics,
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw deployValidationFailed(
      "Docker Compose failed to build or start deployment.",
      deploymentFailureDetails(spec, upResult),
    );
  }

  await updateDeploymentOperationPhase(input.projectRoot, operation, "starting");
  const container = await resolveDeploymentContainer(input.projectRoot, composePath, spec);
  const startupInspection = await inspectDeploymentStartupLogs(input.projectRoot, composePath, spec, "docker compose startup logs");
  if (!container.running) {
    const serviceNotRunning = `Compose app service ${spec.compose.selectedService ?? spec.serviceName} is not running after docker compose up.`;
    const failureKind = startupInspection.logValidation.ok
      ? "container_start"
      : classifyStartupLogFailure(spec, startupInspection.rawLogs);
    const diagnostics = diagnoseDeploymentFailure({
      spec,
      stdout: startupInspection.logsResult.stdout,
      stderr: [serviceNotRunning, startupInspection.logsResult.stderr].filter(Boolean).join("\n"),
    });
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec,
      failureKind,
      command: composeCommand(spec, startupInspection.logValidation.ok
        ? ["ps", spec.compose.selectedService ?? spec.serviceName]
        : startupInspection.logsArgs),
      exitCode: 1,
      stdout: startupInspection.logsResult.stdout,
      stderr: [serviceNotRunning, startupInspection.logsResult.stderr].filter(Boolean).join("\n"),
      diagnostics,
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw deployValidationFailed("Deployment app service is not running after Compose up.", {
      serviceName: spec.compose.selectedService ?? spec.serviceName,
      provider: spec.provider,
      diagnostics,
    });
  }

  if (!startupInspection.logValidation.ok) {
    const diagnostics = diagnoseDeploymentFailure({
      spec,
      stdout: startupInspection.logsResult.stdout,
      stderr: startupInspection.logsResult.stderr,
    });
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec,
      failureKind: classifyStartupLogFailure(spec, startupInspection.rawLogs),
      command: composeCommand(spec, startupInspection.logsArgs),
      exitCode: startupInspection.logsResult.exitCode,
      stdout: startupInspection.logsResult.stdout,
      stderr: startupInspection.logsResult.stderr,
      diagnostics,
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw deployValidationFailed("Deployment startup logs contain a fatal error.", {
      matchedPattern: startupInspection.logValidation.matchedPattern,
      lines: startupInspection.logValidation.lines.slice(-20),
      diagnostics,
    });
  }
  const health = container.running ? await checkDeploymentHealth(spec) : disabledHealth(spec.runtime.healthcheck.url);
  const preview = container.running ? await checkDeploymentPreview(spec) : disabledHealth(spec.runtime.url);
  await updateDeploymentOperationPhase(input.projectRoot, operation, "validating");
  const healthSpec = applyHealthyPath(spec, health);
  if (healthSpec !== spec) {
    await writeDeploymentSpec(input.projectRoot, healthSpec);
  }
  if (health.status === "unhealthy") {
    const healthLogInspection = await inspectDeploymentStartupLogs(input.projectRoot, composePath, healthSpec, "docker compose healthcheck failure logs");
    const failureKind = healthLogInspection.logValidation.ok
      ? "healthcheck"
      : classifyStartupLogFailure(healthSpec, healthLogInspection.rawLogs);
    const diagnostics = diagnoseDeploymentFailure({
      spec: healthSpec,
      stdout: healthLogInspection.logsResult.stdout,
      stderr: [health.error ?? `Healthcheck failed for ${health.url ?? spec.runtime.url}.`, healthLogInspection.logsResult.stderr].filter(Boolean).join("\n"),
    });
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec: healthSpec,
      failureKind,
      command: failureKind === "healthcheck"
        ? ["GET", healthSpec.runtime.healthcheck.url ?? healthSpec.runtime.url]
        : composeCommand(healthSpec, healthLogInspection.logsArgs),
      exitCode: health.statusCode ?? 1,
      stdout: healthLogInspection.logsResult.stdout,
      stderr: [health.error ?? `Healthcheck failed for ${health.url ?? spec.runtime.url}.`, healthLogInspection.logsResult.stderr].filter(Boolean).join("\n"),
      diagnostics,
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw deployValidationFailed("Deployment healthcheck failed.", {
      ...health,
      provider: healthSpec.provider,
      providerCandidates: healthSpec.providerCandidates,
      suggestedActions: [
        "Run loom deploy repair to inspect the healthcheck repair request.",
        "Let the coding agent repair the selected Dockerfile/Compose provider using the structured repair request.",
      ],
    });
  }
  if (preview.status !== "healthy") {
    const previewLogInspection = await inspectDeploymentStartupLogs(input.projectRoot, composePath, healthSpec, "docker compose preview failure logs");
    const failureKind = previewLogInspection.logValidation.ok
      ? "preview_not_verified"
      : classifyStartupLogFailure(healthSpec, previewLogInspection.rawLogs);
    const diagnostics = diagnoseDeploymentFailure({
      spec: healthSpec,
      stdout: previewLogInspection.logsResult.stdout,
      stderr: [preview.error ?? `Preview verification failed for ${preview.url ?? spec.runtime.url}.`, previewLogInspection.logsResult.stderr].filter(Boolean).join("\n"),
    });
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec: healthSpec,
      failureKind,
      command: failureKind === "preview_not_verified"
        ? ["GET", preview.url ?? healthSpec.runtime.url]
        : composeCommand(healthSpec, previewLogInspection.logsArgs),
      exitCode: preview.statusCode ?? 1,
      stdout: previewLogInspection.logsResult.stdout,
      stderr: [preview.error ?? `Preview verification failed for ${preview.url ?? spec.runtime.url}.`, previewLogInspection.logsResult.stderr].filter(Boolean).join("\n"),
      diagnostics,
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw deployValidationFailed("Deployment preview verification failed.", {
      ...preview,
      provider: healthSpec.provider,
      providerCandidates: healthSpec.providerCandidates,
      suggestedActions: [
        "Run loom deploy repair if the failure is in generated Dockerfile/Compose.",
        "If the RuntimeDeliveryContract build/start/preview model is wrong, route back to delivery repair instead of editing application code in deploy repair.",
      ],
    });
  }
  const state = createRunningState({
    projectRoot: input.projectRoot,
    spec: healthSpec,
    specPath: paths.specFile,
    containerName: container.containerName,
    containerId: container.containerId,
    running: container.running,
    health: preview,
  });
  await writeDeploymentState(input.projectRoot, state);
  await clearDeploymentFailureArtifacts(input.projectRoot);

  return {
    started: container.running,
    url: container.running ? healthSpec.runtime.url : null,
    containerId: container.containerId,
    serviceName: spec.serviceName,
    appServiceName: deploymentAppServiceName(spec),
    composePath: spec.files.composePath,
    health: preview,
    instruction: deploySuccessInstruction("deploy.up", container.running ? healthSpec.runtime.url : null),
  };
}

export async function deployStatus(input: { projectRoot: string }): Promise<DeployStatusResult> {
  const activeOperation = await readDeploymentOperationView(input.projectRoot);
  const state = await readDeploymentState(input.projectRoot);
  const spec = await readPreparedDeploymentSpec(input.projectRoot);
  const specServiceName = spec?.serviceName ?? null;
  const specAppServiceName = spec?.compose.selectedService ?? specServiceName;
  if (activeOperation) {
    return {
      running: state?.running ?? false,
      url: state?.url ?? spec?.runtime.url ?? null,
      containerId: state?.containerId ?? null,
      serviceName: specServiceName ?? state?.serviceName ?? null,
      appServiceName: specAppServiceName ?? state?.appServiceName ?? state?.serviceName ?? null,
      health: state?.health ?? null,
      operationActive: true,
      activeOperation,
    };
  }
  if (!state?.containerName) {
    return {
      running: false,
      url: null,
      containerId: null,
      serviceName: specServiceName ?? state?.serviceName ?? null,
      appServiceName: specAppServiceName ?? state?.appServiceName ?? state?.serviceName ?? null,
      health: state?.health ?? null,
    };
  }

  try {
    await ensureDockerAvailable(input.projectRoot);
    const activeSpec = spec ?? await readDeploymentSpec(input.projectRoot);
    const composePath = path.resolve(input.projectRoot, activeSpec.files.composePath);
    const expectedContainerName = containerNameFor(activeSpec);
    const inspected = await resolveDeploymentContainer(input.projectRoot, composePath, activeSpec);
    const health = inspected.running ? await checkDeploymentPreview(activeSpec) : disabledHealth(state.url);
    const healthSpec = applyHealthyPath(activeSpec, health);
    if (healthSpec !== activeSpec) {
      await writeDeploymentSpec(input.projectRoot, healthSpec);
    }
    const serviceName = activeSpec.serviceName;
    const appServiceName = activeSpec.compose.selectedService ?? activeSpec.serviceName;
    const resolvedContainerName = "containerName" in inspected && typeof inspected.containerName === "string" && inspected.containerName
      ? inspected.containerName
      : expectedContainerName;
    const updatedState = {
      ...state,
      provider: activeSpec.provider,
      serviceName,
      appServiceName,
      imageName: activeSpec.imageName,
      specPath: toProjectRelative(input.projectRoot, getDeploymentPaths(input.projectRoot).specFile),
      composePath: activeSpec.files.composePath,
      containerName: resolvedContainerName,
      containerId: inspected.containerId,
      running: inspected.running,
      url: inspected.running ? healthSpec.runtime.url : null,
      health,
      updatedAt: new Date().toISOString(),
    };
    await writeDeploymentState(input.projectRoot, updatedState);
    if (inspected.running && (health.status === "healthy" || health.status === "disabled")) {
      await clearDeploymentFailureArtifacts(input.projectRoot);
    }

    const verifiedRunning = inspected.running && (health.status === "healthy" || health.status === "disabled");
    return {
      running: inspected.running,
      url: inspected.running ? healthSpec.runtime.url : null,
      containerId: inspected.containerId,
      serviceName,
      appServiceName,
      health,
      ...(verifiedRunning ? { instruction: deploySuccessInstruction("deploy.status", healthSpec.runtime.url) } : {}),
    };
  } catch (error) {
    if (isDockerUnavailable(error)) {
      return {
        running: false,
        url: null,
        containerId: state.containerId,
        serviceName: specServiceName ?? state.serviceName,
        appServiceName: specAppServiceName ?? state.appServiceName ?? state.serviceName,
        health: state.health,
      };
    }
    throw error;
  }
}

export async function deployValidate(input: { projectRoot: string }): Promise<DeployValidateResult> {
  await assertNoActiveDeploymentOperation(input.projectRoot);
  const paths = getDeploymentPaths(input.projectRoot);
  const spec = await ensurePrepared(input.projectRoot);
  const composePath = resolveComposePath(input.projectRoot, spec);
  const configResult = await dockerCompose(input.projectRoot, composePath, ["config", "--quiet"], 30_000);
  await appendCommandLog(input.projectRoot, "docker compose validate config", configResult);

  const state = await readDeploymentState(input.projectRoot);
  const health = state?.running ? await checkDeploymentPreview(spec) : null;
  const healthSpec = health ? applyHealthyPath(spec, health) : spec;
  if (healthSpec !== spec) {
    await writeDeploymentSpec(input.projectRoot, healthSpec);
  }
  if (state && health) {
    await writeDeploymentState(input.projectRoot, {
      ...state,
      health,
      url: state.running ? healthSpec.runtime.url : state.url,
      updatedAt: new Date().toISOString(),
    });
  }

  const valid = configResult.exitCode === 0 && (!health || health.status === "healthy" || health.status === "disabled");
  const verifiedRunning = Boolean(state?.running && health && (health.status === "healthy" || health.status === "disabled"));
  if (verifiedRunning) {
    await clearDeploymentFailureArtifacts(input.projectRoot);
  }

  return {
    valid,
    specPath: toProjectRelative(input.projectRoot, paths.specFile),
    composePath: spec.files.composePath,
    config: {
      ok: configResult.exitCode === 0,
      exitCode: configResult.exitCode,
      stderrTail: splitLines(configResult.stderr).slice(-40),
    },
    health,
    ...(verifiedRunning ? { instruction: deploySuccessInstruction("deploy.validate", healthSpec.runtime.url) } : {}),
  };
}

export async function deployLogs(input: { projectRoot: string }): Promise<DeployLogsResult> {
  const paths = getDeploymentPaths(input.projectRoot);
  const activeOperation = await readDeploymentOperationView(input.projectRoot);
  if (activeOperation) {
    const lines = await readDeploymentLogTail(paths.logFile, 120);
    return {
      lines,
      truncated: lines.length >= 120,
      fullLogRef: toProjectRelative(input.projectRoot, paths.logFile),
      operationActive: true,
      activeOperation,
    };
  }

  const state = await readDeploymentState(input.projectRoot);
  if (!state?.containerName) {
    throw deployNotRunning(input.projectRoot);
  }

  const spec = await readDeploymentSpec(input.projectRoot);
  const composePath = path.resolve(input.projectRoot, state.composePath || spec.files.composePath);
  await ensureDockerAvailable(input.projectRoot);
  const logsArgs = composeLogsArgs(spec, "120");
  const result = await dockerCompose(input.projectRoot, composePath, logsArgs, 30_000);
  await appendCommandLog(input.projectRoot, "docker compose logs", result);
  if (result.exitCode !== 0) {
    const diagnostics = diagnoseDeploymentFailure({ spec, stdout: result.stdout, stderr: result.stderr });
    await writeDeploymentRepairRequest({
      projectRoot: input.projectRoot,
      spec,
      failureKind: "logs",
      command: composeCommand(spec, logsArgs),
      exitCode: result.exitCode,
      stdout: result.stdout,
      stderr: result.stderr,
      diagnostics,
      previousAttempts: await nextRepairAttempt(input.projectRoot),
    });
    throw dockerUnavailable("Docker Compose failed to read deployment logs.", deploymentFailureDetails(spec, result));
  }

  const lines = splitLines(`${result.stdout}${result.stderr ? `\n${result.stderr}` : ""}`).slice(-120);
  return {
    lines,
    truncated: lines.length >= 120,
    fullLogRef: toProjectRelative(input.projectRoot, paths.logFile),
  };
}

export async function deployDown(input: { projectRoot: string }): Promise<DeployDownResult> {
  return withDeploymentOperation(
    { projectRoot: input.projectRoot, command: "deploy.down", phase: "stopping" },
    (operation) => deployDownInternal(input, operation),
  );
}

async function deployDownInternal(input: { projectRoot: string }, operation: DeploymentOperationHandle): Promise<DeployDownResult> {
  await updateDeploymentOperationPhase(input.projectRoot, operation, "stopping");
  const state = await readDeploymentState(input.projectRoot);
  if (!state) {
    throw deployNotRunning(input.projectRoot);
  }

  const spec = await readDeploymentSpec(input.projectRoot);
  const composePath = path.resolve(input.projectRoot, state.composePath || spec.files.composePath);
  await ensureDockerAvailable(input.projectRoot);
  const result = await dockerCompose(input.projectRoot, composePath, ["down"], 60_000);
  await appendCommandLog(input.projectRoot, "docker compose down", result);
  if (result.exitCode !== 0) {
    throw dockerUnavailable("Docker Compose failed to stop deployment.", deploymentFailureDetails(spec, result));
  }

  await writeDeploymentState(input.projectRoot, {
    ...state,
    running: false,
    url: null,
    containerId: null,
    health: disabledHealth(state.url),
    updatedAt: new Date().toISOString(),
  });

  return {
    stopped: true,
    removed: true,
  };
}

export async function deployRepair(input: { projectRoot: string }): Promise<DeployRepairResult> {
  await assertNoActiveDeploymentOperation(input.projectRoot);
  const request = await readDeploymentRepairRequest(input.projectRoot);
  if (!request) {
    return emptyDeployRepairResult();
  }

  const currentStatus = await safeDeployStatus(input.projectRoot);
  if (currentStatus?.running && (currentStatus.health?.status === "healthy" || currentStatus.health?.status === "disabled")) {
    await clearDeploymentFailureArtifacts(input.projectRoot);
    return emptyDeployRepairResult();
  }

  const repairClassification = classifyDeploymentRepair({
    failureKind: request.failureKind,
    editableFileCount: request.editableFiles.length,
    failureOwner: request.failureOwner ?? null,
  });
  const failureOwner = repairClassification.failureOwner;
  const repairRoute = request.repairRoute ?? repairClassification.repairRoute;
  const result = {
    hasRepairRequest: true,
    repairId: request.repairId,
    failureKind: request.failureKind,
    provider: request.provider,
    command: request.command,
    failureOwner,
    repairRoute,
    failureRef: request.failureRef ?? null,
    fullLogRef: request.fullLogRef ?? null,
    errorWindow: request.errorWindow ?? null,
    providerCandidates: request.providerCandidates,
    environment: request.environment,
    bootstrap: request.bootstrap,
    diagnostics: request.diagnostics,
    suggestedActions: request.suggestedActions,
    editableFiles: request.editableFiles,
    protectedFiles: request.protectedFiles,
    stdoutTail: request.stdoutTail,
    stderrTail: request.stderrTail,
    instruction: request.instruction,
    maxAttempts: request.maxAttempts,
    attempts: request.attempts,
    nextAction: repairNextAction(request.failureKind, repairRoute, request.editableFiles.length, request.attempts, request.maxAttempts),
  } satisfies DeployRepairResult;
  return {
    ...result,
    repairInstruction: deployRepairInstruction(result, "deploy repair"),
  };
}

function emptyDeployRepairResult(): DeployRepairResult {
  return {
    hasRepairRequest: false,
    repairId: null,
    failureKind: null,
    provider: null,
    command: [],
    failureOwner: null,
    repairRoute: null,
    failureRef: null,
    fullLogRef: null,
    errorWindow: null,
    providerCandidates: [],
    environment: null,
    bootstrap: null,
    diagnostics: null,
    suggestedActions: [],
    editableFiles: [],
    protectedFiles: [],
    stdoutTail: [],
    stderrTail: [],
    instruction: null,
    repairInstruction: undefined,
    maxAttempts: 0,
    attempts: 0,
    nextAction: "none",
  };
}

export async function deployInspect(input: { projectRoot: string; refresh?: boolean }): Promise<DeployInspectResult> {
  const activeOperation = await readDeploymentOperationView(input.projectRoot);
  let spec: DeploymentSpec | null = null;
  try {
    spec = await readDeploymentSpec(input.projectRoot);
  } catch (error) {
    if (!(error instanceof LoomError) || error.code !== "DEPLOY_NOT_PREPARED") {
      throw error;
    }
  }

  if (!activeOperation && input.refresh && spec) {
    await safeDeployStatus(input.projectRoot);
  }
  const state = await readDeploymentState(input.projectRoot);
  const repair = activeOperation ? null : await safeDeployRepair(input.projectRoot);

  return {
    prepared: Boolean(spec),
    refreshed: Boolean(!activeOperation && input.refresh && spec),
    provider: spec?.provider ?? null,
    providerReason: spec?.providerReason ?? null,
    providerPolicy: spec?.providerPolicy ?? null,
    providerCandidates: spec?.providerCandidates ?? [],
    workspace: spec?.workspace ?? null,
    codeEvidence: spec?.codeEvidence ?? null,
    codeEvidenceReadGuide: deploymentCodeEvidenceReadGuide(spec?.codeEvidence?.ref ?? null),
    detectedStack: spec?.detectedStack ?? null,
    files: spec?.files ?? null,
    compose: spec?.compose ?? null,
    runtime: spec?.runtime ?? null,
    environment: spec
      ? {
          missing: spec.environment.missing,
          warnings: spec.environment.warnings,
          provided: spec.environment.provided,
        }
      : null,
    bootstrap: spec?.bootstrap ?? null,
    state,
    repair,
    summary: {
      appPath: spec?.workspace.appPath ?? null,
      appServiceName: spec ? deploymentAppServiceName(spec) : state?.appServiceName ?? null,
      url: state?.url ?? spec?.runtime.url ?? null,
      running: state?.running ?? false,
      healthStatus: state?.health.status ?? null,
      missingEnvCount: spec?.environment.missing.length ?? 0,
      bootstrapTaskCount: spec?.bootstrap.tasks.length ?? 0,
      hasRepairRequest: repair?.hasRepairRequest ?? false,
    },
    ...(activeOperation ? { operationActive: true, activeOperation } : {}),
  };
}

export async function deployBootstrap(input: {
  projectRoot: string;
  confirm?: boolean;
  kind?: DeploymentSpec["bootstrap"]["tasks"][number]["kind"];
}): Promise<DeployBootstrapResult> {
  if (input.confirm) {
    return withDeploymentOperation(
      { projectRoot: input.projectRoot, command: "deploy.bootstrap", phase: "bootstrapping" },
      (operation) => deployBootstrapInternal(input, operation),
    );
  }
  await assertNoActiveDeploymentOperation(input.projectRoot);
  return deployBootstrapInternal(input);
}

async function deployBootstrapInternal(input: {
  projectRoot: string;
  confirm?: boolean;
  kind?: DeploymentSpec["bootstrap"]["tasks"][number]["kind"];
}, operation?: DeploymentOperationHandle): Promise<DeployBootstrapResult> {
  await updateDeploymentOperationPhase(input.projectRoot, operation, "bootstrapping");
  const spec = await ensurePrepared(input.projectRoot, undefined, undefined, undefined, operation);
  const composePath = resolveComposePath(input.projectRoot, spec);
  const selectedTasks = input.kind
    ? spec.bootstrap.tasks.filter((task) => task.kind === input.kind)
    : spec.bootstrap.tasks;
  const appServiceName = deploymentAppServiceName(spec);

  if (selectedTasks.length === 0) {
    return {
      tasks: spec.bootstrap.tasks,
      executed: [],
      skipped: [],
      confirmed: Boolean(input.confirm),
      serviceName: appServiceName,
      composePath: spec.files.composePath,
      warnings: input.kind
        ? [`No bootstrap task of kind ${input.kind} was detected.`]
        : spec.bootstrap.warnings,
    };
  }

  if (!input.confirm) {
    return {
      tasks: spec.bootstrap.tasks,
      executed: [],
      skipped: selectedTasks.map((task) => ({
        kind: task.kind,
        command: task.command,
        serviceName: appServiceName,
        status: "skipped",
        exitCode: null,
        stdoutTail: [],
        stderrTail: [],
        reason: "Bootstrap commands are advisory and require --confirm before execution.",
      })),
      confirmed: false,
      serviceName: appServiceName,
      composePath: spec.files.composePath,
      warnings: [
        ...spec.bootstrap.warnings,
        "Run with --confirm to execute selected bootstrap commands inside the app service container.",
      ],
    };
  }

  await ensureDockerAvailable(input.projectRoot);
  const state = await readDeploymentState(input.projectRoot);
  const container = await resolveDeploymentContainer(input.projectRoot, composePath, spec);
  if (!state?.running || !container.running) {
    throw deployNotRunning(input.projectRoot);
  }

  const executed: DeploymentBootstrapRunResult[] = [];
  for (const task of selectedTasks) {
    const result = await dockerComposeExec(input.projectRoot, composePath, appServiceName, task.command, 10 * 60_000);
    await appendCommandLog(input.projectRoot, `docker compose bootstrap ${task.kind}`, result);
    executed.push({
      kind: task.kind,
      command: task.command,
      serviceName: appServiceName,
      status: result.exitCode === 0 ? "completed" : "failed",
      exitCode: result.exitCode,
      stdoutTail: splitLines(result.stdout).slice(-80),
      stderrTail: splitLines(result.stderr).slice(-80),
      reason: result.exitCode === 0
        ? "Bootstrap command completed."
        : "Bootstrap command failed; inspect stdoutTail/stderrTail before retrying.",
    });

    if (result.exitCode !== 0) {
      break;
    }
  }

  return {
    tasks: spec.bootstrap.tasks,
    executed,
    skipped: [],
    confirmed: true,
    serviceName: appServiceName,
    composePath: spec.files.composePath,
    warnings: spec.bootstrap.warnings,
  };
}

async function ensurePrepared(
  projectRoot: string,
  appPath?: string,
  healthcheck?: DeploymentHealthcheckInput,
  providerPolicy?: Partial<DeploymentProviderPolicy>,
  operation?: DeploymentOperationHandle,
): Promise<DeploymentSpec> {
  const spec = await readDeploymentSpec(projectRoot);
  const composePath = path.resolve(projectRoot, spec.files.composePath);
  const dockerfileMissing = spec.files.dockerfilePath
    ? !(await pathExists(path.resolve(projectRoot, spec.files.dockerfilePath)))
    : false;
  if (
    dockerfileMissing ||
    !(await pathExists(composePath)) ||
    await preparedAppDiffers(projectRoot, appPath) ||
    await preparedRuntimeContractDiffers(projectRoot, spec) ||
    providerPolicyChanged(spec, providerPolicy)
  ) {
    await deployPrepareInternal({ projectRoot, appPath, healthcheck, providerPolicy }, operation);
    return readDeploymentSpec(projectRoot);
  }
  const updatedSpec = applyHealthcheckInput(spec, healthcheck);
  if (updatedSpec !== spec) {
    await writeDeploymentSpec(projectRoot, updatedSpec);
  }
  return updatedSpec;
}

async function readPreparedDeploymentSpec(projectRoot: string): Promise<DeploymentSpec | null> {
  try {
    return await readDeploymentSpec(projectRoot);
  } catch {
    return null;
  }
}

function providerPolicyChanged(
  spec: DeploymentSpec,
  providerPolicy?: Partial<DeploymentProviderPolicy>,
): boolean {
  if (!providerPolicy || Object.values(providerPolicy).every((value) => value === undefined)) {
    return false;
  }
  const requested = {
    provider: providerPolicy.provider,
    reuseExisting: providerPolicy.forceGenerate ? false : providerPolicy.reuseExisting ?? true,
    forceGenerate: providerPolicy.forceGenerate ?? false,
  };
  return (
    requested.provider !== spec.providerPolicy.provider ||
    requested.reuseExisting !== spec.providerPolicy.reuseExisting ||
    requested.forceGenerate !== spec.providerPolicy.forceGenerate
  );
}

async function nextRepairAttempt(projectRoot: string): Promise<number> {
  const existing = await readDeploymentRepairRequest(projectRoot);
  return existing ? existing.attempts + 1 : 0;
}

async function preparedAppDiffers(projectRoot: string, appPath?: string): Promise<boolean> {
  if (!appPath) {
    return false;
  }
  try {
    const spec = await readDeploymentSpec(projectRoot);
    return spec.workspace.appPath !== normalizeAppPathForCompare(appPath);
  } catch {
    return true;
  }
}

async function preparedRuntimeContractDiffers(projectRoot: string, currentSpec?: DeploymentSpec): Promise<boolean> {
  try {
    const spec = currentSpec ?? await readDeploymentSpec(projectRoot);
    const workspace = await resolveDeploymentWorkspaceForApp(projectRoot, spec.workspace.appPath);
    const latestStackForContext = {
      ...workspace.detectedStack,
      workingDirectory: spec.detectedStack.workingDirectory,
      workspacePackageJsonPaths: spec.detectedStack.workspacePackageJsonPaths,
    };
    const latest = await loadDeploymentRuntimeContract(projectRoot, latestStackForContext);
    const latestDeploymentStack = applyRuntimeContractToStack(latestStackForContext, latest);
    const technicalBaseline = await loadDeploymentTechnicalBaseline(projectRoot);
    const latestEvidence = await buildDeploymentCodeEvidence({
      projectRoot,
      stack: latestDeploymentStack,
      technicalBaseline,
    });
    const latestEvidenceStack = applyDeploymentCodeEvidenceToStack(latestDeploymentStack, latestEvidence);
    return !runtimeContractsEquivalent(spec.runtimeContract, latest) ||
      deploymentStackDiffers(spec.detectedStack, latestEvidenceStack) ||
      spec.codeEvidence?.fingerprint !== latestEvidence.fingerprint;
  } catch {
    return false;
  }
}

function runtimeContractsEquivalent(
  left: DeploymentSpec["runtimeContract"],
  right: DeploymentSpec["runtimeContract"],
): boolean {
  return (
    left.source === right.source &&
    left.ref === right.ref &&
    left.status === right.status &&
    left.dependencyServicePolicy === right.dependencyServicePolicy &&
    left.runtimeKind === right.runtimeKind &&
    left.buildCommand === right.buildCommand &&
    left.startCommand === right.startCommand &&
    left.port === right.port &&
    left.previewPath === right.previewPath &&
    left.healthPath === right.healthPath &&
    left.frontendOutputDir === right.frontendOutputDir &&
    left.probeKind === right.probeKind &&
    JSON.stringify(left.apiPaths) === JSON.stringify(right.apiPaths) &&
    JSON.stringify(left.environment) === JSON.stringify(right.environment) &&
    JSON.stringify(left.dependencyServices) === JSON.stringify(right.dependencyServices)
  );
}

function deploymentStackDiffers(
  left: DeploymentSpec["detectedStack"],
  right: DeploymentSpec["detectedStack"],
): boolean {
  return (
    left.buildCommand !== right.buildCommand ||
    left.startCommand !== right.startCommand ||
    left.outputDirectory !== right.outputDirectory ||
    left.port !== right.port ||
    left.kind !== right.kind ||
    left.framework !== right.framework ||
    left.packageManager !== right.packageManager ||
    left.runtimeVersion !== right.runtimeVersion ||
    left.runtimeVersionSource !== right.runtimeVersionSource ||
    JSON.stringify(left.services) !== JSON.stringify(right.services)
  );
}

function normalizeAppPathForCompare(appPath: string): string {
  const normalized = appPath.trim().replace(/\\/g, "/").replace(/^\.\/+/, "").replace(/\/+$/, "");
  return normalized || ".";
}

async function appendCommandLog(projectRoot: string, label: string, result: { exitCode: number; stdout: string; stderr: string }): Promise<void> {
  await appendDeploymentLog(
    projectRoot,
    [
      logLine(label, `exitCode=${result.exitCode}`),
      result.stdout ? result.stdout.trimEnd() : "",
      result.stderr ? result.stderr.trimEnd() : "",
      "",
    ]
      .filter((line) => line.length > 0)
      .join("\n") + "\n",
  );
}

function logLine(scope: string, message: string): string {
  return `[${new Date().toISOString()}] ${scope}: ${message}\n`;
}

function commandDetails(result: { exitCode: number; stdout: string; stderr: string }): {
  exitCode: number;
  stdout: string;
  stderr: string;
} {
  return {
    exitCode: result.exitCode,
    stdout: result.stdout.trim(),
    stderr: result.stderr.trim(),
  };
}

function deploymentFailureDetails(
  spec: DeploymentSpec,
  result: { exitCode: number; stdout: string; stderr: string },
): unknown {
  return {
    ...commandDetails(result),
    provider: spec.provider,
    providerReason: spec.providerReason,
    providerPolicy: spec.providerPolicy,
    providerCandidates: spec.providerCandidates,
    suggestedActions: [
      "Run loom deploy repair to inspect editable files, protected files, output tails, and next repair action.",
      "Let the coding agent repair the selected Dockerfile/Compose provider using the structured repair request.",
      "If the project cannot be deployed with Dockerfile/Compose, explain the blocker clearly instead of switching builders automatically.",
    ],
  };
}

function composeCommand(spec: DeploymentSpec, args: string[]): string[] {
  return ["docker", "compose", "-f", spec.files.composePath, ...args];
}

function deploymentAppServiceName(spec: DeploymentSpec): string {
  return spec.compose.selectedService ?? spec.serviceName;
}

function composeLogsArgs(spec: DeploymentSpec, tail: string): string[] {
  return spec.provider === "compose-existing" && spec.compose.selectedService
    ? ["logs", "--tail", tail, spec.compose.selectedService]
    : ["logs", "--tail", tail];
}

async function inspectDeploymentStartupLogs(
  projectRoot: string,
  composePath: string,
  spec: DeploymentSpec,
  label: string,
) {
  const logsArgs = composeLogsArgs(spec, "120");
  const logsResult = await dockerCompose(projectRoot, composePath, logsArgs, 30_000);
  await appendCommandLog(projectRoot, label, logsResult);
  const rawLogs = `${logsResult.stdout}${logsResult.stderr ? `\n${logsResult.stderr}` : ""}`;
  return {
    logsArgs,
    logsResult,
    rawLogs,
    logValidation: validateStartupLogs(rawLogs),
  };
}

async function resolveDeploymentContainer(
  projectRoot: string,
  composePath: string,
  spec: DeploymentSpec,
): Promise<{ containerId: string | null; running: boolean; containerName: string }> {
  const serviceName = deploymentAppServiceName(spec);
  const container = await findComposeServiceContainer(projectRoot, composePath, serviceName);
  if (container.containerId) {
    return {
      containerId: container.containerId,
      running: container.running,
      containerName: container.containerName ?? serviceName,
    };
  }

  const containerName = containerNameFor(spec);
  const inspected = await inspectContainer(projectRoot, containerName);
  return {
    containerId: inspected.containerId,
    running: inspected.running,
    containerName,
  };
}

function buildContextRootFor(
  projectRoot: string,
  deploymentRoot: string,
  detectedStack: DeploymentSpec["detectedStack"],
  provider: DeployProvider,
): string {
  if (
    provider === "dockerfile-template" &&
    detectedStack.kind === "node" &&
    path.resolve(projectRoot) !== path.resolve(deploymentRoot)
  ) {
    return projectRoot;
  }
  return deploymentRoot;
}

function classifyUpFailure(
  spec: DeploymentSpec,
  stdout: string,
  stderr: string,
): "registry_network" | "image_build" | "container_start" | "build_command_failed" | "unknown" {
  const combined = `${stdout}\n${stderr}`.toLowerCase();
  if (
    combined.includes("failed to fetch oauth token") ||
    combined.includes("failed to authorize") ||
    combined.includes("deadlineexceeded") ||
    combined.includes("i/o timeout") ||
    combined.includes("tls handshake timeout") ||
    combined.includes("temporary failure in name resolution") ||
    combined.includes("no such host") ||
    combined.includes("network is unreachable")
  ) {
    return "registry_network";
  }
  if (
    spec.runtimeContract.source !== "heuristic" &&
    spec.runtimeContract.buildCommand &&
    (
      combined.includes(`run ${spec.runtimeContract.buildCommand.toLowerCase()}`) ||
      combined.includes(spec.runtimeContract.buildCommand.toLowerCase())
    ) &&
    (
      /error ts\d{4}/i.test(combined) ||
      combined.includes("typescript") ||
      combined.includes("tsc -p") ||
      combined.includes("vite build") ||
      combined.includes("failed to compile") ||
      combined.includes("compilation failed")
    )
  ) {
    return "build_command_failed";
  }
  if (
    combined.includes("failed to solve") ||
    combined.includes("dockerfile") ||
    combined.includes("npm err!") ||
    combined.includes("build failed") ||
    combined.includes("executor failed") ||
    (combined.includes("command timed out") &&
      (combined.includes("load metadata") || combined.includes("building")))
  ) {
    return "image_build";
  }
  if (
    combined.includes("container") ||
    combined.includes("port is already allocated") ||
    combined.includes("exited") ||
    combined.includes("health")
  ) {
    return "container_start";
  }
  return "unknown";
}

function classifyStartupLogFailure(
  spec: DeploymentSpec,
  rawLogs: string,
): DeploymentFailureKind {
  const combined = rawLogs.toLowerCase();
  const startCommand = spec.runtimeContract.startCommand ?? spec.detectedStack.startCommand;
  const scriptName = startCommand ? packageScriptNameFromCommand(startCommand) : null;
  const missingScript = combined.match(/missing script:\s*["']?([a-z0-9:_-]+)/i)?.[1]?.toLowerCase() ?? null;
  if (
    startCommand &&
    (
      (scriptName && missingScript === scriptName.toLowerCase()) ||
      combined.includes("npm error missing script") ||
      combined.includes("npm err! missing script") ||
      combined.includes(`missing script: "${scriptName ?? ""}"`)
    )
  ) {
    return "start_command_failed";
  }
  if (isApplicationStartupFailure(combined)) {
    return "application_startup_failed";
  }
  return "container_start";
}

function isApplicationStartupFailure(logs: string): boolean {
  return /application failed to start|beancreationexception|unsatisfieddependencyexception|applicationcontextexception|webserverexception|flywayexception|liquibaseexception|hibernateexception|schemamanagementexception|psqlexception|communications link failure|unable to obtain jdbc connection|prisma.*p20\d{2}|django\.db\.utils\.|improperlyconfigured|active(record|model)::|illuminate\\database|sqlstate\[/.test(logs);
}

function packageScriptNameFromCommand(command: string): string | null {
  const match = command.match(/\b(?:npm|pnpm|bun)\s+(?:run\s+)?([a-zA-Z0-9:_-]+)/) ??
    command.match(/\byarn\s+([a-zA-Z0-9:_-]+)/);
  if (!match?.[1]) {
    return null;
  }
  const script = match[1];
  return ["--", "run"].includes(script) ? null : script;
}

function disabledHealth(url: string | null): DeploymentHealth {
  return {
    status: "disabled",
    url,
    checkedAt: new Date().toISOString(),
    statusCode: null,
    error: null,
  };
}

function splitLines(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0);
}

async function readDeploymentLogTail(logFile: string, limit: number): Promise<string[]> {
  if (!(await pathExists(logFile))) {
    return [];
  }
  const content = await fs.readFile(logFile, "utf8");
  return splitLines(content).slice(-limit);
}

function toPrepareResult(
  projectRoot: string,
  specFile: string,
  spec: DeploymentSpec,
  detectedStack: DeploymentSpec["detectedStack"],
): DeployPrepareResult {
  return {
    prepared: true,
    specPath: toProjectRelative(projectRoot, specFile),
    detectedStack,
    provider: spec.provider,
    providerReason: spec.providerReason,
    providerPolicy: spec.providerPolicy,
    providerCandidates: spec.providerCandidates,
    workspace: spec.workspace,
    codeEvidence: spec.codeEvidence ?? null,
    environment: spec.environment,
    bootstrap: spec.bootstrap,
    compose: spec.compose,
    files: spec.files,
    url: spec.runtime.url,
    reused: spec.files.reused,
  };
}

async function finalizeDeployPrepare(input: {
  projectRoot: string;
  specFile: string;
  spec: DeploymentSpec;
  detectedStack: DeploymentSpec["detectedStack"];
  logMessage: string;
}): Promise<DeployPrepareResult> {
  await writeDeploymentSpec(input.projectRoot, input.spec);
  await clearDeploymentFailureArtifacts(input.projectRoot);
  await appendDeploymentLog(input.projectRoot, logLine("prepare", input.logMessage));
  return toPrepareResult(input.projectRoot, input.specFile, input.spec, input.detectedStack);
}

function assertDeploymentCodeEvidenceReady(
  projectRoot: string,
  evidence: DeploymentCodeEvidence,
  evidenceRef: string,
): void {
  const retryCommand = {
    name: "deploy prepare",
    argv: ["deploy", "prepare"],
    autoContinue: false,
    mustRunImmediately: false,
    usage: "Retry only after the missing facts or conflicts named in this blocker have been resolved.",
  };
  if (evidence.conflicts.length > 0) {
    throw deployConflict("Deployment prepare found conflicting technology and repository evidence.", {
      status: "blocked",
      code: "DEPLOY_CONFLICT",
      evidenceRef,
      conflicts: evidence.conflicts,
      warnings: evidence.warnings,
      nextAction: "ask_user",
      retryCommand,
      agentFacingBlocker: deploySourceBlockerProtocol({
        code: "DEPLOY_CONFLICT",
        evidenceRef,
        reason: "TechnicalBaseline, repository evidence, or existing deployment assets conflict.",
      }),
      projectRoot,
    });
  }
  if (evidence.missingFacts.length > 0) {
    const nextAction = evidence.missingFacts.some((fact) => fact.resolution === "ask_user")
      ? "ask_user"
      : "execution_repair";
    throw deploySourceInsufficient("Deployment prepare could not derive a complete deployment source model.", {
      status: "blocked",
      code: "DEPLOY_SOURCE_INSUFFICIENT",
      evidenceRef,
      missingFacts: evidence.missingFacts,
      ambiguous: evidence.dependencyFacts.ambiguous,
      warnings: evidence.warnings,
      nextAction,
      retryCommand,
      agentFacingBlocker: deploySourceBlockerProtocol({
        code: "DEPLOY_SOURCE_INSUFFICIENT",
        evidenceRef,
        reason: "Deploy prepare cannot derive enough source facts to safely generate deployment assets.",
      }),
      projectRoot,
    });
  }
}

function deploySourceBlockerProtocol(input: {
  code: "DEPLOY_CONFLICT" | "DEPLOY_SOURCE_INSUFFICIENT";
  evidenceRef: string;
  reason: string;
}): Record<string, unknown> {
  return {
    mode: "report_blocker",
    source: "deploy.prepare",
    code: input.code,
    evidenceRef: input.evidenceRef,
    reason: input.reason,
    allowedActions: [
      "report the blocker and its missing facts or conflicts",
      "read the referenced deploy evidence only when more detail is needed",
      "ask the user for the requested decision when nextAction is ask_user",
    ],
    evidenceReadGuide: deploymentCodeEvidenceReadGuide(input.evidenceRef),
    forbiddenActions: [
      "do not retry deploy prepare unchanged",
      "do not run deploy repair for this blocker",
      "do not generate or edit Dockerfile or Compose files from memory",
      "do not run raw Docker or Docker Compose commands",
    ],
    retryPolicy: {
      autoRetry: false,
      retryOnlyAfter: "the missing facts, ambiguity, or conflicts in this blocker have been resolved",
    },
  };
}

function deploymentCodeEvidenceReadGuide(evidenceRef: string | null): Record<string, unknown> | null {
  if (!evidenceRef) {
    return null;
  }
  return {
    ref: evidenceRef,
    purpose: "Read only targeted deploy code evidence fields needed to explain or resolve a blocker; do not print the full evidence JSON into chat.",
    targetedFields: [
      {
        field: "missingFacts",
        useWhen: "DEPLOY_SOURCE_INSUFFICIENT or missing deployment source facts",
      },
      {
        field: "conflicts",
        useWhen: "DEPLOY_CONFLICT or conflicting baseline/code/deploy asset facts",
      },
      {
        field: "runtimeFacts",
        useWhen: "build/start/runtime entrypoint detection is unclear",
      },
      {
        field: "dependencyFacts.services",
        useWhen: "dependency service detection does not match project runtime needs",
      },
      {
        field: "dependencyFacts.ambiguous",
        useWhen: "dependency service candidates are ambiguous",
      },
      {
        field: "baselineExpectation",
        useWhen: "TechnicalBaseline expectations need to be compared with code evidence",
      },
      {
        field: "existingDeployAssets",
        useWhen: "existing Dockerfile or Compose assets may affect deploy behavior",
      },
      {
        field: "warnings",
        useWhen: "deploy prepared but reported code evidence warnings",
      },
    ],
    readPolicy: {
      default: "Start with codeEvidence summary from deploy inspect. Read this evidence ref only for the listed targeted fields.",
      avoid: [
        "do not read or paste the full evidence JSON unless targeted fields are insufficient",
        "do not discover evidence by scanning unrelated .loom directories",
      ],
    },
  };
}

function applyRuntimeContractHealthDefaults(
  spec: DeploymentSpec,
  runtimeContract: DeploymentSpec["runtimeContract"],
  healthcheck?: DeploymentHealthcheckInput,
): void {
  spec.runtimeContract = runtimeContract;
  if (healthcheck?.path !== undefined || healthcheck?.enabled === false) {
    return;
  }
  spec.runtime.healthcheck.path = runtimeContract.healthPath ?? runtimeContract.previewPath;
  spec.runtime.healthcheck.url = spec.runtime.healthcheck.enabled
    ? `${spec.runtime.url.replace(/\/+$/, "")}${spec.runtime.healthcheck.path}`
    : null;
}

function isDockerUnavailable(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "DOCKER_UNAVAILABLE";
}

function repairNextAction(
  failureKind: string | null,
  repairRoute: DeploymentRepairRoute | null,
  editableFileCount: number,
  attempts: number,
  maxAttempts: number,
): DeployRepairResult["nextAction"] {
  if (repairRoute === "execution_repair") {
    return "execution-repair";
  }
  if (failureKind === "registry_network" || failureKind === "docker_unavailable") {
    return "none";
  }
  if (editableFileCount === 0 || attempts >= maxAttempts) {
    return "request-user-approval";
  }
  return "edit-and-rerun";
}

async function safeDeployRepair(projectRoot: string): Promise<DeployRepairResult | null> {
  try {
    return await deployRepair({ projectRoot });
  } catch {
    return null;
  }
}

async function safeDeployStatus(projectRoot: string): Promise<DeployStatusResult | null> {
  try {
    return await deployStatus({ projectRoot });
  } catch {
    return null;
  }
}

function deployRunNextAction(
  error: unknown,
  repair: DeployRepairResult | null,
): DeployRunResult["nextAction"] {
  if (repair?.nextAction === "execution-repair") {
    return "execution_repair";
  }
  if (repair?.nextAction === "edit-and-rerun") {
    return "edit-and-rerun-up";
  }
  if (repair?.nextAction === "request-user-approval") {
    return "request-user-approval";
  }
  if (
    error instanceof LoomError &&
    (
      (error.code === "DOCKER_UNAVAILABLE" && (!repair || repair.failureKind === "docker_unavailable")) ||
      repair?.failureKind === "registry_network"
    )
  ) {
    return "fix-docker";
  }
  return "inspect-error";
}

function deployRunInstruction(repair: DeployRepairResult | null): Record<string, unknown> | undefined {
  if (!repair) {
    return undefined;
  }
  if (repair.nextAction === "edit-and-rerun") {
    return deployRepairInstruction(repair, "deploy run");
  }
  if (repair.repairRoute !== "execution_repair" || !repair.failureRef) {
    return undefined;
  }
  return withAutoRunnableTransition({
    mode: "run_cli",
    command: {
      name: "repair request",
      argv: [
        "repair",
        "request",
        "--type",
        "execution",
        "--source",
        "deploy",
        "--failure-ref",
        repair.failureRef,
      ],
    },
    failureKind: repair.failureKind,
    failureOwner: repair.failureOwner,
    repairRoute: repair.repairRoute,
    failureRef: repair.failureRef,
    routingRule: "Deploy classified this as an application_code failure. Create deploy-sourced execution repair now. Do not run deploy repair and do not edit Dockerfile/Compose from deploy context.",
    userMessage: "Deploy found an application code/runtime delivery failure. Create execution repair from the deployment failure report now.",
  }, {
    sourceCommand: "deploy run",
    sourceSummary: "Deploy classified the failure as application code/runtime delivery repairable through execution repair.",
    primaryAction: "create_deploy_sourced_execution_repair",
  });
}

function deployRepairInstruction(
  repair: DeployRepairResult,
  sourceCommand: string,
): Record<string, unknown> | undefined {
  if (repair.nextAction !== "edit-and-rerun" || repair.editableFiles.length === 0) {
    return undefined;
  }
  const retryCommand = {
    name: "deploy up",
    argv: ["deploy", "up"],
  };
  return withAutoRunnableTransition({
    mode: "deploy_repair_assets",
    repairId: repair.repairId,
    failureKind: repair.failureKind,
    failureOwner: repair.failureOwner,
    repairRoute: repair.repairRoute,
    provider: repair.provider,
    fullLogRef: repair.fullLogRef,
    errorWindow: repair.errorWindow,
    diagnostics: repair.diagnostics,
    suggestedActions: repair.suggestedActions,
    editableFiles: repair.editableFiles,
    protectedFiles: repair.protectedFiles,
    attempts: repair.attempts,
    maxAttempts: repair.maxAttempts,
    repairBoundary: {
      allowedEdits: repair.editableFiles,
      forbiddenEdits: [
        "application source files",
        "package scripts",
        "tests",
        "RuntimeDeliveryContract",
        ".loom delivery artifacts outside editableFiles",
        ...repair.protectedFiles,
      ],
      rule: "Repair only the returned editableFiles. Do not edit application code or package scripts from deploy asset repair.",
    },
    repairInputs: {
      errorWindow: repair.errorWindow,
      fullLogRef: repair.fullLogRef,
      diagnostics: repair.diagnostics,
      providerCandidates: repair.providerCandidates,
      environment: repair.environment,
      bootstrap: repair.bootstrap,
    },
    retryCommand,
    completionBarrier: {
      followUpCommand: retryCommand,
    },
    expectedResponse: {
      kind: "deploy_up_envelope",
      successRule: "If deploy up returns a verified running deployment, report the URL and stop.",
      retryRule: "If deploy up fails again, run deploy repair immediately and follow its returned instruction until attempts are exhausted or a user-gated decision is returned.",
    },
    routingRule: "This deployment failure is repairable inside generated deployment assets. Edit only editableFiles, then run retryCommand. Do not ask the user whether to continue and do not route into normal delivery planning.",
    userMessage: "Deploy produced a bounded deployment-asset repair request. Repair the listed deployment files now, then rerun deploy up.",
  }, {
    sourceCommand,
    sourceSucceeded: false,
    sourceSummary: "Deploy stopped with a deployment-asset failure that is auto-runnable through bounded deploy repair.",
    primaryAction: "repair_deployment_assets_and_rerun_deploy_up",
    requiredSteps: [
      "read instruction.errorWindow first",
      "read instruction.fullLogRef only if errorWindow and diagnostics are insufficient",
      "inspect only the returned editableFiles",
      "edit only instruction.editableFiles to fix the deploy asset failure",
      "run instruction.retryCommand through commandInvocation after edits",
      "read the returned deploy up envelope",
      "if deploy up fails again and attempts remain, run loom deploy repair and follow the returned instruction immediately",
    ],
    forbiddenStops: [
      "do not stop after describing the deploy failure",
      "do not ask whether to repair returned editableFiles",
      "do not tell the user to run a next step while this bounded repair is auto-runnable",
      "do not edit application code, package scripts, tests, or RuntimeDeliveryContract from deploy asset repair",
      "do not run normal loom continue, plan, next-task, or review from deploy asset repair",
    ],
    stopOnlyWhen: [
      "deploy up succeeds and returns a verified running deployment",
      "deploy repair returns request-user-approval, fix-docker, manual_review, report_blocked, or a non-repairable failure",
      "repair attempts are exhausted",
    ],
    completionCondition: "editableFiles have been repaired and retryCommand deploy up has been run; if deploy up fails again, the returned deploy repair instruction has been followed.",
    userVisibleSummary: "Deployment repair is auto-runnable: edit only the returned deployment files, rerun deploy up, and continue repair if it fails again.",
  });
}

function deploySuccessInstruction(commandName: string, url: string | null): Record<string, unknown> {
  return {
    mode: "report_done",
    autoContinue: false,
    nextAction: {
      type: "done",
      reason: "DEPLOYMENT_RUNNING_AND_VERIFIED",
      targetNode: "deploy",
      refs: {
        url,
      },
    },
    routingRule: "Deployment is already managed by loom. Do not run raw docker compose, docker build, docker run, or manually recreate loom containers after this successful deploy command.",
    userMessage: url
      ? `Deployment is running at ${url}. Report this result; do not run raw Docker Compose for extra verification.`
      : "Deployment command succeeded. Report this result; do not run raw Docker Compose for extra verification.",
    advisories: [
      {
        code: "DEPLOY_INTERNAL_DOCKER_COMMANDS",
        severity: "info",
        message: "Use loom deploy status, inspect, validate, logs, or down for follow-up checks. Raw docker compose is an internal implementation detail unless a deploy repair request explicitly allows it.",
        sourceCommand: commandName,
      },
    ],
  };
}

function serializeDeployRunError(error: unknown): NonNullable<DeployRunResult["error"]> {
  if (error instanceof LoomError) {
    const details = sanitizeDeployRunDetails(error.details);
    return {
      code: error.code,
      message: error.message,
      ...(details === undefined ? {} : { details }),
    };
  }

  if (error instanceof Error) {
    return {
      code: "INTERNAL_ERROR",
      message: error.message,
      details: { name: error.name },
    };
  }

  return {
    code: "INTERNAL_ERROR",
    message: "Unexpected internal error.",
    details: { type: typeof error },
  };
}

function sanitizeDeployRunDetails(details: unknown): unknown {
  if (details === undefined) {
    return undefined;
  }

  try {
    return JSON.parse(JSON.stringify(details));
  } catch {
    return { type: typeof details };
  }
}
