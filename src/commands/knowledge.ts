import path from "node:path";
import {
  addKnowledgeSource,
  discardKnowledgePending,
  getKnowledgeStatus,
  listKnowledgePending,
  listKnowledgeSources,
  removeKnowledge,
  setKnowledgeEnabled,
  updateKnowledgeSource,
} from "../core/knowledge/operations";
import { buildKnowledgeSource } from "../core/knowledge/build";
import { resumeKnowledgeSemanticBuild, submitKnowledgeSemanticPack } from "../core/knowledge/semantic";
import { buildBrainstormKnowledgeContext, inspectKnowledge, searchKnowledge } from "../core/knowledge/search";
import { invalidArgument } from "../core/errors";
import { loadDeliveryIndex, loadProjectStatus } from "../core/state/delivery";
import { ensureDir, readJsonFile } from "../core/state/fs";
import { brainstormKnowledgeQueryDir } from "../core/state/paths";
import { withAutoRunnableTransition } from "../core/operations/routing-instructions";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext, CommandHandler } from "./types";
import type {
  KnowledgeSemanticBuildRequest,
  KnowledgeSemanticSubmitIssue,
} from "../core/knowledge/types";

export function createKnowledgeAddHandler(input: {
  name?: string;
  paths?: string[];
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await addKnowledgeSource({
      name: input.name,
      paths: input.paths ?? [],
    });
    return ok("knowledge.add", ctx.projectRoot, result, result.message);
  };
}

export function createKnowledgeUpdateHandler(input: {
  name?: string;
  addPath?: string[];
  removePath?: string[];
  replacePaths?: string[];
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await updateKnowledgeSource({
      name: input.name,
      addPath: input.addPath,
      removePath: input.removePath,
      replacePaths: input.replacePaths,
    });
    return ok("knowledge.update", ctx.projectRoot, result, result.message);
  };
}

export function createKnowledgePendingHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await listKnowledgePending(input.name);
    const suffix = input.name ? ` for "${input.name}"` : "";
    return ok("knowledge.pending", ctx.projectRoot, result, `Loaded pending knowledge changes${suffix}.`);
  };
}

export function createKnowledgeDiscardHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await discardKnowledgePending(input.name);
    return ok("knowledge.discard", ctx.projectRoot, result, result.message);
  };
}

export function createKnowledgeBuildHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await buildKnowledgeSource({ name: input.name });
    return ok("knowledge.build", ctx.projectRoot, {
      ...result,
      instruction: knowledgeSemanticInstruction({
        sourceCommand: "knowledge.build",
        sourceSummary: result.message,
        requestPath: result.firstRequestPath,
        request: result.firstRequest,
      }),
    }, result.message);
  };
}

export function createKnowledgeResumeHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await resumeKnowledgeSemanticBuild({ name: input.name });
    return ok("knowledge.resume", ctx.projectRoot, {
      ...result,
      ...(result.status === "semantic_pending"
        ? {
            instruction: knowledgeSemanticInstruction({
              sourceCommand: "knowledge.resume",
              sourceSummary: result.message,
              requestPath: result.nextRequestPath,
              request: result.nextRequest,
            }),
          }
        : {}),
    }, result.message);
  };
}

export function createKnowledgeSemanticSubmitHandler(input: {
  requestFile?: string;
  resultFile?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await submitKnowledgeSemanticPack({
      requestFile: input.requestFile,
      resultFile: input.resultFile,
    });
    return ok("knowledge.semantic.submit", ctx.projectRoot, {
      ...result,
      ...(result.nextRequestPath && result.nextRequest
        ? {
            instruction: knowledgeSemanticInstruction({
              sourceCommand: "knowledge.semantic.submit",
              sourceSummary: result.message,
              requestPath: result.nextRequestPath,
              request: result.nextRequest,
            }),
          }
        : {}),
      ...(result.status === "needs_repair" && result.repairRequestPath
        ? {
            instruction: knowledgeSemanticInstruction({
              sourceCommand: "knowledge.semantic.submit",
              sourceSummary: result.message,
              requestPath: input.requestFile,
              request: undefined,
              resultFile: input.resultFile,
              issues: result.repairRequest?.issues,
              repairRequestPath: result.repairRequestPath,
            }),
          }
        : {}),
    }, result.message);
  };
}

