import { readFileSync } from "node:fs";
import path from "node:path";
import {
  KNOWLEDGE_SCHEMA_VERSION,
  type KnowledgeChunkRecord,
  type KnowledgeLexicalIndex,
} from "./types";

const FIELD_WEIGHTS = {
  title: 5,
  headingPath: 4,
  summary: 4,
  semanticLabelTexts: 4,
  semanticAliases: 3,
  body: 1,
} as const;

export function buildLexicalIndex(
  sourceId: string,
  buildId: string,
  chunks: KnowledgeChunkRecord[],
  runDir: string,
): KnowledgeLexicalIndex {
  const termPostings = new Map<string, Map<string, {
    chunkId: string;
    tf: number;
    fields: Partial<Record<"title" | "headingPath" | "summary" | "semanticLabelTexts" | "semanticAliases" | "body", number>>;
  }>>();
  let totalLength = 0;
  for (const chunk of chunks) {
    const body = readFileSync(path.join(runDir, chunk.textRef), "utf8");
    const fieldValues = {
      title: chunk.retrievalFields.title,
      headingPath: chunk.retrievalFields.headingPath.join(" "),
      summary: chunk.retrievalFields.summary,
      semanticLabelTexts: chunk.retrievalFields.semanticLabelTexts.join(" "),
      semanticAliases: chunk.retrievalFields.semanticAliases.join(" "),
      body,
    };
    const docTokens = new Set<string>();
    for (const [field, value] of Object.entries(fieldValues) as Array<[keyof typeof fieldValues, string]>) {
      const counts = countTerms(tokenizeKnowledgeText(value));
      for (const [term, count] of counts.entries()) {
        docTokens.add(term);
        if (!termPostings.has(term)) {
          termPostings.set(term, new Map());
        }
        const byChunk = termPostings.get(term)!;
        const posting = byChunk.get(chunk.chunkId) ?? {
          chunkId: chunk.chunkId,
          tf: 0,
          fields: {},
        };
        posting.tf += count;
        posting.fields[field] = (posting.fields[field] ?? 0) + count;
        byChunk.set(chunk.chunkId, posting);
      }
    }
    totalLength += docTokens.size;
  }
  const terms: KnowledgeLexicalIndex["terms"] = {};
  for (const [term, byChunk] of [...termPostings.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    const postings = [...byChunk.values()].sort((a, b) => a.chunkId.localeCompare(b.chunkId));
    terms[term] = {
      df: postings.length,
      postings,
    };
  }
  return {
    schemaVersion: KNOWLEDGE_SCHEMA_VERSION,
    sourceId,
    buildId,
    chunkCount: chunks.length,
    averageDocumentLength: chunks.length > 0 ? totalLength / chunks.length : 0,
    fieldWeights: FIELD_WEIGHTS,
    terms,
  };
}

export function tokenizeKnowledgeText(text: string): string[] {
  const normalized = text.toLowerCase();
  const latin = normalized.match(/[a-z0-9_]+/g) ?? [];
  const cjk = [...normalized.matchAll(/[\u3400-\u9fff]+/g)]
    .flatMap((match) => cjkNgrams(match[0]));
  return [...latin, ...cjk].filter((token) => token.length > 0);
}

function cjkNgrams(text: string): string[] {
  const chars = [...text];
  if (chars.length <= 1) {
    return chars;
  }
  const result: string[] = [];
  for (let index = 0; index < chars.length - 1; index += 1) {
    result.push(chars.slice(index, index + 2).join(""));
  }
  for (let index = 0; index < chars.length - 2; index += 1) {
    result.push(chars.slice(index, index + 3).join(""));
  }
  return result;
}

function countTerms(tokens: string[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const token of tokens) {
    counts.set(token, (counts.get(token) ?? 0) + 1);
  }
  return counts;
}
