import { promises as fs } from "node:fs";
import path from "node:path";
import type { DeploymentCodeProbe } from "./types";
import { pathExists } from "../state/fs";
import {
  dedupeDependencyServices,
  dependencyServiceKindsFromLooseSignals,
  detectedDependencyReason,
  normalizeDependencyConnectionEnv,
  serviceDefinition,
} from "./dependency-signals";

type PackageJson = {
  scripts?: Record<string, string>;
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
  engines?: {
    node?: string;
  };
  volta?: {
    node?: string;
  };
};

type ComposerJson = {
  require?: Record<string, string>;
  "require-dev"?: Record<string, string>;
  scripts?: Record<string, string | string[]>;
};

const DEFAULT_NODE_MAJOR_VERSION = 22;
const PREFERRED_NODE_MAJOR_VERSIONS = [24, 22, 20, 18, 16, 14] as const;

export async function detectDeploymentCodeProbe(projectRoot: string): Promise<DeploymentCodeProbe> {
  if (await hasPhpSignals(projectRoot)) {
    return detectPhpStack(projectRoot);
  }

  if (await hasRubySignals(projectRoot)) {
    return detectRubyStack(projectRoot);
  }

  const packageJsonPath = path.join(projectRoot, "package.json");
  if (await pathExists(packageJsonPath)) {
    return detectNodeStack(projectRoot, packageJsonPath);
  }

  if (await hasPythonSignals(projectRoot)) {
    return detectPythonStack(projectRoot);
  }

  if (await pathExists(path.join(projectRoot, "go.mod"))) {
    return detectGoStack(projectRoot);
  }

  if (await hasJavaSignals(projectRoot)) {
    return detectJavaStack(projectRoot);
  }

  if (await hasDotnetSignals(projectRoot)) {
    return detectDotnetStack(projectRoot);
  }

  const staticIndexPath = path.join(projectRoot, "index.html");
  if (await pathExists(staticIndexPath)) {
    return {
      kind: "static",
      packageManager: null,
      hasLockfile: false,
      framework: "static-html",
      runtimeVersion: null,
      runtimeVersionSource: null,
      buildCommand: null,
      startCommand: null,
      outputDirectory: ".",
      port: 80,
      services: [],
      workingDirectory: null,
    };
  }

  return {
    kind: "unknown",
    packageManager: null,
    hasLockfile: false,
    framework: null,
    runtimeVersion: null,
    runtimeVersionSource: null,
    buildCommand: null,
    startCommand: null,
    outputDirectory: null,
    port: 3000,
    services: [],
    workingDirectory: null,
  };
}

async function detectNodeStack(projectRoot: string, packageJsonPath: string): Promise<DeploymentCodeProbe> {
  const pkg = await readPackageJson(packageJsonPath);
  const scripts = pkg.scripts ?? {};
  const deps = {
    ...(pkg.dependencies ?? {}),
    ...(pkg.devDependencies ?? {}),
  };
  const packageManager = await detectPackageManager(projectRoot);
  const framework = detectFramework(deps, scripts);
  const outputDirectory = detectOutputDirectory(framework);
  const services = await detectDependencyServices(projectRoot, deps, scripts);
  const runtimeVersion = await detectNodeRuntimeVersion(projectRoot, pkg);
  const usesNextStandalone = framework === "next" && await hasNextStandaloneOutput(projectRoot);

  const port = detectPort(framework);

  return {
    kind: "node",
    packageManager: packageManager.name,
    hasLockfile: packageManager.hasLockfile,
    framework,
    runtimeVersion: runtimeVersion.version,
    runtimeVersionSource: runtimeVersion.source,
    buildCommand: scripts.build ? packageManagerRun(packageManager.name, "build") : null,
    startCommand: detectStartCommand(packageManager.name, scripts, framework, port, usesNextStandalone),
    outputDirectory,
    port,
    services,
    workingDirectory: null,
  };
}

async function detectPythonStack(projectRoot: string): Promise<DeploymentCodeProbe> {
  const signals = await readPythonSignals(projectRoot);
  const framework = detectPythonFramework(signals);
  const packageManager = await detectPythonPackageManager(projectRoot);
  const port = detectPythonPort(framework, signals);
  const startCommand = await detectPythonStartCommand(projectRoot, framework, port);

  return {
    kind: "python",
    packageManager,
    hasLockfile: await hasPythonLockfile(projectRoot),
    framework,
    runtimeVersion: null,
    runtimeVersionSource: null,
    buildCommand: null,
    startCommand,
    outputDirectory: null,
    port,
    healthcheckPath: detectHealthcheckPath(signals),
    services: await detectDependencyServices(projectRoot, signals),
    workingDirectory: null,
  };
}

async function detectGoStack(projectRoot: string): Promise<DeploymentCodeProbe> {
  const signals = await readProjectSignals(projectRoot, [
    "go.mod",
    "go.sum",
    ".env.example",
    ".env.sample",
    ".env.local.example",
    ".env",
  ]);

  return {
    kind: "go",
    packageManager: "go",
    hasLockfile: await pathExists(path.join(projectRoot, "go.sum")),
    framework: detectGoFramework(signals),
    runtimeVersion: null,
    runtimeVersionSource: null,
    buildCommand: "go build -o /app/server .",
    startCommand: "/app/server",
    outputDirectory: null,
    port: detectPortFromSignals(signals) ?? 8080,
    services: await detectDependencyServices(projectRoot, signals),
    workingDirectory: null,
  };
}

