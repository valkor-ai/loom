export const KNOWLEDGE_SCHEMA_VERSION = "1.0";

export const SUPPORTED_KNOWLEDGE_EXTENSIONS = [
  ".md",
  ".txt",
  ".json",
  ".yaml",
  ".yml",
  ".pdf",
  ".docx",
] as const;

export const DEFAULT_MAX_KNOWLEDGE_FILE_BYTES = 20 * 1024 * 1024;

export type KnowledgeRoot = {
  type: "file" | "directory";
  path: string;
};

export type KnowledgeSourceStatus = "enabled" | "disabled";

export type KnowledgeSource = {
  sourceId: string;
  name: string;
  status: KnowledgeSourceStatus;
  roots: KnowledgeRoot[];
  index: {
    version: number;
    lastBuiltAt: string | null;
    currentBuildId?: string | null;
    documentCount: number;
    chunkCount: number;
  };
  createdAt: string;
  updatedAt: string;
};

export type KnowledgeRegistry = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  sources: KnowledgeSource[];
};

export type PendingKnowledgeOperation =
  | {
      type: "add_paths";
      paths: string[];
    }
  | {
      type: "remove_paths";
      paths: string[];
    }
  | {
      type: "replace_paths";
      paths: string[];
    };

export type KnowledgePendingSource = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  name: string;
  sourceId: string | null;
  createNew: boolean;
  operations: PendingKnowledgeOperation[];
  validation: KnowledgeValidationSummary;
  createdAt: string;
  updatedAt: string;
};

export type KnowledgePendingSourceView = KnowledgePendingSource & {
  createdAtLocal: string | null;
  updatedAtLocal: string | null;
};

export type KnowledgeSourceView = Omit<KnowledgeSource, "index"> & {
  index: KnowledgeSource["index"] & {
    lastBuiltAtLocal: string | null;
  };
  createdAtLocal: string | null;
  updatedAtLocal: string | null;
};

export type KnowledgeValidationWarning = {
  path: string;
  reason: "unsupported_file_type" | "file_too_large" | "unreadable_path";
  message: string;
};

export type KnowledgeValidationSummary = {
  acceptedPaths: string[];
  acceptedFiles: number;
  acceptedDirectories: number;
  supportedFiles: number;
  skippedFiles: KnowledgeValidationWarning[];
  maxFileBytes: number;
};

export type KnowledgeAddResult = {
  name: string;
  pending: KnowledgePendingSource;
  addedPaths: string[];
  validation: KnowledgeValidationSummary;
  nextCommand: string;
  message: string;
};

export type KnowledgeUpdateResult = {
  name: string;
  pending: KnowledgePendingSource;
  operation: PendingKnowledgeOperation;
  validation: KnowledgeValidationSummary;
  nextCommand: string;
  message: string;
};

export type KnowledgePendingResult = {
  timeZone: string;
  pending: KnowledgePendingSourceView[];
};

export type KnowledgeDiscardResult = {
  name: string;
  discarded: boolean;
  message: string;
};

export type KnowledgeListResult = {
  timeZone: string;
  sources: Array<{
    name: string;
    status: KnowledgeSourceStatus | "pending";
    docs: number | null;
    createdAt: string | null;
    createdAtLocal: string | null;
    updatedAt: string | null;
    updatedAtLocal: string | null;
    lastBuild: string | null;
    lastBuildLocal: string | null;
    pendingOperations: number;
  }>;
};

export type KnowledgeStatusResult = {
  name: string;
  timeZone: string;
  source: KnowledgeSourceView | null;
  pending: KnowledgePendingSourceView | null;
};

export type KnowledgeRemoveResult = {
  name: string;
  removedSource: boolean;
  removedPending: boolean;
  message: string;
};

export type KnowledgeToggleResult = {
  name: string;
  status: KnowledgeSourceStatus;
  message: string;
};

export type KnowledgeFileSnapshot = {
  path: string;
  size: number;
  mtimeMs: number;
  contentHash: string;
  extension: string;
};

export type KnowledgeDocumentBlock =
  | { type: "heading"; level: number; text: string }
  | { type: "paragraph"; text: string }
  | { type: "list"; items: string[] }
  | { type: "table"; header: string[]; rows: string[][] }
  | { type: "code"; text: string }
  | { type: "pageBreak"; page?: number };

export type KnowledgeDocumentRecord = {
  documentId: string;
  sourceId: string;
  path: string;
  title: string;
  extension: string;
  size: number;
  mtimeMs: number;
  contentHash: string;
  chunkIds: string[];
};

