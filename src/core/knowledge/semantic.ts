import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument } from "../errors";
import { readJsonFile, writeJsonAtomic } from "../state/fs";
import { buildLexicalIndex } from "./lexical";
import {
  knowledgeSemanticRepairFile,
  knowledgeSemanticRequestFile,
  knowledgeSemanticResultFile,
  knowledgeSemanticStateFile,
} from "./paths";
import {
  KNOWLEDGE_SCHEMA_VERSION,
  type KnowledgeBlockAffinity,
  type KnowledgeBuildRun,
  type KnowledgeChunkRecord,
  type KnowledgeRoot,
  type KnowledgeSemanticBuildRequest,
  type KnowledgeSemanticBuildState,
  type KnowledgeSemanticChunkResult,
  type KnowledgeSemanticIndex,
  type KnowledgeSemanticLabel,
  type KnowledgeSemanticPackInfo,
  type KnowledgeSemanticPackResult,
  type KnowledgeSemanticSubmitIssue,
  type KnowledgeSemanticSubmitResult,
} from "./types";
import {
  findKnowledgeSource,
  removePendingKnowledge,
  upsertKnowledgeSource,
} from "./state";

const MAX_PACK_INPUT_TOKENS = 7000;
const FIXED_REQUEST_OVERHEAD_ESTIMATE = 1000;
const PER_CHUNK_METADATA_ESTIMATE = 150;
const EFFECTIVE_PACK_BUDGET = MAX_PACK_INPUT_TOKENS - FIXED_REQUEST_OVERHEAD_ESTIMATE;

const LABEL_KINDS: KnowledgeSemanticLabel["kind"][] = [
  "object",
  "operation",
  "state",
  "rule",
  "field",
  "page",
  "flow",
  "other",
];

const CONFIDENCE_VALUES: KnowledgeSemanticLabel["confidence"][] = ["low", "medium", "high"];
const CHUNK_RESULT_STATUSES: KnowledgeSemanticChunkResult["status"][] = ["completed", "low_signal", "unreadable"];

export async function prepareSemanticBuildRequests(input: {
  buildRun: KnowledgeBuildRun;
  buildRunPath: string;
  runDir: string;
}): Promise<{
  packCount: number;
  statePath: string;
  firstRequestPath: string;
  firstRequest: KnowledgeSemanticBuildRequest;
}> {
  const packs = packChunks(input.buildRun.chunks);
  const now = new Date().toISOString();
  const packInfos: KnowledgeSemanticPackInfo[] = [];
  const requests: KnowledgeSemanticBuildRequest[] = [];
  for (let index = 0; index < packs.length; index += 1) {
    const packIndex = index + 1;
    const packId = `kpack_${String(packIndex).padStart(4, "0")}`;
    const requestPath = knowledgeSemanticRequestFile(input.buildRun.sourceId, input.buildRun.buildId, packId);
    const resultFile = knowledgeSemanticResultFile(input.buildRun.sourceId, input.buildRun.buildId, packId);
    const request = createSemanticRequest({
      buildRun: input.buildRun,
      buildRunPath: input.buildRunPath,
      runDir: input.runDir,
      packId,
      packIndex,
      packCount: packs.length,
      chunks: packs[index],
      requestPath,
      resultFile,
    });
    packInfos.push({
      packId,
      packIndex,
      chunkIds: packs[index].map((chunk) => chunk.chunkId),
      requestPath,
      resultFile,
    });
    requests.push(request);
    await writeJsonAtomic(requestPath, request);
  }
  const state: KnowledgeSemanticBuildState = {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    buildId: input.buildRun.buildId,
    sourceId: input.buildRun.sourceId,
    sourceName: input.buildRun.name,
    status: "pending",
    packCount: packs.length,
    acceptedPackIds: [],
    packs: packInfos,
    createdAt: now,
    updatedAt: now,
  };
  const statePath = knowledgeSemanticStateFile(input.buildRun.sourceId, input.buildRun.buildId);
  await writeJsonAtomic(statePath, state);
  return {
    packCount: packs.length,
    statePath,
    firstRequestPath: packInfos[0].requestPath,
    firstRequest: requests[0],
  };
}

