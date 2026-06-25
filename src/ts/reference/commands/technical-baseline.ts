import { invalidArgument } from "../core/errors";
import {
  acceptTechnicalBaseline,
  createTechnicalBaselineRequest,
  detectRepoSignals,
} from "../core/operations/contracts";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export async function handleTechnicalBaselineDetect(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await detectRepoSignals({ projectRoot: ctx.projectRoot });
  return ok("technical-baseline.detect", ctx.projectRoot, result, "Repo signals detected.");
}

export function createTechnicalBaselineRequestHandler(options: {
  deliveryId?: string;
  phaseId?: string;
  brainstormRunId?: string;
  projectKind?: string;
}) {
  return async function handleTechnicalBaselineRequest(ctx: CommandContext): Promise<CliEnvelope> {
    const projectKind = normalizeProjectKind(options.projectKind);
    const result = await createTechnicalBaselineRequest({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      brainstormRunId: options.brainstormRunId,
      projectKind,
    });
    return ok("technical-baseline.request", ctx.projectRoot, compactRequestCommandResult(result), "TechnicalBaseline request created.");
  };
}

export function createTechnicalBaselineAcceptHandler(options: { deliveryId?: string; phaseId?: string; candidateFile?: string }) {
  return async function handleTechnicalBaselineAccept(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.candidateFile) {
      throw invalidArgument("technical-baseline accept requires --candidate-file <path>.");
    }
    const result = await acceptTechnicalBaseline({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      candidateFile: options.candidateFile,
    });
    return ok(
      "technical-baseline.accept",
      ctx.projectRoot,
      result,
      result.accepted
        ? "TechnicalBaseline accepted. Follow data.instruction immediately when autoContinue is true."
        : result.instruction?.mode === "ask_user" || result.nextAction?.type === "needs_user_decision"
        ? "TechnicalBaseline requires explicit user confirmation. Ask the user to confirm or correct the technology baseline before submitting again."
        : "TechnicalBaseline candidate failed validation. Follow data.repairInstruction, then run technical-baseline accept again.",
    );
  };
}

function normalizeProjectKind(value: string | undefined): "greenfield" | "existing_project" | "unknown" | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (value === "greenfield" || value === "existing_project" || value === "unknown") {
    return value;
  }
  throw invalidArgument("Invalid project kind.", {
    accepted: ["greenfield", "existing_project", "unknown"],
  });
}
