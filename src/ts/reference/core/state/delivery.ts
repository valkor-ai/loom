import { ZodError } from "zod";
import { invalidArgument, stateCorrupted } from "../errors";
import {
  type DeliveryIndex,
  type DeliveryIndexPhase,
  type LoomStatusV1,
  deliveryIndexSchema,
  loomStatusV1Schema,
} from "../schemas";
import { pathExists, readJsonFile, readJsonWithSchemaVersion, writeJsonAtomic } from "./fs";
import {
  type DeliveryPhaseLocator,
  brainstormLatestPath,
  deliveryIndexPath,
  getLoomPaths,
  toProjectRelative,
} from "./paths";

export type DeliverySummaryPatch = Partial<Pick<DeliveryIndex, "status" | "requestSummary" | "activePhaseId" | "roadmapId">>;

export async function loadProjectStatus(projectRoot: string): Promise<LoomStatusV1> {
  const paths = getLoomPaths(projectRoot);
  return parseStatus(await readJsonWithSchemaVersion(paths.statusFile));
}

export async function saveProjectStatus(projectRoot: string, status: LoomStatusV1): Promise<void> {
  const paths = getLoomPaths(projectRoot);
  await writeJsonAtomic(paths.statusFile, loomStatusV1Schema.parse(status));
}

