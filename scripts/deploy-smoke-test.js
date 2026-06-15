#!/usr/bin/env node

const assert = require("node:assert/strict");
const { access, chmod, mkdir, mkdtemp, readFile, writeFile } = require("node:fs/promises");
const { join } = require("node:path");
const { tmpdir } = require("node:os");
const { spawn, spawnSync } = require("node:child_process");

async function main() {
  const root = await mkdtemp(join(tmpdir(), "loom-deploy-smoke-"));

  await verifyGeneratedTemplate(join(root, "generated-template"));
  await verifyGeneratedTemplateWithoutEnvironment(join(root, "generated-template-no-env"));
  await verifyExistingDockerfile(join(root, "existing-dockerfile"));
  await verifyExistingCompose(join(root, "existing-compose"));
  await verifyNextBunTemplate(join(root, "next-bun-template"));
  await verifyNextStandaloneTemplate(join(root, "next-standalone-template"));
  await verifyNodeVersionTemplate(join(root, "node-version-template"));
  await verifyPythonTemplate(join(root, "python-template"));
  await verifyPythonStdlibHttpTemplate(join(root, "python-stdlib-http-template"));
  await verifyDependencyServiceTokenBoundaries(join(root, "dependency-service-token-boundaries"));
  await verifyGoTemplate(join(root, "go-template"));
  await verifyJavaMavenTemplate(join(root, "java-maven-template"));
  await verifyJavaMixedFrontendTemplate(join(root, "java-mixed-frontend-template"));
  await verifyDotnetAspnetTemplate(join(root, "dotnet-aspnet-template"));
  await verifyPhpLaravelTemplate(join(root, "php-laravel-template"));
  await verifyRubyRailsTemplate(join(root, "ruby-rails-template"));
  await verifyPnpmWorkspaceTemplate(join(root, "pnpm-workspace-template"));
  await verifyNpmWorkspaceTemplate(join(root, "npm-workspace-template"));
  await verifyExistingDockerfileInWorkspace(join(root, "workspace-existing-dockerfile"));
  await verifyEnvironmentDiagnostics(join(root, "environment-diagnostics"));
  await verifyExplicitAppPath(join(root, "explicit-app-path"));
  await verifyHealthcheckCandidateSelection(join(root, "healthcheck-candidates"));
  await verifyExistingComposeServiceSelection(join(root, "existing-compose-service-selection"));
  await verifyBootstrapDiagnostics(join(root, "bootstrap-diagnostics"));
  await verifyNestedBootstrapDiagnostics(join(root, "nested-bootstrap-diagnostics"));
  await verifyRepairDiagnostics(join(root, "repair-diagnostics"));
  await verifyDeploymentAssetRepairAutoRunnableInstruction(join(root, "deployment-asset-repair-auto-runnable"));
  await verifyRegistryNetworkRepair(join(root, "registry-network-repair"));
  await verifyRuntimeContractPrepare(join(root, "runtime-contract-prepare"));
  await verifyRuntimeContractPromotesRootWorkspaceWhenChildrenAreSourceOnly(join(root, "runtime-contract-root-workspace"));
  await verifyRuntimeContractSuppressesHeuristicDependencyServices(join(root, "runtime-contract-suppresses-heuristic-deps"));
  await verifyRuntimeContractDerivesDependencyServicesFromEnvironment(join(root, "runtime-contract-env-deps"));
  await verifyRuntimeContractUsesDetectedSqlServiceForGenericDatasource(join(root, "runtime-contract-generic-datasource"));
  await verifyStaleRuntimeContractSpecReprepare(join(root, "runtime-contract-stale-reprepare"));
  await verifyDeployUsesPreviousCompletedPhaseRuntimeContract(join(root, "runtime-contract-previous-completed-phase"));
  await verifyTechnicalBaselineOnlyDoesNotProvisionDatabase(join(root, "technical-baseline-only-db"));
  await verifyUnknownDatabaseKindBlocksPrepare(join(root, "unknown-database-kind"));
  await verifyBaselineDatabaseConflictBlocksPrepare(join(root, "baseline-database-conflict"));
  await verifyDeployRunDockerUnavailableWritesRepairRequest(join(root, "docker-unavailable-repair"));
  await verifyRuntimeContractStartFailureRoutesToDeliveryRepair(join(root, "runtime-contract-start-failure"));
  await verifyApplicationStartupFailureRoutesToExecutionRepair(join(root, "application-startup-failure"));
  await verifyRuntimeContractBuildFailureRoutesToDeliveryRepair(join(root, "runtime-contract-build-failure"));
  await verifyHealthcheckOverrides(join(root, "healthcheck-overrides"));
  await verifyBootstrapCommandPreviewAndConfirm(join(root, "bootstrap-command-preview-confirm"));
  await verifyProviderPolicy(join(root, "provider-policy"));
  await verifyDeployInspect(join(root, "deploy-inspect"));
  await verifyDeployLogsCompact(join(root, "deploy-logs-compact"));
  await verifyDeployInspectRefresh(join(root, "deploy-inspect-refresh"));
  await verifyDeploySuccessClearsFailureAndGuardsRawCompose(join(root, "deploy-success-clears-failure"));
  await verifyDeployStatusUsesLatestPreparedSpec(join(root, "deploy-status-latest-spec"));

  console.log(`deploy smoke tests passed in ${root}`);
}

async function verifyGeneratedTemplate(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
      pg: "^8.0.0",
      redis: "^4.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.provider, "dockerfile-template");
  assert.equal(envelope.data.files.generated, true);
  assert.equal(envelope.data.files.reused.length, 0);
  assertSelectedCandidate(envelope, "dockerfile-template");
  assert.equal(envelope.data.detectedStack.services.length, 2);
  assert.equal(envelope.data.detectedStack.runtimeVersion, "22");
  assert.equal(envelope.data.detectedStack.runtimeVersionSource, "default");

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM node:22-slim AS deps/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /postgres:16-alpine/);
  assert.match(compose, /redis:7-alpine/);
  assert.match(compose, /healthcheck:/);
  assert.doesNotMatch(compose, /container_name:/);
  assert.doesNotMatch(compose, /ports:\n\s+- "5432:5432"/);

  const validation = runDeployValidate(projectRoot);
  assert.equal(validation.ok, true);
  assert.equal(validation.data.valid, true);
  assert.equal(validation.data.config.ok, true);
}

async function verifyGeneratedTemplateWithoutEnvironment(projectRoot) {
  await mkdir(projectRoot, { recursive: true });
  await writeFile(join(projectRoot, "NOTES.txt"), "No deployable stack here.\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "unknown");
  assert.equal(envelope.data.provider, "dockerfile-template");

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /environment: \{\}/);
  assert.doesNotMatch(compose, /environment:\n\s+restart:/);

  const validation = runDeployValidate(projectRoot);
  assert.equal(validation.ok, true);
  assert.equal(validation.data.valid, true);
  assert.equal(validation.data.config.ok, true);
}

async function verifyExistingDockerfile(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "Dockerfile"), "FROM node:20-alpine\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.provider, "dockerfile-existing");
  assert.deepEqual(envelope.data.files.reused, ["Dockerfile"]);
  assert.equal(envelope.data.files.dockerfilePath, "Dockerfile");
  assertSelectedCandidate(envelope, "dockerfile-existing");

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /dockerfile: "Dockerfile"/);
}

async function verifyExistingCompose(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  const composePath = join(projectRoot, "compose.yaml");
  const originalCompose = [
    "services:",
    "  app:",
    "    image: nginx:1.27-alpine",
    "    ports:",
    "      - \"8099:80\"",
    "",
  ].join("\n");
  await writeFile(composePath, originalCompose, "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.provider, "compose-existing");
  assert.deepEqual(envelope.data.files.reused, ["compose.yaml"]);
  assert.equal(envelope.data.files.generated, false);
  assert.equal(envelope.data.url, "http://localhost:8099");
  assertSelectedCandidate(envelope, "compose-existing");
  assert.equal(await readFile(composePath, "utf8"), originalCompose);

  const runFailureRoot = `${projectRoot}-run-failure`;
  await writePackage(runFailureRoot, {
    scripts: {
      start: "node server.js",
    },
  });
  await writeFile(join(runFailureRoot, "compose.yaml"), "services:\n  app:\n    image:\n", "utf8");

  const run = runDeployRun(runFailureRoot);
  assert.equal(run.ok, true);
  assert.equal(run.data.completed, false);
  assert.equal(run.data.failedPhase, "up");
  assert.equal(run.data.prepare.provider, "compose-existing");
  assert.equal(run.data.repair.failureKind, "compose_config");
  assert.equal(run.data.nextAction, "request-user-approval");
}

async function verifyProviderPolicy(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "Dockerfile"), "FROM node:22-slim\n", "utf8");
  await writeFile(join(projectRoot, "compose.yaml"), [
    "services:",
    "  app:",
    "    image: nginx:1.27-alpine",
    "    ports:",
    "      - \"8098:80\"",
    "",
  ].join("\n"), "utf8");

  const defaultPrepare = runDeployPrepare(projectRoot);
  assert.equal(defaultPrepare.ok, true);
  assert.equal(defaultPrepare.data.provider, "compose-existing");
  assert.equal(defaultPrepare.data.providerPolicy.reuseExisting, true);

  const dockerfilePrepare = runDeployPrepare(projectRoot, ["--provider", "dockerfile-existing"]);
  assert.equal(dockerfilePrepare.ok, true);
  assert.equal(dockerfilePrepare.data.provider, "dockerfile-existing");
  assert.equal(dockerfilePrepare.data.providerPolicy.provider, "dockerfile-existing");
  assert.deepEqual(dockerfilePrepare.data.files.reused, ["Dockerfile"]);

  const generatedPrepare = runDeployPrepare(projectRoot, ["--reuse-existing", "false"]);
  assert.equal(generatedPrepare.ok, true);
  assert.equal(generatedPrepare.data.provider, "dockerfile-template");
  assert.equal(generatedPrepare.data.providerPolicy.reuseExisting, false);
  assert.deepEqual(generatedPrepare.data.files.reused, []);
  assert.ok(generatedPrepare.data.providerCandidates.some((candidate) => (
    candidate.provider === "compose-existing" &&
    candidate.status === "available" &&
    /disables existing/.test(candidate.reason)
  )));

  const forced = runDeployPrepare(projectRoot, ["--force-generate"]);
  assert.equal(forced.ok, true);
  assert.equal(forced.data.provider, "dockerfile-template");
  assert.equal(forced.data.providerPolicy.forceGenerate, true);

  const invalidRoot = `${projectRoot}-invalid`;
  await writePackage(invalidRoot, {
    scripts: {
      start: "node server.js",
    },
  });
  const invalid = runDeployPrepare(invalidRoot, ["--provider", "compose-existing"], [64]);
  assert.equal(invalid.ok, false);
  assert.equal(invalid.error.code, "INVALID_ARGUMENT");
}

async function verifyExistingComposeServiceSelection(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "compose.yaml"), [
    "services:",
    "  postgres:",
    "    image: postgres:16-alpine",
    "    ports:",
    "      - \"5438:5432\"",
    "  web:",
    "    build: .",
    "    ports:",
    "      - target: 3000",
    "        published: 8123",
    "        protocol: tcp",
    "    depends_on:",
    "      - postgres",
    "",
  ].join("\n"), "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.provider, "compose-existing");
  assert.equal(envelope.data.url, "http://localhost:8123");
  assert.equal(envelope.data.detectedStack.port, 3000);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.compose.selectedService, "web");
  assert.match(spec.compose.serviceReason, /application service|build configuration|publishes a host port/);
  assert.ok(spec.compose.services.some((service) => service.name === "postgres" && service.dependencyLike));
  assert.equal(spec.runtime.containerPort, 3000);
}

