import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument } from "../errors";
import { pathExists } from "../state/fs";
import { toProjectRelative } from "../state/paths";
import { detectDeploymentCodeProbe } from "./detect";
import { findExistingDeploymentFiles } from "./existing";
import type {
  DeploymentWorkspace,
  DeploymentWorkspaceCandidate,
  DeploymentCodeProbe,
} from "./types";

type PackageJson = {
  workspaces?: string[] | {
    packages?: string[];
  };
};

const ROOT_MARKER_FILES = [
  "pnpm-workspace.yaml",
  "turbo.json",
  "nx.json",
  "lerna.json",
  "rush.json",
];

const COMMON_WORKSPACE_PATTERNS = [
  "apps/*",
  "packages/*",
  "services/*",
  "sites/*",
  "web",
  "frontend",
  "backend",
  "api",
];

const IGNORED_DIRECTORIES = new Set([
  ".git",
  ".hg",
  ".svn",
  ".loom",
  ".next",
  ".nuxt",
  ".output",
  ".turbo",
  ".vercel",
  "coverage",
  "dist",
  "build",
  "node_modules",
  "vendor",
  ".venv",
  "__pycache__",
  "tmp",
  "target",
  "bin",
  "obj",
]);

export async function resolveDeploymentWorkspace(projectRoot: string): Promise<{
  deploymentRoot: string;
  workspace: DeploymentWorkspace;
  codeProbe: DeploymentCodeProbe;
}> {
  return resolveDeploymentWorkspaceForApp(projectRoot, null);
}

export async function discoverNodeWorkspacePackageJsonPaths(projectRoot: string): Promise<string[]> {
  const packageJsonPaths = new Set<string>();
  for (const pattern of await workspacePatterns(projectRoot)) {
    for (const candidatePath of await expandWorkspacePattern(projectRoot, pattern)) {
      if (await pathExists(path.join(projectRoot, candidatePath, "package.json"))) {
        packageJsonPaths.add(`${candidatePath}/package.json`);
      }
    }
  }
  return [...packageJsonPaths].sort(comparePaths);
}

export async function resolveDeploymentWorkspaceForApp(
  projectRoot: string,
  appPath: string | null,
): Promise<{
  deploymentRoot: string;
  workspace: DeploymentWorkspace;
  codeProbe: DeploymentCodeProbe;
}> {
  if (appPath) {
    return resolveExplicitDeploymentWorkspace(projectRoot, appPath);
  }

  const rootCodeProbe = await detectDeploymentCodeProbe(projectRoot);
  const rootExisting = await findExistingDeploymentFiles(projectRoot);
  if (rootExisting.composePath || rootExisting.dockerfilePath || isDirectDeployable(rootCodeProbe)) {
    return {
      deploymentRoot: projectRoot,
      codeProbe: rootCodeProbe,
      workspace: {
        appPath: ".",
        isWorkspace: await hasWorkspaceRootSignals(projectRoot),
        buildContextPath: ".",
        reason: rootExisting.composePath
          ? "Using project root because it already has a Compose file."
          : rootExisting.dockerfilePath
            ? "Using project root because it already has a Dockerfile."
            : "Using project root because it is directly deployable.",
        candidates: [
          toWorkspaceCandidate({
            projectRoot,
            candidateRoot: projectRoot,
            codeProbe: rootCodeProbe,
            score: directDeployableScore(rootCodeProbe),
            signals: rootExisting.composePath || rootExisting.dockerfilePath
              ? ["existing-deployment-assets"]
              : ["direct-project-root"],
          }),
        ],
      },
    };
  }

  const rootSignals = await workspaceRootSignals(projectRoot);
  if (rootSignals.length === 0) {
    return {
      deploymentRoot: projectRoot,
      codeProbe: rootCodeProbe,
      workspace: {
        appPath: ".",
        isWorkspace: false,
        buildContextPath: ".",
        reason: "No workspace root markers or directly deployable app candidates were found.",
        candidates: [],
      },
    };
  }

  const candidates = await findWorkspaceCandidates(projectRoot, rootSignals);
  const selected = candidates[0];
  if (!selected) {
    return {
      deploymentRoot: projectRoot,
      codeProbe: rootCodeProbe,
      workspace: {
        appPath: ".",
        isWorkspace: true,
        buildContextPath: ".",
        reason: `Workspace markers detected (${rootSignals.join(", ")}), but no deployable app directory was found.`,
        candidates: [],
      },
    };
  }

  return {
    deploymentRoot: path.resolve(projectRoot, selected.path),
    codeProbe: await detectDeploymentCodeProbe(path.resolve(projectRoot, selected.path)),
    workspace: {
      appPath: selected.path,
      isWorkspace: true,
      buildContextPath: selected.path,
      reason: `Workspace markers detected (${rootSignals.join(", ")}); selected ${selected.path} as the highest-scoring deployable app.`,
      candidates,
    },
  };
}

