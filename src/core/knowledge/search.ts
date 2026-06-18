import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument } from "../errors";
import { readJsonFile } from "../state/fs";
import { tokenizeKnowledgeText } from "./lexical";
import { knowledgeBuildRunFile } from "./paths";
import {
  type KnowledgeBuildRun,
  type BrainstormBlockKnowledgeContext,
  type KnowledgeChunkRecord,
  type KnowledgeInspectResult,
  type KnowledgeLexicalIndex,
  type KnowledgeMatchCandidate,
  type KnowledgeMatchQuery,
  type KnowledgeSearchQuery,
  type KnowledgeSearchResult,
  type KnowledgeSemanticIndex,
  type KnowledgeSemanticLabel,
  type KnowledgeSource,
} from "./types";
import {
  findKnowledgeSource,
  readKnowledgeRegistry,
  validateKnowledgeName,
} from "./state";

const DEFAULT_CHUNK_LIMIT = 8;
const MAX_CHUNK_LIMIT = 20;
const DEFAULT_MATCH_SOURCE_LIMIT = 2;
const MAX_MATCH_SOURCE_LIMIT = 2;
const DEFAULT_MATCH_CHUNK_LIMIT_PER_SOURCE = 3;
const MAX_MATCH_CHUNK_LIMIT_PER_SOURCE = 3;
const MAX_MATCH_CHUNKS_PER_BLOCK = 5;
const K1 = 1.2;
const B = 0.75;

type SearchInput = {
  query?: string;
  queryFile?: string;
  source?: string[];
  block?: string;
  semanticFocus?: string[];
  limit?: string;
};

type InspectInput = {
  source?: string;
  buildId?: string;
  chunkId?: string;
};

type BrainstormContextInput = {
  queryFile?: string;
  query?: string;
  block?: string;
  semanticFocus?: string[];
  sourceLimit?: string;
  chunkLimitPerSource?: string;
};

type LoadedSourceIndex = {
  source: KnowledgeSource;
  buildRun: KnowledgeBuildRun;
  chunks: KnowledgeChunkRecord[];
  lexicalIndex: KnowledgeLexicalIndex;
  semanticIndex: KnowledgeSemanticIndex;
  runDir: string;
};

type ScoredChunk = {
  source: LoadedSourceIndex;
  chunk: KnowledgeChunkRecord;
  lexicalScore: number;
  semanticScore: number;
  blockScore: number;
  finalScore: number;
  matchedLabels: KnowledgeSearchResult["results"][number]["matchedLabels"];
};

export async function searchKnowledge(input: SearchInput): Promise<KnowledgeSearchResult> {
  const query = await searchQueryFromInput(input);
  const sources = await loadSearchSources(input.source);
  const scored = scoreLoadedSources(sources, query);
  const selected = applyDocumentDiversity(scored
    .filter((entry) => entry.finalScore > 0)
    .sort((a, b) => b.finalScore - a.finalScore), query.chunkLimit);
  return {
    query,
    results: selected.map(toChunkCard),
  };
}

export async function buildBrainstormKnowledgeContext(input: BrainstormContextInput): Promise<BrainstormBlockKnowledgeContext> {
  const matchQuery = await matchQueryFromInput(input);
  const sources = await loadSearchSources(undefined);
  const scored = scoreLoadedSources(sources, {
    naturalLanguageQuery: matchQuery.naturalLanguageQuery,
    brainstormBlock: matchQuery.brainstormBlock,
    semanticFocus: matchQuery.semanticFocus,
    chunkLimit: MAX_MATCH_CHUNKS_PER_BLOCK,
  })
    .filter((entry) => entry.finalScore > 0)
    .sort((a, b) => b.finalScore - a.finalScore);
  const matchedSources = aggregateMatchCandidates(scored, matchQuery);
  return {
    status: matchedSources.length > 0 ? "available" : "empty",
    block: matchQuery.brainstormBlock,
    matchQuery,
    matchedSources,
    readPlan: {
      mode: "inspect_all_listed_chunks",
      chunks: matchedSources.flatMap((source) => source.topChunks.map((chunk) => ({
        sourceName: source.sourceName,
        chunkId: chunk.chunkId,
        inspectCommand: chunk.inspectCommand,
      }))),
    },
  };
}

