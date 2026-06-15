import type { ArchitectureArtifactContract } from "../contracts";
import type { DependencyService, DeploymentRuntimeContract, DetectedStack } from "./types";
import {
  dedupeDependencyServices,
  dependencyServiceKindsFromRuntimeSignals,
  hasDatabaseRuntimeSignal,
  isSqlServiceKind,
  serviceDefinition,
  springDatasourceEnv,
} from "./dependency-signals";

export function deploymentRuntimeContractFromAac(
  aac: ArchitectureArtifactContract | null,
  fallbackStack: DetectedStack,
  ref: string | null,
): DeploymentRuntimeContract {
  const runtime = aac?.runtimeDelivery;
  if (!runtime || runtime.status === "not_applicable") {
    return heuristicRuntimeContract(fallbackStack, ref);
  }
  if (runtime.status === "unchanged") {
    return {
      ...heuristicRuntimeContract(fallbackStack, runtime.basis.previousRuntimeDeliveryRef ?? ref),
      source: "previous_accepted_aac",
      status: "unchanged",
      runtimeKind: runtime.runtimeKind,
    };
  }
  return {
    source: "accepted_aac",
    ref,
    status: runtime.status,
    dependencyServicePolicy: "contract_only",
    runtimeKind: runtime.runtimeKind,
    buildCommand: runtime.build?.command ?? null,
    startCommand: runtime.start?.command ?? null,
    port: runtime.start?.port ?? null,
    previewPath: runtime.httpProbes?.previewPath ?? "/",
    healthPath: runtime.httpProbes?.healthPath ?? null,
    apiPaths: runtime.httpProbes?.apiPaths ?? [],
    frontendOutputDir: runtime.frontend?.outputDir ?? null,
    probeKind: runtimeProbeKind(runtime),
    environment: {
      required: [...(runtime.environment?.required ?? [])],
      optional: [...(runtime.environment?.optional ?? [])],
    },
    dependencyServices: contractDependencyServices(runtime),
  };
}

export function heuristicRuntimeContract(stack: DetectedStack, ref: string | null = null): DeploymentRuntimeContract {
  return {
    source: "heuristic",
    ref,
    status: "heuristic",
    dependencyServicePolicy: "heuristic",
    runtimeKind: stack.framework ?? stack.kind,
    buildCommand: stack.buildCommand,
    startCommand: stack.startCommand,
    port: stack.port,
    previewPath: "/",
    healthPath: stack.healthcheckPath ?? null,
    apiPaths: [],
    frontendOutputDir: stack.outputDirectory,
    probeKind: stack.startCommand ? "http" : "process",
    environment: {
      required: [],
      optional: [],
    },
    dependencyServices: [],
  };
}

export function applyRuntimeContractToStack(
  stack: DetectedStack,
  runtimeContract: DeploymentRuntimeContract,
): DetectedStack {
  const inferredKind = inferRuntimeContractStackKind(stack, runtimeContract);
  const inferredPackageManager = inferRuntimeContractPackageManager(stack, runtimeContract, inferredKind);
  return {
    ...stack,
    kind: inferredKind,
    packageManager: inferredPackageManager,
    framework: stack.framework ?? inferRuntimeContractFramework(runtimeContract, inferredKind),
    buildCommand: runtimeContract.buildCommand ?? stack.buildCommand,
    startCommand: deploymentStartCommand(stack, runtimeContract),
    healthcheckPath: runtimeContract.healthPath ?? runtimeContract.previewPath ?? stack.healthcheckPath,
    outputDirectory: runtimeContract.frontendOutputDir ?? stack.outputDirectory,
    port: runtimeContract.port ?? stack.port,
    services: runtimeContract.dependencyServicePolicy === "contract_only"
      ? contractOnlyDependencyServices(runtimeContract, stack.services)
      : mergeDependencyServices(stack.services, runtimeContract.dependencyServices),
  };
}

