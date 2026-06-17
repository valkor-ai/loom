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
  pending: KnowledgePendingSource[];
};

export type KnowledgeDiscardResult = {
  name: string;
  discarded: boolean;
  message: string;
};

export type KnowledgeListResult = {
  sources: Array<{
    name: string;
    status: KnowledgeSourceStatus | "pending";
    docs: number | null;
    lastBuild: string | null;
    pendingOperations: number;
  }>;
};

export type KnowledgeStatusResult = {
  name: string;
  source: KnowledgeSource | null;
  pending: KnowledgePendingSource | null;
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

export type KnowledgeBuildRun = {
  schemaVersion: typeof KNOWLEDGE_SCHEMA_VERSION;
  buildId: string;
  sourceId: string;
  name: string;
  status: "mechanical_ready";
  roots: KnowledgeRoot[];
  documents: KnowledgeDocumentRecord[];
  chunks: KnowledgeChunkRecord[];
  files: KnowledgeFileSnapshot[];
  pendingOperations: PendingKnowledgeOperation[];
  refs: {
    chunks: string;
    snapshot: string;
    lexicalIndex: string;
  };
  createdAt: string;
  updatedAt: string;
};

export type KnowledgeBuildResult = {
  name: string;
  sourceId: string;
  buildId: string;
  status: "mechanical_ready";
  roots: KnowledgeRoot[];
  documentCount: number;
  chunkCount: number;
  skippedFiles: KnowledgeValidationWarning[];
  buildRunPath: string;
  chunksPath: string;
  snapshotPath: string;
  lexicalIndexPath: string;
  message: string;
};
