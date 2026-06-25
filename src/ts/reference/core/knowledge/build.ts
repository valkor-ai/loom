import { promises as fs, type Stats } from "node:fs";
import { createHash, randomBytes } from "node:crypto";
import path from "node:path";
import * as mammoth from "mammoth";
import { parse as parseYaml, stringify as stringifyYaml } from "yaml";
import { PDFParse } from "pdf-parse";
import { invalidArgument } from "../errors";
import { ensureDir, writeJsonAtomic, writeTextAtomic } from "../state/fs";
import { buildLexicalIndex } from "./lexical";
import { knowledgeBuildRunDir, knowledgeBuildRunFile } from "./paths";
import { prepareSemanticBuildRequests } from "./semantic";
import {
  DEFAULT_MAX_KNOWLEDGE_FILE_BYTES,
  KNOWLEDGE_SCHEMA_VERSION,
  SUPPORTED_KNOWLEDGE_EXTENSIONS,
  type KnowledgeBlockAffinity,
  type KnowledgeBuildResult,
  type KnowledgeBuildRun,
  type KnowledgeChunkRecord,
  type KnowledgeDocumentBlock,
  type KnowledgeDocumentRecord,
  type KnowledgeFileSnapshot,
  type KnowledgeRoot,
  type KnowledgeValidationWarning,
  type PendingKnowledgeOperation,
} from "./types";
import {
  findKnowledgeSource,
  readPendingKnowledge,
  validateKnowledgeName,
  writePendingKnowledge,
} from "./state";

const TARGET_TOKENS = 700;
const SOFT_MAX_CHUNK_TOKENS = 1200;
const HARD_MAX_CHUNK_TOKENS = 1800;
const MIN_TOKENS = 120;
const CONTEXT_PREFIX_MAX_TOKENS = 80;

const IGNORED_DIRECTORY_NAMES = new Set([
  ".git",
  ".loom",
  "node_modules",
  "dist",
  "build",
  ".DS_Store",
]);

type BuildRootScan = {
  roots: KnowledgeRoot[];
  files: string[];
  warnings: KnowledgeValidationWarning[];
};

type ParsedDocument = {
  path: string;
  title: string;
  extension: string;
  blocks: KnowledgeDocumentBlock[];
  snapshot: KnowledgeFileSnapshot;
};

type ChunkDraft = {
  title: string;
  headingPath: string[];
  text: string;
  tokenEstimate: number;
  splitReason: KnowledgeChunkRecord["splitReason"];
};

