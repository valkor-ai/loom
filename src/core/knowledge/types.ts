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
