import type { DeploymentFailureDiagnostic, DeploymentSpec } from "./types";

type PatternDiagnostic = {
  code: string;
  severity: DeploymentFailureDiagnostic["severity"];
  pattern: RegExp;
  message: string;
  suggestedAction: string;
};

const logPatterns: PatternDiagnostic[] = [
  {
    code: "registry_network",
    severity: "error",
    pattern: /failed to fetch oauth token|failed to authorize|deadlineexceeded|i\/o timeout|tls handshake timeout|temporary failure in name resolution|lookup .* no such host|connection timed out|network is unreachable|registry-1\.docker\.io.*(timeout|unreachable|no such host|failed)|auth\.docker\.io.*(timeout|unreachable|no such host|failed)/i,
    message: "Docker could not reach or authenticate with the container registry while pulling image metadata or layers.",
    suggestedAction: "Fix Docker registry/network access, configure a registry mirror, pre-pull the base image, or retry when Docker Hub/registry access is available. Do not edit application code for this failure.",
  },
  {
    code: "node_missing_module",
    severity: "error",
    pattern: /cannot find module|module_not_found/i,
    message: "The app could not load a required module inside the container.",
    suggestedAction: "Check dependency installation, package manager lockfiles, optional native packages, and whether production install omitted a runtime dependency.",
  },
  {
    code: "native_optional_dependency",
    severity: "error",
    pattern: /(@next\/swc|@tailwindcss\/oxide|tailwindcss-oxide|lightningcss|sharp|esbuild|rollup).*?(linux|darwin|arm64|x64|gnu|musl)|cannot find module.*?(lightningcss|sharp|esbuild|rollup|swc|oxide)/i,
    message: "A platform-specific native optional dependency is missing or mismatched for the container image.",
    suggestedAction: "Use the correct Linux glibc/musl optional dependency for the selected base image or repair the lockfile/install step so the package is installed in the image.",
  },
  {
    code: "port_in_use",
    severity: "error",
    pattern: /eaddrinuse|address already in use|port is already allocated|bind: address already in use/i,
    message: "A configured port is already in use.",
    suggestedAction: "Change the generated host port or stop the conflicting local/container process before retrying.",
  },
  {
    code: "localhost_binding",
    severity: "warning",
    pattern: /127\.0\.0\.1|localhost/i,
    message: "Logs mention localhost binding; containers must usually bind the app server to 0.0.0.0.",
    suggestedAction: "Verify the start command binds to 0.0.0.0 and listens on the generated container port.",
  },
  {
    code: "database_connection_refused",
    severity: "error",
    pattern: /connection refused|econnrefused|could not connect to server|database .* refused|sqlstate\[hy000\] \[2002\]/i,
    message: "The app could not connect to a dependency service.",
    suggestedAction: "Check generated database/redis host names, ports, depends_on ordering, and whether the dependency needs more startup time.",
  },
  {
    code: "service_not_running",
    severity: "error",
    pattern: /service .* is not running|is not running after docker compose up|exited with code|no container found/i,
    message: "The selected Compose app service is not running.",
    suggestedAction: "Inspect the selected service logs and confirm loom selected the correct application service from the Compose file.",
  },
  {
    code: "database_auth_failed",
    severity: "error",
    pattern: /password authentication failed|access denied for user|authentication failed|role .* does not exist|invalid username-password/i,
    message: "A database or dependency service rejected credentials.",
    suggestedAction: "Align generated connection env with the dependency service env, or ask the user for required credentials without copying real local secrets.",
  },
  {
    code: "missing_database_table",
    severity: "error",
    pattern: /relation .* does not exist|table .* doesn't exist|no such table|undefined table|pendingmigrationerror|pending migrations|migration.*pending/i,
    message: "The app likely needs database migrations or schema bootstrap before it can serve requests.",
    suggestedAction: "Inspect bootstrap diagnostics and ask before running migration commands; do not run migrations automatically.",
  },
  {
    code: "framework_startup_failed",
    severity: "error",
    pattern: /application failed to start|beancreationexception|unsatisfieddependencyexception|applicationcontextexception|webserverexception|flywayexception|liquibaseexception|hibernateexception|schemamanagementexception|psqlexception|communications link failure|unable to obtain jdbc connection|django\.db\.utils\.|improperlyconfigured|active(record|model)::|illuminate\\database|sqlstate\[/i,
    message: "The application framework failed during startup.",
    suggestedAction: "Route this through execution repair; inspect application dependencies, migrations, ORM/database compatibility, runtime configuration, and startup code before editing generated Dockerfile/Compose.",
  },
  {
    code: "prisma_migration_needed",
    severity: "error",
    pattern: /prisma.*(p20\d{2}|migrate|does not exist|database)|the table .* does not exist/i,
    message: "Prisma reported a database/schema problem.",
    suggestedAction: "Use the Prisma bootstrap task as guidance and ask before running migrations against the local Compose database.",
  },
  {
    code: "missing_env",
    severity: "error",
    pattern: /missing .*env|required environment|environment variable .* (is not set|missing|required)|secret.*(missing|not set)|app_key|secret_key_base|database_url/i,
    message: "The app reported a missing or invalid environment variable.",
    suggestedAction: "Compare the log with DeploymentSpec.environment.missing and add safe local placeholders only when appropriate.",
  },
  {
    code: "permission_denied",
    severity: "error",
    pattern: /permission denied|eacces|operation not permitted/i,
    message: "The container hit a filesystem or executable permission problem.",
    suggestedAction: "Repair generated Dockerfile ownership, chmod executable scripts, or adjust writable runtime directories.",
  },
];

export function diagnoseDeploymentFailure(input: {
  spec: DeploymentSpec;
  stdout: string;
  stderr: string;
}): DeploymentFailureDiagnostic[] {
  const lines = splitLines(`${input.stdout}\n${input.stderr}`);
  const diagnostics = logPatterns
    .map((definition) => diagnosticForPattern(definition, lines))
    .filter((diagnostic): diagnostic is DeploymentFailureDiagnostic => Boolean(diagnostic));

  if (input.spec.environment.missing.length > 0 && hasAny(lines, /env|secret|config|credential|database_url/i)) {
    diagnostics.push({
      code: "spec_missing_env",
      severity: "warning",
      message: "DeploymentSpec already contains missing environment diagnostics.",
      evidence: input.spec.environment.missing.map((variable) => variable.name).slice(0, 12),
      suggestedAction: "Review environment.missing before changing Dockerfile or Compose commands.",
    });
  }

  if (input.spec.bootstrap.tasks.length > 0 && hasAny(lines, /migration|migrate|relation|table|schema|database/i)) {
    diagnostics.push({
      code: "bootstrap_task_relevant",
      severity: "warning",
      message: "Detected bootstrap tasks may be relevant to this failure.",
      evidence: input.spec.bootstrap.tasks.map((task) => `${task.kind}: ${task.command}`),
      suggestedAction: "Ask before running bootstrap/migration commands; use them as diagnosis first.",
    });
  }

  return dedupeDiagnostics(diagnostics);
}

export function diagnosticActions(diagnostics: DeploymentFailureDiagnostic[]): string[] {
  return diagnostics.map((diagnostic) => `${diagnostic.code}: ${diagnostic.suggestedAction}`);
}

function diagnosticForPattern(
  definition: PatternDiagnostic,
  lines: string[],
): DeploymentFailureDiagnostic | null {
  const evidence = lines.filter((line) => definition.pattern.test(line)).slice(-5);
  if (evidence.length === 0) {
    return null;
  }

  return {
    code: definition.code,
    severity: definition.severity,
    message: definition.message,
    evidence,
    suggestedAction: definition.suggestedAction,
  };
}

function splitLines(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0)
    .slice(-160);
}

function hasAny(lines: string[], pattern: RegExp): boolean {
  return lines.some((line) => pattern.test(line));
}

function dedupeDiagnostics(diagnostics: DeploymentFailureDiagnostic[]): DeploymentFailureDiagnostic[] {
  const seen = new Set<string>();
  const result: DeploymentFailureDiagnostic[] = [];
  for (const diagnostic of diagnostics) {
    if (seen.has(diagnostic.code)) {
      continue;
    }
    seen.add(diagnostic.code);
    result.push(diagnostic);
  }
  return result;
}
