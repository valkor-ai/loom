import { createPlanningContract } from "../core/operations/contracts";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export function createPlanningContractCreateHandler(options: {
  deliveryId?: string;
  brainstormRunId?: string;
  phaseId?: string;
}) {
  return async function handlePlanningContractCreate(ctx: CommandContext): Promise<CliEnvelope> {
    const result = await createPlanningContract({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      brainstormRunId: options.brainstormRunId,
      phaseId: options.phaseId,
    });
    return ok(
      "planning-contract.create",
      ctx.projectRoot,
      result,
      result.status === "ready"
        ? "PlanningGenerationContract is ready for ArchitectureArtifactContract."
        : "PlanningGenerationContract is blocked.",
    );
  };
}
