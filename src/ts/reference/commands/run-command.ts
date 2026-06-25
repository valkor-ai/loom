import path from "node:path";
import { exitCodeForErrorCode, invalidArgument } from "../core/errors";
import {
  agentProfileFromEnv,
  commandRequiresAgentProfile,
  missingAgentProfileError,
  withAgentProfile,
} from "./agent-profile";
import { toFailureEnvelope } from "./envelope";
import { printEnvelope } from "./output";
import { diagnosticProjectRoot, safeCwd } from "./safe-cwd";
import type { CommandContext, CommandHandler } from "./types";

type CommonOptions = {
  projectRoot?: string;
  appPath?: string;
  healthcheckPath?: string;
  healthcheckCandidate?: string[];
  healthcheckDisabled?: boolean;
  healthcheckAttempts?: string;
  healthcheckIntervalMs?: string;
  healthcheckTimeoutMs?: string;
  healthcheckExpectedStatusMax?: string;
  provider?: string;
  forceGenerate?: boolean;
  reuseExisting?: string;
  kind?: string;
  confirm?: boolean;
  refresh?: boolean;
  compact?: boolean;
  json?: boolean;
};

export async function runCommand<T>(
  command: string,
  options: CommonOptions,
  handler: CommandHandler<T>,
): Promise<void> {
  const normalizedOptions = normalizeOptions(options);
  const argv = process.argv.slice(2);
  const explicitProjectRoot = projectRootFromArgv();
  const cwd = safeCwd();
  let projectRoot: string;
  try {
    projectRoot = resolveProjectRoot(explicitProjectRoot ?? normalizedOptions.projectRoot, cwd);
  } catch (error) {
    const failure = toFailureEnvelope(command, diagnosticProjectRoot(cwd), error, { argv });
    printEnvelope(failure, { compact: shouldUseCompactOutput(normalizedOptions) });
    process.exitCode = exitCodeForErrorCode(failure.error.code);
    return;
  }
  let agentProfile;
  try {
    agentProfile = agentProfileFromEnv(process.env);
  } catch (error) {
    const failure = toFailureEnvelope(command, projectRoot, error, { argv });
    printEnvelope(failure, { compact: shouldUseCompactOutput(normalizedOptions) });
    process.exitCode = exitCodeForErrorCode(failure.error.code);
    return;
  }

  if (commandRequiresAgentProfile(command) && agentProfile === undefined) {
    const failure = toFailureEnvelope(command, projectRoot, missingAgentProfileError(command), { argv });
    printEnvelope(failure, { compact: shouldUseCompactOutput(normalizedOptions) });
    process.exitCode = exitCodeForErrorCode(failure.error.code);
    return;
  }

  const ctx: CommandContext = {
    projectRoot,
    appPath: normalizedOptions.appPath,
    healthcheck: healthcheckFromOptions(normalizedOptions),
    providerPolicy: providerPolicyFromOptions(normalizedOptions),
    bootstrapConfirm: normalizedOptions.confirm,
    bootstrapKind: normalizedOptions.kind as CommandContext["bootstrapKind"],
    refresh: normalizedOptions.refresh,
    cwd: cwd ?? projectRoot,
    json: normalizedOptions.json ?? true,
    compactOutput: shouldUseCompactOutput(normalizedOptions),
    agentProfile: agentProfile?.id,
  };

  try {
    const result = withAgentProfile(await handler(ctx), agentProfile);
    printEnvelope(result, { compact: ctx.compactOutput });
    process.exitCode = result.ok ? 0 : exitCodeForErrorCode(result.error.code);
  } catch (error) {
    const failure = toFailureEnvelope(command, ctx.projectRoot, error, { argv });
    printEnvelope(failure, { compact: ctx.compactOutput });
    process.exitCode = exitCodeForErrorCode(failure.error.code);
  }
}