export async function buildKnowledgeSource(input: {
  name: string | undefined;
}): Promise<KnowledgeBuildResult> {
  const name = validateKnowledgeName(input.name);
  const source = await findKnowledgeSource(name);
  const pending = await readPendingKnowledge(name);
  if (!source && !pending) {
    throw invalidArgument(`Knowledge source "${name}" does not exist. Use knowledge add first.`, {
      name,
      suggestedCommand: `loom knowledge add --name ${name} <path...>`,
    });
  }

  const sourceId = source?.sourceId ?? pending?.sourceId ?? createKnowledgeSourceId(name);
  if (pending && pending.sourceId !== sourceId) {
    await writePendingKnowledge({
      ...pending,
      sourceId,
      updatedAt: new Date().toISOString(),
    });
  }

  const finalPaths = applyPendingOperations(source?.roots ?? [], pending?.operations ?? []);
  if (finalPaths.length === 0) {
    throw invalidArgument(`Knowledge source "${name}" has no paths to build.`, {
      name,
      suggestedCommand: `loom knowledge update ${name} --add-path <path>`,
    });
  }

  const scanned = await scanBuildRoots(finalPaths);
  if (scanned.files.length === 0) {
    throw invalidArgument(`Knowledge source "${name}" has no supported files to build.`, {
      name,
      roots: scanned.roots,
      skippedFiles: scanned.warnings,
    });
  }

  const buildId = createBuildId();
  const runDir = knowledgeBuildRunDir(sourceId, buildId);
  const chunksDir = path.join(runDir, "chunks");
  await ensureDir(chunksDir);

  const parsedDocuments: ParsedDocument[] = [];
  for (const filePath of scanned.files.sort((a, b) => a.localeCompare(b))) {
    parsedDocuments.push(await parseKnowledgeDocument(filePath));
  }

  const documents: KnowledgeDocumentRecord[] = [];
  const chunks: KnowledgeChunkRecord[] = [];
  let nextChunkNumber = 1;
  for (let documentIndex = 0; documentIndex < parsedDocuments.length; documentIndex += 1) {
    const parsed = parsedDocuments[documentIndex];
    const documentId = `kdoc_${String(documentIndex + 1).padStart(6, "0")}`;
    const chunkDrafts = chunkDocument(parsed);
    const chunkIds: string[] = [];
    for (const draft of chunkDrafts) {
      const chunkId = `kchunk_${String(nextChunkNumber).padStart(6, "0")}`;
      nextChunkNumber += 1;
      chunkIds.push(chunkId);
      const textRef = `chunks/${chunkId}.txt`;
      const bodyText = chunkBodyText(parsed, draft);
      await writeTextAtomic(path.join(runDir, textRef), bodyText);
      chunks.push({
        chunkId,
        documentId,
        sourceId,
        title: draft.title,
        headingPath: draft.headingPath,
        textRef,
        tokenEstimate: estimateTokens(bodyText),
        neighborChunkIds: [],
        contextPrefix: takeTokenPrefix(draft.text, CONTEXT_PREFIX_MAX_TOKENS),
        splitReason: draft.splitReason,
        retrievalFields: {
          title: draft.title,
          headingPath: draft.headingPath,
          summary: "",
          semanticLabelTexts: [],
          semanticAliases: [],
          bodyTextRef: textRef,
        },
        semanticLabels: [],
        blockAffinity: emptyBlockAffinity(),
      });
    }
    documents.push({
      documentId,
      sourceId,
      path: parsed.path,
      title: parsed.title,
      extension: parsed.extension,
      size: parsed.snapshot.size,
      mtimeMs: parsed.snapshot.mtimeMs,
      contentHash: parsed.snapshot.contentHash,
      chunkIds,
    });
  }

  for (let index = 0; index < chunks.length; index += 1) {
    chunks[index] = {
      ...chunks[index],
      neighborChunkIds: [
        ...(chunks[index - 1] ? [chunks[index - 1].chunkId] : []),
        ...(chunks[index + 1] ? [chunks[index + 1].chunkId] : []),
      ],
    };
  }

  for (const chunk of chunks) {
    if (chunk.tokenEstimate > HARD_MAX_CHUNK_TOKENS) {
      throw invalidArgument("Knowledge chunk exceeds the hard token limit after mechanical splitting.", {
        chunkId: chunk.chunkId,
        tokenEstimate: chunk.tokenEstimate,
        hardMaxChunkTokens: HARD_MAX_CHUNK_TOKENS,
      });
    }
  }

  const snapshot = parsedDocuments.map((document) => document.snapshot);
  const lexicalIndex = buildLexicalIndex(sourceId, buildId, chunks, runDir);
  const chunksPath = path.join(runDir, "chunks.json");
  const snapshotPath = path.join(runDir, "snapshot.json");
  const lexicalIndexPath = path.join(runDir, "lexical-index.json");
  const buildRunPath = knowledgeBuildRunFile(sourceId, buildId);

  await writeJsonAtomic(chunksPath, {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    sourceId,
    buildId,
    chunks,
  });
  await writeJsonAtomic(snapshotPath, {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    sourceId,
    buildId,
    files: snapshot,
  });
  await writeJsonAtomic(lexicalIndexPath, lexicalIndex);

  const now = new Date().toISOString();
  let buildRun: KnowledgeBuildRun = {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    buildId,
    sourceId,
    name,
    status: "semantic_pending",
    roots: scanned.roots,
    documents,
    chunks,
    files: snapshot,
    pendingOperations: pending?.operations ?? [],
    refs: {
      chunks: relativeToKnowledgeSource(sourceId, chunksPath),
      snapshot: relativeToKnowledgeSource(sourceId, snapshotPath),
      lexicalIndex: relativeToKnowledgeSource(sourceId, lexicalIndexPath),
    },
    createdAt: now,
    updatedAt: now,
  };
  const semanticPreparation = await prepareSemanticBuildRequests({
    buildRun,
    buildRunPath,
    runDir,
  });
  buildRun = {
    ...buildRun,
    refs: {
      ...buildRun.refs,
      semanticState: relativeToKnowledgeSource(sourceId, semanticPreparation.statePath),
    },
  };
  await writeJsonAtomic(buildRunPath, buildRun);

  return {
    name,
    sourceId,
    buildId,
    status: "semantic_pending",
    roots: scanned.roots,
    documentCount: documents.length,
    chunkCount: chunks.length,
    packCount: semanticPreparation.packCount,
    skippedFiles: scanned.warnings,
    buildRunPath,
    chunksPath,
    snapshotPath,
    lexicalIndexPath,
    firstRequestPath: semanticPreparation.firstRequestPath,
    firstRequest: semanticPreparation.firstRequest,
    message: `Knowledge source "${name}" semantic build is pending. Complete all semantic packs before this index is published.`,
  };
}

