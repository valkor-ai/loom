import { ZodError } from "zod";

export type LoomErrorCode =
  | "INVALID_ARGUMENT"
  | "AGENT_PROFILE_REQUIRED"
  | "INVALID_AGENT_PROFILE"
  | "STATE_NOT_INITIALIZED"
  | "STATE_CORRUPTED"
  | "UNSUPPORTED_SCHEMA_VERSION"
  | "BRAINSTORM_NOT_FOUND"
  | "BRAINSTORM_NOT_READY"
  | "NO_ACTIVE_PLAN"
  | "NO_NEXT_TASK"
  | "TASK_NOT_FOUND"
  | "EVIDENCE_MISSING"
  | "REVIEW_BLOCKED"
  | "DEPLOY_NOT_PREPARED"
  | "DEPLOY_NOT_RUNNING"
  | "DOCKER_UNAVAILABLE"
  | "DEPLOY_SOURCE_INSUFFICIENT"
  | "DEPLOY_CONFLICT"
  | "DEPLOY_OPERATION_ACTIVE"
  | "DEPLOY_VALIDATION_FAILED"
  | "INTERNAL_ERROR";

export class LoomError extends Error {
  readonly code: LoomErrorCode;
  readonly details?: unknown;
  readonly exitCode: number;

  constructor(input: {
    code: LoomErrorCode;
    message: string;
    details?: unknown;
    exitCode?: number;
  }) {
    super(input.message);
    this.name = "LoomError";
    this.code = input.code;
    this.details = input.details;
    this.exitCode = input.exitCode ?? exitCodeForErrorCode(input.code);
  }
}

export function exitCodeForErrorCode(code: LoomErrorCode): number {
  switch (code) {
    case "INVALID_ARGUMENT":
    case "AGENT_PROFILE_REQUIRED":
    case "INVALID_AGENT_PROFILE":
      return 64;
    case "INTERNAL_ERROR":
      return 1;
    default:
      return 2;
  }
}

export function invalidArgument(message: string, details?: unknown): LoomError {
  return new LoomError({ code: "INVALID_ARGUMENT", message, details });
}

export function stateNotInitialized(projectRoot: string): LoomError {
  return new LoomError({
    code: "STATE_NOT_INITIALIZED",
    message: "loom state is not initialized. Run loom init first.",
    details: { projectRoot },
  });
}

export function noActivePlan(projectRoot: string): LoomError {
  return new LoomError({
    code: "NO_ACTIVE_PLAN",
    message: "No active loom plan.",
    details: { projectRoot },
  });
}

export function deployNotPrepared(projectRoot: string): LoomError {
  return new LoomError({
    code: "DEPLOY_NOT_PREPARED",
    message: "Deployment is not prepared. Run loom deploy prepare first.",
    details: { projectRoot },
  });
}

export function deployNotRunning(projectRoot: string): LoomError {
  return new LoomError({
    code: "DEPLOY_NOT_RUNNING",
    message: "Deployment is not running.",
    details: { projectRoot },
  });
}

export function dockerUnavailable(message = "Docker is unavailable.", details?: unknown): LoomError {
  return new LoomError({
    code: "DOCKER_UNAVAILABLE",
    message,
    details,
  });
}

export function deployValidationFailed(message: string, details?: unknown): LoomError {
  return new LoomError({
    code: "DEPLOY_VALIDATION_FAILED",
    message,
    details,
  });
}

export function deploySourceInsufficient(message: string, details?: unknown): LoomError {
  return new LoomError({
    code: "DEPLOY_SOURCE_INSUFFICIENT",
    message,
    details,
  });
}

export function deployConflict(message: string, details?: unknown): LoomError {
  return new LoomError({
    code: "DEPLOY_CONFLICT",
    message,
    details,
  });
}

export function deployOperationActive(message: string, details?: unknown): LoomError {
  return new LoomError({
    code: "DEPLOY_OPERATION_ACTIVE",
    message,
    details,
  });
}

export function stateCorrupted(message: string, details?: unknown): LoomError {
  return new LoomError({ code: "STATE_CORRUPTED", message, details });
}

export function unsupportedSchemaVersion(details: {
  file: string;
  found: unknown;
  supported: readonly number[];
}): LoomError {
  return new LoomError({
    code: "UNSUPPORTED_SCHEMA_VERSION",
    message: "Unsupported loom schema version.",
    details,
  });
}

