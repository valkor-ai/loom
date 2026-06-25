import type {
  DeploymentHttpProxyRoute,
  DeploymentRuntimeContract,
  DeploymentSourceModel,
  DeploymentSourceService,
  DeploymentTopology,
} from "./types";

export function buildDeploymentTopology(input: {
  runtimeContract: DeploymentRuntimeContract;
  sourceModel: DeploymentSourceModel;
}): DeploymentTopology {
  if (input.sourceModel.services.length === 0) {
    return {
      schemaVersion: 1,
      publicEntryServiceId: input.sourceModel.previewServiceId || input.sourceModel.primaryServiceId || "app",
      routes: [],
      validation: {
        previewPaths: dedupePaths([input.runtimeContract.previewPath || "/"]),
        apiPaths: dedupePaths(input.runtimeContract.apiPaths),
      },
    };
  }

  const previewService = previewServiceFor(input.sourceModel);
  const previewPaths = dedupePaths([input.runtimeContract.previewPath || "/"]);
  const apiPaths = dedupePaths(input.runtimeContract.apiPaths);
  const routes: DeploymentTopology["routes"] = [];

  if (previewService.role === "frontend" && !previewService.startCommand) {
    routes.push({
      kind: "static-spa",
      publicPath: "/",
      targetServiceId: previewService.serviceId,
    });
  }

  if (input.runtimeContract.deploymentShape === "frontend-and-backend") {
    const backend = backendServiceFor(input.sourceModel);
    routes.push({
      kind: "http-proxy",
      publicPath: apiBasePathFor(input.runtimeContract, apiPaths),
      targetServiceId: backend.serviceId,
      targetPort: backend.port,
      preservePath: true,
    });
  }

  return {
    schemaVersion: 1,
    publicEntryServiceId: previewService.serviceId,
    routes,
    validation: {
      previewPaths,
      apiPaths,
    },
  };
}

export function proxyRoutesForPublicEntry(topology: DeploymentTopology): DeploymentHttpProxyRoute[] {
  return topology.routes.filter((route): route is DeploymentHttpProxyRoute => route.kind === "http-proxy");
}

export function proxyTargetServiceIdsForPublicEntry(topology: DeploymentTopology): string[] {
  return [...new Set(proxyRoutesForPublicEntry(topology).map((route) => route.targetServiceId))].sort(compareStrings);
}

function previewServiceFor(sourceModel: DeploymentSourceModel): DeploymentSourceService {
  return sourceModel.services.find((service) => service.serviceId === sourceModel.previewServiceId) ??
    sourceModel.services[0];
}

function backendServiceFor(sourceModel: DeploymentSourceModel): DeploymentSourceService {
  return sourceModel.services.find((service) => service.serviceId === sourceModel.primaryServiceId) ??
    sourceModel.services.find((service) => service.role === "backend") ??
    sourceModel.services.find((service) => service.serviceId !== sourceModel.previewServiceId) ??
    sourceModel.services[0];
}

function apiBasePathFor(runtimeContract: DeploymentRuntimeContract, apiPaths: string[]): string {
  const explicit = normalizePath(runtimeContract.api?.basePath ?? "");
  if (explicit !== "/") {
    return explicit;
  }

  const firstApiPath = apiPaths.find((item) => item !== "/") ?? "";
  const firstSegment = firstApiPath.match(/^\/[^/?#]+/)?.[0];
  return firstSegment ?? "/api";
}

function dedupePaths(paths: string[]): string[] {
  return [...new Set(paths.map(normalizePath))].sort(compareStrings);
}

function normalizePath(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "/";
  }
  const [pathOnly] = trimmed.split(/[?#]/, 1);
  const normalized = pathOnly.startsWith("/") ? pathOnly : `/${pathOnly}`;
  return normalized.length > 1 ? normalized.replace(/\/+$/, "") : "/";
}

function compareStrings(left: string, right: string): number {
  return left.localeCompare(right);
}
