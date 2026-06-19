import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument } from "../errors";
import { pathExists, readJsonFile } from "../state/fs";
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
  readPendingKnowledge,
  validateKnowledgeName,
} from "./state";

const DEFAULT_CHUNK_LIMIT = 8;
const MAX_CHUNK_LIMIT = 20;
const DEFAULT_MATCH_SOURCE_LIMIT = 2;
const MAX_MATCH_SOURCE_LIMIT = 2;
const DEFAULT_MATCH_CHUNK_LIMIT_PER_SOURCE = 5;
const MAX_MATCH_CHUNK_LIMIT_PER_SOURCE = 5;
const MAX_MATCH_CHUNKS_PER_BLOCK = 5;
const K1 = 1.2;
const B = 0.75;

const BLOCK_RETRIEVAL_INTENT: Record<KnowledgeSearchQuery["brainstormBlock"], string> = {
  phase_scope: [
    "phase scope boundary include exclude defer dependency ordering next phase",
    "阶段范围 边界 纳入 排除 延后 递延 依赖 顺序 下一阶段",
  ].join(" "),
  concept_grounding: [
    "business object operation field state rule invariant precondition validation blocking outcome feedback",
    "业务对象 操作 字段 状态 规则 不变量 前置条件 校验 阻断 成功结果 反馈",
  ].join(" "),
  frontend_experience: [
    "page operation path workspace entry target discovery query filter pagination selection list detail action entry form input success feedback failure feedback business blocking loading empty state refresh readback",
    "页面办理路径 页面操作路径 工作台 入口 目标定位 查询 筛选 分页 选择 列表 详情 操作入口 表单 输入 成功反馈 失败提示 业务阻断 加载中 空状态 刷新 回读",
  ].join(" "),
};

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
  semanticCompleteness: number;
  semanticTier: number;
  blockScore: number;
  finalScore: number;
  matchedLabels: KnowledgeSearchResult["results"][number]["matchedLabels"];
};

type SemanticFocusLookup = {
  focus: KnowledgeSearchQuery["semanticFocus"][number];
  lookupText: string;
  matchMode: "exact" | "contains";
  requiredText?: string;
};

type SemanticChunkMatch = {
  focusScores: Map<string, number>;
  matchedLabels: KnowledgeSearchResult["results"][number]["matchedLabels"];
};

export async function searchKnowledge(input: SearchInput): Promise<KnowledgeSearchResult> {
  const query = await searchQueryFromInput(input);
  const sources = await loadSearchSources(input.source);
  const scored = scoreLoadedSources(sources, query);
  const selected = applyDocumentDiversity(scored
    .filter((entry) => entry.finalScore > 0)
    .sort(compareScoredChunks), query.chunkLimit);
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
    .sort(compareScoredChunks);
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
    const semanticCoverage = semanticCompleteness(entry.matchedLabels, query);
    return {
      ...entry,
      lexicalScore,
      semanticCompleteness: semanticCoverage.score,
      semanticTier: semanticCoverage.tier,
      finalScore: lexicalScore * 0.40 + entry.semanticScore * 0.25 + semanticCoverage.score * 0.20 + entry.blockScore * 0.15,
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
  const sorted = [...entries].sort(compareScoredChunks);
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
    naturalLanguageQuery: withBlockRetrievalIntent(normalized.naturalLanguageQuery, normalized.brainstormBlock),
    brainstormBlock: normalized.brainstormBlock,
    semanticFocus: normalized.semanticFocus,
    sourceLimit: clampMatchLimit(query.sourceLimit, DEFAULT_MATCH_SOURCE_LIMIT, MAX_MATCH_SOURCE_LIMIT),
    chunkLimitPerSource: clampMatchLimit(query.chunkLimitPerSource, DEFAULT_MATCH_CHUNK_LIMIT_PER_SOURCE, MAX_MATCH_CHUNK_LIMIT_PER_SOURCE),
  };
}

function withBlockRetrievalIntent(
  naturalLanguageQuery: string,
  block: KnowledgeSearchQuery["brainstormBlock"],
): string {
  const intent = BLOCK_RETRIEVAL_INTENT[block];
  if (!intent) {
    return naturalLanguageQuery;
  }
  if (naturalLanguageQuery.includes(intent)) {
    return naturalLanguageQuery;
  }
  return [naturalLanguageQuery, intent].filter((part) => part.trim().length > 0).join("\n");
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
      semanticCompleteness: 0,
      semanticTier: 0,
      blockScore: blockAffinityScore(chunk, query.brainstormBlock),
      finalScore: 0,
      matchedLabels: semantic?.matchedLabels ?? [],
    });
  }
  return results;
}

