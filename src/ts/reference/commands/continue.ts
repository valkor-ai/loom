import { continueDelivery } from "../core/operations/continue";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export async function handleContinue(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await continueDelivery({
    projectRoot: ctx.projectRoot,
    commandSurface: ctx.agentProfile === "codex" ? "@loom" : "/loom",
  });
  return ok("continue", ctx.projectRoot, result, "ContinueDecision loaded.");
}
