import { promises as fs } from "node:fs";
import path from "node:path";
import { z } from "zod";
import { invalidArgument, stateCorrupted } from "../errors";
import { ensureDir, pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import { knowledgePaths, pendingKnowledgeFile, knowledgeSourceDir } from "./paths";
import {
  KNOWLEDGE_SCHEMA_VERSION,
  type KnowledgePendingSource,
  type KnowledgeRegistry,
  type KnowledgeSource,
} from "./types";

const knowledgeRootSchema = z.object({
  type: z.enum(["file", "directory"]),
  path: z.string().min(1),
});

const knowledgeSourceSchema: z.ZodType<KnowledgeSource> = z.object({
  sourceId: z.string().min(1),
  name: z.string().min(1),
  status: z.enum(["enabled", "disabled"]),
  roots: z.array(knowledgeRootSchema),
  index: z.object({
    version: z.number().int().nonnegative(),
    lastBuiltAt: z.string().nullable(),
    documentCount: z.number().int().nonnegative(),
    chunkCount: z.number().int().nonnegative(),
  }),
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
});

const validationWarningSchema = z.object({
  path: z.string().min(1),
  reason: z.enum(["unsupported_file_type", "file_too_large", "unreadable_path"]),
  message: z.string().min(1),
});

const validationSummarySchema = z.object({
  acceptedPaths: z.array(z.string().min(1)),
  acceptedFiles: z.number().int().nonnegative(),
  acceptedDirectories: z.number().int().nonnegative(),
  supportedFiles: z.number().int().nonnegative(),
  skippedFiles: z.array(validationWarningSchema),
  maxFileBytes: z.number().int().positive(),
});

const pendingOperationSchema = z.union([
  z.object({
    type: z.literal("add_paths"),
    paths: z.array(z.string().min(1)),
  }),
  z.object({
    type: z.literal("remove_paths"),
    paths: z.array(z.string().min(1)),
  }),
  z.object({
    type: z.literal("replace_paths"),
    paths: z.array(z.string().min(1)),
  }),
]);

const pendingSourceSchema: z.ZodType<KnowledgePendingSource> = z.object({
  schemaVersion: z.literal(KNOWLEDGE_SCHEMA_VERSION),
  name: z.string().min(1),
  sourceId: z.string().min(1).nullable(),
  createNew: z.boolean(),
  operations: z.array(pendingOperationSchema),
  validation: validationSummarySchema,
  createdAt: z.string().min(1),
  updatedAt: z.string().min(1),
});

const registrySchema: z.ZodType<KnowledgeRegistry> = z.object({
  schemaVersion: z.literal(KNOWLEDGE_SCHEMA_VERSION),
  sources: z.array(knowledgeSourceSchema),
});

export async function ensureKnowledgeStore(): Promise<void> {
  const paths = knowledgePaths();
  await ensureDir(paths.root);
  await ensureDir(paths.pendingDir);
  await ensureDir(paths.sourcesDir);
  if (!(await pathExists(paths.registryFile))) {
    await writeJsonAtomic(paths.registryFile, {
      schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
      sources: [],
    } satisfies KnowledgeRegistry);
  }
}

export async function readKnowledgeRegistry(): Promise<KnowledgeRegistry> {
  await ensureKnowledgeStore();
  try {
    return registrySchema.parse(await readJsonFile(knowledgePaths().registryFile));
  } catch (error) {
    if (error instanceof z.ZodError) {
      throw stateCorrupted("Knowledge registry is corrupted.", {
        file: knowledgePaths().registryFile,
        issues: error.issues,
      });
    }
    throw error;
  }
}

export async function writeKnowledgeRegistry(registry: KnowledgeRegistry): Promise<void> {
  await ensureKnowledgeStore();
  await writeJsonAtomic(knowledgePaths().registryFile, registrySchema.parse(registry));
}

export async function findKnowledgeSource(name: string): Promise<KnowledgeSource | null> {
  const registry = await readKnowledgeRegistry();
  return registry.sources.find((source) => source.name === name) ?? null;
}

export async function upsertKnowledgeSource(source: KnowledgeSource): Promise<void> {
  const registry = await readKnowledgeRegistry();
  const existingIndex = registry.sources.findIndex((entry) => entry.name === source.name);
  const nextSources = existingIndex >= 0
    ? registry.sources.map((entry, index) => index === existingIndex ? source : entry)
    : [...registry.sources, source];
  await writeKnowledgeRegistry({
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    sources: nextSources,
  });
}

export async function removeKnowledgeSource(name: string): Promise<boolean> {
  const registry = await readKnowledgeRegistry();
  const source = registry.sources.find((entry) => entry.name === name);
  if (!source) {
    return false;
  }
  await writeKnowledgeRegistry({
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    sources: registry.sources.filter((entry) => entry.name !== name),
  });
  await fs.rm(knowledgeSourceDir(source.sourceId), { recursive: true, force: true });
  return true;
}

export async function readPendingKnowledge(name: string): Promise<KnowledgePendingSource | null> {
  const file = pendingKnowledgeFile(name);
  if (!(await pathExists(file))) {
    return null;
  }
  try {
    return pendingSourceSchema.parse(await readJsonFile(file));
  } catch (error) {
    if (error instanceof z.ZodError) {
      throw stateCorrupted("Knowledge pending queue is corrupted.", {
        file,
        issues: error.issues,
      });
    }
    throw error;
  }
}

export async function writePendingKnowledge(pending: KnowledgePendingSource): Promise<void> {
  validateKnowledgeName(pending.name);
  await ensureKnowledgeStore();
  await writeJsonAtomic(pendingKnowledgeFile(pending.name), pendingSourceSchema.parse(pending));
}

export async function listPendingKnowledge(): Promise<KnowledgePendingSource[]> {
  await ensureKnowledgeStore();
  const entries = await fs.readdir(knowledgePaths().pendingDir);
  const result: KnowledgePendingSource[] = [];
  for (const entry of entries.sort()) {
    if (!entry.endsWith(".json")) {
      continue;
    }
    const file = path.join(knowledgePaths().pendingDir, entry);
    try {
      result.push(pendingSourceSchema.parse(await readJsonFile(file)));
    } catch (error) {
      if (error instanceof z.ZodError) {
        throw stateCorrupted("Knowledge pending queue is corrupted.", {
          file,
          issues: error.issues,
        });
      }
      throw error;
    }
  }
  return result;
}

export async function removePendingKnowledge(name: string): Promise<boolean> {
  const file = pendingKnowledgeFile(name);
  const existed = await pathExists(file);
  await fs.rm(file, { force: true });
  return existed;
}

export function validateKnowledgeName(name: string | undefined): string {
  const normalized = typeof name === "string" ? name.trim() : "";
  if (!normalized) {
    throw invalidArgument("Knowledge source name is required.", {
      example: "loom knowledge add --name securities-domain ~/Documents/securities",
    });
  }
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{1,79}$/.test(normalized)) {
    throw invalidArgument("Knowledge source name must be 2-80 characters and use letters, numbers, dot, underscore, or dash.", {
      name,
    });
  }
  return normalized;
}
