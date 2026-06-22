import { promises as fs } from "node:fs";
import path from "node:path";
import { ZodError, z } from "zod";
import { deployNotPrepared, stateCorrupted } from "../errors";
import { readJsonFile, readJsonWithSchemaVersion, writeJsonAtomic } from "../state/fs";
import { hydrateRequestManifest, writeRequestManifestAtomic } from "../operations/request-manifest";
import { DEFAULT_DEPLOY_REPAIR_MAX_ATTEMPTS } from "./constants";
import { getDeploymentPaths, getDeploymentRepairPaths } from "./paths";
import { buildDeploymentTopology } from "./topology";
import type {
  DeployExecutionRepairRequest,
  DeployExecutionRepairTaskResult,
  DeploymentFailureReport,
  DeploymentRepairRequest,
  DeploymentSpec,
  DeploymentState,
} from "./types";

const healthSchema = z.object({
  status: z.enum(["healthy", "unhealthy", "unknown", "disabled"]),
  url: z.string().nullable(),
  checkedAt: z.string().datetime().nullable(),
  statusCode: z.number().int().nullable(),
  error: z.string().nullable(),
});

const legacyCodeProbeSchema = z.object({
  kind: z.enum(["node", "python", "go", "java", "dotnet", "php", "ruby", "static", "unknown"]),
  packageManager: z.enum(["npm", "pnpm", "yarn", "bun", "pip", "poetry", "uv", "go", "maven", "gradle", "dotnet", "composer", "bundler"]).nullable(),
  hasLockfile: z.boolean(),
  framework: z.string().nullable(),
  runtimeVersion: z.string().nullable().default(null),
  runtimeVersionSource: z.string().nullable().default(null),
  buildCommand: z.string().nullable(),
  startCommand: z.string().nullable(),
  outputDirectory: z.string().nullable(),
  port: z.number().int().positive(),
  healthcheckPath: z.string().nullable().optional(),
  services: z.array(
    z.object({
      kind: z.enum([
        "postgres",
        "redis",
        "mysql",
        "mongodb",
        "rabbitmq",
        "elasticsearch",
        "minio",
      ]),
      serviceName: z.string().min(1),
      image: z.string().min(1),
      port: z.number().int().positive(),
      env: z.record(z.string()),
      connectionEnv: z.record(z.string()),
      volumeName: z.string().nullable(),
      volumeTarget: z.string().nullable(),
      reason: z.string().min(1),
    }),
  ),
  workingDirectory: z.string().nullable().default(null),
  workspacePackageJsonPaths: z.array(z.string().min(1)).default([]),
});

const dependencyServiceSchema = z.object({
  kind: z.enum([
    "postgres",
    "redis",
    "mysql",
    "mongodb",
    "rabbitmq",
    "elasticsearch",
    "minio",
  ]),
  serviceName: z.string().min(1),
  image: z.string().min(1),
  port: z.number().int().positive(),
  env: z.record(z.string()),
  connectionEnv: z.record(z.string()),
  volumeName: z.string().nullable(),
  volumeTarget: z.string().nullable(),
  reason: z.string().min(1),
});

const deploymentWorkspaceCandidateSchema = z.object({
  path: z.string().min(1),
  score: z.number(),
  runtimeKind: z.enum(["node", "python", "go", "java", "dotnet", "php", "ruby", "static", "unknown"]).optional(),
  stackKind: z.enum(["node", "python", "go", "java", "dotnet", "php", "ruby", "static", "unknown"]).optional(),
  framework: z.string().nullable(),
  packageManager: z.enum(["npm", "pnpm", "yarn", "bun", "pip", "poetry", "uv", "go", "maven", "gradle", "dotnet", "composer", "bundler"]).nullable(),
  signals: z.array(z.string()),
}).transform((candidate) => ({
  ...candidate,
  runtimeKind: candidate.runtimeKind ?? candidate.stackKind ?? "unknown",
}));

const deploymentWorkspaceSchema = z.object({
  appPath: z.string().min(1).default("."),
  isWorkspace: z.boolean().default(false),
  buildContextPath: z.string().min(1).default("."),
  reason: z.string().min(1).default("Workspace metadata was not recorded."),
  candidates: z.array(deploymentWorkspaceCandidateSchema).default([]),
}).default({
  appPath: ".",
  isWorkspace: false,
  buildContextPath: ".",
  reason: "Workspace metadata was not recorded.",
  candidates: [],
});

const deploymentEnvVariableSchema = z.object({
  name: z.string().min(1),
  required: z.boolean(),
  sensitive: z.boolean(),
  provided: z.boolean(),
  generated: z.boolean(),
  sources: z.array(z.enum([
    "runtime-default",
    "dependency-service",
    "generated-default",
    "env-example",
    "local-env-file",
    "source-code",
    "configuration",
    "framework",
    "runtime-contract",
  ])),
  reason: z.string().min(1),
});

