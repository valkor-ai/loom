import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import {
  type OperationLease,
  operationLeaseSchema,
} from "../contracts";
import {
  type DeliveryIndex,
  type DeliveryIndexPhase,
  type RouteAction,
} from "../schemas";
import { invalidArgument } from "../errors";
import { pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import {
  loadDeliveryIndex,
  saveDeliveryIndex,
  updatePhase,
  upsertStatusDelivery,
} from "../state/delivery";
import {
  type DeliveryPhaseLocator,
  continueDecisionLatestPath,
  operationLeasePath,
} from "../state/paths";

export type RouteUpdateInput = {
  projectRoot: string;
  locator: DeliveryPhaseLocator;
  deliveryStatus?: DeliveryIndex["status"];
  phaseStatus?: DeliveryIndexPhase["status"];
  activePhaseId?: string;
  latestRefs?: Record<string, string | null>;
  nextAction: RouteAction | null;
};

export async function updateRouteState(input: RouteUpdateInput): Promise<DeliveryIndex> {
  const index = await loadDeliveryIndex(input.projectRoot, input.locator.deliveryId);
  const phase = index.phases.find((item) => item.phaseId === input.locator.phaseId);
  if (!phase) {
    throw invalidArgument("Phase does not exist in DeliveryRun.", input.locator);
  }
  updatePhase(index, input.locator.phaseId, {
    ...(input.phaseStatus ? { status: input.phaseStatus } : {}),
    latestRefs: {
      ...phase.latestRefs,
      ...(input.latestRefs ?? {}),
    },
    nextAction: input.nextAction,
  });
  if (input.deliveryStatus) {
    index.status = input.deliveryStatus;
  }
  if (input.activePhaseId) {
    index.activePhaseId = input.activePhaseId;
  }
  index.updatedAt = new Date().toISOString();
  await saveDeliveryIndex(input.projectRoot, index);
  await upsertStatusDelivery(input.projectRoot, index);
  await clearStaleContinueDecision(input.projectRoot, input.locator);
  return index;
}

async function clearStaleContinueDecision(projectRoot: string, locator: DeliveryPhaseLocator): Promise<void> {
  try {
    await fs.unlink(continueDecisionLatestPath(projectRoot, locator));
  } catch (error) {
    if (!(error instanceof Error && "code" in error && error.code === "ENOENT")) {
      throw error;
    }
  }
}

export async function createOperationLease(input: {
  projectRoot: string;
  locator: DeliveryPhaseLocator;
  operationType: string;
  refs?: Record<string, unknown>;
  ttlSeconds?: number;
}): Promise<OperationLease> {
  const existing = await readOperationLease(input.projectRoot, input.locator.deliveryId);
  if (existing?.status === "active" && new Date(existing.expiresAt).getTime() > Date.now()) {
    throw invalidArgument("Another loom operation is already active.", {
      operationId: existing.operationId,
      operationType: existing.operationType,
      expiresAt: existing.expiresAt,
    });
  }
  if (existing?.status === "active") {
    await markOperationLeaseStale(input.projectRoot, input.locator.deliveryId, "replaced_by_new_operation");
  }
  const now = new Date();
  const lease = operationLeaseSchema.parse({
    schemaVersion: "1.0",
    operationId: createId("op"),
    deliveryId: input.locator.deliveryId,
    phaseId: input.locator.phaseId,
    operationType: input.operationType,
    status: "active",
    startedAt: now.toISOString(),
    heartbeatAt: now.toISOString(),
    expiresAt: new Date(now.getTime() + (input.ttlSeconds ?? ttlForOperation(input.operationType)) * 1000).toISOString(),
    refs: input.refs ?? {},
  });
  await writeJsonAtomic(operationLeasePath(input.projectRoot, input.locator.deliveryId), lease);
  return lease;
}

export async function closeOperationLease(input: {
  projectRoot: string;
  locator: DeliveryPhaseLocator;
  operationType?: string;
  expectedRefs?: Record<string, unknown>;
  reason?: string;
}): Promise<OperationLease | null> {
  const lease = await readOperationLease(input.projectRoot, input.locator.deliveryId);
  if (!lease || lease.status !== "active") {
    return null;
  }
  if (input.operationType && lease.operationType !== input.operationType) {
    return null;
  }
  for (const [key, value] of Object.entries(input.expectedRefs ?? {})) {
    if (lease.refs[key] !== value) {
      return null;
    }
  }
  const closed = operationLeaseSchema.parse({
    ...lease,
    status: "closed",
    refs: {
      ...lease.refs,
      closedReason: input.reason ?? "operation_completed",
      closedAt: new Date().toISOString(),
    },
  });
  await writeJsonAtomic(operationLeasePath(input.projectRoot, input.locator.deliveryId), closed);
  return closed;
}

export async function markOperationLeaseStale(
  projectRoot: string,
  deliveryId: string,
  reason = "stale_recovered",
): Promise<OperationLease | null> {
  const lease = await readOperationLease(projectRoot, deliveryId);
  if (!lease || lease.status !== "active") {
    return null;
  }
  const stale = operationLeaseSchema.parse({
    ...lease,
    status: "stale_recovered",
    refs: {
      ...lease.refs,
      staleReason: reason,
      staleRecoveredAt: new Date().toISOString(),
    },
  });
  await writeJsonAtomic(operationLeasePath(projectRoot, deliveryId), stale);
  return stale;
}

export async function refreshOperationLease(
  projectRoot: string,
  deliveryId: string,
  reason = "resume_existing_operation",
): Promise<OperationLease | null> {
  const lease = await readOperationLease(projectRoot, deliveryId);
  if (!lease || lease.status !== "active") {
    return null;
  }
  const now = new Date();
  const refreshed = operationLeaseSchema.parse({
    ...lease,
    expiresAt: new Date(now.getTime() + ttlForOperation(lease.operationType) * 1000).toISOString(),
    refs: {
      ...lease.refs,
      refreshedReason: reason,
      refreshedAt: now.toISOString(),
    },
  });
  await writeJsonAtomic(operationLeasePath(projectRoot, deliveryId), refreshed);
  return refreshed;
}

export async function readOperationLease(projectRoot: string, deliveryId: string): Promise<OperationLease | null> {
  const file = operationLeasePath(projectRoot, deliveryId);
  if (!(await pathExists(file))) {
    return null;
  }
  return operationLeaseSchema.parse(await readJsonFile(file));
}

export type OperationProgressSignal = "candidate_files" | "candidate_file" | "project_files_and_result_file" | "result_file" | "unknown";

export function progressSignalForOperation(operationType: string): OperationProgressSignal {
  if (operationType === "architecture_generation" || operationType === "taskplan_generation") {
    return "candidate_files";
  }
  if (operationType === "execution_repair") {
    return "project_files_and_result_file";
  }
  if (
    operationType === "technical_baseline_generation" ||
    operationType === "repository_context_generation" ||
    operationType === "task_result_repair" ||
    operationType === "taskplan_repair" ||
    operationType === "architecture_artifact_repair"
  ) {
    return "candidate_file";
  }
  if (operationType === "task_execution" || operationType === "review_generation") {
    return "result_file";
  }
  return "unknown";
}

export function operationRef(lease: OperationLease): Record<string, unknown> {
  return {
    operationId: lease.operationId,
    progressSignal: progressSignalForOperation(lease.operationType),
    resumeCommand: {
      name: "continue",
      argv: ["continue"],
    },
    expiresAt: lease.expiresAt,
  };
}

export function ttlForOperation(operationType: string): number {
  const ttlByOperation: Record<string, number> = {
    technical_baseline_generation: 900,
    repository_context_generation: 900,
    architecture_generation: 1200,
    architecture_artifact_repair: 1200,
    taskplan_generation: 900,
    task_execution: 1800,
    execution_repair: 1800,
    task_result_repair: 600,
    taskplan_repair: 900,
    review_generation: 900,
  };
  return ttlByOperation[operationType] ?? 900;
}

function createId(prefix: string): string {
  return `${prefix}-${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${createHash("sha1")
    .update(`${process.pid}:${Math.random()}:${Date.now()}`)
    .digest("hex")
    .slice(0, 8)}`;
}
