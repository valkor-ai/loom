import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument } from "../errors";
import {
  findKnowledgeSource,
  listPendingKnowledge,
  readKnowledgeRegistry,
  readPendingKnowledge,
  removeKnowledgeSource,
  removePendingKnowledge,
  validateKnowledgeName,
  writeKnowledgeRegistry,
  writePendingKnowledge,
} from "./state";
import {
  DEFAULT_MAX_KNOWLEDGE_FILE_BYTES,
  KNOWLEDGE_SCHEMA_VERSION,
  SUPPORTED_KNOWLEDGE_EXTENSIONS,
  type KnowledgeAddResult,
  type KnowledgeDiscardResult,
  type KnowledgeListResult,
  type KnowledgePendingResult,
  type KnowledgePendingSource,
  type KnowledgeRemoveResult,
  type KnowledgeStatusResult,
  type KnowledgeToggleResult,
  type KnowledgeUpdateResult,
  type KnowledgeValidationSummary,
  type KnowledgeValidationWarning,
  type PendingKnowledgeOperation,
} from "./types";

type KnowledgeUpdateInput = {
  name: string | undefined;
  addPath?: string[];
  removePath?: string[];
  replacePaths?: string[];
};

const IGNORED_DIRECTORY_NAMES = new Set([
  ".git",
  ".loom",
  "node_modules",
  "dist",
  "build",
  ".DS_Store",
]);

