import { initProject } from "../core/operations/init-project";
import { ok } from "./envelope";
import type { CommandContext, CliEnvelope } from "./types";

export async function handleInit(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await initProject({ projectRoot: ctx.projectRoot });
  return ok("init", ctx.projectRoot, result, "loom state is initialized.");
}
