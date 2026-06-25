import { execFile } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";
import { toProjectRelative } from "../state/paths";

const execFileAsync = promisify(execFile);

export type IndexedFile = {
  relativePath: string;
  absolutePath: string;
  kind: "manifest" | "config" | "env" | "source" | "deploy_asset";
};

export type FileSignal = {
  file: IndexedFile;
  text: string;
  lower: string;
};

const MAX_SOURCE_FILES = 250;
const MAX_SOURCE_FILE_BYTES = 96_000;
const MAX_DECLARATION_FILE_BYTES = 512_000;

const IGNORED_DIRECTORIES = new Set([
  ".git",
  ".hg",
  ".svn",
  ".loom",
  "node_modules",
  "vendor",
  ".venv",
  "venv",
  "__pycache__",
  ".next",
  ".nuxt",
  ".output",
  ".turbo",
  ".vercel",
  "dist",
  "build",
  "coverage",
  "target",
  "bin",
  "obj",
  "tmp",
  "log",
  "storage",
]);

const DECLARATION_BASENAMES = new Set([
  "package.json",
  "pnpm-workspace.yaml",
  "turbo.json",
  "nx.json",
  "pom.xml",
  "build.gradle",
  "build.gradle.kts",
  "settings.gradle",
  "settings.gradle.kts",
  "pyproject.toml",
  "requirements.txt",
  "go.mod",
  "composer.json",
  "Gemfile",
  "application.yml",
  "application.yaml",
  "application.properties",
  "appsettings.json",
  "appsettings.Development.json",
  ".env.example",
  ".env.sample",
  ".env.local.example",
  ".env.template",
  ".env.dist",
  "schema.prisma",
  "Dockerfile",
  "dockerfile",
  "compose.yaml",
  "compose.yml",
  "docker-compose.yaml",
  "docker-compose.yml",
]);

const SOURCE_EXTENSIONS = new Set([
  ".js",
  ".jsx",
  ".ts",
  ".tsx",
  ".mjs",
  ".cjs",
  ".java",
  ".kt",
  ".py",
  ".go",
  ".cs",
  ".php",
  ".rb",
  ".yml",
  ".yaml",
  ".properties",
  ".toml",
]);

export async function indexProjectFiles(projectRoot: string): Promise<IndexedFile[]> {
  const gitFiles = await gitTrackedFiles(projectRoot);
  const candidates = gitFiles ?? await walkedFiles(projectRoot);
  const indexed: IndexedFile[] = [];
  let sourceCount = 0;
  for (const relativePath of candidates.sort(comparePaths)) {
    if (isIgnoredPath(relativePath)) {
      continue;
    }
    const kind = classifyIndexedFile(relativePath);
    if (!kind) {
      continue;
    }
    if (kind === "source" && sourceCount >= MAX_SOURCE_FILES) {
      continue;
    }
    if (kind === "source") {
      sourceCount += 1;
    }
    indexed.push({
      relativePath,
      absolutePath: path.join(projectRoot, relativePath),
      kind,
    });
  }
  return indexed;
}

export async function readFileSignals(files: IndexedFile[]): Promise<FileSignal[]> {
  const signals: FileSignal[] = [];
  for (const file of files) {
    try {
      const stat = await fs.stat(file.absolutePath);
      const maxBytes = file.kind === "source" ? MAX_SOURCE_FILE_BYTES : MAX_DECLARATION_FILE_BYTES;
      if (stat.size > maxBytes) {
        continue;
      }
      const text = await fs.readFile(file.absolutePath, "utf8");
      signals.push({
        file,
        text,
        lower: text.toLowerCase(),
      });
    } catch {
      continue;
    }
  }
  return signals;
}

async function gitTrackedFiles(projectRoot: string): Promise<string[] | null> {
  try {
    const result = await execFileAsync("git", ["-C", projectRoot, "ls-files", "--cached", "--others", "--exclude-standard"], {
      maxBuffer: 8 * 1024 * 1024,
    });
    const files = String(result.stdout)
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
    return files.length > 0 ? files : null;
  } catch {
    return null;
  }
}

async function walkedFiles(projectRoot: string): Promise<string[]> {
  const output: string[] = [];
  async function walk(dir: string): Promise<void> {
    let entries;
    try {
      entries = await fs.readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (IGNORED_DIRECTORIES.has(entry.name)) {
        continue;
      }
      const absolutePath = path.join(dir, entry.name);
      const relativePath = toProjectRelative(projectRoot, absolutePath);
      if (entry.isDirectory()) {
        await walk(absolutePath);
      } else if (entry.isFile()) {
        output.push(relativePath);
      }
    }
  }
  await walk(projectRoot);
  return output;
}

function classifyIndexedFile(relativePath: string): IndexedFile["kind"] | null {
  const normalized = relativePath.split(path.sep).join("/");
  const basename = path.basename(normalized);
  if (["Dockerfile", "dockerfile", "compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"].includes(basename)) {
    return "deploy_asset";
  }
  if (DECLARATION_BASENAMES.has(basename) || /\.csproj$/i.test(basename) || /\.sln$/i.test(basename)) {
    if (basename.startsWith(".env")) {
      return "env";
    }
    if (/application\.(ya?ml|properties)$/.test(basename) || /appsettings.*\.json$/i.test(basename)) {
      return "config";
    }
    return "manifest";
  }
  if (SOURCE_EXTENSIONS.has(path.extname(basename))) {
    return "source";
  }
  return null;
}

function isIgnoredPath(relativePath: string): boolean {
  return relativePath
    .split(/[\\/]/)
    .some((segment) => IGNORED_DIRECTORIES.has(segment));
}

function comparePaths(left: string, right: string): number {
  return left.localeCompare(right);
}