function applyPendingOperations(
  roots: KnowledgeRoot[],
  operations: PendingKnowledgeOperation[],
): string[] {
  let paths = roots.map((root) => root.path);
  for (const operation of operations) {
    if (operation.type === "replace_paths") {
      paths = [...operation.paths];
      continue;
    }
    if (operation.type === "add_paths") {
      paths.push(...operation.paths);
      continue;
    }
    const remove = new Set(operation.paths.map((entry) => path.resolve(entry)));
    paths = paths.filter((entry) => !remove.has(path.resolve(entry)));
  }
  return [...new Set(paths.map((entry) => path.resolve(entry)))];
}

async function scanBuildRoots(inputPaths: string[]): Promise<BuildRootScan> {
  const roots: KnowledgeRoot[] = [];
  const files: string[] = [];
  const warnings: KnowledgeValidationWarning[] = [];
  for (const inputPath of inputPaths) {
    const stat = await readableStat(inputPath);
    if (stat.isFile()) {
      const warning = validateSupportedFile(inputPath, stat.size, true);
      if (warning) {
        throw invalidArgument(warning.message, {
          path: inputPath,
          reason: warning.reason,
        });
      }
      roots.push({ type: "file", path: inputPath });
      files.push(inputPath);
      continue;
    }
    if (stat.isDirectory()) {
      const scanned = await scanDirectory(inputPath);
      if (scanned.files.length === 0) {
        throw invalidArgument("Knowledge directory does not contain any supported files.", {
          path: inputPath,
          skippedFiles: scanned.warnings,
        });
      }
      roots.push({ type: "directory", path: inputPath });
      files.push(...scanned.files);
      warnings.push(...scanned.warnings);
      continue;
    }
    throw invalidArgument("Knowledge path must be a file or directory.", { path: inputPath });
  }
  return {
    roots,
    files: [...new Set(files.map((file) => path.resolve(file)))],
    warnings,
  };
}

async function scanDirectory(dir: string): Promise<{
  files: string[];
  warnings: KnowledgeValidationWarning[];
}> {
  const files: string[] = [];
  const warnings: KnowledgeValidationWarning[] = [];
  const entries = await fs.readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (IGNORED_DIRECTORY_NAMES.has(entry.name)) {
      continue;
    }
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const child = await scanDirectory(entryPath);
      files.push(...child.files);
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
    const warning = validateSupportedFile(entryPath, stat.size, false);
    if (warning) {
      warnings.push(warning);
    } else {
      files.push(entryPath);
    }
  }
  return { files, warnings };
}

async function readableStat(filePath: string): Promise<Stats> {
  try {
    return await fs.stat(filePath);
  } catch {
    throw invalidArgument("Knowledge path does not exist or is not readable.", {
      path: filePath,
    });
  }
}

function validateSupportedFile(filePath: string, size: number, explicit: boolean): KnowledgeValidationWarning | null {
  const ext = path.extname(filePath).toLowerCase();
  if (!SUPPORTED_KNOWLEDGE_EXTENSIONS.includes(ext as typeof SUPPORTED_KNOWLEDGE_EXTENSIONS[number])) {
    return {
      path: filePath,
      reason: "unsupported_file_type",
      message: explicit
        ? `Unsupported knowledge file type: ${filePath}.`
        : `Skipped unsupported knowledge file type: ${filePath}.`,
    };
  }
  if (size > DEFAULT_MAX_KNOWLEDGE_FILE_BYTES) {
    return {
      path: filePath,
      reason: "file_too_large",
      message: explicit
        ? `Knowledge file is too large: ${filePath}.`
        : `Skipped oversized knowledge file: ${filePath}.`,
    };
  }
  return null;
}

