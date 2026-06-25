import type { AgentProfileId } from "./agent-profile";

export type AgentCommandInvocation = {
  kind: "loom_user_launcher";
  launcherRef: "$HOME/.loom/bin/loom-cli";
  env: {
    LOOM_AGENT_PROFILE: AgentProfileId;
    LOOM_COMPACT_OUTPUT: "1";
  };
  argv: string[];
  argvWithProjectRoot: string[];
  projectRoot: string;
  projectRootRequired: true;
  usage: string;
};

export function loomCommandInvocation(
  profile: AgentProfileId,
  argv: unknown,
  projectRoot: string,
): AgentCommandInvocation | undefined {
  if (!Array.isArray(argv) || !argv.every((item): item is string => typeof item === "string")) {
    return undefined;
  }
  const argvWithProjectRoot = argvHasProjectRoot(argv) ? argv : [...argv, "--project-root", projectRoot];
  return {
    kind: "loom_user_launcher",
    launcherRef: "$HOME/.loom/bin/loom-cli",
    env: {
      LOOM_AGENT_PROFILE: profile,
      LOOM_COMPACT_OUTPUT: "1",
    },
    argv,
    argvWithProjectRoot,
    projectRoot,
    projectRootRequired: true,
    usage: "Run env variables plus launcherRef plus argvWithProjectRoot exactly. Do not use bare loom or depend on PATH.",
  };
}

function argvHasProjectRoot(argv: string[]): boolean {
  return argv.some((arg) => arg === "--project-root" || arg.startsWith("--project-root="));
}
