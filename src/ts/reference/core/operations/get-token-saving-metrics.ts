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
  advisory: TokenSavingMetricsAdvisory;
  recentEvents: TokenSavingMetricsEvent[];
};

type TokenSavingMetricsEvent = Pick<
  TokenSavingEvent,
  "at" | "source" | "command" | "artifactRef" | "fullBytes" | "compactBytes" | "bytesAvoided" | "estimatedTokensSaved" | "metadata"
>;

type TokenSavingMetricsAdvisory = {
  hotspotThresholdBytes: number;
  hotspotCount: number;
  hotspots: TokenSavingHotspot[];
};

type TokenSavingHotspot = {
  kind: "large_context_projection";
  severity: "warning";
  at: string;
  source: TokenSavingEvent["source"];
  command?: string;
  artifactRef?: string;
  compactBytes: number;
  estimatedCompactTokens: number;
  fullBytes: number;
  bytesAvoided: number;
  estimatedTokensSaved: number;
  metadata?: Record<string, unknown>;
  recommendation: string;
};

const HOTSPOT_THRESHOLD_BYTES = 16 * 1024;
const ESTIMATED_BYTES_PER_TOKEN = 4;
const MAX_HOTSPOTS = 10;

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
    advisory: advisoryForEvents(telemetry?.recentEvents ?? []),
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

function advisoryForEvents(events: TokenSavingEvent[]): TokenSavingMetricsAdvisory {
  const hotspots = events
    .filter((event) => event.compactBytes >= HOTSPOT_THRESHOLD_BYTES)
    .sort((left, right) => right.compactBytes - left.compactBytes)
    .slice(0, MAX_HOTSPOTS)
    .map((event): TokenSavingHotspot => ({
      kind: "large_context_projection",
      severity: "warning",
      at: event.at,
      source: event.source,
      ...(event.command ? { command: event.command } : {}),
      ...(event.artifactRef ? { artifactRef: event.artifactRef } : {}),
      compactBytes: event.compactBytes,
      estimatedCompactTokens: Math.ceil(event.compactBytes / ESTIMATED_BYTES_PER_TOKEN),
      fullBytes: event.fullBytes,
      bytesAvoided: event.bytesAvoided,
      estimatedTokensSaved: event.estimatedTokensSaved,
      ...(event.metadata ? { metadata: event.metadata } : {}),
      recommendation: recommendationForEvent(event),
    }));

  return {
    hotspotThresholdBytes: HOTSPOT_THRESHOLD_BYTES,
    hotspotCount: hotspots.length,
    hotspots,
  };
}

function recommendationForEvent(event: TokenSavingEvent): string {
  if (event.source === "inspect_selectors") {
    return "This inspect projection is still large. Prefer a narrower selector or split the request read plan fieldGroup before adding automatic context policy.";
  }
  if (event.source === "request_manifest_refs") {
    return "The compact request manifest is still large. Move bulky root fields behind refs or split large protocol refs by narrower authority sections.";
  }
  return "This compact command envelope is still large. Move bulky command data behind refs or narrow the command output projection.";
}
