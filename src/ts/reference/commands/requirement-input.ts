import { createHash } from "node:crypto";
import path from "node:path";
import { invalidArgument } from "../core/errors";
import { extractRequirementText } from "../core/requirements/extract-text";
import type { InputSource, RequirementInput } from "../core/schemas";

export type PlanInputOptions = {
  positional?: string[];
  request?: string[];
  requestFile?: string[];
  requirementFile?: string[];
  stdin?: boolean;
  context?: string[];
  contextFile?: string[];
  skipKeywordHints?: boolean;
};

export async function buildRequirementInput(
  projectRoot: string,
  options: PlanInputOptions,
): Promise<RequirementInput> {
  const requestSources: InputSource[] = [];
  const contextSources: InputSource[] = [];

  const positional = joinText(options.positional ?? []);
  if (positional) {
    requestSources.push({ kind: "positional", content: positional });
  }

  for (const request of options.request ?? []) {
    if (request.trim()) {
      requestSources.push({ kind: "request-option", content: request });
    }
  }

  for (const requestFile of options.requestFile ?? []) {
    const source = await readInputFile(projectRoot, requestFile, "request-file");
    requestSources.push(source);
  }

  for (const requirementFile of options.requirementFile ?? []) {
    const source = await readInputFile(projectRoot, requirementFile, "request-file");
    requestSources.push(source);
  }

  if (options.stdin) {
    requestSources.push({
      kind: "stdin",
      content: await readStdin(),
    });
  }

  for (const context of options.context ?? []) {
    if (context.trim()) {
      contextSources.push({ kind: "context", content: context });
    }
  }

  for (const contextFile of options.contextFile ?? []) {
    const source = await readInputFile(projectRoot, contextFile, "context-file");
    contextSources.push(source);
  }

  const primaryRequest =
    requestSources.find(
      (source) =>
        (source.kind === "positional" || source.kind === "request-option") &&
        source.content.trim(),
    )?.content.trim() ??
    firstParagraph(requestSources.find((source) => source.content.trim())?.content ?? "");

  if (!primaryRequest) {
    throw invalidArgument("Plan requires a request source.", {
      acceptedInputs: ["positional", "--request", "--request-file", "--stdin"],
    });
  }

  return {
    primaryRequest,
    requestSources,
    contextSources,
    ...(options.skipKeywordHints ? { skipKeywordHints: true } : {}),
  };
}

function joinText(values: string[]): string {
  return values.join(" ").trim();
}

async function readInputFile(
  projectRoot: string,
  filePath: string,
  kind: "request-file" | "context-file",
): Promise<InputSource> {
  if (filePath === "-") {
    return {
      kind: "stdin",
      label: "stdin",
      content: await readStdin(),
    };
  }

  const absolutePath = path.resolve(projectRoot, filePath);
  let content: string;
  let extractionStatus: InputSource["extractionStatus"] = "completed";
  let extractionReason: string | undefined;
  try {
    const extracted = await extractRequirementText(absolutePath);
    content = extracted.text;
    extractionStatus = extracted.status;
    extractionReason = extracted.reason;
    const source: InputSource = {
      kind,
      label: filePath,
      path: absolutePath,
      content,
      extractionStatus,
      ...(extractionReason ? { extractionReason } : {}),
      mimeType: extracted.mimeType,
      digest: extracted.digest,
      ...(content.trim() ? { textDigest: `sha256:${hashText(content.trim())}` } : {}),
    };
    return source;
  } catch {
    throw invalidArgument(`${kind === "request-file" ? "Request" : "Context"} file does not exist.`, {
      path: absolutePath,
    });
  }
}

async function readStdin(): Promise<string> {
  if (process.stdin.isTTY) {
    return "";
  }

  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

function firstParagraph(content: string): string {
  return content
    .split(/\n\s*\n/)
    .map((part) => part.trim())
    .find(Boolean) ?? "";
}

function hashText(content: string): string {
  return createHash("sha256").update(content).digest("hex");
}
