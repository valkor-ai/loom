import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { ensureDir, pathExists, readJsonFile, writeJsonAtomic } from "../state/fs";
import { getLoomPaths, toProjectRelative } from "../state/paths";

const SCHEMA_VERSION = "1.0";
const ESTIMATED_BYTES_PER_TOKEN = 4;
const MAX_RECENT_EVENTS = 100;

export type TokenSavingEventSource =
  | "compact_envelope"
  | "inspect_selectors"
  | "request_manifest_refs";

export type TokenSavingEventInput = {
  projectRoot: string;
  source: TokenSavingEventSource;
  fullBytes: number;
  compactBytes: number;
  command?: string;
  artifactRef?: string;
  metadata?: Record<string, unknown>;
};

export type TokenSavingSourceTotals = {
  eventCount: number;
  bytesAvoided: number;
  estimatedTokensSaved: number;
};

export type TokenSavingSummary = {
  schemaVersion: typeof SCHEMA_VERSION;
  updatedAt: string;
  totals: TokenSavingSourceTotals & {
    bySource: Record<string, TokenSavingSourceTotals>;
  };
  recentEvents: TokenSavingEvent[];
};

export type TokenSavingEvent = {
  eventId: string;
  at: string;
  source: TokenSavingEventSource;
  command?: string;
  artifactRef?: string;
  fullBytes: number;
  compactBytes: number;
  bytesAvoided: number;
  estimatedTokensSaved: number;
  metadata?: Record<string, unknown>;
};

