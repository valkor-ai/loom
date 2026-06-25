import {
  DATABASE_SERVICE_KINDS,
  databaseRuntimeSignalLabel,
  dependencyServiceEvidenceMatches,
  hasDatabaseRuntimeSignal,
  hasSqliteSignal,
  isDependencyManifestPath,
  persistenceKindFromSelection,
  persistenceServiceKindFromSelection,
  prismaProvider,
  serviceDefinition,
  withSpringDatasourceConnectionEnv,
} from "./dependency-signals";
import {
  dedupeEvidenceValues,
  dedupeRefs,
  evidence,
  valueEvidence,
} from "./evidence-utils";
import type { FileSignal } from "./file-index";
import type {
  DependencyService,
  DependencyServiceKind,
  DeploymentCodeEvidence,
  DeploymentCodeEvidenceTrack,
  DeploymentEvidenceConfidence,
  DeploymentEvidenceRef,
  DeploymentEvidenceValue,
  DeployConflict,
  DeployMissingFact,
  DeploymentCodeProbe,
} from "./types";

type ServiceCandidate = {
  kind: DependencyServiceKind;
  strength: "driver" | "runtime_config" | "explicit_provider" | "env";
  evidence: DeploymentEvidenceRef[];
};

export function runtimeFactsFor(stack: DeploymentCodeProbe, signals: FileSignal[]): DeploymentCodeEvidence["runtimeFacts"] {
  const framework = stack.framework ?? stack.kind;
  const evidenceRefs = signalsForRuntime(stack, signals);
  const backend = ["java", "python", "go", "dotnet", "php", "ruby"].includes(stack.kind) ||
      (stack.kind === "node" && framework && /express|fastify|hono|koa|server|next/.test(framework))
    ? valueEvidence(framework, stack.kind === "unknown" ? "low" : "high", evidenceRefs)
    : null;
  const web = stack.kind === "static" || (stack.kind === "node" && framework && /vite|next|react|vue|svelte|astro/.test(framework))
    ? valueEvidence(framework, "high", evidenceRefs)
    : null;
  return {
    web,
    backend,
    fullstack: web && backend ? valueEvidence(`${web.value}+${backend.value}`, "medium", evidenceRefs) : null,
    workers: [],
  };
}

export function buildStartFactsFor(stack: DeploymentCodeProbe): DeploymentCodeEvidence["buildStartFacts"] {
  const baseEvidence = [evidence("codeProbe", "Derived from current project runtime detection.")];
  return {
    buildCommand: stack.buildCommand ? valueEvidence(stack.buildCommand, "medium", baseEvidence) : null,
    startCommand: stack.startCommand ? valueEvidence(stack.startCommand, "medium", baseEvidence) : null,
    port: valueEvidence(stack.port, "medium", baseEvidence),
    healthPath: stack.healthcheckPath ? valueEvidence(stack.healthcheckPath, "medium", baseEvidence) : null,
    previewPath: valueEvidence("/", "low", baseEvidence),
    frontendOutputDir: stack.outputDirectory ? valueEvidence(stack.outputDirectory, "medium", baseEvidence) : null,
    staticServing: null,
  };
}

export function collectServiceCandidates(signals: FileSignal[]): ServiceCandidate[] {
  const candidates: ServiceCandidate[] = [];
  for (const signal of signals) {
    const add = (kind: DependencyServiceKind, reason: string, strength: ServiceCandidate["strength"] = "driver") => {
      candidates.push({
        kind,
        strength,
        evidence: [evidence(signal.file.relativePath, reason)],
      });
    };
    const lower = signal.lower;
    const text = signal.text;

    if (signal.file.relativePath.endsWith("schema.prisma")) {
      const provider = prismaProvider(text);
      if (provider === "postgresql") add("postgres", "Prisma datasource provider is postgresql.", "explicit_provider");
      if (provider === "mysql") add("mysql", "Prisma datasource provider is mysql.", "explicit_provider");
      if (provider === "mongodb") add("mongodb", "Prisma datasource provider is mongodb.", "explicit_provider");
    }

    for (const match of dependencyServiceEvidenceMatches({
      path: signal.file.relativePath,
      text,
      lower,
    })) {
      add(match.kind, match.reason, match.database ? dbStrength(signal) : serviceStrength(signal));
    }
  }
  return candidates;
}

export function collectEmbeddedStores(signals: FileSignal[]): Array<DeploymentEvidenceValue<"sqlite" | "file">> {
  const stores: Array<DeploymentEvidenceValue<"sqlite" | "file">> = [];
  for (const signal of signals) {
    if (hasSqliteSignal(signal.file.relativePath, signal.text, signal.lower)) {
      stores.push(valueEvidence(
        "sqlite",
        "high",
        [evidence(signal.file.relativePath, signal.file.relativePath.endsWith("schema.prisma") && prismaProvider(signal.text) === "sqlite"
          ? "Prisma datasource provider is sqlite."
          : "SQLite driver or connection signal found.")],
      ));
    }
  }
  return dedupeEvidenceValues(stores);
}

