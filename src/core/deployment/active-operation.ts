import { promises as fs } from "node:fs";
import { z } from "zod";
import { deployOperationActive } from "../errors";
import { ensureDir, pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import { toProjectRelative } from "../state/paths";
import { getDeploymentPaths } from "./paths";
import type {
  DeploymentActiveOperation,
  DeploymentActiveOperationView,
  DeploymentOperationCommand,
  DeploymentOperationPhase,
} from "./types";

export type DeploymentOperationHandle = {
  operationId: string;
  ownsOperation: boolean;
};

const allowedCommands = ["deploy status", "deploy inspect", "deploy logs"] as const;
const forbiddenActions = ["deploy run", "deploy up", "deploy down", "raw docker compose", "kill process"] as const;

const activeOperationSchema = z.object({
  schemaVersion: z.literal(1),
  operationId: z.string().min(1),
  command: z.enum(["deploy.run", "deploy.prepare", "deploy.up", "deploy.down", "deploy.bootstrap"]),
  phase: z.enum([
    "preparing",
    "building",
    "starting",
    "validating",
    "checking_status",
    "stopping",
    "bootstrapping",
    "completed",
    "failed",
  ]),
  pid: z.number().int(),
  projectRoot: z.string().min(1),
  startedAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
  logRef: z.string().min(1),
  specRef: z.string().min(1).nullable(),
  status: z.enum(["running", "stale"]),
});

export async function withDeploymentOperation<T>(
  input: {
    projectRoot: string;
    command: DeploymentOperationCommand;
    phase: DeploymentOperationPhase;
  },
  run: (operation: DeploymentOperationHandle) => Promise<T>,
): Promise<T> {
  const operation = await acquireDeploymentOperation(input.projectRoot, input.command, input.phase);
  try {
    const result = await run(operation);
    await clearDeploymentOperation(input.projectRoot, operation);
    return result;
  } catch (error) {
    await markDeploymentOperationFailed(input.projectRoot, operation);
    throw error;
  }
}

export async function acquireDeploymentOperation(
  projectRoot: string,
  command: DeploymentOperationCommand,
  phase: DeploymentOperationPhase,
): Promise<DeploymentOperationHandle> {
  const paths = getDeploymentPaths(projectRoot);
  await ensureDir(paths.stateDir);
  await ensureDir(paths.logsDir);

  const existing = await readLiveDeploymentOperation(projectRoot);
  if (existing && existing.pid === process.pid) {
    await updateDeploymentOperationPhase(projectRoot, { operationId: existing.operationId, ownsOperation: false }, phase);
    return {
      operationId: existing.operationId,
      ownsOperation: false,
    };
  }
  if (existing) {
    throw deployOperationActive("Deployment operation is already active.", activeOperationDetails(projectRoot, existing));
  }

  const now = new Date().toISOString();
  const operation: DeploymentActiveOperation = {
    schemaVersion: 1,
    operationId: `deploy-op-${Date.now()}-${Math.random().toString(16).slice(2, 10)}`,
    command,
    phase,
    pid: process.pid,
    projectRoot,
    startedAt: now,
    updatedAt: now,
    logRef: toProjectRelative(projectRoot, paths.logFile),
    specRef: await pathExists(paths.specFile) ? toProjectRelative(projectRoot, paths.specFile) : null,
    status: "running",
  };

  try {
    const handle = await fs.open(paths.activeOperationFile, "wx");
    try {
      await handle.writeFile(`${JSON.stringify(operation, null, 2)}\n`, "utf8");
    } finally {
      await handle.close();
    }
    return {
      operationId: operation.operationId,
      ownsOperation: true,
    };
  } catch (error) {
    if (isNodeError(error) && error.code === "EEXIST") {
      let active = await readLiveDeploymentOperation(projectRoot);
      for (let attempt = 0; !active && attempt < 3; attempt += 1) {
        await sleep(50);
        active = await readLiveDeploymentOperation(projectRoot);
      }
      if (active && active.pid === process.pid) {
        return {
          operationId: active.operationId,
          ownsOperation: false,
        };
      }
      if (active) {
        throw deployOperationActive("Deployment operation is already active.", activeOperationDetails(projectRoot, active));
      }
      await fs.rm(paths.activeOperationFile, { force: true });
      return acquireDeploymentOperation(projectRoot, command, phase);
    }
    throw error;
  }
}

export async function assertNoActiveDeploymentOperation(
  projectRoot: string,
): Promise<void> {
  const active = await readLiveDeploymentOperation(projectRoot);
  if (active && active.pid !== process.pid) {
    throw deployOperationActive("Deployment operation is already active.", activeOperationDetails(projectRoot, active));
  }
}

export async function readDeploymentOperationView(
  projectRoot: string,
): Promise<DeploymentActiveOperationView | null> {
  const active = await readLiveDeploymentOperation(projectRoot);
  if (!active || active.pid === process.pid) {
    return null;
  }
  return activeOperationView(projectRoot, active);
}

export async function updateDeploymentOperationPhase(
  projectRoot: string,
  operation: DeploymentOperationHandle | undefined,
  phase: DeploymentOperationPhase,
): Promise<void> {
  if (!operation) return;
  const paths = getDeploymentPaths(projectRoot);
  const active = await readActiveDeploymentOperation(paths.activeOperationFile);
  if (!active || active.operationId !== operation.operationId) {
    return;
  }
  await writeJsonAtomic(paths.activeOperationFile, {
    ...active,
    phase,
    updatedAt: new Date().toISOString(),
    specRef: await pathExists(paths.specFile) ? toProjectRelative(projectRoot, paths.specFile) : active.specRef,
  });
}

async function clearDeploymentOperation(
  projectRoot: string,
  operation: DeploymentOperationHandle,
): Promise<void> {
  if (!operation.ownsOperation) return;
  const paths = getDeploymentPaths(projectRoot);
  const active = await readActiveDeploymentOperation(paths.activeOperationFile);
  if (!active || active.operationId !== operation.operationId) {
    return;
  }
  await fs.rm(paths.activeOperationFile, { force: true });
}

async function markDeploymentOperationFailed(
  projectRoot: string,
  operation: DeploymentOperationHandle,
): Promise<void> {
  if (!operation.ownsOperation) return;
  await updateDeploymentOperationPhase(projectRoot, operation, "failed");
  await clearDeploymentOperation(projectRoot, operation);
}

async function readLiveDeploymentOperation(projectRoot: string): Promise<DeploymentActiveOperation | null> {
  const paths = getDeploymentPaths(projectRoot);
  const active = await readActiveDeploymentOperation(paths.activeOperationFile);
  if (!active) {
    return null;
  }
  if (active.pid === process.pid || isProcessAlive(active.pid)) {
    return active;
  }

  const stale: DeploymentActiveOperation = {
    ...active,
    status: "stale",
    updatedAt: new Date().toISOString(),
  };
  await writeJsonAtomic(paths.staleOperationFile, stale);
  await fs.rm(paths.activeOperationFile, { force: true });
  return null;
}

async function readActiveDeploymentOperation(filePath: string): Promise<DeploymentActiveOperation | null> {
  if (!(await pathExists(filePath))) {
    return null;
  }
  try {
    return activeOperationSchema.parse(await readJsonFile(filePath));
  } catch {
    return null;
  }
}

function activeOperationDetails(projectRoot: string, active: DeploymentActiveOperation): Record<string, unknown> {
  return {
    status: "blocked",
    code: "DEPLOY_OPERATION_ACTIVE",
    activeOperation: activeOperationView(projectRoot, active),
    allowedCommands,
    forbiddenActions,
    nextAction: "wait_or_inspect",
    agentFacingBlocker: {
      mode: "observe_active_operation",
      source: "deploy.activeOperation",
      code: "DEPLOY_OPERATION_ACTIVE",
      reason: "A Loom-managed deploy operation is already running for this project.",
      allowedActions: [
        "report the active operation command, phase, elapsedMs, and logRef",
        "observe with deploy status, deploy inspect, or deploy logs",
        "wait for the active operation to finish before running another mutating deploy command",
      ],
      forbiddenActions: [
        "do not start deploy run, deploy up, deploy down, or deploy bootstrap --confirm",
        "do not run raw Docker or Docker Compose commands",
        "do not kill Loom, Docker, Docker Compose, or deployment processes",
      ],
      retryPolicy: {
        autoRetry: false,
        retryOnlyAfter: "the active operation has completed or is no longer present",
      },
    },
    projectRoot,
  };
}

function activeOperationView(projectRoot: string, active: DeploymentActiveOperation): DeploymentActiveOperationView {
  const started = Date.parse(active.startedAt);
  return {
    ...active,
    operationActive: true,
    elapsedMs: Number.isFinite(started) ? Math.max(0, Date.now() - started) : 0,
    activeOperationRef: toProjectRelative(projectRoot, getDeploymentPaths(projectRoot).activeOperationFile),
    allowedCommands,
    forbiddenActions,
  };
}

function isProcessAlive(pid: number): boolean {
  if (!Number.isInteger(pid) || pid <= 0) {
    return false;
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (isNodeError(error) && error.code === "ESRCH") {
      return false;
    }
    return true;
  }
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