const deploymentEnvDiagnosticsSchema = z.object({
  required: z.array(deploymentEnvVariableSchema).default([]),
  referenced: z.array(deploymentEnvVariableSchema).default([]),
  provided: z.array(z.string()).default([]),
  generated: z.record(z.string()).default({}),
  missing: z.array(deploymentEnvVariableSchema).default([]),
  localEnvFiles: z.array(z.object({
    path: z.string().min(1),
    variables: z.array(z.string()),
    ignored: z.boolean(),
  })).default([]),
  warnings: z.array(z.string()).default([]),
}).default({
  required: [],
  referenced: [],
  provided: [],
  generated: {},
  missing: [],
  localEnvFiles: [],
  warnings: [],
});

const deploymentBootstrapDiagnosticsSchema = z.object({
  tasks: z.array(z.object({
    kind: z.enum(["prisma", "django", "rails", "laravel", "flyway", "liquibase"]),
    command: z.string().min(1),
    automatic: z.boolean(),
    reason: z.string().min(1),
  })).default([]),
  warnings: z.array(z.string()).default([]),
}).default({
  tasks: [],
  warnings: [],
});

const deploymentComposeInfoSchema = z.object({
  selectedService: z.string().min(1).nullable(),
  serviceReason: z.string().min(1),
  services: z.array(z.object({
    name: z.string().min(1),
    score: z.number(),
    image: z.string().nullable(),
    build: z.boolean(),
    ports: z.array(z.object({
      hostPort: z.number().int().positive().nullable(),
      containerPort: z.number().int().positive(),
      protocol: z.string().nullable(),
      raw: z.string().min(1),
    })),
    expose: z.array(z.number().int().positive()),
    dependsOn: z.array(z.string()),
    profiles: z.array(z.string()),
    dependencyLike: z.boolean(),
    reason: z.string().min(1),
  })).default([]),
  warnings: z.array(z.string()).default([]),
}).default({
  selectedService: null,
  serviceReason: "Compose service metadata was not recorded.",
  services: [],
  warnings: [],
});

const deploymentFailureDiagnosticSchema = z.object({
  code: z.string().min(1),
  severity: z.enum(["error", "warning", "info"]),
  message: z.string().min(1),
  evidence: z.array(z.string()),
  suggestedAction: z.string().min(1),
});

const deploymentErrorWindowSchema = z.object({
  lines: z.array(z.string()),
  truncated: z.boolean(),
  totalLineCount: z.number().int().nonnegative(),
  matchedPatterns: z.array(z.string()),
});

const deploymentRuntimeContractSchema = z.object({
  source: z.enum(["accepted_aac", "previous_accepted_aac", "heuristic"]),
  ref: z.string().nullable(),
  status: z.enum(["modified", "unchanged", "not_applicable", "heuristic"]),
  dependencyServicePolicy: z.enum(["heuristic", "contract_only"]).default("heuristic"),
  deploymentShape: z.enum(["single-service", "frontend-and-backend"]).nullable().default("single-service"),
  runtimeKind: z.string().nullable(),
  buildCommand: z.string().nullable(),
  startCommand: z.string().nullable(),
  port: z.number().int().positive().nullable(),
  previewPath: z.string().min(1),
  healthPath: z.string().nullable(),
  apiPaths: z.array(z.string()),
  frontendOutputDir: z.string().nullable(),
  probeKind: z.enum(["http", "process", "command"]).default("http"),
  environment: z.object({
    required: z.array(z.string()).default([]),
    optional: z.array(z.string()).default([]),
  }).default({
    required: [],
    optional: [],
  }),
  frontend: z.object({
    required: z.boolean(),
    kind: z.string().nullable(),
    buildCommand: z.string().nullable(),
    sourceRoot: z.string().nullable(),
    outputDir: z.string().nullable(),
    servedBy: z.string().nullable(),
    servedByRef: z.string().nullable(),
  }).nullable().default(null),
  api: z.object({
    required: z.boolean(),
    kind: z.string().nullable(),
    buildCommand: z.string().nullable(),
    entry: z.string().nullable(),
    basePath: z.string().nullable(),
    probePaths: z.array(z.string()).default([]),
  }).nullable().default(null),
  dependencyServices: z.array(dependencyServiceSchema).default([]),
}).optional();

