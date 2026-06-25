import { promises as fs } from "node:fs";
import path from "node:path";
import { ZodError } from "zod";
import { type OperationLease } from "../contracts";
import { LoomError, stateCorrupted, stateNotInitialized } from "../errors";
import { loomConfigV1Schema, loomStatusV1Schema, type LoomStatusV1 } from "../schemas";
import { pathExists, readJsonFile, readJsonWithSchemaVersion } from "../state/fs";
import { loadDeliveryIndex, upsertStatusDelivery } from "../state/delivery";
import { getLoomPaths } from "../state/paths";
import { type OperationProgressSignal, progressSignalForOperation, readOperationLease } from "./control";
import { hydrateRequestManifest } from "./request-manifest";
import { type PossibleRuntimeForegroundStall, possibleRuntimeForegroundStall } from "./runtime-stall";
import {
  projectRelativeTelemetryPath,
  readTokenSavingSummary,
  type TokenSavingSummary,
} from "./token-saving-telemetry";
import { completedDeliveryUserMessage, type LoomCommandSurface } from "./user-guidance";

export type GetStatusInput = {
  projectRoot: string;
  commandSurface?: LoomCommandSurface;
};

export type GetStatusResult = {
  initialized: true;
  activeDeliveryId: string | null;
  activePhaseId: string | null;
  deliveryStatus: string | null;
  phase: LoomStatusV1["phase"];
  currentRequirementId: string | null;
  currentPlanId: string | null;
  currentTaskId: string | null;
  currentReviewId: string | null;
  currentRepairId: string | null;
  currentDeploymentId: string | null;
  nextAction: LoomStatusV1["nextAction"];
  effectiveNextAction: LoomStatusV1["effectiveNextAction"] | null;
  activeOperation: ActiveOperationStatus | null;
  tokenSaving: TokenSavingStatus | null;
  userGuidance: string | null;
  warnings: string[];
};

type TokenSavingStatus = {
  telemetryRef: string;
  updatedAt: string;
  eventCount: number;
  bytesAvoided: number;
  estimatedTokensSaved: number;
  bySource: TokenSavingSummary["totals"]["bySource"];
};

type ActiveOperationStatus = {
  operationId: string;
  operationType: string;
  phaseId: string;
  status: string;
  startedAt: string;
  expiresAt: string;
  stale: boolean;
  progressSignal: OperationProgressSignal;
  progress: Record<string, unknown> | null;
  possibleRuntimeForegroundStall?: PossibleRuntimeForegroundStall;
  resumeCommand: {
    name: "continue";
    argv: ["continue"];
    userCommand: "@loom continue" | "/loom continue";
    guidance: string;
  };
};