async function verifyNextBunTemplate(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "bun run build",
      start: "bun run start",
    },
    dependencies: {
      next: "16.2.2",
      react: "19.2.4",
      "react-dom": "19.2.4",
    },
  });
  await writeFile(join(projectRoot, "bun.lock"), "", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.framework, "next");
  assert.equal(envelope.data.detectedStack.packageManager, "bun");

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM oven\/bun:1 AS deps/);
  assert.doesNotMatch(dockerfile, /npm install -g bun/);
  assert.match(dockerfile, /NEXT_TELEMETRY_DISABLED=1/);

  const dockerignore = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile.dockerignore"), "utf8");
  assert.match(dockerignore, /^\.next$/m);
  assert.match(dockerignore, /^\.turbo$/m);
  assert.match(dockerignore, /^\.vercel$/m);
}

async function verifyNextStandaloneTemplate(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "next build",
      start: "next start",
    },
    dependencies: {
      next: "16.2.2",
      react: "19.2.4",
      "react-dom": "19.2.4",
    },
  });
  await writeFile(join(projectRoot, "next.config.mjs"), "export default { output: 'standalone' };\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.framework, "next");
  assert.equal(envelope.data.detectedStack.startCommand, "node .next/standalone/server.js");

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /RUN npm install/);
  assert.ok(dockerfile.includes('CMD ["sh","-c","node .next/standalone/server.js"]'));
}

async function verifyNodeVersionTemplate(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    engines: {
      node: "20.x",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.runtimeVersion, "20");
  assert.equal(envelope.data.detectedStack.runtimeVersionSource, "package.json engines.node");

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM node:20-slim AS deps/);
}

async function verifyPythonTemplate(projectRoot) {
  await mkdir(projectRoot, { recursive: true });
  await writeFile(join(projectRoot, "requirements.txt"), "fastapi\nuvicorn\npymongo\npika\n", "utf8");
  await writeFile(join(projectRoot, "main.py"), "from fastapi import FastAPI\napp = FastAPI()\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "python");
  assert.equal(envelope.data.detectedStack.packageManager, "pip");
  assert.equal(envelope.data.detectedStack.framework, "fastapi");
  assert.equal(envelope.data.detectedStack.port, 8000);
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "mongodb"));
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "rabbitmq"));

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM python:3\.12-slim/);
  assert.match(dockerfile, /uvicorn main:app/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /mongo:7/);
  assert.match(compose, /rabbitmq:3-alpine/);
}

async function verifyPythonStdlibHttpTemplate(projectRoot) {
  await mkdir(join(projectRoot, "trading_system"), { recursive: true });
  await writeFile(join(projectRoot, "server.py"), [
    "from trading_system.http_adapter import run_http_server",
    "import argparse",
    "",
    "def main():",
    "    parser = argparse.ArgumentParser()",
    "    parser.add_argument('--host', default='127.0.0.1')",
    "    parser.add_argument('--port', default=8000, type=int)",
    "    args = parser.parse_args()",
    "    run_http_server(host=args.host, port=args.port)",
    "",
    "if __name__ == '__main__':",
    "    main()",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, "trading_system/__init__.py"), "", "utf8");
  await writeFile(join(projectRoot, "trading_system/http_adapter.py"), [
    "from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer",
    "",
    "def run_http_server(host='127.0.0.1', port=8000):",
    "    server = ThreadingHTTPServer((host, port), BaseHTTPRequestHandler)",
    "    server.serve_forever()",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, "HTTP_API.md"), [
    "# API",
    "",
    "Run with:",
    "python3 server.py --host 127.0.0.1 --port 8000",
    "",
    "Routes:",
    "GET /",
    "GET /health",
    "",
  ].join("\n"), "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "python");
  assert.equal(envelope.data.detectedStack.packageManager, "pip");
  assert.equal(envelope.data.detectedStack.framework, "stdlib-http");
  assert.equal(envelope.data.detectedStack.port, 8000);
  assert.equal(envelope.data.detectedStack.healthcheckPath, "/health");
  assert.equal(envelope.data.detectedStack.startCommand, "python server.py --host 0.0.0.0 --port 8000");
  assert.match(envelope.data.url, /^http:\/\/localhost:\d+$/);

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM python:3\.12-slim/);
  assert.match(dockerfile, /python server\.py --host 0\.0\.0\.0 --port 8000/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /environment:\n\s+PORT: "8000"/);
  assert.match(compose, /127\.0\.0\.1:8000\/health/);
  assert.match(compose, /"\d+:8000"/);
  assert.doesNotMatch(compose, /environment:\n\s+restart:/);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtime.containerPort, 8000);
  assert.equal(spec.runtime.healthcheck.enabled, true);
  assert.equal(spec.runtime.healthcheck.path, "/health");
  assert.match(spec.runtime.healthcheck.url, /\/health$/);
  assert.equal(spec.compose.services[0].ports[0].containerPort, 8000);
}

async function verifyDependencyServiceTokenBoundaries(projectRoot) {
  await mkdir(projectRoot, { recursive: true });
  await writeFile(join(projectRoot, "server.py"), [
    "from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer",
    "def main():",
    "    # The word upgrade should not be interpreted as a database package.",
    "    ThreadingHTTPServer(('0.0.0.0', 8000), BaseHTTPRequestHandler).serve_forever()",
    "if __name__ == '__main__':",
    "    main()",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, "HTTP_API.md"), "POST /info/upgrade\nGET /health\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "python");
  assert.equal(envelope.data.detectedStack.services.length, 0);
}

async function verifyGoTemplate(projectRoot) {
  await mkdir(projectRoot, { recursive: true });
  await writeFile(join(projectRoot, "go.mod"), "module example.com/app\n\ngo 1.23\n\nrequire github.com/gin-gonic/gin v1.10.0\n", "utf8");
  await writeFile(join(projectRoot, "main.go"), "package main\nfunc main() {}\n", "utf8");
  await writeFile(join(projectRoot, ".env.example"), "PORT=9090\nMYSQL_URL=mysql://example\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "go");
  assert.equal(envelope.data.detectedStack.packageManager, "go");
  assert.equal(envelope.data.detectedStack.framework, "gin");
  assert.equal(envelope.data.detectedStack.port, 9090);
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "mysql"));

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM golang:1\.23-alpine AS builder/);
  assert.match(dockerfile, /go build -o \/out\/server/);
}

async function verifyJavaMavenTemplate(projectRoot) {
  await mkdir(join(projectRoot, "src/main/resources"), { recursive: true });
  await writeFile(join(projectRoot, "pom.xml"), [
    "<project>",
    "  <properties>",
    "    <java.version>17</java.version>",
    "  </properties>",
    "  <dependencies>",
    "    <dependency>",
    "      <groupId>org.springframework.boot</groupId>",
    "      <artifactId>spring-boot-starter-web</artifactId>",
    "    </dependency>",
    "    <dependency>",
    "      <groupId>org.postgresql</groupId>",
    "      <artifactId>postgresql</artifactId>",
    "    </dependency>",
    "  </dependencies>",
    "</project>",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, "src/main/resources/application.properties"), "server.port=9091\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "java");
  assert.equal(envelope.data.detectedStack.packageManager, "maven");
  assert.equal(envelope.data.detectedStack.framework, "spring-boot");
  assert.equal(envelope.data.detectedStack.runtimeVersion, "17");
  assert.equal(envelope.data.detectedStack.runtimeVersionSource, "java.version");
  assert.equal(envelope.data.detectedStack.port, 9091);
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "postgres"));

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM maven:3-eclipse-temurin-17 AS builder/);
  assert.match(dockerfile, /FROM eclipse-temurin:17-jre AS runner/);
  assert.match(dockerfile, /mvn -DskipTests package/);
  assert.match(dockerfile, /java -jar \/app\/app\.jar/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /SERVER_PORT: "9091"/);
  assert.match(compose, /postgres:16-alpine/);
}

async function verifyJavaMixedFrontendTemplate(projectRoot) {
  await mkdir(join(projectRoot, "src/main/resources"), { recursive: true });
  await mkdir(join(projectRoot, "apps/web"), { recursive: true });
  await writeFile(join(projectRoot, "pom.xml"), [
    "<project>",
    "  <properties>",
    "    <java.version>21</java.version>",
    "  </properties>",
    "  <dependencies>",
    "    <dependency>",
    "      <groupId>org.springframework.boot</groupId>",
    "      <artifactId>spring-boot-starter-web</artifactId>",
    "    </dependency>",
    "  </dependencies>",
    "</project>",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, "src/main/resources/application.properties"), "server.port=8088\n", "utf8");
  await writeFile(join(projectRoot, "apps/web/package.json"), `${JSON.stringify({
    scripts: {
      build: "vite build",
    },
    dependencies: {
      vite: "^6.0.0",
    },
  }, null, 2)}\n`, "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    runtimeKind: "spring_boot_serves_vite_static",
    startPort: 8088,
    buildCommand: "npm --prefix apps/web run build && mvn -DskipTests package",
    startCommand: "java -jar target/demo.jar --spring.profiles.active=local",
    previewPath: "/",
    healthPath: "/actuator/health",
    frontendOutputDir: "apps/web/dist",
  });

  const envelope = runDeployPrepare(projectRoot);
  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "java");
  assert.equal(envelope.data.detectedStack.buildCommand, "npm --prefix apps/web run build && mvn -DskipTests package");

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /USER root/);
  assert.match(dockerfile, /apt-get install -y --no-install-recommends curl ca-certificates/);
  assert.match(dockerfile, /apt-get install -y --no-install-recommends nodejs/);
  assert.match(dockerfile, /corepack enable/);
  assert.match(dockerfile, /RUN npm --prefix apps\/web run build && mvn -DskipTests package/);
  assert.match(dockerfile, /find \. -type f -name '\*\.jar'/);
  assert.match(dockerfile, /CMD \["sh","-c","java -jar \/app\/app\.jar --spring\.profiles\.active=local"\]/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.doesNotMatch(compose, /container_name:/);
}

async function verifyDotnetAspnetTemplate(projectRoot) {
  await mkdir(projectRoot, { recursive: true });
  await writeFile(join(projectRoot, "App.csproj"), [
    '<Project Sdk="Microsoft.NET.Sdk.Web">',
    "  <PropertyGroup>",
    "    <TargetFramework>net8.0</TargetFramework>",
    "  </PropertyGroup>",
    "  <ItemGroup>",
    '    <PackageReference Include="Npgsql" Version="8.0.0" />',
    '    <PackageReference Include="StackExchange.Redis" Version="2.8.0" />',
    "  </ItemGroup>",
    "</Project>",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, "appsettings.json"), JSON.stringify({
    ASPNETCORE_URLS: "http://0.0.0.0:7070",
  }, null, 2), "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "dotnet");
  assert.equal(envelope.data.detectedStack.packageManager, "dotnet");
  assert.equal(envelope.data.detectedStack.framework, "aspnetcore");
  assert.equal(envelope.data.detectedStack.runtimeVersion, "8");
  assert.equal(envelope.data.detectedStack.runtimeVersionSource, "TargetFramework");
  assert.equal(envelope.data.detectedStack.port, 7070);
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "postgres"));
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "redis"));

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM mcr\.microsoft\.com\/dotnet\/sdk:8 AS build/);
  assert.match(dockerfile, /FROM mcr\.microsoft\.com\/dotnet\/aspnet:8 AS runner/);
  assert.match(dockerfile, /dotnet publish -c Release -o \/app\/publish --no-restore/);
  assert.match(dockerfile, /dotnet \/app\/App\.dll/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /ASPNETCORE_URLS: "http:\/\/0\.0\.0\.0:7070"/);
  assert.match(compose, /redis:7-alpine/);
  assert.match(compose, /postgres:16-alpine/);
}

