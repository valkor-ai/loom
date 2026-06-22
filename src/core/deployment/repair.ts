import { createHash } from "node:crypto";
import { getDeploymentPaths } from "./paths";
import { writeJsonAtomic } from "../state/fs";
import { loadDeliveryIndex, loadProjectStatus } from "../state/delivery";
import { toProjectRelative } from "../state/paths";
import { DEFAULT_DEPLOY_REPAIR_MAX_ATTEMPTS } from "./constants";
import { diagnosticActions } from "./diagnostics";
import { writeDeploymentFailureReport } from "./state";
import type {
  DeploymentErrorWindow,
  DeploymentFailureOwner,
  DeploymentFailureReport,
  DeploymentFailureDiagnostic,
  DeploymentFailureKind,
  DeploymentRepairRequest,
  DeploymentRepairRoute,
  DeploymentSpec,
} from "./types";

const ERROR_WINDOW_MAX_LINES = 80;
const ERROR_WINDOW_FALLBACK_LINES = 40;
const ERROR_WINDOW_CONTEXT_BEFORE = 3;
const ERROR_WINDOW_CONTEXT_AFTER = 6;

const errorWindowSignals: Array<{ code: string; pattern: RegExp }> = [
  { code: "error", pattern: /\berror\b|err!/i },
  { code: "failed", pattern: /\bfailed\b|\bfailure\b|exit code|exited with code/i },
  { code: "fatal", pattern: /\bfatal\b|panic|exception|traceback/i },
  { code: "missing_module", pattern: /cannot find module|module_not_found|modulenotfounderror|no module named/i },
  { code: "missing_file", pattern: /no such file|not found|enoent/i },
  { code: "permission", pattern: /permission denied|eacces|operation not permitted/i },
  { code: "network", pattern: /timeout|timed out|network is unreachable|temporary failure|no such host/i },
  { code: "port", pattern: /eaddrinuse|address already in use|port is already allocated/i },
  { code: "database", pattern: /database|relation .* does not exist|table .* does not exist|migration/i },
  { code: "native_optional_dependency", pattern: /lightningcss|sharp|esbuild|rollup|swc|oxide|linux-(arm64|x64)|(gnu|musl)\.node/i },
];

export async function writeDeploymentRepairRequest(input: {
  projectRoot: string;
  spec: DeploymentSpec;
  failureKind: DeploymentFailureKind;
  command: string[];
  exitCode: number;
  stdout: string;
  stderr: string;
  diagnostics?: DeploymentFailureDiagnostic[];
  previousAttempts?: number;
}): Promise<DeploymentRepairRequest> {
  const paths = getDeploymentPaths(input.projectRoot);
  const editableFileCount = editableFilesFor(input.spec, input.failureKind).length;
  const { failureOwner, repairRoute } = classifyDeploymentRepair({
    failureKind: input.failureKind,
    editableFileCount,
  });
  const failureRef = repairRoute === "execution_repair"
    ? toProjectRelative(input.projectRoot, paths.failureFile)
    : null;
  const request = createDeploymentRepairRequest({
    ...input,
    failureOwner,
    repairRoute,
    failureRef,
  });
  await writeJsonAtomic(paths.repairFile, request);
  if (repairRoute === "execution_repair") {
    await writeDeploymentFailureReport(input.projectRoot, await createDeploymentFailureReport({
      ...input,
      failureOwner,
      repairRoute,
      failureRef,
    }));
  }
  return request;
}