export async function getStatus(input: GetStatusInput): Promise<GetStatusResult> {
  const paths = getLoomPaths(input.projectRoot);
  const warnings: string[] = [];

  try {
    await fs.access(paths.configFile);
    await fs.access(paths.statusFile);
  } catch {
    throw stateNotInitialized(paths.root);
  }

  const configJson = await readJsonWithSchemaVersion(paths.configFile);
  const statusJson = await readJsonWithSchemaVersion(paths.statusFile);

  try {
    const config = loomConfigV1Schema.parse(configJson);
    let status = loomStatusV1Schema.parse(statusJson);
    if (status.activeDeliveryId) {
      try {
        const index = await loadDeliveryIndex(paths.root, status.activeDeliveryId);
        await upsertStatusDelivery(paths.root, index);
        status = loomStatusV1Schema.parse(await readJsonWithSchemaVersion(paths.statusFile));
      } catch (error) {
        if (error instanceof LoomError) {
          throw error;
        }
        warnings.push("Active delivery index could not be used to refresh status.");
      }
    }

    if (config.project.createdAtRoot !== paths.root) {
      warnings.push("Project root differs from config.project.createdAtRoot.");
    }

    const commandSurface = input.commandSurface ?? "@loom";
    const activeOperation = status.activeDeliveryId
      ? await activeOperationStatus(paths.root, status.activeDeliveryId, commandSurface)
      : null;
    const tokenSaving = await tokenSavingStatus(paths.root);
    const userGuidance = activeOperation?.resumeCommand.guidance
      ?? (!status.activeDeliveryId && status.phase === "completed" ? completedDeliveryUserMessage(commandSurface) : null);

    return {
      initialized: true,
      activeDeliveryId: status.activeDeliveryId ?? null,
      activePhaseId: status.deliveries?.find((delivery) => delivery.deliveryId === status.activeDeliveryId)?.activePhaseId ?? null,
      deliveryStatus: status.deliveries?.find((delivery) => delivery.deliveryId === status.activeDeliveryId)?.status ?? null,
      phase: status.phase,
      currentRequirementId: status.current.requirementId,
      currentPlanId: status.current.planId,
      currentTaskId: status.current.taskId,
      currentReviewId: status.current.reviewId,
      currentRepairId: status.current.repairId,
      currentDeploymentId: status.current.deploymentId,
      nextAction: status.nextAction,
      effectiveNextAction: status.effectiveNextAction ?? null,
      activeOperation,
      tokenSaving,
      userGuidance,
      warnings,
    };
  } catch (error) {
    if (error instanceof LoomError) {
      throw error;
    }
    if (error instanceof ZodError) {
      throw stateCorrupted("loom state file does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

async function tokenSavingStatus(projectRoot: string): Promise<TokenSavingStatus | null> {
  const telemetry = await readTokenSavingSummary(projectRoot);
  if (!telemetry || telemetry.totals.eventCount === 0) {
    return null;
  }
  return {
    telemetryRef: projectRelativeTelemetryPath(projectRoot),
    updatedAt: telemetry.updatedAt,
    eventCount: telemetry.totals.eventCount,
    bytesAvoided: telemetry.totals.bytesAvoided,
    estimatedTokensSaved: telemetry.totals.estimatedTokensSaved,
    bySource: telemetry.totals.bySource,
  };
}

async function activeOperationStatus(
  projectRoot: string,
  deliveryId: string,
  commandSurface: LoomCommandSurface,
): Promise<ActiveOperationStatus | null> {
  const lease = await readOperationLease(projectRoot, deliveryId);
  if (!lease || lease.status !== "active") {
    return null;
  }
  const progress = await progressForLease(projectRoot, lease);
  const runtimeStall = await possibleRuntimeForegroundStallForLease(projectRoot, lease);
  const continueCommand = `${commandSurface} continue` as "@loom continue" | "/loom continue";
  return {
    operationId: lease.operationId,
    operationType: lease.operationType,
    phaseId: lease.phaseId,
    status: lease.status,
    startedAt: lease.startedAt,
    expiresAt: lease.expiresAt,
    stale: new Date(lease.expiresAt).getTime() <= Date.now(),
    progressSignal: progressSignalForOperation(lease.operationType),
    progress,
    ...(runtimeStall ? { possibleRuntimeForegroundStall: runtimeStall } : {}),
    resumeCommand: {
      name: "continue",
      argv: ["continue"],
      userCommand: continueCommand,
      guidance: runtimeStall
        ? `Run ${continueCommand} in the Agent chat. The task result is missing and the task may be waiting on a local runtime command that will not exit by itself; the Agent should finish the probe, record cleanup state, and submit TaskResult.`
        : `Run ${continueCommand} in the Agent chat to resume this active loom operation.`,
    },
  };
}

async function possibleRuntimeForegroundStallForLease(
  projectRoot: string,
  lease: OperationLease,
): Promise<PossibleRuntimeForegroundStall | null> {
  const requestRef = typeof lease.refs.requestRef === "string" ? lease.refs.requestRef : null;
  const request = requestRef && await pathExists(path.join(projectRoot, requestRef))
    ? await hydrateRequestManifest(projectRoot, path.join(projectRoot, requestRef))
    : null;
  return possibleRuntimeForegroundStall({
    projectRoot,
    lease,
    request,
    resultFile: typeof lease.refs.resultFile === "string" ? lease.refs.resultFile : null,
  });
}

async function progressForLease(projectRoot: string, lease: OperationLease): Promise<Record<string, unknown> | null> {
  const requestRef = typeof lease.refs.requestRef === "string" ? lease.refs.requestRef : null;
  const request = requestRef && await pathExists(path.join(projectRoot, requestRef))
    ? await hydrateRequestManifest(projectRoot, path.join(projectRoot, requestRef))
    : null;
  if (lease.operationType === "architecture_generation") {
    const outputContract = request && typeof request === "object" && typeof (request as { outputContract?: unknown }).outputContract === "object" && (request as { outputContract?: unknown }).outputContract !== null
      ? (request as { outputContract: { sectionOutputs?: unknown } }).outputContract
      : null;
    const sectionOutputs = Array.isArray(outputContract?.sectionOutputs)
      ? outputContract.sectionOutputs as Array<{ section?: unknown; candidateFile?: unknown }>
      : Array.isArray(lease.refs.sectionOutputs)
        ? lease.refs.sectionOutputs as Array<{ section?: unknown; candidateFile?: unknown }>
        : [];
    const files = [];
    const completed = [];
    const missing = [];
    for (const output of sectionOutputs) {
      const section = typeof output.section === "string" ? output.section : null;
      const candidateFile = typeof output.candidateFile === "string" ? output.candidateFile : null;
      const stat = candidateFile ? await statOrNull(path.join(projectRoot, candidateFile)) : null;
      if (section) {
        if (stat) completed.push(section);
        else missing.push(section);
      }
      files.push({
        section,
        candidateFile,
        status: stat ? "written" : "missing",
        updatedAt: stat?.mtime.toISOString() ?? null,
      });
    }
    return { completedSections: completed, missingSections: missing, files };
  }
  if (lease.operationType === "taskplan_generation") {
    return taskPlanProgress(projectRoot, request);
  }
  const candidateFile = typeof lease.refs.candidateFile === "string"
    ? lease.refs.candidateFile
    : typeof lease.refs.resultFile === "string"
      ? lease.refs.resultFile
      : null;
  if (!candidateFile) {
    return null;
  }
  const stat = await statOrNull(path.join(projectRoot, candidateFile));
  return {
    candidateFile,
    status: stat ? "written" : "missing",
    updatedAt: stat?.mtime.toISOString() ?? null,
  };
}

async function taskPlanProgress(projectRoot: string, request: unknown): Promise<Record<string, unknown>> {
  const outputContract = request && typeof request === "object"
    ? (request as { outputContract?: { outlineFile?: unknown; groupFilePattern?: unknown } }).outputContract
    : undefined;
  const outlineFile = typeof outputContract?.outlineFile === "string" ? outputContract.outlineFile : null;
  const outlineStat = outlineFile ? await statOrNull(path.join(projectRoot, outlineFile)) : null;
  if (!outlineFile || !outlineStat) {
    return {
      progressSignal: "candidate_files",
      complete: false,
      outline: { candidateFile: outlineFile, status: "missing", updatedAt: null },
      groupTotal: 0,
      completedGroupCount: 0,
      completedGroups: [],
      missingGroups: [],
      groups: [],
      recommendedAction: "write_outline",
      summary: "TaskPlan outline is missing. Generate outline first, then generate group files.",
    };
  }
  const outline = await readJsonFile(path.join(projectRoot, outlineFile)).catch(() => null);
  if (!outline) {
    return {
      progressSignal: "candidate_files",
      complete: false,
      outline: { candidateFile: outlineFile, status: "invalid", updatedAt: outlineStat.mtime.toISOString() },
      groupTotal: 0,
      completedGroupCount: 0,
      completedGroups: [],
      missingGroups: [],
      groups: [],
      recommendedAction: "repair_outline",
      summary: "TaskPlan outline exists but cannot be read as JSON. Repair outline before generating groups.",
    };
  }
  const outlineGroups = outline && typeof outline === "object" && Array.isArray((outline as { groups?: unknown }).groups)
    ? (outline as { groups: Array<{ groupId?: unknown }> }).groups
    : [];
  const completedGroups = [];
  const missingGroups = [];
  const groups = [];
  for (const group of outlineGroups) {
    if (typeof group.groupId !== "string") continue;
    const candidateFile = typeof outputContract?.groupFilePattern === "string"
      ? outputContract.groupFilePattern.replace("{groupId}", group.groupId)
      : null;
    const stat = candidateFile ? await statOrNull(path.join(projectRoot, candidateFile)) : null;
    if (stat) completedGroups.push(group.groupId);
    else missingGroups.push(group.groupId);
    groups.push({
      groupId: group.groupId,
      candidateFile,
      status: stat ? "written" : "missing",
      updatedAt: stat?.mtime.toISOString() ?? null,
    });
  }
  const complete = outlineGroups.length > 0 && missingGroups.length === 0;
  return {
    progressSignal: "candidate_files",
    complete,
    outline: { candidateFile: outlineFile, status: "written", updatedAt: outlineStat.mtime.toISOString() },
    groupTotal: outlineGroups.length,
    completedGroupCount: completedGroups.length,
    completedGroups,
    missingGroups,
    groups,
    recommendedAction: complete ? "submit_accept" : "write_missing_groups",
    summary: complete
      ? `TaskPlan outline exists and all ${outlineGroups.length} group file(s) are written. Submit task-plan accept.`
      : `TaskPlan outline exists and ${completedGroups.length}/${outlineGroups.length} group file(s) are written. Generate missing groups: ${missingGroups.join(", ") || "unknown"}.`,
  };
}

async function statOrNull(file: string): Promise<import("node:fs").Stats | null> {
  try {
    return await fs.stat(file);
  } catch {
    return null;
  }
}
