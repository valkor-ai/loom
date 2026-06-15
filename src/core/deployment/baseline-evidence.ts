import { technicalBaselineSchema, type TechnicalBaseline } from "../contracts";
import { getActiveLocator, loadProjectStatus } from "../state/delivery";
import { pathExists, readJsonFile } from "../state/fs";
import { technicalBaselinePath, toProjectRelative } from "../state/paths";
import {
  normalizeTechnologyName,
  recordValue,
  stringValue,
} from "./evidence-utils";
import type {
  DeploymentCodeEvidence,
  DeploymentCodeEvidenceTrack,
} from "./types";

export type BaselineInfo = {
  baseline: TechnicalBaseline;
  ref: string;
};

export async function loadDeploymentTechnicalBaseline(projectRoot: string): Promise<BaselineInfo | null> {
  const deliveryIds = new Set<string>();
  try {
    const locator = await getActiveLocator(projectRoot);
    deliveryIds.add(locator.deliveryId);
  } catch {
    // Deploy can be used before a loom delivery exists.
  }
  try {
    const status = await loadProjectStatus(projectRoot);
    if (status.activeDeliveryId) deliveryIds.add(status.activeDeliveryId);
    if (status.lastCompletedDeliveryId) deliveryIds.add(status.lastCompletedDeliveryId);
  } catch {
    // Missing or partial state simply means no baseline is available.
  }

  for (const deliveryId of deliveryIds) {
    try {
      const absolutePath = technicalBaselinePath(projectRoot, deliveryId);
      if (!(await pathExists(absolutePath))) {
        continue;
      }
      const baseline = technicalBaselineSchema.parse(await readJsonFile(absolutePath));
      return {
        baseline,
        ref: toProjectRelative(projectRoot, absolutePath),
      };
    } catch {
      continue;
    }
  }
  return null;
}

export function normalizeBaselineExpectation(baseline: TechnicalBaseline | null): DeploymentCodeEvidence["baselineExpectation"] {
  const tracks = recordValue(recordValue(baseline?.stack)?.tracks);
  return {
    web: normalizeTrack(tracks?.web),
    app: normalizeTrack(tracks?.app),
    backend: normalizeTrack(tracks?.backend),
    persistence: normalizeTrack(tracks?.persistence),
    dataAccess: normalizeTrack(tracks?.dataAccess),
    externalServices: normalizeTrack(tracks?.externalServices),
  };
}

function normalizeTrack(value: unknown): DeploymentCodeEvidenceTrack | null {
  const record = recordValue(value);
  if (!record) {
    return null;
  }
  const selection = stringValue(record.selection);
  return {
    status: stringValue(record.status),
    selection,
    normalizedSelection: selection ? normalizeTechnologyName(selection) : null,
    source: stringValue(record.source),
    rationale: stringValue(record.rationale),
  };
}
