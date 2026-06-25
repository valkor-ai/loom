import { initProject } from "../core/operations/init-project";
import { startBrainstorm } from "../core/operations/brainstorm";
import { brainstormStartInstruction } from "./brainstorm-instruction";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import { buildRequirementInput, type PlanInputOptions } from "./requirement-input";
import type { CliEnvelope, CommandContext } from "./types";

export function createPlanHandler(options: PlanInputOptions) {
  return async function handlePlan(ctx: CommandContext): Promise<CliEnvelope> {
    await initProject({ projectRoot: ctx.projectRoot });
    const requirementInput = await buildRequirementInput(ctx.projectRoot, options);
    const result = await startBrainstorm({
      projectRoot: ctx.projectRoot,
      requirementInput,
    });
    return ok("plan", ctx.projectRoot, compactRequestCommandResult({
      ...result,
      instruction: brainstormStartInstruction(result),
    }), "Brainstorm started for loom plan flow.");
  };
}
