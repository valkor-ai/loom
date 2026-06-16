import type { AgentProfileId } from "./agent-profile";
import { loomCommandInvocation } from "./command-invocation";

export function agentFacingInstruction(instruction: unknown): Record<string, unknown> | undefined {
  if (!isRecord(instruction)) {
    return undefined;
  }

  const output: Record<string, unknown> = {};
  const autoRunnable = isAutoRunnableInstruction(instruction);
  for (const key of ROUTING_KEYS) {
    if (key === "outputSummary" && isArchitectureSingleSectionInstruction(instruction)) {
      continue;
    }
    if (key === "stopRecoveryInstruction" && autoRunnable) {
      continue;
    }
    if (key === "possibleRuntimeForegroundStall" && isRecord(instruction[key])) {
      output[key] = compactRuntimeForegroundStall(instruction[key]);
      continue;
    }
    if (key in instruction) {
      output[key] = instruction[key];
    }
  }

  if (isRecord(instruction.task)) {
    output.task = pick(instruction.task, ["taskId", "groupId", "title", "taskKind", "acceptanceRefs"]);
  }
  if (isRecord(instruction.command)) {
    output.command = pick(instruction.command, ["name", "argv", "commandInvocation"]);
  }
  if (isRecord(instruction.retryCommand)) {
    output.retryCommand = pick(instruction.retryCommand, ["name", "argv", "commandInvocation"]);
  }
  if (isRecord(instruction.submitCommand)) {
    output.submitCommand = pick(instruction.submitCommand, ["name", "argv", "argvTemplate", "commandInvocation"]);
  }
  if (isRecord(instruction.completionBarrier)) {
    output.completionBarrier = pick(instruction.completionBarrier, [
      "resultFile",
      "targetCandidateFile",
      "followUpCommand",
    ]);
  }
  if (isRecord(instruction.expectedResponse)) {
    output.expectedResponse = compactExpectedResponse(instruction.expectedResponse);
  }
  if (isRecord(instruction.requestReadProtocol)) {
    output.requestReadProtocol = pick(instruction.requestReadProtocol, [
      "authority",
      "firstSelector",
      "readRule",
      "nullFieldRule",
      "unlistedSidecarRule",
    ]);
  }
  if (isRecord(instruction.continuationContract)) {
    output.continuationContract = compactContinuationContract(instruction.continuationContract);
  }

  return Object.keys(output).length > 0 ? output : undefined;
}

export function withAgentCommandInvocations(
  instruction: Record<string, unknown> | undefined,
  profile: AgentProfileId,
  projectRoot: string,
): Record<string, unknown> | undefined {
  if (!instruction) {
    return undefined;
  }
  const output = { ...instruction };
  if (isAutoRunnableInstruction(output)) {
    delete output.stopRecoveryInstruction;
  }
  if (isRecord(output.command)) {
    const command = withNestedInvocation(output.command, profile, projectRoot);
    output.command = command;
    const invocation = loomCommandInvocation(profile, command.argv, projectRoot);
    if (invocation) {
      output.commandInvocation = invocation;
    }
  }
  if (isRecord(output.submitCommand)) {
    output.submitCommand = withNestedInvocation(output.submitCommand, profile, projectRoot);
  }
  if (isRecord(output.retryCommand)) {
    output.retryCommand = withNestedInvocation(output.retryCommand, profile, projectRoot);
  }
  if (isRecord(output.completionBarrier) && isRecord(output.completionBarrier.submitCommand)) {
    output.completionBarrier = {
      ...output.completionBarrier,
      submitCommand: withNestedInvocation(output.completionBarrier.submitCommand, profile, projectRoot),
    };
  }
  if (isRecord(output.completionBarrier) && isRecord(output.completionBarrier.followUpCommand)) {
    output.completionBarrier = {
      ...output.completionBarrier,
      followUpCommand: withNestedInvocation(output.completionBarrier.followUpCommand, profile, projectRoot),
    };
  }
  if (isRecord(output.expectedResponse)) {
    output.expectedResponse = withExpectedResponseInvocations(output.expectedResponse, profile, projectRoot);
  }
  return output;
}

function isAutoRunnableInstruction(instruction: Record<string, unknown>): boolean {
  return (
    instruction.autoContinue === true ||
    instruction.mustRunImmediately === true ||
    instruction.mustStartImmediately === true ||
    instruction.mustNotAskUserBeforeRunning === true ||
    instruction.mustNotAskUserBeforeExecuting === true
  );
}

