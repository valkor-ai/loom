import path from "node:path";
import type {
  DependencyService,
  DeploymentBootstrapDiagnostics,
  DeploymentComposeInfo,
  DeploymentRuntimeContract,
  DeploymentTopology,
  DeploymentSourceModel,
  DeploymentSourceService,
  DeployProvider,
  DeploymentProviderPolicy,
  DeploymentEnvDiagnostics,
  DeploymentProviderCandidate,
  DeploymentWorkspace,
  DeploymentSpec,
  DeploymentCodeEvidenceSummary,
} from "./types";
import { toProjectRelative } from "../state/paths";
import { buildDeploymentTopology, proxyRoutesForPublicEntry, proxyTargetServiceIdsForPublicEntry } from "./topology";

export type GeneratedDeploymentFiles = {
  dockerfiles: Record<string, string>;
  nginxConfigs: Record<string, string>;
  compose: string;
  dockerignore: string;
};

export function createDeploymentSpec(input: {
  projectRoot: string;
  deploymentRoot: string;
  buildContextRoot: string;
  workspace: DeploymentWorkspace;
  provider: DeployProvider;
  providerReason: string;
  providerPolicy: DeploymentProviderPolicy;
  providerCandidates: DeploymentProviderCandidate[];
  runtimeContract: DeploymentRuntimeContract;
  sourceModel: DeploymentSourceModel;
  topology?: DeploymentTopology;
  environment: DeploymentEnvDiagnostics;
  bootstrap: DeploymentBootstrapDiagnostics;
  compose?: DeploymentComposeInfo;
  codeEvidence?: DeploymentCodeEvidenceSummary;
  dockerfilePaths: Record<string, string>;
  nginxConfigPaths?: Record<string, string>;
  composePath: string;
  dockerignorePath: string;
  generated: boolean;
  reused: string[];
  hostPort: number;
}): DeploymentSpec {
  const serviceName = sanitizeName(path.basename(input.deploymentRoot));
  const imageName = `${serviceName}:loom-local`;
  const composePath = toProjectRelative(input.projectRoot, input.composePath);
  const dockerfilePaths = Object.fromEntries(Object.entries(input.dockerfilePaths).map(([serviceId, filePath]) => [
    serviceId,
    toProjectRelative(input.projectRoot, filePath),
  ]));
  const nginxConfigPaths = Object.fromEntries(Object.entries(input.nginxConfigPaths ?? {}).map(([serviceId, filePath]) => [
    serviceId,
    toProjectRelative(input.projectRoot, filePath),
  ]));
  const primaryService = primaryServiceFor(input.sourceModel);
  const previewService = previewServiceFor(input.sourceModel);
  const topology = input.topology ?? buildDeploymentTopology({
    runtimeContract: input.runtimeContract,
    sourceModel: input.sourceModel,
  });
  const dockerfilePath = dockerfilePaths[primaryService.serviceId] ?? Object.values(dockerfilePaths)[0] ?? null;
  const buildContextPath = toProjectRelative(input.projectRoot, input.buildContextRoot) || ".";
  const healthcheckPath = previewService.healthcheckPath ?? "/";
  const healthcheckEnabled = previewService.startCommand !== null || input.provider === "dockerfile-template";
  const baseUrl = `http://localhost:${input.hostPort}`;

  return {
    schemaVersion: 1,
    provider: input.provider,
    providerReason: input.providerReason,
    providerPolicy: input.providerPolicy,
    providerCandidates: input.providerCandidates,
    serviceName,
    imageName,
    projectRoot: input.projectRoot,
    generatedAt: new Date().toISOString(),
    workspace: input.workspace,
    environment: input.environment,
    bootstrap: input.bootstrap,
    compose: input.compose ?? generatedComposeInfo(input.sourceModel, topology, input.hostPort),
    ...(input.codeEvidence ? { codeEvidence: input.codeEvidence } : {}),
    runtimeContract: input.runtimeContract,
    sourceModel: input.sourceModel,
    topology,
    files: {
      dockerfilePath,
      dockerfilePaths,
      nginxConfigPaths,
      composePath,
      dockerignorePath: toProjectRelative(input.projectRoot, input.dockerignorePath),
      buildContextPath,
      generated: input.generated,
      reused: input.reused,
    },
    runtime: {
      containerPort: previewService.port,
      hostPort: input.hostPort,
      url: `http://localhost:${input.hostPort}`,
      healthcheck: {
        enabled: healthcheckEnabled,
        path: healthcheckPath,
        candidates: healthcheckCandidatesFor(previewService),
        url: healthcheckEnabled ? `${baseUrl}${healthcheckPath}` : null,
        expectedStatusMax: 399,
        attempts: 12,
        intervalMs: 1_000,
        timeoutMs: 2_000,
      },
    },
    commands: {
      build: ["docker", "compose", "-f", composePath, "build"],
      up: ["docker", "compose", "-f", composePath, "up", "-d", "--build"],
      down: ["docker", "compose", "-f", composePath, "down"],
      logs: ["docker", "compose", "-f", composePath, "logs", "--tail", "120"],
      status: ["docker", "compose", "-f", composePath, "ps"],
    },
  };
}