export type KnowledgeSemanticLabel = {
  kind: "object" | "operation" | "state" | "rule" | "field" | "page" | "flow" | "other";
  text: string;
  normalizedText: string;
  aliases: string[];
  confidence: "low" | "medium" | "high";
};

export type KnowledgeBlockAffinity = {
  phaseScope: number;
  conceptGrounding: number;
  frontendExperience: number;
  finalSummary: number;
};

export type KnowledgeRetrievalFields = {
  title: string;
  headingPath: string[];
  summary: string;
  semanticLabelTexts: string[];
  semanticAliases: string[];
  bodyTextRef: string;
};

export type KnowledgeChunkRecord = {
  chunkId: string;
  documentId: string;
  sourceId: string;
  title: string;
  headingPath: string[];
  textRef: string;
  tokenEstimate: number;
  neighborChunkIds: string[];
  contextPrefix: string;
  splitReason: "section" | "soft_boundary" | "hard_boundary" | "hard_window_fallback" | "merged_small";
  retrievalFields: KnowledgeRetrievalFields;
  semanticLabels: KnowledgeSemanticLabel[];
  blockAffinity: KnowledgeBlockAffinity;
};

export type KnowledgeLexicalIndex = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  sourceId: string;
  buildId: string;
  chunkCount: number;
  averageDocumentLength: number;
  documentLengths: Record<string, number>;
  fieldWeights: {
    title: number;
    headingPath: number;
    summary: number;
    semanticLabelTexts: number;
    semanticAliases: number;
    body: number;
  };
  terms: Record<string, {
    df: number;
    postings: Array<{
      chunkId: string;
      tf: number;
      fields: Partial<Record<"title" | "headingPath" | "summary" | "semanticLabelTexts" | "semanticAliases" | "body", number>>;
    }>;
  }>;
};

export type KnowledgeSearchQuery = {
  naturalLanguageQuery: string;
  brainstormBlock: "phase_scope" | "concept_grounding" | "frontend_experience";
  semanticFocus: Array<{
    kind: KnowledgeSemanticLabel["kind"];
    text: string;
  }>;
  chunkLimit: number;
};

export type KnowledgeMatchQuery = {
  naturalLanguageQuery: string;
  brainstormBlock: KnowledgeSearchQuery["brainstormBlock"];
  semanticFocus: KnowledgeSearchQuery["semanticFocus"];
  sourceLimit: number;
  chunkLimitPerSource: number;
};

export type KnowledgeChunkCard = {
  sourceName: string;
  chunkId: string;
  documentTitle: string;
  headingPath: string[];
  summary: string;
  matchedLabels: Array<{
    kind: KnowledgeSemanticLabel["kind"];
    text: string;
    matchSource: "text" | "alias";
    confidence: KnowledgeSemanticLabel["confidence"];
  }>;
  score: number;
  tokenEstimate: number;
  inspectCommand: KnowledgeCommandInvocation & {
    name: "knowledge inspect";
  };
};

export type KnowledgeSearchResult = {
  query: KnowledgeSearchQuery;
  results: KnowledgeChunkCard[];
};

export type KnowledgeMatchCandidate = {
  sourceId: string;
  sourceName: string;
  lastBuiltAt: string;
  documentCount: number;
  chunkCount: number;
  matchScore: number;
  scoreBreakdown: {
    bestChunkScore: number;
    averageTop3ChunkScore: number;
    matchedFocusCoverage: number;
  };
  matchedFocus: Array<{
    kind: KnowledgeSemanticLabel["kind"];
    text: string;
    matchedChunkIds: string[];
  }>;
  topChunks: KnowledgeChunkCard[];
};

export type KnowledgeBlockReadPlan = {
  mode: "inspect_all_listed_chunks";
  chunks: Array<{
    sourceName: string;
    chunkId: string;
    inspectCommand: KnowledgeCommandInvocation & {
      name: "knowledge inspect";
    };
  }>;
};

export type BrainstormBlockKnowledgeContext = {
  status: "available" | "empty";
  block: KnowledgeSearchQuery["brainstormBlock"];
  matchQuery: KnowledgeMatchQuery;
  matchedSources: KnowledgeMatchCandidate[];
  readPlan: KnowledgeBlockReadPlan;
};

export type BrainstormKnowledgeContextResult = {
  context: BrainstormBlockKnowledgeContext;
};

