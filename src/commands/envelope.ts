import path from "node:path";
import { LoomError, internalError, summaryForError, withFailureRecovery, type FailureRecoveryContext } from "../core/errors";
import { LOOM_VERSION } from "../version";
import { agentFacingInstruction } from "./agent-facing-instruction";
import type { CliEnvelope, CliFailureEnvelope } from "./types";

export function ok<T>(
  command: string,
  projectRoot: string,
  data: T,
  summary: string,
): CliEnvelope<T> {
  const rawInstruction = extractInstruction(data);
  const instruction = agentFacingInstruction(rawInstruction);
  const actionRequired = rawInstruction ? summarizeActionRequired(rawInstruction) : undefined;
  const responseData = instruction ? withAgentFacingDataInstruction(data, instruction) : data;
  return {
    ok: true,
    command,
    version: LOOM_VERSION,
    projectRoot: path.resolve(projectRoot),
    ...(actionRequired ? { actionRequired } : {}),
    ...(instruction ? { instruction } : {}),
    data: responseData,
    summary: actionRequired?.summary ?? summary,
  };
}

export function fail(
  command: string,
  projectRoot: string,
  error: LoomError,
  summary = summaryForError(error.code),
): CliFailureEnvelope {
  const details = sanitizeDetails(error.details);
  return {
    ok: false,
    command,
    version: LOOM_VERSION,
    projectRoot: path.resolve(projectRoot),
    error: {
      code: error.code,
      message: error.message,
      ...(details === undefined ? {} : { details }),
    },
    summary,
  };
}

export function toFailureEnvelope(
  command: string,
  projectRoot: string,
  error: unknown,
  context: FailureRecoveryContext = {},
): CliFailureEnvelope {
  const recoveryContext = {
    command,
    projectRoot,
    ...context,
  };
  const normalizedError = error instanceof LoomError
    ? withFailureRecovery(error, recoveryContext)
    : internalError(error, recoveryContext);
  return fail(command, projectRoot, normalizedError);
}

function sanitizeDetails(details: unknown): unknown {
  if (details === undefined) {
    return undefined;
  }

  try {
    return JSON.parse(JSON.stringify(details));
  } catch {
    return { type: typeof details };
  }
}

function extractInstruction(data: unknown): Record<string, unknown> | undefined {
  if (!isRecord(data)) return undefined;
  const instruction = isRecord(data.instruction) ? data.instruction : data.repairInstruction;
  if (!isRecord(instruction)) return undefined;
  const mode = instruction.mode;
  if (typeof mode !== "string" || mode.length === 0) return undefined;
  return instruction;
}

function withAgentFacingDataInstruction<T>(data: T, instruction: Record<string, unknown>): T {
  if (!isRecord(data)) {
    return data;
  }
  if (!isRecord(data.instruction) && isRecord(data.repairInstruction)) {
    return {
      ...data,
      instruction,
    } as T;
  }
  if (!isRecord(data.instruction)) return data;
  return {
    ...data,
    instruction,
  } as T;
}

