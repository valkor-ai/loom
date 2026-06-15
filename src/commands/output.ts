import type { CliEnvelope } from "./types";
import { agentFacingInstruction } from "./agent-facing-instruction";
import {
  compactJsonByteLength,
  prettyJsonByteLength,
  recordTokenSavingEventSync,
} from "../core/operations/token-saving-telemetry";

export function printEnvelope(envelope: CliEnvelope, options: { compact?: boolean } = {}): void {
  const payload = options.compact ? compactEnvelope(envelope) : envelope;
  const json = options.compact ? JSON.stringify(payload) : JSON.stringify(payload, null, 2);
  if (options.compact && shouldRecordCompactEnvelopeTelemetry(envelope)) {
    recordTokenSavingEventSync({
      projectRoot: envelope.projectRoot,
      source: "compact_envelope",
      command: envelope.command,
      fullBytes: prettyJsonByteLength(envelope),
      compactBytes: compactJsonByteLength(payload),
      metadata: {
        ok: envelope.ok,
      },
    });
  }
  process.stdout.write(`${json}\n`);
}

function shouldRecordCompactEnvelopeTelemetry(envelope: CliEnvelope): boolean {
  return envelope.command !== "status" && !envelope.command.startsWith("metrics.");
}

function compactEnvelope(envelope: CliEnvelope): Record<string, unknown> {
  if (!envelope.ok) {
    return {
      ok: false,
      command: envelope.command,
      version: envelope.version,
      projectRoot: envelope.projectRoot,
      error: envelope.error,
      summary: envelope.summary,
    };
  }

  return {
    ok: true,
    command: envelope.command,
    version: envelope.version,
    projectRoot: envelope.projectRoot,
    ...(envelope.agentProfile ? { agentProfile: envelope.agentProfile } : {}),
    ...(envelope.actionRequired ? { actionRequired: envelope.actionRequired } : {}),
    instruction: compactInstruction(envelope.instruction),
    data: envelope.command === "inspect" ? envelope.data : compactData(envelope.data),
    summary: envelope.summary,
    compact: true,
  };
}

function compactInstruction(instruction: unknown): Record<string, unknown> | null {
  return agentFacingInstruction(instruction) ?? null;
}

