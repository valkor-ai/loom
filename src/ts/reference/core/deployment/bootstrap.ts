import { promises as fs } from "node:fs";
import path from "node:path";
import { pathExists } from "../state/fs";
import type { DeploymentBootstrapDiagnostics, DeploymentBootstrapTask, DeploymentCodeProbe } from "./types";

const BOOTSTRAP_IGNORED_DIRECTORIES = new Set([
  ".git",
  ".loom",
  "node_modules",
  "vendor",
  ".venv",
  "venv",
  "__pycache__",
  "target",
  "build",
  "dist",
  "coverage",
  "tmp",
  "log",
]);
const BOOTSTRAP_MAX_DEPTH = 8;

export async function analyzeDeploymentBootstrap(input: {
  deploymentRoot: string;
  stack: DeploymentCodeProbe;
}): Promise<DeploymentBootstrapDiagnostics> {
  const tasks: DeploymentBootstrapTask[] = [];
  const warnings: string[] = [];

  const prismaRoot = await findPrismaRoot(input.deploymentRoot);
  if (prismaRoot) {
    pushTask(tasks, {
      kind: "prisma",
      command: commandInDirectory(input.deploymentRoot, prismaRoot, packageManagerRun(input.stack.packageManager, "prisma migrate deploy")),
      automatic: false,
      reason: "Prisma schema detected; databases may require migrations before the app can serve requests.",
    });
  }

  const djangoRoot = await findDirectoryWithFile(input.deploymentRoot, "manage.py");
  if (input.stack.framework === "django" && djangoRoot) {
    pushTask(tasks, {
      kind: "django",
      command: commandInDirectory(input.deploymentRoot, djangoRoot, "python manage.py migrate --noinput"),
      automatic: false,
      reason: "Django manage.py detected; pending migrations can surface as missing-table errors at boot or first request.",
    });
  }

  const railsRoot = await findDirectoryWithPath(input.deploymentRoot, "db/migrate");
  if (input.stack.framework === "rails" && railsRoot) {
    pushTask(tasks, {
      kind: "rails",
      command: commandInDirectory(input.deploymentRoot, railsRoot, "bundle exec rails db:migrate"),
      automatic: false,
      reason: "Rails migrations detected; pending migrations can cause boot or request failures.",
    });
  }

  const laravelRoot = await findDirectoryWithPath(input.deploymentRoot, "database/migrations");
  if (input.stack.framework === "laravel" && laravelRoot) {
    pushTask(tasks, {
      kind: "laravel",
      command: commandInDirectory(input.deploymentRoot, laravelRoot, "php artisan migrate --force"),
      automatic: false,
      reason: "Laravel migrations detected; pending migrations can cause database/table failures.",
    });
  }

  const flywayRoot = await findFlywayRoot(input.deploymentRoot);
  if (flywayRoot) {
    pushTask(tasks, {
      kind: "flyway",
      command: commandInDirectory(input.deploymentRoot, flywayRoot, flywayCommand(input.stack)),
      automatic: false,
      reason: "Flyway configuration detected; schema migrations may need to run before deployment is healthy.",
    });
  }

  const liquibaseRoot = await findLiquibaseRoot(input.deploymentRoot);
  if (liquibaseRoot) {
    pushTask(tasks, {
      kind: "liquibase",
      command: commandInDirectory(input.deploymentRoot, liquibaseRoot, liquibaseCommand(input.stack)),
      automatic: false,
      reason: "Liquibase configuration detected; schema migrations may need to run before deployment is healthy.",
    });
  }

  if (tasks.length > 0) {
    warnings.push("Bootstrap tasks are diagnostic only; loom does not run migrations automatically.");
  }

  return { tasks, warnings };
}

async function findPrismaRoot(root: string): Promise<string | null> {
  const schema = await findDirectoryWithPath(root, "prisma/schema.prisma");
  if (schema) {
    return schema;
  }
  return findPackageScriptDirectory(root, /prisma\s+migrate|prisma\s+db\s+push/i);
}

async function findFlywayRoot(root: string): Promise<string | null> {
  return (
    await findDirectoryWithAnyFile(root, ["flyway.conf", "flyway.toml"]) ??
    await findDirectoryWithPath(root, "src/main/resources/db/migration")
  );
}

async function findLiquibaseRoot(root: string): Promise<string | null> {
  return (
    await findDirectoryWithAnyFile(root, ["liquibase.properties", "liquibase.yml", "liquibase.yaml"]) ??
    await findDirectoryWithPath(root, "src/main/resources/db/changelog")
  );
}

async function findPackageScriptDirectory(root: string, pattern: RegExp): Promise<string | null> {
  const packageFiles = await collectFiles(root, 0, BOOTSTRAP_MAX_DEPTH, 200, (relativePath) => relativePath.endsWith("package.json"));
  for (const packagePath of packageFiles) {
    try {
      const pkg = JSON.parse(await fs.readFile(packagePath, "utf8")) as { scripts?: Record<string, unknown> };
      if (Object.values(pkg.scripts ?? {}).some((script) => typeof script === "string" && pattern.test(script))) {
        return path.dirname(packagePath);
      }
    } catch {
      // Ignore malformed package files during diagnostic bootstrap discovery.
    }
  }
  return null;
}