function createDeploymentRepairRequest(input: {
  projectRoot: string;
  spec: DeploymentSpec;
  failureKind: DeploymentFailureKind;
  command: string[];
  exitCode: number;
  stdout: string;
  stderr: string;
  diagnostics?: DeploymentFailureDiagnostic[];
  previousAttempts?: number;
  failureOwner: DeploymentFailureOwner;
  repairRoute: DeploymentRepairRoute;
  failureRef: string | null;
}): DeploymentRepairRequest {
  const paths = getDeploymentPaths(input.projectRoot);
  const diagnostics = input.diagnostics ?? [];
  const fullLogRef = toProjectRelative(input.projectRoot, paths.logFile);
  const errorWindow = createErrorWindow(input.stdout, input.stderr, diagnostics);
  return {
    schemaVersion: 1,
    repairId: `deploy-repair-${Date.now()}`,
    createdAt: new Date().toISOString(),
    projectRoot: input.projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    provider: input.spec.provider,
    failureKind: input.failureKind,
    failureOwner: input.failureOwner,
    repairRoute: input.repairRoute,
    failureRef: input.failureRef,
    command: input.command,
    exitCode: input.exitCode,
    fullLogRef,
    errorWindow,
    stdoutTail: tailLines(input.stdout),
    stderrTail: tailLines(input.stderr),
    providerCandidates: input.spec.providerCandidates,
    environment: input.spec.environment,
    bootstrap: input.spec.bootstrap,
    diagnostics,
    suggestedActions: suggestedActionsFor(input.spec, input.failureKind, diagnostics),
    editableFiles: editableFilesFor(input.spec, input.failureKind),
    protectedFiles: protectedFilesFor(input.spec),
    instruction: instructionFor(input.spec, input.failureKind),
    maxAttempts: DEFAULT_DEPLOY_REPAIR_MAX_ATTEMPTS,
    attempts: input.previousAttempts ?? 0,
    status: "pending",
  };
}

async function createDeploymentFailureReport(input: {
  projectRoot: string;
  spec: DeploymentSpec;
  failureKind: DeploymentFailureKind;
  command: string[];
  exitCode: number;
  stdout: string;
  stderr: string;
  diagnostics?: DeploymentFailureDiagnostic[];
  previousAttempts?: number;
  failureOwner: DeploymentFailureOwner;
  repairRoute: DeploymentRepairRoute;
  failureRef: string | null;
}): Promise<DeploymentFailureReport> {
  const paths = getDeploymentPaths(input.projectRoot);
  const stdoutTail = tailLines(input.stdout);
  const stderrTail = tailLines(input.stderr);
  const fullLogRef = toProjectRelative(input.projectRoot, paths.logFile);
  const errorWindow = createErrorWindow(input.stdout, input.stderr, input.diagnostics ?? []);
  const failedContract = failedContractFor(input.spec, input.failureKind);
  const signature = failureSignature(input.failureKind, failedContract.field, errorWindow, stdoutTail, stderrTail);
  return {
    schemaVersion: "1.0",
    failureId: `deploy-failure-${Date.now()}`,
    source: "deploy",
    createdAt: new Date().toISOString(),
    deploymentAttemptId: `deploy-attempt-${Date.now()}`,
    failureKind: input.failureKind,
    failureOwner: input.failureOwner,
    repairRoute: input.repairRoute,
    runtimeDeliveryRef: input.spec.runtimeContract.ref,
    sourceRefs: await deploymentSourceRefs(input.projectRoot, input.spec),
    failedContract,
    evidence: {
      failedAt: failedAtFor(input.failureKind),
      deployCommand: input.command,
      exitCode: input.exitCode,
      fullLogRef,
      errorWindow,
      stdoutTail,
      stderrTail,
      logMarkers: logMarkersFor(input.failureKind),
      diagnostics: input.diagnostics ?? [],
    },
    routing: {
      editableBoundary: "application_code_only",
      mustNotEdit: [
        ".loom",
        ...(input.spec.files.dockerfilePath ? [input.spec.files.dockerfilePath] : []),
        input.spec.files.composePath,
        ...(input.spec.files.dockerignorePath ? [input.spec.files.dockerignorePath] : []),
      ],
      nextCommand: {
        name: "repair request",
        argv: [
          "repair",
          "request",
          "--type",
          "execution",
          "--source",
          "deploy",
          "--failure-ref",
          toProjectRelative(input.projectRoot, paths.failureFile),
        ],
      },
    },
    loopGuard: {
      signature,
      attempt: Math.max(1, (input.previousAttempts ?? 0) + 1),
      maxAttempts: 2,
    },
  };
}

