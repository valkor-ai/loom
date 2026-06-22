import net from "node:net";
import path from "node:path";
import { dockerUnavailable } from "../errors";
import { toProjectRelative } from "../state/paths";
import { execFile, type ExecResult } from "./exec";
import type { DeploymentHealth, DeploymentSpec, DeploymentState } from "./types";

export async function findAvailablePort(preferredPort: number): Promise<number> {
  for (let port = preferredPort; port < preferredPort + 100; port += 1) {
    if (await isPortAvailable(port)) {
      return port;
    }
  }
  return preferredPort;
}

export async function ensureDockerAvailable(projectRoot: string): Promise<void> {
  try {
    const result = await execFile("docker", ["version", "--format", "{{.Server.Version}}"], {
      cwd: projectRoot,
      timeoutMs: 10_000,
    });
    if (result.exitCode !== 0) {
      throw dockerUnavailable("Docker is unavailable.", dockerUnavailableDetails(result));
    }
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw dockerUnavailable("Docker CLI was not found.", {
        command: "docker",
        ...dockerAccessGuidance(),
      });
    }
    throw error;
  }
}

export async function dockerCompose(
  projectRoot: string,
  composePath: string,
  args: string[],
  timeoutMs?: number,
): Promise<ExecResult> {
  const relativeComposePath = toProjectRelative(projectRoot, composePath);
  return execFile("docker", ["compose", "-f", relativeComposePath, ...args], {
    cwd: projectRoot,
    timeoutMs,
  });
}

export async function dockerComposeExec(
  projectRoot: string,
  composePath: string,
  serviceName: string,
  command: string,
  timeoutMs?: number,
): Promise<ExecResult> {
  return dockerCompose(projectRoot, composePath, ["exec", "-T", serviceName, "sh", "-lc", command], timeoutMs);
}

export async function inspectContainer(
  projectRoot: string,
  containerName: string,
): Promise<{ containerId: string | null; running: boolean }> {
  const result = await execFile(
    "docker",
    ["inspect", "--format", "{{.Id}} {{.State.Running}}", containerName],
    { cwd: projectRoot, timeoutMs: 10_000 },
  );

  if (result.exitCode !== 0) {
    return { containerId: null, running: false };
  }

  const [containerId, running] = result.stdout.trim().split(/\s+/, 2);
  return {
    containerId: containerId || null,
    running: running === "true",
  };
}

export async function findComposeServiceContainer(
  projectRoot: string,
  composePath: string,
  serviceName: string,
): Promise<{ containerId: string | null; running: boolean; containerName: string | null }> {
  const result = await dockerCompose(projectRoot, composePath, ["ps", "-q", serviceName], 10_000);
  const containerId = result.stdout.trim().split(/\s+/).find(Boolean) ?? null;
  if (!containerId) {
    return { containerId: null, running: false, containerName: null };
  }

  const inspected = await inspectContainer(projectRoot, containerId);
  const nameResult = await execFile("docker", ["inspect", "--format", "{{.Name}}", containerId], {
    cwd: projectRoot,
    timeoutMs: 10_000,
  });
  const containerName = nameResult.exitCode === 0
    ? nameResult.stdout.trim().replace(/^\//, "") || null
    : null;

  return {
    containerId: inspected.containerId ?? containerId,
    running: inspected.running,
    containerName,
  };
}

export function createRunningState(input: {
  projectRoot: string;
  spec: DeploymentSpec;
  specPath: string;
  containerName: string;
  containerId: string | null;
  running: boolean;
  health: DeploymentHealth;
}): DeploymentState {
  const now = new Date().toISOString();
  return {
    schemaVersion: 1,
    provider: input.spec.provider,
    serviceName: input.spec.serviceName,
    appServiceName: input.spec.compose.selectedService ?? input.spec.serviceName,
    imageName: input.spec.imageName,
    projectRoot: input.projectRoot,
    specPath: toProjectRelative(input.projectRoot, input.specPath),
    composePath: input.spec.files.composePath,
    containerName: input.containerName,
    containerId: input.containerId,
    running: input.running,
    url: input.running ? input.spec.runtime.url : null,
    health: input.health,
    startedAt: input.running ? now : null,
    updatedAt: now,
  };
}

export function containerNameFor(spec: DeploymentSpec): string {
  return spec.provider === "compose-existing" && spec.compose.selectedService
    ? spec.compose.selectedService
    : `loom-${spec.serviceName}`;
}

export function resolveComposePath(projectRoot: string, spec: DeploymentSpec): string {
  return path.resolve(projectRoot, spec.files.composePath);
}

function commandDetails(result: ExecResult): Record<string, unknown> {
  return {
    exitCode: result.exitCode,
    stdout: result.stdout.trim(),
    stderr: result.stderr.trim(),
  };
}

function dockerUnavailableDetails(result: ExecResult): Record<string, unknown> {
  return {
    ...commandDetails(result),
    ...dockerAccessGuidance(),
  };
}

function dockerAccessGuidance(): Record<string, unknown> {
  return {
    possibleCauses: [
      "Docker Desktop or the Docker daemon is not running.",
      "The current OS user or shell cannot access the Docker daemon.",
      "The current agent session is sandboxed or lacks full local shell/Docker access.",
    ],
    userActions: [
      "Start Docker Desktop or the Docker daemon and wait until `docker version` works.",
      "If Docker works in a normal terminal but not in this agent chat, reopen or reconfigure the agent session with full local access / Docker command permission.",
      "Retry the same loom deploy command after Docker is reachable from the agent session.",
    ],
  };
}

function isPortAvailable(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once("error", () => resolve(false));
    server.once("listening", () => {
      server.close(() => resolve(true));
    });
    server.listen(port, "127.0.0.1");
  });
}