const deploymentSourceServiceSchema = z.object({
  serviceId: z.string().min(1),
  role: z.enum(["app", "frontend", "backend"]),
  root: z.string().min(1),
  workingDirectory: z.string().nullable(),
  workspacePackageJsonPaths: z.array(z.string()).default([]),
  runtimeKind: z.enum(["node", "python", "go", "java", "dotnet", "php", "ruby", "static", "unknown"]),
  packageManager: z.enum(["npm", "pnpm", "yarn", "bun", "pip", "poetry", "uv", "go", "maven", "gradle", "dotnet", "composer", "bundler"]).nullable(),
  hasLockfile: z.boolean(),
  framework: z.string().nullable(),
  runtimeVersion: z.string().nullable(),
  runtimeVersionSource: z.string().nullable(),
  buildCommand: z.string().nullable(),
  startCommand: z.string().nullable(),
  outputDirectory: z.string().nullable(),
  port: z.number().int().positive(),
  healthcheckPath: z.string().nullable(),
});

const deploymentSourceModelSchema = z.object({
  schemaVersion: z.literal(1),
  source: z.enum(["code-probe", "runtime-contract"]),
  shape: z.enum(["single-service", "frontend-and-backend"]),
  primaryServiceId: z.string().min(1),
  previewServiceId: z.string().min(1),
  buildContextPath: z.string().min(1),
  services: z.array(deploymentSourceServiceSchema),
  dependencies: z.array(dependencyServiceSchema),
  notes: z.array(z.string()).default([]),
});

const deploymentRouteSchema = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("static-spa"),
    publicPath: z.literal("/"),
    targetServiceId: z.string().min(1),
  }),
  z.object({
    kind: z.literal("http-proxy"),
    publicPath: z.string().min(1),
    targetServiceId: z.string().min(1),
    targetPort: z.number().int().positive(),
    preservePath: z.literal(true),
  }),
]);

const deploymentTopologySchema = z.object({
  schemaVersion: z.literal(1),
  publicEntryServiceId: z.string().min(1),
  routes: z.array(deploymentRouteSchema).default([]),
  validation: z.object({
    previewPaths: z.array(z.string().min(1)).default(["/"]),
    apiPaths: z.array(z.string().min(1)).default([]),
  }).default({
    previewPaths: ["/"],
    apiPaths: [],
  }),
}).optional();

const deploymentCodeEvidenceSummarySchema = z.object({
  ref: z.string().min(1),
  fingerprint: z.string().min(1),
  technicalBaselineRef: z.string().min(1).nullable(),
  runtimeFacts: z.object({
    web: z.string().nullable(),
    backend: z.string().nullable(),
    fullstack: z.string().nullable(),
  }),
  dependencyServices: z.array(z.object({
    kind: z.enum([
      "postgres",
      "redis",
      "mysql",
      "mongodb",
      "rabbitmq",
      "elasticsearch",
      "minio",
    ]),
    serviceName: z.string().min(1),
    reason: z.string().min(1),
  })).default([]),
  embeddedStores: z.array(z.string()).default([]),
  warningCount: z.number().int().nonnegative(),
  conflictCount: z.number().int().nonnegative(),
  missingFactCount: z.number().int().nonnegative(),
}).optional();

const legacyProbeInputKey = ["detected", "Stack"].join("");

function normalizeLegacyDeploymentSpecInput(value: unknown): unknown {
  if (!isRecord(value)) {
    return value;
  }
  const normalized = { ...value };
  if (!("legacyCodeProbe" in normalized) && legacyProbeInputKey in normalized) {
    normalized.legacyCodeProbe = normalized[legacyProbeInputKey];
  }
  delete normalized[legacyProbeInputKey];
  return normalized;
}