async function deploymentSourceRefs(
  projectRoot: string,
  spec: DeploymentSpec,
): Promise<DeploymentFailureReport["sourceRefs"]> {
  const paths = getDeploymentPaths(projectRoot);
  const refs: DeploymentFailureReport["sourceRefs"] = {
    runtimeDeliveryRef: spec.runtimeContract.ref,
    taskPlanRef: null,
    taskPlanRunRef: null,
    reviewResultRef: null,
    deploymentSpecRef: toProjectRelative(projectRoot, paths.specFile),
  };
  try {
    const status = await loadProjectStatus(projectRoot);
    const deliveryId = status.activeDeliveryId ?? status.lastCompletedDeliveryId;
    if (!deliveryId) {
      return refs;
    }
    const delivery = await loadDeliveryIndex(projectRoot, deliveryId);
    const phase = delivery.phases.find((item) => item.phaseId === delivery.activePhaseId) ??
      [...delivery.phases].reverse().find((item) => item.status === "completed") ??
      delivery.phases.at(-1);
    return {
      ...refs,
      taskPlanRef: phase?.latestRefs.taskPlan ?? null,
      taskPlanRunRef: phase?.latestRefs.taskPlanRun ?? null,
      reviewResultRef: phase?.latestRefs.reviewResult ?? phase?.latestRefs.review ?? null,
    };
  } catch {
    return refs;
  }
}

export function classifyDeploymentRepair(input: {
  failureKind: DeploymentFailureKind;
  editableFileCount: number;
  failureOwner?: DeploymentFailureOwner | null;
}): {
  failureOwner: DeploymentFailureOwner;
  repairRoute: DeploymentRepairRoute;
} {
  const { failureKind, editableFileCount } = input;
  if (input.failureOwner) {
    return {
      failureOwner: input.failureOwner,
      repairRoute: repairRouteForFailureOwner(input.failureOwner, failureKind),
    };
  }
  if (failureKind === "docker_unavailable") {
    return { failureOwner: "environment", repairRoute: "none" };
  }
  if (failureKind === "registry_network") {
    return { failureOwner: "external_system", repairRoute: "none" };
  }
  if (
    failureKind === "build_command_failed" ||
    failureKind === "start_command_failed" ||
    failureKind === "application_startup_failed" ||
    failureKind === "http_probe_failed" ||
    failureKind === "preview_not_verified" ||
    (failureKind === "healthcheck" && editableFileCount === 0)
  ) {
    return {
      failureOwner: "application_code",
      repairRoute: failureKind === "healthcheck" ? "manual_review" : "execution_repair",
    };
  }
  if (editableFileCount > 0) {
    return { failureOwner: "deployment_assets", repairRoute: "deploy_repair" };
  }
  return { failureOwner: "unknown", repairRoute: "manual_review" };
}

function repairRouteForFailureOwner(
  failureOwner: DeploymentFailureOwner,
  failureKind: DeploymentFailureKind,
): DeploymentRepairRoute {
  if (failureOwner === "application_code" && failureKind !== "healthcheck") {
    return "execution_repair";
  }
  if (failureOwner === "deployment_assets") {
    return "deploy_repair";
  }
  if (failureOwner === "environment" || failureOwner === "external_system") {
    return "none";
  }
  return "manual_review";
}