export type FailureRecoveryContext = {
  command?: string;
  projectRoot?: string;
  argv?: string[];
};

export function internalError(error: unknown, context: FailureRecoveryContext = {}): LoomError {
  if (error instanceof LoomError) {
    return error;
  }

  if (error instanceof ZodError) {
    return new LoomError({
      code: "INTERNAL_ERROR",
      message: "Unexpected internal schema error.",
      details: {
        errorKind: "zod_error",
        name: error.name,
        issues: error.issues.map((issue) => compactZodIssue(issue)),
        failureRecovery: genericFailureRecovery(context, "zod_error"),
      },
    });
  }

  const details = error instanceof Error
    ? {
      errorKind: error instanceof SyntaxError ? "syntax_error" : "uncaught_error",
      name: error.name,
      message: error.message,
      failureRecovery: genericFailureRecovery(context, error instanceof SyntaxError ? "syntax_error" : "uncaught_error"),
    }
    : {
      errorKind: "unknown_throwable",
      type: typeof error,
      failureRecovery: genericFailureRecovery(context, "unknown_throwable"),
    };

  return new LoomError({
    code: "INTERNAL_ERROR",
    message: "Unexpected internal error.",
    details,
  });
}

export function withFailureRecovery(error: LoomError, context: FailureRecoveryContext = {}): LoomError {
  if (!shouldAttachFailureRecovery(error)) {
    return error;
  }
  const details = isRecord(error.details) ? error.details : {};
  if (isRecord(details.failureRecovery)) {
    return error;
  }
  return new LoomError({
    code: error.code,
    message: error.message,
    exitCode: error.exitCode,
    details: {
      ...details,
      failureRecovery: genericFailureRecovery(context, error.code.toLowerCase()),
    },
  });
}

function shouldAttachFailureRecovery(error: LoomError): boolean {
  return error.code === "INTERNAL_ERROR" ||
    error.code === "INVALID_ARGUMENT" ||
    error.code === "STATE_CORRUPTED" ||
    error.code === "UNSUPPORTED_SCHEMA_VERSION";
}

function genericFailureRecovery(context: FailureRecoveryContext, failureKind: string): Record<string, unknown> {
  const projectRootArgs = context.projectRoot ? ["--project-root", context.projectRoot] : [];
  return {
    status: "structured_failure_recovery",
    failureKind,
    ...(context.command ? { command: context.command } : {}),
    ...(context.projectRoot ? { projectRoot: context.projectRoot } : {}),
    ...(context.argv && context.argv.length > 0 ? { originalArgv: context.argv } : {}),
    mode: "run_cli_recovery_sequence",
    requiredSteps: [
      {
        step: "read_failure_context",
        instruction: "Use this failureRecovery object as the recovery authority for the failed loom command.",
        completeWhen: "command, projectRoot, originalArgv, and failureKind have been read from this object.",
      },
      {
        step: "run_status",
        instruction: "Run commandInvocations.status exactly once to get the current loom route state.",
        commandRef: "commandInvocations.status",
        completeWhen: "status envelope is read.",
      },
      {
        step: "follow_status_or_continue",
        instruction: "If status returns an instruction/actionRequired, follow that instruction. If status shows an active or recoverable operation without a terminal instruction, run commandInvocations.continue exactly once.",
        commandRef: "commandInvocations.continue",
        completeWhen: "the returned auto-runnable instruction is followed, or status proves there is no active recoverable operation.",
      },
      {
        step: "repair_targeted_artifact",
        instruction: "When the failed command or recovered instruction names a candidateFile, resultFile, inputFile, requestRef, or requestId, repair that exact artifact/request path. Use requestReadPlan groups first when a requestRef is present; if absent, use the request's agentAction.read.fieldGroups inspect commands as compatibility fallback.",
        completeWhen: "the exact target artifact is repaired and its submit command succeeds, or the CLI returns a non-repairable/user-gated result.",
      },
    ],
    commandInvocations: {
      status: {
        name: "status",
        argv: ["status", ...projectRootArgs],
        projectRootRequired: true,
        preserveEnv: ["LOOM_AGENT_PROFILE", "LOOM_COMPACT_OUTPUT"],
      },
      continue: {
        name: "continue",
        argv: ["continue", ...projectRootArgs],
        projectRootRequired: true,
        preserveEnv: ["LOOM_AGENT_PROFILE", "LOOM_COMPACT_OUTPUT"],
      },
    },
    stopCondition: "Stop only when status/continue reports a terminal user-gated state, completed delivery, or no active recoverable operation; otherwise follow the returned auto-runnable instruction.",
    fallbackWhenStatusFails: {
      mode: "bounded_allowlist_only",
      instruction: "If commandInvocations.status itself fails, recover only from explicit command inputs, current project loom state, and refs returned by loom CLI. Do not discover recovery inputs by directory enumeration.",
      allowedReadClasses: [
        {
          class: "explicit_command_argument",
          rule: "Read only the exact candidateFile, resultFile, inputFile, requestRef, requestId-derived request, or failureRef named by originalArgv.",
        },
        {
          class: "current_project_state",
          rule: "Read only loom state files under projectRoot that are necessary to identify the active delivery, active operation, or current request.",
        },
        {
          class: "cli_returned_ref",
          rule: "Read only refs returned by status, continue, inspect, repair, deploy, or another loom CLI envelope in the current recovery chain.",
        },
      ],
      denyByDefaultRule: "Any path or directory not matched by allowedReadClasses is outside the recovery input boundary, regardless of its name or location.",
    },
    retryPolicy: {
      sameCommandUnchangedMaxAttempts: 1,
      afterOneFailure: "Use commandInvocations.status/continue or repair the exact target artifact before retrying.",
    },
  };
}