async function parseKnowledgeDocument(filePath: string): Promise<ParsedDocument> {
  const stat = await fs.stat(filePath);
  const contentHash = await hashFile(filePath);
  const extension = path.extname(filePath).toLowerCase();
  const snapshot = {
    path: filePath,
    size: stat.size,
    mtimeMs: stat.mtimeMs,
    contentHash,
    extension,
  };
  const title = path.basename(filePath);
  if (extension === ".md") {
    return {
      path: filePath,
      title,
      extension,
      snapshot,
      blocks: parseMarkdown(await fs.readFile(filePath, "utf8")),
    };
  }
  if (extension === ".txt") {
    return {
      path: filePath,
      title,
      extension,
      snapshot,
      blocks: parsePlainText(await fs.readFile(filePath, "utf8")),
    };
  }
  if (extension === ".json") {
    const raw = await fs.readFile(filePath, "utf8");
    return {
      path: filePath,
      title,
      extension,
      snapshot,
      blocks: parseStructuredText(title, normalizeJsonText(raw)),
    };
  }
  if (extension === ".yaml" || extension === ".yml") {
    const raw = await fs.readFile(filePath, "utf8");
    return {
      path: filePath,
      title,
      extension,
      snapshot,
      blocks: parseStructuredText(title, normalizeYamlText(raw)),
    };
  }
  if (extension === ".pdf") {
    return {
      path: filePath,
      title,
      extension,
      snapshot,
      blocks: parsePlainText(await extractPdfText(filePath)),
    };
  }
  if (extension === ".docx") {
    const extracted = await mammoth.extractRawText({ path: filePath });
    return {
      path: filePath,
      title,
      extension,
      snapshot,
      blocks: parsePlainText(extracted.value),
    };
  }
  throw invalidArgument("Unsupported knowledge file type.", {
    path: filePath,
    supportedExtensions: SUPPORTED_KNOWLEDGE_EXTENSIONS,
  });
}

function parseMarkdown(raw: string): KnowledgeDocumentBlock[] {
  const lines = raw.replace(/\r\n/g, "\n").split("\n");
  const blocks: KnowledgeDocumentBlock[] = [];
  let paragraph: string[] = [];
  let listItems: string[] = [];
  let tableRows: string[][] = [];
  let codeLines: string[] = [];
  let inCode = false;

  const flushParagraph = () => {
    if (paragraph.length > 0) {
      blocks.push({ type: "paragraph", text: paragraph.join(" ").trim() });
      paragraph = [];
    }
  };
  const flushList = () => {
    if (listItems.length > 0) {
      blocks.push({ type: "list", items: listItems });
      listItems = [];
    }
  };
  const flushTable = () => {
    if (tableRows.length > 0) {
      const [header = [], ...rows] = tableRows;
      blocks.push({ type: "table", header, rows });
      tableRows = [];
    }
  };

  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith("```")) {
      if (inCode) {
        blocks.push({ type: "code", text: codeLines.join("\n") });
        codeLines = [];
        inCode = false;
      } else {
        flushParagraph();
        flushList();
        flushTable();
        inCode = true;
      }
      continue;
    }
    if (inCode) {
      codeLines.push(line);
      continue;
    }
    if (!trimmed) {
      flushParagraph();
      flushList();
      flushTable();
      continue;
    }
    const heading = /^(#{1,6})\s+(.+)$/.exec(trimmed);
    if (heading) {
      flushParagraph();
      flushList();
      flushTable();
      blocks.push({ type: "heading", level: heading[1].length, text: heading[2].trim() });
      continue;
    }
    const list = /^(?:[-*+]|\d+[.)])\s+(.+)$/.exec(trimmed);
    if (list) {
      flushParagraph();
      flushTable();
      listItems.push(list[1].trim());
      continue;
    }
    if (trimmed.includes("|") && trimmed.startsWith("|")) {
      flushParagraph();
      flushList();
      const cells = trimmed.split("|").map((cell) => cell.trim()).filter(Boolean);
      if (cells.length > 0 && !cells.every((cell) => /^-+$/.test(cell))) {
        tableRows.push(cells);
      }
      continue;
    }
    flushList();
    flushTable();
    paragraph.push(trimmed);
  }
  flushParagraph();
  flushList();
  flushTable();
  if (inCode && codeLines.length > 0) {
    blocks.push({ type: "code", text: codeLines.join("\n") });
  }
  return blocks;
}