async function findDirectoryWithFile(root: string, fileName: string): Promise<string | null> {
  const file = (await collectFiles(root, 0, BOOTSTRAP_MAX_DEPTH, 200, (relativePath) => path.basename(relativePath) === fileName))[0];
  return file ? path.dirname(file) : null;
}

async function findDirectoryWithAnyFile(root: string, fileNames: string[]): Promise<string | null> {
  const fileNameSet = new Set(fileNames);
  const file = (await collectFiles(root, 0, BOOTSTRAP_MAX_DEPTH, 200, (relativePath) => fileNameSet.has(path.basename(relativePath))))[0];
  return file ? path.dirname(file) : null;
}

async function findDirectoryWithPath(root: string, relativeSuffix: string): Promise<string | null> {
  const normalizedSuffix = normalizeRelativePath(relativeSuffix);
  if (await pathExists(path.join(root, relativeSuffix))) {
    return root;
  }
  const match = (await collectFiles(root, 0, BOOTSTRAP_MAX_DEPTH, 300, (relativePath) => {
    const normalizedPath = normalizeRelativePath(relativePath);
    return normalizedPath.endsWith(normalizedSuffix) || normalizedPath.includes(`${normalizedSuffix}/`);
  }))[0];
  if (!match) {
    return null;
  }
  const normalizedMatch = normalizeRelativePath(path.relative(root, match));
  const suffixIndex = normalizedMatch.endsWith(normalizedSuffix)
    ? normalizedMatch.length - normalizedSuffix.length
    : normalizedMatch.indexOf(`${normalizedSuffix}/`);
  const prefix = normalizedMatch.slice(0, Math.max(0, suffixIndex)).replace(/\/$/, "");
  return prefix ? path.join(root, ...prefix.split("/")) : root;
}

async function collectFiles(
  root: string,
  depth: number,
  maxDepth: number,
  limit: number,
  predicate: (relativePath: string) => boolean,
  baseRoot = root,
): Promise<string[]> {
  if (depth > maxDepth || limit <= 0) {
    return [];
  }

  const entries = await safeReadDir(root);
  const files: string[] = [];
  for (const entry of entries) {
    if (files.length >= limit) {
      break;
    }
    if (entry.isDirectory()) {
      if (!BOOTSTRAP_IGNORED_DIRECTORIES.has(entry.name)) {
        files.push(...await collectFiles(path.join(root, entry.name), depth + 1, maxDepth, limit - files.length, predicate, baseRoot));
      }
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    const filePath = path.join(root, entry.name);
    const relativePath = normalizeRelativePath(path.relative(baseRoot, filePath));
    if (predicate(relativePath) || predicate(entry.name)) {
      files.push(filePath);
    }
  }
  return files.slice(0, limit);
}

async function safeReadDir(directory: string): Promise<import("node:fs").Dirent[]> {
  try {
    return await fs.readdir(directory, { withFileTypes: true });
  } catch {
    return [];
  }
}

function commandInDirectory(root: string, directory: string, command: string): string {
  const relative = normalizeRelativePath(path.relative(root, directory));
  return relative && relative !== "."
    ? `cd ${JSON.stringify(relative)} && ${command}`
    : command;
}

function flywayCommand(stack: DeploymentCodeProbe): string {
  if (stack.kind === "java" && stack.packageManager === "maven") {
    return "mvn -DskipTests flyway:migrate";
  }
  if (stack.kind === "java" && stack.packageManager === "gradle") {
    return "gradle flywayMigrate";
  }
  return "flyway migrate";
}

function liquibaseCommand(stack: DeploymentCodeProbe): string {
  if (stack.kind === "java" && stack.packageManager === "maven") {
    return "mvn -DskipTests liquibase:update";
  }
  if (stack.kind === "java" && stack.packageManager === "gradle") {
    return "gradle liquibaseUpdate";
  }
  return "liquibase update";
}

function pushTask(tasks: DeploymentBootstrapTask[], task: DeploymentBootstrapTask): void {
  if (!tasks.some((current) => current.kind === task.kind && current.command === task.command)) {
    tasks.push(task);
  }
}

function normalizeRelativePath(value: string): string {
  return value.split(path.sep).join("/");
}

function packageManagerRun(packageManager: DeploymentCodeProbe["packageManager"], command: string): string {
  switch (packageManager) {
    case "pnpm":
      return `pnpm exec ${command}`;
    case "yarn":
      return `yarn ${command}`;
    case "bun":
      return `bunx ${command}`;
    case "npm":
    default:
      return `npx ${command}`;
  }
}