function generatedComposeInfo(sourceModel: DeploymentSourceModel, topology: DeploymentTopology, hostPort: number): DeploymentComposeInfo {
  return {
    selectedService: previewServiceFor(sourceModel).serviceId,
    serviceReason: "Generated Compose uses the deployment source model preview service.",
    services: [
      ...sourceModel.services.map((service) => ({
        name: service.serviceId,
        score: 100,
        image: null,
        build: true,
        ports: service.serviceId === sourceModel.previewServiceId ? [
          {
            hostPort,
            containerPort: service.port,
            protocol: "tcp",
            raw: String(service.port),
          },
        ] : [],
        expose: [],
        dependsOn: composeDependsOnForService(sourceModel, topology, service),
        profiles: [],
        dependencyLike: false,
        reason: "Generated application service from deployment source model.",
      })),
    ],
    warnings: [],
  };
}

function primaryServiceFor(sourceModel: DeploymentSourceModel): DeploymentSourceService {
  return sourceModel.services.find((service) => service.serviceId === sourceModel.primaryServiceId) ?? sourceModel.services[0];
}

function previewServiceFor(sourceModel: DeploymentSourceModel): DeploymentSourceService {
  return sourceModel.services.find((service) => service.serviceId === sourceModel.previewServiceId) ?? primaryServiceFor(sourceModel);
}

export function generateDeploymentFiles(spec: DeploymentSpec): GeneratedDeploymentFiles {
  if (Object.keys(spec.files.dockerfilePaths).length === 0) {
    throw new Error("Cannot generate deployment files without service Dockerfile paths.");
  }

  return {
    dockerfiles: Object.fromEntries(spec.sourceModel.services.map((service) => [
      service.serviceId,
      generateDockerfile(service, spec),
    ])),
    nginxConfigs: Object.fromEntries(Object.keys(spec.files.nginxConfigPaths).map((serviceId) => [
      serviceId,
      generateNginxConfig(spec),
    ])),
    compose: generateCompose(spec),
    dockerignore: generateDockerignore(),
  };
}

function healthcheckCandidatesFor(stack: DeploymentSourceService): string[] {
  const common = ["/", "/health", "/healthz", "/api/health", "/ready", "/readiness"];
  const detected = stack.healthcheckPath ? [stack.healthcheckPath] : [];
  switch (stack.framework) {
    case "fastapi":
    case "flask":
    case "django":
    case "stdlib-http":
      return dedupeStrings([...detected, "/health", "/healthz", "/ready", "/", ...common]);
    case "spring-boot":
      return dedupeStrings([...detected, "/actuator/health", "/health", "/ready", "/", ...common]);
    case "laravel":
    case "rails":
      return dedupeStrings([...detected, "/up", "/health", "/", ...common]);
    case "next":
      return dedupeStrings([...detected, "/api/health", "/health", "/", ...common]);
    default:
      return dedupeStrings([...detected, ...common]);
  }
}

function generateDockerfile(stack: DeploymentSourceService, spec: DeploymentSpec): string {
  if (stack.runtimeKind === "static" || (stack.role === "frontend" && !stack.startCommand)) {
    return generateStaticFrontendDockerfile(stack, spec);
  }

  if (stack.runtimeKind === "node") {
    return generateNodeDockerfile(stack);
  }

  if (stack.runtimeKind === "python") {
    return generatePythonDockerfile(stack);
  }

  if (stack.runtimeKind === "go") {
    return generateGoDockerfile(stack);
  }

  if (stack.runtimeKind === "java") {
    return generateJavaDockerfile(stack);
  }

  if (stack.runtimeKind === "dotnet") {
    return generateDotnetDockerfile(stack);
  }

  if (stack.runtimeKind === "php") {
    return generatePhpDockerfile(stack);
  }

  if (stack.runtimeKind === "ruby") {
    return generateRubyDockerfile(stack);
  }

  return [
    "FROM alpine:3.20",
    "WORKDIR /app",
    "COPY . .",
    "CMD [\"sh\", \"-c\", \"echo 'loom could not detect a runnable stack for this project.' && exit 64\"]",
    "",
  ].join("\n");
}

