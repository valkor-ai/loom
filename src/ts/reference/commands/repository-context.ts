import { invalidArgument } from "../core/errors";
import {
  acceptRepositoryContext,
  createRepositoryContextRequest,
} from "../core/operations/repository-context";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export function createRepositoryContextRequestHandler(options: { deliveryId?: string; phaseId?: string }) {
  return async function handleRepositoryContextRequest(ctx: CommandContext): Promise<CliEnvelope> {
    const result = await createRepositoryContextRequest({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
    });
    return ok("repository-context.request", ctx.projectRoot, compactRequestCommandResult(result), "RepositoryContextRequest created.");
  };
}

export function createRepositoryContextAcceptHandler(options: {
  deliveryId?: string;
  phaseId?: string;
  requestId?: string;
  candidateFile?: string;
}) {
  return async function handleRepositoryContextAccept(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.candidateFile) {
      throw invalidArgument("repository-context accept requires --candidate-file <path>.");
    }
    const result = await acceptRepositoryContext({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      requestId: options.requestId,
      candidateFile: options.candidateFile,
    });
    return ok(
      "repository-context.accept",
      ctx.projectRoot,
      result,
      result.operation === "repository_context_accepted"
        ? "RepositoryContext accepted."
        : "RepositoryContext candidate failed validation. Follow data.repairInstruction, then run repository-context accept again.",
    );
  };
}
