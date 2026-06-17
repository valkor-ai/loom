import type { RouteAction } from "../schemas";
import { architectureSingleSectionRequiredSteps } from "./architecture-section-completion";
import { completedDeliveryUserMessage } from "./user-guidance";

type SubmitOrRunCommand = {
  name?: unknown;
  argv?: unknown;
  argvTemplate?: unknown;
};

type AutoRunnableTransitionInput = {
  sourceCommand: string;
  sourceSucceeded?: boolean;
  sourceSummary: string;
  primaryAction?: string;
  requiredSteps?: string[];
  forbiddenStops?: string[];
  stopOnlyWhen?: string[];
  userVisibleSummary?: string;
  completionCondition?: string;
  mustStartImmediately?: boolean;
};

export function withAutoRunnableTransition<T extends Record<string, unknown>>(
  instruction: T,
  input: AutoRunnableTransitionInput,
): T {
  const next = nextInstructionDescriptor(instruction);
  const obligation = agentObligationForInstruction(instruction, input);
  return {
    ...instruction,
    autoContinue: true,
    mustRunImmediately: true,
    ...(input.mustStartImmediately ? { mustStartImmediately: true } : {}),
    mustNotAskUserBeforeRunning: true,
    mustNotAskUserBeforeExecuting: true,
    mustNotReportProgressBeforeExecuting: true,
    stopAfterCommand: instruction.stopAfterCommand ?? false,
    continuationContract: {
      kind: "auto_runnable_transition",
      source: {
        command: input.sourceCommand,
        succeeded: input.sourceSucceeded ?? true,
        summary: input.sourceSummary,
      },
      next,
      agentObligation: obligation,
      userVisibleMeaning: {
        summary: input.userVisibleSummary ?? `${input.sourceSummary} Continue with ${String(instruction.mode ?? "the returned instruction")} now.`,
        notAStoppingPoint: true,
      },
    },
  };
}

function nextInstructionDescriptor(instruction: Record<string, unknown>): Record<string, unknown> {
  const next: Record<string, unknown> = {
    instructionMode: typeof instruction.mode === "string" ? instruction.mode : "unknown",
  };
  for (const key of ["requestRef", "candidateFile", "resultFile", "targetCandidateFile"] as const) {
    if (typeof instruction[key] === "string") {
      next[key] = instruction[key];
    }
  }
  if (isCommand(instruction.command)) {
    next.command = instruction.command;
  }
  if (isCommand(instruction.retryCommand)) {
    next.retryCommand = instruction.retryCommand;
  }
  if (isCommand(instruction.submitCommand)) {
    next.submitCommand = instruction.submitCommand;
  }
  if (isRecord(instruction.outputSummary) && !isArchitectureSingleSectionInstruction(instruction)) {
    next.outputSummary = instruction.outputSummary;
  }
  return next;
}

function agentObligationForInstruction(
  instruction: Record<string, unknown>,
  input: AutoRunnableTransitionInput,
): Record<string, unknown> {
  const mode = typeof instruction.mode === "string" ? instruction.mode : "unknown";
  return {
    primaryAction: input.primaryAction ?? primaryActionForMode(mode),
    mustBeginWithoutUserInput: true,
    mustFollowReturnedInstruction: true,
    inputRefs: inputRefsForInstruction(instruction),
    outputRefs: outputRefsForInstruction(instruction),
    completionCondition: input.completionCondition ?? completionConditionForMode(mode),
    requiredSteps: input.requiredSteps ?? requiredStepsForInstruction(instruction),
    forbiddenStops: input.forbiddenStops ?? defaultForbiddenStops(mode),
    stopOnlyWhen: input.stopOnlyWhen ?? defaultStopOnlyWhen(mode),
  };
}

function inputRefsForInstruction(instruction: Record<string, unknown>): string[] {
  const refs = new Set<string>();
  if (typeof instruction.requestRef === "string") refs.add(instruction.requestRef);
  if (isRecord(instruction.nextAction) && typeof instruction.nextAction.ref === "string") refs.add(instruction.nextAction.ref);
  return [...refs];
}

function outputRefsForInstruction(instruction: Record<string, unknown>): string[] {
  const refs = new Set<string>();
  for (const key of ["resultFile", "candidateFile", "targetCandidateFile"] as const) {
    if (typeof instruction[key] === "string") refs.add(instruction[key]);
  }
  if (Array.isArray(instruction.editableFiles)) {
    for (const item of instruction.editableFiles) {
      if (typeof item === "string") refs.add(item);
    }
  }
  const outputSummary = isRecord(instruction.outputSummary) ? instruction.outputSummary : {};
  for (const key of ["outlineFile", "groupFilePattern"] as const) {
    if (typeof outputSummary[key] === "string") refs.add(outputSummary[key]);
  }
  return [...refs];
}

