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
import { submitKnowledgeSemanticPack } from "../core/knowledge/semantic";
import { buildBrainstormKnowledgeContext, inspectKnowledge, searchKnowledge } from "../core/knowledge/search";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext, CommandHandler } from "./types";

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
    return ok("knowledge.build", ctx.projectRoot, result, result.message);
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
    return ok("knowledge.semantic.submit", ctx.projectRoot, result, result.message);
  };
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
    const context = await buildBrainstormKnowledgeContext(input);
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