function compactZodIssue(issue: ZodError["issues"][number]): Record<string, unknown> {
  const rawIssue = issue as unknown as Record<string, unknown>;
  const allowedValues = Array.isArray(rawIssue.options)
    ? rawIssue.options
    : Array.isArray(rawIssue.values)
      ? rawIssue.values
      : [];
  return {
    path: issue.path.map(String).join("."),
    message: issue.message,
    code: String(rawIssue.code ?? "unknown"),
    ...(allowedValues.length > 0 ? { allowedValues: allowedValues.map(String) } : {}),
    ...(rawIssue.expected !== undefined ? { expected: String(rawIssue.expected) } : {}),
    ...(rawIssue.received !== undefined ? { received: String(rawIssue.received) } : {}),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function summaryForError(code: LoomErrorCode): string {
  switch (code) {
    case "INVALID_ARGUMENT":
      return "Invalid argument.";
    case "AGENT_PROFILE_REQUIRED":
      return "Agent profile is required.";
    case "INVALID_AGENT_PROFILE":
      return "Agent profile is invalid.";
    case "STATE_NOT_INITIALIZED":
      return "loom state is not initialized.";
    case "STATE_CORRUPTED":
      return "loom state is corrupted.";
    case "UNSUPPORTED_SCHEMA_VERSION":
      return "Unsupported loom schema version.";
    case "BRAINSTORM_NOT_FOUND":
      return "Brainstorm run was not found.";
    case "BRAINSTORM_NOT_READY":
      return "Brainstorm run is not ready for this operation.";
    case "NO_ACTIVE_PLAN":
      return "No active loom plan.";
    case "NO_NEXT_TASK":
      return "No next loom task.";
    case "TASK_NOT_FOUND":
      return "Task was not found.";
    case "EVIDENCE_MISSING":
      return "Required evidence is missing.";
    case "REVIEW_BLOCKED":
      return "Review blocked delivery.";
    case "DEPLOY_NOT_PREPARED":
      return "Deployment is not prepared.";
    case "DEPLOY_NOT_RUNNING":
      return "Deployment is not running.";
    case "DOCKER_UNAVAILABLE":
      return "Docker is unavailable.";
    case "DEPLOY_SOURCE_INSUFFICIENT":
      return "Deployment source is insufficient.";
    case "DEPLOY_CONFLICT":
      return "Deployment source has conflicts.";
    case "DEPLOY_OPERATION_ACTIVE":
      return "Deployment operation is already active.";
    case "DEPLOY_VALIDATION_FAILED":
      return "Deployment validation failed.";
    case "INTERNAL_ERROR":
      return "Unexpected internal error.";
  }
}

export function brainstormNotFound(projectRoot: string, brainstormRunId: string): LoomError {
  return new LoomError({
    code: "BRAINSTORM_NOT_FOUND",
    message: "Brainstorm run was not found.",
    details: { projectRoot, brainstormRunId },
  });
}

export function brainstormNotReady(message: string, details?: unknown): LoomError {
  return new LoomError({ code: "BRAINSTORM_NOT_READY", message, details });
}
