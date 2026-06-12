import { stateNotInitialized } from "../errors";
import { pathExists } from "../state/fs";
import { getLoomPaths } from "../state/paths";
import {
  projectRelativeTelemetryPath,
  readTokenSavingSummary,
  type TokenSavingEvent,
  type TokenSavingSourceTotals,
} from "./token-saving-telemetry";

export type GetTokenSavingMetricsInput = {
  projectRoot: string;
};

export type GetTokenSavingMetricsResult = {
  telemetryRef: string;
  updatedAt: string | null;
  eventCount: number;
  bytesAvoided: number;
  estimatedTokensSaved: number;
  bySource: Record<string, TokenSavingSourceTotals>;
  recentEvents: TokenSavingMetricsEvent[];
};

type TokenSavingMetricsEvent = Pick<
  TokenSavingEvent,
  "at" | "source" | "command" | "artifactRef" | "fullBytes" | "compactBytes" | "bytesAvoided" | "estimatedTokensSaved" | "metadata"
>;

export async function getTokenSavingMetrics(input: GetTokenSavingMetricsInput): Promise<GetTokenSavingMetricsResult> {
  const paths = getLoomPaths(input.projectRoot);
  if (!(await pathExists(paths.configFile)) || !(await pathExists(paths.statusFile))) {
    throw stateNotInitialized(paths.root);
  }

  const telemetry = await readTokenSavingSummary(paths.root);
  return {
    telemetryRef: projectRelativeTelemetryPath(paths.root),
    updatedAt: telemetry && telemetry.totals.eventCount > 0 ? telemetry.updatedAt : null,
    eventCount: telemetry?.totals.eventCount ?? 0,
    bytesAvoided: telemetry?.totals.bytesAvoided ?? 0,
    estimatedTokensSaved: telemetry?.totals.estimatedTokensSaved ?? 0,
    bySource: telemetry?.totals.bySource ?? {},
    recentEvents: telemetry?.recentEvents.map(compactEvent) ?? [],
  };
}

function compactEvent(event: TokenSavingEvent): TokenSavingMetricsEvent {
  return {
    at: event.at,
    source: event.source,
    ...(event.command ? { command: event.command } : {}),
    ...(event.artifactRef ? { artifactRef: event.artifactRef } : {}),
    fullBytes: event.fullBytes,
    compactBytes: event.compactBytes,
    bytesAvoided: event.bytesAvoided,
    estimatedTokensSaved: event.estimatedTokensSaved,
    ...(event.metadata ? { metadata: event.metadata } : {}),
  };
}
