export type DeployProvider = "compose-existing" | "dockerfile-existing" | "dockerfile-template";

export type DeploymentProviderPolicy = {
  provider?: DeployProvider;
  reuseExisting: boolean;
  forceGenerate: boolean;
};

export type DeploymentProviderCandidate = {
  provider: DeployProvider;
  status: "selected" | "available" | "skipped";
  reason: string;
  commands: string[][];
};

export type DeploymentHealthStatus = "healthy" | "unhealthy" | "unknown" | "disabled";

export type DeploymentHealth = {
  status: DeploymentHealthStatus;
  url: string | null;
  checkedAt: string | null;
  statusCode: number | null;
  error: string | null;
};

export type DeploymentHealthcheckInput = {
  enabled?: boolean;
  path?: string;
  candidates?: string[];
  expectedStatusMax?: number;
  attempts?: number;
  intervalMs?: number;
  timeoutMs?: number;
};

export type DeploymentOperationCommand =
  | "deploy.run"
  | "deploy.prepare"
  | "deploy.up"
  | "deploy.down"
  | "deploy.bootstrap";

export type DeploymentOperationPhase =
  | "preparing"
  | "building"
  | "starting"
  | "validating"
  | "checking_status"
  | "stopping"
  | "bootstrapping"
  | "completed"
  | "failed";

export type DeploymentActiveOperation = {
  schemaVersion: 1;
  operationId: string;
  command: DeploymentOperationCommand;
  phase: DeploymentOperationPhase;
  pid: number;
  projectRoot: string;
  startedAt: string;
  updatedAt: string;
  logRef: string;
  specRef: string | null;
  status: "running" | "stale";
};

export type DeploymentActiveOperationView = DeploymentActiveOperation & {
  operationActive: true;
  elapsedMs: number;
  activeOperationRef: string;
  allowedCommands: readonly ["deploy status", "deploy inspect", "deploy logs"];
  forbiddenActions: readonly [
    "deploy run",
    "deploy up",
    "deploy down",
    "raw docker compose",
    "kill process",
  ];
};

export type DeploymentComposePort = {
  hostPort: number | null;
  containerPort: number;
  protocol: string | null;
  raw: string;
};

export type DeploymentComposeService = {
  name: string;
  score: number;
  image: string | null;
  build: boolean;
  ports: DeploymentComposePort[];
  expose: number[];
  dependsOn: string[];
  profiles: string[];
  dependencyLike: boolean;
  reason: string;
};

export type DeploymentComposeInfo = {
  selectedService: string | null;
  serviceReason: string;
  services: DeploymentComposeService[];
  warnings: string[];
};

export type DependencyServiceKind =
  | "postgres"
  | "redis"
  | "mysql"
  | "mongodb"
  | "rabbitmq"
  | "elasticsearch"
  | "minio";

export type DependencyService = {
  kind: DependencyServiceKind;
  serviceName: string;
  image: string;
  port: number;
  env: Record<string, string>;
  connectionEnv: Record<string, string>;
  volumeName: string | null;
  volumeTarget: string | null;
  reason: string;
};

export type DeploymentEvidenceRef = {
  path: string;
  reason: string;
};

export type DeploymentEvidenceConfidence = "low" | "medium" | "high";

export type DeploymentEvidenceValue<T> = {
  value: T;
  confidence: DeploymentEvidenceConfidence;
  evidence: DeploymentEvidenceRef[];
};

export type DeploymentCodeEvidenceTrack = {
  status: string | null;
  selection: string | null;
  normalizedSelection: string | null;
  source: string | null;
  rationale: string | null;
};

