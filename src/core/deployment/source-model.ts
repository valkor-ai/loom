import path from "node:path";
import type {
  DeploymentCodeProbe,
  DeploymentPackageManager,
  DeploymentRuntimeContract,
  DeploymentRuntimeKind,
  DeploymentSourceModel,
  DeploymentSourceService,
} from "./types";

export function sourceModelFromProbe(input: {
  probe: DeploymentCodeProbe;
  buildContextPath: string;
}): DeploymentSourceModel {
  const service = sourceServiceFromProbe({
    serviceId: "app",
    role: "app",
    root: input.probe.workingDirectory ?? ".",
    probe: input.probe,
  });
  return {
    schemaVersion: 1,
    source: "code-probe",
    shape: "single-service",
    primaryServiceId: service.serviceId,
    previewServiceId: service.serviceId,
    buildContextPath: input.buildContextPath,
    services: [service],
    dependencies: [...input.probe.services],
    notes: ["Deployment source model was derived from repository runtime probes."],
  };
}

export function sourceModelFromRuntimeContract(input: {
  runtimeContract: DeploymentRuntimeContract;
  fallbackProbe: DeploymentCodeProbe;
  buildContextPath: string;
}): DeploymentSourceModel {
  if (input.runtimeContract.source === "heuristic") {
    return sourceModelFromProbe({ probe: input.fallbackProbe, buildContextPath: input.buildContextPath });
  }

  const dependencies = input.runtimeContract.dependencyServices.length > 0
    ? [...input.runtimeContract.dependencyServices]
    : [...input.fallbackProbe.services];

  if (runtimeContractShape(input.runtimeContract) === "frontend-and-backend") {
    const frontendRoot = serviceRootFromRefs([
      input.runtimeContract.frontend?.sourceRoot,
      input.runtimeContract.frontend?.outputDir,
      input.runtimeContract.frontend?.buildCommand,
      input.runtimeContract.buildCommand,
    ]);
    const backendRoot = serviceRootFromRefs([
      input.runtimeContract.api?.entry,
      input.runtimeContract.api?.buildCommand,
      input.runtimeContract.startCommand,
      input.runtimeContract.buildCommand,
    ]);
    const frontend: DeploymentSourceService = {
      serviceId: "frontend",
      role: "frontend",
      root: frontendRoot,
      workingDirectory: frontendRoot === "." ? null : frontendRoot,
      workspacePackageJsonPaths: [],
      runtimeKind: "node",
      packageManager: packageManagerFromCommand(input.runtimeContract.frontend?.buildCommand ?? input.runtimeContract.buildCommand) ?? "npm",
      hasLockfile: input.fallbackProbe.hasLockfile,
      framework: input.runtimeContract.frontend?.kind ?? "frontend",
      runtimeVersion: null,
      runtimeVersionSource: null,
      buildCommand: commandForServiceRoot(input.runtimeContract.frontend?.buildCommand ?? input.runtimeContract.buildCommand, frontendRoot),
      startCommand: null,
      outputDirectory: input.runtimeContract.frontend?.outputDir ?? input.runtimeContract.frontendOutputDir,
      port: 80,
      healthcheckPath: "/",
    };
    const backend: DeploymentSourceService = {
      serviceId: "backend",
      role: "backend",
      root: backendRoot,
      workingDirectory: backendRoot === "." ? null : backendRoot,
      workspacePackageJsonPaths: [],
      runtimeKind: runtimeKindFromContract(input.runtimeContract.api?.kind, input.runtimeContract.startCommand, input.fallbackProbe.kind),
      packageManager: packageManagerFromCommand(input.runtimeContract.api?.buildCommand ?? input.runtimeContract.startCommand ?? input.runtimeContract.buildCommand) ??
        packageManagerForRuntime(runtimeKindFromContract(input.runtimeContract.api?.kind, input.runtimeContract.startCommand, input.fallbackProbe.kind)),
      hasLockfile: input.fallbackProbe.hasLockfile,
      framework: input.runtimeContract.api?.kind ?? input.fallbackProbe.framework,
      runtimeVersion: input.fallbackProbe.runtimeVersion,
      runtimeVersionSource: input.fallbackProbe.runtimeVersionSource,
      buildCommand: commandForServiceRoot(input.runtimeContract.api?.buildCommand ?? input.runtimeContract.buildCommand, backendRoot),
      startCommand: commandForServiceRoot(input.runtimeContract.startCommand, backendRoot),
      outputDirectory: null,
      port: input.runtimeContract.port ?? input.fallbackProbe.port,
      healthcheckPath: input.runtimeContract.healthPath ?? input.runtimeContract.previewPath,
    };
    return {
      schemaVersion: 1,
      source: "runtime-contract",
      shape: "frontend-and-backend",
      primaryServiceId: backend.serviceId,
      previewServiceId: frontend.serviceId,
      buildContextPath: ".",
      services: [frontend, backend],
      dependencies,
      notes: ["Deployment source model was derived from RuntimeDelivery frontend and api services."],
    };
  }

  const service = sourceServiceFromProbe({
    serviceId: "app",
    role: "app",
    root: input.fallbackProbe.workingDirectory ?? ".",
    probe: {
      ...input.fallbackProbe,
      kind: runtimeKindFromContract(input.runtimeContract.api?.kind ?? input.runtimeContract.runtimeKind, input.runtimeContract.startCommand, input.fallbackProbe.kind),
      packageManager: packageManagerFromCommand(input.runtimeContract.buildCommand ?? input.runtimeContract.startCommand) ?? input.fallbackProbe.packageManager,
      buildCommand: input.runtimeContract.buildCommand ?? input.fallbackProbe.buildCommand,
      startCommand: input.runtimeContract.startCommand ?? input.fallbackProbe.startCommand,
      outputDirectory: input.runtimeContract.frontendOutputDir ?? input.fallbackProbe.outputDirectory,
      port: input.runtimeContract.port ?? input.fallbackProbe.port,
      healthcheckPath: input.runtimeContract.healthPath ?? input.runtimeContract.previewPath ?? input.fallbackProbe.healthcheckPath,
      services: input.runtimeContract.dependencyServices,
    },
  });
  return {
    schemaVersion: 1,
    source: "runtime-contract",
    shape: "single-service",
    primaryServiceId: service.serviceId,
    previewServiceId: service.serviceId,
    buildContextPath: input.buildContextPath,
    services: [service],
    dependencies,
    notes: ["Deployment source model was derived from RuntimeDelivery single service."],
  };
}

