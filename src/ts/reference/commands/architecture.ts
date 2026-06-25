import { invalidArgument } from "../core/errors";
import {
  acceptArchitectureArtifact,
  createArchitectureRequest,
} from "../core/operations/contracts";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export function createArchitectureRequestHandler(options: { deliveryId?: string; phaseId?: string; planningContractId?: string; replaceActive?: boolean }) {
  return async function handleArchitectureRequest(ctx: CommandContext): Promise<CliEnvelope> {
    const result = await createArchitectureRequest({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      planningContractId: options.planningContractId,
      replaceActive: options.replaceActive,
    });
    return ok("architecture.request", ctx.projectRoot, compactRequestCommandResult(result), "ArchitectureArtifactRequest created.");
  };
}

export function createArchitectureAcceptHandler(options: { deliveryId?: string; phaseId?: string; candidateFile?: string; requestId?: string; repairId?: string }) {
  return async function handleArchitectureAccept(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.candidateFile && !options.requestId) {
      throw invalidArgument("architecture accept requires --request-id or --candidate-file <path>.");
    }
    const result = await acceptArchitectureArtifact({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      candidateFile: options.candidateFile,
      requestId: options.requestId,
      repairId: options.repairId,
    });
    return ok(
      "architecture.accept",
      ctx.projectRoot,
      result,
      result.accepted
        ? "ArchitectureArtifactContract accepted."
        : "ArchitectureArtifactContract candidate failed validation.",
    );
  };
}
