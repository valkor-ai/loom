import { getStatus } from "../core/operations/get-status";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export async function handleStatus(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await getStatus({
    projectRoot: ctx.projectRoot,
    commandSurface: ctx.agentProfile === "codex" ? "@loom" : "/loom",
  });
  return ok("status", ctx.projectRoot, result, "loom state is initialized.");
}
