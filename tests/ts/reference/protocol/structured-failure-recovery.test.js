#!/usr/bin/env node
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const path = require("node:path");
const { z } = require("zod");

const repoRoot = path.resolve(__dirname, "../../../../src/ts/reference");

execFileSync("npm", ["run", "build"], { cwd: repoRoot, stdio: "inherit" });

const { toFailureEnvelope } = require("../../../../src/ts/reference/dist/commands/envelope");
const { invalidArgument } = require("../../../../src/ts/reference/dist/core/errors");

const projectRoot = "/tmp/loom-structured-failure";
const argv = ["architecture", "accept", "--request-id", "arch-req-test"];

function assertRecovery(envelope, label) {
  const recovery = envelope.error.details?.failureRecovery;
  assert.ok(recovery, `${label}: failureRecovery is required.`);
  assert.equal(recovery.status, "structured_failure_recovery", `${label}: recovery status must be structured.`);
  assert.equal(recovery.command, "architecture.accept", `${label}: recovery must include command.`);
  assert.equal(recovery.projectRoot, projectRoot, `${label}: recovery must include projectRoot.`);
  assert.deepEqual(recovery.originalArgv, argv, `${label}: recovery must preserve original argv.`);
  assert.equal(recovery.mode, "run_cli_recovery_sequence", `${label}: recovery must be an executable recovery sequence.`);
  assert.deepEqual(
    recovery.requiredSteps.map((step) => step.step),
    ["read_failure_context", "run_status", "follow_status_or_continue", "repair_targeted_artifact"],
    `${label}: recovery must provide fixed required steps.`,
  );
  assert.deepEqual(
    recovery.commandInvocations.status.argv,
    ["status", "--project-root", projectRoot],
    `${label}: recovery must provide an executable status command.`,
  );
  assert.deepEqual(
    recovery.commandInvocations.continue.argv,
    ["continue", "--project-root", projectRoot],
    `${label}: recovery must provide an executable continue command.`,
  );
  assert.ok(
    recovery.requiredSteps.some((step) => step.instruction.includes("requestReadPlan.groups")),
    `${label}: recovery must route request reads through requestReadPlan when a requestRef exists.`,
  );
  assert.ok(
    recovery.fallbackWhenStatusFails.allowedReadClasses.some((entry) =>
      entry.class === "explicit_command_argument" &&
      entry.rule.includes("candidateFile") &&
      entry.rule.includes("requestRef")
    ),
    `${label}: fallback must allow only targeted explicit file args.`,
  );
  assert.equal(recovery.fallbackWhenStatusFails.mode, "bounded_allowlist_only", `${label}: fallback must be allowlist-only.`);
  assert.ok(recovery.fallbackWhenStatusFails.denyByDefaultRule.includes("outside the recovery input boundary"), `${label}: fallback must deny paths outside the allowlist by default.`);
  assert.equal("disallowedReads" in recovery.fallbackWhenStatusFails, false, `${label}: fallback must not encode scenario-specific denied directories.`);
  assert.equal(recovery.retryPolicy.sameCommandUnchangedMaxAttempts, 1, `${label}: retry policy must be bounded.`);
}

{
  let zodError = null;
  try {
    z.object({ status: z.enum(["ready", "blocked"]) }).parse({ status: "ready | blocked" });
  } catch (error) {
    zodError = error;
  }
  const envelope = toFailureEnvelope("architecture.accept", projectRoot, zodError, { argv });
  assert.equal(envelope.ok, false);
  assert.equal(envelope.error.code, "INTERNAL_ERROR");
  assert.equal(envelope.error.details.errorKind, "zod_error");
  assert.deepEqual(envelope.error.details.issues[0].allowedValues, ["ready", "blocked"]);
  assertRecovery(envelope, "zod error");
}

{
  const envelope = toFailureEnvelope("architecture.accept", projectRoot, new SyntaxError("Unexpected token } in JSON"), { argv });
  assert.equal(envelope.ok, false);
  assert.equal(envelope.error.code, "INTERNAL_ERROR");
  assert.equal(envelope.error.details.errorKind, "syntax_error");
  assert.equal(envelope.error.details.message, "Unexpected token } in JSON");
  assertRecovery(envelope, "syntax error");
}

{
  const envelope = toFailureEnvelope("architecture.accept", projectRoot, new Error("fixture exploded"), { argv });
  assert.equal(envelope.ok, false);
  assert.equal(envelope.error.code, "INTERNAL_ERROR");
  assert.equal(envelope.error.details.errorKind, "uncaught_error");
  assert.equal(envelope.error.details.message, "fixture exploded");
  assertRecovery(envelope, "plain error");
}

{
  const envelope = toFailureEnvelope(
    "architecture.accept",
    projectRoot,
    invalidArgument("fixture invalid argument", { field: "candidateFile" }),
    { argv },
  );
  assert.equal(envelope.ok, false);
  assert.equal(envelope.error.code, "INVALID_ARGUMENT");
  assert.equal(envelope.error.details.field, "candidateFile");
  assertRecovery(envelope, "invalid argument");
}

console.log("structured failure recovery verification passed");