export type KnowledgeInspectResult = {
  sourceName: string;
  sourceId: string;
  buildId: string;
  chunkId: string;
  documentTitle: string;
  headingPath: string[];
  tokenEstimate: number;
  text: string;
};

export type KnowledgeSemanticIndex = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  sourceId: string;
  buildId: string;
  labels: Record<string, {
    postings: Array<{
      chunkId: string;
      kind: KnowledgeSemanticLabel["kind"];
      source: "label" | "alias";
      confidence: KnowledgeSemanticLabel["confidence"];
    }>;
  }>;
};

export type KnowledgeBuildRun = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  buildId: string;
  sourceId: string;
  name: string;
  status: "semantic_pending" | "published";
  roots: KnowledgeRoot[];
  documents: KnowledgeDocumentRecord[];
  chunks: KnowledgeChunkRecord[];
  files: KnowledgeFileSnapshot[];
  pendingOperations: PendingKnowledgeOperation[];
  refs: {
    chunks: string;
    snapshot: string;
    lexicalIndex: string;
    semanticIndex?: string;
    semanticState?: string;
  };
  createdAt: string;
  updatedAt: string;
};

export type KnowledgeBuildResult = {
  name: string;
  sourceId: string;
  buildId: string;
  status: "semantic_pending";
  roots: KnowledgeRoot[];
  documentCount: number;
  chunkCount: number;
  packCount: number;
  skippedFiles: KnowledgeValidationWarning[];
  buildRunPath: string;
  chunksPath: string;
  snapshotPath: string;
  lexicalIndexPath: string;
  firstRequestPath: string;
  firstRequest: KnowledgeSemanticBuildRequest;
  message: string;
};

export type KnowledgeCommandInvocation = {
  argv: string[];
};

export type KnowledgeSemanticPackInfo = {
  packId: string;
  packIndex: number;
  chunkIds: string[];
  requestPath: string;
  resultFile: string;
};

export type KnowledgeSemanticBuildState = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  buildId: string;
  sourceId: string;
  sourceName: string;
  status: "pending" | "published";
  packCount: number;
  acceptedPackIds: string[];
  packs: KnowledgeSemanticPackInfo[];
  createdAt: string;
  updatedAt: string;
};

export type KnowledgeSemanticBuildRequest = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  requestId: string;
  buildId: string;
  buildRunPath: string;
  sourceId: string;
  sourceName: string;
  packId: string;
  packIndex: number;
  packCount: number;
  chunkPack: {
    chunks: Array<{
      chunkId: string;
      documentId: string;
      documentTitle: string;
      relativePath: string;
      headingPath: string[];
      tokenEstimate: number;
      textRef: string;
      readCommand: KnowledgeCommandInvocation;
      previousChunkTitle?: string;
      nextChunkTitle?: string;
      splitReason?: string;
    }>;
  };
  outputContract: {
    resultFile: string;
    schema: "KnowledgeSemanticPackResult";
  };
  generationRules: {
    labelKinds: KnowledgeSemanticLabel["kind"][];
    confidenceValues: KnowledgeSemanticLabel["confidence"][];
    summaryRule: string;
    semanticLabelRule: string;
    blockAffinityRule: string;
  };
  submitCommand: KnowledgeCommandInvocation;
  requestReadPlan: {
    mustReadChunkText: boolean;
    chunkTextRefs: string[];
  };
};

export type KnowledgeSemanticChunkResult = {
  chunkId: string;
  status: "completed" | "low_signal" | "unreadable";
  summary: string;
  semanticLabels: KnowledgeSemanticLabel[];
  blockAffinity: KnowledgeBlockAffinity;
  notes?: string[];
};

export type KnowledgeSemanticPackResult = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  buildId: string;
  packId: string;
  chunkResults: KnowledgeSemanticChunkResult[];
};

export type KnowledgeSemanticSubmitIssue = {
  code: string;
  message: string;
  path?: string;
  chunkId?: string;
};

export type KnowledgeSemanticSubmitResult = {
  status: "accepted" | "needs_repair";
  buildId: string;
  packId: string;
  acceptedPackIds: string[];
  packCount: number;
  nextRequestPath?: string;
  nextRequest?: KnowledgeSemanticBuildRequest;
  published?: {
    name: string;
    sourceId: string;
    buildId: string;
    documentCount: number;
    chunkCount: number;
  };
  repairRequestPath?: string;
  repairRequest?: {
    schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
    buildId: string;
    packId: string;
    resultFile: string;
    issues: KnowledgeSemanticSubmitIssue[];
    repairScope: "current_pack_result_only";
  };
  message: string;
};
