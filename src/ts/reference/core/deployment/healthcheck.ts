import { invalidArgument } from "../errors";
import type { DeploymentHealthcheckInput, DeploymentSpec } from "./types";

export function applyHealthcheckInput(
  spec: DeploymentSpec,
  input?: DeploymentHealthcheckInput,
): DeploymentSpec {
  if (!input || Object.keys(input).length === 0) {
    return spec;
  }

  const normalized = normalizeHealthcheckInput(input);
  const baseUrl = spec.runtime.url.replace(/\/+$/, "");
  const path = normalized.path ?? spec.runtime.healthcheck.path;
  const enabled = normalized.enabled ?? spec.runtime.healthcheck.enabled;

  return {
    ...spec,
    runtime: {
      ...spec.runtime,
      healthcheck: {
        ...spec.runtime.healthcheck,
        enabled,
        path,
        candidates: normalized.candidates ?? spec.runtime.healthcheck.candidates,
        expectedStatusMax: normalized.expectedStatusMax ?? spec.runtime.healthcheck.expectedStatusMax,
        attempts: normalized.attempts ?? spec.runtime.healthcheck.attempts,
        intervalMs: normalized.intervalMs ?? spec.runtime.healthcheck.intervalMs,
        timeoutMs: normalized.timeoutMs ?? spec.runtime.healthcheck.timeoutMs,
        url: enabled ? `${baseUrl}${path}` : null,
      },
    },
  };
}

export function normalizeHealthcheckInput(input: DeploymentHealthcheckInput): DeploymentHealthcheckInput {
  return {
    enabled: input.enabled,
    path: input.path === undefined ? undefined : normalizeHealthPath(input.path, "healthcheck path"),
    candidates: input.candidates === undefined
      ? undefined
      : dedupePaths(input.candidates.map((candidate) => normalizeHealthPath(candidate, "healthcheck candidate"))),
    expectedStatusMax: input.expectedStatusMax === undefined
      ? undefined
      : positiveIntInRange(input.expectedStatusMax, "healthcheck expected status max", 200, 599),
    attempts: input.attempts === undefined
      ? undefined
      : positiveIntInRange(input.attempts, "healthcheck attempts", 1, 120),
    intervalMs: input.intervalMs === undefined
      ? undefined
      : positiveIntInRange(input.intervalMs, "healthcheck interval ms", 0, 120_000),
    timeoutMs: input.timeoutMs === undefined
      ? undefined
      : positiveIntInRange(input.timeoutMs, "healthcheck timeout ms", 1, 120_000),
  };
}

function normalizeHealthPath(value: string, label: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    throw invalidArgument(`${label} cannot be empty.`);
  }

  if (/^https?:\/\//i.test(trimmed)) {
    try {
      const url = new URL(trimmed);
      return `${url.pathname || "/"}${url.search}`;
    } catch {
      throw invalidArgument(`${label} must be a path or URL.`);
    }
  }

  return trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
}

function positiveIntInRange(value: number, label: string, min: number, max: number): number {
  if (!Number.isInteger(value) || value < min || value > max) {
    throw invalidArgument(`${label} must be an integer between ${min} and ${max}.`, {
      value,
      min,
      max,
    });
  }
  return value;
}

function dedupePaths(values: string[]): string[] {
  return [...new Set(values)];
}
