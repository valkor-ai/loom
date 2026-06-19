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
  DeploymentEvidenceRef,
  DeploymentRuntimeContract,
  DeployProvider,
  DetectedStack,
} from "./types";
export { loadDeploymentTechnicalBaseline } from "./baseline-evidence";

export async function buildDeploymentCodeEvidence(input: {
  projectRoot: string;
  stack: DetectedStack;
  technicalBaseline: BaselineInfo | null;
  provider?: DeployProvider;
  runtimeContract?: DeploymentRuntimeContract;
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
  const missingFacts = [
    ...missingFactsFor({
      baselineExpectation,
      dependencyServices,
      databaseRuntimeEvidence,
    }),
    ...deploymentSourceModelMissingFacts({
      provider: input.provider ?? null,
      stack: input.stack,
      runtimeContract: input.runtimeContract ?? null,
      existingDeployAssets,
    }),
  ];
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

function deploymentSourceModelMissingFacts(input: {
  provider: DeployProvider | null;
  stack: DetectedStack;
  runtimeContract: DeploymentRuntimeContract | null;
  existingDeployAssets: DeploymentEvidenceRef[];
}): DeploymentCodeEvidence["missingFacts"] {
  if (input.provider !== "dockerfile-template") {
    return [];
  }
  if (!input.runtimeContract || input.runtimeContract.source === "heuristic") {
    return [];
  }
  if (!requiresCompositeDeploymentShape(input.runtimeContract, input.stack)) {
    return [];
  }

  return [{
    factId: "composite-runtime-deployment-shape-required",
    type: "deployment_shape",
    message: "RuntimeDelivery describes separate frontend and backend capabilities, but generated dockerfile-template can only safely generate a single runnable stack. Deploy needs an explicit composite deployment shape before generating Dockerfile or Compose assets.",
    evidence: [
      evidence(input.runtimeContract.ref ?? "RuntimeDeliveryContract", `runtimeKind=${input.runtimeContract.runtimeKind ?? "unknown"}`),
      evidence("RuntimeDeliveryContract.buildCommand", input.runtimeContract.buildCommand ?? "No build command declared."),
      evidence("RuntimeDeliveryContract.startCommand", input.runtimeContract.startCommand ?? "No start command declared."),
      ...(input.runtimeContract.frontendOutputDir
        ? [evidence("RuntimeDeliveryContract.frontendOutputDir", input.runtimeContract.frontendOutputDir)]
        : []),
      evidence("detectedStack", `kind=${input.stack.kind}; framework=${input.stack.framework ?? "unknown"}`),
      ...(input.existingDeployAssets.length > 0
        ? input.existingDeployAssets.map((ref) => evidence(ref.path, `Existing deployment asset was detected but generated template provider is selected: ${ref.reason}`))
        : [evidence("deployment.provider", "No reusable root-level deployment asset selected for this composite runtime.")]),
    ],
    resolution: "ask_user",
  }];
}

function requiresCompositeDeploymentShape(
  runtimeContract: DeploymentRuntimeContract,
  stack: DetectedStack,
): boolean {
  const signals = [
    runtimeContract.runtimeKind,
    runtimeContract.buildCommand,
    runtimeContract.startCommand,
    runtimeContract.frontendOutputDir,
    stack.kind,
    stack.framework,
    stack.buildCommand,
    stack.startCommand,
    stack.outputDirectory,
    ...runtimeContract.apiPaths,
    ...runtimeContract.environment.required,
    ...runtimeContract.environment.optional,
  ]
    .filter((value): value is string => typeof value === "string")
    .join("\n")
    .toLowerCase();

  if (hasExplicitSingleServiceStaticShape(signals)) {
    return false;
  }

  const hasFrontend = Boolean(runtimeContract.frontendOutputDir) ||
    hasAnySignal(signals, ["vite", "react", "vue", "svelte", "astro", "frontend", "web-admin"]);
  const hasBackend = runtimeContract.apiPaths.length > 0 ||
    hasAnySignal(signals, ["spring", "spring-boot", "java", "maven", "gradle", "backend", "api", "mvn", "gradlew"]);

  return hasFrontend && hasBackend;
}

function hasExplicitSingleServiceStaticShape(signals: string): boolean {
  return /(^|[_\-\s])(serves|serve|served)[_\-\s]?(?:vite|react|frontend|web|static)?[_\-\s]?static($|[_\-\s])/.test(signals) ||
    /(^|[_\-\s])(?:express|spring|spring-boot|rails|django|laravel)[_\-\s]?static($|[_\-\s])/.test(signals);
}

function hasAnySignal(signals: string, terms: string[]): boolean {
  return terms.some((term) => {
    const escaped = term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`(^|[^a-z0-9])${escaped}($|[^a-z0-9])`).test(signals);
  });
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
  stack: DetectedStack,
  evidenceValue: DeploymentCodeEvidence,
): DetectedStack {
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