export async function inspectKnowledge(input: InspectInput): Promise<KnowledgeInspectResult> {
  const chunkId = requireString(input.chunkId, "--chunk");
  const source = await resolveInspectSource(input);
  const buildId = input.buildId ?? source.index.currentBuildId;
  if (!buildId) {
    throw invalidArgument(`Knowledge source "${source.name}" has no published build.`, {
      source: source.name,
    });
  }
  const buildRunPath = knowledgeBuildRunFile(source.sourceId, buildId);
  const buildRun = asBuildRun(await readJsonFile(buildRunPath));
  const chunk = buildRun.chunks.find((entry) => entry.chunkId === chunkId);
  if (!chunk) {
    throw invalidArgument("Knowledge chunk was not found in the selected build.", {
      source: source.name,
      buildId,
      chunkId,
    });
  }
  const runDir = path.dirname(buildRunPath);
  const text = await fs.readFile(path.join(runDir, chunk.textRef), "utf8");
  const document = buildRun.documents.find((entry) => entry.documentId === chunk.documentId);
  return {
    sourceName: source.name,
    sourceId: source.sourceId,
    buildId,
    chunkId,
    documentTitle: document?.title ?? chunk.title,
    headingPath: chunk.headingPath,
    tokenEstimate: chunk.tokenEstimate,
    text,
  };
}

function scoreLoadedSources(sources: LoadedSourceIndex[], query: KnowledgeSearchQuery): ScoredChunk[] {
  const scored: ScoredChunk[] = [];
  for (const source of sources) {
    scored.push(...scoreSourceChunks(source, query));
  }
  const maxLexicalScore = Math.max(0, ...scored.map((entry) => entry.lexicalScore));
  return scored.map((entry) => {
    const lexicalScore = maxLexicalScore > 0 ? entry.lexicalScore / maxLexicalScore : 0;
    return {
      ...entry,
      lexicalScore,
      finalScore: lexicalScore * 0.55 + entry.semanticScore * 0.30 + entry.blockScore * 0.15,
    };
  });
}

function aggregateMatchCandidates(scored: ScoredChunk[], query: KnowledgeMatchQuery): KnowledgeMatchCandidate[] {
  const bySource = new Map<string, ScoredChunk[]>();
  for (const entry of scored) {
    const sourceId = entry.source.source.sourceId;
    bySource.set(sourceId, [...(bySource.get(sourceId) ?? []), entry]);
  }
  const candidates = [...bySource.values()]
    .map((entries) => buildMatchCandidate(entries, query))
    .sort((a, b) => b.matchScore - a.matchScore);
  const selected: KnowledgeMatchCandidate[] = [];
  let remainingChunks = MAX_MATCH_CHUNKS_PER_BLOCK;
  for (const candidate of candidates) {
    if (selected.length >= query.sourceLimit || remainingChunks <= 0) {
      break;
    }
    const topChunks = candidate.topChunks.slice(0, Math.min(query.chunkLimitPerSource, remainingChunks));
    if (topChunks.length === 0) {
      continue;
    }
    selected.push(recalculateCandidateWithChunks(candidate, topChunks, query));
    remainingChunks -= topChunks.length;
  }
  return selected;
}

function buildMatchCandidate(entries: ScoredChunk[], query: KnowledgeMatchQuery): KnowledgeMatchCandidate {
  const source = entries[0].source.source;
  const sorted = [...entries].sort((a, b) => b.finalScore - a.finalScore);
  const topChunks = sorted.slice(0, query.chunkLimitPerSource).map(toChunkCard);
  return recalculateCandidateWithChunks({
    sourceId: source.sourceId,
    sourceName: source.name,
    lastBuiltAt: source.index.lastBuiltAt ?? "",
    documentCount: source.index.documentCount,
    chunkCount: source.index.chunkCount,
    matchScore: 0,
    scoreBreakdown: {
      bestChunkScore: 0,
      averageTop3ChunkScore: 0,
      matchedFocusCoverage: 0,
    },
    matchedFocus: [],
    topChunks,
  }, topChunks, query);
}

