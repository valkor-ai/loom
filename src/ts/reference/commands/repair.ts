import { invalidArgument } from "../core/errors";
import {
  createDeployExecutionRepairRequest,
  createRepairRequest,
  submitDeployExecutionRepairResult,
} from "../core/operations/repair";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export function createRepairRequestHandler(options: { deliveryId?: string; phaseId?: string; type?: string; source?: string; failureRef?: string }) {
  return async function handleRepairRequest(ctx: CommandContext): Promise<CliEnvelope> {
    const type = normalizeType(options.type);
    const source = normalizeSource(options.source);
    if (source === "deploy") {
      if (type !== "execution") {
        throw invalidArgument("deploy-sourced repair request requires --type execution.");
      }
      if (!options.failureRef) {
        throw invalidArgument("deploy-sourced repair request requires --failure-ref.");
      }
      const result = await createDeployExecutionRepairRequest({
        projectRoot: ctx.projectRoot,
        failureRef: options.failureRef,
      });
      return ok("repair.request", ctx.projectRoot, compactRequestCommandResult(result), "Deploy-sourced execution repair request created.");
    }
    const result = await createRepairRequest({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      type,
    });
    return ok("repair.request", ctx.projectRoot, compactRequestCommandResult(result), "RepairRequest created.");
  };
}

export function createRepairSubmitHandler(options: { type?: string; source?: string; repairId?: string; resultFile?: string }) {
  return async function handleRepairSubmit(ctx: CommandContext): Promise<CliEnvelope> {
    const type = normalizeType(options.type);
    const source = normalizeSource(options.source);
    if (source !== "deploy" || type !== "execution") {
      throw invalidArgument("repair submit currently supports --type execution --source deploy.");
    }
    if (!options.repairId) {
      throw invalidArgument("repair submit requires --repair-id.");
    }
    if (!options.resultFile) {
      throw invalidArgument("repair submit requires --result-file.");
    }
    const result = await submitDeployExecutionRepairResult({
      projectRoot: ctx.projectRoot,
      repairId: options.repairId,
      resultFile: options.resultFile,
    });
    return ok(
      "repair.submit",
      ctx.projectRoot,
      result,
      result.accepted
        ? "Deploy-sourced execution repair result accepted."
        : "Deploy-sourced execution repair result did not complete.",
    );
  };
}

function normalizeType(value: string | undefined): "execution" | "task-result" | "taskplan" | "architecture" {
  if (value === "execution" || value === "task-result" || value === "taskplan" || value === "architecture") {
    return value;
  }
  throw invalidArgument("repair request requires --type execution|task-result|taskplan|architecture.");
}

function normalizeSource(value: string | undefined): "delivery" | "deploy" {
  if (value === undefined || value === "delivery") {
    return "delivery";
  }
  if (value === "deploy") {
    return "deploy";
  }
  throw invalidArgument("repair source must be delivery or deploy.", { source: value });
}