function compareScoredChunks(a: ScoredChunk, b: ScoredChunk): number {
  if (a.finalScore !== b.finalScore) {
    return b.finalScore - a.finalScore;
  }
  if (a.semanticTier !== b.semanticTier) {
    return b.semanticTier - a.semanticTier;
  }
  if (a.semanticCompleteness !== b.semanticCompleteness) {
    return b.semanticCompleteness - a.semanticCompleteness;
  }
  return 0;
}

function semanticCompleteness(
  matchedLabels: KnowledgeSearchResult["results"][number]["matchedLabels"],
  query: KnowledgeSearchQuery,
): { score: number; tier: number } {
  if (query.semanticFocus.length === 0 || matchedLabels.length === 0) {
    return { score: 0, tier: 0 };
  }

  const matchedKeys = new Set(matchedLabels.map((label) => semanticFocusKey(label.kind, label.text)));
  const focusItems = dedupeSemanticFocus(query.semanticFocus);
  const matchedFocus = focusItems.filter((focus) => matchedKeys.has(semanticFocusKey(focus.kind, focus.text)));
  if (matchedFocus.length === 0) {
    return { score: 0, tier: 0 };
  }

  const coverageRatio = matchedFocus.length / focusItems.length;
  const focusGroups = semanticFocusGroups(query.brainstormBlock);
  const activeGroups = focusGroups
    .map((group) => focusItems.some((focus) => group.includes(focus.kind)) ? group : [])
    .filter((group) => group.length > 0);
  if (activeGroups.length < 2) {
    return {
      score: coverageRatio,
      tier: coverageRatio >= 1 ? 2 : 1,
    };
  }

  const matchedGroupCount = activeGroups.filter((group) => (
    matchedFocus.some((focus) => group.includes(focus.kind))
  )).length;
  const groupCoverage = matchedGroupCount / activeGroups.length;
  return {
    score: coverageRatio * 0.45 + groupCoverage * 0.55,
    tier: matchedGroupCount === activeGroups.length ? 2 : 1,
  };
}

function dedupeSemanticFocus(
  semanticFocus: KnowledgeSearchQuery["semanticFocus"],
): KnowledgeSearchQuery["semanticFocus"] {
  const seen = new Set<string>();
  const result: KnowledgeSearchQuery["semanticFocus"] = [];
  for (const focus of semanticFocus) {
    const key = semanticFocusKey(focus.kind, focus.text);
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(focus);
  }
  return result;
}

function semanticFocusKey(kind: KnowledgeSemanticLabel["kind"], text: string): string {
  return `${kind}:${normalizeSemanticText(text)}`;
}

function semanticFocusGroups(
  block: KnowledgeSearchQuery["brainstormBlock"],
): KnowledgeSemanticLabel["kind"][][] {
  if (block === "frontend_experience") {
    return [
      ["page", "flow"],
      ["operation", "field", "state"],
    ];
  }
  return [
    ["object"],
    ["operation", "rule", "state", "field", "flow"],
  ];
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
  const matches = new Map<string, SemanticChunkMatch>();
  for (const lookup of semanticFocusLookups(query.semanticFocus, source.semanticIndex)) {
    for (const [, posting] of semanticLookupPostings(source.semanticIndex, lookup)) {
      if (!semanticPostingMatchesFocus(posting.kind, lookup.focus.kind)) {
        continue;
      }
      const base = posting.source === "label" ? 1.0 : 0.8;
      const confidence = confidenceMultiplier(posting.confidence);
      const previous = matches.get(posting.chunkId) ?? { focusScores: new Map(), matchedLabels: [] };
      const focusKey = semanticFocusKey(lookup.focus.kind, lookup.focus.text);
      previous.focusScores.set(focusKey, Math.max(previous.focusScores.get(focusKey) ?? 0, base * confidence));
      pushMatchedLabel(previous.matchedLabels, {
        kind: lookup.focus.kind,
        text: lookup.focus.text,
        matchSource: posting.source === "label" ? "text" : "alias",
        confidence: posting.confidence,
      });
      matches.set(posting.chunkId, previous);
    }
  }
  const result = new Map<string, {
    score: number;
    matchedLabels: KnowledgeSearchResult["results"][number]["matchedLabels"];
  }>();
  for (const [chunkId, match] of matches) {
    result.set(chunkId, {
      score: Math.min(1, [...match.focusScores.values()].reduce((sum, score) => sum + score, 0)),
      matchedLabels: match.matchedLabels,
    });
  }
  return result;
}