async function verifyPhpLaravelTemplate(projectRoot) {
  await mkdir(join(projectRoot, "public"), { recursive: true });
  await writeFile(join(projectRoot, "composer.json"), JSON.stringify({
    require: {
      php: "^8.2",
      "laravel/framework": "^11.0",
      predis: "^2.0",
      "ext-pdo_mysql": "*",
    },
  }, null, 2), "utf8");
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build",
    },
    dependencies: {
      vite: "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "artisan"), "#!/usr/bin/env php\n<?php\n", "utf8");
  await writeFile(join(projectRoot, "public/index.php"), "<?php echo 'ok';\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "php");
  assert.equal(envelope.data.detectedStack.packageManager, "composer");
  assert.equal(envelope.data.detectedStack.framework, "laravel");
  assert.equal(envelope.data.detectedStack.runtimeVersion, "8.2");
  assert.equal(envelope.data.detectedStack.runtimeVersionSource, "composer.json require.php");
  assert.equal(envelope.data.detectedStack.port, 8000);
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "mysql"));
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "redis"));
  assert.ok(envelope.data.environment.generated.APP_KEY);
  assert.ok(envelope.data.environment.provided.includes("APP_KEY"));

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM php:8\.2-cli AS runner/);
  assert.match(dockerfile, /COPY --from=composer:2/);
  assert.match(dockerfile, /docker-php-ext-install pdo pdo_mysql pdo_pgsql zip/);
  assert.match(dockerfile, /php artisan serve --host=0\.0\.0\.0/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /mysql:8/);
  assert.match(compose, /redis:7-alpine/);
  assert.match(compose, /APP_KEY: "base64:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="/);
}

async function verifyRubyRailsTemplate(projectRoot) {
  await mkdir(join(projectRoot, "config"), { recursive: true });
  await writeFile(join(projectRoot, "Gemfile"), [
    'source "https://rubygems.org"',
    'ruby "3.2.2"',
    'gem "rails", "~> 7.1"',
    'gem "pg", "~> 1.5"',
    'gem "redis", "~> 5.0"',
    "",
  ].join("\n"), "utf8");
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build",
    },
    dependencies: {
      vite: "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "config/application.rb"), "require 'rails/all'\n", "utf8");
  await writeFile(join(projectRoot, "config/database.yml"), "production:\n  adapter: postgresql\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.detectedStack.kind, "ruby");
  assert.equal(envelope.data.detectedStack.packageManager, "bundler");
  assert.equal(envelope.data.detectedStack.framework, "rails");
  assert.equal(envelope.data.detectedStack.runtimeVersion, "3.2");
  assert.equal(envelope.data.detectedStack.runtimeVersionSource, "Gemfile ruby");
  assert.equal(envelope.data.detectedStack.port, 3000);
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "postgres"));
  assert.ok(envelope.data.detectedStack.services.some((service) => service.kind === "redis"));
  assert.ok(envelope.data.environment.generated.SECRET_KEY_BASE);
  assert.ok(envelope.data.environment.provided.includes("SECRET_KEY_BASE"));

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM ruby:3\.2-slim AS runner/);
  assert.match(dockerfile, /bundle config set without 'development test' && bundle install/);
  assert.match(dockerfile, /bundle exec rails server -b 0\.0\.0\.0/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /postgres:16-alpine/);
  assert.match(compose, /redis:7-alpine/);
  assert.match(compose, /SECRET_KEY_BASE: "loom-local-secret-key-base-change-me"/);
}