async function detectJavaStack(projectRoot: string): Promise<DeploymentCodeProbe> {
  const signals = await readProjectSignals(projectRoot, [
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "gradle.properties",
    "mvnw",
    "gradlew",
    "src/main/resources/application.properties",
    "src/main/resources/application.yml",
    "src/main/resources/application.yaml",
    ".env.example",
    ".env.sample",
    ".env.local.example",
    ".env",
  ]);
  const packageManager = await detectJavaPackageManager(projectRoot);
  const runtimeVersion = detectJavaRuntimeVersion(signals);

  return {
    kind: "java",
    packageManager,
    hasLockfile: await hasJavaLockfile(projectRoot),
    framework: detectJavaFramework(signals),
    runtimeVersion: runtimeVersion.version,
    runtimeVersionSource: runtimeVersion.source,
    buildCommand: javaBuildCommand(packageManager),
    startCommand: "java -jar /app/app.jar",
    outputDirectory: javaOutputDirectory(packageManager),
    port: detectJavaPort(signals),
    services: await detectDependencyServices(projectRoot, signals),
    workingDirectory: null,
  };
}

async function detectDotnetStack(projectRoot: string): Promise<DeploymentCodeProbe> {
  const projectFile = await findDotnetProjectFile(projectRoot);
  const signals = await readProjectSignals(projectRoot, [
    projectFile ?? "",
    "global.json",
    "appsettings.json",
    "appsettings.Development.json",
    ".env.example",
    ".env.sample",
    ".env.local.example",
    ".env",
  ].filter(Boolean));
  const projectName = projectFile ? path.basename(projectFile, path.extname(projectFile)) : path.basename(projectRoot);
  const runtimeVersion = detectDotnetRuntimeVersion(signals);

  return {
    kind: "dotnet",
    packageManager: "dotnet",
    hasLockfile: await pathExists(path.join(projectRoot, "packages.lock.json")),
    framework: detectDotnetFramework(signals),
    runtimeVersion: runtimeVersion.version,
    runtimeVersionSource: runtimeVersion.source,
    buildCommand: "dotnet publish -c Release -o /app/publish",
    startCommand: `dotnet /app/${projectName}.dll`,
    outputDirectory: "bin/Release",
    port: detectDotnetPort(signals),
    services: await detectDependencyServices(projectRoot, signals),
    workingDirectory: null,
  };
}

async function detectPhpStack(projectRoot: string): Promise<DeploymentCodeProbe> {
  const composerPath = path.join(projectRoot, "composer.json");
  const composer = await readComposerJson(composerPath);
  const deps = {
    ...(composer.require ?? {}),
    ...(composer["require-dev"] ?? {}),
  };
  const signals = `${Object.keys(deps).join("\n")}\n${Object.values(composer.scripts ?? {}).flat().join("\n")}\n${await readPhpSignals(projectRoot)}`;
  const framework = detectPhpFramework(signals, projectRoot);

  return {
    kind: "php",
    packageManager: "composer",
    hasLockfile: await pathExists(path.join(projectRoot, "composer.lock")),
    framework,
    runtimeVersion: detectPhpRuntimeVersion(deps),
    runtimeVersionSource: deps.php ? "composer.json require.php" : "default",
    buildCommand: "composer install --no-dev --prefer-dist --no-interaction --optimize-autoloader",
    startCommand: detectPhpStartCommand(framework),
    outputDirectory: framework === "laravel" ? "public" : ".",
    port: detectPhpPort(signals),
    services: await detectDependencyServices(projectRoot, signals),
    workingDirectory: null,
  };
}

async function detectRubyStack(projectRoot: string): Promise<DeploymentCodeProbe> {
  const signals = await readProjectSignals(projectRoot, [
    "Gemfile",
    "Gemfile.lock",
    ".ruby-version",
    "config/application.rb",
    "config/puma.rb",
    "config/database.yml",
    ".env.example",
    ".env.sample",
    ".env.local.example",
    ".env",
  ]);
  const framework = detectRubyFramework(signals);

  return {
    kind: "ruby",
    packageManager: "bundler",
    hasLockfile: await pathExists(path.join(projectRoot, "Gemfile.lock")),
    framework,
    runtimeVersion: await detectRubyRuntimeVersion(projectRoot, signals),
    runtimeVersionSource: await rubyRuntimeVersionSource(projectRoot, signals),
    buildCommand: "bundle install",
    startCommand: detectRubyStartCommand(framework),
    outputDirectory: null,
    port: detectRubyPort(signals),
    services: await detectDependencyServices(projectRoot, signals),
    workingDirectory: null,
  };
}

async function readPackageJson(filePath: string): Promise<PackageJson> {
  const raw = await fs.readFile(filePath, "utf8");
  return JSON.parse(raw) as PackageJson;
}

async function readComposerJson(filePath: string): Promise<ComposerJson> {
  const raw = await fs.readFile(filePath, "utf8");
  return JSON.parse(raw) as ComposerJson;
}