function recalculateCandidateWithChunks(
  candidate: KnowledgeMatchCandidate,
  topChunks: KnowledgeMatchCandidate["topChunks"],
  query: KnowledgeMatchQuery,
): KnowledgeMatchCandidate {
  const scores = topChunks.map((chunk) => chunk.score);
  const top3 = scores.slice(0, 3);
  const bestChunkScore = scores[0] ?? 0;
  const averageTop3ChunkScore = top3.length > 0
    ? top3.reduce((sum, score) => sum + score, 0) / top3.length
    : 0;
  const matchedFocus = buildMatchedFocus(topChunks, query);
  const matchedFocusCoverage = query.semanticFocus.length > 0
    ? matchedFocus.length / query.semanticFocus.length
    : 0;
  const matchScore = bestChunkScore * 0.55 + averageTop3ChunkScore * 0.25 + matchedFocusCoverage * 0.20;
  return {
    ...candidate,
    topChunks,
    scoreBreakdown: {
      bestChunkScore: roundScore(bestChunkScore),
      averageTop3ChunkScore: roundScore(averageTop3ChunkScore),
      matchedFocusCoverage: roundScore(matchedFocusCoverage),
    },
    matchedFocus,
    matchScore: roundScore(matchScore),
  };
}

function buildMatchedFocus(
  topChunks: KnowledgeMatchCandidate["topChunks"],
  query: KnowledgeMatchQuery,
): KnowledgeMatchCandidate["matchedFocus"] {
  return query.semanticFocus
    .map((focus) => ({
      kind: focus.kind,
      text: focus.text,
      matchedChunkIds: topChunks
        .filter((chunk) => chunk.matchedLabels.some((label) => label.kind === focus.kind && label.text === focus.text))
        .map((chunk) => chunk.chunkId),
    }))
    .filter((focus) => focus.matchedChunkIds.length > 0);
}

function toChunkCard(entry: ScoredChunk): KnowledgeSearchResult["results"][number] {
  const document = entry.source.buildRun.documents.find((candidate) => candidate.documentId === entry.chunk.documentId);
  return {
    chunkId: entry.chunk.chunkId,
    sourceName: entry.source.source.name,
    documentTitle: document?.title ?? entry.chunk.title,
    headingPath: entry.chunk.headingPath,
    summary: entry.chunk.retrievalFields.summary,
    matchedLabels: entry.matchedLabels,
    score: roundScore(entry.finalScore),
    tokenEstimate: entry.chunk.tokenEstimate,
    inspectCommand: {
      name: "knowledge inspect",
      argv: ["knowledge", "inspect", "--source", entry.source.source.name, "--chunk", entry.chunk.chunkId],
    },
  };
}

async function searchQueryFromInput(input: SearchInput): Promise<KnowledgeSearchQuery> {
  if (input.queryFile) {
    const fromFile = asSearchQuery(await readJsonFile(path.resolve(input.queryFile)));
    return normalizeSearchQuery(fromFile);
  }
  return normalizeSearchQuery({
    naturalLanguageQuery: input.query ?? "",
    brainstormBlock: normalizeBlock(input.block),
    semanticFocus: (input.semanticFocus ?? []).map(parseSemanticFocus),
    chunkLimit: parseLimit(input.limit),
  });
}

async function matchQueryFromInput(input: BrainstormContextInput): Promise<KnowledgeMatchQuery> {
  if (input.queryFile) {
    const fromFile = asMatchQuery(await readJsonFile(path.resolve(input.queryFile)));
    return normalizeMatchQuery(fromFile);
  }
  return normalizeMatchQuery({
    naturalLanguageQuery: input.query ?? "",
    brainstormBlock: normalizeBlock(input.block),
    semanticFocus: (input.semanticFocus ?? []).map(parseSemanticFocus),
    sourceLimit: parseLimit(input.sourceLimit),
    chunkLimitPerSource: parseLimit(input.chunkLimitPerSource),
  });
}

function normalizeSearchQuery(query: KnowledgeSearchQuery): KnowledgeSearchQuery {
  const naturalLanguageQuery = String(query.naturalLanguageQuery ?? "").trim();
  const semanticFocus = Array.isArray(query.semanticFocus) ? query.semanticFocus : [];
  if (!naturalLanguageQuery && semanticFocus.length === 0) {
    throw invalidArgument("knowledge search requires --query or --semantic-focus.", {});
  }
  return {
    naturalLanguageQuery,
    brainstormBlock: normalizeBlock(query.brainstormBlock),
    semanticFocus: semanticFocus.map((focus) => ({
      kind: focus.kind,
      text: String(focus.text ?? "").trim(),
    })).filter((focus) => focus.text.length > 0).map((focus) => {
      if (!isLabelKind(focus.kind)) {
        throw invalidArgument("semantic focus kind is not allowed.", {
          kind: focus.kind,
        });
      }
      return focus;
    }),
    chunkLimit: clampLimit(query.chunkLimit),
  };
}