function generateStaticFrontendDockerfile(stack: DeploymentSourceService, spec: DeploymentSpec): string {
  const packageManager = stack.packageManager ?? "npm";
  const installCommand = installCommandFor(packageManager, stack.hasLockfile);
  const outputDirectory = stack.outputDirectory ?? "dist";
  const buildCommand = stack.buildCommand ?? packageManagerRun(packageManager, "build");
  const nginxConfigPath = spec.files.nginxConfigPaths[stack.serviceId] ?? null;
  const nginxConfigCopy = nginxConfigPath
    ? [`COPY ${projectPathRelativeToDirectory(spec.files.buildContextPath, nginxConfigPath)} /etc/nginx/conf.d/default.conf`]
    : [];
  return [
    `FROM ${nodeBaseImageFor(stack)} AS builder`,
    "WORKDIR /workspace",
    "COPY . .",
    ...(stack.root !== "." ? [`WORKDIR /workspace/${stack.root}`] : []),
    `RUN ${installCommand}`,
    ...(buildCommand ? [`RUN ${buildCommand}`] : []),
    "",
    "FROM nginx:1.27-alpine AS runner",
    ...nginxConfigCopy,
    `COPY --from=builder /workspace/${outputDirectory} /usr/share/nginx/html`,
    "EXPOSE 80",
    "",
  ].join("\n");
}

function generateNodeDockerfile(stack: DeploymentSourceService): string {
  const installCommand = installCommandFor(stack.packageManager ?? "npm", stack.hasLockfile);
  const lockfileCopy = lockfileCopyFor(stack.packageManager ?? "npm");
  const baseImage = nodeBaseImageFor(stack);
  const buildLines = stack.buildCommand ? [`RUN ${stack.buildCommand}`] : [];
  const startCommand = stack.startCommand ?? "echo 'loom cannot start this Node project because no start script was detected.' && exit 64";

  return [
    `FROM ${baseImage} AS deps`,
    "WORKDIR /app",
    lockfileCopy,
    ...workspaceManifestCopyLines(stack),
    `RUN ${installCommand}`,
    "RUN mkdir -p node_modules",
    "",
    `FROM ${baseImage} AS runner`,
    "WORKDIR /app",
    "ENV NODE_ENV=production",
    "ENV NEXT_TELEMETRY_DISABLED=1",
    "COPY --from=deps /app/package*.json ./",
    "COPY --from=deps /app/node_modules ./node_modules",
    ...workspaceManifestCopyLines(stack, "--from=deps /app"),
    ...workspaceNodeModulesCopyLines(stack),
    "COPY . .",
    ...workingDirectoryLines(stack),
    ...buildLines,
    `EXPOSE ${stack.port}`,
    `CMD ${JSON.stringify(["sh", "-c", startCommand])}`,
    "",
  ].join("\n");
}

function generatePythonDockerfile(stack: DeploymentSourceService): string {
  const installLines = pythonInstallLines(stack.packageManager ?? "pip");
  const startCommand =
    stack.startCommand ??
    "echo 'loom cannot start this Python project because no runnable web command was detected.' && exit 64";

  return [
    "FROM python:3.12-slim AS runner",
    "WORKDIR /app",
    "ENV PYTHONDONTWRITEBYTECODE=1",
    "ENV PYTHONUNBUFFERED=1",
    `ENV PORT=${stack.port}`,
    ...installLines,
    "COPY . .",
    `EXPOSE ${stack.port}`,
    `CMD ${JSON.stringify(["sh", "-c", startCommand])}`,
    "",
  ].join("\n");
}

