import type {
  DeploymentEvidenceConfidence,
  DeploymentEvidenceRef,
  DeploymentEvidenceValue,
} from "./types";

export function evidence(pathValue: string, reason: string): DeploymentEvidenceRef {
  return { path: pathValue, reason };
}

export function valueEvidence<T>(
  value: T,
  confidence: DeploymentEvidenceConfidence,
  refs: DeploymentEvidenceRef[],
): DeploymentEvidenceValue<T> {
  return {
    value,
    confidence,
    evidence: dedupeRefs(refs),
  };
}

export function dedupeEvidenceValues<T extends string>(values: Array<DeploymentEvidenceValue<T>>): Array<DeploymentEvidenceValue<T>> {
  const seen = new Set<T>();
  const output: Array<DeploymentEvidenceValue<T>> = [];
  for (const value of values) {
    if (seen.has(value.value)) {
      continue;
    }
    seen.add(value.value);
    output.push(value);
  }
  return output;
}

export function dedupeRefs(refs: DeploymentEvidenceRef[]): DeploymentEvidenceRef[] {
  const seen = new Set<string>();
  const output: DeploymentEvidenceRef[] = [];
  for (const ref of refs) {
    const key = `${ref.path}\0${ref.reason}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    output.push(ref);
  }
  return output;
}

export function compactEvidenceValues(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(compactEvidenceValues);
  }
  if (!recordValue(value)) {
    return value;
  }
  const record = value as Record<string, unknown>;
  if ("value" in record && "confidence" in record) {
    return {
      value: record.value,
      confidence: record.confidence,
      evidence: record.evidence,
    };
  }
  return Object.fromEntries(Object.entries(record).map(([key, entry]) => [key, compactEvidenceValues(entry)]));
}

export function normalizeTechnologyName(value: string): string {
  return value.toLowerCase().replace(/\s*\+\s*/g, "+").trim();
}

export function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null;
}

export function recordValue(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}