function semanticPostingMatchesFocus(
  postingKind: KnowledgeSemanticLabel["kind"],
  focusKind: KnowledgeSemanticLabel["kind"],
): boolean {
  if (postingKind === focusKind) {
    return true;
  }
  if (
    (postingKind === "flow" && focusKind === "operation") ||
    (postingKind === "operation" && focusKind === "flow")
  ) {
    return true;
  }
  if (
    (postingKind === "state" && focusKind === "rule") ||
    (postingKind === "rule" && focusKind === "state")
  ) {
    return true;
  }
  return false;
}

function semanticLookupPostings(
  semanticIndex: KnowledgeSemanticIndex,
  lookup: SemanticFocusLookup,
): Array<[string, KnowledgeSemanticIndex["labels"][string]["postings"][number]]> {
  const normalized = normalizeSemanticText(lookup.lookupText);
  const required = lookup.requiredText ? normalizeSemanticText(lookup.requiredText) : "";
  if (!normalized) {
    return [];
  }
  if (lookup.matchMode === "exact") {
    const entry = semanticIndex.labels[normalized];
    return entry ? entry.postings.map((posting) => [normalized, posting]) : [];
  }
  return Object.entries(semanticIndex.labels)
    .filter(([labelText]) => labelText.includes(normalized) && (!required || labelText.includes(required)))
    .flatMap(([labelText, entry]) => entry.postings.map((posting): [string, typeof posting] => [labelText, posting]));
}

function semanticFocusLookups(
  semanticFocus: KnowledgeSearchQuery["semanticFocus"],
  semanticIndex: KnowledgeSemanticIndex,
): SemanticFocusLookup[] {
  const subjects = dedupeSemanticFocus(semanticFocus.filter((focus) => isSubjectFocusKind(focus.kind)));
  const sourceSubjects = semanticIndexSubjectTexts(semanticIndex);
  const result: SemanticFocusLookup[] = [];
  const seen = new Set<string>();
  for (const focus of semanticFocus) {
    pushSemanticFocusLookup(result, seen, focus, focus.text);
    if (!isAttributiveFocusKind(focus.kind)) {
      continue;
    }
    for (const subject of subjects) {
      if (!subject.text || semanticFocusKey(subject.kind, subject.text) === semanticFocusKey(focus.kind, focus.text)) {
        continue;
      }
      for (const combined of combineSemanticAnchorTexts(subject.text, focus.text)) {
        pushSemanticFocusLookup(result, seen, focus, combined);
        pushSemanticFocusLookup(result, seen, focus, combined, "contains");
      }
      pushSemanticFocusLookup(result, seen, focus, focus.text, "contains", subject.text);
      const stripped = stripSemanticAnchorQualifier(focus.text, subject.text);
      if (stripped) {
        pushSemanticFocusTextAndParts(result, seen, focus, stripped);
      }
    }
    for (const subjectText of sourceSubjects) {
      const stripped = stripSemanticAnchorQualifier(focus.text, subjectText);
      if (stripped) {
        pushSemanticFocusTextAndParts(result, seen, focus, stripped);
      }
    }
  }
  return result;
}

function semanticIndexSubjectTexts(semanticIndex: KnowledgeSemanticIndex): string[] {
  return Object.entries(semanticIndex.labels)
    .filter(([, entry]) => entry.postings.some((posting) => isSubjectFocusKind(posting.kind)))
    .map(([text]) => text)
    .filter((text) => text.length > 0);
}

function pushSemanticFocusLookup(
  result: SemanticFocusLookup[],
  seen: Set<string>,
  focus: KnowledgeSearchQuery["semanticFocus"][number],
  lookupText: string,
  matchMode: SemanticFocusLookup["matchMode"] = "exact",
  requiredText?: string,
): void {
  const normalized = normalizeSemanticText(lookupText);
  if (!normalized) {
    return;
  }
  const required = requiredText ? normalizeSemanticText(requiredText) : "";
  const key = `${semanticFocusKey(focus.kind, focus.text)}=>${matchMode}:${required}:${normalized}`;
  if (seen.has(key)) {
    return;
  }
  seen.add(key);
  result.push({ focus, lookupText, matchMode, ...(requiredText ? { requiredText } : {}) });
}