const ROUTING_KEYS = [
  "mode",
  "autoContinue",
  "mustRunImmediately",
  "mustStartImmediately",
  "mustNotAskUserBeforeRunning",
  "mustNotAskUserBeforeExecuting",
  "mustNotReportProgressBeforeExecuting",
  "stopAfterCommand",
  "commandInvocation",
  "requestRef",
  "candidateKind",
  "candidateFile",
  "resultFile",
  "outputSummary",
  "repairId",
  "failureKind",
  "failureOwner",
  "repairRoute",
  "provider",
  "fullLogRef",
  "errorWindow",
  "diagnostics",
  "suggestedActions",
  "editableFiles",
  "protectedFiles",
  "attempts",
  "maxAttempts",
  "repairBoundary",
  "repairInputs",
  "retryCommand",
  "existingOutputs",
  "sectionGenerationMode",
  "targetSection",
  "targetCandidateFile",
  "recovery",
  "requestAlreadyExists",
  "nextAction",
  "acceptedShortReplies",
  "advisories",
  "instruction",
  "routingRule",
  "userMessage",
  "primaryAction",
  "observationPolicy",
  "completionCondition",
  "possibleRuntimeForegroundStall",
  "stopRecoveryInstruction",
  "finalResponseGuard",
  "requestReadProtocol",
  "schema",
  "issues",
  "fieldRepairPlan",
  "schemaShape",
  "repairContractProfile",
  "issueConflicts",
  "minimalRepairRules",
  "inspectRepairContractCommand",
  "resultRules",
  "allowedVerificationResults",
  "allowedRuntimeCodeLevelChecks",
  "allowedRuntimeCheckedFields",
  "repairSubmitRouting",
  "instructions",
] as const;

function compactExpectedResponse(expectedResponse: Record<string, unknown>): Record<string, unknown> {
  const output = pick(expectedResponse, [
    "kind",
    "rule",
    "successRule",
    "retryRule",
    "requestRef",
    "candidateFile",
    "resultFile",
    "requestReadRule",
    "currentTurnAnswerRule",
  ]);
  if (isRecord(expectedResponse.submitCommand)) {
    output.submitCommand = pick(expectedResponse.submitCommand, ["name", "argv", "argvTemplate", "commandInvocation"]);
  }
  if (isRecord(expectedResponse.acceptCommand)) {
    output.acceptCommand = pick(expectedResponse.acceptCommand, ["name", "argv", "argvTemplate", "commandInvocation"]);
  }
  return output;
}

function withExpectedResponseInvocations(
  expectedResponse: Record<string, unknown>,
  profile: AgentProfileId,
  projectRoot: string,
): Record<string, unknown> {
  return {
    ...expectedResponse,
    ...(isRecord(expectedResponse.submitCommand)
      ? { submitCommand: withNestedInvocation(expectedResponse.submitCommand, profile, projectRoot) }
      : {}),
    ...(isRecord(expectedResponse.acceptCommand)
      ? { acceptCommand: withNestedInvocation(expectedResponse.acceptCommand, profile, projectRoot) }
      : {}),
  };
}

function compactContinuationContract(contract: Record<string, unknown>): Record<string, unknown> {
  const output: Record<string, unknown> = pick(contract, ["kind"]);
  if (isRecord(contract.source)) {
    output.source = pick(contract.source, ["command", "succeeded", "summary"]);
  }
  if (isRecord(contract.next)) {
    const next = pick(contract.next, [
      "instructionMode",
      "requestRef",
      "resultFile",
      "candidateFile",
      "targetCandidateFile",
    ]);
    if (isRecord(contract.next.command)) {
      next.command = compactCommand(contract.next.command);
    }
    if (isRecord(contract.next.retryCommand)) {
      next.retryCommand = compactCommand(contract.next.retryCommand);
    }
    if (isRecord(contract.next.submitCommand)) {
      next.submitCommand = compactCommand(contract.next.submitCommand);
    }
    output.next = next;
  }
  if (isRecord(contract.agentObligation)) {
    output.agentObligation = pick(contract.agentObligation, [
      "primaryAction",
      "mustBeginWithoutUserInput",
      "mustFollowReturnedInstruction",
      "inputRefs",
      "outputRefs",
      "completionCondition",
    ]);
  }
  if (isRecord(contract.userVisibleMeaning)) {
    output.userVisibleMeaning = pick(contract.userVisibleMeaning, ["summary", "notAStoppingPoint"]);
  }
  output.readMore = "Use actionRequired.requiredSteps, actionRequired.forbiddenStops, actionRequired.stopOnlyWhen, and requestRef for full execution details.";
  return output;
}

function compactCommand(command: Record<string, unknown>): Record<string, unknown> {
  return pick(command, ["name", "argv", "argvTemplate", "commandInvocation"]);
}

function compactRuntimeForegroundStall(stall: Record<string, unknown>): Record<string, unknown> {
  const output = pick(stall, ["applies", "reason", "confidence", "userSummary"]);
  if (isRecord(stall.agentInstruction)) {
    output.agentInstruction = pick(stall.agentInstruction, ["primaryAction"]);
  }
  if (isRecord(stall.evidence)) {
    output.evidence = pick(stall.evidence, ["activeOperationType", "resultFile", "resultFileStatus", "processScanPerformed"]);
  }
  return output;
}

function withNestedInvocation(command: Record<string, unknown>, profile: AgentProfileId, projectRoot: string): Record<string, unknown> {
  const invocation = loomCommandInvocation(profile, command.argv, projectRoot);
  return invocation ? { ...command, commandInvocation: invocation } : command;
}

function pick(source: Record<string, unknown>, keys: readonly string[]): Record<string, unknown> {
  const output: Record<string, unknown> = {};
  for (const key of keys) {
    if (key in source) {
      output[key] = source[key];
    }
  }
  return output;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isArchitectureSingleSectionInstruction(instruction: Record<string, unknown>): boolean {
  return instruction.candidateKind === "ArchitectureSections" && instruction.sectionGenerationMode === "single_section";
}