export function primarySourceService(model: DeploymentSourceModel): DeploymentSourceService {
  return model.services.find((service) => service.serviceId === model.primaryServiceId) ?? model.services[0];
}

export function previewSourceService(model: DeploymentSourceModel): DeploymentSourceService {
  return model.services.find((service) => service.serviceId === model.previewServiceId) ?? primarySourceService(model);
}

function sourceServiceFromProbe(input: {
  serviceId: string;
  role: DeploymentSourceService["role"];
  root: string;
  probe: DeploymentCodeProbe;
}): DeploymentSourceService {
  return {
    serviceId: input.serviceId,
    role: input.role,
    root: input.root,
    workingDirectory: input.probe.workingDirectory,
    workspacePackageJsonPaths: input.probe.workspacePackageJsonPaths ?? [],
    runtimeKind: input.probe.kind,
    packageManager: input.probe.packageManager,
    hasLockfile: input.probe.hasLockfile,
    framework: input.probe.framework,
    runtimeVersion: input.probe.runtimeVersion,
    runtimeVersionSource: input.probe.runtimeVersionSource,
    buildCommand: input.probe.buildCommand,
    startCommand: input.probe.startCommand,
    outputDirectory: input.probe.outputDirectory,
    port: input.probe.port,
    healthcheckPath: input.probe.healthcheckPath ?? null,
  };
}

