import { LoomError } from "../core/errors";
import { withAgentCommandInvocations } from "./agent-facing-instruction";
import type { CliEnvelope, CliSuccessEnvelope } from "./types";

export type AgentProfileId = "codex" | "claude" | "opencode";

export type AgentProfileDescriptor = {
  id: AgentProfileId;
  adapter: "codex_plugin" | "claude_code" | "opencode";
  commandSurface: "@loom" | "/loom";
};

const PROFILE_REGISTRY: Record<AgentProfileId, AgentProfileDescriptor> = {
  codex: {
    id: "codex",
    adapter: "codex_plugin",
    commandSurface: "@loom",
  },
  claude: {
    id: "claude",
    adapter: "claude_code",
    commandSurface: "/loom",
  },
  opencode: {
    id: "opencode",
    adapter: "opencode",
    commandSurface: "/loom",
  },
};

const AGENT_FACING_COMMANDS = new Set([
  "plan",
  "inspect",
  "continue",
  "next-task",
  "record-result",
  "review",
  "review.accept",
  "review.resolve",
  "brainstorm.start",
  "brainstorm.accept",
  "brainstorm.answer",
  "brainstorm.confirm",
  "technical-baseline.request",
  "technical-baseline.accept",
  "repository-context.request",
  "repository-context.accept",
  "planning-contract.create",
  "architecture.request",
  "architecture.accept",
  "task-plan.request",
  "task-plan.accept",
  "repair.request",
  "repair.submit",
  "deploy.run",
  "deploy.prepare",
  "deploy.up",
  "deploy.validate",
  "deploy.bootstrap",
  "deploy.repair",
]);

export function supportedAgentProfiles(): AgentProfileId[] {
  return Object.keys(PROFILE_REGISTRY) as AgentProfileId[];
}

export function agentProfileFromEnv(env: NodeJS.ProcessEnv): AgentProfileDescriptor | undefined {
  const raw = env.LOOM_AGENT_PROFILE;
  if (raw === undefined || raw.trim() === "") {
    return undefined;
  }
  const normalized = raw.trim().toLowerCase();
  if (normalized === "codex" || normalized === "claude" || normalized === "opencode") {
    return PROFILE_REGISTRY[normalized];
  }
  throw new LoomError({
    code: "INVALID_AGENT_PROFILE",
    message: `Unsupported LOOM_AGENT_PROFILE: ${raw}.`,
    details: {
      provided: raw,
      supportedProfiles: supportedAgentProfiles(),
      repairInstruction: "Use the active adapter to rerun the same loom command with its own LOOM_AGENT_PROFILE value.",
    },
    exitCode: 64,
  });
}

export function commandRequiresAgentProfile(command: string): boolean {
  return AGENT_FACING_COMMANDS.has(command);
}

export function missingAgentProfileError(command: string): LoomError {
  return new LoomError({
    code: "AGENT_PROFILE_REQUIRED",
    message: "LOOM_AGENT_PROFILE is required for agent-facing loom workflow commands.",
    details: {
      command,
      env: "LOOM_AGENT_PROFILE",
      supportedProfiles: supportedAgentProfiles(),
      adapterContract: {
        codex: "Codex plugin/skill commands must set LOOM_AGENT_PROFILE=codex.",
        claude: "Claude Code plugin skills must set LOOM_AGENT_PROFILE=claude.",
        opencode: "opencode command files must set LOOM_AGENT_PROFILE=opencode.",
      },
      repairInstruction: "Adapter misconfiguration: rerun the exact same loom command with the current adapter's LOOM_AGENT_PROFILE and preserve all original arguments.",
    },
    exitCode: 64,
  });
}

export function withAgentProfile<T>(
  envelope: CliEnvelope<T>,
  profile: AgentProfileDescriptor | undefined,
): CliEnvelope<T> {
  if (!envelope.ok || profile === undefined) {
    return envelope;
  }
  const instruction = withAgentCommandInvocations(envelope.instruction, profile.id, envelope.projectRoot);
  const actionRequired = withActionRequiredCommandInvocations(envelope.actionRequired, profile.id, envelope.projectRoot);
  return {
    ...envelope,
    agentProfile: profile,
    ...(actionRequired ? { actionRequired } : {}),
    ...(instruction ? { instruction } : {}),
    data: withDataInstruction(envelope.data, instruction, profile.id, envelope.projectRoot),
  } satisfies CliSuccessEnvelope<T>;
}

function withActionRequiredCommandInvocations(
  actionRequired: CliSuccessEnvelope["actionRequired"],
  profile: AgentProfileId,
  projectRoot: string,
): CliSuccessEnvelope["actionRequired"] {
  if (!actionRequired) {
    return actionRequired;
  }
  return withAgentCommandInvocations(actionRequired, profile, projectRoot) as CliSuccessEnvelope["actionRequired"] ?? actionRequired;
}

function withDataInstruction<T>(
  data: T,
  instruction: Record<string, unknown> | undefined,
  profile: AgentProfileId,
  projectRoot: string,
): T {
  if (!isRecord(data)) {
    return data;
  }
  const output: Record<string, unknown> = { ...data };
  if (instruction && isRecord(data.instruction)) {
    output.instruction = instruction;
  }
  if (isRecord(data.executionRequest) && isRecord(data.executionRequest.submitCommand)) {
    const executionRequestWithInvocation = withAgentCommandInvocations({
      submitCommand: data.executionRequest.submitCommand,
    }, profile, projectRoot);
    output.executionRequest = {
      ...data.executionRequest,
      submitCommand: executionRequestWithInvocation?.submitCommand ?? data.executionRequest.submitCommand,
    };
  }
  if (isRecord(data.repairInstruction)) {
    output.repairInstruction = withAgentCommandInvocations(data.repairInstruction, profile, projectRoot) ?? data.repairInstruction;
  }
  return output as T;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