const deploymentSpecSchema = z.preprocess(normalizeLegacyDeploymentSpecInput, z.object({
  schemaVersion: z.literal(1),
  provider: z.enum(["compose-existing", "dockerfile-existing", "dockerfile-template"]),
  providerReason: z
    .string()
    .min(1)
    .default("Provider selected before strategy metadata was recorded."),
  providerPolicy: z.object({
    provider: z.enum(["compose-existing", "dockerfile-existing", "dockerfile-template"]).optional(),
    reuseExisting: z.boolean(),
    forceGenerate: z.boolean(),
  }).default({
    reuseExisting: true,
    forceGenerate: false,
  }),
  providerCandidates: z.array(z.object({
    provider: z.enum([
      "compose-existing",
      "dockerfile-existing",
      "dockerfile-template",
    ]),
    status: z.enum(["selected", "available", "skipped"]),
    reason: z.string().min(1),
    commands: z.array(z.array(z.string())),
  })).default([]),
  serviceName: z.string().min(1),
  imageName: z.string().min(1),
  projectRoot: z.string().min(1),
  generatedAt: z.string().datetime(),
  workspace: deploymentWorkspaceSchema,
  legacyCodeProbe: legacyCodeProbeSchema.optional(),
  environment: deploymentEnvDiagnosticsSchema,
  bootstrap: deploymentBootstrapDiagnosticsSchema,
  compose: deploymentComposeInfoSchema,
  runtimeContract: deploymentRuntimeContractSchema,
  sourceModel: deploymentSourceModelSchema.optional(),
  topology: deploymentTopologySchema,
  codeEvidence: deploymentCodeEvidenceSummarySchema,
  files: z.object({
    dockerfilePath: z.string().min(1).nullable(),
    dockerfilePaths: z.record(z.string()).default({}),
    nginxConfigPaths: z.record(z.string()).default({}),
    composePath: z.string().min(1),
    dockerignorePath: z.string().min(1).nullable(),
    buildContextPath: z.string().min(1).default("."),
    generated: z.boolean(),
    reused: z.array(z.string()),
  }),
  runtime: z.object({
    containerPort: z.number().int().positive(),
    hostPort: z.number().int().positive(),
    url: z.string().min(1),
    healthcheck: z
      .object({
        enabled: z.boolean(),
        path: z.string().min(1),
        candidates: z.array(z.string().min(1)).default(["/", "/health", "/healthz", "/api/health", "/ready"]),
        url: z.string().min(1).nullable(),
        expectedStatusMax: z.number().int().positive(),
        attempts: z.number().int().positive(),
        intervalMs: z.number().int().nonnegative(),
        timeoutMs: z.number().int().positive(),
      })
      .default({
        enabled: true,
        path: "/",
        candidates: ["/", "/health", "/healthz", "/api/health", "/ready"],
        url: null,
        expectedStatusMax: 499,
        attempts: 12,
        intervalMs: 1_000,
        timeoutMs: 2_000,
      }),
  }),
  commands: z.object({
    build: z.array(z.string()),
    up: z.array(z.string()),
    down: z.array(z.string()),
    logs: z.array(z.string()),
    status: z.array(z.string()),
  }),
})).transform(({ legacyCodeProbe, ...spec }) => {
  const runtimeContract = spec.runtimeContract ?? {
    source: "heuristic" as const,
    ref: null,
    status: "heuristic" as const,
    dependencyServicePolicy: "heuristic" as const,
    deploymentShape: "single-service" as const,
    runtimeKind: legacyCodeProbe?.framework ?? legacyCodeProbe?.kind ?? null,
    buildCommand: legacyCodeProbe?.buildCommand ?? null,
    startCommand: legacyCodeProbe?.startCommand ?? null,
    port: legacyCodeProbe?.port ?? null,
    previewPath: "/",
    healthPath: legacyCodeProbe?.healthcheckPath ?? null,
    apiPaths: [],
    frontendOutputDir: legacyCodeProbe?.outputDirectory ?? null,
    probeKind: legacyCodeProbe?.startCommand ? "http" as const : "process" as const,
    environment: {
      required: [],
      optional: [],
    },
    frontend: null,
    api: null,
    dependencyServices: [],
  };
  const sourceModel = spec.sourceModel ?? sourceModelFromLegacySpec({ files: spec.files, legacyCodeProbe });
  const files = {
    ...spec.files,
    dockerfilePaths: Object.keys(spec.files.dockerfilePaths).length > 0
      ? spec.files.dockerfilePaths
      : spec.files.dockerfilePath && legacyCodeProbe
        ? { app: spec.files.dockerfilePath }
        : {},
    nginxConfigPaths: spec.files.nginxConfigPaths,
  };
  return {
    ...spec,
    runtimeContract,
    sourceModel,
    topology: spec.topology ?? buildDeploymentTopology({ runtimeContract, sourceModel }),
    files,
  };
});

const deploymentStateSchema = z.object({
  schemaVersion: z.literal(1),
  provider: z.enum(["compose-existing", "dockerfile-existing", "dockerfile-template"]),
  serviceName: z.string().min(1),
  appServiceName: z.string().min(1).nullable().default(null),
  imageName: z.string().min(1),
  projectRoot: z.string().min(1),
  specPath: z.string().min(1),
  composePath: z.string().min(1),
  containerName: z.string().nullable(),
  containerId: z.string().nullable(),
  running: z.boolean(),
  url: z.string().nullable(),
  health: healthSchema.default({
    status: "unknown",
    url: null,
    checkedAt: null,
    statusCode: null,
    error: null,
  }),
  startedAt: z.string().datetime().nullable(),
  updatedAt: z.string().datetime(),
});

