import type { LoomErrorCode } from "../core/errors";
import type {
  DeployProvider,
  DeploymentBootstrapKind,
  DeploymentHealthcheckInput,
} from "../core/deployment/types";
import type { AgentProfileDescriptor, AgentProfileId } from "./agent-profile";

export type CommandContext = {
  projectRoot: string;
  appPath?: string;
  healthcheck?: DeploymentHealthcheckInput;
  providerPolicy?: {
    provider?: DeployProvider;
    reuseExisting?: boolean;
    forceGenerate?: boolean;
  };
  bootstrapConfirm?: boolean;
  bootstrapKind?: DeploymentBootstrapKind;
  refresh?: boolean;
  cwd: string;
  json: boolean;
  compactOutput: boolean;
  agentProfile?: AgentProfileId;
};

export type CliSuccessEnvelope<T = unknown> = {
  ok: true;
  command: string;
  version: string;
  projectRoot: string;
  agentProfile?: AgentProfileDescriptor;
  actionRequired?: {
    mode: string;
    autoContinue: boolean;
    mustRunImmediately: boolean;
    mustNotReportProgress: boolean;
    mustNotAskBeforeExecuting: boolean;
    requestRef?: string;
    resultFile?: string;
    candidateFile?: string;
    targetCandidateFile?: string;
    submitCommand?: Record<string, unknown>;
    command?: Record<string, unknown>;
    retryCommand?: Record<string, unknown>;
    commandInvocation?: Record<string, unknown>;
    completionBarrier?: Record<string, unknown>;
    finalResponseGuard?: Record<string, unknown>;
    primaryAction?: string;
    fieldRepairPlan?: unknown[];
    requestReadProtocol?: Record<string, unknown>;
    requiredSteps?: unknown[];
    forbiddenStops?: unknown[];
    stopOnlyWhen?: unknown[];
    summary: string;
  };
  instruction?: Record<string, unknown>;
  data: T;
  summary: string;
};

export type CliFailureEnvelope = {
  ok: false;
  command: string;
  version: string;
  projectRoot: string;
  error: {
    code: LoomErrorCode;
    message: string;
    details?: unknown;
  };
  summary: string;
};

export type CliEnvelope<T = unknown> = CliSuccessEnvelope<T> | CliFailureEnvelope;

export type CommandHandler<T = unknown> = (
  ctx: CommandContext,
) => Promise<CliEnvelope<T>> | CliEnvelope<T>;