function knowledgeSemanticInstruction(input: {
  sourceCommand: string;
  sourceSummary: string;
  requestPath: string | undefined;
  request: KnowledgeSemanticBuildRequest | undefined;
  resultFile?: string;
  issues?: KnowledgeSemanticSubmitIssue[];
  repairRequestPath?: string;
}): Record<string, unknown> {
  const requestRef = input.requestPath;
  const resultFile = input.resultFile ?? input.request?.outputContract.resultFile;
  const request = input.request;
  const submitCommand = requestRef && resultFile
    ? {
        name: "knowledge semantic submit",
        argv: ["knowledge", "semantic", "submit", "--request", requestRef, "--result-file", resultFile],
      }
    : undefined;
  return withAutoRunnableTransition({
    mode: "generate_knowledge_semantics",
    requestRef,
    resultFile,
    schema: "KnowledgeSemanticPackResult",
    submitCommand,
    issues: input.issues,
    repairRequestPath: input.repairRequestPath,
    knowledgeSemantic: request
      ? {
          sourceName: request.sourceName,
          buildId: request.buildId,
          packId: request.packId,
          packIndex: request.packIndex,
          packCount: request.packCount,
          chunkCount: request.chunkPack.chunks.length,
          mustReadChunkText: request.requestReadPlan.mustReadChunkText,
        }
      : undefined,
    routingRule: "Generate the requested knowledge semantic pack now. Do not ask the user whether to continue between knowledge build, semantic pack generation, and semantic submit.",
    instructions: [
      "Read instruction.requestRef as a KnowledgeSemanticBuildRequest.",
      "For every chunk listed in request.chunkPack.chunks, read the chunk text through chunk.readCommand; direct chunk file paths are not part of the agent-facing request contract.",
      "Copy request.outputContract.resultTemplate as the result file shape, then fill each existing chunkResult for its matching chunkId.",
      "Do not inspect Loom source files, dist files, TypeScript type definitions, or old semantic result files to infer the KnowledgeSemanticPackResult schema; the request outputContract.resultTemplate is the schema authority.",
      "Follow request.generationRules as the semantic generation authority; do not replace those rules with script-generated semantic heuristics.",
      "Fill each chunkResult according to request.outputContract.resultTemplate and request.generationRules.",
      "Run instruction.submitCommand after writing the result.",
      "If the submit response returns another generate_knowledge_semantics instruction, continue immediately until the source is published or a non-repairable blocker appears.",
    ],
  }, {
    sourceCommand: input.sourceCommand,
    sourceSummary: input.sourceSummary,
    primaryAction: "generate_knowledge_semantic_pack_and_submit",
    userVisibleSummary: `${input.sourceSummary} Continue generating the semantic pack now; this is part of the knowledge build, not a separate user decision.`,
    completionCondition: "The semantic pack result has been written and submitted; if another pack is returned, it has also been generated and submitted until publish or blocker.",
    requiredSteps: [
      "read instruction.requestRef",
      "read every chunk body through request.chunkPack.chunks[].readCommand",
      "copy request.outputContract.resultTemplate and fill it as instruction.resultFile",
      "run instruction.submitCommand",
      "follow any returned generate_knowledge_semantics instruction immediately",
    ],
    forbiddenStops: [
      "do not stop after knowledge build reports semantic_pending",
      "do not ask the user whether to continue to the semantic pack",
      "do not summarize progress before instruction.submitCommand succeeds",
      "do not read Loom source, dist, TypeScript definitions, or old results to infer the semantic result schema",
      "do not replace request.generationRules with generated script heuristics",
    ],
    stopOnlyWhen: [
      "the knowledge source is published",
      "the requestRef or required chunk text cannot be read",
      "semantic submit returns a non-repairable failure",
    ],
  });
}

export function createKnowledgeSearchHandler(input: {
  query?: string;
  queryFile?: string;
  source?: string[];
  block?: string;
  semanticFocus?: string[];
  limit?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await searchKnowledge(input);
    return ok("knowledge.search", ctx.projectRoot, result, `Found ${result.results.length} knowledge chunk(s).`);
  };
}

export function createKnowledgeBrainstormContextHandler(input: {
  query?: string;
  queryFile?: string;
  block?: string;
  semanticFocus?: string[];
  sourceLimit?: string;
  chunkLimitPerSource?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const scopedInput = {
      ...input,
      queryFile: input.queryFile ? resolveProjectFile(ctx.projectRoot, input.queryFile) : input.queryFile,
    };
    await enforceCurrentBrainstormQueryWorkspace(ctx, scopedInput.queryFile);
    const context = await buildBrainstormKnowledgeContext(scopedInput);
    return ok(
      "knowledge.brainstorm_context",
      ctx.projectRoot,
      { context },
      context.status === "available"
        ? `Prepared Brainstorm knowledge context for ${context.block}.`
        : `No matching Brainstorm knowledge context for ${context.block}.`,
    );
  };
}