function suggestedActionsFor(
  spec: DeploymentSpec,
  failureKind: DeploymentFailureKind,
  diagnostics: DeploymentFailureDiagnostic[],
): string[] {
  const actions = [
    "Inspect the failed command output and repair the selected provider before retrying.",
  ];

  if (failureKind === "registry_network") {
    return [
      "Docker could not reach the image registry or fetch image metadata.",
      "Retry the build, pre-pull the blocked base image, or configure Docker registry/mirror/network access.",
      "Do not edit Dockerfile, Compose, or application code unless registry access succeeds and a new deployment failure appears.",
      ...diagnosticActions(diagnostics),
    ];
  }

  if (failureKind === "docker_unavailable") {
    return [
      "Docker is unavailable from the current Loom agent session.",
      "Start Docker Desktop or the Docker daemon and verify `docker version` works from the same terminal/session.",
      "If Docker works in a normal terminal but not in this agent chat, reopen or reconfigure the agent session with full local access / Docker command permission.",
      "Do not edit Dockerfile, Compose, or application code for this failure.",
      ...diagnosticActions(diagnostics),
    ];
  }

  if (classifyDeploymentRepair({
    failureKind,
    editableFileCount: editableFilesFor(spec, failureKind).length,
  }).repairRoute === "execution_repair") {
    return [
      "Deploy classified this as an application code/runtime delivery failure.",
      "Create deploy-sourced execution repair from .loom/deployment/state/latest-failure.json.",
      "Do not edit generated Dockerfile/Compose through deploy repair for this failure.",
      ...diagnosticActions(diagnostics),
    ];
  }

  if (editableFilesFor(spec, failureKind).length > 0) {
    actions.push("Edit only the deployment files listed in editableFiles, then rerun loom deploy up.");
  }

  if (failureKind === "api_route_not_verified") {
    actions.push("Keep the public frontend entry as the only exposed service and route API paths to the backend service through the deployment topology.");
  }

  if (spec.environment.missing.length > 0) {
    actions.push(`Review missing environment variables before retrying: ${spec.environment.missing.map((variable) => variable.name).join(", ")}.`);
  }

  if (spec.bootstrap.tasks.length > 0) {
    actions.push(`Review bootstrap diagnostics before retrying: ${spec.bootstrap.tasks.map((task) => `${task.kind} (${task.command})`).join(", ")}. Do not run migrations automatically.`);
  }

  actions.push(...diagnosticActions(diagnostics));

  actions.push("If deployment cannot be repaired within Dockerfile/Compose, explain the blocker clearly instead of switching builders automatically.");

  if (failureKind === "healthcheck") {
    actions.push("Verify the app binds to 0.0.0.0 and that the healthcheck URL/path matches an endpoint that returns a non-error status.");
  }

  return actions;
}

function editableFilesFor(spec: DeploymentSpec, failureKind: DeploymentFailureKind): string[] {
  if (
    failureKind === "docker_unavailable" ||
    failureKind === "registry_network" ||
    failureKind === "runtime_contract_missing" ||
    failureKind === "runtime_contract_not_applicable" ||
    failureKind === "runtime_contract_mismatch" ||
    failureKind === "build_command_failed" ||
    failureKind === "start_command_failed" ||
    failureKind === "application_startup_failed" ||
    failureKind === "http_probe_failed" ||
    failureKind === "preview_not_verified"
  ) {
    return [];
  }
  if (spec.provider === "compose-existing") {
    return [];
  }

  return [
    spec.files.composePath,
    ...Object.values(spec.files.dockerfilePaths),
    ...Object.values(spec.files.nginxConfigPaths),
    ...(spec.files.dockerignorePath ? [spec.files.dockerignorePath] : []),
  ].filter((item, index, items) => items.indexOf(item) === index);
}

function protectedFilesFor(spec: DeploymentSpec): string[] {
  return spec.files.reused;
}