export async function addKnowledgeSource(input: {
  name: string | undefined;
  paths: string[];
}): Promise<KnowledgeAddResult> {
  const name = validateKnowledgeName(input.name);
  const source = await findKnowledgeSource(name);
  if (source) {
    throw invalidArgument(`Knowledge source "${name}" already exists. Use knowledge update to change its paths.`, {
      name,
      suggestedCommands: [
        `loom knowledge update ${name} --add-path <path>`,
        `loom knowledge update ${name} --replace-paths <path...>`,
      ],
    });
  }
  const normalizedPaths = normalizeInputPaths(input.paths);
  const validation = await validateAddLikePaths(normalizedPaths);
  const existing = await readPendingKnowledge(name);
  if (existing && !existing.createNew) {
    throw invalidArgument(`Knowledge source "${name}" has a pending update but no registered source. Discard the pending queue or choose another name.`, {
      name,
      suggestedCommand: `loom knowledge discard ${name}`,
    });
  }
  const now = new Date().toISOString();
  const pending: KnowledgePendingSource = {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    name,
    sourceId: existing?.sourceId ?? null,
    createNew: true,
    operations: [
      ...(existing?.operations ?? []),
      {
        type: "add_paths",
        paths: normalizedPaths,
      },
    ],
    validation: mergeValidationSummaries(existing?.validation, validation),
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
  await writePendingKnowledge(pending);
  return {
    name,
    pending,
    addedPaths: normalizedPaths,
    validation,
    nextCommand: `loom knowledge build ${name}`,
    message: `Knowledge source "${name}" has pending path changes. Run loom knowledge build ${name} when ready.`,
  };
}

export async function updateKnowledgeSource(input: KnowledgeUpdateInput): Promise<KnowledgeUpdateResult> {
  const name = validateKnowledgeName(input.name);
  const operation = await updateOperationFor(input);
  const source = await findKnowledgeSource(name);
  const existing = await readPendingKnowledge(name);
  if (!source && !existing) {
    throw invalidArgument(`Knowledge source "${name}" does not exist. Use knowledge add to create it.`, {
      name,
      suggestedCommand: `loom knowledge add --name ${name} <path...>`,
    });
  }
  const now = new Date().toISOString();
  const validation = operation.type === "remove_paths"
    ? emptyValidationSummary()
    : await validateAddLikePaths(operation.paths);
  const pending: KnowledgePendingSource = {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    name,
    sourceId: source?.sourceId ?? existing?.sourceId ?? null,
    createNew: existing?.createNew ?? !source,
    operations: [
      ...(existing?.operations ?? []),
      operation,
    ],
    validation: mergeValidationSummaries(existing?.validation, validation),
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
  };
  await writePendingKnowledge(pending);
  return {
    name,
    pending,
    operation,
    validation,
    nextCommand: `loom knowledge build ${name}`,
    message: `Knowledge source "${name}" has pending path changes. Run loom knowledge build ${name} when ready.`,
  };
}

export async function listKnowledgePending(name?: string): Promise<KnowledgePendingResult> {
  if (name) {
    const normalizedName = validateKnowledgeName(name);
    const pending = await readPendingKnowledge(normalizedName);
    return { pending: pending ? [pending] : [] };
  }
  return { pending: await listPendingKnowledge() };
}

export async function discardKnowledgePending(name: string | undefined): Promise<KnowledgeDiscardResult> {
  const normalizedName = validateKnowledgeName(name);
  const discarded = await removePendingKnowledge(normalizedName);
  return {
    name: normalizedName,
    discarded,
    message: discarded
      ? `Discarded pending knowledge changes for "${normalizedName}".`
      : `No pending knowledge changes found for "${normalizedName}".`,
  };
}

export async function listKnowledgeSources(): Promise<KnowledgeListResult> {
  const registry = await readKnowledgeRegistry();
  const pending = await listPendingKnowledge();
  const pendingByName = new Map(pending.map((entry) => [entry.name, entry]));
  const sourceRows = registry.sources
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((source) => ({
      name: source.name,
      status: source.status,
      docs: source.index.documentCount,
      lastBuild: source.index.lastBuiltAt,
      pendingOperations: pendingByName.get(source.name)?.operations.length ?? 0,
    }));
  const pendingRows = pending
    .filter((entry) => !registry.sources.some((source) => source.name === entry.name))
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((entry) => ({
      name: entry.name,
      status: "pending" as const,
      docs: null,
      lastBuild: null,
      pendingOperations: entry.operations.length,
    }));
  return {
    sources: [...sourceRows, ...pendingRows],
  };
}

export async function getKnowledgeStatus(name: string | undefined): Promise<KnowledgeStatusResult> {
  const normalizedName = validateKnowledgeName(name);
  return {
    name: normalizedName,
    source: await findKnowledgeSource(normalizedName),
    pending: await readPendingKnowledge(normalizedName),
  };
}

export async function removeKnowledge(name: string | undefined): Promise<KnowledgeRemoveResult> {
  const normalizedName = validateKnowledgeName(name);
  const removedSource = await removeKnowledgeSource(normalizedName);
  const removedPending = await removePendingKnowledge(normalizedName);
  return {
    name: normalizedName,
    removedSource,
    removedPending,
    message: removedSource || removedPending
      ? `Removed knowledge source state for "${normalizedName}". Original source documents were not deleted.`
      : `No knowledge source or pending queue found for "${normalizedName}".`,
  };
}

export async function setKnowledgeEnabled(input: {
  name: string | undefined;
  enabled: boolean;
}): Promise<KnowledgeToggleResult> {
  const name = validateKnowledgeName(input.name);
  const registry = await readKnowledgeRegistry();
  const source = registry.sources.find((entry) => entry.name === name);
  if (!source) {
    throw invalidArgument(`Knowledge source "${name}" does not exist.`, {
      name,
    });
  }
  const updated = {
    ...source,
    status: input.enabled ? "enabled" as const : "disabled" as const,
    updatedAt: new Date().toISOString(),
  };
  await writeKnowledgeRegistry({
    ...registry,
    sources: registry.sources.map((entry) => entry.name === name ? updated : entry),
  });
  return {
    name,
    status: updated.status,
    message: `Knowledge source "${name}" is now ${updated.status}.`,
  };
}

async function updateOperationFor(input: KnowledgeUpdateInput): Promise<PendingKnowledgeOperation> {
  const kinds = [
    input.addPath && input.addPath.length > 0 ? "add-path" : null,
    input.removePath && input.removePath.length > 0 ? "remove-path" : null,
    input.replacePaths && input.replacePaths.length > 0 ? "replace-paths" : null,
  ].filter((value) => value !== null);
  if (kinds.length !== 1) {
    throw invalidArgument("knowledge update requires exactly one of --add-path, --remove-path, or --replace-paths.", {
      provided: kinds,
    });
  }
  if (input.addPath && input.addPath.length > 0) {
    return {
      type: "add_paths",
      paths: normalizeInputPaths(input.addPath),
    };
  }
  if (input.removePath && input.removePath.length > 0) {
    return {
      type: "remove_paths",
      paths: normalizeInputPaths(input.removePath),
    };
  }
  return {
    type: "replace_paths",
    paths: normalizeInputPaths(input.replacePaths ?? []),
  };
}

function normalizeInputPaths(inputPaths: string[]): string[] {
  const normalized = inputPaths
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0)
    .map((entry) => path.resolve(entry));
  const unique = [...new Set(normalized)];
  if (unique.length === 0) {
    throw invalidArgument("knowledge add/update requires at least one path.");
  }
  return unique;
}