function parsePlainText(raw: string): KnowledgeDocumentBlock[] {
  const paragraphs = raw
    .replace(/\r\n/g, "\n")
    .split(/\n\s*\n/g)
    .map((entry) => entry.trim())
    .filter(Boolean);
  return paragraphs.length > 0
    ? paragraphs.map((text) => ({ type: "paragraph" as const, text }))
    : [{ type: "paragraph", text: raw.trim() || "(empty document)" }];
}

function parseStructuredText(title: string, text: string): KnowledgeDocumentBlock[] {
  return [
    { type: "heading", level: 1, text: title },
    { type: "code", text },
  ];
}

function normalizeJsonText(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

function normalizeYamlText(raw: string): string {
  try {
    return stringifyYaml(parseYaml(raw));
  } catch {
    return raw;
  }
}

async function extractPdfText(filePath: string): Promise<string> {
  const parser = new PDFParse({ data: await fs.readFile(filePath) });
  try {
    const result = await parser.getText();
    return result.text;
  } finally {
    await parser.destroy();
  }
}

function chunkDocument(document: ParsedDocument): ChunkDraft[] {
  const headingPath: string[] = [];
  const units: ChunkDraft[] = [];
  for (const block of document.blocks) {
    if (block.type === "heading") {
      headingPath.splice(block.level - 1);
      headingPath[block.level - 1] = block.text;
      continue;
    }
    if (block.type === "pageBreak") {
      continue;
    }
    const text = blockToText(block);
    if (!text.trim()) {
      continue;
    }
    const title = headingPath[headingPath.length - 1] ?? document.title;
    for (const split of splitOversizedUnit(text)) {
      units.push({
        title,
        headingPath: [...headingPath],
        text: split.text,
        tokenEstimate: estimateTokens(split.text),
        splitReason: split.reason,
      });
    }
  }
  if (units.length === 0) {
    units.push({
      title: document.title,
      headingPath: [],
      text: "(empty document)",
      tokenEstimate: estimateTokens("(empty document)"),
      splitReason: "section",
    });
  }
  return mergeSmallChunks(packChunkUnits(units));
}

function blockToText(block: Exclude<KnowledgeDocumentBlock, { type: "heading" | "pageBreak" }>): string {
  if (block.type === "paragraph") {
    return block.text;
  }
  if (block.type === "list") {
    return block.items.map((item) => `- ${item}`).join("\n");
  }
  if (block.type === "table") {
    return [
      block.header.join(" | "),
      ...block.rows.map((row) => row.join(" | ")),
    ].filter((row) => row.trim().length > 0).join("\n");
  }
  return block.text;
}

function splitOversizedUnit(text: string): Array<{
  text: string;
  reason: KnowledgeChunkRecord["splitReason"];
}> {
  const estimate = estimateTokens(text);
  if (estimate <= HARD_MAX_CHUNK_TOKENS) {
    return [{ text, reason: "section" }];
  }
  const parts = splitBySentenceBoundary(text);
  if (parts.length > 1) {
    const grouped: Array<{ text: string; reason: KnowledgeChunkRecord["splitReason"] }> = [];
    let current = "";
    for (const part of parts) {
      const projected = current ? `${current}\n${part}` : part;
      if (estimateTokens(projected) > HARD_MAX_CHUNK_TOKENS && current) {
        grouped.push({ text: current, reason: "hard_boundary" });
        current = part;
      } else {
        current = projected;
      }
    }
    if (current) {
      grouped.push({ text: current, reason: "hard_boundary" });
    }
    if (grouped.every((entry) => estimateTokens(entry.text) <= HARD_MAX_CHUNK_TOKENS)) {
      return grouped;
    }
  }
  return splitByWindow(text).map((entry) => ({
    text: entry,
    reason: "hard_window_fallback" as const,
  }));
}

function packChunkUnits(units: ChunkDraft[]): ChunkDraft[] {
  const chunks: ChunkDraft[] = [];
  let current: ChunkDraft | null = null;
  for (const unit of units) {
    if (!current) {
      current = { ...unit };
      continue;
    }
    const sameSection = current.headingPath.join("\u0000") === unit.headingPath.join("\u0000");
    const projectedText: string = `${current.text}\n\n${unit.text}`;
    const projectedTokens = estimateTokens(projectedText);
    if (!sameSection || (current.tokenEstimate >= MIN_TOKENS && projectedTokens > TARGET_TOKENS) || projectedTokens > SOFT_MAX_CHUNK_TOKENS) {
      chunks.push(current);
      current = { ...unit };
      continue;
    }
    current = {
      ...current,
      text: projectedText,
      tokenEstimate: projectedTokens,
      splitReason: current.splitReason === "section" ? "soft_boundary" : current.splitReason,
    };
  }
  if (current) {
    chunks.push(current);
  }
  return chunks;
}

function mergeSmallChunks(chunks: ChunkDraft[]): ChunkDraft[] {
  const merged: ChunkDraft[] = [];
  for (const chunk of chunks) {
    const previous = merged[merged.length - 1];
    const sameSection = previous && previous.headingPath.join("\u0000") === chunk.headingPath.join("\u0000");
    const projected = previous ? `${previous.text}\n\n${chunk.text}` : chunk.text;
    if (previous && sameSection && chunk.tokenEstimate < MIN_TOKENS && estimateTokens(projected) <= SOFT_MAX_CHUNK_TOKENS) {
      merged[merged.length - 1] = {
        ...previous,
        text: projected,
        tokenEstimate: estimateTokens(projected),
        splitReason: "merged_small",
      };
      continue;
    }
    merged.push(chunk);
  }
  return merged;
}

function splitBySentenceBoundary(text: string): string[] {
  return text
    .split(/(?<=[。！？.!?；;：:])\s+/u)
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function splitByWindow(text: string): string[] {
  const parts: string[] = [];
  let remaining = text;
  while (estimateTokens(remaining) > HARD_MAX_CHUNK_TOKENS) {
    let windowSize = Math.min(remaining.length, 1800);
    while (windowSize > 200 && estimateTokens(remaining.slice(0, windowSize)) > HARD_MAX_CHUNK_TOKENS) {
      windowSize = Math.floor(windowSize * 0.8);
    }
    parts.push(remaining.slice(0, windowSize));
    remaining = remaining.slice(windowSize);
  }
  if (remaining.trim()) {
    parts.push(remaining);
  }
  return parts;
}

function chunkBodyText(document: ParsedDocument, chunk: ChunkDraft): string {
  return [
    `Document: ${document.title}`,
    `Path: ${document.path}`,
    `Section: ${chunk.headingPath.length > 0 ? chunk.headingPath.join(" > ") : "(root)"}`,
    "",
    chunk.text,
    "",
  ].join("\n");
}

function estimateTokens(text: string): number {
  const chineseChars = text.match(/[\u3400-\u9fff]/g)?.length ?? 0;
  const latinWords = text.match(/[A-Za-z0-9_]+/g)?.length ?? 0;
  const symbols = text.replace(/[\sA-Za-z0-9_\u3400-\u9fff]/g, "").length;
  return Math.max(1, Math.ceil(chineseChars * 0.7 + latinWords * 1.3 + symbols * 0.3));
}

function takeTokenPrefix(text: string, maxTokens: number): string {
  let prefix = "";
  for (const char of text) {
    const next = `${prefix}${char}`;
    if (estimateTokens(next) > maxTokens) {
      break;
    }
    prefix = next;
  }
  return prefix.trim();
}

function emptyBlockAffinity(): KnowledgeBlockAffinity {
  return {
    phaseScope: 0,
    conceptGrounding: 0,
    frontendExperience: 0,
    finalSummary: 0,
  };
}

async function hashFile(filePath: string): Promise<string> {
  const hash = createHash("sha256");
  hash.update(await fs.readFile(filePath));
  return `sha256:${hash.digest("hex")}`;
}

function createKnowledgeSourceId(name: string): string {
  const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 32);
  return `ksrc_${slug || "source"}_${randomBytes(4).toString("hex")}`;
}

function createBuildId(): string {
  const timestamp = new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
  return `kbld_${timestamp}_${randomBytes(4).toString("hex")}`;
}

function relativeToKnowledgeSource(sourceId: string, filePath: string): string {
  const sourceRootMarker = `${path.sep}${sourceId}${path.sep}`;
  const markerIndex = filePath.indexOf(sourceRootMarker);
  if (markerIndex < 0) {
    return filePath;
  }
  return filePath.slice(markerIndex + sourceRootMarker.length);
}