function normalizeMatchQuery(query: KnowledgeMatchQuery): KnowledgeMatchQuery {
  const normalized = normalizeSearchQuery({
    naturalLanguageQuery: query.naturalLanguageQuery,
    brainstormBlock: normalizeBlock(query.brainstormBlock),
    semanticFocus: query.semanticFocus,
    chunkLimit: MAX_MATCH_CHUNKS_PER_BLOCK,
  });
  return {
    naturalLanguageQuery: normalized.naturalLanguageQuery,
    brainstormBlock: normalized.brainstormBlock,
    semanticFocus: normalized.semanticFocus,
    sourceLimit: clampMatchLimit(query.sourceLimit, DEFAULT_MATCH_SOURCE_LIMIT, MAX_MATCH_SOURCE_LIMIT),
    chunkLimitPerSource: clampMatchLimit(query.chunkLimitPerSource, DEFAULT_MATCH_CHUNK_LIMIT_PER_SOURCE, MAX_MATCH_CHUNK_LIMIT_PER_SOURCE),
  };
}

async function loadSearchSources(sourceNames: string[] | undefined): Promise<LoadedSourceIndex[]> {
  const registry = await readKnowledgeRegistry();
  const selectedSources = sourceNames && sourceNames.length > 0
    ? await Promise.all(sourceNames.map(async (name) => {
      const source = await findKnowledgeSource(validateKnowledgeName(name));
      if (!source) {
        throw invalidArgument(`Knowledge source "${name}" does not exist.`, { name });
      }
      return source;
    }))
    : registry.sources;
  const enabled = selectedSources.filter((source) => source.status === "enabled" && source.index.currentBuildId);
  const loaded: LoadedSourceIndex[] = [];
  for (const source of enabled) {
    loaded.push(await loadSourceIndex(source));
  }
  return loaded;
}

async function loadSourceIndex(source: KnowledgeSource): Promise<LoadedSourceIndex> {
  const buildId = source.index.currentBuildId;
  if (!buildId) {
    throw invalidArgument(`Knowledge source "${source.name}" has no published build.`, { source: source.name });
  }
  const buildRunPath = knowledgeBuildRunFile(source.sourceId, buildId);
  const runDir = path.dirname(buildRunPath);
  const buildRun = asBuildRun(await readJsonFile(buildRunPath));
  const chunksPayload = await readJsonFile(path.join(runDir, "chunks.json"));
  const lexicalIndex = asLexicalIndex(await readJsonFile(path.join(runDir, "lexical-index.json")));
  const semanticIndex = asSemanticIndex(await readJsonFile(path.join(runDir, "semantic-index.json")));
  const chunks = asChunksPayload(chunksPayload);
  return {
    source,
    buildRun,
    chunks,
    lexicalIndex,
    semanticIndex,
    runDir,
  };
}

function scoreSourceChunks(source: LoadedSourceIndex, query: KnowledgeSearchQuery): ScoredChunk[] {
  const lexicalScores = scoreLexical(source.lexicalIndex, query.naturalLanguageQuery);
  const semanticMatches = scoreSemantic(source, query);
  const chunkById = new Map(source.chunks.map((chunk) => [chunk.chunkId, chunk]));
  const candidateIds = new Set<string>([
    ...lexicalScores.keys(),
    ...semanticMatches.keys(),
  ]);
  const results: ScoredChunk[] = [];
  for (const chunkId of candidateIds) {
    const chunk = chunkById.get(chunkId);
    if (!chunk) {
      continue;
    }
    const semantic = semanticMatches.get(chunkId);
    results.push({
      source,
      chunk,
      lexicalScore: lexicalScores.get(chunkId) ?? 0,
      semanticScore: semantic?.score ?? 0,
      blockScore: blockAffinityScore(chunk, query.brainstormBlock),
      finalScore: 0,
      matchedLabels: semantic?.matchedLabels ?? [],
    });
  }
  return results;
}