async function detectNodeRuntimeVersion(
  projectRoot: string,
  pkg: PackageJson,
): Promise<{ version: string; source: string }> {
  const candidates = [
    { source: "package.json engines.node", value: pkg.engines?.node, kind: "engine" },
    { source: "package.json volta.node", value: pkg.volta?.node, kind: "exact" },
    { source: ".nvmrc", value: await readOptionalFile(path.join(projectRoot, ".nvmrc")), kind: "exact" },
    { source: ".node-version", value: await readOptionalFile(path.join(projectRoot, ".node-version")), kind: "exact" },
    { source: ".tool-versions", value: await readToolVersionsNode(projectRoot), kind: "exact" },
  ] as const;

  for (const candidate of candidates) {
    if (!candidate.value) {
      continue;
    }

    const major = candidate.kind === "engine"
      ? detectNodeMajorFromEngineRange(candidate.value)
      : detectNodeMajorFromVersion(candidate.value);
    if (major !== null) {
      return {
        version: String(major),
        source: candidate.source,
      };
    }
  }

  return {
    version: String(DEFAULT_NODE_MAJOR_VERSION),
    source: "default",
  };
}

async function readOptionalFile(filePath: string): Promise<string | null> {
  if (!(await pathExists(filePath))) {
    return null;
  }
  return fs.readFile(filePath, "utf8");
}

async function readToolVersionsNode(projectRoot: string): Promise<string | null> {
  const raw = await readOptionalFile(path.join(projectRoot, ".tool-versions"));
  if (!raw) {
    return null;
  }

  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }
    const [tool, version] = trimmed.split(/\s+/, 2);
    if ((tool === "nodejs" || tool === "node") && version) {
      return version;
    }
  }
  return null;
}

function detectNodeMajorFromEngineRange(value: string): number | null {
  const exactMajor = detectExactNodeMajor(value);
  if (exactMajor !== null) {
    return exactMajor;
  }

  const firstMajor = detectNodeMajorFromVersion(value);
  if (firstMajor === null) {
    return null;
  }

  if (engineRangeCanUseMajor(value, DEFAULT_NODE_MAJOR_VERSION)) {
    return DEFAULT_NODE_MAJOR_VERSION;
  }

  const preferredMajor = PREFERRED_NODE_MAJOR_VERSIONS.find((major) => engineRangeCanUseMajor(value, major));
  if (preferredMajor !== undefined) {
    return preferredMajor;
  }

  return firstMajor;
}

function detectExactNodeMajor(value: string): number | null {
  const normalized = value.trim();
  const match = normalized.match(/^(?:node@|v)?(\d{1,3})(?:\.(?:\d{1,3}|x|\*))?(?:\.(?:\d{1,3}|x|\*))?$/i);
  return match ? Number(match[1]) : null;
}

function detectNodeMajorFromVersion(value: string): number | null {
  const match = value.trim().match(/(?:^|[^\d])(?:node@|v)?(\d{1,3})(?:\.\d{1,3}){0,2}/i);
  return match ? Number(match[1]) : null;
}

function engineRangeCanUseMajor(value: string, targetMajor: number): boolean {
  return value.split("||").some((alternative) => engineRangeAlternativeCanUseMajor(alternative, targetMajor));
}

function engineRangeAlternativeCanUseMajor(value: string, targetMajor: number): boolean {
  const trimmed = value.trim();
  const comparators = [...trimmed.matchAll(/(<=|<|>=|>)\s*v?(\d{1,3})(?:\.(\d{1,3}))?(?:\.(\d{1,3}))?/g)];

  if (comparators.length > 0) {
    return comparators.every((comparator) => {
      const operator = comparator[1];
      const major = Number(comparator[2]);
      if (operator === "<") {
        return targetMajor < major;
      }
      if (operator === "<=") {
        return targetMajor <= major;
      }
      if (operator === ">") {
        return targetMajor > major;
      }
      return targetMajor >= major;
    });
  }

  const major = detectNodeMajorFromVersion(trimmed);
  return major === targetMajor;
}

async function detectPackageManager(projectRoot: string): Promise<{
  name: NonNullable<DeploymentCodeProbe["packageManager"]>;
  hasLockfile: boolean;
}> {
  if ((await pathExists(path.join(projectRoot, "pnpm-lock.yaml"))) || (await pathExistsInWorkspaceAncestor(projectRoot, "pnpm-lock.yaml"))) {
    return { name: "pnpm", hasLockfile: true };
  }
  if ((await pathExists(path.join(projectRoot, "yarn.lock"))) || (await pathExistsInWorkspaceAncestor(projectRoot, "yarn.lock"))) {
    return { name: "yarn", hasLockfile: true };
  }
  if (
    (await pathExists(path.join(projectRoot, "bun.lock"))) ||
    (await pathExists(path.join(projectRoot, "bun.lockb"))) ||
    (await pathExistsInWorkspaceAncestor(projectRoot, "bun.lock")) ||
    (await pathExistsInWorkspaceAncestor(projectRoot, "bun.lockb"))
  ) {
    return { name: "bun", hasLockfile: true };
  }
  return {
    name: "npm",
    hasLockfile: (
      (await pathExists(path.join(projectRoot, "package-lock.json"))) ||
      (await pathExistsInWorkspaceAncestor(projectRoot, "package-lock.json"))
    ),
  };
}