const deploymentRepairRequestSchema = z.object({
  schemaVersion: z.literal(1),
  repairId: z.string().min(1),
  createdAt: z.string().datetime(),
  projectRoot: z.string().min(1),
  specPath: z.string().min(1),
  provider: z.enum(["compose-existing", "dockerfile-existing", "dockerfile-template"]),
  failureKind: z.enum([
    "docker_unavailable",
    "compose_config",
    "registry_network",
    "image_build",
    "container_start",
    "healthcheck",
    "runtime_contract_missing",
    "runtime_contract_not_applicable",
    "runtime_contract_mismatch",
    "build_command_failed",
    "start_command_failed",
    "application_startup_failed",
    "http_probe_failed",
    "preview_not_verified",
    "api_route_not_verified",
    "deploy_asset_invalid",
    "logs",
    "unknown",
  ]),
  failureOwner: z.enum(["application_code", "deployment_assets", "environment", "external_system", "unknown"]).optional(),
  repairRoute: z.enum(["execution_repair", "deploy_repair", "manual_review", "none"]).optional(),
  failureRef: z.string().min(1).nullable().optional(),
  command: z.array(z.string()),
  exitCode: z.number().int(),
  fullLogRef: z.string().min(1).nullable().optional(),
  errorWindow: deploymentErrorWindowSchema.optional(),
  stdoutTail: z.array(z.string()),
  stderrTail: z.array(z.string()),
  providerCandidates: z.array(z.object({
    provider: z.enum([
      "compose-existing",
      "dockerfile-existing",
      "dockerfile-template",
    ]),
    status: z.enum(["selected", "available", "skipped"]),
    reason: z.string().min(1),
    commands: z.array(z.array(z.string())),
  })).default([]),
  environment: deploymentEnvDiagnosticsSchema,
  bootstrap: deploymentBootstrapDiagnosticsSchema,
  diagnostics: z.array(deploymentFailureDiagnosticSchema).default([]),
  suggestedActions: z.array(z.string()).default([]),
  editableFiles: z.array(z.string()),
  protectedFiles: z.array(z.string()),
  instruction: z.string().min(1),
  maxAttempts: z.number().int().positive(),
  attempts: z.number().int().nonnegative(),
  status: z.literal("pending"),
});

const deploymentFailureOwnerSchema = z.enum([
  "application_code",
  "deployment_assets",
  "environment",
  "external_system",
  "unknown",
]);

const deploymentRepairRouteSchema = z.enum([
  "execution_repair",
  "deploy_repair",
  "manual_review",
  "none",
]);

const deploymentFailureKindSchema = z.enum([
  "docker_unavailable",
  "compose_config",
  "registry_network",
  "image_build",
  "container_start",
  "healthcheck",
  "runtime_contract_missing",
  "runtime_contract_not_applicable",
  "runtime_contract_mismatch",
  "build_command_failed",
  "start_command_failed",
  "application_startup_failed",
  "http_probe_failed",
  "preview_not_verified",
  "api_route_not_verified",
  "deploy_asset_invalid",
  "logs",
  "unknown",
]);

const deploymentSourceRefsSchema = z.object({
  runtimeDeliveryRef: z.string().min(1).nullable(),
  taskPlanRef: z.string().min(1).nullable(),
  taskPlanRunRef: z.string().min(1).nullable(),
  reviewResultRef: z.string().min(1).nullable(),
  deploymentSpecRef: z.string().min(1),
});

const deploymentFailureReportSchema = z.object({
  schemaVersion: z.literal("1.0"),
  failureId: z.string().min(1),
  source: z.literal("deploy"),
  createdAt: z.string().datetime(),
  deploymentAttemptId: z.string().min(1),
  failureKind: deploymentFailureKindSchema,
  failureOwner: deploymentFailureOwnerSchema,
  repairRoute: deploymentRepairRouteSchema,
  runtimeDeliveryRef: z.string().min(1).nullable(),
  sourceRefs: deploymentSourceRefsSchema,
  failedContract: z.object({
    field: z.string().min(1),
    command: z.string().min(1).nullable(),
    workingDirectory: z.string().min(1),
  }),
  evidence: z.object({
    failedAt: z.string().min(1),
    deployCommand: z.array(z.string()),
    exitCode: z.number().int(),
    fullLogRef: z.string().min(1).nullable().optional(),
    errorWindow: deploymentErrorWindowSchema.optional(),
    stdoutTail: z.array(z.string()),
    stderrTail: z.array(z.string()),
    logMarkers: z.array(z.string()),
    diagnostics: z.array(deploymentFailureDiagnosticSchema),
  }),
  routing: z.object({
    editableBoundary: z.enum(["application_code_only", "deployment_assets_only", "manual_only", "none"]),
    mustNotEdit: z.array(z.string()),
    nextCommand: z.object({
      name: z.string().min(1),
      argv: z.array(z.string()),
    }).nullable(),
  }),
  loopGuard: z.object({
    signature: z.string().min(1),
    attempt: z.number().int().positive(),
    maxAttempts: z.number().int().positive(),
  }),
});