export async function submitKnowledgeSemanticPack(input: {
  requestFile: string | undefined;
  resultFile: string | undefined;
}): Promise<KnowledgeSemanticSubmitResult> {
  const requestPath = requirePath(input.requestFile, "--request");
  const resultPath = requirePath(input.resultFile, "--result-file");
  const request = asSemanticBuildRequest(await readJsonFile(requestPath));
  const statePath = knowledgeSemanticStateFile(request.sourceId, request.buildId);
  const state = asSemanticBuildState(await readJsonFile(statePath));
  const rawResult = await readPackResultJson(resultPath);
  if (!rawResult.ok) {
    return writeRepairResult({
      request,
      state,
      resultFile: resultPath,
      issues: [{
        code: "invalid_json",
        message: rawResult.message,
      }],
    });
  }

  const validation = validatePackResult(request, rawResult.value);
  if (validation.issues.length > 0 || !validation.result) {
    return writeRepairResult({
      request,
      state,
      resultFile: resultPath,
      issues: validation.issues,
    });
  }

  await writeJsonAtomic(request.outputContract.resultFile, validation.result);
  const acceptedPackIds = [...new Set([...state.acceptedPackIds, request.packId])];
  const nextState: KnowledgeSemanticBuildState = {
    ...state,
    acceptedPackIds,
    updatedAt: new Date().toISOString(),
  };

  if (acceptedPackIds.length < state.packCount) {
    await writeJsonAtomic(statePath, nextState);
    const nextPack = state.packs.find((pack) => !acceptedPackIds.includes(pack.packId));
    if (!nextPack) {
      throw invalidArgument("Knowledge semantic build state is missing the next pack.", {
        buildId: request.buildId,
      });
    }
    const nextRequest = asSemanticBuildRequest(await readJsonFile(nextPack.requestPath));
    return {
      status: "accepted",
      buildId: request.buildId,
      packId: request.packId,
      acceptedPackIds,
      packCount: state.packCount,
      nextRequestPath: nextPack.requestPath,
      nextRequest,
      message: `Accepted semantic pack ${request.packIndex}/${state.packCount}. Continue with the next pack.`,
    };
  }

  const published = await publishSemanticBuild({
    request,
    state: {
      ...nextState,
      status: "published",
    },
  });
  return {
    status: "accepted",
    buildId: request.buildId,
    packId: request.packId,
    acceptedPackIds,
    packCount: state.packCount,
    published,
    message: `Knowledge source "${published.name}" has been published.`,
  };
}

function packChunks(chunks: KnowledgeChunkRecord[]): KnowledgeChunkRecord[][] {
  const packs: KnowledgeChunkRecord[][] = [];
  let current: KnowledgeChunkRecord[] = [];
  let currentCost = 0;
  for (const chunk of chunks) {
    const chunkCost = chunk.tokenEstimate + PER_CHUNK_METADATA_ESTIMATE;
    if (chunkCost > EFFECTIVE_PACK_BUDGET) {
      throw invalidArgument("Knowledge chunk exceeds semantic pack input budget.", {
        chunkId: chunk.chunkId,
        tokenEstimate: chunk.tokenEstimate,
        effectivePackBudget: EFFECTIVE_PACK_BUDGET,
      });
    }
    if (current.length === 0) {
      current.push(chunk);
      currentCost = chunkCost;
      continue;
    }
    if (currentCost + chunkCost <= EFFECTIVE_PACK_BUDGET) {
      current.push(chunk);
      currentCost += chunkCost;
      continue;
    }
    packs.push(current);
    current = [chunk];
    currentCost = chunkCost;
  }
  if (current.length > 0) {
    packs.push(current);
  }
  if (packs.length === 0) {
    throw invalidArgument("Knowledge build has no chunks to pack.", {});
  }
  return packs;
}