function instructionFor(spec: DeploymentSpec, failureKind: DeploymentFailureKind): string {
  const base =
    "Repair only deployment files listed in editableFiles. Do not edit application code, package scripts, tests, or RuntimeDeliveryContract from deploy repair. Preserve reused user assets unless they are listed as editable.";

  switch (failureKind) {
    case "compose_config":
      return `${base} Focus on fixing Compose syntax, build context, service names, ports, and file references.`;
    case "image_build":
      return `${base} Focus on Dockerfile install/build commands, lockfile handling, build context, and ignored files.`;
    case "registry_network":
      return "Docker registry/network access failed while pulling image metadata or layers; do not edit deployment files. Ask the user to retry, pre-pull the blocked image, configure a registry mirror, or fix Docker network/proxy/auth.";
    case "container_start":
      return `${base} Focus on runtime command, exposed port, host binding, environment diagnostics, and missing production artifacts.`;
    case "healthcheck":
      return `${base} Focus on the HTTP healthcheck path, bound port, app listen address, startup timing, missing environment variables, and whether the app returns a non-error status at the configured healthcheck URL.`;
    case "runtime_contract_missing":
    case "runtime_contract_not_applicable":
    case "runtime_contract_mismatch":
    case "build_command_failed":
    case "start_command_failed":
    case "application_startup_failed":
    case "http_probe_failed":
    case "preview_not_verified":
      return "Deploy cannot repair this by editing generated deployment assets. Route this application-code/runtime delivery failure through repair request --type execution --source deploy using the latest failure report.";
    case "api_route_not_verified":
      return `${base} Focus on the public entry service, generated nginx proxy routes, Compose service names, internal backend port, and depends_on wiring. Do not edit application API handlers for this failure.`;
    case "deploy_asset_invalid":
      return `${base} Focus only on generated Dockerfile, Compose, dockerignore, healthcheck, port, and environment injection.`;
    case "logs":
      return `${base} Focus on whether the Compose project and service still exist before attempting file edits.`;
    case "docker_unavailable":
      return "Docker is unavailable from the current Loom agent session; do not edit deployment files. Ask the user to start Docker Desktop/the Docker daemon, verify `docker version` works in the same terminal/session, and enable full local access or Docker command permission if this agent chat is sandboxed.";
    case "unknown":
      return `${base} Use the command output to classify the failure before editing files.`;
  }
}

function failedContractFor(
  spec: DeploymentSpec,
  failureKind: DeploymentFailureKind,
): DeploymentFailureReport["failedContract"] {
  const primary = spec.sourceModel.services.find((service) => service.serviceId === spec.sourceModel.primaryServiceId) ??
    spec.sourceModel.services[0];
  const workingDirectory = primary?.workingDirectory ?? spec.workspace.appPath;
  if (failureKind === "build_command_failed") {
    return {
      field: "build.command",
      command: spec.runtimeContract.buildCommand,
      workingDirectory,
    };
  }
  if (failureKind === "start_command_failed") {
    return {
      field: "start.command",
      command: spec.runtimeContract.startCommand,
      workingDirectory,
    };
  }
  if (failureKind === "http_probe_failed" || failureKind === "preview_not_verified") {
    return {
      field: "httpProbes.previewPath",
      command: null,
      workingDirectory,
    };
  }
  if (failureKind === "api_route_not_verified") {
    return {
      field: "deploymentTopology.apiRoutes",
      command: null,
      workingDirectory,
    };
  }
  if (failureKind === "application_startup_failed") {
    return {
      field: "runtime.startup",
      command: spec.runtimeContract.startCommand ?? primary?.startCommand ?? null,
      workingDirectory,
    };
  }
  return {
    field: failureKind === "healthcheck" ? "httpProbes.healthPath" : "runtime.delivery",
    command: null,
    workingDirectory,
  };
}

function failedAtFor(failureKind: DeploymentFailureKind): string {
  if (failureKind === "build_command_failed") return "runtime_build_command";
  if (failureKind === "start_command_failed") return "runtime_start_command";
  if (failureKind === "application_startup_failed") return "runtime_application_startup";
  if (failureKind === "http_probe_failed") return "runtime_http_probe";
  if (failureKind === "preview_not_verified") return "runtime_preview_probe";
  if (failureKind === "api_route_not_verified") return "runtime_api_route_probe";
  if (failureKind === "healthcheck") return "runtime_healthcheck";
  return "deployment_runtime_validation";
}

function logMarkersFor(failureKind: DeploymentFailureKind): string[] {
  if (failureKind === "build_command_failed") return ["LOOM_RUNTIME_BUILD_START"];
  if (failureKind === "start_command_failed") return ["LOOM_RUNTIME_START_COMMAND"];
  if (failureKind === "application_startup_failed") return ["LOOM_RUNTIME_START_COMMAND"];
  return [];
}