function generateGoDockerfile(stack: DeploymentSourceService): string {
  const startCommand = stack.startCommand ?? "/app/server";

  return [
    "FROM golang:1.23-alpine AS builder",
    "WORKDIR /src",
    "COPY go.mod go.sum* ./",
    "RUN go mod download",
    "COPY . .",
    "RUN CGO_ENABLED=0 GOOS=linux go build -o /out/server .",
    "",
    "FROM alpine:3.20 AS runner",
    "WORKDIR /app",
    "RUN adduser -D -H appuser",
    "COPY --from=builder /out/server /app/server",
    "USER appuser",
    `EXPOSE ${stack.port}`,
    `CMD ${JSON.stringify(["sh", "-c", startCommand])}`,
    "",
  ].join("\n");
}

function generateJavaDockerfile(stack: DeploymentSourceService): string {
  const javaVersion = stack.runtimeVersion ?? "21";
  const packageManager = stack.packageManager === "gradle" ? "gradle" : "maven";
  const builderImage = packageManager === "maven"
    ? `maven:3-eclipse-temurin-${javaVersion}`
    : `gradle:8-jdk${javaVersion}`;
  const buildCommand = stack.buildCommand ?? javaBuildCommand(packageManager);
  const startCommand = javaRuntimeStartCommand(stack.startCommand);

  return [
    `FROM ${builderImage} AS builder`,
    "WORKDIR /workspace",
    ...javaFrontendToolchainLines(buildCommand),
    "COPY . .",
    `RUN ${buildCommand}`,
    "RUN JAR=\"$(find . -type f -name '*.jar' ! -name '*-plain.jar' ! -name '*sources.jar' ! -name '*javadoc.jar' | sort | head -n 1)\" && test -n \"$JAR\" && cp \"$JAR\" /workspace/app.jar",
    "",
    `FROM eclipse-temurin:${javaVersion}-jre AS runner`,
    "WORKDIR /app",
    `ENV PORT=${stack.port}`,
    `ENV SERVER_PORT=${stack.port}`,
    "COPY --from=builder /workspace/app.jar /app/app.jar",
    `EXPOSE ${stack.port}`,
    `CMD ${JSON.stringify(["sh", "-c", startCommand])}`,
    "",
  ].join("\n");
}

function generateDotnetDockerfile(stack: DeploymentSourceService): string {
  const dotnetVersion = stack.runtimeVersion ?? "8";
  const runtimeImage = stack.framework === "aspnetcore"
    ? `mcr.microsoft.com/dotnet/aspnet:${dotnetVersion}`
    : `mcr.microsoft.com/dotnet/runtime:${dotnetVersion}`;
  const startCommand = stack.startCommand ?? "dotnet /app/app.dll";

  return [
    `FROM mcr.microsoft.com/dotnet/sdk:${dotnetVersion} AS build`,
    "WORKDIR /src",
    "COPY . .",
    "RUN dotnet restore",
    "RUN dotnet publish -c Release -o /app/publish --no-restore",
    "",
    `FROM ${runtimeImage} AS runner`,
    "WORKDIR /app",
    `ENV ASPNETCORE_URLS=http://0.0.0.0:${stack.port}`,
    `ENV PORT=${stack.port}`,
    "COPY --from=build /app/publish .",
    `EXPOSE ${stack.port}`,
    `CMD ${JSON.stringify(["sh", "-c", startCommand])}`,
    "",
  ].join("\n");
}

function generatePhpDockerfile(stack: DeploymentSourceService): string {
  const phpVersion = stack.runtimeVersion ?? "8.3";
  const startCommand = stack.startCommand ?? "php -S 0.0.0.0:${PORT:-8000} -t public public/index.php";

  return [
    `FROM php:${phpVersion}-cli AS runner`,
    "WORKDIR /app",
    "RUN apt-get update && apt-get install -y --no-install-recommends \\",
    "    git unzip libpq-dev libzip-dev \\",
    "  && docker-php-ext-install pdo pdo_mysql pdo_pgsql zip \\",
    "  && rm -rf /var/lib/apt/lists/*",
    "COPY --from=composer:2 /usr/bin/composer /usr/bin/composer",
    "COPY composer.json composer.lock* ./",
    "RUN composer install --no-dev --prefer-dist --no-interaction --optimize-autoloader --no-scripts",
    "COPY . .",
    ...(stack.framework === "laravel"
      ? [
          "RUN mkdir -p storage bootstrap/cache && chmod -R 775 storage bootstrap/cache",
          "RUN php artisan package:discover --ansi || true",
        ]
      : []),
    `ENV PORT=${stack.port}`,
    `EXPOSE ${stack.port}`,
    `CMD ${JSON.stringify(["sh", "-c", startCommand])}`,
    "",
  ].join("\n");
}