export type DeploymentCodeEvidence = {
  schemaVersion: 1;
  evidenceId: string;
  generatedAt: string;
  fingerprint: string;
  projectRoot: string;
  technicalBaselineRef: string | null;
  baselineExpectation: {
    web: DeploymentCodeEvidenceTrack | null;
    app: DeploymentCodeEvidenceTrack | null;
    backend: DeploymentCodeEvidenceTrack | null;
    persistence: DeploymentCodeEvidenceTrack | null;
    dataAccess: DeploymentCodeEvidenceTrack | null;
    externalServices: DeploymentCodeEvidenceTrack | null;
  };
  runtimeFacts: {
    web: DeploymentEvidenceValue<string> | null;
    backend: DeploymentEvidenceValue<string> | null;
    fullstack: DeploymentEvidenceValue<string> | null;
    workers: DeploymentEvidenceValue<string>[];
  };
  buildStartFacts: {
    buildCommand: DeploymentEvidenceValue<string> | null;
    startCommand: DeploymentEvidenceValue<string> | null;
    port: DeploymentEvidenceValue<number> | null;
    healthPath: DeploymentEvidenceValue<string> | null;
    previewPath: DeploymentEvidenceValue<string> | null;
    frontendOutputDir: DeploymentEvidenceValue<string> | null;
    staticServing: DeploymentEvidenceValue<boolean> | null;
  };
  dependencyFacts: {
    services: Array<DeploymentEvidenceValue<DependencyService>>;
    embeddedStores: Array<DeploymentEvidenceValue<"sqlite" | "file">>;
    ambiguous: Array<{
      kind: "database" | "cache" | "queue" | "object_storage" | "search";
      reason: string;
      evidence: DeploymentEvidenceRef[];
    }>;
  };
  environmentFacts: {
    required: DeploymentEvidenceRef[];
    provided: DeploymentEvidenceRef[];
    generated: Record<string, string>;
    missing: DeploymentEvidenceRef[];
  };
  existingDeployAssets: DeploymentEvidenceRef[];
  conflicts: DeployConflict[];
  missingFacts: DeployMissingFact[];
  warnings: string[];
};

export type DeployConflict = {
  conflictId: string;
  type: "technical_baseline_code_conflict" | "deployment_asset_conflict" | "runtime_fact_conflict";
  message: string;
  left: DeploymentEvidenceRef;
  right: DeploymentEvidenceRef;
  resolution: "ask_user" | "execution_repair";
};

export type DeployMissingFact = {
  factId: string;
  type: "database_kind" | "build_command" | "start_command" | "probe" | "external_config" | "deployment_shape";
  message: string;
  evidence: DeploymentEvidenceRef[];
  resolution: "ask_user" | "execution_repair";
};

export type DeploymentCodeEvidenceSummary = {
  ref: string;
  fingerprint: string;
  technicalBaselineRef: string | null;
  runtimeFacts: {
    web: string | null;
    backend: string | null;
    fullstack: string | null;
  };
  dependencyServices: Array<{
    kind: DependencyServiceKind;
    serviceName: string;
    reason: string;
  }>;
  embeddedStores: string[];
  warningCount: number;
  conflictCount: number;
  missingFactCount: number;
};

export type DetectedStack = {
  kind: "node" | "python" | "go" | "java" | "dotnet" | "php" | "ruby" | "static" | "unknown";
  packageManager:
    | "npm"
    | "pnpm"
    | "yarn"
    | "bun"
    | "pip"
    | "poetry"
    | "uv"
    | "go"
    | "maven"
    | "gradle"
    | "dotnet"
    | "composer"
    | "bundler"
    | null;
  hasLockfile: boolean;
  framework: string | null;
  runtimeVersion: string | null;
  runtimeVersionSource: string | null;
  buildCommand: string | null;
  startCommand: string | null;
  outputDirectory: string | null;
  port: number;
  healthcheckPath?: string | null;
  services: DependencyService[];
  workingDirectory: string | null;
  workspacePackageJsonPaths?: string[];
};

export type DeploymentWorkspaceCandidate = {
  path: string;
  score: number;
  stackKind: DetectedStack["kind"];
  framework: string | null;
  packageManager: DetectedStack["packageManager"];
  signals: string[];
};

export type DeploymentWorkspace = {
  appPath: string;
  isWorkspace: boolean;
  buildContextPath: string;
  reason: string;
  candidates: DeploymentWorkspaceCandidate[];
};

export type DeploymentEnvSource =
  | "runtime-default"
  | "dependency-service"
  | "generated-default"
  | "env-example"
  | "local-env-file"
  | "source-code"
  | "configuration"
  | "framework"
  | "runtime-contract";

export type DeploymentEnvVariable = {
  name: string;
  required: boolean;
  sensitive: boolean;
  provided: boolean;
  generated: boolean;
  sources: DeploymentEnvSource[];
  reason: string;
};

export type DeploymentEnvFile = {
  path: string;
  variables: string[];
  ignored: boolean;
};

export type DeploymentEnvDiagnostics = {
  required: DeploymentEnvVariable[];
  referenced: DeploymentEnvVariable[];
  provided: string[];
  generated: Record<string, string>;
  missing: DeploymentEnvVariable[];
  localEnvFiles: DeploymentEnvFile[];
  warnings: string[];
};

export type DeploymentBootstrapKind =
  | "prisma"
  | "django"
  | "rails"
  | "laravel"
  | "flyway"
  | "liquibase";

export type DeploymentBootstrapTask = {
  kind: DeploymentBootstrapKind;
  command: string;
  automatic: boolean;
  reason: string;
};