function requiredStepsForInstruction(instruction: Record<string, unknown>): string[] {
  const mode = typeof instruction.mode === "string" ? instruction.mode : "unknown";
  if (mode === "generate_candidate" && isArchitectureSingleSectionInstruction(instruction)) {
    return architectureSingleSectionRequiredSteps();
  }
  if (mode === "run_cli") {
    return [
      "run instruction.command.argv with loom in the same project root",
      "read the returned CLI envelope",
      "immediately follow the returned instruction when it is auto-runnable",
    ];
  }
  if (mode === "execute_task") {
    return [
      "read instruction.requestRef and its root requestReadPlan when present",
      "use requestReadPlan.groups inspect commands for required TaskExecutionRequest fields",
      "if requestReadPlan is absent, use agentAction.read.fieldGroups inspect commands as compatibility fallback",
      "execute exactly that TaskExecutionRequest",
      "write instruction.resultFile",
      "run instruction.submitCommand",
      "immediately follow the returned instruction when it is auto-runnable",
    ];
  }
  if (mode === "submit_existing_candidate") {
    return [
      "read instruction.requestRef and its root requestReadPlan when request fields are needed",
      "use requestReadPlan groups first; if absent, use agentAction.read.fieldGroups as compatibility fallback",
      "verify the existing candidate/result files referenced by the instruction",
      "run instruction.submitCommand",
      "immediately follow the returned instruction when it is auto-runnable",
    ];
  }
  if (mode === "repair_candidate" || mode === "repair_result_contract") {
    return [
      "read instruction.requestRef and its root requestReadPlan when present",
      "use requestReadPlan groups first; if absent, use agentAction.read.fieldGroups as compatibility fallback",
      "inspect instruction.issues",
      "repair only the referenced candidate/result artifact",
      "run instruction.submitCommand",
      "immediately follow the returned instruction when it is auto-runnable",
    ];
  }
  return [
    "read instruction.requestRef when present",
    "produce the requested output refs",
    "run instruction.submitCommand when present",
    "immediately follow the returned instruction when it is auto-runnable",
  ];
}

function primaryActionForMode(mode: string): string {
  if (mode === "run_cli") return "run_returned_cli_command";
  if (mode === "deploy_repair_assets") return "repair_deployment_assets_and_rerun_deploy";
  if (mode === "execute_task") return "execute_task_request_and_submit_result";
  if (mode === "submit_existing_candidate") return "submit_existing_candidate";
  if (mode === "repair_candidate") return "repair_candidate_and_submit";
  if (mode === "repair_result_contract") return "repair_task_result_contract_and_submit";
  return "follow_returned_instruction";
}

function completionConditionForMode(mode: string): string {
  if (mode === "run_cli") return "instruction.command has run and its returned instruction has been followed when auto-runnable.";
  if (mode === "deploy_repair_assets") return "instruction.editableFiles have been repaired and instruction.retryCommand has succeeded, or the returned deploy repair instruction has been followed.";
  if (mode === "execute_task") return "instruction.resultFile exists and instruction.submitCommand has succeeded.";
  if (mode === "submit_existing_candidate") return "instruction.submitCommand has succeeded for the existing candidate/result files.";
  if (mode === "repair_candidate" || mode === "repair_result_contract") return "the repaired artifact has been submitted successfully.";
  return "the requested output refs exist and the submit command has succeeded when present.";
}

function defaultForbiddenStops(mode: string): string[] {
  const stops = [
    "do not stop with a progress summary",
    "do not stop with a recap",
    "do not ask whether to continue",
    "do not replace the required action with a user-facing recovery hint",
  ];
  if (mode === "execute_task") {
    stops.push("do not stop after updating an internal todo/task list before TaskResult submit succeeds");
    stops.push("do not run loom continue or next-task before submitting the already-created TaskResult");
  }
  return stops;
}

function defaultStopOnlyWhen(mode: string): string[] {
  const stops = [
    "the returned instruction is ask_user, manual_review, needs_user_decision, report_blocked, or report_done",
    "the command returns a non-repairable failure",
  ];
  if (mode !== "run_cli") {
    stops.unshift("the required requestRef cannot be read");
  }
  return stops;
}

