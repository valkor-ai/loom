import path from "node:path";
import { getLoomPaths, toProjectRelative } from "../state/paths";
import { ensureDir, writeJsonIfMissing, writeTextIfMissing } from "../state/fs";
import type { LoomConfigV1, LoomStatusV1 } from "../schemas";

export type InitProjectInput = {
  projectRoot: string;
};

export type InitProjectResult = {
  initialized: true;
  created: string[];
  alreadyExisted: boolean;
};

export async function initProject(input: InitProjectInput): Promise<InitProjectResult> {
  const paths = getLoomPaths(input.projectRoot);
  const created: string[] = [];
  const alreadyExisted = await directoryExists(paths.loomDir);
  const now = new Date().toISOString();

  await ensureDir(paths.loomDir);

  const config: LoomConfigV1 = {
    schemaVersion: 1,
    project: {
      name: path.basename(paths.root),
      createdAtRoot: paths.root,
    },
    defaults: {
      language: "auto",
      mode: "build",
      verificationLevel: "standard",
    },
    git: {
      policy: "local",
    },
    features: {
      plan: true,
      build: true,
      review: true,
      repair: true,
      deploy: false,
    },
    createdAt: now,
    updatedAt: now,
  };

  if (await writeJsonIfMissing(paths.configFile, config)) {
    created.push(toProjectRelative(paths.root, paths.configFile));
  }

  const status: LoomStatusV1 = {
    schemaVersion: 1,
    activeDeliveryId: null,
    lastCompletedDeliveryId: null,
    deliveries: [],
    effectiveNextAction: {
      type: "brainstorm_start",
      source: "initialized_fallback",
      ref: null,
      reason: "NO_ACTIVE_DELIVERY",
    },
    phase: "idle",
    current: {
      requirementId: null,
      planId: null,
      taskId: null,
      reviewId: null,
      repairId: null,
      deploymentId: null,
    },
    lastAction: null,
    nextAction: "plan",
    updatedAt: now,
  };

  if (await writeJsonIfMissing(paths.statusFile, status)) {
    created.push(toProjectRelative(paths.root, paths.statusFile));
  }

  for (const dir of paths.directories) {
    if (await ensureDir(dir)) {
      created.push(toProjectRelative(paths.root, dir));
    }
  }

  if (await writeTextIfMissing(paths.gitignoreFile, "*\n!.gitignore\n")) {
    created.push(toProjectRelative(paths.root, paths.gitignoreFile));
  }

  return {
    initialized: true,
    created,
    alreadyExisted,
  };
}

async function directoryExists(dirPath: string): Promise<boolean> {
  try {
    const { promises: fs } = await import("node:fs");
    const stat = await fs.stat(dirPath);
    return stat.isDirectory();
  } catch {
    return false;
  }
}