export type DeploymentBootstrapDiagnostics = {
  tasks: DeploymentBootstrapTask[];
  warnings: string[];
};

export type DeploymentFailureDiagnostic = {
  code: string;
  severity: "error" | "warning" | "info";
  message: string;
  evidence: string[];
  suggestedAction: string;
};

export type DeploymentFailureOwner =
  | "application_code"
  | "deployment_assets"
  | "environment"
  | "external_system"
  | "unknown";

export type DeploymentRepairRoute =
  | "execution_repair"
  | "deploy_repair"
  | "manual_review"
  | "none";

export type DeploymentRuntimeContractSource = "accepted_aac" | "previous_accepted_aac" | "heuristic";

export type DeploymentRuntimeContract = {
  source: DeploymentRuntimeContractSource;
  ref: string | null;
  status: "modified" | "unchanged" | "not_applicable" | "heuristic";
  dependencyServicePolicy: "heuristic" | "contract_only";
  runtimeKind: string | null;
  buildCommand: string | null;
  startCommand: string | null;
  port: number | null;
  previewPath: string;
  healthPath: string | null;
  apiPaths: string[];
  frontendOutputDir: string | null;
  probeKind: "http" | "process" | "command";
  environment: {
    required: string[];
    optional: string[];
  };
  dependencyServices: DependencyService[];
};

export type DeploymentSpec = {
  schemaVersion: 1;
  provider: DeployProvider;
  providerReason: string;
  providerPolicy: DeploymentProviderPolicy;
  providerCandidates: DeploymentProviderCandidate[];
  serviceName: string;
  imageName: string;
  projectRoot: string;
  generatedAt: string;
  workspace: DeploymentWorkspace;
  detectedStack: DetectedStack;
  environment: DeploymentEnvDiagnostics;
  bootstrap: DeploymentBootstrapDiagnostics;
  compose: DeploymentComposeInfo;
  runtimeContract: DeploymentRuntimeContract;
  codeEvidence?: DeploymentCodeEvidenceSummary;
  files: {
    dockerfilePath: string | null;
    composePath: string;
    dockerignorePath: string | null;
    buildContextPath: string;
    generated: boolean;
    reused: string[];
  };
  runtime: {
    containerPort: number;
    hostPort: number;
    url: string;
    healthcheck: {
      enabled: boolean;
      path: string;
      candidates: string[];
      url: string | null;
      expectedStatusMax: number;
      attempts: number;
      intervalMs: number;
      timeoutMs: number;
    };
  };
  commands: {
    build: string[];
    up: string[];
    down: string[];
    logs: string[];
    status: string[];
  };
};

export type DeploymentBootstrapRunStatus =
  | "skipped"
  | "completed"
  | "failed";

export type DeploymentBootstrapRunResult = {
  kind: DeploymentBootstrapKind;
  command: string;
  serviceName: string;
  status: DeploymentBootstrapRunStatus;
  exitCode: number | null;
  stdoutTail: string[];
  stderrTail: string[];
  reason: string;
};

export type DeploymentFailureKind =
  | "docker_unavailable"
  | "compose_config"
  | "registry_network"
  | "image_build"
  | "container_start"
  | "healthcheck"
  | "runtime_contract_missing"
  | "runtime_contract_not_applicable"
  | "runtime_contract_mismatch"
  | "build_command_failed"
  | "start_command_failed"
  | "application_startup_failed"
  | "http_probe_failed"
  | "preview_not_verified"
  | "deploy_asset_invalid"
  | "logs"
  | "unknown";

export type DeploymentErrorWindow = {
  lines: string[];
  truncated: boolean;
  totalLineCount: number;
  matchedPatterns: string[];
};

export type DeploymentRepairRequest = {
  schemaVersion: 1;
  repairId: string;
  createdAt: string;
  projectRoot: string;
  specPath: string;
  provider: DeployProvider;
  failureKind: DeploymentFailureKind;
  failureOwner?: DeploymentFailureOwner;
  repairRoute?: DeploymentRepairRoute;
  failureRef?: string | null;
  command: string[];
  exitCode: number;
  fullLogRef?: string | null;
  errorWindow?: DeploymentErrorWindow;
  stdoutTail: string[];
  stderrTail: string[];
  providerCandidates: DeploymentProviderCandidate[];
  environment: DeploymentEnvDiagnostics;
  bootstrap: DeploymentBootstrapDiagnostics;
  diagnostics: DeploymentFailureDiagnostic[];
  suggestedActions: string[];
  editableFiles: string[];
  protectedFiles: string[];
  instruction: string;
  maxAttempts: number;
  attempts: number;
  status: "pending";
};