function scoreLexical(index: KnowledgeLexicalIndex, naturalLanguageQuery: string): Map<string, number> {
  const scores = new Map<string, number>();
  const queryTerms = [...new Set(tokenizeKnowledgeText(naturalLanguageQuery))];
  const averageDocumentLength = index.averageDocumentLength || 1;
  for (const term of queryTerms) {
    const entry = index.terms[term];
    if (!entry) {
      continue;
    }
    const idf = Math.log(1 + (index.chunkCount - entry.df + 0.5) / (entry.df + 0.5));
    for (const posting of entry.postings) {
      const weightedTf = weightedTermFrequency(posting.fields, index.fieldWeights);
      const docLength = index.documentLengths[posting.chunkId] ?? averageDocumentLength;
      const denominator = weightedTf + K1 * (1 - B + B * docLength / averageDocumentLength);
      const score = denominator > 0 ? idf * (weightedTf * (K1 + 1)) / denominator : 0;
      scores.set(posting.chunkId, (scores.get(posting.chunkId) ?? 0) + score);
    }
  }
  return scores;
}

function scoreSemantic(
  source: LoadedSourceIndex,
  query: KnowledgeSearchQuery,
): Map<string, {
  score: number;
  matchedLabels: KnowledgeSearchResult["results"][number]["matchedLabels"];
}> {
  const result = new Map<string, {
    score: number;
    matchedLabels: KnowledgeSearchResult["results"][number]["matchedLabels"];
  }>();
  for (const focus of query.semanticFocus) {
    const key = normalizeSemanticText(focus.text);
    const entry = source.semanticIndex.labels[key];
    if (!entry) {
      continue;
    }
    for (const posting of entry.postings) {
      if (posting.kind !== focus.kind) {
        continue;
      }
      const base = posting.source === "label" ? 1.0 : 0.8;
      const confidence = confidenceMultiplier(posting.confidence);
      const previous = result.get(posting.chunkId) ?? { score: 0, matchedLabels: [] };
      previous.score = Math.min(1, previous.score + base * confidence);
      previous.matchedLabels.push({
        kind: posting.kind,
        text: focus.text,
        matchSource: posting.source === "label" ? "text" : "alias",
        confidence: posting.confidence,
      });
      result.set(posting.chunkId, previous);
    }
  }
  return result;
}

function applyDocumentDiversity(scored: ScoredChunk[], limit: number): ScoredChunk[] {
  const selected: ScoredChunk[] = [];
  const perDocument = new Map<string, number>();
  for (const entry of scored) {
    const documentCount = perDocument.get(entry.chunk.documentId) ?? 0;
    if (documentCount >= 3) {
      continue;
    }
    selected.push(entry);
    perDocument.set(entry.chunk.documentId, documentCount + 1);
    if (selected.length >= limit) {
      break;
    }
  }
  return selected;
}

async function resolveInspectSource(input: InspectInput): Promise<KnowledgeSource> {
  if (input.source) {
    const source = await findKnowledgeSource(validateKnowledgeName(input.source));
    if (!source) {
      throw invalidArgument(`Knowledge source "${input.source}" does not exist.`, { source: input.source });
    }
    return source;
  }
  if (input.buildId) {
    const registry = await readKnowledgeRegistry();
    const source = registry.sources.find((entry) => entry.index.currentBuildId === input.buildId);
    if (source) {
      return source;
    }
  }
  throw invalidArgument("knowledge inspect requires --source when --build-id cannot identify a source.", {
    source: input.source,
    buildId: input.buildId,
  });
}

function weightedTermFrequency(
  fields: KnowledgeLexicalIndex["terms"][string]["postings"][number]["fields"],
  weights: KnowledgeLexicalIndex["fieldWeights"],
): number {
  return (fields.title ?? 0) * weights.title +
    (fields.headingPath ?? 0) * weights.headingPath +
    (fields.summary ?? 0) * weights.summary +
    (fields.semanticLabelTexts ?? 0) * weights.semanticLabelTexts +
    (fields.semanticAliases ?? 0) * weights.semanticAliases +
    (fields.body ?? 0) * weights.body;
}