async function detectPythonPackageManager(projectRoot: string): Promise<"pip" | "poetry" | "uv"> {
  if ((await pathExists(path.join(projectRoot, "uv.lock"))) || (await fileContains(path.join(projectRoot, "pyproject.toml"), "[tool.uv"))) {
    return "uv";
  }
  if ((await pathExists(path.join(projectRoot, "poetry.lock"))) || (await fileContains(path.join(projectRoot, "pyproject.toml"), "[tool.poetry"))) {
    return "poetry";
  }
  return "pip";
}

async function hasPythonLockfile(projectRoot: string): Promise<boolean> {
  return (
    (await pathExists(path.join(projectRoot, "uv.lock"))) ||
    (await pathExists(path.join(projectRoot, "poetry.lock"))) ||
    (await pathExists(path.join(projectRoot, "Pipfile.lock")))
  );
}

async function hasPythonSignals(projectRoot: string): Promise<boolean> {
  const candidates = [
    "requirements.txt",
    "pyproject.toml",
    "Pipfile",
    "uv.lock",
    "poetry.lock",
    "server.py",
    "main.py",
    "app.py",
    "manage.py",
  ];
  for (const candidate of candidates) {
    if (await pathExists(path.join(projectRoot, candidate))) {
      return true;
    }
  }
  return false;
}

async function readPythonSignals(projectRoot: string): Promise<string> {
  return [
    await readProjectSignals(projectRoot, [
      "requirements.txt",
      "pyproject.toml",
      "Pipfile",
      "uv.lock",
      "poetry.lock",
      "server.py",
      "main.py",
      "app.py",
      "manage.py",
      "README.md",
      "HTTP_API.md",
      ".env.example",
      ".env.sample",
      ".env.local.example",
      ".env",
    ]),
    await readPythonPackageSignals(projectRoot),
    await readProjectSignalsFromDirectory(projectRoot, "trading_system"),
  ].filter(Boolean).join("\n");
}

async function hasPhpSignals(projectRoot: string): Promise<boolean> {
  return pathExists(path.join(projectRoot, "composer.json"));
}

async function hasRubySignals(projectRoot: string): Promise<boolean> {
  return pathExists(path.join(projectRoot, "Gemfile"));
}

async function readPhpSignals(projectRoot: string): Promise<string> {
  return readProjectSignals(projectRoot, [
    "artisan",
    "public/index.php",
    "index.php",
    "config/app.php",
    "config/database.php",
    ".env.example",
    ".env.sample",
    ".env.local.example",
    ".env",
  ]);
}

async function hasJavaSignals(projectRoot: string): Promise<boolean> {
  const candidates = [
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "mvnw",
    "gradlew",
  ];
  for (const candidate of candidates) {
    if (await pathExists(path.join(projectRoot, candidate))) {
      return true;
    }
  }
  return false;
}

async function hasDotnetSignals(projectRoot: string): Promise<boolean> {
  return Boolean(await findDotnetProjectFile(projectRoot)) || Boolean(await findDotnetSolutionFile(projectRoot));
}

async function findDotnetProjectFile(projectRoot: string): Promise<string | null> {
  const entries = await safeReadDir(projectRoot);
  const project = entries.find((entry) => entry.isFile() && entry.name.endsWith(".csproj"));
  return project ? project.name : null;
}

async function findDotnetSolutionFile(projectRoot: string): Promise<string | null> {
  const entries = await safeReadDir(projectRoot);
  const solution = entries.find((entry) => entry.isFile() && entry.name.endsWith(".sln"));
  return solution ? solution.name : null;
}

async function safeReadDir(directory: string): Promise<import("node:fs").Dirent[]> {
  try {
    return await fs.readdir(directory, { withFileTypes: true });
  } catch {
    return [];
  }
}

async function detectJavaPackageManager(projectRoot: string): Promise<"maven" | "gradle"> {
  if ((await pathExists(path.join(projectRoot, "pom.xml"))) || (await pathExists(path.join(projectRoot, "mvnw")))) {
    return "maven";
  }
  return "gradle";
}

async function hasJavaLockfile(projectRoot: string): Promise<boolean> {
  return (
    (await pathExists(path.join(projectRoot, "gradle.lockfile"))) ||
    (await pathExists(path.join(projectRoot, "gradle/dependency-locks"))) ||
    (await pathExists(path.join(projectRoot, ".mvn/wrapper/maven-wrapper.properties"))) ||
    (await pathExists(path.join(projectRoot, "gradle/wrapper/gradle-wrapper.properties")))
  );
}

function detectFramework(deps: Record<string, string>, scripts: Record<string, string>): string {
  if ("next" in deps) {
    return "next";
  }
  if ("vite" in deps || scripts.dev?.includes("vite") || scripts.build?.includes("vite")) {
    return "vite";
  }
  if ("@sveltejs/kit" in deps) {
    return "sveltekit";
  }
  if ("astro" in deps) {
    return "astro";
  }
  if ("express" in deps || "fastify" in deps || "koa" in deps || "hono" in deps) {
    return "node-server";
  }
  if (!scripts.start && !scripts.preview) {
    return "node-cli";
  }
  return "node";
}