function createSemanticRequest(input: {
  buildRun: KnowledgeBuildRun;
  buildRunPath: string;
  runDir: string;
  packId: string;
  packIndex: number;
  packCount: number;
  chunks: KnowledgeChunkRecord[];
  requestPath: string;
  resultFile: string;
}): KnowledgeSemanticBuildRequest {
  const documents = new Map(input.buildRun.documents.map((document) => [document.documentId, document]));
  const chunkIndex = new Map(input.buildRun.chunks.map((chunk, index) => [chunk.chunkId, index]));
  const chunkRefs = input.chunks.map((chunk) => path.join(input.runDir, chunk.textRef));
  return {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    requestId: `ksemreq_${input.buildRun.buildId}_${input.packId}`,
    buildId: input.buildRun.buildId,
    buildRunPath: input.buildRunPath,
    sourceId: input.buildRun.sourceId,
    sourceName: input.buildRun.name,
    packId: input.packId,
    packIndex: input.packIndex,
    packCount: input.packCount,
    chunkPack: {
      chunks: input.chunks.map((chunk) => {
        const document = documents.get(chunk.documentId);
        const index = chunkIndex.get(chunk.chunkId) ?? -1;
        const previous = index > 0 ? input.buildRun.chunks[index - 1] : undefined;
        const next = index >= 0 ? input.buildRun.chunks[index + 1] : undefined;
        return {
          chunkId: chunk.chunkId,
          documentId: chunk.documentId,
          documentTitle: document?.title ?? chunk.title,
          relativePath: relativeDocumentPath(document?.path ?? chunk.title, input.buildRun.roots),
          headingPath: chunk.headingPath,
          tokenEstimate: chunk.tokenEstimate,
          textRef: path.join(input.runDir, chunk.textRef),
          readCommand: {
            argv: [
              "knowledge",
              "inspect",
              "--source",
              input.buildRun.name,
              "--build-id",
              input.buildRun.buildId,
              "--chunk",
              chunk.chunkId,
            ],
          },
          ...(previous ? { previousChunkTitle: previous.title } : {}),
          ...(next ? { nextChunkTitle: next.title } : {}),
          splitReason: chunk.splitReason,
        };
      }),
    },
    outputContract: {
      resultFile: input.resultFile,
      schema: "KnowledgeSemanticPackResult",
    },
    generationRules: {
      labelKinds: LABEL_KINDS,
      confidenceValues: CONFIDENCE_VALUES,
      summaryRule: "Write a concise summary of this chunk only. Do not add external knowledge.",
      semanticLabelRule: "Generate labels only from the chunk text, title, heading path, or local neighboring context. An empty label list is valid for low-signal chunks.",
      blockAffinityRule: "Score affinity from 0 to 1 for each Brainstorm block based only on this chunk.",
    },
    submitCommand: {
      argv: ["knowledge", "semantic", "submit", "--request", input.requestPath, "--result-file", input.resultFile],
    },
    requestReadPlan: {
      mustReadChunkText: true,
      chunkTextRefs: chunkRefs,
    },
  };
}

async function readPackResultJson(filePath: string): Promise<
  | { ok: true; value: unknown }
  | { ok: false; message: string }
> {
  let raw: string;
  try {
    raw = await fs.readFile(filePath, "utf8");
  } catch {
    return { ok: false, message: `Cannot read semantic pack result file: ${filePath}` };
  }
  try {
    return { ok: true, value: JSON.parse(raw) };
  } catch {
    return { ok: false, message: `Semantic pack result is not valid JSON: ${filePath}` };
  }
}