function blockAffinityScore(chunk: KnowledgeChunkRecord, block: KnowledgeSearchQuery["brainstormBlock"]): number {
  if (block === "phase_scope") return chunk.blockAffinity.phaseScope;
  if (block === "concept_grounding") return chunk.blockAffinity.conceptGrounding;
  if (block === "frontend_experience") return chunk.blockAffinity.frontendExperience;
  return 0;
}

function confidenceMultiplier(confidence: KnowledgeSemanticLabel["confidence"]): number {
  if (confidence === "high") return 1.0;
  if (confidence === "medium") return 0.7;
  return 0.3;
}

function parseSemanticFocus(value: string): KnowledgeSearchQuery["semanticFocus"][number] {
  const separator = value.indexOf(":");
  if (separator <= 0) {
    throw invalidArgument("semantic focus must use kind:text format.", {
      value,
    });
  }
  const kind = value.slice(0, separator) as KnowledgeSemanticLabel["kind"];
  if (!isLabelKind(kind)) {
    throw invalidArgument("semantic focus kind is not allowed.", {
      value,
      kind,
    });
  }
  return {
    kind,
    text: value.slice(separator + 1),
  };
}

function normalizeBlock(value: unknown): KnowledgeSearchQuery["brainstormBlock"] {
  if (value === undefined || value === null || value === "") {
    return "phase_scope";
  }
  if (value === "phase_scope" || value === "concept_grounding" || value === "frontend_experience") {
    return value;
  }
  throw invalidArgument("knowledge search only supports phase_scope, concept_grounding, and frontend_experience blocks.", {
    block: value,
  });
}

function isLabelKind(value: unknown): value is KnowledgeSemanticLabel["kind"] {
  return value === "object" ||
    value === "operation" ||
    value === "state" ||
    value === "rule" ||
    value === "field" ||
    value === "page" ||
    value === "flow" ||
    value === "other";
}

function parseLimit(value: string | undefined): number {
  if (!value) {
    return DEFAULT_CHUNK_LIMIT;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : DEFAULT_CHUNK_LIMIT;
}

function clampLimit(value: number | undefined): number {
  if (!Number.isFinite(value)) {
    return DEFAULT_CHUNK_LIMIT;
  }
  return Math.max(1, Math.min(MAX_CHUNK_LIMIT, Math.floor(value as number)));
}

function clampMatchLimit(value: number | undefined, fallback: number, max: number): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(1, Math.min(max, Math.floor(value as number)));
}

function requireString(value: string | undefined, optionName: string): string {
  const normalized = typeof value === "string" ? value.trim() : "";
  if (!normalized) {
    throw invalidArgument(`${optionName} is required.`, { option: optionName });
  }
  return normalized;
}

function roundScore(value: number): number {
  return Math.round(value * 10000) / 10000;
}

function normalizeSemanticText(value: string): string {
  return value.trim().toLowerCase();
}

function asSearchQuery(value: unknown): KnowledgeSearchQuery {
  if (!isRecord(value)) {
    throw invalidArgument("Knowledge search query file must contain a JSON object.", {});
  }
  return value as KnowledgeSearchQuery;
}

function asMatchQuery(value: unknown): KnowledgeMatchQuery {
  if (!isRecord(value)) {
    throw invalidArgument("Knowledge match query file must contain a JSON object.", {});
  }
  return value as KnowledgeMatchQuery;
}

function asBuildRun(value: unknown): KnowledgeBuildRun {
  if (!isRecord(value) || !Array.isArray(value.chunks)) {
    throw invalidArgument("Invalid KnowledgeBuildRun.", {});
  }
  return value as KnowledgeBuildRun;
}

function asChunksPayload(value: unknown): KnowledgeChunkRecord[] {
  if (!isRecord(value) || !Array.isArray(value.chunks)) {
    throw invalidArgument("Invalid chunks payload.", {});
  }
  return value.chunks as KnowledgeChunkRecord[];
}

function asLexicalIndex(value: unknown): KnowledgeLexicalIndex {
  if (!isRecord(value) || !isRecord(value.terms)) {
    throw invalidArgument("Invalid KnowledgeLexicalIndex.", {});
  }
  return value as KnowledgeLexicalIndex;
}

function asSemanticIndex(value: unknown): KnowledgeSemanticIndex {
  if (!isRecord(value) || !isRecord(value.labels)) {
    throw invalidArgument("Invalid KnowledgeSemanticIndex.", {});
  }
  return value as KnowledgeSemanticIndex;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