export type DeploymentFailureReport = {
  schemaVersion: "1.0";
  failureId: string;
  source: "deploy";
  createdAt: string;
  deploymentAttemptId: string;
  failureKind: DeploymentFailureKind;
  failureOwner: DeploymentFailureOwner;
  repairRoute: DeploymentRepairRoute;
  runtimeDeliveryRef: string | null;
  sourceRefs: {
    runtimeDeliveryRef: string | null;
    taskPlanRef: string | null;
    taskPlanRunRef: string | null;
    reviewResultRef: string | null;
    deploymentSpecRef: string;
  };
  failedContract: {
    field: string;
    command: string | null;
    workingDirectory: string;
  };
  evidence: {
    failedAt: string;
    deployCommand: string[];
    exitCode: number;
    fullLogRef?: string | null;
    errorWindow?: DeploymentErrorWindow;
    stdoutTail: string[];
    stderrTail: string[];
    logMarkers: string[];
    diagnostics: DeploymentFailureDiagnostic[];
  };
  routing: {
    editableBoundary: "application_code_only" | "deployment_assets_only" | "manual_only" | "none";
    mustNotEdit: string[];
    nextCommand: {
      name: string;
      argv: string[];
    } | null;
  };
  loopGuard: {
    signature: string;
    attempt: number;
    maxAttempts: number;
  };
};

export type DeployExecutionRepairRequest = {
  schemaVersion: "1.0";
  repairId: string;
  repairType: "execution_repair";
  source: "deploy_failure";
  deploymentFailureRef: string;
  agentAction?: Record<string, unknown>;
  sourceRefs: DeploymentFailureReport["sourceRefs"];
  referencedArtifactReadGuide?: Array<Record<string, unknown>>;
  syntheticTask: {
    taskId: string;
    taskKind: "runtime_delivery";
    title: string;
    objective: string;
    mutatesOriginalTaskPlan: false;
    relatedTaskIds: string[];
    writeBoundary: {
      forbiddenPaths: string[];
    };
    runtimeDeliveryRequirement: {
      appliesToThisTask: true;
      source: "deploy_failure";
      deploymentFailureRef: string;
      runtimeDeliveryRef: string | null;
      affectedContractFields: string[];
      requiredCodeLevelChecks: Array<{
        checkId: string;
        contractField: string;
        objective: string;
        acceptableEvidence: string[];
      }>;
      forbiddenActions: string[];
    };
  };
  executionRules: Record<string, unknown>;
  outputContract: {
    format: "json";
    schema: "DeployExecutionRepairTaskResult";
    resultFile: string;
    schemaShape: Record<string, unknown>;
    submitCommand: {
      name: string;
      argv: string[];
    };
  };
  createdAt: string;
};

export type DeployExecutionRepairTaskResult = {
  schemaVersion: "1.0";
  repairId: string;
  status: "completed" | "completed_with_notes" | "blocked" | "failed";
  deploymentFailureRef: string;
  changedFiles: string[];
  runtimeDeliveryEvidence: {
    source: "deploy_failure_repair";
    addressedFailedContractFields: string[];
    codeLevelChecks: Array<{
      checkId: string;
      status: "passed" | "failed" | "blocked" | "not_applicable";
      evidence: string;
    }>;
    commandsRun: Array<{
      command: string;
      status: "passed" | "failed" | "not_run";
      environment: string;
      summary?: string;
    }>;
    unverifiedItems: Array<{
      item: string;
      reason: string;
    }>;
    runtimeProbeCleanup?: {
      temporaryRuntimeStarted: boolean;
      attempted: boolean;
      status: "not_needed" | "succeeded" | "failed" | "unknown" | "not_safe_to_cleanup";
      targets?: Array<{
        kind: "process" | "port" | "container" | "dev_server" | "other";
        pid?: number | null;
        port?: number | null;
        command?: string | null;
        summary: string;
      }>;
      summary: string;
    };
  };
  selfRepairSummary: {
    attempted: boolean;
    attemptCount: number;
    stopReason: string;
    progressObserved: boolean;
  };
  notes: string[];
};

export type DeploymentState = {
  schemaVersion: 1;
  provider: DeployProvider;
  serviceName: string;
  appServiceName: string | null;
  imageName: string;
  projectRoot: string;
  specPath: string;
  composePath: string;
  containerName: string | null;
  containerId: string | null;
  running: boolean;
  url: string | null;
  health: DeploymentHealth;
  startedAt: string | null;
  updatedAt: string;
};