export function collectDatabaseRuntimeEvidence(signals: FileSignal[]): DeploymentEvidenceRef[] {
  const refs: DeploymentEvidenceRef[] = [];
  for (const signal of signals) {
    const lower = signal.lower;
    if (hasDatabaseRuntimeSignal(lower)) {
      const matched = databaseRuntimeSignalLabel(lower);
      refs.push(evidence(signal.file.relativePath, `Database runtime configuration or environment reference found${matched ? `: ${matched}` : ""}.`));
    }
  }
  return dedupeRefs(refs);
}

export function resolveDependencyServices(input: {
  baselineExpectation: DeploymentCodeEvidence["baselineExpectation"];
  serviceCandidates: ServiceCandidate[];
  embeddedStores: Array<DeploymentEvidenceValue<"sqlite" | "file">>;
  databaseRuntimeEvidence: DeploymentEvidenceRef[];
  stack: DeploymentCodeProbe;
}): {
  services: Array<DeploymentEvidenceValue<DependencyService>>;
  ambiguous: DeploymentCodeEvidence["dependencyFacts"]["ambiguous"];
} {
  const sqliteObserved = input.embeddedStores.some((store) => store.value === "sqlite");
  const grouped = new Map<DependencyServiceKind, ServiceCandidate[]>();
  for (const candidate of input.serviceCandidates) {
    if (sqliteObserved && isPackageOnlySqlDriver(candidate)) {
      continue;
    }
    const current = grouped.get(candidate.kind) ?? [];
    current.push(candidate);
    grouped.set(candidate.kind, current);
  }

  const baselineKind = baselinePersistenceServiceKind(input.baselineExpectation.persistence);
  const groupedDatabaseKinds = [...grouped.keys()].filter((kind) => DATABASE_SERVICE_KINDS.has(kind));
  if (input.databaseRuntimeEvidence.length > 0 && baselineKind && !grouped.has(baselineKind) && groupedDatabaseKinds.length === 0) {
    grouped.set(baselineKind, [{
      kind: baselineKind,
      strength: "runtime_config",
      evidence: [
        ...input.databaseRuntimeEvidence,
        evidence("TechnicalBaseline", `Database runtime signal uses baseline persistence ${input.baselineExpectation.persistence?.selection}.`),
      ],
    }]);
  }

  const services = [...grouped.entries()]
    .map(([kind, candidates]) => {
      const refs = dedupeRefs([
        ...candidates.flatMap((candidate) => candidate.evidence),
        ...(DATABASE_SERVICE_KINDS.has(kind) ? input.databaseRuntimeEvidence : []),
      ]);
      const service = withSpringDatasourceConnectionEnv(
        serviceDefinition(kind, refs.map((ref) => `${ref.path}: ${ref.reason}`).join(" ")),
        input.stack,
        refs.map((ref) => `${ref.path} ${ref.reason}`).join("\n"),
      );
      return valueEvidence(service, serviceConfidence(candidates), refs);
    })
    .sort((left, right) => left.value.kind.localeCompare(right.value.kind));

  const hasDatabaseService = services.some((service) => DATABASE_SERVICE_KINDS.has(service.value.kind));
  const ambiguous = input.databaseRuntimeEvidence.length > 0 && !hasDatabaseService && !sqliteObserved
    ? [{
        kind: "database" as const,
        reason: "Code references a database runtime binding, but no database kind was identified from code evidence or TechnicalBaseline.",
        evidence: input.databaseRuntimeEvidence,
      }]
    : [];

  return { services, ambiguous };
}

export function conflictFacts(
  baselineExpectation: DeploymentCodeEvidence["baselineExpectation"],
  services: Array<DeploymentEvidenceValue<DependencyService>>,
  embeddedStores: Array<DeploymentEvidenceValue<"sqlite" | "file">>,
): DeployConflict[] {
  const conflicts: DeployConflict[] = [];
  const baselineKind = baselinePersistenceKind(baselineExpectation.persistence);
  const codeDatabaseKinds = [
    ...services
      .map((service) => service.value.kind)
      .filter((kind) => DATABASE_SERVICE_KINDS.has(kind)),
    ...embeddedStores.map((store) => store.value),
  ];
  if (!baselineKind || codeDatabaseKinds.length === 0) {
    return conflicts;
  }
  const normalizedCodeKinds = new Set(codeDatabaseKinds.map((kind) => kind === "file" ? "file" : kind));
  if (!normalizedCodeKinds.has(baselineKind)) {
    const firstService = services.find((service) => DATABASE_SERVICE_KINDS.has(service.value.kind));
    const firstEmbedded = embeddedStores[0];
    conflicts.push({
      conflictId: "baseline-persistence-code-conflict",
      type: "technical_baseline_code_conflict",
      message: `TechnicalBaseline persistence is ${baselineExpectation.persistence?.selection}, but repository evidence indicates ${[...normalizedCodeKinds].join(", ")}.`,
      left: evidence("TechnicalBaseline", `persistence=${baselineExpectation.persistence?.selection ?? "unknown"}`),
      right: firstService?.evidence[0] ?? firstEmbedded?.evidence[0] ?? evidence("repository", "Database evidence found."),
      resolution: "ask_user",
    });
  }
  return conflicts;
}