function validatePackResult(request: KnowledgeSemanticBuildRequest, value: unknown): {
  result: KnowledgeSemanticPackResult | null;
  issues: KnowledgeSemanticSubmitIssue[];
} {
  const issues: KnowledgeSemanticSubmitIssue[] = [];
  if (!isRecord(value)) {
    return {
      result: null,
      issues: [{ code: "invalid_shape", message: "Result must be a JSON object." }],
    };
  }
  if (value.schemaVersion !== KNOWLEDGE_SCHEMA_VERSION) {
    issues.push({ code: "schema_version", message: "schemaVersion must be 1.0.", path: "schemaVersion" });
  }
  if (value.buildId !== request.buildId) {
    issues.push({ code: "build_id_mismatch", message: "buildId does not match request.", path: "buildId" });
  }
  if (value.packId !== request.packId) {
    issues.push({ code: "pack_id_mismatch", message: "packId does not match request.", path: "packId" });
  }
  if (!Array.isArray(value.chunkResults)) {
    issues.push({ code: "chunk_results_missing", message: "chunkResults must be an array.", path: "chunkResults" });
    return { result: null, issues };
  }

  const requestChunkIds = new Set(request.chunkPack.chunks.map((chunk) => chunk.chunkId));
  const seen = new Set<string>();
  const chunkResults: KnowledgeSemanticChunkResult[] = [];
  for (let index = 0; index < value.chunkResults.length; index += 1) {
    const raw = value.chunkResults[index];
    const basePath = `chunkResults[${index}]`;
    const result = validateChunkResult(raw, basePath, issues);
    if (!result) {
      continue;
    }
    if (!requestChunkIds.has(result.chunkId)) {
      issues.push({
        code: "chunk_outside_pack",
        message: "chunkResult contains a chunkId outside this pack.",
        path: `${basePath}.chunkId`,
        chunkId: result.chunkId,
      });
    }
    if (seen.has(result.chunkId)) {
      issues.push({
        code: "duplicate_chunk_result",
        message: "chunkResult contains a duplicate chunkId.",
        path: `${basePath}.chunkId`,
        chunkId: result.chunkId,
      });
    }
    seen.add(result.chunkId);
    chunkResults.push(result);
  }
  for (const chunkId of requestChunkIds) {
    if (!seen.has(chunkId)) {
      issues.push({
        code: "missing_chunk_result",
        message: "Pack result is missing a chunk result.",
        chunkId,
      });
    }
  }
  if (issues.length > 0) {
    return { result: null, issues };
  }
  return {
    result: {
      schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
      buildId: request.buildId,
      packId: request.packId,
      chunkResults,
    },
    issues: [],
  };
}

function validateChunkResult(
  value: unknown,
  basePath: string,
  issues: KnowledgeSemanticSubmitIssue[],
): KnowledgeSemanticChunkResult | null {
  if (!isRecord(value)) {
    issues.push({ code: "invalid_chunk_result", message: "chunk result must be an object.", path: basePath });
    return null;
  }
  const chunkId = typeof value.chunkId === "string" ? value.chunkId : "";
  if (!chunkId) {
    issues.push({ code: "chunk_id_missing", message: "chunkId is required.", path: `${basePath}.chunkId` });
  }
  if (!CHUNK_RESULT_STATUSES.includes(value.status as KnowledgeSemanticChunkResult["status"])) {
    issues.push({ code: "status_invalid", message: "status is not allowed.", path: `${basePath}.status`, chunkId });
  }
  if (typeof value.summary !== "string") {
    issues.push({ code: "summary_invalid", message: "summary must be a string.", path: `${basePath}.summary`, chunkId });
  }
  const semanticLabels = validateSemanticLabels(value.semanticLabels, `${basePath}.semanticLabels`, issues, chunkId);
  const blockAffinity = validateBlockAffinity(value.blockAffinity, `${basePath}.blockAffinity`, issues, chunkId);
  let notes: string[] | undefined;
  if (value.notes !== undefined) {
    if (!Array.isArray(value.notes) || value.notes.some((note) => typeof note !== "string")) {
      issues.push({ code: "notes_invalid", message: "notes must be an array of strings when provided.", path: `${basePath}.notes`, chunkId });
    } else {
      notes = value.notes;
    }
  }
  if (!chunkId || typeof value.summary !== "string" || !semanticLabels || !blockAffinity) {
    return null;
  }
  return {
    chunkId,
    status: value.status as KnowledgeSemanticChunkResult["status"],
    summary: value.summary,
    semanticLabels,
    blockAffinity,
    ...(notes ? { notes } : {}),
  };
}

