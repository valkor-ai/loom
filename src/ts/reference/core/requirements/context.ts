import { promises as fs } from "node:fs";
import path from "node:path";
import type { RequirementInput, InputSource } from "../schemas";
import {
  requirementContextPath,
  requirementExtractedTextPath,
  requirementInputTextPath,
  requirementKeywordHintsPath,
  requirementNormalizedTextPath,
  toProjectRelative,
} from "../state/paths";
import { writeJsonAtomic } from "../state/fs";
import { digestText, extractRequirementText, mimeTypeForPath } from "./extract-text";
import { generateKeywordHints } from "./keyword-hints";
import type { RequirementContext, RequirementSourceItem } from "./types";

export type RequirementContextResult = {
  context: RequirementContext;
  contextRef: string;
  normalizedTextRef: string | null;
  keywordHintsRef: string | null;
};

export async function createRequirementContext(input: {
  projectRoot: string;
  deliveryId: string;
  requirementInput: RequirementInput;
  createdAt: string;
}): Promise<RequirementContextResult> {
  const allSources = [
    ...input.requirementInput.requestSources.map((source) => ({ source, origin: originForSource(source) })),
    ...input.requirementInput.contextSources.map((source) => ({ source, origin: originForSource(source) })),
  ];
  const sourceItems: RequirementSourceItem[] = [];
  const normalizedParts: Array<{ itemId: string; title?: string; textRef: string; text: string }> = [];

  for (const [index, entry] of allSources.entries()) {
    const itemId = `req-${String(index + 1).padStart(3, "0")}`;
    if (entry.source.path) {
      const extracted = entry.source.extractionStatus
        ? {
            text: entry.source.content,
            mimeType: entry.source.mimeType ?? mimeTypeForPath(entry.source.path),
            status: entry.source.extractionStatus,
            reason: entry.source.extractionReason,
            digest: entry.source.digest ?? digestText(entry.source.path),
          }
        : await extractRequirementText(entry.source.path);
      const text = extracted.text.trim();
      const extractedRef = text ? toProjectRelative(input.projectRoot, requirementExtractedTextPath(input.projectRoot, input.deliveryId, itemId)) : null;
      if (text && extractedRef) {
        await writeText(input.projectRoot, extractedRef, text);
        normalizedParts.push({
          itemId,
          title: entry.source.label,
          textRef: extractedRef,
          text,
        });
      }
      sourceItems.push({
        itemId,
        kind: "file",
        origin: entry.origin === "context_file" ? "context_file" : "request_file",
        ...(entry.source.label ? { title: entry.source.label } : {}),
        path: entry.source.path,
        mimeType: extracted.mimeType || mimeTypeForPath(entry.source.path),
        ...(extractedRef ? { extractedTextRef: extractedRef, textRef: extractedRef } : {}),
        extractionStatus: extracted.status,
        ...(extracted.reason ? { extractionReason: extracted.reason } : {}),
        digest: extracted.digest,
        ...(text ? { textDigest: entry.source.textDigest ?? digestText(text), characterCount: text.length } : {}),
      });
      continue;
    }

    const text = entry.source.content.trim();
    if (!text) {
      continue;
    }
    const textRef = toProjectRelative(input.projectRoot, requirementInputTextPath(input.projectRoot, input.deliveryId, itemId));
    await writeText(input.projectRoot, textRef, text);
    normalizedParts.push({
      itemId,
      title: entry.source.label ?? entry.source.kind,
      textRef,
      text,
    });
    sourceItems.push({
      itemId,
      kind: "text",
      origin: textOriginForSource(entry.source),
      ...(entry.source.label ? { title: entry.source.label } : {}),
      textRef,
      extractionStatus: "completed",
      digest: digestText(text),
      characterCount: text.length,
    });
  }

  const normalizedText = normalizedParts
    .map((part) => `## ${part.itemId}${part.title ? ` ${part.title}` : ""}\n\n${part.text}`)
    .join("\n\n")
    .trim();
  const normalizedTextRef = normalizedText
    ? toProjectRelative(input.projectRoot, requirementNormalizedTextPath(input.projectRoot, input.deliveryId))
    : null;
  if (normalizedTextRef) {
    await writeText(input.projectRoot, normalizedTextRef, normalizedText);
  }

  let keywordHintsRef: string | null = null;
  let keywordHintsStatus: RequirementContext["keywordHintsStatus"] = input.requirementInput.skipKeywordHints ? "skipped" : "empty";
  let keywordHintsReason: string | undefined;
  if (input.requirementInput.skipKeywordHints) {
    keywordHintsReason = "Keyword hints were disabled by --skip-keyword-hints.";
  } else if (normalizedParts.length > 0) {
    const hints = generateKeywordHints({
      deliveryId: input.deliveryId,
      generatedAt: input.createdAt,
      sources: normalizedParts.map((part) => ({
        sourceItemId: part.itemId,
        title: part.title,
        textRef: part.textRef,
        text: part.text,
      })),
    });
    keywordHintsRef = toProjectRelative(input.projectRoot, requirementKeywordHintsPath(input.projectRoot, input.deliveryId));
    await writeJsonAtomic(path.resolve(input.projectRoot, keywordHintsRef), hints);
    keywordHintsStatus = hints.status;
    keywordHintsReason = hints.status === "empty" ? "No stable keyword hints were extracted." : undefined;
  } else {
    keywordHintsReason = "No extracted requirement text was available.";
  }

  const context: RequirementContext = {
    schemaVersion: "1.0",
    deliveryId: input.deliveryId,
    createdAt: input.createdAt,
    sourceItems,
    normalizedTextRef,
    normalizedTextStatus: normalizedTextRef ? "completed" : "empty",
    ...(normalizedTextRef ? {} : { normalizedTextReason: "No requirement text was available after extraction." }),
    keywordHintsRef,
    keywordHintsStatus,
    ...(keywordHintsReason ? { keywordHintsReason } : {}),
  };
  const contextPath = requirementContextPath(input.projectRoot, input.deliveryId);
  await writeJsonAtomic(contextPath, context);

  return {
    context,
    contextRef: toProjectRelative(input.projectRoot, contextPath),
    normalizedTextRef,
    keywordHintsRef,
  };
}

async function writeText(projectRoot: string, relativePath: string, text: string): Promise<void> {
  const absolutePath = path.resolve(projectRoot, relativePath);
  await fs.mkdir(path.dirname(absolutePath), { recursive: true });
  await fs.writeFile(absolutePath, `${text}\n`, "utf8");
}

function originForSource(source: InputSource): "request_file" | "context_file" | "text" {
  if (source.kind === "context-file") {
    return "context_file";
  }
  if (source.path) {
    return "request_file";
  }
  return "text";
}

function textOriginForSource(source: InputSource): "user_message" | "stdin" | "cli_option" {
  if (source.kind === "stdin") {
    return "stdin";
  }
  if (source.kind === "request-option" || source.kind === "context") {
    return "cli_option";
  }
  return "user_message";
}
