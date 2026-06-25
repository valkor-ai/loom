import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument } from "../core/errors";
import { recordTaskResult } from "../core/operations/tasks";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export function createRecordResultHandler(options: { deliveryId?: string; phaseId?: string; inputFile?: string }) {
  return async function handleRecordResult(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.inputFile) {
      throw invalidArgument("record-result requires --input-file <path>.", {
        option: "--input-file",
      });
    }

    const inputFile = path.resolve(ctx.projectRoot, options.inputFile);
    try {
      await fs.access(inputFile);
    } catch {
      throw invalidArgument("Result input file does not exist.", { path: inputFile });
    }

    const result = await recordTaskResult({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      inputFile: options.inputFile,
    });

    return ok(
      "record-result",
      ctx.projectRoot,
      result,
      result.recorded
        ? "TaskResult recorded. Follow data.instruction immediately when autoContinue is true."
        : "TaskResult failed validation. Follow the returned repair instruction, then run record-result again.",
    );
  };
}