function failureSignature(
  failureKind: DeploymentFailureKind,
  field: string,
  errorWindow: DeploymentErrorWindow,
  stdoutTail: string[],
  stderrTail: string[],
): string {
  const hash = createHash("sha1")
    .update([failureKind, field, ...errorWindow.lines.slice(-16), ...stdoutTail.slice(-4), ...stderrTail.slice(-4)].join("\n"))
    .digest("hex")
    .slice(0, 10);
  return `${failureKind}:${field}:${hash}`;
}

function createErrorWindow(
  stdout: string,
  stderr: string,
  diagnostics: DeploymentFailureDiagnostic[],
): DeploymentErrorWindow {
  const lines = [
    ...streamLines("stdout", stdout),
    ...streamLines("stderr", stderr),
  ];
  const matchedIndexes = new Set<number>();
  const matchedPatterns = new Set<string>();

  for (const diagnostic of diagnostics) {
    matchedPatterns.add(diagnostic.code);
    for (const evidence of diagnostic.evidence) {
      const normalizedEvidence = normalizeLine(evidence);
      if (!normalizedEvidence) continue;
      lines.forEach((line, index) => {
        if (normalizeLine(line.text).includes(normalizedEvidence)) {
          matchedIndexes.add(index);
        }
      });
    }
  }

  lines.forEach((line, index) => {
    for (const signal of errorWindowSignals) {
      if (signal.pattern.test(line.text)) {
        matchedIndexes.add(index);
        matchedPatterns.add(signal.code);
      }
    }
  });

  if (lines.length === 0) {
    return {
      lines: [],
      truncated: false,
      totalLineCount: 0,
      matchedPatterns: [...matchedPatterns],
    };
  }

  if (matchedIndexes.size === 0) {
    const selected = lines.slice(-ERROR_WINDOW_FALLBACK_LINES);
    return {
      lines: selected.map(formatErrorWindowLine),
      truncated: lines.length > selected.length,
      totalLineCount: lines.length,
      matchedPatterns: [],
    };
  }

  const selectedIndexes = new Set<number>();
  for (const index of matchedIndexes) {
    const start = Math.max(0, index - ERROR_WINDOW_CONTEXT_BEFORE);
    const end = Math.min(lines.length - 1, index + ERROR_WINDOW_CONTEXT_AFTER);
    for (let current = start; current <= end; current += 1) {
      selectedIndexes.add(current);
    }
  }

  const orderedIndexes = [...selectedIndexes].sort((left, right) => left - right);
  const clippedIndexes = orderedIndexes.slice(-ERROR_WINDOW_MAX_LINES);

  return {
    lines: withGapMarkers(clippedIndexes, lines).map((line) =>
      typeof line === "string" ? line : formatErrorWindowLine(line)
    ),
    truncated: clippedIndexes.length < lines.length,
    totalLineCount: lines.length,
    matchedPatterns: [...matchedPatterns],
  };
}

function streamLines(stream: "stdout" | "stderr", value: string): Array<{ stream: "stdout" | "stderr"; text: string }> {
  return value
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0)
    .map((text) => ({ stream, text }));
}

function withGapMarkers(
  indexes: number[],
  lines: Array<{ stream: "stdout" | "stderr"; text: string }>,
): Array<string | { stream: "stdout" | "stderr"; text: string }> {
  const output: Array<string | { stream: "stdout" | "stderr"; text: string }> = [];
  let previous = -1;
  for (const index of indexes) {
    if (previous >= 0 && index > previous + 1) {
      output.push(`... ${index - previous - 1} line(s) omitted ...`);
    }
    output.push(lines[index]);
    previous = index;
  }
  return output;
}

function formatErrorWindowLine(line: { stream: "stdout" | "stderr"; text: string }): string {
  return `[${line.stream}] ${line.text}`;
}

function normalizeLine(value: string): string {
  return value.trim().toLowerCase();
}

function tailLines(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0)
    .slice(-80);
}