async function resolveExplicitDeploymentWorkspace(
  projectRoot: string,
  appPath: string,
): Promise<{
  deploymentRoot: string;
  workspace: DeploymentWorkspace;
  codeProbe: DeploymentCodeProbe;
}> {
  const normalizedAppPath = normalizeAppPath(appPath);
  const deploymentRoot = path.resolve(projectRoot, normalizedAppPath);
  const projectRootResolved = path.resolve(projectRoot);
  if (!isInsideProject(projectRootResolved, deploymentRoot)) {
    throw invalidArgument("--app-path must stay inside --project-root.", {
      projectRoot,
      appPath,
    });
  }
  if (!(await isDirectory(deploymentRoot))) {
    throw invalidArgument("--app-path must point to an existing directory.", {
      projectRoot,
      appPath,
    });
  }

  const codeProbe = await detectDeploymentCodeProbe(deploymentRoot);
  const existing = await findExistingDeploymentFiles(deploymentRoot);
  const candidate = toWorkspaceCandidate({
    projectRoot,
    candidateRoot: deploymentRoot,
    codeProbe,
    score: scoreCandidate(codeProbe, existing, normalizedAppPath, ["explicit-app-path"]),
    signals: candidateSignals(codeProbe, existing, normalizedAppPath, ["explicit-app-path"]),
  });

  return {
    deploymentRoot,
    codeProbe,
    workspace: {
      appPath: normalizedAppPath,
      isWorkspace: normalizedAppPath !== ".",
      buildContextPath: normalizedAppPath,
      reason: `Using explicit app path ${normalizedAppPath}.`,
      candidates: [candidate],
    },
  };
}

async function findWorkspaceCandidates(
  projectRoot: string,
  rootSignals: string[],
): Promise<DeploymentWorkspaceCandidate[]> {
  const patterns = await workspacePatterns(projectRoot);
  const candidatePaths = new Set<string>();

  for (const pattern of [...patterns, ...COMMON_WORKSPACE_PATTERNS]) {
    for (const candidatePath of await expandWorkspacePattern(projectRoot, pattern)) {
      candidatePaths.add(candidatePath);
    }
  }

  for (const candidatePath of await shallowDeployableDirectories(projectRoot)) {
    candidatePaths.add(candidatePath);
  }

  const candidates: DeploymentWorkspaceCandidate[] = [];
  for (const candidatePath of [...candidatePaths].sort(comparePaths)) {
    const candidateRoot = path.resolve(projectRoot, candidatePath);
    const codeProbe = await detectDeploymentCodeProbe(candidateRoot);
    const existing = await findExistingDeploymentFiles(candidateRoot);
    const signals = candidateSignals(codeProbe, existing, candidatePath, rootSignals);
    const score = scoreCandidate(codeProbe, existing, candidatePath, signals);
    if (score <= 0) {
      continue;
    }

    candidates.push(toWorkspaceCandidate({
      projectRoot,
      candidateRoot,
      codeProbe,
      score,
      signals,
    }));
  }

  return candidates.sort((left, right) => {
    if (right.score !== left.score) {
      return right.score - left.score;
    }
    return comparePaths(left.path, right.path);
  });
}

async function workspacePatterns(projectRoot: string): Promise<string[]> {
  return dedupeStrings([
    ...await packageJsonWorkspacePatterns(projectRoot),
    ...await pnpmWorkspacePatterns(projectRoot),
  ]);
}

async function packageJsonWorkspacePatterns(projectRoot: string): Promise<string[]> {
  const packageJsonPath = path.join(projectRoot, "package.json");
  if (!(await pathExists(packageJsonPath))) {
    return [];
  }

  try {
    const pkg = JSON.parse(await fs.readFile(packageJsonPath, "utf8")) as PackageJson;
    if (Array.isArray(pkg.workspaces)) {
      return pkg.workspaces;
    }
    if (Array.isArray(pkg.workspaces?.packages)) {
      return pkg.workspaces.packages;
    }
  } catch {
    return [];
  }

  return [];
}