function generateRubyDockerfile(stack: DeploymentSourceService): string {
  const rubyVersion = stack.runtimeVersion ?? "3.3";
  const startCommand = stack.startCommand ?? "bundle exec rails server -b 0.0.0.0 -p ${PORT:-3000}";

  return [
    `FROM ruby:${rubyVersion}-slim AS runner`,
    "WORKDIR /app",
    "RUN apt-get update && apt-get install -y --no-install-recommends \\",
    "    build-essential git libpq-dev pkg-config \\",
    "  && rm -rf /var/lib/apt/lists/*",
    "COPY Gemfile Gemfile.lock* ./",
    "RUN bundle config set without 'development test' && bundle install",
    "COPY . .",
    ...(stack.framework === "rails"
      ? [
          "RUN mkdir -p tmp/pids tmp/cache log storage",
        ]
      : []),
    `ENV RAILS_ENV=production`,
    `ENV RACK_ENV=production`,
    `ENV PORT=${stack.port}`,
    `EXPOSE ${stack.port}`,
    `CMD ${JSON.stringify(["sh", "-c", startCommand])}`,
    "",
  ].join("\n");
}

export function generateComposeForDockerfile(spec: DeploymentSpec): string {
  if (!spec.files.dockerfilePath) {
    throw new Error("Cannot generate Compose file without a Dockerfile path.");
  }
  return generateCompose(spec);
}

function generateCompose(spec: DeploymentSpec): string {
  const lines = [
    "services:",
    ...spec.sourceModel.services.flatMap((service) => generateAppService(spec, service)),
    ...spec.sourceModel.dependencies.flatMap(generateDependencyService),
    ...generateVolumes(spec),
  ];

  return lines.join("\n");
}

function generateAppService(spec: DeploymentSpec, service: DeploymentSourceService): string[] {
  const dockerfilePath = spec.files.dockerfilePaths[service.serviceId] ?? spec.files.dockerfilePath;
  if (!dockerfilePath) {
    throw new Error(`Cannot generate Compose service ${service.serviceId} without a Dockerfile path.`);
  }
  const contextPath = projectPathRelativeToFile(spec.files.composePath, spec.files.buildContextPath);
  const dockerfile = projectPathRelativeToDirectory(spec.files.buildContextPath, dockerfilePath);
  const environment = {
    ...generatedRuntimeEnvironmentForService(service),
    ...(service.role === "frontend"
      ? {}
      : {
          ...generatedDependencyEnvironmentForServices(spec.sourceModel.dependencies),
          ...spec.environment.generated,
        }),
  };
  const dependsOn = composeDependsOnForService(spec.sourceModel, spec.topology, service);
  const ports = service.serviceId === spec.sourceModel.previewServiceId
    ? [`    ports:`, `      - "${spec.runtime.hostPort}:${service.port}"`]
    : [];
  return [
    `  ${service.serviceId}:`,
    "    build:",
    `      context: ${yamlString(contextPath)}`,
    `      dockerfile: ${yamlString(dockerfile)}`,
    `    image: ${spec.imageName}-${service.serviceId}`,
    ...ports,
    ...yamlEnvironment(environment, 4),
    ...(service.startCommand
      ? [
          "    healthcheck:",
          `      test: ["CMD-SHELL", "wget -qO- http://127.0.0.1:${service.port}${service.healthcheckPath ?? "/"} >/dev/null 2>&1 || exit 1"]`,
          "      interval: 10s",
          "      timeout: 3s",
          "      retries: 6",
          "      start_period: 10s",
        ]
      : []),
    ...(dependsOn.length > 0
      ? [
          "    depends_on:",
          ...dependsOn.map((serviceName) => `      - ${serviceName}`),
        ]
      : []),
    "    restart: unless-stopped",
    "",
  ];
}

function composeDependsOnForService(
  sourceModel: DeploymentSourceModel,
  topology: DeploymentTopology,
  service: DeploymentSourceService,
): string[] {
  const dependencies = new Set<string>();
  if (service.serviceId === topology.publicEntryServiceId) {
    for (const targetServiceId of proxyTargetServiceIdsForPublicEntry(topology)) {
      if (targetServiceId !== service.serviceId) {
        dependencies.add(targetServiceId);
      }
    }
  }
  if (service.role !== "frontend") {
    for (const dependency of sourceModel.dependencies) {
      dependencies.add(dependency.serviceName);
    }
  }
  return [...dependencies].sort(comparePaths);
}