async function validateAddLikePaths(paths: string[]): Promise<KnowledgeValidationSummary> {
  const warnings: KnowledgeValidationWarning[] = [];
  let acceptedFiles = 0;
  let acceptedDirectories = 0;
  let supportedFiles = 0;
  const acceptedPaths: string[] = [];
  for (const inputPath of paths) {
    let stat;
    try {
      stat = await fs.stat(inputPath);
    } catch {
      throw invalidArgument("Knowledge path does not exist or is not readable.", {
        path: inputPath,
      });
    }
    if (stat.isFile()) {
      const fileValidation = validateFile(inputPath, stat.size, true);
      if (fileValidation.warning) {
        throw invalidArgument(fileValidation.warning.message, {
          path: inputPath,
          reason: fileValidation.warning.reason,
          supportedExtensions: SUPPORTED_KNOWLEDGE_EXTENSIONS,
          maxFileBytes: DEFAULT_MAX_KNOWLEDGE_FILE_BYTES,
        });
      }
      acceptedFiles += 1;
      supportedFiles += 1;
      acceptedPaths.push(inputPath);
      continue;
    }
    if (stat.isDirectory()) {
      const scanned = await scanDirectory(inputPath);
      warnings.push(...scanned.warnings);
      if (scanned.supportedFiles === 0) {
        throw invalidArgument("Knowledge directory does not contain any supported files.", {
          path: inputPath,
          supportedExtensions: SUPPORTED_KNOWLEDGE_EXTENSIONS,
          skippedFiles: scanned.warnings,
        });
      }
      acceptedDirectories += 1;
      supportedFiles += scanned.supportedFiles;
      acceptedPaths.push(inputPath);
      continue;
    }
    throw invalidArgument("Knowledge path must be a file or directory.", {
      path: inputPath,
    });
  }
  return {
    acceptedPaths,
    acceptedFiles,
    acceptedDirectories,
    supportedFiles,
    skippedFiles: warnings,
    maxFileBytes: DEFAULT_MAX_KNOWLEDGE_FILE_BYTES,
  };
}

async function scanDirectory(dir: string): Promise<{
  supportedFiles: number;
  warnings: KnowledgeValidationWarning[];
}> {
  let supportedFiles = 0;
  const warnings: KnowledgeValidationWarning[] = [];
  const entries = await fs.readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (IGNORED_DIRECTORY_NAMES.has(entry.name)) {
      continue;
    }
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const child = await scanDirectory(entryPath);
      supportedFiles += child.supportedFiles;
      warnings.push(...child.warnings);
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    let stat;
    try {
      stat = await fs.stat(entryPath);
    } catch {
      warnings.push({
        path: entryPath,
        reason: "unreadable_path",
        message: `Skipped unreadable file: ${entryPath}`,
      });
      continue;
    }
    const fileValidation = validateFile(entryPath, stat.size, false);
    if (fileValidation.warning) {
      warnings.push(fileValidation.warning);
    } else {
      supportedFiles += 1;
    }
  }
  return { supportedFiles, warnings };
}

function validateFile(filePath: string, size: number, explicit: boolean): {
  warning: KnowledgeValidationWarning | null;
} {
  const ext = path.extname(filePath).toLowerCase();
  if (!SUPPORTED_KNOWLEDGE_EXTENSIONS.includes(ext as typeof SUPPORTED_KNOWLEDGE_EXTENSIONS[number])) {
    return {
      warning: {
        path: filePath,
        reason: "unsupported_file_type",
        message: explicit
          ? `Unsupported knowledge file type: ${filePath}.`
          : `Skipped unsupported knowledge file type: ${filePath}.`,
      },
    };
  }
  if (size > DEFAULT_MAX_KNOWLEDGE_FILE_BYTES) {
    return {
      warning: {
        path: filePath,
        reason: "file_too_large",
        message: explicit
          ? `Knowledge file is too large: ${filePath}.`
          : `Skipped oversized knowledge file: ${filePath}.`,
      },
    };
  }
  return { warning: null };
}

function mergeValidationSummaries(
  previous: KnowledgeValidationSummary | undefined,
  next: KnowledgeValidationSummary,
): KnowledgeValidationSummary {
  if (!previous) {
    return next;
  }
  return {
    acceptedPaths: [...new Set([...previous.acceptedPaths, ...next.acceptedPaths])],
    acceptedFiles: previous.acceptedFiles + next.acceptedFiles,
    acceptedDirectories: previous.acceptedDirectories + next.acceptedDirectories,
    supportedFiles: previous.supportedFiles + next.supportedFiles,
    skippedFiles: [...previous.skippedFiles, ...next.skippedFiles],
    maxFileBytes: Math.max(previous.maxFileBytes, next.maxFileBytes),
  };
}

function emptyValidationSummary(): KnowledgeValidationSummary {
  return {
    acceptedPaths: [],
    acceptedFiles: 0,
    acceptedDirectories: 0,
    supportedFiles: 0,
    skippedFiles: [],
    maxFileBytes: DEFAULT_MAX_KNOWLEDGE_FILE_BYTES,
  };
}