export function missingFactsFor(input: {
  baselineExpectation: DeploymentCodeEvidence["baselineExpectation"];
  dependencyServices: {
    services: Array<DeploymentEvidenceValue<DependencyService>>;
    ambiguous: DeploymentCodeEvidence["dependencyFacts"]["ambiguous"];
  };
  databaseRuntimeEvidence: DeploymentEvidenceRef[];
}): DeployMissingFact[] {
  if (input.dependencyServices.ambiguous.length === 0) {
    return [];
  }
  return [{
    factId: "database-kind-required",
    type: "database_kind",
    message: "Repository code references a database runtime binding, but deploy cannot determine the database kind.",
    evidence: input.databaseRuntimeEvidence,
    resolution: "execution_repair",
  }];
}

export function warningsFor(
  baselineExpectation: DeploymentCodeEvidence["baselineExpectation"],
  services: Array<DeploymentEvidenceValue<DependencyService>>,
  embeddedStores: Array<DeploymentEvidenceValue<"sqlite" | "file">>,
): string[] {
  const warnings: string[] = [];
  const baselineKind = baselinePersistenceKind(baselineExpectation.persistence);
  const codeHasDatabase = services.some((service) => DATABASE_SERVICE_KINDS.has(service.value.kind)) || embeddedStores.length > 0;
  if (baselineKind && !codeHasDatabase) {
    warnings.push(`TechnicalBaseline expects ${baselineExpectation.persistence?.selection}, but current code evidence does not show an implemented database dependency. Deploy will not start that service from baseline alone.`);
  }
  return warnings;
}

function serviceConfidence(candidates: ServiceCandidate[]): DeploymentEvidenceConfidence {
  if (candidates.some((candidate) => candidate.strength === "explicit_provider" || candidate.strength === "runtime_config" || candidate.strength === "env")) {
    return "high";
  }
  return candidates.length > 1 ? "high" : "medium";
}

function dbStrength(signal: FileSignal): ServiceCandidate["strength"] {
  return signal.file.kind === "manifest" ? "driver" : "runtime_config";
}

function serviceStrength(signal: FileSignal): ServiceCandidate["strength"] {
  return signal.file.kind === "env" || signal.file.kind === "config" ? "env" : "driver";
}

function isPackageOnlySqlDriver(candidate: ServiceCandidate): boolean {
  return candidate.strength === "driver" &&
    candidate.evidence.every((ref) => isDependencyManifestPath(ref.path));
}

function baselinePersistenceKind(track: DeploymentCodeEvidenceTrack | null): DependencyServiceKind | "sqlite" | "file" | null {
  return track ? persistenceKindFromSelection(track) : null;
}

function baselinePersistenceServiceKind(track: DeploymentCodeEvidenceTrack | null): DependencyServiceKind | null {
  return track ? persistenceServiceKindFromSelection(track) : null;
}

function signalsForRuntime(stack: DeploymentCodeProbe, signals: FileSignal[]): DeploymentEvidenceRef[] {
  const refs = signals
    .filter((signal) => {
      const lower = signal.lower;
      if (stack.kind === "java") return /spring-boot|pom\.xml|build\.gradle|application\.properties|application\.ya?ml/.test(lower) || /pom\.xml|build\.gradle|application\./.test(signal.file.relativePath);
      if (stack.kind === "node") return /package\.json$/.test(signal.file.relativePath);
      if (stack.kind === "python") return /pyproject\.toml|requirements\.txt|fastapi|django|flask/.test(signal.file.relativePath) || /fastapi|django|flask/.test(lower);
      if (stack.kind === "go") return /go\.mod$/.test(signal.file.relativePath);
      if (stack.kind === "dotnet") return /\.csproj$|\.sln$|appsettings/.test(signal.file.relativePath);
      return false;
    })
    .slice(0, 5)
    .map((signal) => evidence(signal.file.relativePath, "Runtime declaration signal."));
  return refs.length > 0 ? refs : [evidence("codeProbe", "Derived from current project runtime detection.")];
}