function runtimeProbeKind(runtime: NonNullable<ArchitectureArtifactContract["runtimeDelivery"]>): DeploymentRuntimeContract["probeKind"] {
  const surfaces = runtime.runtimeSurfaces ?? [];
  if (
    runtime.httpProbes?.previewPath ||
    runtime.httpProbes?.healthPath ||
    (runtime.httpProbes?.apiPaths ?? []).length > 0 ||
    surfaces.some((surface) => surface.kind === "http" || surface.probe.type === "http_path")
  ) {
    return "http";
  }
  if (surfaces.some((surface) => surface.probe.type === "command")) {
    return "command";
  }
  return "process";
}

function contractDependencyServices(
  runtime: NonNullable<ArchitectureArtifactContract["runtimeDelivery"]>,
): DeploymentRuntimeContract["dependencyServices"] {
  const signals = [
    runtime.runtimeKind,
    runtime.api?.kind,
    runtime.deliveryMechanics?.api?.basePath,
    ...(runtime.environment?.required ?? []),
    ...(runtime.environment?.optional ?? []),
    ...(runtime.httpProbes?.apiPaths ?? []),
  ]
    .filter((value): value is string => typeof value === "string")
    .join("\n")
    .toLowerCase();
  const services: DeploymentRuntimeContract["dependencyServices"] = [];

  for (const kind of dependencyServiceKindsFromRuntimeSignals(signals)) {
    const service = serviceDefinition(kind, "Declared by RuntimeDeliveryContract environment/runtime signals.");
    services.push(isSqlServiceKind(kind)
      ? { ...service, connectionEnv: springDatasourceEnv(kind) }
      : service);
  }

  return dedupeDependencyServices(services);
}

