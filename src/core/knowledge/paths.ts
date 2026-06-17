import os from "node:os";
import path from "node:path";

export type KnowledgePaths = {
  home: string;
  root: string;
  registryFile: string;
  pendingDir: string;
  sourcesDir: string;
};

export function knowledgePaths(): KnowledgePaths {
  const home = process.env.LOOM_HOME && process.env.LOOM_HOME.trim().length > 0
    ? path.resolve(process.env.LOOM_HOME)
    : path.join(os.homedir(), ".loom");
  const root = path.join(home, "knowledge");
  return {
    home,
    root,
    registryFile: path.join(root, "registry.json"),
    pendingDir: path.join(root, "pending"),
    sourcesDir: path.join(root, "sources"),
  };
}

export function pendingKnowledgeFile(name: string): string {
  return path.join(knowledgePaths().pendingDir, `${safeKnowledgeFileName(name)}.json`);
}

export function knowledgeSourceDir(sourceId: string): string {
  return path.join(knowledgePaths().sourcesDir, sourceId);
}

export function knowledgeBuildRunDir(sourceId: string, buildId: string): string {
  return path.join(knowledgeSourceDir(sourceId), "build-runs", buildId);
}

export function knowledgeBuildRunFile(sourceId: string, buildId: string): string {
  return path.join(knowledgeBuildRunDir(sourceId, buildId), "build-run.json");
}

export function knowledgeSemanticStateFile(sourceId: string, buildId: string): string {
  return path.join(knowledgeBuildRunDir(sourceId, buildId), "semantic-state.json");
}

export function knowledgeSemanticRequestFile(sourceId: string, buildId: string, packId: string): string {
  return path.join(knowledgeBuildRunDir(sourceId, buildId), "semantic-requests", `${packId}.json`);
}

export function knowledgeSemanticResultFile(sourceId: string, buildId: string, packId: string): string {
  return path.join(knowledgeBuildRunDir(sourceId, buildId), "semantic-results", `${packId}.json`);
}

export function knowledgeSemanticRepairFile(sourceId: string, buildId: string, packId: string): string {
  return path.join(knowledgeBuildRunDir(sourceId, buildId), "semantic-repairs", `${packId}.json`);
}

function safeKnowledgeFileName(name: string): string {
  return name.replace(/[^a-zA-Z0-9._-]+/g, "_");
}