async function verifyPnpmWorkspaceTemplate(projectRoot) {
  await mkdir(join(projectRoot, "apps/web"), { recursive: true });
  await writeFile(join(projectRoot, "pnpm-workspace.yaml"), [
    "packages:",
    "  - apps/*",
    "  - packages/*",
    "",
  ].join("\n"), "utf8");
  await writePackage(projectRoot, {
    private: true,
    workspaces: [
      "apps/*",
      "packages/*",
    ],
    scripts: {
      lint: "echo root",
    },
  });
  await writeFile(join(projectRoot, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n", "utf8");
  await writePackage(join(projectRoot, "apps/web"), {
    scripts: {
      build: "vite build",
      preview: "vite preview",
    },
    dependencies: {
      "@vitejs/plugin-react": "^5.0.0",
      vite: "^6.0.0",
      react: "^19.0.0",
    },
  });
  await writeFile(join(projectRoot, "apps/web/index.html"), "<div id=\"root\"></div>\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.workspace.isWorkspace, true);
  assert.equal(envelope.data.workspace.appPath, "apps/web");
  assert.equal(envelope.data.workspace.buildContextPath, ".");
  assert.equal(envelope.data.files.buildContextPath, ".");
  assert.equal(envelope.data.detectedStack.kind, "node");
  assert.equal(envelope.data.detectedStack.framework, "vite");
  assert.equal(envelope.data.detectedStack.packageManager, "pnpm");
  assert.equal(envelope.data.detectedStack.workingDirectory, "apps/web");
  assert.equal(envelope.data.provider, "dockerfile-template");
  assert.ok(envelope.data.workspace.candidates.some((candidate) => candidate.path === "apps/web"));

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /COPY apps\/web\/package\.json \.\/apps\/web\/package\.json/);
  assert.match(dockerfile, /COPY --from=deps \/app\/apps\/web\/node_modules \.\/apps\/web\/node_modules/);
  assert.match(dockerfile, /WORKDIR \/app\/apps\/web/);
  assert.match(dockerfile, /pnpm install --frozen-lockfile/);
  assert.match(dockerfile, /pnpm run preview -- --host 0\.0\.0\.0/);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /context: "\.\.\/\.\.\/\.\.\/\.\."/);
  assert.match(compose, /dockerfile: "\.loom\/deployment\/specs\/generated\/Dockerfile"/);
}

async function verifyNpmWorkspaceTemplate(projectRoot) {
  await mkdir(join(projectRoot, "apps/api"), { recursive: true });
  await mkdir(join(projectRoot, "apps/web"), { recursive: true });
  await writePackage(projectRoot, {
    private: true,
    workspaces: [
      "apps/*",
    ],
    scripts: {
      build: "npm run build --workspace apps/web",
      start: "npm run start --workspace apps/api",
    },
  });
  await writeFile(join(projectRoot, "package-lock.json"), `${JSON.stringify({
    lockfileVersion: 3,
    requires: true,
    packages: {},
  }, null, 2)}\n`, "utf8");
  await writePackage(join(projectRoot, "apps/api"), {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writePackage(join(projectRoot, "apps/web"), {
    scripts: {
      build: "vite build",
    },
    dependencies: {
      "@vitejs/plugin-react": "^5.0.0",
      vite: "^6.0.0",
      react: "^19.0.0",
      "react-dom": "^19.0.0",
    },
  });
  await writeFile(join(projectRoot, "apps/api/server.js"), "console.log('api')\n", "utf8");
  await writeFile(join(projectRoot, "apps/web/index.html"), "<div id=\"root\"></div>\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.workspace.appPath, ".");
  assert.equal(envelope.data.detectedStack.kind, "node");
  assert.equal(envelope.data.detectedStack.packageManager, "npm");
  assert.deepEqual(envelope.data.detectedStack.workspacePackageJsonPaths, [
    "apps/api/package.json",
    "apps/web/package.json",
  ]);

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /COPY apps\/api\/package\.json \.\/apps\/api\/package\.json/);
  assert.match(dockerfile, /COPY apps\/web\/package\.json \.\/apps\/web\/package\.json/);
  assert.ok(
    dockerfile.indexOf("COPY apps/api/package.json ./apps/api/package.json") <
      dockerfile.indexOf("RUN npm ci"),
  );
  assert.ok(
    dockerfile.indexOf("COPY apps/web/package.json ./apps/web/package.json") <
      dockerfile.indexOf("RUN npm ci"),
  );
}

async function verifyExistingDockerfileInWorkspace(projectRoot) {
  await mkdir(join(projectRoot, "apps/api"), { recursive: true });
  await writePackage(projectRoot, {
    private: true,
    workspaces: {
      packages: [
        "apps/*",
      ],
    },
  });
  await writePackage(join(projectRoot, "apps/api"), {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "apps/api/server.js"), "console.log('api')\n", "utf8");
  await writeFile(join(projectRoot, "apps/api/Dockerfile"), "FROM node:22-slim\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.provider, "dockerfile-existing");
  assert.equal(envelope.data.workspace.appPath, "apps/api");
  assert.equal(envelope.data.workspace.buildContextPath, "apps/api");
  assert.equal(envelope.data.files.buildContextPath, "apps/api");
  assert.deepEqual(envelope.data.files.reused, ["apps/api/Dockerfile"]);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /context: "..\/..\/..\/..\/apps\/api"/);
  assert.match(compose, /dockerfile: "Dockerfile"/);
}

async function verifyEnvironmentDiagnostics(projectRoot) {
  await mkdir(projectRoot, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
      pg: "^8.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), [
    "const token = process.env.API_TOKEN;",
    "const publicValue = process.env.NEXT_PUBLIC_SITE_URL;",
    "console.log(token, publicValue);",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, ".env.example"), [
    "DATABASE_URL=postgresql://example",
    "API_TOKEN=",
    "NEXT_PUBLIC_SITE_URL=http://localhost:3000",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, ".env"), "API_TOKEN=real-value-is-not-read\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);

  assert.equal(envelope.ok, true);
  assert.ok(envelope.data.environment.referenced.some((variable) => variable.name === "API_TOKEN"));
  assert.ok(!envelope.data.environment.provided.includes("API_TOKEN"));
  assert.ok(envelope.data.environment.provided.includes("DATABASE_URL"));
  assert.ok(envelope.data.environment.missing.some((variable) => variable.name === "API_TOKEN"));
  assert.ok(!envelope.data.environment.missing.some((variable) => variable.name === "NEXT_PUBLIC_SITE_URL"));
  assert.equal(envelope.data.environment.localEnvFiles[0].ignored, true);
  assert.deepEqual(envelope.data.environment.localEnvFiles[0].variables, ["API_TOKEN"]);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /DATABASE_URL: "postgresql:\/\/loom:loom@postgres:5432\/loom"/);
  assert.doesNotMatch(compose, /real-value-is-not-read/);
}

async function verifyBootstrapDiagnostics(projectRoot) {
  await mkdir(join(projectRoot, "prisma"), { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      build: "next build",
      start: "next start",
      migrate: "prisma migrate deploy",
    },
    dependencies: {
      next: "16.2.2",
      prisma: "^6.0.0",
      "@prisma/client": "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "prisma/schema.prisma"), "datasource db { provider = \"postgresql\" url = env(\"DATABASE_URL\") }\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);
  assert.equal(envelope.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.ok(spec.bootstrap.tasks.some((task) => task.kind === "prisma"));
  assert.equal(spec.bootstrap.tasks.find((task) => task.kind === "prisma").automatic, false);
  assert.match(spec.bootstrap.warnings.join("\n"), /does not run migrations automatically/);
}

async function verifyNestedBootstrapDiagnostics(projectRoot) {
  await mkdir(join(projectRoot, "apps/api/prisma"), { recursive: true });
  await mkdir(join(projectRoot, "apps/api/src/main/resources/db/migration"), { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "apps/api/package.json"), `${JSON.stringify({
    scripts: {
      migrate: "prisma migrate deploy",
    },
    dependencies: {
      prisma: "^6.0.0",
    },
  }, null, 2)}\n`, "utf8");
  await writeFile(join(projectRoot, "apps/api/prisma/schema.prisma"), "datasource db { provider = \"mysql\" url = env(\"DATABASE_URL\") }\n", "utf8");
  await writeFile(join(projectRoot, "apps/api/src/main/resources/db/migration/V1__init.sql"), "select 1;\n", "utf8");

  const envelope = runDeployPrepare(projectRoot);
  assert.equal(envelope.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.ok(spec.bootstrap.tasks.some((task) => task.kind === "prisma" && task.command === "cd \"apps/api\" && npx prisma migrate deploy"));
  assert.ok(spec.bootstrap.tasks.some((task) => task.kind === "flyway" && task.command === "cd \"apps/api\" && flyway migrate"));
}

async function verifyRepairDiagnostics(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  const envelope = runDeployPrepare(projectRoot);
  assert.equal(envelope.ok, true);

  const specPath = join(projectRoot, ".loom/deployment/specs/local.json");
  const spec = JSON.parse(await readFile(specPath, "utf8"));
  await mkdir(join(projectRoot, ".loom/deployment/state"), { recursive: true });
  await writeFile(join(projectRoot, ".loom/deployment/state/repair-request.json"), `${JSON.stringify({
    schemaVersion: 1,
    repairId: "deploy-repair-test",
    createdAt: new Date().toISOString(),
    projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    provider: spec.provider,
    failureKind: "container_start",
    command: ["docker", "compose", "-f", spec.files.composePath, "logs", "--tail", "120"],
    exitCode: 1,
    stdoutTail: [],
    stderrTail: [
      "Error: Cannot find module '../lightningcss.linux-arm64-gnu.node'",
      "relation users does not exist",
    ],
    providerCandidates: spec.providerCandidates,
    environment: spec.environment,
    bootstrap: {
      tasks: [
        {
          kind: "prisma",
          command: "npx prisma migrate deploy",
          automatic: false,
          reason: "test",
        },
      ],
      warnings: ["diagnostic only"],
    },
    diagnostics: [
      {
        code: "native_optional_dependency",
        severity: "error",
        message: "native package missing",
        evidence: ["Error: Cannot find module '../lightningcss.linux-arm64-gnu.node'"],
        suggestedAction: "repair native optional dependency",
      },
      {
        code: "missing_database_table",
        severity: "error",
        message: "table missing",
        evidence: ["relation users does not exist"],
        suggestedAction: "review migrations",
      },
    ],
    suggestedActions: [
      "native_optional_dependency: repair native optional dependency",
      "missing_database_table: review migrations",
    ],
    editableFiles: [spec.files.composePath, spec.files.dockerfilePath, spec.files.dockerignorePath].filter(Boolean),
    protectedFiles: [],
    instruction: "Repair generated deployment files.",
    maxAttempts: 10,
    attempts: 1,
    status: "pending",
  }, null, 2)}\n`, "utf8");

  const repair = runDeployRepair(projectRoot);
  assert.equal(repair.ok, true);
  assert.equal(repair.data.hasRepairRequest, true);
  assert.ok(repair.data.diagnostics.some((diagnostic) => diagnostic.code === "native_optional_dependency"));
  assert.ok(repair.data.diagnostics.some((diagnostic) => diagnostic.code === "missing_database_table"));
  assert.ok(repair.data.bootstrap.tasks.some((task) => task.kind === "prisma"));
  assertDeployAssetRepairInstruction(repair, spec.files.composePath);
}

async function verifyDeploymentAssetRepairAutoRunnableInstruction(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"config\" ]; then",
    "  echo 'services.app.environment must be a mapping' >&2",
    "  exit 1",
    "fi",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "exit 0",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const run = runDeployRun(projectRoot, {
    PATH: `${binDir}:${process.env.PATH}`,
  });
  assert.equal(run.ok, true);
  assert.equal(run.data.completed, false);
  assert.equal(run.data.failedPhase, "up");
  assert.equal(run.data.nextAction, "edit-and-rerun-up");
  assert.equal(run.data.repair.failureOwner, "deployment_assets");
  assert.equal(run.data.repair.repairRoute, "deploy_repair");
  assert.equal(run.data.repair.nextAction, "edit-and-rerun");
  assertDeployAssetRepairInstruction(run, ".loom/deployment/specs/generated/compose.yaml");

  const repair = runDeployRepair(projectRoot);
  assert.equal(repair.ok, true);
  assert.equal(repair.data.hasRepairRequest, true);
  assert.equal(repair.data.nextAction, "edit-and-rerun");
  assertDeployAssetRepairInstruction(repair, ".loom/deployment/specs/generated/compose.yaml");
}

async function verifyRegistryNetworkRepair(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  await mkdir(join(projectRoot, ".loom/deployment/state"), { recursive: true });
  await writeFile(join(projectRoot, ".loom/deployment/state/repair-request.json"), `${JSON.stringify({
    schemaVersion: 1,
    repairId: "deploy-repair-registry-network",
    createdAt: new Date().toISOString(),
    projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    provider: spec.provider,
    failureKind: "registry_network",
    command: ["docker", "compose", "-f", spec.files.composePath, "up", "-d", "--build"],
    exitCode: 1,
    stdoutTail: [
      "#3 ERROR: failed to authorize: DeadlineExceeded: failed to fetch oauth token",
    ],
    stderrTail: [
      "failed to solve: failed to fetch oauth token: dial tcp 108.160.163.116:443: i/o timeout",
    ],
    providerCandidates: spec.providerCandidates,
    environment: spec.environment,
    bootstrap: spec.bootstrap,
    diagnostics: [
      {
        code: "registry_network",
        severity: "error",
        message: "registry unavailable",
        evidence: ["failed to fetch oauth token"],
        suggestedAction: "fix Docker registry access",
      },
    ],
    suggestedActions: ["fix Docker registry access"],
    editableFiles: [],
    protectedFiles: [],
    instruction: "Fix Docker registry/network access.",
    maxAttempts: 10,
    attempts: 1,
    status: "pending",
  }, null, 2)}\n`, "utf8");

  const repair = runDeployRepair(projectRoot);
  assert.equal(repair.ok, true);
  assert.equal(repair.data.failureKind, "registry_network");
  assert.deepEqual(repair.data.editableFiles, []);
  assert.equal(repair.data.nextAction, "none");
  assert.ok(repair.data.suggestedActions.some((action) => /registry|pre-pull|network/i.test(action)));
}

async function verifyRuntimeContractPrepare(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build && tsc -p tsconfig.server.json",
      start: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
      vite: "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/ready",
    frontendOutputDir: "dist/web",
  });

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtimeContract.source, "accepted_aac");
  assert.equal(spec.runtimeContract.port, 4173);
  assert.equal(spec.runtime.containerPort, 4173);
  assert.equal(spec.runtime.healthcheck.path, "/ready");
  assert.equal(spec.runtimeContract.previewPath, "/");
  assert.equal(spec.runtimeContract.frontendOutputDir, "dist/web");

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /4173:4173/);
}

async function verifyRuntimeContractPromotesRootWorkspaceWhenChildrenAreSourceOnly(projectRoot) {
  await mkdir(join(projectRoot, "apps/api/src"), { recursive: true });
  await mkdir(join(projectRoot, "apps/staff-web/src"), { recursive: true });
  await mkdir(join(projectRoot, "packages/domain/src"), { recursive: true });
  await mkdir(join(projectRoot, "scripts"), { recursive: true });
  await writePackage(projectRoot, {
    private: true,
    workspaces: [
      "apps/*",
      "packages/*",
    ],
    scripts: {
      build: "tsc -p tsconfig.json && node scripts/build-staff-web.mjs",
      dev: "node dist/apps/api/src/main.js",
      test: "vitest run",
    },
    devDependencies: {
      typescript: "^5.6.3",
      vitest: "^2.1.4",
    },
  });
  await writeFile(join(projectRoot, "package-lock.json"), `${JSON.stringify({
    lockfileVersion: 3,
    requires: true,
    packages: {},
  }, null, 2)}\n`, "utf8");
  await writeFile(join(projectRoot, "apps/api/src/main.ts"), "console.log('api')\n", "utf8");
  await writeFile(join(projectRoot, "apps/staff-web/src/index.html"), "<div id=\"app\"></div>\n", "utf8");
  await writeFile(join(projectRoot, "scripts/build-staff-web.mjs"), "console.log('build web')\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run dev",
    previewPath: "/",
    healthPath: "/",
    frontendOutputDir: "dist/apps/staff-web",
  });

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);
  assert.equal(prepare.data.workspace.appPath, ".");
  assert.equal(prepare.data.workspace.buildContextPath, ".");
  assert.equal(prepare.data.files.buildContextPath, ".");
  assert.equal(prepare.data.detectedStack.kind, "node");
  assert.equal(prepare.data.detectedStack.packageManager, "npm");
  assert.equal(prepare.data.detectedStack.buildCommand, "npm run build");
  assert.equal(prepare.data.detectedStack.startCommand, "npm run dev");
  assert.equal(prepare.data.detectedStack.outputDirectory, "dist/apps/staff-web");

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /FROM node:22-slim AS deps/);
  assert.match(dockerfile, /RUN npm run build/);
  assert.match(dockerfile, /CMD \["sh","-c","npm run dev"\]/);
  assert.doesNotMatch(dockerfile, /loom could not detect a runnable stack/);
}