function validateSemanticLabels(
  value: unknown,
  basePath: string,
  issues: KnowledgeSemanticSubmitIssue[],
  chunkId: string,
): KnowledgeSemanticLabel[] | null {
  if (!Array.isArray(value)) {
    issues.push({ code: "semantic_labels_invalid", message: "semanticLabels must be an array.", path: basePath, chunkId });
    return null;
  }
  const labels: KnowledgeSemanticLabel[] = [];
  for (let index = 0; index < value.length; index += 1) {
    const raw = value[index];
    const labelPath = `${basePath}[${index}]`;
    if (!isRecord(raw)) {
      issues.push({ code: "semantic_label_invalid", message: "semantic label must be an object.", path: labelPath, chunkId });
      continue;
    }
    if (!LABEL_KINDS.includes(raw.kind as KnowledgeSemanticLabel["kind"])) {
      issues.push({ code: "label_kind_invalid", message: "semantic label kind is not allowed.", path: `${labelPath}.kind`, chunkId });
    }
    if (!CONFIDENCE_VALUES.includes(raw.confidence as KnowledgeSemanticLabel["confidence"])) {
      issues.push({ code: "label_confidence_invalid", message: "semantic label confidence is not allowed.", path: `${labelPath}.confidence`, chunkId });
    }
    if (typeof raw.text !== "string") {
      issues.push({ code: "label_text_invalid", message: "semantic label text must be a string.", path: `${labelPath}.text`, chunkId });
    }
    if (typeof raw.normalizedText !== "string") {
      issues.push({ code: "label_normalized_text_invalid", message: "semantic label normalizedText must be a string.", path: `${labelPath}.normalizedText`, chunkId });
    }
    if (!Array.isArray(raw.aliases) || raw.aliases.some((alias) => typeof alias !== "string")) {
      issues.push({ code: "label_aliases_invalid", message: "semantic label aliases must be an array of strings.", path: `${labelPath}.aliases`, chunkId });
    }
    if (
      LABEL_KINDS.includes(raw.kind as KnowledgeSemanticLabel["kind"]) &&
      CONFIDENCE_VALUES.includes(raw.confidence as KnowledgeSemanticLabel["confidence"]) &&
      typeof raw.text === "string" &&
      typeof raw.normalizedText === "string" &&
      Array.isArray(raw.aliases) &&
      raw.aliases.every((alias) => typeof alias === "string")
    ) {
      labels.push({
        kind: raw.kind as KnowledgeSemanticLabel["kind"],
        text: raw.text,
        normalizedText: raw.normalizedText,
        aliases: raw.aliases,
        confidence: raw.confidence as KnowledgeSemanticLabel["confidence"],
      });
    }
  }
  return labels;
}

function validateBlockAffinity(
  value: unknown,
  basePath: string,
  issues: KnowledgeSemanticSubmitIssue[],
  chunkId: string,
): KnowledgeBlockAffinity | null {
  if (!isRecord(value)) {
    issues.push({ code: "block_affinity_invalid", message: "blockAffinity must be an object.", path: basePath, chunkId });
    return null;
  }
  const fields: Array<keyof KnowledgeBlockAffinity> = ["phaseScope", "conceptGrounding", "frontendExperience", "finalSummary"];
  const result: Partial<KnowledgeBlockAffinity> = {};
  for (const field of fields) {
    const fieldValue = value[field];
    if (typeof fieldValue !== "number" || !Number.isFinite(fieldValue) || fieldValue < 0 || fieldValue > 1) {
      issues.push({
        code: "block_affinity_out_of_range",
        message: "blockAffinity values must be numbers from 0 to 1.",
        path: `${basePath}.${field}`,
        chunkId,
      });
    } else {
      result[field] = fieldValue;
    }
  }
  return fields.every((field) => result[field] !== undefined) ? result as KnowledgeBlockAffinity : null;
}

async function writeRepairResult(input: {
  request: KnowledgeSemanticBuildRequest;
  state: KnowledgeSemanticBuildState;
  resultFile: string;
  issues: KnowledgeSemanticSubmitIssue[];
}): Promise<KnowledgeSemanticSubmitResult> {
  const repairRequest = {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    buildId: input.request.buildId,
    packId: input.request.packId,
    resultFile: input.resultFile,
    issues: input.issues,
    repairScope: "current_pack_result_only" as const,
  } satisfies NonNullable<KnowledgeSemanticSubmitResult["repairRequest"]>;
  const repairRequestPath = knowledgeSemanticRepairFile(input.request.sourceId, input.request.buildId, input.request.packId);
  await writeJsonAtomic(repairRequestPath, repairRequest);
  return {
    status: "needs_repair",
    buildId: input.request.buildId,
    packId: input.request.packId,
    acceptedPackIds: input.state.acceptedPackIds,
    packCount: input.state.packCount,
    repairRequestPath,
    repairRequest,
    message: `Semantic pack ${input.request.packIndex}/${input.state.packCount} needs structural repair.`,
  };
}