function detectOutputDirectory(framework: string): string | null {
  switch (framework) {
    case "vite":
      return "dist";
    case "astro":
      return "dist";
    default:
      return null;
  }
}

function detectStartCommand(
  packageManager: NonNullable<DeploymentCodeProbe["packageManager"]>,
  scripts: Record<string, string>,
  framework: string,
  port: number,
  usesNextStandalone = false,
): string | null {
  if (usesNextStandalone) {
    return "node .next/standalone/server.js";
  }
  if (framework === "vite" && scripts.preview) {
    return packageManagerRunWithArgs(packageManager, "preview", ["--host", "0.0.0.0", "--port", String(port)]);
  }
  if (scripts.start) {
    return packageManagerRun(packageManager, "start");
  }
  if (framework === "node-cli") {
    return null;
  }
  return null;
}

async function hasNextStandaloneOutput(projectRoot: string): Promise<boolean> {
  const candidates = [
    "next.config.js",
    "next.config.mjs",
    "next.config.cjs",
    "next.config.ts",
  ];

  for (const candidate of candidates) {
    const filePath = path.join(projectRoot, candidate);
    if (await pathExists(filePath)) {
      const raw = await fs.readFile(filePath, "utf8");
      if (/output\s*:\s*["']standalone["']/.test(raw)) {
        return true;
      }
    }
  }
  return false;
}

function detectPort(framework: string): number {
  switch (framework) {
    case "vite":
      return 4173;
    case "astro":
      return 4321;
    default:
      return 3000;
  }
}

async function detectDependencyServices(
  projectRoot: string,
  depsOrSignals: Record<string, string> | string,
  scripts: Record<string, string> = {},
): Promise<DeploymentCodeProbe["services"]> {
  const signals = typeof depsOrSignals === "string"
    ? `${depsOrSignals}\n${await readEnvSignals(projectRoot)}`.toLowerCase()
    : `${Object.keys(depsOrSignals).join("\n")}\n${Object.values(scripts).join("\n")}\n${await readEnvSignals(projectRoot)}`.toLowerCase();
  const services = dependencyServiceKindsFromLooseSignals(signals)
    .map((kind) => serviceDefinition(kind, detectedDependencyReason(kind)));
  return normalizeDependencyConnectionEnv(dedupeDependencyServices(services));
}

async function readEnvSignals(projectRoot: string): Promise<string> {
  const candidates = [".env.example", ".env.sample", ".env.local.example", ".env"];
  const values: string[] = [];
  for (const candidate of candidates) {
    const filePath = path.join(projectRoot, candidate);
    if (await pathExists(filePath)) {
      values.push(await fs.readFile(filePath, "utf8"));
    }
  }
  return values.join("\n");
}

function hasAny(value: string, needles: string[]): boolean {
  return needles.some((needle) => value.includes(needle));
}

function detectPythonFramework(signals: string): string {
  const normalized = signals.toLowerCase();
  if (hasAny(normalized, ["fastapi", "uvicorn"])) {
    return "fastapi";
  }
  if (hasAny(normalized, ["flask", "gunicorn"])) {
    return "flask";
  }
  if (hasAny(normalized, ["django", "manage.py"])) {
    return "django";
  }
  if (hasAny(normalized, ["streamlit"])) {
    return "streamlit";
  }
  if (hasAny(normalized, ["threadinghttpserver", "basehttprequesthandler", "http.server", "run_http_server"])) {
    return "stdlib-http";
  }
  return "python";
}

async function detectPythonStartCommand(
  projectRoot: string,
  framework: string,
  port: number,
): Promise<string | null> {
  switch (framework) {
    case "fastapi":
      return "uvicorn main:app --host 0.0.0.0 --port ${PORT:-8000}";
    case "flask":
      return "gunicorn -b 0.0.0.0:${PORT:-8000} app:app";
    case "django":
      return "python manage.py runserver 0.0.0.0:${PORT:-8000}";
    case "streamlit":
      return "streamlit run app.py --server.address 0.0.0.0 --server.port ${PORT:-8501}";
    case "stdlib-http": {
      const entrypoint = await detectPythonHttpEntrypoint(projectRoot);
      return entrypoint ? `python ${entrypoint} --host 0.0.0.0 --port ${port}` : null;
    }
    default:
      return null;
  }
}

function detectPythonPort(framework: string, signals: string): number {
  const detected = detectPortFromPythonSignals(signals);
  if (detected !== null) {
    return detected;
  }

  switch (framework) {
    case "streamlit":
      return 8501;
    default:
      return 8000;
  }
}

function detectGoFramework(signals: string): string {
  const normalized = signals.toLowerCase();
  if (hasAny(normalized, ["github.com/gin-gonic/gin"])) {
    return "gin";
  }
  if (hasAny(normalized, ["github.com/labstack/echo"])) {
    return "echo";
  }
  if (hasAny(normalized, ["github.com/gofiber/fiber"])) {
    return "fiber";
  }
  return "go";
}

function detectJavaFramework(signals: string): string {
  const normalized = signals.toLowerCase();
  if (hasAny(normalized, ["spring-boot", "org.springframework.boot", "springframework.boot"])) {
    return "spring-boot";
  }
  if (hasAny(normalized, ["quarkus", "io.quarkus"])) {
    return "quarkus";
  }
  if (hasAny(normalized, ["micronaut", "io.micronaut"])) {
    return "micronaut";
  }
  return "java";
}

function detectDotnetFramework(signals: string): string {
  const normalized = signals.toLowerCase();
  if (hasAny(normalized, ["microsoft.net.sdk.web", "microsoft.aspnetcore", "webapplication"])) {
    return "aspnetcore";
  }
  if (hasAny(normalized, ["microsoft.azure.functions.worker", "azurefunctions"])) {
    return "azure-functions";
  }
  return "dotnet";
}

function detectDotnetRuntimeVersion(signals: string): { version: string; source: string } {
  const targetFrameworkMatch = signals.match(/<TargetFramework>\s*net(\d+)(?:\.\d+)?\s*<\/TargetFramework>/i);
  if (targetFrameworkMatch) {
    return {
      version: targetFrameworkMatch[1],
      source: "TargetFramework",
    };
  }

  const targetFrameworksMatch = signals.match(/<TargetFrameworks>\s*([^<]+)\s*<\/TargetFrameworks>/i);
  if (targetFrameworksMatch) {
    const firstModernTarget = targetFrameworksMatch[1].split(";").find((target) => /^net\d+(?:\.\d+)?$/i.test(target.trim()));
    const major = firstModernTarget?.match(/^net(\d+)/i)?.[1];
    if (major) {
      return {
        version: major,
        source: "TargetFrameworks",
      };
    }
  }

  const globalJsonMatch = signals.match(/"version"\s*:\s*"(\d+)\.\d+\.\d+"/);
  if (globalJsonMatch) {
    return {
      version: globalJsonMatch[1],
      source: "global.json sdk.version",
    };
  }

  return {
    version: "8",
    source: "default",
  };
}

function detectDotnetPort(signals: string): number {
  const urlsMatch = signals.match(/ASPNETCORE_URLS["']?\s*[:=]\s*["']?https?:\/\/(?:\+|\*|0\.0\.0\.0|localhost|127\.0\.0\.1)?:(\d{2,5})/i);
  if (urlsMatch) {
    return Number(urlsMatch[1]);
  }

  const launchUrlMatch = signals.match(/applicationUrl"\s*:\s*"[^"]*:(\d{2,5})/i);
  if (launchUrlMatch) {
    return Number(launchUrlMatch[1]);
  }

  return detectPortFromSignals(signals) ?? 8080;
}

function detectPhpFramework(signals: string, projectRoot: string): string {
  const normalized = signals.toLowerCase();
  if (hasAny(normalized, ["laravel/framework", "artisan"])) {
    return "laravel";
  }
  if (hasAny(normalized, ["symfony/framework-bundle", "symfony/runtime"])) {
    return "symfony";
  }
  if (hasAny(normalized, ["slim/slim"])) {
    return "slim";
  }
  if (path.basename(projectRoot).toLowerCase().includes("laravel")) {
    return "laravel";
  }
  return "php";
}

function detectPhpRuntimeVersion(deps: Record<string, string>): string {
  const phpRange = deps.php;
  if (!phpRange) {
    return "8.3";
  }

  const preferred = ["8.4", "8.3", "8.2", "8.1", "8.0", "7.4"];
  for (const version of preferred) {
    if (phpRangeAllowsMinor(phpRange, version)) {
      return version;
    }
  }

  const match = phpRange.match(/(?:\^|~|>=|>|=)?\s*(\d+\.\d+)/);
  return match ? match[1] : "8.3";
}

function phpRangeAllowsMinor(range: string, minor: string): boolean {
  const [major, patchMinor] = minor.split(".").map(Number);
  if (range.includes(minor)) {
    return true;
  }
  if (range.includes(`^${major}.${patchMinor}`) || range.includes(`~${major}.${patchMinor}`)) {
    return true;
  }
  const lowerBound = range.match(/>=\s*(\d+)\.(\d+)/);
  if (lowerBound) {
    const lowerMajor = Number(lowerBound[1]);
    const lowerMinor = Number(lowerBound[2]);
    return major > lowerMajor || (major === lowerMajor && patchMinor >= lowerMinor);
  }
  return false;
}

function detectPhpStartCommand(framework: string): string {
  if (framework === "laravel") {
    return "php artisan serve --host=0.0.0.0 --port=${PORT:-8000}";
  }
  return "php -S 0.0.0.0:${PORT:-8000} -t public public/index.php";
}

function detectPhpPort(signals: string): number {
  return detectPortFromSignals(signals) ?? 8000;
}

function detectRubyFramework(signals: string): string {
  const normalized = signals.toLowerCase();
  if (hasAny(normalized, ["rails", "railties", "config/application.rb"])) {
    return "rails";
  }
  if (hasAny(normalized, ["sinatra"])) {
    return "sinatra";
  }
  if (hasAny(normalized, ["puma"])) {
    return "ruby-web";
  }
  return "ruby";
}

async function detectRubyRuntimeVersion(projectRoot: string, signals: string): Promise<string> {
  const rubyVersionFile = await readOptionalFile(path.join(projectRoot, ".ruby-version"));
  if (rubyVersionFile) {
    const version = rubyVersionFile.trim().match(/(\d+\.\d+(?:\.\d+)?)/)?.[1];
    if (version) {
      return rubyMinorVersion(version);
    }
  }

  const gemfileVersion = signals.match(/ruby\s+["'](\d+\.\d+(?:\.\d+)?)["']/i)?.[1];
  if (gemfileVersion) {
    return rubyMinorVersion(gemfileVersion);
  }

  return "3.3";
}

async function rubyRuntimeVersionSource(projectRoot: string, signals: string): Promise<string> {
  if (await pathExists(path.join(projectRoot, ".ruby-version"))) {
    return ".ruby-version";
  }
  if (/ruby\s+["'](\d+\.\d+(?:\.\d+)?)["']/i.test(signals)) {
    return "Gemfile ruby";
  }
  return "default";
}

function rubyMinorVersion(version: string): string {
  const match = version.match(/^(\d+\.\d+)/);
  return match ? match[1] : version;
}

function detectRubyStartCommand(framework: string): string {
  if (framework === "rails") {
    return "bundle exec rails server -b 0.0.0.0 -p ${PORT:-3000}";
  }
  if (framework === "sinatra" || framework === "ruby-web") {
    return "bundle exec rackup -o 0.0.0.0 -p ${PORT:-3000}";
  }
  return "ruby -run -e httpd . -p ${PORT:-3000}";
}

function detectRubyPort(signals: string): number {
  return detectPortFromSignals(signals) ?? 3000;
}

function detectJavaRuntimeVersion(signals: string): { version: string; source: string } {
  const candidates = [
    { source: "maven.compiler.release", pattern: /<maven\.compiler\.release>\s*(\d{1,2})\s*<\/maven\.compiler\.release>/i },
    { source: "maven.compiler.target", pattern: /<maven\.compiler\.target>\s*(\d{1,2})\s*<\/maven\.compiler\.target>/i },
    { source: "java.version", pattern: /<java\.version>\s*(\d{1,2})\s*<\/java\.version>/i },
    { source: "sourceCompatibility", pattern: /sourceCompatibility\s*=?\s*["']?(?:JavaVersion\.VERSION_)?(?:1_)?(\d{1,2})/i },
    { source: "targetCompatibility", pattern: /targetCompatibility\s*=?\s*["']?(?:JavaVersion\.VERSION_)?(?:1_)?(\d{1,2})/i },
    { source: "toolchain.languageVersion", pattern: /languageVersion\s*=?\s*JavaLanguageVersion\.of\((\d{1,2})\)/i },
  ];

  for (const candidate of candidates) {
    const match = signals.match(candidate.pattern);
    if (match) {
      return {
        version: normalizeJavaMajorVersion(match[1]),
        source: candidate.source,
      };
    }
  }

  return {
    version: "21",
    source: "default",
  };
}

function normalizeJavaMajorVersion(value: string): string {
  return value === "8" ? "8" : String(Number(value));
}

function javaBuildCommand(packageManager: "maven" | "gradle"): string {
  return packageManager === "maven"
    ? "if [ -x ./mvnw ]; then ./mvnw -DskipTests package; else mvn -DskipTests package; fi"
    : "if [ -x ./gradlew ]; then ./gradlew build -x test; else gradle build -x test; fi";
}

function javaOutputDirectory(packageManager: "maven" | "gradle"): string {
  return packageManager === "maven" ? "target" : "build/libs";
}

function detectJavaPort(signals: string): number {
  return detectSpringServerPort(signals) ?? detectPortFromSignals(signals) ?? 8080;
}

function detectSpringServerPort(signals: string): number | null {
  const propertiesMatch = signals.match(/(?:^|\n)\s*server\.port\s*=\s*(\d{2,5})/);
  if (propertiesMatch) {
    return Number(propertiesMatch[1]);
  }

  const yamlMatch = signals.match(/(?:^|\n)\s*port:\s*(\d{2,5})/);
  return yamlMatch ? Number(yamlMatch[1]) : null;
}

function detectPortFromSignals(signals: string): number | null {
  const match = signals.match(/\b(?:port|PORT)\s*[:=]\s*["']?(\d{2,5})/);
  return match ? Number(match[1]) : null;
}

function detectPortFromPythonSignals(signals: string): number | null {
  const patterns = [
    /\b(?:port|PORT)\s*[:=]\s*["']?(\d{2,5})/,
    /--port\s+(\d{2,5})/,
    /add_argument\(["']--port["'][\s\S]{0,160}?default\s*=\s*(\d{2,5})/,
    /run_http_server\([\s\S]{0,160}?port\s*=\s*(\d{2,5})/,
    /ThreadingHTTPServer\(\([^,\n]+,\s*(\d{2,5})\)/,
  ];

  for (const pattern of patterns) {
    const match = signals.match(pattern);
    if (match) {
      return Number(match[1]);
    }
  }
  return null;
}

function detectHealthcheckPath(signals: string): string | null {
  const candidates = ["/health", "/healthz", "/ready", "/readiness", "/api/health", "/up"];
  return candidates.find((candidate) => pathAppearsInSignals(signals, candidate)) ?? null;
}

function pathAppearsInSignals(signals: string, pathName: string): boolean {
  const escaped = pathName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`["'\`]${escaped}(?:[?"'\`\\s]|$)|\\b(?:GET|POST|PUT|PATCH|DELETE)\\s+${escaped}(?:\\b|[?\\s])`, "i").test(signals);
}

async function detectPythonHttpEntrypoint(projectRoot: string): Promise<string | null> {
  const candidates = ["server.py", "main.py", "app.py"];
  for (const candidate of candidates) {
    const filePath = path.join(projectRoot, candidate);
    if ((await pathExists(filePath)) && await fileContainsAny(filePath, [
      "ThreadingHTTPServer",
      "BaseHTTPRequestHandler",
      "HTTPServer",
      "http.server",
      "run_http_server",
    ])) {
      return candidate;
    }
  }
  return null;
}

async function readPythonPackageSignals(projectRoot: string): Promise<string> {
  const entries = await safeReadDir(projectRoot);
  const values: string[] = [];
  for (const entry of entries) {
    if (!entry.isDirectory() || entry.name.startsWith(".") || entry.name === "__pycache__") {
      continue;
    }
    if (await pathExists(path.join(projectRoot, entry.name, "__init__.py"))) {
      values.push(await readProjectSignalsFromDirectory(projectRoot, entry.name));
    }
  }
  return values.join("\n");
}

async function readProjectSignals(projectRoot: string, candidates: string[]): Promise<string> {
  const values: string[] = [];
  for (const candidate of candidates) {
    const filePath = path.join(projectRoot, candidate);
    if (await pathExists(filePath)) {
      values.push(candidate);
      values.push(await fs.readFile(filePath, "utf8"));
    }
  }
  return values.join("\n");
}

async function readProjectSignalsFromDirectory(projectRoot: string, directory: string): Promise<string> {
  const directoryPath = path.join(projectRoot, directory);
  const entries = await safeReadDir(directoryPath);
  const values: string[] = [];
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.endsWith(".py")) {
      continue;
    }
    const relativePath = path.join(directory, entry.name);
    values.push(relativePath);
    values.push(await fs.readFile(path.join(projectRoot, relativePath), "utf8"));
  }
  return values.join("\n");
}

async function fileContains(filePath: string, needle: string): Promise<boolean> {
  if (!(await pathExists(filePath))) {
    return false;
  }
  return (await fs.readFile(filePath, "utf8")).includes(needle);
}

async function fileContainsAny(filePath: string, needles: string[]): Promise<boolean> {
  if (!(await pathExists(filePath))) {
    return false;
  }
  const raw = await fs.readFile(filePath, "utf8");
  return needles.some((needle) => raw.includes(needle));
}

async function pathExistsInWorkspaceAncestor(projectRoot: string, fileName: string): Promise<boolean> {
  let current = path.dirname(path.resolve(projectRoot));
  while (true) {
    if ((await hasWorkspaceMarker(current)) && (await pathExists(path.join(current, fileName)))) {
      return true;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      return false;
    }
    current = parent;
  }
}

async function hasWorkspaceMarker(projectRoot: string): Promise<boolean> {
  const markerFiles = ["pnpm-workspace.yaml", "turbo.json", "nx.json", "lerna.json", "rush.json"];
  for (const markerFile of markerFiles) {
    if (await pathExists(path.join(projectRoot, markerFile))) {
      return true;
    }
  }

  const packageJsonPath = path.join(projectRoot, "package.json");
  if (!(await pathExists(packageJsonPath))) {
    return false;
  }

  try {
    const pkg = JSON.parse(await fs.readFile(packageJsonPath, "utf8")) as {
      workspaces?: unknown;
    };
    return Array.isArray(pkg.workspaces) ||
      (typeof pkg.workspaces === "object" &&
        pkg.workspaces !== null &&
        Array.isArray((pkg.workspaces as { packages?: unknown }).packages));
  } catch {
    return false;
  }
}

function packageManagerRun(packageManager: NonNullable<DeploymentCodeProbe["packageManager"]>, script: string): string {
  switch (packageManager) {
    case "npm":
      return `npm run ${script}`;
    case "pnpm":
      return `pnpm run ${script}`;
    case "yarn":
      return `yarn ${script}`;
    case "bun":
      return `bun run ${script}`;
    case "pip":
    case "poetry":
    case "uv":
    case "go":
    case "maven":
    case "gradle":
    case "dotnet":
    case "composer":
    case "bundler":
      return script;
  }
}

function packageManagerRunWithArgs(
  packageManager: NonNullable<DeploymentCodeProbe["packageManager"]>,
  script: string,
  args: string[],
): string {
  const suffix = args.join(" ");
  switch (packageManager) {
    case "npm":
    case "pnpm":
    case "bun":
      return `${packageManagerRun(packageManager, script)} -- ${suffix}`;
    case "yarn":
      return `${packageManagerRun(packageManager, script)} ${suffix}`;
    case "pip":
    case "poetry":
    case "uv":
    case "go":
    case "maven":
    case "gradle":
    case "dotnet":
    case "composer":
    case "bundler":
      return `${script} ${suffix}`;
  }
}