async function verifyDeployStatusUsesLatestPreparedSpec(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  const now = new Date().toISOString();
  await writeFile(join(projectRoot, ".loom/deployment/state/local.json"), `${JSON.stringify({
    schemaVersion: 1,
    provider: "dockerfile-template",
    serviceName: "api",
    appServiceName: "api",
    imageName: "api:loom-local",
    projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    composePath: spec.files.composePath,
    containerName: "loom-api",
    containerId: null,
    running: true,
    url: "http://localhost:3000",
    health: {
      status: "unknown",
      url: null,
      checkedAt: null,
      statusCode: null,
      error: null,
    },
    startedAt: now,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "if [ \"$1\" = \"inspect\" ]; then exit 1; fi",
    "exit 1",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const status = runDeployStatus(projectRoot, {
    PATH: `${binDir}:${process.env.PATH}`,
  });
  assert.equal(status.ok, true);
  assert.equal(status.data.running, false);
  assert.equal(status.data.serviceName, spec.serviceName);
  assert.equal(status.data.appServiceName, spec.compose.selectedService);
  assert.notEqual(status.data.serviceName, "api");
}

async function verifyRuntimeContractSuppressesHeuristicDependencyServices(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build && tsc -p tsconfig.server.json",
      start: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
      pg: "^8.0.0",
      prisma: "^6.0.0",
      vite: "^6.0.0",
    },
  });
  await mkdir(join(projectRoot, "prisma"), { recursive: true });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.DATABASE_URL)\n", "utf8");
  await writeFile(join(projectRoot, "prisma/schema.prisma"), "datasource db { provider = \"sqlite\" url = env(\"DATABASE_URL\") }\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/ready",
    frontendOutputDir: "dist/web",
  });

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtimeContract.source, "accepted_aac");
  assert.equal(spec.runtimeContract.dependencyServicePolicy, "contract_only");
  assert.deepEqual(spec.detectedStack.services, []);
  assert.ok(!spec.environment.provided.includes("DATABASE_URL"));

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.doesNotMatch(compose, /postgres:16-alpine/);
  assert.doesNotMatch(compose, /depends_on:/);
  assert.doesNotMatch(compose, /DATABASE_URL: "postgresql:\/\/loom:loom@postgres:5432\/loom"/);
}

async function verifyRuntimeContractDerivesDependencyServicesFromEnvironment(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build && tsc -p tsconfig.server.json",
      start: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
      vite: "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.SPRING_DATASOURCE_URL)\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    runtimeKind: "spring_boot_postgres_serves_vite_static",
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/ready",
    frontendOutputDir: "dist/web",
    environment: {
      required: [
        "PORT",
        "SPRING_DATASOURCE_URL",
        "SPRING_DATASOURCE_USERNAME",
        "SPRING_DATASOURCE_PASSWORD",
      ],
      optional: ["SPRING_PROFILES_ACTIVE"],
    },
  });
  await writeTechnicalBaseline(projectRoot, {
    backend: "Java + Spring Boot",
    persistence: "PostgreSQL",
    dataAccess: "Spring Data JPA",
    web: "React + Vite",
  });

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtimeContract.dependencyServicePolicy, "contract_only");
  assert.equal(spec.runtimeContract.probeKind, "http");
  assert.deepEqual(spec.runtimeContract.environment.required, [
    "PORT",
    "SPRING_DATASOURCE_URL",
    "SPRING_DATASOURCE_USERNAME",
    "SPRING_DATASOURCE_PASSWORD",
  ]);
  assert.ok(spec.runtimeContract.dependencyServices.some((service) => service.kind === "postgres"));
  assert.ok(spec.detectedStack.services.some((service) => service.kind === "postgres"));
  assert.ok(spec.environment.provided.includes("SPRING_DATASOURCE_URL"));
  assert.ok(!spec.environment.missing.some((variable) => variable.name === "SPRING_DATASOURCE_URL"));

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /postgres:16-alpine/);
  assert.match(compose, /SPRING_DATASOURCE_URL: "jdbc:postgresql:\/\/postgres:5432\/loom"/);
}

async function verifyRuntimeContractUsesDetectedSqlServiceForGenericDatasource(projectRoot) {
  await mkdir(join(projectRoot, "src/main/resources"), { recursive: true });
  await writeFile(join(projectRoot, "pom.xml"), [
    "<project>",
    "  <properties>",
    "    <java.version>21</java.version>",
    "  </properties>",
    "  <dependencies>",
    "    <dependency>",
    "      <groupId>com.mysql</groupId>",
    "      <artifactId>mysql-connector-j</artifactId>",
    "    </dependency>",
    "  </dependencies>",
    "</project>",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, "src/main/resources/application.properties"), "spring.datasource.url=${SPRING_DATASOURCE_URL}\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    runtimeKind: "spring_boot_serves_static",
    startPort: 8080,
    buildCommand: "mvn -DskipTests package",
    startCommand: "java -jar target/demo.jar",
    previewPath: "/",
    healthPath: "/actuator/health",
    frontendOutputDir: "dist",
    environment: {
      required: [
        "SPRING_DATASOURCE_URL",
        "SPRING_DATASOURCE_USERNAME",
        "SPRING_DATASOURCE_PASSWORD",
      ],
      optional: ["PORT"],
    },
  });

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtimeContract.dependencyServices.length, 0);
  assert.ok(spec.detectedStack.services.some((service) => service.kind === "mysql"));
  assert.ok(!spec.detectedStack.services.some((service) => service.kind === "postgres"));
  assert.ok(spec.environment.provided.includes("SPRING_DATASOURCE_URL"));

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /mysql:8/);
  assert.match(compose, /SPRING_DATASOURCE_URL: "jdbc:mysql:\/\/mysql:3306\/loom"/);
  assert.doesNotMatch(compose, /postgres:16-alpine/);
}

async function verifyStaleRuntimeContractSpecReprepare(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build && tsc -p tsconfig.server.json",
      start: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
      pg: "^8.0.0",
      prisma: "^6.0.0",
      vite: "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.DATABASE_URL)\n", "utf8");

  const heuristicPrepare = runDeployPrepare(projectRoot);
  assert.equal(heuristicPrepare.ok, true);
  let spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtimeContract.source, "heuristic");
  assert.ok(spec.detectedStack.services.some((service) => service.kind === "postgres"));

  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/ready",
    frontendOutputDir: "dist/web",
  });
  const validate = runDeployValidate(projectRoot);
  assert.equal(validate.ok, true);

  spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtimeContract.source, "accepted_aac");
  assert.equal(spec.runtimeContract.dependencyServicePolicy, "contract_only");
  assert.ok(spec.detectedStack.services.some((service) => service.kind === "postgres"));
  assert.equal(spec.runtime.containerPort, 4173);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /postgres:16-alpine/);
  assert.match(compose, /4173:4173/);
}

async function verifyDeployUsesPreviousCompletedPhaseRuntimeContract(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build && tsc -p tsconfig.server.json",
      start: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
      pg: "^8.0.0",
      prisma: "^6.0.0",
      vite: "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.DATABASE_URL)\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/ready",
    frontendOutputDir: "dist/web",
    activePhaseId: "phase-2",
    phase1Status: "completed",
    includePhase2: true,
  });

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtimeContract.source, "accepted_aac");
  assert.equal(spec.runtimeContract.ref, ".loom/deliveries/delivery-runtime/artifacts/architecture/phase-1/aac.json#/runtimeDelivery");
  assert.equal(spec.runtimeContract.dependencyServicePolicy, "contract_only");
  assert.ok(spec.detectedStack.services.some((service) => service.kind === "postgres"));

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.match(compose, /postgres:16-alpine/);
  assert.match(compose, /DATABASE_URL: "postgresql:\/\/loom:loom@postgres:5432\/loom"/);
}

async function verifyTechnicalBaselineOnlyDoesNotProvisionDatabase(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      build: "vite build",
      start: "vite preview --host 0.0.0.0 --port 4173",
    },
    dependencies: {
      vite: "^6.0.0",
    },
  });
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/",
    frontendOutputDir: "dist",
  });
  await writeTechnicalBaseline(projectRoot, {
    web: "React + Vite",
    backend: "No independent backend",
    persistence: "PostgreSQL",
    dataAccess: "Prisma",
  });

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);
  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.codeEvidence.warningCount, 1);
  assert.ok(!spec.detectedStack.services.some((service) => service.kind === "postgres"));

  const evidence = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/evidence/latest-code-evidence.json"), "utf8"));
  assert.match(evidence.warnings.join("\n"), /will not start that service from baseline alone/i);

  const compose = await readFile(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml"), "utf8");
  assert.doesNotMatch(compose, /postgres:16-alpine/);
}

async function verifyUnknownDatabaseKindBlocksPrepare(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.DATABASE_URL)\n", "utf8");

  const prepare = runDeployPrepare(projectRoot, [], [2]);
  assert.equal(prepare.ok, false);
  assert.equal(prepare.error.code, "DEPLOY_SOURCE_INSUFFICIENT");
  assert.equal(prepare.error.details.code, "DEPLOY_SOURCE_INSUFFICIENT");
  assert.equal(prepare.error.details.nextAction, "execution_repair");
  assert.ok(prepare.error.details.missingFacts.some((fact) => fact.type === "database_kind"));
  assert.ok(await fileExists(join(projectRoot, ".loom/deployment/evidence/latest-code-evidence.json")));
  assert.equal(await fileExists(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml")), false);
}

async function verifyBaselineDatabaseConflictBlocksPrepare(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
      mysql2: "^3.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log(process.env.DATABASE_URL)\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/",
    frontendOutputDir: "dist",
  });
  await writeTechnicalBaseline(projectRoot, {
    backend: "Node.js + Express",
    persistence: "PostgreSQL",
    dataAccess: "Raw SQL / lightweight wrapper",
  });

  const prepare = runDeployPrepare(projectRoot, [], [2]);
  assert.equal(prepare.ok, false);
  assert.equal(prepare.error.code, "DEPLOY_CONFLICT");
  assert.equal(prepare.error.details.code, "DEPLOY_CONFLICT");
  assert.equal(prepare.error.details.nextAction, "ask_user");
  assert.ok(prepare.error.details.conflicts.some((conflict) => conflict.type === "technical_baseline_code_conflict"));
  assert.ok(await fileExists(join(projectRoot, ".loom/deployment/evidence/latest-code-evidence.json")));
  assert.equal(await fileExists(join(projectRoot, ".loom/deployment/specs/generated/compose.yaml")), false);
}

async function verifyDeployRunDockerUnavailableWritesRepairRequest(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"compose\" ]; then exit 0; fi",
    "if [ \"$1\" = \"version\" ]; then echo 'Cannot connect to the Docker daemon' >&2; exit 1; fi",
    "exit 1",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const run = runDeployRun(projectRoot, {
    PATH: `${binDir}:${process.env.PATH}`,
  });
  assert.equal(run.ok, true);
  assert.equal(run.data.completed, false);
  assert.equal(run.data.failedPhase, "up");
  assert.equal(run.data.nextAction, "fix-docker");
  assert.equal(run.data.repair.failureKind, "docker_unavailable");
  assert.deepEqual(run.data.repair.editableFiles, []);

  const repair = runDeployRepair(projectRoot);
  assert.equal(repair.ok, true);
  assert.equal(repair.data.hasRepairRequest, true);
  assert.equal(repair.data.failureKind, "docker_unavailable");
  assert.equal(repair.data.nextAction, "none");
  assert.ok(repair.data.suggestedActions.some((action) => /start docker|daemon|permissions/i.test(action)));
  assert.ok(!repair.data.suggestedActions.some((action) => /edit .*deployment files|repair the selected provider/i.test(action)));
}

async function verifyRuntimeContractStartFailureRoutesToDeliveryRepair(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      build: "node -e \"require('fs').mkdirSync('dist', { recursive: true })\"",
      dev: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 4173,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/health",
    frontendOutputDir: "dist/web",
  });
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"config\" ]; then exit 0; fi",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"up\" ]; then exit 0; fi",
    "if [ \"$1\" = \"inspect\" ]; then echo 'container-id true'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"logs\" ]; then",
    "  echo 'app | npm error Missing script: \"start\"'",
    "  exit 0",
    "fi",
    "exit 0",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const run = runDeployRun(projectRoot, {
    PATH: `${binDir}:${process.env.PATH}`,
  });
  assert.equal(run.ok, true);
  assert.equal(run.data.completed, false);
  assert.equal(run.data.failedPhase, "up");
  assert.equal(run.data.nextAction, "execution_repair");
  assert.equal(run.data.repair.failureKind, "start_command_failed");
  assert.equal(run.data.repair.failureOwner, "application_code");
  assert.equal(run.data.repair.repairRoute, "execution_repair");
  assert.equal(run.data.repair.nextAction, "execution-repair");
  assert.deepEqual(run.data.repair.editableFiles, []);
  assert.equal(run.instruction.mode, "run_cli");
  assert.deepEqual(run.instruction.command.argv, [
    "repair",
    "request",
    "--type",
    "execution",
    "--source",
    "deploy",
    "--failure-ref",
    ".loom/deployment/state/latest-failure.json",
  ]);
}

async function verifyApplicationStartupFailureRoutesToExecutionRepair(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      build: "node -e \"require('fs').mkdirSync('dist', { recursive: true })\"",
      start: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 8080,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/actuator/health",
    frontendOutputDir: "dist/web",
  });
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"config\" ]; then exit 0; fi",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"up\" ]; then exit 0; fi",
    "if [ \"$1\" = \"inspect\" ]; then echo 'container-id true'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"logs\" ]; then",
    "  echo 'app | org.springframework.boot.context.event.ApplicationFailedEvent'",
    "  echo 'app | APPLICATION FAILED TO START'",
    "  echo 'app | org.flywaydb.core.api.FlywayException: Unsupported Database: PostgreSQL 15.18'",
    "  exit 0",
    "fi",
    "exit 0",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const run = runDeployRun(projectRoot, {
    PATH: `${binDir}:${process.env.PATH}`,
  });
  assert.equal(run.ok, true);
  assert.equal(run.data.completed, false);
  assert.equal(run.data.failedPhase, "up");
  assert.equal(run.data.nextAction, "execution_repair");
  assert.equal(run.data.repair.failureKind, "application_startup_failed");
  assert.equal(run.data.repair.failureOwner, "application_code");
  assert.equal(run.data.repair.repairRoute, "execution_repair");
  assert.deepEqual(run.data.repair.editableFiles, []);
  assert.ok(run.data.repair.diagnostics.some((diagnostic) => diagnostic.code === "framework_startup_failed"));
  assert.ok(run.data.repair.errorWindow.lines.some((line) => /FlywayException|APPLICATION FAILED TO START/.test(line)));
}

async function verifyRuntimeContractBuildFailureRoutesToDeliveryRepair(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      build: "npm run build:server",
      start: "node dist/server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 3000,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/",
    healthPath: "/health",
    frontendOutputDir: "dist/web",
  });
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"config\" ]; then exit 0; fi",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"up\" ]; then",
    "  echo '#13 RUN npm run build'",
    "  echo \"#13 1.262 src/server/services/accountService.ts(89,52): error TS7006: Parameter 'tx' implicitly has an 'any' type.\"",
    "  echo 'failed to solve: process \"/bin/sh -c npm run build\" did not complete successfully: exit code: 2'",
    "  exit 1",
    "fi",
    "exit 0",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const run = runDeployRun(projectRoot, {
    PATH: `${binDir}:${process.env.PATH}`,
  });
  assert.equal(run.ok, true);
  assert.equal(run.data.completed, false);
  assert.equal(run.data.failedPhase, "up");
  assert.equal(run.data.repair.failureKind, "build_command_failed");
  assert.equal(run.data.repair.failureOwner, "application_code");
  assert.equal(run.data.repair.repairRoute, "execution_repair");
  assert.equal(run.data.repair.failureRef, ".loom/deployment/state/latest-failure.json");
  assert.equal(run.data.repair.fullLogRef, ".loom/deployment/logs/local.log");
  assert.ok(run.data.repair.errorWindow.lines.some((line) => /TS7006|npm run build/.test(line)));
  assert.equal(run.data.repair.nextAction, "execution-repair");
  assert.equal(run.data.nextAction, "execution_repair");
  assert.equal(run.instruction.mode, "run_cli");
  assert.deepEqual(run.instruction.command.argv, [
    "repair",
    "request",
    "--type",
    "execution",
    "--source",
    "deploy",
    "--failure-ref",
    ".loom/deployment/state/latest-failure.json",
  ]);
  assert.deepEqual(run.data.repair.editableFiles, []);
  assert.match(run.data.repair.instruction, /repair request --type execution --source deploy/i);

  const failure = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/state/latest-failure.json"), "utf8"));
  assert.equal(failure.failureOwner, "application_code");
  assert.equal(failure.repairRoute, "execution_repair");
  assert.equal(failure.failedContract.field, "build.command");
  assert.equal(failure.sourceRefs.deploymentSpecRef, ".loom/deployment/specs/local.json");
  assert.equal(failure.evidence.fullLogRef, ".loom/deployment/logs/local.log");
  assert.ok(failure.evidence.errorWindow.lines.some((line) => /TS7006|exit code: 2/.test(line)));

  const compactRepair = runLoom(["deploy", "repair", "--project-root", projectRoot], [0], {
    LOOM_COMPACT_OUTPUT: "1",
  });
  assert.equal(compactRepair.ok, true);
  assert.equal(compactRepair.compact, true);
  assert.equal(compactRepair.data.fullLogRef, ".loom/deployment/logs/local.log");
  assert.ok(compactRepair.data.errorWindow.lines.some((line) => /TS7006|exit code: 2/.test(line)));

  const repair = runRepairRequest(projectRoot, [
    "--type",
    "execution",
    "--source",
    "deploy",
    "--failure-ref",
    ".loom/deployment/state/latest-failure.json",
  ]);
  assert.equal(repair.ok, true);
  assert.equal(repair.data.operation, "deploy_execution_repair_request_created");
  const repairRequest = JSON.parse(await readFile(join(projectRoot, repair.data.requestRef), "utf8"));
  assert.equal(repairRequest.syntheticTask.mutatesOriginalTaskPlan, false);
  assert.equal(repairRequest.syntheticTask.writeBoundary.forbiddenPaths.includes(".loom"), true);
  assert.equal(repairRequest.executionRules.evidenceReadPolicy.firstRead, "deploymentFailureRef#.evidence.errorWindow");
  assert.equal(repair.instruction.mode, "execute_task");
  assert.match(repair.data.requestRef, /^\.loom\/deployment\/repairs\/deploy-exec-repair-/);

  await writeFile(join(projectRoot, repair.data.resultFile), `${JSON.stringify({
    schemaVersion: "1.0",
    repairId: repair.data.repairId,
    status: "failed",
    deploymentFailureRef: ".loom/deployment/state/latest-failure.json",
    changedFiles: ["package.json"],
    runtimeDeliveryEvidence: {
      source: "deploy_failure_repair",
      addressedFailedContractFields: ["build.command"],
      codeLevelChecks: [{
        checkId: "repair-build-command",
        status: "failed",
        evidence: "Build script chain is still failing.",
      }],
      commandsRun: [{
        command: "npm run build",
        status: "failed",
        environment: "local_warm",
      }],
      unverifiedItems: [],
    },
    selfRepairSummary: {
      attempted: true,
      attemptCount: 1,
      stopReason: "same_failure_repeated_without_progress",
      progressObserved: false,
    },
    notes: [],
  }, null, 2)}\n`, "utf8");
  const failedSubmit = runRepairSubmit(projectRoot, [
    "--type",
    "execution",
    "--source",
    "deploy",
    "--repair-id",
    repair.data.repairId,
    "--result-file",
    repair.data.resultFile,
  ]);
  assert.equal(failedSubmit.ok, true);
  assert.equal(failedSubmit.data.accepted, false);
  assert.equal(failedSubmit.data.nextAction, "manual_review");
  assert.equal(failedSubmit.instruction.mode, "report_blocked");
  assert.equal(failedSubmit.instruction.autoContinue, false);

  await writeFile(join(projectRoot, repair.data.resultFile), `${JSON.stringify({
    schemaVersion: "1.0",
    repairId: repair.data.repairId,
    status: "completed",
    deploymentFailureRef: ".loom/deployment/state/latest-failure.json",
    changedFiles: ["package.json", "server.js"],
    runtimeDeliveryEvidence: {
      source: "deploy_failure_repair",
      addressedFailedContractFields: ["build.command"],
      codeLevelChecks: [{
        checkId: "repair-build-command",
        status: "passed",
        evidence: "Build script chain was repaired in application code.",
      }],
      commandsRun: [{
        command: "npm run build",
        status: "passed",
        environment: "local_warm",
      }],
      unverifiedItems: [],
    },
    selfRepairSummary: {
      attempted: true,
      attemptCount: 1,
      stopReason: "verification_passed",
      progressObserved: true,
    },
    notes: [],
  }, null, 2)}\n`, "utf8");
  const submit = runRepairSubmit(projectRoot, [
    "--type",
    "execution",
    "--source",
    "deploy",
    "--repair-id",
    repair.data.repairId,
    "--result-file",
    repair.data.resultFile,
  ]);
  assert.equal(submit.ok, true);
  assert.equal(submit.data.nextAction, "deploy_retry");
  assert.equal(submit.instruction.mode, "run_cli");
  assert.deepEqual(submit.instruction.command.argv, ["deploy", "run"]);
}

async function verifyDeployInspect(projectRoot) {
  const unprepared = runDeployInspect(projectRoot);
  assert.equal(unprepared.ok, true);
  assert.equal(unprepared.data.prepared, false);
  assert.equal(unprepared.data.summary.running, false);

  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), [
    "const secret = process.env.API_SECRET;",
    "console.log(secret);",
    "",
  ].join("\n"), "utf8");
  await writeFile(join(projectRoot, ".env.example"), "API_SECRET=\n", "utf8");

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);
  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  await mkdir(join(projectRoot, ".loom/deployment/state"), { recursive: true });
  await writeFile(join(projectRoot, ".loom/deployment/state/repair-request.json"), `${JSON.stringify({
    schemaVersion: 1,
    repairId: "deploy-repair-inspect",
    createdAt: new Date().toISOString(),
    projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    provider: spec.provider,
    failureKind: "healthcheck",
    command: ["GET", spec.runtime.healthcheck.url],
    exitCode: 1,
    stdoutTail: [],
    stderrTail: ["health failed"],
    providerCandidates: spec.providerCandidates,
    environment: spec.environment,
    bootstrap: spec.bootstrap,
    diagnostics: [],
    suggestedActions: ["inspect"],
    editableFiles: [spec.files.composePath, spec.files.dockerfilePath, spec.files.dockerignorePath].filter(Boolean),
    protectedFiles: [],
    instruction: "Inspect failure.",
    maxAttempts: 10,
    attempts: 1,
    status: "pending",
  }, null, 2)}\n`, "utf8");

  const inspect = runDeployInspect(projectRoot);
  assert.equal(inspect.ok, true);
  assert.equal(inspect.data.prepared, true);
  assert.equal(inspect.data.provider, "dockerfile-template");
  assert.equal(inspect.data.summary.appPath, ".");
  assert.equal(inspect.data.summary.hasRepairRequest, true);
  assert.equal(inspect.data.summary.missingEnvCount, 1);
  assert.equal(inspect.data.repair.failureKind, "healthcheck");
  assert.ok(inspect.data.files.composePath);
  assert.equal(inspect.data.codeEvidenceReadGuide.ref, ".loom/deployment/evidence/latest-code-evidence.json");
  assert.ok(
    inspect.data.codeEvidenceReadGuide.targetedFields.some((field) => field.field === "dependencyFacts.services"),
    "deploy inspect must expose targeted dependency evidence guidance.",
  );
  assert.ok(
    inspect.data.codeEvidenceReadGuide.readPolicy.avoid.some((rule) => /full evidence JSON/i.test(rule)),
    "deploy inspect must tell agents not to print the full code evidence artifact.",
  );
}

async function verifyDeployInspectRefresh(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "if [ \"$1\" = \"inspect\" ]; then echo 'container123 true'; exit 0; fi",
    "echo ok",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const prepare = runDeployPrepare(projectRoot, ["--healthcheck-disabled"]);
  assert.equal(prepare.ok, true);
  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  spec.runtimeContract.previewPath = "/health";
  await writeFile(join(projectRoot, ".loom/deployment/specs/local.json"), `${JSON.stringify(spec, null, 2)}\n`, "utf8");
  await mkdir(join(projectRoot, ".loom/deployment/state"), { recursive: true });
  await writeFile(join(projectRoot, ".loom/deployment/state/local.json"), `${JSON.stringify({
    schemaVersion: 1,
    provider: spec.provider,
    serviceName: spec.serviceName,
    appServiceName: spec.compose.selectedService,
    imageName: spec.imageName,
    projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    composePath: spec.files.composePath,
    containerName: `loom-${spec.serviceName}`,
    containerId: null,
    running: false,
    url: null,
    health: {
      status: "unknown",
      url: null,
      checkedAt: null,
      statusCode: null,
      error: null,
    },
    startedAt: null,
    updatedAt: new Date().toISOString(),
  }, null, 2)}\n`, "utf8");

  const server = await startHealthServer(spec.runtime.hostPort);
  try {
    const inspect = runDeployInspect(projectRoot, ["--refresh"], {
      PATH: `${binDir}:${process.env.PATH}`,
    });
    assert.equal(inspect.ok, true);
    assert.equal(inspect.data.refreshed, true);
    assert.equal(inspect.data.summary.running, true);
    assert.equal(inspect.data.state.running, true);
    assert.equal(inspect.data.state.containerId, "container123");
    assert.equal(inspect.data.state.health.status, "healthy");
  } finally {
    await stopHealthServer(server);
  }
}

async function verifyDeploySuccessClearsFailureAndGuardsRawCompose(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "if [ \"$1\" = \"inspect\" ]; then echo 'container123 true'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"config\" ]; then exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"up\" ]; then echo 'Container started'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && [ \"$4\" = \"logs\" ]; then echo 'server started'; exit 0; fi",
    "echo ok",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const prepare = runDeployPrepare(projectRoot, ["--healthcheck-path", "/health"]);
  assert.equal(prepare.ok, true);
  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  await mkdir(join(projectRoot, ".loom/deployment/state"), { recursive: true });
  await writeFile(join(projectRoot, ".loom/deployment/state/repair-request.json"), "{}\n", "utf8");
  await writeFile(join(projectRoot, ".loom/deployment/state/latest-failure.json"), "{}\n", "utf8");

  const server = await startOkServer(spec.runtime.hostPort);
  try {
    const up = runDeployUp(projectRoot, {
      PATH: `${binDir}:${process.env.PATH}`,
    });
    assert.equal(up.ok, true);
    assert.equal(up.data.started, true);
    assert.equal(up.instruction.mode, "report_done");
    assert.match(up.instruction.routingRule, /Do not run raw docker compose/i);
    assert.equal(await fileExists(join(projectRoot, ".loom/deployment/state/repair-request.json")), false);
    assert.equal(await fileExists(join(projectRoot, ".loom/deployment/state/latest-failure.json")), false);
  } finally {
    await stopHealthServer(server);
  }
}

async function verifyDeployLogsCompact(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "case \"$*\" in",
    "  *logs*)",
    "    i=1",
    "    while [ $i -le 45 ]; do echo \"log line $i\"; i=$((i + 1)); done",
    "    exit 0",
    "    ;;",
    "esac",
    "echo ok",
    "exit 0",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);
  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  await mkdir(join(projectRoot, ".loom/deployment/state"), { recursive: true });
  await writeFile(join(projectRoot, ".loom/deployment/state/local.json"), `${JSON.stringify({
    schemaVersion: 1,
    provider: spec.provider,
    serviceName: spec.serviceName,
    appServiceName: spec.compose.selectedService,
    imageName: spec.imageName,
    projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    composePath: spec.files.composePath,
    containerName: `loom-${spec.serviceName}`,
    containerId: "container123",
    running: true,
    url: spec.runtime.url,
    health: {
      status: "healthy",
      url: spec.runtime.url,
      checkedAt: new Date().toISOString(),
      statusCode: 200,
      error: null,
    },
    startedAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  }, null, 2)}\n`, "utf8");

  const compact = runLoom(["deploy", "logs", "--project-root", projectRoot], [0], {
    PATH: `${binDir}:${process.env.PATH}`,
    LOOM_COMPACT_OUTPUT: "1",
  });
  assert.equal(compact.ok, true);
  assert.equal(compact.compact, true);
  assert.equal(compact.data.fullLogRef, ".loom/deployment/logs/local.log");
  assert.equal(compact.data.lines.length, 40);
  assert.equal(compact.data.lines[0], "log line 6");
  assert.equal(compact.data.linesOmitted, 5);
}

async function verifyExplicitAppPath(projectRoot) {
  await mkdir(join(projectRoot, "apps/web"), { recursive: true });
  await mkdir(join(projectRoot, "apps/admin"), { recursive: true });
  await writePackage(projectRoot, {
    private: true,
    workspaces: ["apps/*"],
  });
  await writePackage(join(projectRoot, "apps/web"), {
    scripts: {
      start: "node web.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writePackage(join(projectRoot, "apps/admin"), {
    scripts: {
      start: "node admin.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "apps/web/web.js"), "console.log('web')\n", "utf8");
  await writeFile(join(projectRoot, "apps/admin/admin.js"), "console.log('admin')\n", "utf8");

  const envelope = runDeployPrepare(projectRoot, ["--app-path", "apps/admin"]);

  assert.equal(envelope.ok, true);
  assert.equal(envelope.data.workspace.appPath, "apps/admin");
  assert.equal(envelope.data.detectedStack.startCommand, "npm run start");

  const dockerfile = await readFile(join(projectRoot, ".loom/deployment/specs/generated/Dockerfile"), "utf8");
  assert.match(dockerfile, /WORKDIR \/app\/apps\/admin/);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.serviceName, "admin");
  assert.equal(spec.workspace.reason, "Using explicit app path apps/admin.");
}

async function verifyHealthcheckCandidateSelection(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeAcceptedRuntimeDelivery(projectRoot, {
    startPort: 3900,
    buildCommand: "npm run build",
    startCommand: "npm run start",
    previewPath: "/app",
    healthPath: "/health",
    frontendOutputDir: "dist/web",
  });

  const envelope = runDeployPrepare(projectRoot);
  assert.equal(envelope.ok, true);

  const specPath = join(projectRoot, ".loom/deployment/specs/local.json");
  const statePath = join(projectRoot, ".loom/deployment/state/local.json");
  const spec = JSON.parse(await readFile(specPath, "utf8"));
  const server = await startHealthServer(spec.runtime.hostPort);
  try {
    await writeFile(statePath, `${JSON.stringify({
      schemaVersion: 1,
      provider: spec.provider,
      serviceName: spec.serviceName,
      imageName: spec.imageName,
      projectRoot,
      specPath: ".loom/deployment/specs/local.json",
      composePath: spec.files.composePath,
      containerName: "manual-health-test",
      containerId: null,
      running: true,
      url: spec.runtime.url,
      health: {
        status: "unknown",
        url: null,
        checkedAt: null,
        statusCode: null,
        error: null,
      },
      startedAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    }, null, 2)}\n`, "utf8");

    const validation = runDeployValidate(projectRoot);
    assert.equal(validation.ok, true);
    assert.equal(validation.data.valid, false);
    assert.equal(validation.data.health.status, "unhealthy");
    assert.equal(validation.data.health.statusCode, 500);
    assert.match(validation.data.health.url, /\/app$/);
  } finally {
    await stopHealthServer(server);
  }
}

async function verifyHealthcheckOverrides(projectRoot) {
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      express: "^4.18.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");

  const envelope = runDeployPrepare(projectRoot, [
    "--healthcheck-path", "ready",
    "--healthcheck-candidate", "/ready",
    "--healthcheck-candidate", "healthz",
    "--healthcheck-attempts", "3",
    "--healthcheck-interval-ms", "250",
    "--healthcheck-timeout-ms", "750",
    "--healthcheck-expected-status-max", "399",
  ]);

  assert.equal(envelope.ok, true);

  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(spec.runtime.healthcheck.path, "/ready");
  assert.deepEqual(spec.runtime.healthcheck.candidates, ["/ready", "/healthz"]);
  assert.equal(spec.runtime.healthcheck.attempts, 3);
  assert.equal(spec.runtime.healthcheck.intervalMs, 250);
  assert.equal(spec.runtime.healthcheck.timeoutMs, 750);
  assert.equal(spec.runtime.healthcheck.expectedStatusMax, 399);
  assert.match(spec.runtime.healthcheck.url, /\/ready$/);

  const disabled = runDeployPrepare(projectRoot, ["--healthcheck-disabled"]);
  assert.equal(disabled.ok, true);
  const disabledSpec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  assert.equal(disabledSpec.runtime.healthcheck.enabled, false);
  assert.equal(disabledSpec.runtime.healthcheck.url, null);
}

async function verifyBootstrapCommandPreviewAndConfirm(projectRoot) {
  const binDir = join(projectRoot, "mock-bin");
  const callsPath = join(projectRoot, "docker-calls.log");
  await mkdir(join(projectRoot, "prisma"), { recursive: true });
  await mkdir(binDir, { recursive: true });
  await writePackage(projectRoot, {
    scripts: {
      start: "node server.js",
    },
    dependencies: {
      prisma: "^6.0.0",
    },
  });
  await writeFile(join(projectRoot, "server.js"), "console.log('ok')\n", "utf8");
  await writeFile(join(projectRoot, "prisma/schema.prisma"), "datasource db { provider = \"postgresql\" url = env(\"DATABASE_URL\") }\n", "utf8");
  await writeFile(join(projectRoot, "mock-bin/docker"), [
    "#!/bin/sh",
    `printf '%s\\n' \"$*\" >> ${JSON.stringify(callsPath)}`,
    "if [ \"$1\" = \"version\" ]; then echo '25.0.0'; exit 0; fi",
    "if [ \"$1\" = \"inspect\" ]; then echo 'container123 true'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && printf '%s\\n' \"$@\" | grep -qx 'ps'; then echo 'container123'; exit 0; fi",
    "if [ \"$1\" = \"compose\" ] && printf '%s\\n' \"$@\" | grep -qx 'exec'; then echo 'migrated'; exit 0; fi",
    "echo ok",
    "",
  ].join("\n"), "utf8");
  await chmod(join(projectRoot, "mock-bin/docker"), 0o755);

  const prepare = runDeployPrepare(projectRoot);
  assert.equal(prepare.ok, true);
  const spec = JSON.parse(await readFile(join(projectRoot, ".loom/deployment/specs/local.json"), "utf8"));
  await mkdir(join(projectRoot, ".loom/deployment/state"), { recursive: true });
  await writeFile(join(projectRoot, ".loom/deployment/state/local.json"), `${JSON.stringify({
    schemaVersion: 1,
    provider: spec.provider,
    serviceName: spec.serviceName,
    appServiceName: spec.compose.selectedService,
    imageName: spec.imageName,
    projectRoot,
    specPath: ".loom/deployment/specs/local.json",
    composePath: spec.files.composePath,
    containerName: `loom-${spec.serviceName}`,
    containerId: "container123",
    running: true,
    url: spec.runtime.url,
    health: {
      status: "healthy",
      url: spec.runtime.healthcheck.url,
      checkedAt: new Date().toISOString(),
      statusCode: 200,
      error: null,
    },
    startedAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  }, null, 2)}\n`, "utf8");

  const preview = runDeployBootstrap(projectRoot);
  assert.equal(preview.ok, true);
  assert.equal(preview.data.confirmed, false);
  assert.ok(preview.data.skipped.some((task) => task.kind === "prisma"));

  const confirmed = runDeployBootstrap(projectRoot, ["--kind", "prisma", "--confirm"], {
    PATH: `${binDir}:${process.env.PATH}`,
  });
  assert.equal(confirmed.ok, true);
  assert.equal(confirmed.data.confirmed, true);
  assert.equal(confirmed.data.executed[0].status, "completed");
  assert.match(confirmed.data.executed[0].stdoutTail.join("\n"), /migrated/);

  const calls = await readFile(callsPath, "utf8");
  assert.match(calls, /compose -f .* exec -T .* sh -lc npx prisma migrate deploy/);
}

