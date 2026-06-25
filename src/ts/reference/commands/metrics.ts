import { getTokenSavingMetrics } from "../core/operations/get-token-saving-metrics";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export async function handleTokenSavingMetrics(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await getTokenSavingMetrics({
    projectRoot: ctx.projectRoot,
  });
  return ok("metrics.token-saving", ctx.projectRoot, {
    tokenSaving: result,
  }, "Token-saving telemetry loaded.");
}