function resolveProjectRoot(projectRootOption: string | undefined, cwd: string | null): string {
  const rawProjectRoot = projectRootOption?.trim();
  if (rawProjectRoot) {
    if (path.isAbsolute(rawProjectRoot)) {
      return path.normalize(rawProjectRoot);
    }
    if (cwd) {
      return path.resolve(cwd, rawProjectRoot);
    }
    throw invalidArgument("Cannot resolve relative --project-root because the current working directory is unavailable. Pass an absolute --project-root.", {
      projectRoot: projectRootOption,
    });
  }

  if (cwd) {
    return cwd;
  }
  throw invalidArgument("Cannot determine project root because the current working directory is unavailable. Pass --project-root /abs/project.", {
    option: "--project-root",
  });
}

function healthcheckFromOptions(options: CommonOptions): CommandContext["healthcheck"] {
  const healthcheck: NonNullable<CommandContext["healthcheck"]> = {};

  if (typeof options.healthcheckPath === "string") {
    healthcheck.path = options.healthcheckPath;
  }
  if (Array.isArray(options.healthcheckCandidate) && options.healthcheckCandidate.length > 0) {
    healthcheck.candidates = options.healthcheckCandidate;
  }
  if (typeof options.healthcheckDisabled === "boolean" && options.healthcheckDisabled) {
    healthcheck.enabled = false;
  }
  const attempts = numberOption(options.healthcheckAttempts);
  if (attempts !== undefined) {
    healthcheck.attempts = attempts;
  }
  const intervalMs = numberOption(options.healthcheckIntervalMs);
  if (intervalMs !== undefined) {
    healthcheck.intervalMs = intervalMs;
  }
  const timeoutMs = numberOption(options.healthcheckTimeoutMs);
  if (timeoutMs !== undefined) {
    healthcheck.timeoutMs = timeoutMs;
  }
  const expectedStatusMax = numberOption(options.healthcheckExpectedStatusMax);
  if (expectedStatusMax !== undefined) {
    healthcheck.expectedStatusMax = expectedStatusMax;
  }

  return Object.keys(healthcheck).length > 0 ? healthcheck : undefined;
}

function providerPolicyFromOptions(options: CommonOptions): CommandContext["providerPolicy"] {
  const policy: NonNullable<CommandContext["providerPolicy"]> = {};

  if (typeof options.provider === "string") {
    policy.provider = options.provider as NonNullable<CommandContext["providerPolicy"]>["provider"];
  }
  if (typeof options.forceGenerate === "boolean" && options.forceGenerate) {
    policy.forceGenerate = true;
  }
  if (typeof options.reuseExisting === "string") {
    policy.reuseExisting = options.reuseExisting !== "false";
  }

  return Object.keys(policy).length > 0 ? policy : undefined;
}

function numberOption(value: string | undefined): number | undefined {
  if (value === undefined) {
    return undefined;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function projectRootFromArgv(): string | undefined {
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--project-root") {
      return args[index + 1];
    }
    if (arg.startsWith("--project-root=")) {
      return arg.slice("--project-root=".length);
    }
  }
  return undefined;
}

function shouldUseCompactOutput(options: CommonOptions): boolean {
  if (process.env.LOOM_COMPACT_OUTPUT === "1") {
    return true;
  }
  if (options.compact === true) {
    return true;
  }
  return !argvIncludes("--json");
}

function argvIncludes(name: string): boolean {
  return process.argv.slice(2).some((arg) => arg === name || arg.startsWith(`${name}=`));
}

function normalizeOptions(options: CommonOptions): CommonOptions {
  const commandLike = options as CommonOptions & {
    parent?: { opts?: () => CommonOptions; optsWithGlobals?: () => CommonOptions };
    opts?: () => CommonOptions;
    optsWithGlobals?: () => CommonOptions;
  };
  if (typeof commandLike.opts === "function") {
    return {
      ...(typeof commandLike.parent?.optsWithGlobals === "function" ? commandLike.parent.optsWithGlobals() : {}),
      ...(typeof commandLike.parent?.opts === "function" ? commandLike.parent.opts() : {}),
      ...(typeof commandLike.optsWithGlobals === "function" ? commandLike.optsWithGlobals() : {}),
      ...commandLike.opts(),
    };
  }
  return options;
}