async function pnpmWorkspacePatterns(projectRoot: string): Promise<string[]> {
  const workspacePath = path.join(projectRoot, "pnpm-workspace.yaml");
  if (!(await pathExists(workspacePath))) {
    return [];
  }

  const raw = await fs.readFile(workspacePath, "utf8");
  const patterns: string[] = [];
  let inPackages = false;
  let packageIndent = 0;

  for (const line of raw.split(/\r?\n/)) {
    const withoutComment = line.replace(/\s+#.*$/, "");
    if (!withoutComment.trim()) {
      continue;
    }

    const indent = withoutComment.search(/\S/);
    const trimmed = withoutComment.trim();
    if (/^packages\s*:/.test(trimmed)) {
      inPackages = true;
      packageIndent = indent;
      const inlineMatch = trimmed.match(/^packages\s*:\s*\[(.*)]/);
      if (inlineMatch) {
        patterns.push(...inlineMatch[1]
          .split(",")
          .map((value) => unquote(value.trim()))
          .filter(Boolean));
      }
      continue;
    }

    if (!inPackages) {
      continue;
    }
    if (indent <= packageIndent && !trimmed.startsWith("-")) {
      inPackages = false;
      continue;
    }
    if (trimmed.startsWith("-")) {
      const value = unquote(trimmed.slice(1).trim());
      if (value && !value.startsWith("!")) {
        patterns.push(value);
      }
    }
  }

  return patterns;
}

async function expandWorkspacePattern(projectRoot: string, pattern: string): Promise<string[]> {
  const normalized = pattern.trim().replace(/\\/g, "/");
  if (!normalized || normalized.startsWith("!") || normalized.includes("..")) {
    return [];
  }

  const segments = normalized.split("/").filter(Boolean);
  const expanded = await expandSegments(projectRoot, segments);
  return expanded
    .map((candidate) => toProjectRelative(projectRoot, candidate))
    .filter((candidate) => candidate && candidate !== ".")
    .filter((candidate) => !candidate.split("/").some((part) => IGNORED_DIRECTORIES.has(part)));
}

async function expandSegments(currentRoot: string, segments: string[]): Promise<string[]> {
  if (segments.length === 0) {
    return [currentRoot];
  }

  const [segment, ...rest] = segments;
  if (segment === "*") {
    const entries = await safeReadDir(currentRoot);
    const directories = entries
      .filter((entry) => entry.isDirectory() && !IGNORED_DIRECTORIES.has(entry.name))
      .sort((left, right) => left.name.localeCompare(right.name));
    const expanded: string[] = [];
    for (const directory of directories) {
      expanded.push(...await expandSegments(path.join(currentRoot, directory.name), rest));
    }
    return expanded;
  }

  if (segment.includes("*")) {
    return [];
  }

  const nextRoot = path.join(currentRoot, segment);
  if (!(await isDirectory(nextRoot))) {
    return [];
  }
  return expandSegments(nextRoot, rest);
}

async function shallowDeployableDirectories(projectRoot: string): Promise<string[]> {
  const candidates: string[] = [];
  const entries = await safeReadDir(projectRoot);
  for (const entry of entries) {
    if (!entry.isDirectory() || IGNORED_DIRECTORIES.has(entry.name)) {
      continue;
    }

    const firstLevel = path.join(projectRoot, entry.name);
    if (await hasDeployableSignals(firstLevel)) {
      candidates.push(entry.name);
    }

    if (["apps", "packages", "services", "sites"].includes(entry.name)) {
      for (const nested of await safeReadDir(firstLevel)) {
        if (nested.isDirectory() && !IGNORED_DIRECTORIES.has(nested.name)) {
          const nestedRoot = path.join(firstLevel, nested.name);
          if (await hasDeployableSignals(nestedRoot)) {
            candidates.push(`${entry.name}/${nested.name}`);
          }
        }
      }
    }
  }
  return candidates;
}

async function hasDeployableSignals(candidateRoot: string): Promise<boolean> {
  const files = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
    "Dockerfile",
    "dockerfile",
    "package.json",
    "requirements.txt",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "App.csproj",
    "Gemfile",
    "composer.json",
    "index.html",
  ];
  if (await hasAnyFile(candidateRoot, files)) {
    return true;
  }

  const entries = await safeReadDir(candidateRoot);
  return entries.some((entry) => entry.isFile() && entry.name.endsWith(".csproj"));
}

async function workspaceRootSignals(projectRoot: string): Promise<string[]> {
  const signals: string[] = [];
  for (const file of ROOT_MARKER_FILES) {
    if (await pathExists(path.join(projectRoot, file))) {
      signals.push(file);
    }
  }
  if ((await packageJsonWorkspacePatterns(projectRoot)).length > 0) {
    signals.push("package.json workspaces");
  }
  return dedupeStrings(signals);
}

async function hasWorkspaceRootSignals(projectRoot: string): Promise<boolean> {
  return (await workspaceRootSignals(projectRoot)).length > 0;
}