async function writePackage(projectRoot, pkg) {
  await mkdir(projectRoot, { recursive: true });
  await writeFile(join(projectRoot, "package.json"), `${JSON.stringify(pkg, null, 2)}\n`, "utf8");
}

async function writeTechnicalBaseline(projectRoot, input = {}) {
  const deliveryId = "delivery-runtime";
  const now = new Date().toISOString();
  const contractsDir = join(projectRoot, ".loom/deliveries", deliveryId, "contracts");
  await mkdir(contractsDir, { recursive: true });
  await writeFile(join(contractsDir, "technical-baseline.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    technicalBaselineId: "tb-runtime",
    status: "confirmed",
    source: "user_specified",
    projectKind: "greenfield",
    scope: "roadmap",
    stack: {
      tracks: {
        web: track(input.web ?? "No Web client"),
        app: track(input.app ?? "No App client"),
        backend: track(input.backend ?? "No independent backend"),
        persistence: track(input.persistence ?? "No persistence yet"),
        dataAccess: track(input.dataAccess ?? "No ORM"),
        externalServices: track(input.externalServices ?? "None"),
      },
      derivedLater: ["testing", "build", "local run", "deployment preparation"],
    },
    constraints: [],
    evidence: [{ reason: "Deploy smoke test technical baseline fixture." }],
    approval: {
      type: "user_confirmed",
      confirmedAt: now,
      confirmedBy: "test",
    },
    confidence: "high",
    reasoningSummary: ["Deploy smoke test baseline."],
    alternatives: [],
    createdAt: now,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
}

function track(selection) {
  const normalized = String(selection).toLowerCase();
  const notNeeded = /no |none|不需要/.test(normalized);
  return {
    status: notNeeded ? "not_needed" : "selected",
    selection,
    source: "user_specified",
    rationale: "Deploy smoke test fixture.",
  };
}

async function writeAcceptedRuntimeDelivery(projectRoot, input) {
  const deliveryId = "delivery-runtime";
  const phaseId = "phase-1";
  const activePhaseId = input.activePhaseId ?? phaseId;
  const now = new Date().toISOString();
  await mkdir(join(projectRoot, ".loom/deliveries", deliveryId, "artifacts/architecture", phaseId), { recursive: true });
  await writeFile(join(projectRoot, ".loom/status.json"), `${JSON.stringify({
    schemaVersion: 1,
    activeDeliveryId: deliveryId,
    lastCompletedDeliveryId: null,
    deliveries: [{
      deliveryId,
      status: "planning",
      requestSummary: "Deploy runtime contract fixture.",
      activePhaseId,
      indexRef: `.loom/deliveries/${deliveryId}/index.json`,
      updatedAt: now,
    }],
    phase: "planning",
    current: { requirementId: null, planId: null, taskId: null, reviewId: null, repairId: null, deploymentId: null },
    lastAction: null,
    nextAction: "plan",
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
  await writeFile(join(projectRoot, ".loom/deliveries", deliveryId, "index.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    deliveryId,
    status: "planning",
    requestSummary: "Deploy runtime contract fixture.",
    roadmapId: null,
    activePhaseId,
    phases: [
      {
        phaseId,
        name: "Phase 1",
        status: input.phase1Status ?? "planning",
        latestRefs: {
          architectureArtifact: `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`,
        },
        nextAction: null,
      },
      ...(input.includePhase2 ? [{
        phaseId: "phase-2",
        name: "Phase 2",
        status: "pending",
        latestRefs: {},
        nextAction: null,
      }] : []),
    ],
    createdAt: now,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
  await writeFile(join(projectRoot, ".loom/deliveries", deliveryId, "artifacts/architecture", phaseId, "latest.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-runtime",
    artifactRef: `.loom/deliveries/${deliveryId}/artifacts/architecture/${phaseId}/aac.json`,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
  await writeFile(join(projectRoot, ".loom/deliveries", deliveryId, "artifacts/architecture", phaseId, "aac.json"), `${JSON.stringify({
    schemaVersion: "1.0",
    architectureArtifactContractId: "aac-runtime",
    status: "ready",
    source: {
      planningGenerationContractId: "pgc-runtime",
      technicalBaselineId: "tb-runtime",
      brainstormContractId: "bc-runtime",
      roadmapId: null,
      phaseId,
    },
    engineeringBoundary: {
      projectKind: "existing_project",
      strategy: "extend_existing_modules",
      applications: [{ appId: "app-main", type: "web_app", root: "." }],
      modules: [],
      creationPolicy: { createOnlyCurrentPhasePaths: true, avoidFuturePhaseScaffolding: true },
    },
    modules: [],
    dataModel: { entities: [], relationships: [], constraints: [] },
    interfaces: [],
    userFlows: [],
    stateMachines: [],
    runtimeDelivery: {
      status: "modified",
      contractVersion: "phase-1-v1",
      runtimeKind: input.runtimeKind ?? "node_express_serves_vite_static",
      basis: { technicalBaselineRef: "technical-baseline", repositoryContextRef: "repository-context", planningGenerationContractRef: "planning-contract", previousRuntimeDeliveryRef: null, reason: "Deploy smoke fixture runtime contract." },
      build: { command: input.buildCommand, workingDirectory: ".", outputs: ["dist/server", input.frontendOutputDir], codeLevelExpectations: ["Build produces server and frontend outputs."] },
      start: { command: input.startCommand, workingDirectory: ".", entry: "dist/server.js", host: "0.0.0.0", port: input.startPort, portEnv: "PORT", codeLevelExpectations: ["Start serves the app on the declared port."] },
      runtimeSurfaces: [{ surfaceId: "preview-root", kind: "http", probe: { type: "http_path", target: input.previewPath, expected: "2xx_or_3xx" } }],
      deliveryMechanics: {
        staticAssets: { required: true, source: "src/ui", output: input.frontendOutputDir, servedBy: "express_static" },
        api: { required: true, entry: "dist/server.js", basePath: "/api", probePaths: ["/api/health"] },
        codegen: { required: "no", commands: [], codeLevelExpectations: [] },
      },
      httpProbes: { previewPath: input.previewPath, healthPath: input.healthPath, apiPaths: ["/api/health"], expectedStatus: "2xx_or_3xx" },
      frontend: { required: true, kind: "vite_react", buildCommand: "npm run build:web", sourceRoot: "src/ui", outputDir: input.frontendOutputDir, servedBy: "express_static", servedByRef: "src/server.ts", codeLevelExpectations: ["Frontend output is mounted by the server."] },
      api: { required: true, kind: "express", buildCommand: "npm run build:api", entry: "dist/server.js", basePath: "/api", probePaths: ["/api/health"], codeLevelExpectations: ["Health API remains available."] },
      environment: input.environment ?? { required: [], optional: ["PORT"] },
      taskPlanningGuidance: {
        requireRuntimeDeliveryRequirementWhenTaskTouches: ["build_or_packaging", "runtime_entry", "serving_or_routing", "configuration_or_environment", "generated_artifacts", "runtime_surface"],
        doNotRequireForTaskKinds: ["domain_only_validation", "copy_only_documentation", "pure_unit_test_additions"],
        verificationBoundary: "code_level_only",
        doNotRequireCleanInstallOrContainerBuild: true,
      },
      deployability: { localDocker: "supported", notes: [] },
    },
    acceptanceMatrix: [],
    risksAndDecisions: { decisions: [], risks: [], assumptions: [], deferredNotes: [] },
    handoff: { readyForTaskPlan: true, blockingReasons: [], nextNode: "task_plan" },
    createdAt: now,
    updatedAt: now,
  }, null, 2)}\n`, "utf8");
}

function runDeployPrepare(projectRoot, extraArgs = [], expectedStatuses = [0]) {
  return runLoom(["deploy", "prepare", "--project-root", projectRoot, "--json", ...extraArgs], expectedStatuses);
}

function runDeployValidate(projectRoot) {
  return runLoom(["deploy", "validate", "--project-root", projectRoot, "--json"]);
}

function runDeployUp(projectRoot, env = {}) {
  return runLoom(["deploy", "up", "--project-root", projectRoot, "--json"], [0], env);
}

function runDeployRun(projectRoot, env = {}) {
  return runLoom(["deploy", "run", "--project-root", projectRoot, "--json"], [0, 2], env);
}

function runDeployRepair(projectRoot) {
  return runLoom(["deploy", "repair", "--project-root", projectRoot, "--json"]);
}

function runDeployStatus(projectRoot, env = {}) {
  return runLoom(["deploy", "status", "--project-root", projectRoot, "--json"], [0], env);
}

function runRepairRequest(projectRoot, extraArgs = []) {
  return runLoom(["repair", "request", "--project-root", projectRoot, "--json", ...extraArgs]);
}

function runRepairSubmit(projectRoot, extraArgs = []) {
  return runLoom(["repair", "submit", "--project-root", projectRoot, "--json", ...extraArgs]);
}

function runDeployBootstrap(projectRoot, extraArgs = [], env = {}) {
  return runLoom(["deploy", "bootstrap", "--project-root", projectRoot, "--json", ...extraArgs], [0], env);
}

function runDeployInspect(projectRoot, extraArgs = [], env = {}) {
  return runLoom(["deploy", "inspect", "--project-root", projectRoot, "--json", ...extraArgs], [0], env);
}

function assertDeployAssetRepairInstruction(envelope, expectedEditableFile) {
  assert.equal(envelope.instruction.mode, "deploy_repair_assets");
  assert.equal(envelope.instruction.autoContinue, true);
  assert.equal(envelope.instruction.mustRunImmediately, true);
  assert.equal(envelope.instruction.repairRoute, "deploy_repair");
  assert.ok(envelope.instruction.editableFiles.includes(expectedEditableFile));
  assert.ok(envelope.instruction.repairBoundary.allowedEdits.includes(expectedEditableFile));
  assert.ok(envelope.instruction.repairBoundary.forbiddenEdits.includes("application source files"));
  assert.deepEqual(envelope.instruction.retryCommand.argv, ["deploy", "up"]);
  assert.deepEqual(envelope.instruction.completionBarrier.followUpCommand.argv, ["deploy", "up"]);
  assert.equal(envelope.actionRequired.mode, "deploy_repair_assets");
  assert.equal(envelope.actionRequired.mustRunImmediately, true);
  assert.deepEqual(envelope.actionRequired.retryCommand.argv, ["deploy", "up"]);
  assert.ok(envelope.actionRequired.requiredSteps.some((step) => /edit only instruction\.editableFiles/i.test(step)));
  assert.ok(envelope.actionRequired.forbiddenStops.some((stop) => /do not ask whether to repair/i.test(stop)));
  assert.match(envelope.summary, /Deployment repair is auto-runnable/i);
}

function startHealthServer(port) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [
      "-e",
      [
        "const http = require('node:http');",
        `const server = http.createServer((req, res) => {`,
        "  if (req.url === '/health') { res.writeHead(200); res.end('ok'); return; }",
        "  res.writeHead(500); res.end('not ready');",
        "});",
        `server.listen(${port}, () => console.log('ready'));`,
      ].join("\n"),
    ], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        child.kill();
        reject(new Error("Health test server did not start."));
      }
    }, 5_000);
    child.stdout.on("data", (chunk) => {
      if (!settled && chunk.toString().includes("ready")) {
        settled = true;
        clearTimeout(timer);
        resolve(child);
      }
    });
    child.once("error", (error) => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        reject(error);
      }
    });
    child.once("exit", (code) => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        reject(new Error(`Health test server exited with ${code}.`));
      }
    });
  });
}

function startOkServer(port) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [
      "-e",
      [
        "const http = require('node:http');",
        "const server = http.createServer((req, res) => {",
        "  res.writeHead(200); res.end('ok');",
        "});",
        `server.listen(${port}, () => console.log('ready'));`,
      ].join("\n"),
    ], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true;
        child.kill();
        reject(new Error("Preview test server did not start."));
      }
    }, 5000);
    child.stdout.on("data", (chunk) => {
      if (!settled && chunk.toString().includes("ready")) {
        settled = true;
        clearTimeout(timer);
        resolve(child);
      }
    });
    child.on("error", (error) => {
      if (!settled) {
        settled = true;
        clearTimeout(timer);
        reject(error);
      }
    });
  });
}

async function fileExists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

function stopHealthServer(child) {
  return new Promise((resolve) => {
    child.once("exit", () => resolve());
    child.kill();
    setTimeout(resolve, 500);
  });
}

function runLoom(args, expectedStatuses = [0], env = {}) {
  const result = spawnSync(
    process.execPath,
    ["dist/cli.js", ...args],
    {
      cwd: join(__dirname, ".."),
      encoding: "utf8",
      env: { ...process.env, LOOM_AGENT_PROFILE: "codex", ...env },
    },
  );

  assert.ok(expectedStatuses.includes(result.status), result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

function assertSelectedCandidate(envelope, provider) {
  assert.equal(typeof envelope.data.providerReason, "string");
  assert.ok(envelope.data.providerReason.length > 0);

  const selected = envelope.data.providerCandidates.filter((candidate) => candidate.status === "selected");
  assert.equal(selected.length, 1);
  assert.equal(selected[0].provider, provider);
  assert.equal(envelope.data.providerCandidates.length, 3);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