export async function checkDeploymentHealth(spec: DeploymentSpec): Promise<DeploymentHealth> {
  if (!spec.runtime.healthcheck.enabled || !spec.runtime.healthcheck.url) {
    return {
      status: "disabled",
      url: spec.runtime.healthcheck.url,
      checkedAt: new Date().toISOString(),
      statusCode: null,
      error: null,
    };
  }

  let last: DeploymentHealth = {
    status: "unknown",
    url: spec.runtime.healthcheck.url,
    checkedAt: new Date().toISOString(),
    statusCode: null,
    error: null,
  };
  const baseUrl = spec.runtime.url.replace(/\/+$/, "");
  const candidates = healthcheckUrls(spec, baseUrl);

  for (let attempt = 0; attempt < spec.runtime.healthcheck.attempts; attempt += 1) {
    for (const candidateUrl of candidates) {
      last = await requestHealth(candidateUrl, spec.runtime.healthcheck.timeoutMs);
      if (
        last.statusCode !== null &&
        last.statusCode >= 200 &&
        last.statusCode <= spec.runtime.healthcheck.expectedStatusMax
      ) {
        return {
          ...last,
          status: "healthy",
        };
      }
    }
    if (attempt < spec.runtime.healthcheck.attempts - 1) {
      await delay(spec.runtime.healthcheck.intervalMs);
    }
  }

  return {
    ...last,
    status: "unhealthy",
  };
}

export async function checkDeploymentPreview(spec: DeploymentSpec): Promise<DeploymentHealth> {
  const previewPath = spec.runtimeContract.previewPath || "/";
  const url = `${spec.runtime.url.replace(/\/+$/, "")}${previewPath.startsWith("/") ? previewPath : `/${previewPath}`}`;
  let last: DeploymentHealth = {
    status: "unknown",
    url,
    checkedAt: new Date().toISOString(),
    statusCode: null,
    error: null,
  };
  for (let attempt = 0; attempt < spec.runtime.healthcheck.attempts; attempt += 1) {
    last = await requestPreview(url, spec.runtime.healthcheck.timeoutMs);
    if (last.statusCode !== null && last.statusCode >= 200 && last.statusCode <= 399 && last.error === null) {
      return {
        ...last,
        status: "healthy",
      };
    }
    if (attempt < spec.runtime.healthcheck.attempts - 1) {
      await delay(spec.runtime.healthcheck.intervalMs);
    }
  }
  return {
    ...last,
    status: "unhealthy",
  };
}

