import {
  deployBootstrap,
  deployDown,
  deployInspect,
  deployLogs,
  deployPrepare,
  deployRepair,
  deployRun,
  deployStatus,
  deployUp,
  deployValidate,
} from "../core/deployment/operations";
import { initProject } from "../core/operations/init-project";
import { ok } from "./envelope";
import type { CliEnvelope, CommandContext } from "./types";

export async function handleDeployPrepare(ctx: CommandContext): Promise<CliEnvelope> {
  await initProject({ projectRoot: ctx.projectRoot });
  const result = await deployPrepare({
    projectRoot: ctx.projectRoot,
    appPath: ctx.appPath,
    healthcheck: ctx.healthcheck,
    providerPolicy: ctx.providerPolicy,
  });
  return ok("deploy.prepare", ctx.projectRoot, result, "Deployment is prepared.");
}

export async function handleDeployRun(ctx: CommandContext): Promise<CliEnvelope> {
  await initProject({ projectRoot: ctx.projectRoot });
  const result = await deployRun({
    projectRoot: ctx.projectRoot,
    appPath: ctx.appPath,
    healthcheck: ctx.healthcheck,
    providerPolicy: ctx.providerPolicy,
  });
  return ok(
    "deploy.run",
    ctx.projectRoot,
    result,
    result.completed
      ? `Deployment is running at ${result.status?.url ?? result.up?.url}.`
      : `Deployment run stopped at ${result.failedPhase ?? "unknown"}; next action is ${result.nextAction}.`,
  );
}

export async function handleDeployUp(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployUp({
    projectRoot: ctx.projectRoot,
    appPath: ctx.appPath,
    healthcheck: ctx.healthcheck,
    providerPolicy: ctx.providerPolicy,
  });
  return ok(
    "deploy.up",
    ctx.projectRoot,
    result,
    result.started ? `Deployment is running at ${result.url}.` : "Deployment command completed, but the container is not running.",
  );
}

export async function handleDeployBootstrap(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployBootstrap({
    projectRoot: ctx.projectRoot,
    confirm: ctx.bootstrapConfirm,
    kind: ctx.bootstrapKind,
  });
  return ok(
    "deploy.bootstrap",
    ctx.projectRoot,
    result,
    result.confirmed
      ? `Deployment bootstrap executed ${result.executed.length} command(s).`
      : `Deployment bootstrap has ${result.skipped.length || result.tasks.length} pending command(s); rerun with --confirm to execute.`,
  );
}

export async function handleDeployStatus(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployStatus({ projectRoot: ctx.projectRoot });
  return ok(
    "deploy.status",
    ctx.projectRoot,
    result,
    result.operationActive && result.activeOperation
      ? `Deployment operation is active: ${result.activeOperation.command} is ${result.activeOperation.phase}.`
      : result.running
      ? result.health && result.health.status !== "healthy" && result.health.status !== "disabled"
        ? `Deployment container is running at ${result.url}, but preview verification is ${result.health.status}.`
        : `Deployment is running at ${result.url}.`
      : "No loom deployment is running.",
  );
}

export async function handleDeployInspect(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployInspect({ projectRoot: ctx.projectRoot, refresh: ctx.refresh });
  return ok(
    "deploy.inspect",
    ctx.projectRoot,
    result,
    result.operationActive && result.activeOperation
      ? `Deployment operation is active: ${result.activeOperation.command} is ${result.activeOperation.phase}.`
      : result.prepared
      ? `Deployment inspect ready for provider ${result.provider}.`
      : "Deployment is not prepared.",
  );
}

export async function handleDeployValidate(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployValidate({ projectRoot: ctx.projectRoot });
  return ok(
    "deploy.validate",
    ctx.projectRoot,
    result,
    result.valid ? "Deployment validation passed." : "Deployment validation failed.",
  );
}

export async function handleDeployLogs(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployLogs({ projectRoot: ctx.projectRoot });
  return ok(
    "deploy.logs",
    ctx.projectRoot,
    result,
    result.operationActive && result.activeOperation
      ? `Deployment operation is active: ${result.activeOperation.command} is ${result.activeOperation.phase}; returning Loom deploy log tail.`
      : "Deployment logs are available.",
  );
}

export async function handleDeployDown(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployDown({ projectRoot: ctx.projectRoot });
  return ok("deploy.down", ctx.projectRoot, result, "Deployment is stopped.");
}

export async function handleDeployRepair(ctx: CommandContext): Promise<CliEnvelope> {
  const result = await deployRepair({ projectRoot: ctx.projectRoot });
  return ok(
    "deploy.repair",
    ctx.projectRoot,
    result,
    result.hasRepairRequest
      ? "Deployment repair request is ready."
      : "No deployment repair request is available.",
  );
}