const deployExecutionRepairRequestSchema = z.object({
  schemaVersion: z.literal("1.0"),
  repairId: z.string().min(1),
  repairType: z.literal("execution_repair"),
  source: z.literal("deploy_failure"),
  deploymentFailureRef: z.string().min(1),
  agentAction: z.record(z.unknown()).optional(),
  sourceRefs: deploymentSourceRefsSchema,
  referencedArtifactReadGuide: z.array(z.record(z.unknown())).optional(),
  syntheticTask: z.object({
    taskId: z.string().min(1),
    taskKind: z.literal("runtime_delivery"),
    title: z.string().min(1),
    objective: z.string().min(1),
    mutatesOriginalTaskPlan: z.literal(false),
    relatedTaskIds: z.array(z.string()),
    writeBoundary: z.object({
      forbiddenPaths: z.array(z.string()),
    }),
    runtimeDeliveryRequirement: z.object({
      appliesToThisTask: z.literal(true),
      source: z.literal("deploy_failure"),
      deploymentFailureRef: z.string().min(1),
      runtimeDeliveryRef: z.string().min(1).nullable(),
      affectedContractFields: z.array(z.string().min(1)).min(1),
      requiredCodeLevelChecks: z.array(z.object({
        checkId: z.string().min(1),
        contractField: z.string().min(1),
        objective: z.string().min(1),
        acceptableEvidence: z.array(z.string().min(1)),
      })).min(1),
      forbiddenActions: z.array(z.string().min(1)),
    }),
  }),
  executionRules: z.record(z.unknown()),
  outputContract: z.object({
    format: z.literal("json"),
    schema: z.literal("DeployExecutionRepairTaskResult"),
    resultFile: z.string().min(1),
    schemaShape: z.record(z.unknown()),
    submitCommand: z.object({
      name: z.string().min(1),
      argv: z.array(z.string()),
    }),
  }),
  createdAt: z.string().datetime(),
});

const deployExecutionRepairTaskResultSchema = z.object({
  schemaVersion: z.literal("1.0"),
  repairId: z.string().min(1),
  status: z.enum(["completed", "completed_with_notes", "blocked", "failed"]),
  deploymentFailureRef: z.string().min(1),
  changedFiles: z.array(z.string().min(1)),
  runtimeDeliveryEvidence: z.object({
    source: z.literal("deploy_failure_repair"),
    addressedFailedContractFields: z.array(z.string().min(1)),
    codeLevelChecks: z.array(z.object({
      checkId: z.string().min(1),
      status: z.enum(["passed", "failed", "blocked", "not_applicable"]),
      evidence: z.string().min(1),
    })),
    commandsRun: z.array(z.object({
      command: z.string().min(1),
      status: z.enum(["passed", "failed", "not_run"]),
      environment: z.string().min(1),
      summary: z.string().min(1).optional(),
    })),
    unverifiedItems: z.array(z.object({
      item: z.string().min(1),
      reason: z.string().min(1),
    })),
    runtimeProbeCleanup: z.object({
      temporaryRuntimeStarted: z.boolean(),
      attempted: z.boolean(),
      status: z.enum(["not_needed", "succeeded", "failed", "unknown", "not_safe_to_cleanup"]),
      targets: z.array(z.object({
        kind: z.enum(["process", "port", "container", "dev_server", "other"]),
        pid: z.number().int().positive().nullable().optional(),
        port: z.number().int().positive().nullable().optional(),
        command: z.string().min(1).nullable().optional(),
        summary: z.string().min(1),
      })).optional(),
      summary: z.string().min(1),
    }).optional(),
  }),
  selfRepairSummary: z.object({
    attempted: z.boolean(),
    attemptCount: z.number().int().nonnegative(),
    stopReason: z.string().min(1),
    progressObserved: z.boolean(),
  }),
  notes: z.array(z.string()),
});

