import { createHash } from "node:crypto";
import { ensureDir, writeJsonAtomic } from "../state/fs";
import { toProjectRelative } from "../state/paths";
import {
  type BaselineInfo,
  normalizeBaselineExpectation,
} from "./baseline-evidence";
import {
  buildStartFactsFor,
  collectDatabaseRuntimeEvidence,
  collectEmbeddedStores,
  collectServiceCandidates,
  conflictFacts,
  missingFactsFor,
  resolveDependencyServices,
  runtimeFactsFor,
  warningsFor,
} from "./dependency-evidence";
import { compactEvidenceValues, evidence } from "./evidence-utils";
import { indexProjectFiles, readFileSignals } from "./file-index";
import { getDeploymentPaths } from "./paths";
import type {
  DeploymentCodeEvidence,
  DeploymentCodeEvidenceSummary,
  DeploymentCodeProbe,
} from "./types";
export { loadDeploymentTechnicalBaseline } from "./baseline-evidence";

export async function buildDeploymentCodeEvidence(input: {
  projectRoot: string;
  stack: DeploymentCodeProbe;
  technicalBaseline: BaselineInfo | null;
}): Promise<DeploymentCodeEvidence> {
  const files = await indexProjectFiles(input.projectRoot);
  const signals = await readFileSignals(files);
  const baselineExpectation = normalizeBaselineExpectation(input.technicalBaseline?.baseline ?? null);
  const runtimeFacts = runtimeFactsFor(input.stack, signals);
  const serviceCandidates = collectServiceCandidates(signals);
  const embeddedStores = collectEmbeddedStores(signals);
  const databaseRuntimeEvidence = collectDatabaseRuntimeEvidence(signals);
  const dependencyServices = resolveDependencyServices({
    baselineExpectation,
    serviceCandidates,
    embeddedStores,
    databaseRuntimeEvidence,
    stack: input.stack,
  });
  const existingDeployAssets = signals
    .filter((signal) => signal.file.kind === "deploy_asset")
    .map((signal) => evidence(signal.file.relativePath, "Existing deployment asset found."));
  const conflicts = conflictFacts(baselineExpectation, dependencyServices.services, embeddedStores);
  const missingFacts = missingFactsFor({
    baselineExpectation,
    dependencyServices,
    databaseRuntimeEvidence,
  });
  const warnings = warningsFor(baselineExpectation, dependencyServices.services, embeddedStores);
  const generatedAt = new Date().toISOString();
  const evidenceId = `deploy-code-evidence-${Date.now()}`;
  const partial = {
    schemaVersion: 1 as const,
    evidenceId,
    generatedAt,
    fingerprint: "",
    projectRoot: input.projectRoot,
    technicalBaselineRef: input.technicalBaseline?.ref ?? null,
    baselineExpectation,
    runtimeFacts,
    buildStartFacts: buildStartFactsFor(input.stack),
    dependencyFacts: {
      services: dependencyServices.services,
      embeddedStores,
      ambiguous: dependencyServices.ambiguous,
    },
    environmentFacts: {
      required: databaseRuntimeEvidence,
      provided: [],
      generated: Object.assign({}, ...dependencyServices.services.map((service) => service.value.connectionEnv)),
      missing: missingFacts.flatMap((fact) => fact.evidence),
    },
    existingDeployAssets,
    conflicts,
    missingFacts,
    warnings,
  };
  return {
    ...partial,
    fingerprint: fingerprintEvidence(partial),
  };
}

export async function writeDeploymentCodeEvidence(
  projectRoot: string,
  evidenceValue: DeploymentCodeEvidence,
): Promise<DeploymentCodeEvidenceSummary> {
  const paths = getDeploymentPaths(projectRoot);
  await ensureDir(paths.evidenceDir);
  await writeJsonAtomic(paths.codeEvidenceFile, evidenceValue);
  return summarizeDeploymentCodeEvidence(projectRoot, evidenceValue);
}

export function summarizeDeploymentCodeEvidence(
  projectRoot: string,
  evidenceValue: DeploymentCodeEvidence,
): DeploymentCodeEvidenceSummary {
  return {
    ref: toProjectRelative(projectRoot, getDeploymentPaths(projectRoot).codeEvidenceFile),
    fingerprint: evidenceValue.fingerprint,
    technicalBaselineRef: evidenceValue.technicalBaselineRef,
    runtimeFacts: {
      web: evidenceValue.runtimeFacts.web?.value ?? null,
      backend: evidenceValue.runtimeFacts.backend?.value ?? null,
      fullstack: evidenceValue.runtimeFacts.fullstack?.value ?? null,
    },
    dependencyServices: evidenceValue.dependencyFacts.services.map((service) => ({
      kind: service.value.kind,
      serviceName: service.value.serviceName,
      reason: service.value.reason,
    })),
    embeddedStores: evidenceValue.dependencyFacts.embeddedStores.map((store) => store.value),
    warningCount: evidenceValue.warnings.length,
    conflictCount: evidenceValue.conflicts.length,
    missingFactCount: evidenceValue.missingFacts.length,
  };
}

export function applyDeploymentCodeEvidenceToStack(
  stack: DeploymentCodeProbe,
  evidenceValue: DeploymentCodeEvidence,
): DeploymentCodeProbe {
  return {
    ...stack,
    services: evidenceValue.dependencyFacts.services.map((service) => service.value),
  };
}

function fingerprintEvidence(value: Omit<DeploymentCodeEvidence, "fingerprint">): string {
  const stable = JSON.stringify({
    technicalBaselineRef: value.technicalBaselineRef,
    baselineExpectation: value.baselineExpectation,
    runtimeFacts: compactEvidenceValues(value.runtimeFacts),
    buildStartFacts: compactEvidenceValues(value.buildStartFacts),
    dependencyFacts: value.dependencyFacts,
    environmentFacts: value.environmentFacts,
    existingDeployAssets: value.existingDeployAssets,
    conflicts: value.conflicts,
    missingFacts: value.missingFacts,
    warnings: value.warnings,
  });
  return createHash("sha256").update(stable).digest("hex");
}