function summarizeActionRequired(instruction: Record<string, unknown>): {
  mode: string;
  autoContinue: boolean;
  mustRunImmediately: boolean;
  mustNotReportProgress: boolean;
  mustNotAskBeforeExecuting: boolean;
  requestRef?: string;
  resultFile?: string;
  candidateFile?: string;
  targetCandidateFile?: string;
  submitCommand?: Record<string, unknown>;
  command?: Record<string, unknown>;
  retryCommand?: Record<string, unknown>;
  commandInvocation?: Record<string, unknown>;
  completionBarrier?: Record<string, unknown>;
  finalResponseGuard?: Record<string, unknown>;
  primaryAction?: string;
  fieldRepairPlan?: unknown[];
  requiredSteps?: unknown[];
  forbiddenStops?: unknown[];
  stopOnlyWhen?: unknown[];
  summary: string;
} | undefined {
  const mode = typeof instruction.mode === "string" ? instruction.mode : "";
  const autoContinue = instruction.autoContinue === true;
  const mustRunImmediately =
    instruction.mustRunImmediately === true ||
    instruction.mustStartImmediately === true ||
    instruction.mustNotAskUserBeforeRunning === true ||
    instruction.mustNotAskUserBeforeExecuting === true;

  if (!autoContinue && !mustRunImmediately) {
    return undefined;
  }

  const completionBarrier = isRecord(instruction.completionBarrier)
    ? instruction.completionBarrier
    : undefined;
  const finalResponseGuard = finalResponseGuardForInstruction(mode, instruction, completionBarrier);
  const continuationContract = isRecord(instruction.continuationContract)
    ? instruction.continuationContract
    : undefined;
  const agentObligation = isRecord(continuationContract?.agentObligation)
    ? continuationContract.agentObligation
    : undefined;

  return {
    mode,
    autoContinue,
    mustRunImmediately: autoContinue || mustRunImmediately,
    mustNotReportProgress: true,
    mustNotAskBeforeExecuting: true,
    ...pickStringFields(instruction, ["requestRef", "resultFile", "candidateFile", "targetCandidateFile"]),
    ...(mode !== "execute_task" && isRecord(instruction.submitCommand) ? { submitCommand: instruction.submitCommand } : {}),
    ...(isRecord(instruction.command) ? { command: instruction.command } : {}),
    ...(isRecord(instruction.retryCommand) ? { retryCommand: instruction.retryCommand } : {}),
    ...(isRecord(instruction.commandInvocation) ? { commandInvocation: instruction.commandInvocation } : {}),
    ...(completionBarrier ? { completionBarrier: compactActionCompletionBarrier(completionBarrier) } : {}),
    ...(finalResponseGuard ? { finalResponseGuard } : {}),
    ...(typeof instruction.primaryAction === "string" ? { primaryAction: instruction.primaryAction } : {}),
    ...(Array.isArray(instruction.fieldRepairPlan) ? { fieldRepairPlan: instruction.fieldRepairPlan } : {}),
    ...(Array.isArray(agentObligation?.requiredSteps) ? { requiredSteps: agentObligation.requiredSteps } : {}),
    ...(Array.isArray(agentObligation?.forbiddenStops) ? { forbiddenStops: agentObligation.forbiddenStops } : {}),
    ...(Array.isArray(agentObligation?.stopOnlyWhen) ? { stopOnlyWhen: agentObligation.stopOnlyWhen } : {}),
    summary: summaryForInstruction(mode, instruction),
  };
}

function compactActionCompletionBarrier(completionBarrier: Record<string, unknown>): Record<string, unknown> {
  const output: Record<string, unknown> = {};
  for (const key of ["resultFile", "targetCandidateFile", "followUpCommand"] as const) {
    if (key in completionBarrier) {
      output[key] = completionBarrier[key];
    }
  }
  return output;
}