export function prettyJsonByteLength(value: unknown): number {
  return Buffer.byteLength(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

export function compactJsonByteLength(value: unknown): number {
  return Buffer.byteLength(`${JSON.stringify(value)}\n`, "utf8");
}

export async function recordTokenSavingEvent(input: TokenSavingEventInput): Promise<void> {
  const normalized = normalizeEvent(input);
  if (!normalized) return;

  const paths = getLoomPaths(input.projectRoot);
  if (!(await pathExists(paths.loomDir))) return;

  try {
    const current = normalizeTelemetryFile(
      await readJsonFile(paths.tokenSavingTelemetryFile).catch(() => null),
    );
    await ensureDir(paths.metricsDir);
    await writeJsonAtomic(paths.tokenSavingTelemetryFile, addEvent(current, normalized));
  } catch {
    // Telemetry must never block delivery commands.
  }
}

export function recordTokenSavingEventSync(input: TokenSavingEventInput): void {
  const normalized = normalizeEvent(input);
  if (!normalized) return;

  const paths = getLoomPaths(input.projectRoot);
  if (!existsSync(paths.loomDir)) return;

  try {
    const current = normalizeTelemetryFile(readJsonSync(paths.tokenSavingTelemetryFile));
    mkdirSync(paths.metricsDir, { recursive: true });
    writeJsonAtomicSync(paths.tokenSavingTelemetryFile, addEvent(current, normalized));
  } catch {
    // Telemetry must never block delivery commands or stdout.
  }
}

export async function readTokenSavingSummary(projectRoot: string): Promise<TokenSavingSummary | null> {
  const paths = getLoomPaths(projectRoot);
  if (!(await pathExists(paths.tokenSavingTelemetryFile))) {
    return null;
  }
  return normalizeTelemetryFile(await readJsonFile(paths.tokenSavingTelemetryFile).catch(() => null));
}

function normalizeEvent(input: TokenSavingEventInput): TokenSavingEvent | null {
  const fullBytes = Math.max(0, Math.round(input.fullBytes));
  const compactBytes = Math.max(0, Math.round(input.compactBytes));
  const bytesAvoided = fullBytes - compactBytes;
  if (bytesAvoided <= 0) return null;

  const at = new Date().toISOString();
  return {
    eventId: `tse-${at.replace(/[-:.TZ]/g, "").slice(0, 14)}-${Math.random().toString(16).slice(2, 10)}`,
    at,
    source: input.source,
    ...(input.command ? { command: input.command } : {}),
    ...(input.artifactRef ? { artifactRef: input.artifactRef } : {}),
    fullBytes,
    compactBytes,
    bytesAvoided,
    estimatedTokensSaved: estimateTokens(bytesAvoided),
    ...(input.metadata ? { metadata: sanitizeMetadata(input.metadata) } : {}),
  };
}

function addEvent(current: TokenSavingSummary, event: TokenSavingEvent): TokenSavingSummary {
  const bySource = { ...current.totals.bySource };
  const sourceTotals = bySource[event.source] ?? emptySourceTotals();
  bySource[event.source] = {
    eventCount: sourceTotals.eventCount + 1,
    bytesAvoided: sourceTotals.bytesAvoided + event.bytesAvoided,
    estimatedTokensSaved: sourceTotals.estimatedTokensSaved + event.estimatedTokensSaved,
  };

  return {
    schemaVersion: SCHEMA_VERSION,
    updatedAt: event.at,
    totals: {
      eventCount: current.totals.eventCount + 1,
      bytesAvoided: current.totals.bytesAvoided + event.bytesAvoided,
      estimatedTokensSaved: current.totals.estimatedTokensSaved + event.estimatedTokensSaved,
      bySource,
    },
    recentEvents: [event, ...current.recentEvents].slice(0, MAX_RECENT_EVENTS),
  };
}

function normalizeTelemetryFile(value: unknown): TokenSavingSummary {
  if (!isRecord(value) || value.schemaVersion !== SCHEMA_VERSION || !isRecord(value.totals)) {
    return emptyTelemetryFile();
  }

  const totals = value.totals;
  const bySource: Record<string, TokenSavingSourceTotals> = {};
  if (isRecord(totals.bySource)) {
    for (const [source, sourceTotals] of Object.entries(totals.bySource)) {
      if (isRecord(sourceTotals)) {
        bySource[source] = normalizeSourceTotals(sourceTotals);
      }
    }
  }

  return {
    schemaVersion: SCHEMA_VERSION,
    updatedAt: typeof value.updatedAt === "string" ? value.updatedAt : new Date(0).toISOString(),
    totals: {
      ...normalizeSourceTotals(totals),
      bySource,
    },
    recentEvents: Array.isArray(value.recentEvents)
      ? value.recentEvents
        .filter(isRecord)
        .map(normalizeStoredEvent)
        .filter((event): event is TokenSavingEvent => event !== null)
        .slice(0, MAX_RECENT_EVENTS)
      : [],
  };
}

function normalizeStoredEvent(value: Record<string, unknown>): TokenSavingEvent | null {
  const source = value.source;
  if (source !== "compact_envelope" && source !== "inspect_selectors" && source !== "request_manifest_refs") return null;
  const fullBytes = numberValue(value.fullBytes);
  const compactBytes = numberValue(value.compactBytes);
  const bytesAvoided = numberValue(value.bytesAvoided);
  if (bytesAvoided <= 0) return null;
  return {
    eventId: typeof value.eventId === "string" ? value.eventId : `tse-${Math.random().toString(16).slice(2, 10)}`,
    at: typeof value.at === "string" ? value.at : new Date(0).toISOString(),
    source,
    ...(typeof value.command === "string" ? { command: value.command } : {}),
    ...(typeof value.artifactRef === "string" ? { artifactRef: value.artifactRef } : {}),
    fullBytes,
    compactBytes,
    bytesAvoided,
    estimatedTokensSaved: Math.max(0, numberValue(value.estimatedTokensSaved)),
    ...(isRecord(value.metadata) ? { metadata: sanitizeMetadata(value.metadata) } : {}),
  };
}

function normalizeSourceTotals(value: Record<string, unknown>): TokenSavingSourceTotals {
  return {
    eventCount: numberValue(value.eventCount),
    bytesAvoided: numberValue(value.bytesAvoided),
    estimatedTokensSaved: numberValue(value.estimatedTokensSaved),
  };
}

function emptyTelemetryFile(): TokenSavingSummary {
  return {
    schemaVersion: SCHEMA_VERSION,
    updatedAt: new Date(0).toISOString(),
    totals: {
      ...emptySourceTotals(),
      bySource: {},
    },
    recentEvents: [],
  };
}

function emptySourceTotals(): TokenSavingSourceTotals {
  return {
    eventCount: 0,
    bytesAvoided: 0,
    estimatedTokensSaved: 0,
  };
}

function estimateTokens(bytes: number): number {
  return Math.ceil(bytes / ESTIMATED_BYTES_PER_TOKEN);
}

function readJsonSync(filePath: string): unknown {
  try {
    return JSON.parse(readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function writeJsonAtomicSync(filePath: string, value: unknown): void {
  mkdirSync(path.dirname(filePath), { recursive: true });
  const tmp = `${filePath}.tmp-${process.pid}-${Date.now()}`;
  writeFileSync(tmp, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  renameSync(tmp, filePath);
}

function sanitizeMetadata(value: Record<string, unknown>): Record<string, unknown> {
  const output: Record<string, unknown> = {};
  for (const [key, child] of Object.entries(value)) {
    if (typeof child === "string" || typeof child === "number" || typeof child === "boolean" || child === null) {
      output[key] = child;
    } else if (Array.isArray(child)) {
      output[key] = child
        .filter((item) => typeof item === "string" || typeof item === "number" || typeof item === "boolean")
        .slice(0, 50);
    }
  }
  return output;
}

function numberValue(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? Math.max(0, Math.round(value)) : 0;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function projectRelativeTelemetryPath(projectRoot: string): string {
  return toProjectRelative(projectRoot, getLoomPaths(projectRoot).tokenSavingTelemetryFile);
}