function isDirectDeployable(codeProbe: DeploymentCodeProbe): boolean {
  if (codeProbe.kind === "unknown") {
    return false;
  }
  if (codeProbe.kind === "node" && codeProbe.framework === "node-cli" && !codeProbe.startCommand) {
    return false;
  }
  return true;
}

function directDeployableScore(codeProbe: DeploymentCodeProbe): number {
  return isDirectDeployable(codeProbe) ? 100 : 0;
}

function scoreCandidate(
  codeProbe: DeploymentCodeProbe,
  existing: Awaited<ReturnType<typeof findExistingDeploymentFiles>>,
  candidatePath: string,
  signals: string[],
): number {
  const hasExistingDeploymentAssets = Boolean(existing.composePath || existing.dockerfilePath);
  if (!hasExistingDeploymentAssets && !isDirectDeployable(codeProbe)) {
    return 0;
  }

  let score = 0;
  if (existing.composePath) {
    score += 100;
  }
  if (existing.dockerfilePath) {
    score += 80;
  }
  if (isDirectDeployable(codeProbe)) {
    score += 50;
  }
  if (codeProbe.startCommand) {
    score += 20;
  }
  if (codeProbe.buildCommand) {
    score += 10;
  }
  if (codeProbe.framework && !["node-cli", "unknown"].includes(codeProbe.framework)) {
    score += 10;
  }
  if (codeProbe.services.length > 0) {
    score += Math.min(codeProbe.services.length * 3, 9);
  }
  if (candidatePath.startsWith("apps/")) {
    score += 8;
  }
  if (candidatePath.startsWith("services/")) {
    score += 6;
  }
  if (/^(web|frontend|app|api|server|backend)$/.test(path.basename(candidatePath))) {
    score += 6;
  }
  if (signals.includes("workspace-pattern")) {
    score += 4;
  }

  return score;
}

function candidateSignals(
  codeProbe: DeploymentCodeProbe,
  existing: Awaited<ReturnType<typeof findExistingDeploymentFiles>>,
  candidatePath: string,
  rootSignals: string[],
): string[] {
  return dedupeStrings([
    "workspace-pattern",
    ...rootSignals,
    ...(existing.composePath ? ["compose"] : []),
    ...(existing.dockerfilePath ? ["dockerfile"] : []),
    codeProbe.kind,
    ...(codeProbe.framework ? [codeProbe.framework] : []),
    ...(codeProbe.packageManager ? [codeProbe.packageManager] : []),
    ...(candidatePath.startsWith("apps/") ? ["apps-directory"] : []),
    ...(candidatePath.startsWith("services/") ? ["services-directory"] : []),
  ].filter((signal) => signal && signal !== "unknown"));
}

function toWorkspaceCandidate(input: {
  projectRoot: string;
  candidateRoot: string;
  codeProbe: DeploymentCodeProbe;
  score: number;
  signals: string[];
}): DeploymentWorkspaceCandidate {
  return {
    path: toProjectRelative(input.projectRoot, input.candidateRoot) || ".",
    score: input.score,
    runtimeKind: input.codeProbe.kind,
    framework: input.codeProbe.framework,
    packageManager: input.codeProbe.packageManager,
    signals: input.signals,
  };
}

async function hasAnyFile(directory: string, files: string[]): Promise<boolean> {
  for (const file of files) {
    if (await pathExists(path.join(directory, file))) {
      return true;
    }
  }
  return false;
}

async function isDirectory(filePath: string): Promise<boolean> {
  try {
    return (await fs.stat(filePath)).isDirectory();
  } catch {
    return false;
  }
}

async function safeReadDir(directory: string): Promise<import("node:fs").Dirent[]> {
  try {
    return await fs.readdir(directory, { withFileTypes: true });
  } catch {
    return [];
  }
}

function unquote(value: string): string {
  return value.replace(/^["']|["']$/g, "").trim();
}

function normalizeAppPath(appPath: string): string {
  const normalized = appPath.trim().replace(/\\/g, "/").replace(/^\.\/+/, "").replace(/\/+$/, "");
  if (!normalized || normalized === ".") {
    return ".";
  }
  if (path.isAbsolute(normalized) || normalized.split("/").some((part) => part === "..")) {
    throw invalidArgument("--app-path must be relative to --project-root.", { appPath });
  }
  return normalized;
}

function isInsideProject(projectRoot: string, candidatePath: string): boolean {
  const relative = path.relative(projectRoot, candidatePath);
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function dedupeStrings(values: string[]): string[] {
  return [...new Set(values)];
}

function comparePaths(left: string, right: string): number {
  return left.localeCompare(right);
}