function isCommand(value: unknown): value is SubmitOrRunCommand {
  return isRecord(value) && (typeof value.name === "string" || Array.isArray(value.argv) || Array.isArray(value.argvTemplate));
}

function isArchitectureSingleSectionInstruction(instruction: Record<string, unknown>): boolean {
  return instruction.candidateKind === "ArchitectureSections" && instruction.sectionGenerationMode === "single_section";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function autoRunInstruction(input: {
  actionType: string;
  deliveryId: string;
  phaseId: string;
  reason: string;
  argv: string[];
  userMessage: string;
  targetNode?: string;
  ref?: string | null;
  refs?: Record<string, unknown>;
}): Record<string, unknown> {
  return withAutoRunnableTransition({
    mode: "run_cli",
    nextAction: {
      type: input.actionType,
      reason: input.reason,
      targetNode: input.targetNode ?? input.actionType,
      ref: input.ref ?? null,
      refs: input.refs,
    },
    command: {
      name: input.actionType,
      argv: input.argv,
    },
    routingRule: "Run this command immediately. After it finishes, follow the command response instruction or run loom continue. Do not ask whether to continue while the next action is auto-runnable.",
    userMessage: input.userMessage,
  }, {
    sourceCommand: "route",
    sourceSummary: input.reason,
    primaryAction: "run_next_loom_command",
  });
}

export function instructionForRouteAction(action: RouteAction, fallback: {
  deliveryId: string;
  phaseId: string;
}): Record<string, unknown> | undefined {
  const deliveryId = action.deliveryId ?? fallback.deliveryId;
  const phaseId = action.phaseId ?? fallback.phaseId;
  const type = action.type;
  const commandByAction: Partial<Record<RouteAction["type"], string[]>> = {
    technical_baseline_request: ["technical-baseline", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    repository_context_request: ["repository-context", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    planning_contract_create: ["planning-contract", "create", "--delivery-id", deliveryId, "--phase-id", phaseId],
    architecture_artifact_contract: ["architecture", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    taskplan_generation: ["task-plan", "request", "--delivery-id", deliveryId, "--phase-id", phaseId],
    continue_execution: ["next-task", "--delivery-id", deliveryId, "--phase-id", phaseId],
    review: ["review", "--delivery-id", deliveryId, "--phase-id", phaseId],
    execution_repair: ["repair", "request", "--type", "execution", "--delivery-id", deliveryId, "--phase-id", phaseId],
    task_result_repair: ["repair", "request", "--type", "task-result", "--delivery-id", deliveryId, "--phase-id", phaseId],
    taskplan_repair: ["repair", "request", "--type", "taskplan", "--delivery-id", deliveryId, "--phase-id", phaseId],
    architecture_artifact_repair: ["repair", "request", "--type", "architecture", "--delivery-id", deliveryId, "--phase-id", phaseId],
    continue_to_next_phase: ["continue"],
  };
  const argv = commandByAction[type];
  if (!argv) {
    if (type === "done") {
      return {
        mode: "report_done",
        autoContinue: false,
        nextAction: action,
        userMessage: completedDeliveryUserMessage(),
      };
    }
    if (type === "needs_user_decision" || type === "manual_review" || type === "brainstorm_clarification" || type === "brainstorm_confirmation") {
      return {
        mode: "ask_user",
        autoContinue: false,
        nextAction: action,
        command: null,
        userMessage: "当前需要用户输入后才能继续。",
      };
    }
    return undefined;
  }
  return autoRunInstruction({
    actionType: type,
    deliveryId,
    phaseId,
    reason: action.reason ?? "NEXT_ACTION_READY",
    targetNode: action.targetNode,
    ref: action.ref,
    refs: action.refs,
    argv,
    userMessage: "提交成功。请立即执行下一步 loom 命令。",
  });
}

export function postRepairSubmitRouting(input: {
  source: string;
  submitCommand: string;
  nextActionTypes: string[];
  repairedArtifact?: Record<string, unknown>;
}): Record<string, unknown> {
  return {
    source: input.source,
    submitCommand: input.submitCommand,
    repairedSubmitSucceeded: true,
    followReturnedInstructionImmediately: true,
    doNotRunContinueBeforeReturnedInstruction: true,
    doNotAskUserAfterSuccessfulSubmit: true,
    continueAutomaticallyWhenNextActionIs: input.nextActionTypes,
    repairedArtifact: input.repairedArtifact ?? null,
    rule: "This submit succeeded after candidate/result repair. Follow data.instruction or data.routeDecision immediately; do not ask whether to continue while the next action is auto-runnable.",
  };
}