export async function checkDeploymentApiRoutes(spec: DeploymentSpec): Promise<DeploymentHealth> {
  const apiPaths = spec.topology.validation.apiPaths;
  if (apiPaths.length === 0) {
    return {
      status: "disabled",
      url: spec.runtime.url,
      checkedAt: new Date().toISOString(),
      statusCode: null,
      error: null,
    };
  }

  const baseUrl = spec.runtime.url.replace(/\/+$/, "");
  let last: DeploymentHealth = {
    status: "unknown",
    url: `${baseUrl}${normalizeHealthPath(apiPaths[0] ?? "/")}`,
    checkedAt: new Date().toISOString(),
    statusCode: null,
    error: null,
  };

  for (let attempt = 0; attempt < spec.runtime.healthcheck.attempts; attempt += 1) {
    let allHealthy = true;
    for (const apiPath of apiPaths) {
      const url = `${baseUrl}${normalizeHealthPath(apiPath)}`;
      const result = await requestText(url, spec.runtime.healthcheck.timeoutMs);
      last = apiRouteHealth(url, result);
      if (last.status !== "healthy") {
        allHealthy = false;
        break;
      }
    }
    if (allHealthy) {
      return last;
    }
    if (attempt < spec.runtime.healthcheck.attempts - 1) {
      await delay(spec.runtime.healthcheck.intervalMs);
    }
  }

  return {
    ...last,
    status: "unhealthy",
  };
}

type PreviewFetchResult = {
  health: DeploymentHealth;
  contentType: string;
  body: string;
};

export function applyHealthyPath(spec: DeploymentSpec, health: DeploymentHealth): DeploymentSpec {
  if (health.status !== "healthy" || !health.url) {
    return spec;
  }

  const pathName = pathFromHealthUrl(spec.runtime.url, health.url);
  if (!pathName || pathName === spec.runtime.healthcheck.path) {
    return spec;
  }

  return {
    ...spec,
    runtime: {
      ...spec.runtime,
      healthcheck: {
        ...spec.runtime.healthcheck,
        path: pathName,
        url: `${spec.runtime.url.replace(/\/+$/, "")}${pathName}`,
      },
    },
  };
}

function healthcheckUrls(spec: DeploymentSpec, baseUrl: string): string[] {
  const paths = [
    spec.runtime.healthcheck.path,
    ...spec.runtime.healthcheck.candidates,
  ];
  return [...new Set(paths.map((candidatePath) => normalizeHealthPath(candidatePath)))]
    .map((candidatePath) => `${baseUrl}${candidatePath}`);
}

function normalizeHealthPath(candidatePath: string): string {
  if (!candidatePath) {
    return "/";
  }
  return candidatePath.startsWith("/") ? candidatePath : `/${candidatePath}`;
}

function pathFromHealthUrl(baseUrl: string, healthUrl: string): string | null {
  try {
    const base = new URL(baseUrl);
    const health = new URL(healthUrl);
    if (base.origin !== health.origin) {
      return null;
    }
    return `${health.pathname || "/"}${health.search}`;
  } catch {
    return null;
  }
}

function requestHealth(url: string, timeoutMs: number): Promise<DeploymentHealth> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  return fetch(url, {
    method: "GET",
    signal: controller.signal,
  })
    .then(async (response) => {
      await response.body?.cancel();
      return {
        status: "unknown" as const,
        url,
        checkedAt: new Date().toISOString(),
        statusCode: response.status,
        error: null,
      };
    })
    .catch((error) => ({
      status: "unknown" as const,
      url,
      checkedAt: new Date().toISOString(),
      statusCode: null,
      error: error instanceof Error ? error.message : String(error),
    }))
    .finally(() => clearTimeout(timeout));
}

async function requestPreview(url: string, timeoutMs: number): Promise<DeploymentHealth> {
  const result = await requestText(url, timeoutMs);
  if (result.health.statusCode === null || result.health.statusCode < 200 || result.health.statusCode > 399) {
    return result.health;
  }

  const error = await validatePreviewBody(url, result, timeoutMs);
  return {
    ...result.health,
    error,
  };
}