function compactData(data: unknown): unknown {
  if (!isRecord(data)) {
    return data;
  }
  const output: Record<string, unknown> = {};
  for (const key of [
    "initialized",
    "status",
    "completed",
    "prepared",
    "refreshed",
    "failedPhase",
    "recorded",
    "accepted",
    "hasTask",
    "hasRepairRequest",
    "reason",
    "activeDeliveryId",
    "activePhaseId",
    "deliveryStatus",
    "deliveryId",
    "phaseId",
    "phase",
    "currentRequirementId",
    "currentPlanId",
    "currentTaskId",
    "currentReviewId",
    "currentRepairId",
    "currentDeploymentId",
    "requestPath",
    "requestRef",
    "executionRequestPath",
    "resultPath",
    "resultFile",
    "candidateFile",
    "candidatePath",
    "reviewRequestPath",
    "deploymentId",
    "operationActive",
    "url",
    "provider",
    "providerReason",
    "serviceName",
    "appServiceName",
    "composePath",
    "specPath",
    "repairId",
    "failureKind",
    "failureRef",
    "fullLogRef",
    "failureOwner",
    "repairRoute",
    "nextAction",
    "effectiveNextAction",
    "editableFiles",
    "protectedFiles",
    "attempts",
    "maxAttempts",
    "truncated",
    "tokenSaving",
    "userGuidance",
    "warnings",
    "codeEvidenceReadGuide",
  ]) {
    if (key in data) {
      output[key] = data[key];
    }
  }
  if (isRecord(data.possibleRuntimeForegroundStall)) {
    output.possibleRuntimeForegroundStall = compactRuntimeForegroundStall(data.possibleRuntimeForegroundStall);
  }
  if (typeof data.instruction === "string") {
    output.instruction = data.instruction;
  }
  if (isRecord(data.nextAction)) {
    output.nextAction = compactNextAction(data.nextAction);
  }
  if (isRecord(data.effectiveNextAction)) {
    output.effectiveNextAction = compactNextAction(data.effectiveNextAction);
  }
  if (isRecord(data.summary)) {
    output.summary = pick(data.summary, [
      "appPath",
      "appServiceName",
      "url",
      "running",
      "healthStatus",
      "missingEnvCount",
      "bootstrapTaskCount",
      "hasRepairRequest",
    ]);
  }
  if (Array.isArray(data.lines)) {
    if (typeof data.fullLogRef === "string" && data.fullLogRef.length > 0) {
      output.lines = data.lines.slice(-40);
      output.linesOmitted = Math.max(0, data.lines.length - 40);
    } else {
      output.lines = data.lines;
    }
  }
  if (isRecord(data.task)) {
    output.task = pick(data.task, ["taskId", "groupId", "title", "taskKind", "acceptanceRefs"]);
  }
  if (isRecord(data.executionRequest)) {
    output.executionRequest = pick(data.executionRequest, [
      "requestId",
      "requestType",
      "taskId",
      "groupId",
      "title",
      "taskKind",
      "resultFile",
      "submitCommand",
      "requestRef",
    ]);
  }
  if (isRecord(data.repair)) {
    output.repair = {
      ...pick(data.repair, [
        "hasRepairRequest",
        "repairId",
        "failureKind",
        "failureOwner",
        "repairRoute",
        "failureRef",
        "fullLogRef",
        "nextAction",
        "editableFiles",
        "protectedFiles",
        "attempts",
        "maxAttempts",
        "instruction",
      ]),
      ...(isRecord(data.repair.errorWindow) ? { errorWindow: data.repair.errorWindow } : {}),
    };
  }
  if (isRecord(data.errorWindow)) {
    output.errorWindow = data.errorWindow;
  }
  if (isRecord(data.run)) {
    output.run = {
      ...pick(data.run, ["runId", "taskPlanId", "status", "summary", "updatedAt"]),
      nextAction: isRecord(data.run.nextAction) ? compactNextAction(data.run.nextAction) : data.run.nextAction,
      currentTask: isRecord(data.run.currentTask) ? pick(data.run.currentTask, ["taskId", "groupId", "status"]) : data.run.currentTask,
    };
  }
  if (Array.isArray(data.issues)) {
    output.issues = data.issues;
  }
  if (isRecord(data.repairInstruction)) {
    output.repairInstruction = compactDataRepairInstruction(data.repairInstruction);
  }
  if (Array.isArray(data.findings)) {
    output.findings = data.findings;
  }
  if (isRecord(data.activeOperation)) {
    output.activeOperation = compactActiveOperation(data.activeOperation);
  }
  if (isRecord(data.concurrency) && isRecord(data.concurrency.activeOperation)) {
    output.activeOperation = compactActiveOperation(data.concurrency.activeOperation);
    output.recoveryReason = data.concurrency.recoveryReason;
  }
  return Object.keys(output).length > 0 ? output : pick(data, ["schemaVersion", "summary"]);
}

function compactDataRepairInstruction(instruction: Record<string, unknown>): Record<string, unknown> {
  const output = pick(instruction, [
    "mode",
    "schema",
    "resultFile",
    "candidateFile",
    "repairContractProfile",
  ]);
  if (Array.isArray(instruction.issues)) {
    output.issueCount = instruction.issues.length;
  }
  if (Array.isArray(instruction.issueConflicts)) {
    output.issueConflictCount = instruction.issueConflicts.length;
  }
  output.fullInstructionLocation = "top-level instruction";
  return output;
}

function compactNextAction(action: Record<string, unknown>): Record<string, unknown> {
  return pick(action, ["type", "source", "deliveryId", "phaseId", "reason", "targetNode", "sourceTaskId", "ref", "refs"]);
}

function compactActiveOperation(operation: Record<string, unknown>): Record<string, unknown> {
  const output = pick(operation, [
    "operationActive",
    "operationId",
    "command",
    "phase",
    "operationType",
    "phaseId",
    "status",
    "startedAt",
    "updatedAt",
    "elapsedMs",
    "logRef",
    "activeOperationRef",
    "allowedCommands",
    "forbiddenActions",
    "expiresAt",
    "stale",
    "progressSignal",
    "progress",
    "resumeCommand",
    "refs",
  ]);
  if (isRecord(operation.possibleRuntimeForegroundStall)) {
    output.possibleRuntimeForegroundStall = compactRuntimeForegroundStall(operation.possibleRuntimeForegroundStall);
  }
  return output;
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

function pick(source: Record<string, unknown>, keys: string[]): Record<string, unknown> {
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