async function publishSemanticBuild(input: {
  request: KnowledgeSemanticBuildRequest;
  state: KnowledgeSemanticBuildState;
}): Promise<NonNullable<KnowledgeSemanticSubmitResult["published"]>> {
  const runDir = path.dirname(input.request.buildRunPath);
  const buildRun = asBuildRun(await readJsonFile(input.request.buildRunPath));
  const semanticResults = await readAcceptedResults(input.state);
  const byChunk = new Map<string, KnowledgeSemanticChunkResult>();
  for (const result of semanticResults) {
    for (const chunkResult of result.chunkResults) {
      byChunk.set(chunkResult.chunkId, chunkResult);
    }
  }
  const chunks = buildRun.chunks.map((chunk) => mergeSemanticResult(chunk, byChunk.get(chunk.chunkId)));
  const lexicalIndex = buildLexicalIndex(buildRun.sourceId, buildRun.buildId, chunks, runDir);
  const semanticIndex = buildSemanticIndex(buildRun.sourceId, buildRun.buildId, chunks);
  const chunksPath = path.join(runDir, "chunks.json");
  const lexicalIndexPath = path.join(runDir, "lexical-index.json");
  const semanticIndexPath = path.join(runDir, "semantic-index.json");
  const now = new Date().toISOString();

  await writeJsonAtomic(chunksPath, {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    sourceId: buildRun.sourceId,
    buildId: buildRun.buildId,
    chunks,
  });
  await writeJsonAtomic(lexicalIndexPath, lexicalIndex);
  await writeJsonAtomic(semanticIndexPath, semanticIndex);

  const publishedRun: KnowledgeBuildRun = {
    ...buildRun,
    status: "published",
    chunks,
    refs: {
      ...buildRun.refs,
      semanticIndex: relativeToRunDir(runDir, semanticIndexPath),
    },
    updatedAt: now,
  };
  await writeJsonAtomic(input.request.buildRunPath, publishedRun);

  const existingSource = await findKnowledgeSource(buildRun.name);
  await upsertKnowledgeSource({
    sourceId: buildRun.sourceId,
    name: buildRun.name,
    status: existingSource?.status ?? "enabled",
    roots: buildRun.roots,
    index: {
      version: (existingSource?.index.version ?? 0) + 1,
      lastBuiltAt: now,
      currentBuildId: buildRun.buildId,
      documentCount: buildRun.documents.length,
      chunkCount: chunks.length,
    },
    createdAt: existingSource?.createdAt ?? now,
    updatedAt: now,
  });

  const statePath = knowledgeSemanticStateFile(input.request.sourceId, input.request.buildId);
  await writeJsonAtomic(statePath, {
    ...input.state,
    status: "published",
    updatedAt: now,
  });
  await removePendingKnowledge(buildRun.name);

  return {
    name: buildRun.name,
    sourceId: buildRun.sourceId,
    buildId: buildRun.buildId,
    documentCount: buildRun.documents.length,
    chunkCount: chunks.length,
  };
}

async function readAcceptedResults(state: KnowledgeSemanticBuildState): Promise<KnowledgeSemanticPackResult[]> {
  const results: KnowledgeSemanticPackResult[] = [];
  for (const pack of state.packs) {
    if (!state.acceptedPackIds.includes(pack.packId)) {
      continue;
    }
    results.push(asPackResult(await readJsonFile(pack.resultFile)));
  }
  return results;
}

