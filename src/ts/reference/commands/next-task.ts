import { getNextTask } from "../core/operations/tasks";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export function createNextTaskHandler(options: { deliveryId?: string; phaseId?: string; taskPlanRunId?: string }) {
  return async function handleNextTask(ctx: CommandContext): Promise<CliEnvelope> {
    const result = await getNextTask({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      taskPlanRunId: options.taskPlanRunId,
    });
    return ok(
      "next-task",
      ctx.projectRoot,
      result,
      result.hasTask ? "Next task execution request created." : "No ready loom task.",
    );
  };
}