function generateNginxConfig(spec: DeploymentSpec): string {
  const proxyRoutes = proxyRoutesForPublicEntry(spec.topology);
  return [
    "server {",
    "  listen 80;",
    "  server_name localhost;",
    "  root /usr/share/nginx/html;",
    "  index index.html;",
    "",
    ...proxyRoutes.flatMap((route) => nginxProxyLocationLines(route.publicPath, route.targetServiceId, route.targetPort)),
    "  location / {",
    "    try_files $uri $uri/ /index.html;",
    "  }",
    "}",
    "",
  ].join("\n");
}

function nginxProxyLocationLines(publicPath: string, targetServiceId: string, targetPort: number): string[] {
  const pathPrefix = normalizeNginxPublicPath(publicPath);
  const slashPath = pathPrefix === "/" ? "/" : `${pathPrefix}/`;
  const lines: string[] = [];
  if (pathPrefix !== "/") {
    lines.push(
      `  location = ${pathPrefix} {`,
      ...nginxProxyPassLines(targetServiceId, targetPort),
      "  }",
      "",
    );
  }
  lines.push(
    `  location ${slashPath} {`,
    ...nginxProxyPassLines(targetServiceId, targetPort),
    "  }",
    "",
  );
  return lines;
}

function nginxProxyPassLines(targetServiceId: string, targetPort: number): string[] {
  return [
    `    proxy_pass http://${targetServiceId}:${targetPort};`,
    "    proxy_http_version 1.1;",
    "    proxy_set_header Host $host;",
    "    proxy_set_header X-Real-IP $remote_addr;",
    "    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;",
    "    proxy_set_header X-Forwarded-Proto $scheme;",
  ];
}