function runtimeContractShape(runtimeContract: DeploymentRuntimeContract): DeploymentSourceModel["shape"] {
  if (runtimeContract.deploymentShape) {
    return runtimeContract.deploymentShape;
  }
  if (runtimeContract.frontend?.required && runtimeContract.api?.required) {
    const servedBy = runtimeContract.frontend.servedBy ?? "";
    if (!/(express|spring|rails|django|laravel|static)/i.test(servedBy)) {
      return "frontend-and-backend";
    }
  }
  return "single-service";
}

function runtimeKindFromContract(
  value: string | null | undefined,
  command: string | null,
  fallback: DeploymentRuntimeKind,
): DeploymentRuntimeKind {
  const signals = `${value ?? ""}\n${command ?? ""}`.toLowerCase();
  if (/(spring|java|maven|gradle|mvn)/.test(signals)) return "java";
  if (/(express|node|vite|react|npm|pnpm|yarn|bun)/.test(signals)) return "node";
  if (/(fastapi|flask|django|python|pip|poetry|uvicorn|gunicorn)/.test(signals)) return "python";
  if (/(golang|\bgo\b)/.test(signals)) return "go";
  if (/(dotnet|aspnet|csharp|c#)/.test(signals)) return "dotnet";
  if (/(laravel|php|composer)/.test(signals)) return "php";
  if (/(rails|ruby|bundle)/.test(signals)) return "ruby";
  return fallback;
}

function packageManagerFromCommand(command: string | null | undefined): DeploymentPackageManager {
  const value = command?.toLowerCase() ?? "";
  if (/\bpnpm\b/.test(value)) return "pnpm";
  if (/\byarn\b/.test(value)) return "yarn";
  if (/\bbun\b/.test(value)) return "bun";
  if (/\bnpm\b/.test(value)) return "npm";
  if (/\bmvnw?\b/.test(value)) return "maven";
  if (/\bgradlew?\b/.test(value)) return "gradle";
  if (/\bdotnet\b/.test(value)) return "dotnet";
  if (/\bcomposer\b/.test(value)) return "composer";
  if (/\bbundle\b/.test(value)) return "bundler";
  if (/\buv\b/.test(value)) return "uv";
  if (/\bpoetry\b/.test(value)) return "poetry";
  if (/\bpip\b/.test(value)) return "pip";
  if (/\bgo\b/.test(value)) return "go";
  return null;
}

function packageManagerForRuntime(kind: DeploymentRuntimeKind): DeploymentPackageManager {
  switch (kind) {
    case "node":
      return "npm";
    case "java":
      return "maven";
    case "python":
      return "pip";
    case "go":
      return "go";
    case "dotnet":
      return "dotnet";
    case "php":
      return "composer";
    case "ruby":
      return "bundler";
    case "static":
    case "unknown":
      return null;
  }
}

function serviceRootFromRefs(values: Array<string | null | undefined>): string {
  for (const value of values) {
    if (!value) continue;
    const prefixMatch = value.match(/--prefix\s+([^\s;&|]+)/);
    if (prefixMatch) return normalizeRoot(prefixMatch[1]);
    const fileMatch = value.match(/\b((?:apps|services|packages)\/[^/\s;&|]+)/);
    if (fileMatch) return normalizeRoot(fileMatch[1]);
  }
  return ".";
}

function commandForServiceRoot(command: string | null | undefined, root: string): string | null {
  if (!command) return null;
  if (root === ".") return command;
  const prefix = `${root}/`;
  return command
    .replace(new RegExp(`\\s--prefix\\s+${escapeRegExp(root)}\\b`, "g"), "")
    .replace(new RegExp(`\\s-f\\s+${escapeRegExp(prefix)}`, "g"), " -f ")
    .replaceAll(prefix, "");
}

function normalizeRoot(root: string): string {
  const normalized = path.posix.normalize(root.replace(/\\/g, "/")).replace(/^\/+/, "");
  return normalized && normalized !== "." ? normalized : ".";
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