function apiRouteHealth(url: string, result: PreviewFetchResult): DeploymentHealth {
  const statusCode = result.health.statusCode;
  if (statusCode === null) {
    return {
      ...result.health,
      status: "unhealthy",
      error: result.health.error ?? `API route ${url} could not be reached.`,
    };
  }
  if ((statusCode >= 200 && statusCode <= 399) || [400, 401, 403, 405].includes(statusCode)) {
    if (statusCode >= 200 && statusCode <= 399 && isHtmlResponse(result.contentType, result.body)) {
      return {
        ...result.health,
        status: "unhealthy",
        error: `API route ${url} returned HTML instead of an API response. The public entry is likely serving the frontend fallback instead of proxying to the backend.`,
      };
    }
    return {
      ...result.health,
      status: "healthy",
      error: null,
    };
  }
  return {
    ...result.health,
    status: "unhealthy",
    error: `API route ${url} returned HTTP ${statusCode}.`,
  };
}

async function requestText(url: string, timeoutMs: number): Promise<PreviewFetchResult> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const response = await fetch(url, {
      method: "GET",
      signal: controller.signal,
    });
    return {
      health: {
        status: "unknown",
        url,
        checkedAt: new Date().toISOString(),
        statusCode: response.status,
        error: null,
      },
      contentType: response.headers.get("content-type") ?? "",
      body: await response.text(),
    };
  } catch (error) {
    return {
      health: {
        status: "unknown",
        url,
        checkedAt: new Date().toISOString(),
        statusCode: null,
        error: error instanceof Error ? error.message : String(error),
      },
      contentType: "",
      body: "",
    };
  } finally {
    clearTimeout(timeout);
  }
}

async function validatePreviewBody(url: string, result: PreviewFetchResult, timeoutMs: number): Promise<string | null> {
  if (!isHtmlResponse(result.contentType, result.body)) {
    return null;
  }

  const scriptBodies: string[] = [];
  for (const scriptUrl of extractModuleScriptUrls(result.body, url).slice(0, 12)) {
    const script = await requestText(scriptUrl, timeoutMs);
    if (script.health.statusCode === null || script.health.statusCode < 200 || script.health.statusCode > 399) {
      return `Preview HTML loaded, but module script ${scriptUrl} did not load successfully.`;
    }
    if (isJavaScriptResponse(script.contentType, script.body)) {
      scriptBodies.push(script.body);
    }
  }

  if (needsReactRefreshPreamble(scriptBodies) && !hasReactRefreshPreamble(result.body)) {
    return "Preview JavaScript requires the React Fast Refresh preamble, but the served HTML did not include it. The page can return HTTP 200 while rendering a blank root.";
  }

  return null;
}

function isHtmlResponse(contentType: string, body: string): boolean {
  return contentType.toLowerCase().includes("text/html") || /^\s*<!doctype html|^\s*<html[\s>]/i.test(body);
}

function isJavaScriptResponse(contentType: string, body: string): boolean {
  const normalized = contentType.toLowerCase();
  return normalized.includes("javascript") || /\$Refresh(Sig|Reg)\$\s*\(/.test(body);
}

function extractModuleScriptUrls(html: string, baseUrl: string): string[] {
  const urls: string[] = [];
  const scriptPattern = /<script\b[^>]*\bsrc=(["'])([^"']+)\1[^>]*>/gi;
  let match: RegExpExecArray | null;
  while ((match = scriptPattern.exec(html)) !== null) {
    const tag = match[0];
    const src = match[2];
    if (!/\btype=(["'])module\1/i.test(tag) && !src.startsWith("/@vite/") && !src.includes("/assets/")) {
      continue;
    }
    try {
      const resolved = new URL(src, baseUrl);
      const base = new URL(baseUrl);
      if (resolved.origin === base.origin) {
        urls.push(resolved.toString());
      }
    } catch {
      // Ignore malformed script URLs; the browser will fail them separately.
    }
  }
  return [...new Set(urls)];
}

function needsReactRefreshPreamble(scriptBodies: string[]): boolean {
  return scriptBodies.some((body) => /\$Refresh(Sig|Reg)\$\s*\(/.test(body));
}

function hasReactRefreshPreamble(html: string): boolean {
  return html.includes("/@react-refresh") || /\$Refresh(Sig|Reg)\$/.test(html);
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}