function normalizeNginxPublicPath(value: string): string {
  const trimmed = value.trim();
  if (!trimmed || trimmed === "/") {
    return "/";
  }
  const pathOnly = trimmed.split(/[?#]/, 1)[0];
  const normalized = pathOnly.startsWith("/") ? pathOnly : `/${pathOnly}`;
  return normalized.replace(/\/+$/, "") || "/";
}

function generateDependencyService(service: DependencyService): string[] {
  const commandLines = dependencyCommand(service);

  return [
    `  ${service.serviceName}:`,
    `    image: ${service.image}`,
    ...commandLines,
    ...(Object.keys(service.env).length > 0 ? yamlEnvironment(service.env, 4) : []),
    "    expose:",
    `      - \"${service.port}\"`,
    ...(service.volumeName
      ? [
          "    volumes:",
          `      - ${service.volumeName}:${service.volumeTarget ?? "/data"}`,
        ]
      : []),
    "",
  ];
}

function generatedRuntimeEnvironmentForService(service: DeploymentSourceService): Record<string, string> {
  switch (service.runtimeKind) {
    case "node":
      return {
        NODE_ENV: "production",
        PORT: String(service.port),
      };
    case "python":
      return {
        PORT: String(service.port),
      };
    case "go":
    case "java":
    case "dotnet":
    case "php":
    case "ruby":
      return {
        PORT: String(service.port),
        ...(service.runtimeKind === "ruby" ? { RAILS_ENV: "production", RACK_ENV: "production" } : {}),
        ...(service.runtimeKind === "java" ? { SERVER_PORT: String(service.port) } : {}),
        ...(service.runtimeKind === "dotnet" ? { ASPNETCORE_URLS: `http://0.0.0.0:${service.port}` } : {}),
      };
    case "static":
    case "unknown":
      return {};
  }
}

function generatedDependencyEnvironmentForServices(services: DependencyService[]): Record<string, string> {
  return Object.assign({}, ...services.map((dependency) => dependency.connectionEnv));
}

function generateVolumes(spec: DeploymentSpec): string[] {
  const volumes = spec.sourceModel.dependencies
    .map((service) => service.volumeName)
    .filter((volumeName): volumeName is string => Boolean(volumeName));
  if (volumes.length === 0) {
    return [];
  }

  return [
    "volumes:",
    ...volumes.map((volumeName) => `  ${volumeName}:`),
    "",
  ];
}

function yamlMap(values: Record<string, string>, indent: number): string[] {
  const prefix = " ".repeat(indent);
  return Object.entries(values).map(([key, value]) => `${prefix}${key}: ${JSON.stringify(value)}`);
}

function yamlEnvironment(values: Record<string, string>, indent: number): string[] {
  const prefix = " ".repeat(indent);
  if (Object.keys(values).length === 0) {
    return [`${prefix}environment: {}`];
  }
  return [
    `${prefix}environment:`,
    ...yamlMap(values, indent + 2),
  ];
}

function projectPathRelativeToFile(fromProjectRelativeFile: string, toProjectRelativePath: string): string {
  const fromDirectory = path.dirname(fromProjectRelativeFile);
  return projectPathRelativeToDirectory(fromDirectory, toProjectRelativePath);
}

function projectPathRelativeToDirectory(fromProjectRelativeDirectory: string, toProjectRelativePath: string): string {
  const relative = path.relative(fromProjectRelativeDirectory, toProjectRelativePath).split(path.sep).join("/");
  return relative || ".";
}

function yamlString(value: string): string {
  return JSON.stringify(value);
}

function generateDockerignore(): string {
  return [
    ".git",
    ".loom/deployment/specs/local.json",
    ".loom/deployment/specs/generated/compose.yaml",
    ".loom/deployment/state",
    ".loom/deployment/logs",
    ".loom/tmp",
    "node_modules",
    ".next",
    ".turbo",
    ".vercel",
    "out",
    "dist",
    "build",
    ".venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "*.pyc",
    "target",
    "tmp",
    "coverage",
    "*.log",
    ".env",
    ".env.*",
    "!.env.example",
    "",
  ].join("\n");
}

function installCommandFor(
  packageManager: Exclude<DeploymentSourceService["packageManager"], null>,
  hasLockfile: boolean,
): string {
  switch (packageManager) {
    case "npm":
      return hasLockfile ? "npm ci" : "npm install";
    case "pnpm":
      return hasLockfile
        ? "corepack enable && pnpm install --frozen-lockfile"
        : "corepack enable && pnpm install";
    case "yarn":
      return hasLockfile
        ? "corepack enable && yarn install --frozen-lockfile"
        : "corepack enable && yarn install";
    case "bun":
      return hasLockfile
        ? "bun install --frozen-lockfile"
        : "bun install";
    case "pip":
    case "poetry":
    case "uv":
    case "go":
    case "maven":
    case "gradle":
    case "dotnet":
    case "composer":
    case "bundler":
      return "";
  }
}

function packageManagerRun(
  packageManager: Exclude<DeploymentSourceService["packageManager"], null>,
  script: string,
): string {
  switch (packageManager) {
    case "npm":
      return `npm run ${script}`;
    case "pnpm":
      return `pnpm run ${script}`;
    case "yarn":
      return `yarn ${script}`;
    case "bun":
      return `bun run ${script}`;
    case "pip":
    case "poetry":
    case "uv":
    case "go":
    case "maven":
    case "gradle":
    case "dotnet":
    case "composer":
    case "bundler":
      return "";
  }
}

function nodeBaseImageFor(stack: DeploymentSourceService): string {
  if (stack.packageManager === "bun") {
    return "oven/bun:1";
  }

  return `node:${stack.runtimeVersion ?? "22"}-slim`;
}

function lockfileCopyFor(packageManager: Exclude<DeploymentSourceService["packageManager"], null>): string {
  switch (packageManager) {
    case "npm":
      return "COPY package.json package-lock.json* ./";
    case "pnpm":
      return "COPY package.json pnpm-lock.yaml* pnpm-workspace.yaml* ./";
    case "yarn":
      return "COPY package.json yarn.lock* ./";
    case "bun":
      return "COPY package.json bun.lock* bun.lockb* ./";
    case "pip":
    case "poetry":
    case "uv":
    case "go":
    case "maven":
    case "gradle":
    case "dotnet":
    case "composer":
    case "bundler":
      return "COPY . ./";
  }
}

function workingDirectoryLines(stack: DeploymentSourceService): string[] {
  return stack.workingDirectory ? [`WORKDIR /app/${stack.workingDirectory}`] : [];
}

function workspaceManifestCopyLines(stack: DeploymentSourceService, sourcePrefix = "."): string[] {
  if (stack.runtimeKind !== "node") {
    return [];
  }

  const manifestPaths = new Set<string>();
  if (stack.workingDirectory) {
    manifestPaths.add(`${stack.workingDirectory}/package.json`);
  }
  for (const manifestPath of stack.workspacePackageJsonPaths ?? []) {
    manifestPaths.add(manifestPath);
  }

  return [...manifestPaths]
    .sort(comparePaths)
    .map((manifestPath) => {
      const source = sourcePrefix === "." ? manifestPath : `${sourcePrefix}/${manifestPath}`;
      return `COPY ${source} ./${manifestPath}`;
    });
}

function workspaceNodeModulesCopyLines(stack: DeploymentSourceService): string[] {
  if (!stack.workingDirectory || stack.runtimeKind !== "node") {
    return [];
  }
  if (!["pnpm", "yarn"].includes(stack.packageManager ?? "")) {
    return [];
  }

  return [
    `COPY --from=deps /app/${stack.workingDirectory}/node_modules ./${stack.workingDirectory}/node_modules`,
  ];
}

function javaBuildCommand(packageManager: "maven" | "gradle"): string {
  return packageManager === "maven"
    ? "if [ -x ./mvnw ]; then ./mvnw -DskipTests package; else mvn -DskipTests package; fi"
    : "if [ -x ./gradlew ]; then ./gradlew build -x test; else gradle build -x test; fi";
}

function javaFrontendToolchainLines(buildCommand: string): string[] {
  if (!requiresJavaScriptToolchain(buildCommand)) {
    return [];
  }

  return [
    "USER root",
    "RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \\",
    "  && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \\",
    "  && apt-get install -y --no-install-recommends nodejs \\",
    "  && corepack enable \\",
    "  && rm -rf /var/lib/apt/lists/*",
    ...(usesBun(buildCommand) ? ["RUN npm install -g bun"] : []),
  ];
}

function requiresJavaScriptToolchain(command: string): boolean {
  return commandTokenPattern(["npm", "npx", "pnpm", "yarn", "node", "bun", "vite", "next", "tsc"]).test(command);
}

function usesBun(command: string): boolean {
  return commandTokenPattern(["bun"]).test(command);
}

function commandTokenPattern(tokens: string[]): RegExp {
  return new RegExp(`(^|[\\s;&|()])(?:${tokens.map(escapeRegExp).join("|")})(?=$|[\\s;&|()])`);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function javaRuntimeStartCommand(command: string | null): string {
  if (!command || /(^|[\/\s])(?:mvnw|mvn|gradlew|gradle)(\s|$)/.test(command)) {
    return "java -jar /app/app.jar";
  }

  if (/(^|\s)java\s+/.test(command) && /\s-jar\s+/.test(command)) {
    return command.replace(/(-jar\s+)(?:"[^"]+"|'[^']+'|\S+)/, "$1/app/app.jar");
  }

  return command;
}

function pythonInstallLines(packageManager: Exclude<DeploymentSourceService["packageManager"], null>): string[] {
  switch (packageManager) {
    case "uv":
      return [
        "RUN pip install --no-cache-dir uv",
        "COPY pyproject.toml uv.lock* requirements.txt* ./",
        "RUN if [ -f uv.lock ] || [ -f pyproject.toml ]; then uv pip install --system -r pyproject.toml || uv pip install --system -r requirements.txt; elif [ -f requirements.txt ]; then pip install --no-cache-dir -r requirements.txt; fi",
      ];
    case "poetry":
      return [
        "RUN pip install --no-cache-dir poetry",
        "COPY pyproject.toml poetry.lock* requirements.txt* ./",
        "RUN if [ -f pyproject.toml ]; then poetry config virtualenvs.create false && poetry install --only main --no-interaction --no-ansi; elif [ -f requirements.txt ]; then pip install --no-cache-dir -r requirements.txt; fi",
      ];
    case "pip":
    default:
      return [
        "COPY requirements.txt pyproject.toml* ./",
        "RUN if [ -f requirements.txt ]; then pip install --no-cache-dir -r requirements.txt; fi",
      ];
  }
}

function dependencyCommand(service: DependencyService): string[] {
  if (service.kind === "minio") {
    return ["    command: server /data --console-address \":9001\""];
  }
  return [];
}

function comparePaths(left: string, right: string): number {
  return left.localeCompare(right);
}

function sanitizeName(value: string): string {
  const normalized = value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return normalized || "app";
}

function dedupeStrings(values: string[]): string[] {
  return [...new Set(values)];
}
