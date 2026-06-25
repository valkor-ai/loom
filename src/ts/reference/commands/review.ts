import { invalidArgument } from "../core/errors";
import {
  acceptReviewResult,
  createReviewRequest,
  resolveReview,
} from "../core/operations/review";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export async function handleReview(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await createReviewRequest({ projectRoot: ctx.projectRoot });
  return ok("review", ctx.projectRoot, compactRequestCommandResult(result), "ReviewRequest created.");
}

export function createReviewHandler(options: { deliveryId?: string; phaseId?: string }) {
  return async function handleReviewWithOptions(ctx: CommandContext): Promise<CliEnvelope> {
    const result = await createReviewRequest({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
    });
    return ok("review", ctx.projectRoot, compactRequestCommandResult(result), "ReviewRequest created.");
  };
}

export function createReviewAcceptHandler(options: { deliveryId?: string; phaseId?: string; resultFile?: string }) {
  return async function handleReviewAccept(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.resultFile) {
      throw invalidArgument("review accept requires --result-file <path>.");
    }
    const result = await acceptReviewResult({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      resultFile: options.resultFile,
    });
    return ok(
      "review.accept",
      ctx.projectRoot,
      result,
      result.accepted ? "ReviewResult accepted." : "ReviewResult failed validation; fallback manual review result created.",
    );
  };
}

export function createReviewResolveHandler(options: {
  deliveryId?: string;
  phaseId?: string;
  candidateFile?: string;
}) {
  return async function handleReviewResolve(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.candidateFile) {
      throw invalidArgument("review resolve requires --candidate-file <path>.");
    }
    const result = await resolveReview({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      candidateFile: options.candidateFile,
    });
    return ok("review.resolve", ctx.projectRoot, result, "Manual review resolution recorded.");
  };
}
