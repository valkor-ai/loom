import { invalidArgument } from "../core/errors";
import {
  acceptTaskPlan,
  createTaskPlanRequest,
} from "../core/operations/tasks";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export function createTaskPlanRequestHandler(options: {
  deliveryId?: string;
  phaseId?: string;
  planningContractId?: string;
  architectureArtifactContractId?: string;
}) {
  return async function handleTaskPlanRequest(ctx: CommandContext): Promise<CliEnvelope> {
    const result = await createTaskPlanRequest({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      planningContractId: options.planningContractId,
      architectureArtifactContractId: options.architectureArtifactContractId,
    });
    return ok("task-plan.request", ctx.projectRoot, compactRequestCommandResult(result), "TaskPlanGenerationRequest created.");
  };
}

export function createTaskPlanAcceptHandler(options: { deliveryId?: string; phaseId?: string; candidateFile?: string; requestId?: string; repairId?: string }) {
  return async function handleTaskPlanAccept(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.candidateFile && !options.requestId) {
      throw invalidArgument("task-plan accept requires --request-id or --candidate-file <path>.");
    }
    const result = await acceptTaskPlan({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      candidateFile: options.candidateFile,
      requestId: options.requestId,
      repairId: options.repairId,
    });
    return ok(
      "task-plan.accept",
      ctx.projectRoot,
      result,
      result.accepted ? "TaskPlan accepted and run created." : "TaskPlan candidate failed validation.",
    );
  };
}