function mergeSemanticResult(
  chunk: KnowledgeChunkRecord,
  result: KnowledgeSemanticChunkResult | undefined,
): KnowledgeChunkRecord {
  if (!result) {
    throw invalidArgument("Cannot publish knowledge build because a semantic chunk result is missing.", {
      chunkId: chunk.chunkId,
    });
  }
  return {
    ...chunk,
    retrievalFields: {
      ...chunk.retrievalFields,
      summary: result.summary,
      semanticLabelTexts: result.semanticLabels.map((label) => label.text),
      semanticAliases: result.semanticLabels.flatMap((label) => label.aliases),
    },
    semanticLabels: result.semanticLabels,
    blockAffinity: result.blockAffinity,
  };
}

function buildSemanticIndex(
  sourceId: string,
  buildId: string,
  chunks: KnowledgeChunkRecord[],
): KnowledgeSemanticIndex {
  const labels: KnowledgeSemanticIndex["labels"] = {};
  for (const chunk of chunks) {
    for (const label of chunk.semanticLabels) {
      addSemanticPosting(labels, normalizeSemanticText(label.normalizedText || label.text), {
        chunkId: chunk.chunkId,
        kind: label.kind,
        source: "label",
        confidence: label.confidence,
      });
      for (const alias of label.aliases) {
        addSemanticPosting(labels, normalizeSemanticText(alias), {
          chunkId: chunk.chunkId,
          kind: label.kind,
          source: "alias",
          confidence: label.confidence,
        });
      }
    }
  }
  return {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    sourceId,
    buildId,
    labels,
  };
}

function addSemanticPosting(
  labels: KnowledgeSemanticIndex["labels"],
  normalizedText: string,
  posting: KnowledgeSemanticIndex["labels"][string]["postings"][number],
): void {
  if (!normalizedText) {
    return;
  }
  labels[normalizedText] ??= { postings: [] };
  const exists = labels[normalizedText].postings.some((entry) =>
    entry.chunkId === posting.chunkId &&
    entry.kind === posting.kind &&
    entry.source === posting.source &&
    entry.confidence === posting.confidence
  );
  if (!exists) {
    labels[normalizedText].postings.push(posting);
  }
}

function normalizeSemanticText(value: string): string {
  return value.trim().toLowerCase();
}

function relativeDocumentPath(filePath: string, roots: KnowledgeRoot[]): string {
  const directoryRoots = roots.filter((root) => root.type === "directory").map((root) => root.path);
  const root = directoryRoots.find((entry) => filePath.startsWith(`${entry}${path.sep}`));
  return root ? path.relative(root, filePath) : path.basename(filePath);
}

function requirePath(value: string | undefined, optionName: string): string {
  const normalized = typeof value === "string" ? value.trim() : "";
  if (!normalized) {
    throw invalidArgument(`${optionName} is required.`, { option: optionName });
  }
  return path.resolve(normalized);
}

function relativeToRunDir(runDir: string, filePath: string): string {
  return path.relative(runDir, filePath);
}

function asSemanticBuildRequest(value: unknown): KnowledgeSemanticBuildRequest {
  if (!isRecord(value) || typeof value.buildId !== "string" || typeof value.packId !== "string") {
    throw invalidArgument("Invalid KnowledgeSemanticBuildRequest.", {});
  }
  return value as KnowledgeSemanticBuildRequest;
}

function asSemanticBuildState(value: unknown): KnowledgeSemanticBuildState {
  if (!isRecord(value) || typeof value.buildId !== "string" || !Array.isArray(value.packs)) {
    throw invalidArgument("Invalid KnowledgeSemanticBuildState.", {});
  }
  return value as KnowledgeSemanticBuildState;
}

function asBuildRun(value: unknown): KnowledgeBuildRun {
  if (!isRecord(value) || typeof value.buildId !== "string" || !Array.isArray(value.chunks)) {
    throw invalidArgument("Invalid KnowledgeBuildRun.", {});
  }
  return value as KnowledgeBuildRun;
}

function asPackResult(value: unknown): KnowledgeSemanticPackResult {
  if (!isRecord(value) || typeof value.buildId !== "string" || !Array.isArray(value.chunkResults)) {
    throw invalidArgument("Invalid KnowledgeSemanticPackResult.", {});
  }
  return value as KnowledgeSemanticPackResult;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