function inferRuntimeContractStackKind(
  stack: DetectedStack,
  runtimeContract: DeploymentRuntimeContract,
): DetectedStack["kind"] {
  if (stack.kind !== "unknown" || runtimeContract.source === "heuristic") {
    return stack.kind;
  }

  const signals = runtimeContractSignals(runtimeContract);
  if (/\b(node|npm|pnpm|yarn|bun|vite|next|react|express|fastify|hono|koa)\b/.test(signals)) {
    return "node";
  }
  if (/\b(python|pip|poetry|uv|uvicorn|gunicorn|fastapi|flask|django)\b/.test(signals)) {
    return "python";
  }
  if (/\b(go|golang)\b/.test(signals)) {
    return "go";
  }
  if (/\b(java|maven|gradle|spring)\b/.test(signals)) {
    return "java";
  }
  if (/\b(dotnet|aspnet|csharp|c#)\b/.test(signals)) {
    return "dotnet";
  }
  if (/\b(php|composer|laravel|symfony)\b/.test(signals)) {
    return "php";
  }
  if (/\b(ruby|bundler|bundle|rails|sinatra)\b/.test(signals)) {
    return "ruby";
  }
  if (runtimeContract.frontendOutputDir && !runtimeContract.startCommand) {
    return "static";
  }
  return stack.kind;
}

function inferRuntimeContractPackageManager(
  stack: DetectedStack,
  runtimeContract: DeploymentRuntimeContract,
  kind: DetectedStack["kind"],
): DetectedStack["packageManager"] {
  if (stack.packageManager || runtimeContract.source === "heuristic") {
    return stack.packageManager;
  }

  const signals = runtimeContractSignals(runtimeContract);
  if (kind === "node") {
    if (/\bpnpm\b/.test(signals)) return "pnpm";
    if (/\byarn\b/.test(signals)) return "yarn";
    if (/\bbun\b/.test(signals)) return "bun";
    return "npm";
  }
  if (kind === "python") {
    if (/\bpoetry\b/.test(signals)) return "poetry";
    if (/\buv\b/.test(signals)) return "uv";
    return "pip";
  }
  if (kind === "java") {
    return /\bgradle\b/.test(signals) ? "gradle" : "maven";
  }
  if (kind === "dotnet") return "dotnet";
  if (kind === "go") return "go";
  if (kind === "php") return "composer";
  if (kind === "ruby") return "bundler";
  return stack.packageManager;
}

function inferRuntimeContractFramework(
  runtimeContract: DeploymentRuntimeContract,
  kind: DetectedStack["kind"],
): string | null {
  if (runtimeContract.source === "heuristic") {
    return null;
  }
  const signals = runtimeContractSignals(runtimeContract);
  if (kind === "node") {
    if (/\bvite\b/.test(signals)) return "vite";
    if (/\bnext\b/.test(signals)) return "next";
    if (/\b(express|fastify|hono|koa)\b/.test(signals)) return "node-server";
    return "node";
  }
  return null;
}

function runtimeContractSignals(runtimeContract: DeploymentRuntimeContract): string {
  return [
    runtimeContract.runtimeKind,
    runtimeContract.buildCommand,
    runtimeContract.startCommand,
    runtimeContract.frontendOutputDir,
    ...runtimeContract.environment.required,
    ...runtimeContract.environment.optional,
    ...runtimeContract.dependencyServices.flatMap((service) => [
      service.kind,
      service.serviceName,
      service.image,
      ...Object.keys(service.connectionEnv),
      ...Object.values(service.connectionEnv),
    ]),
    ...runtimeContract.apiPaths,
  ]
    .filter((value): value is string => typeof value === "string")
    .join("\n")
    .toLowerCase();
}

function mergeDependencyServices(detected: DependencyService[], declared: DependencyService[]): DependencyService[] {
  const byKind = new Map<string, DependencyService>();
  for (const service of detected) {
    byKind.set(service.kind, service);
  }
  for (const service of declared) {
    byKind.set(service.kind, service);
  }
  return [...byKind.values()];
}

function contractOnlyDependencyServices(
  runtimeContract: DeploymentRuntimeContract,
  detected: DependencyService[],
): DependencyService[] {
  if (runtimeContract.dependencyServices.length > 0) {
    return runtimeContract.dependencyServices;
  }
  if (!hasDatabaseRuntimeSignal(runtimeContractSignals(runtimeContract))) {
    return [];
  }
  return detected
    .filter((service) => isSqlServiceKind(service.kind))
    .map((service) => serviceWithRuntimeContractConnectionEnv(service, runtimeContract));
}

function serviceWithRuntimeContractConnectionEnv(
  service: DependencyService,
  runtimeContract: DeploymentRuntimeContract,
): DependencyService {
  const requested = new Set([
    ...runtimeContract.environment.required,
    ...runtimeContract.environment.optional,
  ]);
  if (
    !requested.has("SPRING_DATASOURCE_URL") &&
    !requested.has("SPRING_DATASOURCE_USERNAME") &&
    !requested.has("SPRING_DATASOURCE_PASSWORD")
  ) {
    return service;
  }

  if (!isSqlServiceKind(service.kind)) {
    return service;
  }

  return {
    ...service,
    connectionEnv: {
      ...service.connectionEnv,
      ...springDatasourceEnv(service.kind),
    },
  };
}

function deploymentStartCommand(
  stack: DetectedStack,
  runtimeContract: DeploymentRuntimeContract,
): string | null {
  const contractCommand = runtimeContract.startCommand;
  const detectedCommand = stack.startCommand;
  const targetPort = runtimeContract.port ?? stack.port;

  if (contractCommand && detectedCommand && isLongLivedDevCommand(contractCommand) && !isLongLivedDevCommand(detectedCommand)) {
    return ensureStartCommandPort(detectedCommand, stack, targetPort);
  }

  return ensureStartCommandPort(contractCommand ?? detectedCommand, stack, targetPort);
}

function isLongLivedDevCommand(command: string): boolean {
  return /\b(dev|watch)\b/i.test(command);
}

function ensureStartCommandPort(
  command: string | null,
  stack: DetectedStack,
  port: number,
): string | null {
  if (!command || stack.framework !== "vite" || !/\bpreview\b/i.test(command) || /\s--port(?:=|\s)/.test(command)) {
    return command;
  }
  return `${command} --port ${port}`;
}