async function enforceCurrentBrainstormQueryWorkspace(ctx: CommandContext, queryFile: string | undefined): Promise<void> {
  if (!queryFile) {
    return;
  }
  const scope = await currentBrainstormKnowledgeQueryScope(ctx.projectRoot);
  if (!scope) {
    return;
  }

  await ensureDir(scope.allowedDirectory);
  const queryPath = path.resolve(queryFile);
  if (isInsidePath(queryPath, scope.allowedDirectory)) {
    return;
  }

  throw invalidArgument("Brainstorm knowledge query file must be written inside the current Brainstorm request query workspace.", {
    queryFile: queryPath,
    allowedDirectory: scope.allowedDirectory,
    deliveryId: scope.deliveryId,
    phaseId: scope.phaseId,
    requestId: scope.requestId,
    reason: "Active Brainstorm sessions do not accept project tmp/loom files, previous request files, or old query files as knowledge query input.",
  });
}

async function currentBrainstormKnowledgeQueryScope(projectRoot: string): Promise<{
  deliveryId: string;
  phaseId: string;
  requestId: string;
  allowedDirectory: string;
} | null> {
  let status: Awaited<ReturnType<typeof loadProjectStatus>>;
  try {
    status = await loadProjectStatus(projectRoot);
  } catch {
    return null;
  }

  const deliveryId = status.activeDeliveryId;
  if (!deliveryId) {
    return null;
  }

  let index: Awaited<ReturnType<typeof loadDeliveryIndex>>;
  try {
    index = await loadDeliveryIndex(projectRoot, deliveryId);
  } catch {
    return null;
  }
  if (index.status !== "brainstorming") {
    return null;
  }

  const phaseId = index.activePhaseId;
  const phase = index.phases.find((item) => item.phaseId === phaseId);
  const requestId = phase?.latestRefs.brainstormRequestId;
  const requestRef = phase?.latestRefs.brainstormRequest;
  if (!requestId || !requestRef) {
    return null;
  }

  let requestWorkspace: string | null = null;
  try {
    const request = await readJsonFile(path.resolve(projectRoot, requestRef));
    if (!isRecord(request) || request.requestType !== "brainstorm_session") {
      return null;
    }
    requestWorkspace = readQueryWorkspaceDirectory(request);
  } catch {
    requestWorkspace = null;
  }

  return {
    deliveryId,
    phaseId,
    requestId,
    allowedDirectory: requestWorkspace
      ? path.resolve(projectRoot, requestWorkspace)
      : brainstormKnowledgeQueryDir(projectRoot, deliveryId, phaseId, requestId),
  };
}

function readQueryWorkspaceDirectory(request: Record<string, unknown>): string | null {
  const protocol = request.knowledgeContextProtocol;
  if (!isRecord(protocol)) {
    return null;
  }
  const workspace = protocol.queryWorkspace;
  if (!isRecord(workspace) || typeof workspace.directory !== "string" || workspace.directory.trim().length === 0) {
    return null;
  }
  return workspace.directory;
}

function isInsidePath(childPath: string, parentPath: string): boolean {
  const relative = path.relative(path.resolve(parentPath), path.resolve(childPath));
  return relative === "" || (relative.length > 0 && !relative.startsWith("..") && !path.isAbsolute(relative));
}

function resolveProjectFile(projectRoot: string, fileRef: string): string {
  return path.isAbsolute(fileRef)
    ? fileRef
    : path.resolve(projectRoot, fileRef);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function createKnowledgeInspectHandler(input: {
  source?: string;
  buildId?: string;
  chunkId?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await inspectKnowledge(input);
    return ok("knowledge.inspect", ctx.projectRoot, result, `Loaded knowledge chunk ${result.chunkId}.`);
  };
}

export async function handleKnowledgeList(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await listKnowledgeSources();
  return ok("knowledge.list", ctx.projectRoot, result, "Loaded knowledge sources.");
}

export function createKnowledgeStatusHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await getKnowledgeStatus(input.name);
    return ok("knowledge.status", ctx.projectRoot, result, `Loaded knowledge source "${result.name}".`);
  };
}

export function createKnowledgeRemoveHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await removeKnowledge(input.name);
    return ok("knowledge.remove", ctx.projectRoot, result, result.message);
  };
}

export function createKnowledgeEnableHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await setKnowledgeEnabled({
      name: input.name,
      enabled: true,
    });
    return ok("knowledge.enable", ctx.projectRoot, result, result.message);
  };
}

export function createKnowledgeDisableHandler(input: {
  name?: string;
}): CommandHandler {
  return async (ctx: CommandContext): Promise<CliEnvelope> => {
    const result = await setKnowledgeEnabled({
      name: input.name,
      enabled: false,
    });
    return ok("knowledge.disable", ctx.projectRoot, result, result.message);
  };
}