function sourceModelFromLegacySpec(spec: {
  files: { buildContextPath: string };
  legacyCodeProbe?: z.infer<typeof legacyCodeProbeSchema>;
}) {
  const probe = spec.legacyCodeProbe;
  if (!probe) {
    return {
      schemaVersion: 1 as const,
      source: "code-probe" as const,
      shape: "single-service" as const,
      primaryServiceId: "app",
      previewServiceId: "app",
      buildContextPath: spec.files.buildContextPath,
      services: [],
      dependencies: [],
      notes: ["Legacy deployment spec did not contain a source model."],
    };
  }
  return {
    schemaVersion: 1 as const,
    source: "code-probe" as const,
    shape: "single-service" as const,
    primaryServiceId: "app",
    previewServiceId: "app",
    buildContextPath: spec.files.buildContextPath,
    services: [{
      serviceId: "app",
      role: "app" as const,
      root: probe.workingDirectory ?? ".",
      workingDirectory: probe.workingDirectory,
      workspacePackageJsonPaths: probe.workspacePackageJsonPaths,
      runtimeKind: probe.kind,
      packageManager: probe.packageManager,
      hasLockfile: probe.hasLockfile,
      framework: probe.framework,
      runtimeVersion: probe.runtimeVersion,
      runtimeVersionSource: probe.runtimeVersionSource,
      buildCommand: probe.buildCommand,
      startCommand: probe.startCommand,
      outputDirectory: probe.outputDirectory,
      port: probe.port,
      healthcheckPath: probe.healthcheckPath ?? null,
    }],
    dependencies: probe.services,
    notes: ["Deployment source model migrated from a legacy deployment spec."],
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export async function readDeploymentSpec(projectRoot: string): Promise<DeploymentSpec> {
  const paths = getDeploymentPaths(projectRoot);
  try {
    const raw = await readJsonWithSchemaVersion(paths.specFile);
    return deploymentSpecSchema.parse(raw);
  } catch (error) {
    if (isMissingFileError(error)) {
      throw deployNotPrepared(projectRoot);
    }
    if (error instanceof ZodError) {
      throw stateCorrupted("Deployment spec does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

export async function writeDeploymentSpec(projectRoot: string, spec: DeploymentSpec): Promise<void> {
  const paths = getDeploymentPaths(projectRoot);
  await writeJsonAtomic(paths.specFile, spec);
}

export async function readDeploymentState(projectRoot: string): Promise<DeploymentState | null> {
  const paths = getDeploymentPaths(projectRoot);
  try {
    const raw = await readJsonWithSchemaVersion(paths.stateFile);
    return deploymentStateSchema.parse(raw);
  } catch (error) {
    if (isMissingFileError(error)) {
      return null;
    }
    if (error instanceof ZodError) {
      throw stateCorrupted("Deployment state does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

export async function writeDeploymentState(projectRoot: string, state: DeploymentState): Promise<void> {
  const paths = getDeploymentPaths(projectRoot);
  await writeJsonAtomic(paths.stateFile, state);
}

export async function clearDeploymentRepairRequest(projectRoot: string): Promise<boolean> {
  const paths = getDeploymentPaths(projectRoot);
  try {
    await fs.unlink(paths.repairFile);
    return true;
  } catch (error) {
    if (isMissingFileError(error)) {
      return false;
    }
    throw error;
  }
}

export async function clearDeploymentFailureArtifacts(projectRoot: string): Promise<{
  repairRequestCleared: boolean;
  failureReportCleared: boolean;
}> {
  const paths = getDeploymentPaths(projectRoot);
  const repairRequestCleared = await unlinkIfExists(paths.repairFile);
  const failureReportCleared = await unlinkIfExists(paths.failureFile);
  return { repairRequestCleared, failureReportCleared };
}

async function unlinkIfExists(filePath: string): Promise<boolean> {
  try {
    await fs.unlink(filePath);
    return true;
  } catch (error) {
    if (isMissingFileError(error)) {
      return false;
    }
    throw error;
  }
}

export async function readDeploymentFailureReport(
  projectRoot: string,
  failureRef?: string,
): Promise<DeploymentFailureReport | null> {
  const paths = getDeploymentPaths(projectRoot);
  const file = failureRef ? path.resolve(projectRoot, failureRef) : paths.failureFile;
  try {
    const raw = await readJsonFile(file);
    return deploymentFailureReportSchema.parse(raw);
  } catch (error) {
    if (isMissingFileError(error)) {
      return null;
    }
    if (error instanceof ZodError) {
      throw stateCorrupted("Deployment failure report does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

export async function writeDeploymentFailureReport(
  projectRoot: string,
  report: DeploymentFailureReport,
): Promise<void> {
  const paths = getDeploymentPaths(projectRoot);
  await writeJsonAtomic(paths.failureFile, deploymentFailureReportSchema.parse(report));
}

export async function readDeploymentRepairRequest(
  projectRoot: string,
): Promise<DeploymentRepairRequest | null> {
  const paths = getDeploymentPaths(projectRoot);
  try {
    const raw = await readJsonWithSchemaVersion(paths.repairFile);
    const request = deploymentRepairRequestSchema.parse(raw);
    return {
      ...request,
      maxAttempts: Math.max(request.maxAttempts, DEFAULT_DEPLOY_REPAIR_MAX_ATTEMPTS),
    };
  } catch (error) {
    if (isMissingFileError(error)) {
      return null;
    }
    if (error instanceof ZodError) {
      throw stateCorrupted("Deployment repair request does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

export async function readDeployExecutionRepairRequest(
  projectRoot: string,
  repairId: string,
): Promise<DeployExecutionRepairRequest> {
  const paths = getDeploymentRepairPaths(projectRoot, repairId);
  try {
    const raw = await hydrateRequestManifest(projectRoot, paths.requestFile);
    return deployExecutionRepairRequestSchema.parse(raw);
  } catch (error) {
    if (error instanceof ZodError) {
      throw stateCorrupted("Deploy execution repair request does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

export async function writeDeployExecutionRepairRequest(
  projectRoot: string,
  request: DeployExecutionRepairRequest,
): Promise<void> {
  const paths = getDeploymentRepairPaths(projectRoot, request.repairId);
  await writeRequestManifestAtomic(projectRoot, paths.requestFile, deployExecutionRepairRequestSchema.parse(request));
}

export async function readDeployExecutionRepairTaskResult(
  projectRoot: string,
  resultFile: string,
  request?: DeployExecutionRepairRequest,
): Promise<DeployExecutionRepairTaskResult> {
  try {
    const raw = await readJsonFile(path.resolve(projectRoot, resultFile));
    return deployExecutionRepairTaskResultSchema.parse(normalizeDeployExecutionRepairMachineOwnedFields(raw, request));
  } catch (error) {
    if (error instanceof ZodError) {
      throw stateCorrupted("Deploy execution repair task result does not match schema.", {
        issues: error.issues.map((issue) => ({
          path: issue.path.join("."),
          message: issue.message,
        })),
      });
    }
    throw error;
  }
}

function normalizeDeployExecutionRepairMachineOwnedFields(
  raw: unknown,
  request?: DeployExecutionRepairRequest,
): unknown {
  if (!request || !isPlainRecord(raw)) {
    return raw;
  }
  const normalized: Record<string, unknown> = {
    ...raw,
    schemaVersion: "1.0",
    repairId: request.repairId,
    deploymentFailureRef: request.deploymentFailureRef,
  };
  if (!Array.isArray(normalized.changedFiles)) {
    normalized.changedFiles = [];
  }
  if (!Array.isArray(normalized.notes)) {
    normalized.notes = [];
  }
  if (!isPlainRecord(normalized.selfRepairSummary)) {
    normalized.selfRepairSummary = {
      attempted: false,
      attemptCount: 0,
      stopReason: "not_attempted",
      progressObserved: false,
    };
  }
  const rawRuntime = isPlainRecord(normalized.runtimeDeliveryEvidence) ? normalized.runtimeDeliveryEvidence : {};
  const requiredChecks = request.syntheticTask.runtimeDeliveryRequirement.requiredCodeLevelChecks;
  const rawChecks = Array.isArray(rawRuntime.codeLevelChecks) ? rawRuntime.codeLevelChecks.filter(isPlainRecord) : [];
  const usedIndexes = new Set<number>();
  const codeLevelChecks = requiredChecks
    .map((check, index) => {
      const matchingIndex = rawChecks.findIndex((item, itemIndex) =>
        !usedIndexes.has(itemIndex) && (
          item.checkId === check.checkId ||
          item.contractField === check.contractField
        )
      );
      const rawIndex = matchingIndex >= 0 ? matchingIndex : index;
      const rawCheck = rawChecks[rawIndex];
      if (!rawCheck) {
        return null;
      }
      usedIndexes.add(rawIndex);
      return {
        ...rawCheck,
        checkId: check.checkId,
      };
    })
    .filter((item) => item !== null);
  const extraChecks = rawChecks.filter((_, index) => !usedIndexes.has(index));
  normalized.runtimeDeliveryEvidence = {
    ...rawRuntime,
    source: "deploy_failure_repair",
    addressedFailedContractFields: request.syntheticTask.runtimeDeliveryRequirement.affectedContractFields,
    codeLevelChecks: [...codeLevelChecks, ...extraChecks],
    commandsRun: Array.isArray(rawRuntime.commandsRun) ? rawRuntime.commandsRun : [],
    unverifiedItems: Array.isArray(rawRuntime.unverifiedItems) ? rawRuntime.unverifiedItems : [],
  };
  return normalized;
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export async function appendDeploymentLog(projectRoot: string, content: string): Promise<void> {
  const paths = getDeploymentPaths(projectRoot);
  await fs.mkdir(path.dirname(paths.logFile), { recursive: true });
  await fs.appendFile(paths.logFile, content, "utf8");
}

function isMissingFileError(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}