function pushSemanticFocusTextAndParts(
  result: SemanticFocusLookup[],
  seen: Set<string>,
  focus: KnowledgeSearchQuery["semanticFocus"][number],
  lookupText: string,
): void {
  pushSemanticFocusLookup(result, seen, focus, lookupText);
  for (const part of splitSemanticCompoundText(lookupText)) {
    pushSemanticFocusLookup(result, seen, focus, part);
  }
}

function splitSemanticCompoundText(value: string): string[] {
  return value
    .split(/\s*(?:与|及|和|、|,|，|\/|／|&|\+|\band\b)\s*/iu)
    .map((part) => part.trim())
    .filter((part) => part.length > 0 && normalizeSemanticText(part) !== normalizeSemanticText(value));
}

function pushMatchedLabel(
  labels: KnowledgeSearchResult["results"][number]["matchedLabels"],
  label: KnowledgeSearchResult["results"][number]["matchedLabels"][number],
): void {
  const existingIndex = labels.findIndex((existing) =>
    existing.kind === label.kind && normalizeSemanticText(existing.text) === normalizeSemanticText(label.text)
  );
  if (existingIndex < 0) {
    labels.push(label);
    return;
  }
  const existing = labels[existingIndex];
  if (matchedLabelPriority(label) > matchedLabelPriority(existing)) {
    labels[existingIndex] = label;
  }
}

function matchedLabelPriority(label: KnowledgeSearchResult["results"][number]["matchedLabels"][number]): number {
  return (label.matchSource === "text" ? 10 : 0) + confidenceMultiplier(label.confidence);
}

function isSubjectFocusKind(kind: KnowledgeSemanticLabel["kind"]): boolean {
  return kind === "object" || kind === "page" || kind === "flow";
}

function isAttributiveFocusKind(kind: KnowledgeSemanticLabel["kind"]): boolean {
  return kind === "operation" || kind === "rule" || kind === "state" || kind === "field";
}

function combineSemanticAnchorTexts(subjectText: string, focusText: string): string[] {
  const subject = subjectText.trim();
  const focus = focusText.trim();
  if (!subject || !focus) {
    return [];
  }
  const variants = [`${subject} ${focus}`];
  if (containsCjk(subject) || containsCjk(focus)) {
    variants.push(`${subject}${focus}`);
  }
  return variants;
}

function stripSemanticAnchorQualifier(focusText: string, subjectText: string): string | null {
  const focus = focusText.trim();
  const subject = subjectText.trim();
  if (!focus || !subject) {
    return null;
  }
  const lowerFocus = focus.toLowerCase();
  const lowerSubject = subject.toLowerCase();
  if (!lowerFocus.startsWith(lowerSubject)) {
    return null;
  }
  const stripped = focus.slice(subject.length).replace(/^[\s:：/／,，\-–—_]+/, "").trim();
  return stripped.length > 0 ? stripped : null;
}

function containsCjk(value: string): boolean {
  return /[\u3400-\u9fff]/u.test(value);
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
    const sourceName = validateKnowledgeName(input.source);
    const source = await findKnowledgeSource(sourceName);
    if (!source) {
      const pending = await readPendingKnowledge(sourceName);
      if (pending?.sourceId && input.buildId) {
        const buildRunPath = knowledgeBuildRunFile(pending.sourceId, input.buildId);
        if (await pathExists(buildRunPath)) {
          const buildRun = asBuildRun(await readJsonFile(buildRunPath));
          if (buildRun.name !== sourceName) {
            throw invalidArgument("Knowledge build does not belong to the requested source.", {
              source: sourceName,
              buildId: input.buildId,
              buildSourceName: buildRun.name,
            });
          }
          return {
            sourceId: pending.sourceId,
            name: sourceName,
            status: "enabled",
            roots: buildRun.roots,
            index: {
              version: 0,
              lastBuiltAt: null,
              currentBuildId: input.buildId,
              documentCount: buildRun.documents.length,
              chunkCount: buildRun.chunks.length,
            },
            createdAt: pending.createdAt,
            updatedAt: pending.updatedAt,
          };
        }
      }
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