function finalResponseGuardForInstruction(
  mode: string,
  instruction: Record<string, unknown>,
  completionBarrier: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined {
  if (mode === "execute_task") {
    const resultFile = typeof instruction.resultFile === "string"
      ? instruction.resultFile
      : typeof completionBarrier?.resultFile === "string"
        ? completionBarrier.resultFile
        : undefined;
    return {
      rule: "Do not send a final or progress-only response while this auto-runnable task is incomplete.",
      invalidFinalResponseWhen: resultFile
        ? `TaskResult is missing at ${resultFile} or submitCommand has not succeeded.`
        : "TaskResult is missing or submitCommand has not succeeded.",
      requiredActionBeforeFinalResponse: "Continue executing instruction.requestRef, write resultFile, and run submitCommand. If implementation cannot complete, write a failed or blocked TaskResult and run submitCommand.",
    };
  }

  if (mode === "run_cli") {
    return {
      rule: "Do not send a final or progress-only response while this auto-runnable CLI transition is incomplete.",
      invalidFinalResponseWhen: "instruction.commandInvocation or instruction.command has not been run, or the returned auto-runnable instruction has not been followed.",
      requiredActionBeforeFinalResponse: "Run instruction.commandInvocation with the launcher/env/argvWithProjectRoot exactly, then read the returned CLI envelope and immediately follow its auto-runnable instruction.",
    };
  }
  if (mode === "deploy_repair_assets") {
    return {
      rule: "Do not send a final or progress-only response while this auto-runnable deployment asset repair is incomplete.",
      invalidFinalResponseWhen: "The returned editableFiles have not been repaired or retryCommand deploy up has not been run.",
      requiredActionBeforeFinalResponse: "Read errorWindow/diagnostics, edit only editableFiles, run retryCommand, then follow any returned deploy repair instruction immediately.",
    };
  }

  if (mode === "generate_candidate") {
    if (instruction.candidateKind === "ArchitectureSections" && instruction.sectionGenerationMode === "single_section") {
      const targetCandidateFile = typeof instruction.targetCandidateFile === "string"
        ? instruction.targetCandidateFile
        : typeof completionBarrier?.targetCandidateFile === "string"
          ? completionBarrier.targetCandidateFile
          : "instruction.targetCandidateFile";
      return {
        rule: "Do not send a final or progress-only response while this auto-runnable architecture section generation step is incomplete.",
        invalidFinalResponseWhen: `${targetCandidateFile} is missing, or completionBarrier.followUpCommand has not been run and its returned auto-runnable instruction has not been followed.`,
        requiredActionBeforeFinalResponse: "Read instruction.requestRef, generate only instruction.targetSection at instruction.targetCandidateFile, run completionBarrier.followUpCommand immediately, then follow the returned instruction when auto-runnable.",
      };
    }
    return {
      rule: "Do not send a final or progress-only response while this auto-runnable candidate generation is incomplete.",
      invalidFinalResponseWhen: "The requested candidate/result files are missing or submitCommand has not succeeded.",
      requiredActionBeforeFinalResponse: "Read instruction.requestRef, generate the requested output files, run submitCommand, then follow the returned instruction when auto-runnable.",
    };
  }

  if (mode === "submit_existing_candidate") {
    return {
      rule: "Do not send a final or progress-only response while this auto-runnable submit is incomplete.",
      invalidFinalResponseWhen: "submitCommand has not succeeded for the existing candidate/result files.",
      requiredActionBeforeFinalResponse: "Read instruction.requestRef when needed, verify the referenced files exist, run submitCommand, then follow the returned instruction when auto-runnable.",
    };
  }

  if (mode === "repair_candidate" || mode === "repair_result_contract") {
    return {
      rule: "Do not send a final or progress-only response while this auto-runnable repair is incomplete.",
      invalidFinalResponseWhen: "The referenced candidate/result artifact has not been repaired and resubmitted successfully.",
      requiredActionBeforeFinalResponse: "Repair only the referenced artifact, run submitCommand, then follow the returned instruction when auto-runnable.",
    };
  }

  return {
    rule: "Do not send a final or progress-only response while this auto-runnable instruction is incomplete.",
    invalidFinalResponseWhen: "The top-level instruction has not been executed or its returned auto-runnable instruction has not been followed.",
    requiredActionBeforeFinalResponse: "Follow the top-level instruction now, then follow the returned instruction when auto-runnable.",
  };
}

function summaryForInstruction(mode: string, instruction: Record<string, unknown>): string {
  const continuationContract = isRecord(instruction.continuationContract)
    ? instruction.continuationContract
    : undefined;
  const userVisibleMeaning = isRecord(continuationContract?.userVisibleMeaning)
    ? continuationContract.userVisibleMeaning
    : undefined;
  if (mode === "execute_task") {
    if (typeof userVisibleMeaning?.summary === "string" && userVisibleMeaning.summary.length > 0) {
      return withNoRecapGuard(`ACTION REQUIRED: ${userVisibleMeaning.summary}`);
    }
    const requestRef = typeof instruction.requestRef === "string" ? instruction.requestRef : "instruction.requestRef";
    return withNoRecapGuard(`ACTION REQUIRED: execute_task is auto-runnable. Read ${requestRef}, load requestManifest.refs.agentAction.ref when root agentAction is absent, use agentAction.read.fieldGroups inspect readCommands for required request fields, complete the task, write the resultFile, and run submitCommand now. A final response before resultFile exists and submitCommand succeeds is invalid. Do not summarize progress or ask whether to continue.`);
  }
  if (mode === "run_cli") {
    const command = isRecord(instruction.command) && Array.isArray(instruction.command.argv)
      ? instruction.command.argv.join(" ")
      : "instruction.command.argv";
    return withNoRecapGuard(`ACTION REQUIRED: run_cli is auto-runnable. Run the commandInvocation launcher with ${command} now and follow the returned instruction. Do not use bare loom, summarize progress, or ask whether to continue.`);
  }
  if (mode === "generate_candidate") {
    const requestRef = typeof instruction.requestRef === "string" ? instruction.requestRef : "instruction.requestRef";
    if (instruction.candidateKind === "ArchitectureSections" && instruction.sectionGenerationMode === "single_section") {
      const targetSection = typeof instruction.targetSection === "string" ? instruction.targetSection : "targetSection";
      const targetCandidateFile = typeof instruction.targetCandidateFile === "string" ? instruction.targetCandidateFile : "targetCandidateFile";
      return withNoRecapGuard(`ACTION REQUIRED: generate_candidate is auto-runnable. Read ${requestRef}, load requestManifest.refs.agentAction.ref when root agentAction is absent, use agentAction.read.fieldGroups inspect readCommands, write only the ${targetSection} section to ${targetCandidateFile}, then run loom continue through commandInvocation so the CLI can route the next missing section or submit_existing_candidate. Do not summarize progress or ask whether to continue.`);
    }
    if (instruction.candidateKind === "TaskPlanGroupedOutputs") {
      const outputSummary = isRecord(instruction.outputSummary) ? instruction.outputSummary : {};
      const outlineFile = typeof outputSummary.outlineFile === "string" ? outputSummary.outlineFile : "outputContract.outlineFile";
      const groupFilePattern = typeof outputSummary.groupFilePattern === "string" ? outputSummary.groupFilePattern : "outputContract.groupFilePattern";
      return withNoRecapGuard(`ACTION REQUIRED: generate_candidate is auto-runnable. Read ${requestRef}, load requestManifest.refs.agentAction.ref when root agentAction is absent, use agentAction.read.fieldGroups inspect readCommands, write TaskPlan outline to ${outlineFile}, write group files using ${groupFilePattern}, and run submitCommand now. Do not summarize request creation, report progress, or ask whether to continue.`);
    }
    return withNoRecapGuard(`ACTION REQUIRED: generate_candidate is auto-runnable. Read ${requestRef}, load requestManifest.refs.agentAction.ref when root agentAction is absent, use agentAction.read.fieldGroups inspect readCommands, write the requested candidate files, and run submitCommand now. Do not summarize progress or ask whether to continue.`);
  }
  if (mode === "submit_existing_candidate") {
    const requestRef = typeof instruction.requestRef === "string" ? instruction.requestRef : "instruction.requestRef";
    return withNoRecapGuard(`ACTION REQUIRED: submit_existing_candidate is auto-runnable. Read ${requestRef}, load requestManifest.refs.agentAction.ref when root agentAction is absent, use agentAction.read.fieldGroups inspect readCommands when request fields are needed, and run submitCommand now. Do not summarize progress or ask whether to continue.`);
  }
  if (typeof userVisibleMeaning?.summary === "string" && userVisibleMeaning.summary.length > 0) {
    return withNoRecapGuard(`ACTION REQUIRED: ${userVisibleMeaning.summary}`);
  }
  return withNoRecapGuard(`ACTION REQUIRED: ${mode} is auto-runnable. Follow top-level instruction now. Do not summarize progress or ask whether to continue.`);
}

function withNoRecapGuard(summary: string): string {
  if (summary.includes("Do not send a recap or progress summary")) {
    return summary;
  }
  return `${summary} Do not send a recap or progress summary; follow top-level instruction now.`;
}

function pickStringFields(source: Record<string, unknown>, keys: readonly string[]): Record<string, string> {
  const output: Record<string, string> = {};
  for (const key of keys) {
    if (typeof source[key] === "string") {
      output[key] = source[key];
    }
  }
  return output;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
