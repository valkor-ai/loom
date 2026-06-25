import { promises as fs } from "node:fs";
import path from "node:path";
import { invalidArgument } from "../core/errors";
import {
  acceptBrainstormCandidate,
  answerBrainstorm,
  confirmBrainstorm,
  getBrainstormStatus,
  startBrainstorm,
} from "../core/operations/brainstorm";
import { brainstormStartInstruction } from "./brainstorm-instruction";
import { compactRequestCommandResult } from "./compact-request-output";
import { ok } from "./envelope";
import { buildRequirementInput, type PlanInputOptions } from "./requirement-input";
import type { CliEnvelope, CommandContext } from "./types";

export type BrainstormStartOptions = PlanInputOptions & {
  inputFile?: string[];
};

export type BrainstormAnswerOptions = {
  deliveryId?: string;
  phaseId?: string;
  runId?: string;
  questionId?: string;
  answer?: string;
  answerFile?: string;
  selectedOption?: string[];
};

export type BrainstormConfirmOptions = {
  deliveryId?: string;
  phaseId?: string;
  runId?: string;
  confirmationId?: string;
  decision?: string;
  revision?: string;
  confirmationFile?: string;
};

export type BrainstormAcceptOptions = {
  deliveryId?: string;
  phaseId?: string;
  runId?: string;
  requestId?: string;
  candidateFile?: string;
};

export type BrainstormStatusOptions = {
  runId?: string;
};

export function createBrainstormStartHandler(options: BrainstormStartOptions) {
  return async function handleBrainstormStart(ctx: CommandContext): Promise<CliEnvelope> {
    const requirementInput = await buildRequirementInput(ctx.projectRoot, {
      ...options,
      requestFile: [...(options.requestFile ?? []), ...(options.inputFile ?? [])],
    });
    const result = await startBrainstorm({
      projectRoot: ctx.projectRoot,
      requirementInput,
    });

    return ok(
      "brainstorm.start",
      ctx.projectRoot,
      compactRequestCommandResult({
        ...result,
        instruction: brainstormStartInstruction(result),
      }),
      "Brainstorm session request created. Agent must manage clarification and submit a BrainstormCandidate.",
    );
  };
}

export function createBrainstormAnswerHandler(options: BrainstormAnswerOptions) {
  return async function handleBrainstormAnswer(ctx: CommandContext): Promise<CliEnvelope> {
    const runId = requireRunId(options.runId);
    const fileInput = options.answerFile
      ? await readJsonOrTextFile(resolveCliFilePath(ctx.projectRoot, options.answerFile))
      : {};
    const answerText = pickString(fileInput, "answerText") ?? pickString(fileInput, "answer") ?? options.answer;

    if (!answerText?.trim()) {
      throw invalidArgument("brainstorm answer requires --answer or --answer-file.");
    }

    const result = await answerBrainstorm({
      projectRoot: ctx.projectRoot,
      brainstormRunId: runId,
      answerText,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      questionId: pickString(fileInput, "questionId") ?? options.questionId,
      selectedOptionIds: pickStringArray(fileInput, "selectedOptionIds") ?? options.selectedOption,
    });

    return ok(
      "brainstorm.answer",
      ctx.projectRoot,
      result,
      "Brainstorm interpreted the answer and needs confirmation.",
    );
  };
}

export function createBrainstormConfirmHandler(options: BrainstormConfirmOptions) {
  return async function handleBrainstormConfirm(ctx: CommandContext): Promise<CliEnvelope> {
    const runId = requireRunId(options.runId);
    const fileInput = options.confirmationFile
      ? await readJsonOrTextFile(resolveCliFilePath(ctx.projectRoot, options.confirmationFile))
      : {};
    const decision = normalizeDecision(pickString(fileInput, "decision") ?? options.decision);
    const result = await confirmBrainstorm({
      projectRoot: ctx.projectRoot,
      brainstormRunId: runId,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      confirmationId: pickString(fileInput, "confirmationId") ?? options.confirmationId,
      decision,
      revisionText: pickString(fileInput, "revisionText") ?? pickString(fileInput, "revision") ?? options.revision,
    });

    return ok(
      "brainstorm.confirm",
      ctx.projectRoot,
      result,
      result.status === "confirmed"
        ? "Brainstorm contract is confirmed."
        : "Brainstorm needs clarification before planning.",
    );
  };
}

export function createBrainstormStatusHandler(options: BrainstormStatusOptions) {
  return async function handleBrainstormStatus(ctx: CommandContext): Promise<CliEnvelope> {
    const result = await getBrainstormStatus({
      projectRoot: ctx.projectRoot,
      brainstormRunId: requireRunId(options.runId),
    });

    return ok("brainstorm.status", ctx.projectRoot, result, "Brainstorm status loaded.");
  };
}

export function createBrainstormAcceptHandler(options: BrainstormAcceptOptions) {
  return async function handleBrainstormAccept(ctx: CommandContext): Promise<CliEnvelope> {
    if (!options.candidateFile?.trim()) {
      throw invalidArgument("brainstorm accept requires --candidate-file.");
    }
    const result = await acceptBrainstormCandidate({
      projectRoot: ctx.projectRoot,
      deliveryId: options.deliveryId,
      phaseId: options.phaseId,
      brainstormRunId: options.runId,
      requestId: options.requestId,
      candidateFile: options.candidateFile,
    });

    return ok(
      "brainstorm.accept",
      ctx.projectRoot,
      result,
      result.accepted
        ? "Brainstorm candidate accepted."
        : "Brainstorm candidate was not accepted.",
    );
  };
}

function requireRunId(runId: string | undefined): string {
  if (!runId?.trim()) {
    throw invalidArgument("Brainstorm command requires --run-id.");
  }
  return runId.trim();
}

function normalizeDecision(value: string | undefined): "confirmed" | "revise" {
  const normalized = value?.trim().toLowerCase();
  if (normalized === "confirmed" || normalized === "yes" || normalized === "y") {
    return "confirmed";
  }
  if (normalized === "revise" || normalized === "revision") {
    return "revise";
  }
  throw invalidArgument("brainstorm confirm requires --decision confirmed|revise.");
}

async function readJsonOrTextFile(filePath: string): Promise<Record<string, unknown>> {
  const raw = await fs.readFile(filePath, "utf8");
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
    throw invalidArgument("Input JSON file must contain an object.", { path: filePath });
  } catch (error) {
    if (error instanceof SyntaxError) {
      return { answerText: raw };
    }
    throw error;
  }
}

function pickString(input: Record<string, unknown>, key: string): string | undefined {
  const value = input[key];
  return typeof value === "string" ? value : undefined;
}

function pickStringArray(input: Record<string, unknown>, key: string): string[] | undefined {
  const value = input[key];
  if (!Array.isArray(value)) {
    return undefined;
  }
  return value.filter((item): item is string => typeof item === "string");
}

function resolveCliFilePath(projectRoot: string, filePath: string): string {
  return path.isAbsolute(filePath) ? filePath : path.resolve(projectRoot, filePath);
}