export async function loadDeliveryIndex(projectRoot: string, deliveryId: string): Promise<DeliveryIndex> {
  const filePath = deliveryIndexPath(projectRoot, deliveryId);
  if (!(await pathExists(filePath))) {
    throw invalidArgument("DeliveryRun does not exist.", { deliveryId });
  }
  try {
    return deliveryIndexSchema.parse(await readJsonFile(filePath));
  } catch (error) {
    if (error instanceof ZodError) {
      throw stateCorrupted("Delivery index does not match schema.", {
        file: filePath,
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

export async function saveDeliveryIndex(projectRoot: string, index: DeliveryIndex): Promise<void> {
  await writeJsonAtomic(deliveryIndexPath(projectRoot, index.deliveryId), deliveryIndexSchema.parse(index));
}

export async function getActiveLocator(projectRoot: string, phaseId?: string): Promise<DeliveryPhaseLocator> {
  const status = await loadProjectStatus(projectRoot);
  const deliveryId = status.activeDeliveryId;
  if (!deliveryId) {
    throw invalidArgument("No active loom delivery. Start with loom plan or brainstorm start.");
  }
  const index = await loadDeliveryIndex(projectRoot, deliveryId);
  const selectedPhaseId = phaseId ?? index.activePhaseId;
  ensurePhase(index, selectedPhaseId);
  return {
    deliveryId,
    phaseId: selectedPhaseId,
  };
}

export async function resolveLocator(projectRoot: string, deliveryId?: string, phaseId?: string): Promise<DeliveryPhaseLocator> {
  if (deliveryId) {
    return getLocatorForDelivery(projectRoot, deliveryId, phaseId);
  }
  return getActiveLocator(projectRoot, phaseId);
}

export async function getLocatorForDelivery(projectRoot: string, deliveryId: string, phaseId?: string): Promise<DeliveryPhaseLocator> {
  const index = await loadDeliveryIndex(projectRoot, deliveryId);
  const selectedPhaseId = phaseId ?? index.activePhaseId;
  ensurePhase(index, selectedPhaseId);
  return {
    deliveryId,
    phaseId: selectedPhaseId,
  };
}

export async function getLocatorForBrainstormRun(projectRoot: string, brainstormRunId: string): Promise<DeliveryPhaseLocator> {
  const status = await loadProjectStatus(projectRoot);
  for (const delivery of status.deliveries ?? []) {
    const index = await loadDeliveryIndex(projectRoot, delivery.deliveryId);
    const phase = index.phases.find((item) => item.latestRefs.brainstormRunId === brainstormRunId);
    if (phase) {
      return {
        deliveryId: delivery.deliveryId,
        phaseId: phase.phaseId,
      };
    }
    const brainstorm = await readDeliveryLatestBrainstorm(projectRoot, delivery.deliveryId);
    if (brainstorm?.brainstormRunId === brainstormRunId) {
      return {
        deliveryId: delivery.deliveryId,
        phaseId: index.activePhaseId,
      };
    }
  }
  throw invalidArgument("Brainstorm run is not attached to a delivery.", { brainstormRunId });
}

export async function upsertStatusDelivery(projectRoot: string, index: DeliveryIndex): Promise<void> {
  const status = await loadProjectStatus(projectRoot);
  const deliveries = [...(status.deliveries ?? [])];
  const entry = {
    deliveryId: index.deliveryId,
    status: index.status,
    requestSummary: index.requestSummary,
    activePhaseId: index.activePhaseId,
    indexRef: toProjectRelative(projectRoot, deliveryIndexPath(projectRoot, index.deliveryId)),
    updatedAt: index.updatedAt,
  };
  const existingIndex = deliveries.findIndex((item) => item.deliveryId === index.deliveryId);
  if (existingIndex >= 0) {
    deliveries[existingIndex] = entry;
  } else {
    deliveries.push(entry);
  }
  status.deliveries = deliveries;
  status.activeDeliveryId = index.status === "completed" ? null : index.deliveryId;
  status.lastCompletedDeliveryId = index.status === "completed" ? index.deliveryId : status.lastCompletedDeliveryId ?? null;
  status.effectiveNextAction = index.phases.find((phase) => phase.phaseId === index.activePhaseId)?.nextAction ?? null;
  status.phase = statusPhaseForDelivery(index.status);
  status.nextAction = nextActionForDelivery(index.status, status.effectiveNextAction);
  status.updatedAt = index.updatedAt;
  await saveProjectStatus(projectRoot, status);
}

export function updatePhase(index: DeliveryIndex, phaseId: string, patch: Partial<DeliveryIndexPhase>): DeliveryIndex {
  const phase = ensurePhase(index, phaseId);
  Object.assign(phase, patch);
  return index;
}

function ensurePhase(index: DeliveryIndex, phaseId: string): DeliveryIndexPhase {
  const phase = index.phases.find((item) => item.phaseId === phaseId);
  if (!phase) {
    throw invalidArgument("Phase does not exist in DeliveryRun.", {
      deliveryId: index.deliveryId,
      phaseId,
    });
  }
  return phase;
}

function statusPhaseForDelivery(status: DeliveryIndex["status"]): LoomStatusV1["phase"] {
  if (status === "executing") return "building";
  if (status === "reviewing") return "reviewing";
  if (status === "repairing") return "repairing";
  if (status === "completed") return "completed";
  if (status === "blocked") return "blocked";
  return "planning";
}

function nextActionForDelivery(
  status: DeliveryIndex["status"],
  action: DeliveryIndexPhase["nextAction"] | null,
): LoomStatusV1["nextAction"] {
  if (status === "completed") return "none";
  if (!action) return "none";
  if (action.type === "continue_execution") {
    return action.refs?.activeOperationType === "task_execution" && typeof action.refs.executionRequestRef === "string"
      ? "execute-task"
      : "next-task";
  }
  if (action.type === "review") return "review";
  if (action.type.endsWith("_repair")) return "repair";
  return "plan";
}

async function readDeliveryLatestBrainstorm(projectRoot: string, deliveryId: string): Promise<{ brainstormRunId?: string } | null> {
  const file = brainstormLatestPath(projectRoot, deliveryId);
  if (!(await pathExists(file))) {
    return null;
  }
  const json = await readJsonFile(file);
  return typeof json === "object" && json !== null ? json as { brainstormRunId?: string } : null;
}

function parseStatus(json: unknown): LoomStatusV1 {
  try {
    return loomStatusV1Schema.parse(json);
  } catch (error) {
    if (error instanceof ZodError) {
      throw stateCorrupted("loom status file does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}
