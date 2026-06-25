import { invalidArgument } from "../errors";
import type {
  DeployProvider,
  DeploymentProviderCandidate,
  DeploymentProviderPolicy,
  DeploymentCodeProbe,
  DeploymentSourceModel,
} from "./types";
import type { ExistingDeploymentFiles } from "./existing";

export type DeploymentStrategy = {
  provider: DeployProvider;
  reason: string;
  policy: DeploymentProviderPolicy;
  candidates: DeploymentProviderCandidate[];
};

export function resolveDeploymentStrategy(input: {
  codeProbe: DeploymentCodeProbe;
  existing: ExistingDeploymentFiles;
  policy?: Partial<DeploymentProviderPolicy>;
}): DeploymentStrategy {
  const policy = normalizeProviderPolicy(input.policy);
  validateProviderPolicy({ ...input, policy });
  const selectedProvider = selectProvider({ ...input, policy });

  return {
    provider: selectedProvider,
    reason: reasonFor(selectedProvider, { ...input, policy }),
    policy,
    candidates: providerCandidates(selectedProvider, { ...input, policy }),
  };
}

export function deploymentStrategyForSourceModel(input: {
  strategy: DeploymentStrategy;
  sourceModel: DeploymentSourceModel;
}): DeploymentStrategy {
  if (input.sourceModel.services.length <= 1 || input.strategy.provider !== "dockerfile-existing") {
    return input.strategy;
  }

  return {
    ...input.strategy,
    provider: "dockerfile-template",
    reason: "Deployment source model has multiple application services, so loom will generate service-specific Dockerfile/Compose assets instead of reusing one root Dockerfile as the whole application.",
    candidates: input.strategy.candidates.map((candidate) => {
      if (candidate.provider === "dockerfile-existing") {
        return {
          ...candidate,
          status: "skipped" as const,
          reason: "Skipped because one root Dockerfile cannot represent multiple application services from the deployment source model.",
        };
      }
      if (candidate.provider === "dockerfile-template") {
        return {
          ...candidate,
          status: "selected" as const,
          reason: "Selected because the deployment source model requires service-specific generated Dockerfiles.",
        };
      }
      return candidate;
    }),
  };
}

function validateProviderPolicy(input: {
  existing: ExistingDeploymentFiles;
  policy: DeploymentProviderPolicy;
}): void {
  if (input.policy.forceGenerate && input.policy.provider && input.policy.provider !== "dockerfile-template") {
    throw invalidArgument("--force-generate cannot be combined with an existing-asset provider.", {
      provider: input.policy.provider,
    });
  }
  if (input.policy.provider === "compose-existing" && !input.existing.composePath) {
    throw invalidArgument("Provider policy selected compose-existing, but no root-level Compose file was found.");
  }
  if (input.policy.provider === "dockerfile-existing" && !input.existing.dockerfilePath) {
    throw invalidArgument("Provider policy selected dockerfile-existing, but no root-level Dockerfile was found.");
  }
}

function selectProvider(input: {
  codeProbe: DeploymentCodeProbe;
  existing: ExistingDeploymentFiles;
  policy: DeploymentProviderPolicy;
}): DeployProvider {
  if (input.policy.forceGenerate) {
    return "dockerfile-template";
  }
  if (input.policy.provider) {
    return input.policy.provider;
  }
  if (input.policy.reuseExisting && input.existing.composePath) {
    return "compose-existing";
  }
  if (input.policy.reuseExisting && input.existing.dockerfilePath) {
    return "dockerfile-existing";
  }
  return "dockerfile-template";
}

function reasonFor(
  provider: DeployProvider,
  input: {
    codeProbe: DeploymentCodeProbe;
    existing: ExistingDeploymentFiles;
    policy: DeploymentProviderPolicy;
  },
): string {
  if (input.policy.forceGenerate) {
    return "Provider policy forces generated Dockerfile/Compose assets.";
  }
  if (input.policy.provider) {
    return `Provider policy explicitly selected ${input.policy.provider}.`;
  }
  if (!input.policy.reuseExisting) {
    return "Provider policy disables existing deployment asset reuse, so loom will generate Dockerfile/Compose assets.";
  }

  switch (provider) {
    case "compose-existing":
      return "Root-level Compose file exists, so loom will reuse it without overwriting user deployment assets.";
    case "dockerfile-existing":
      return "Root-level Dockerfile exists, so loom will reuse it and materialize only a local Compose wrapper.";
    case "dockerfile-template":
      return `Repository probes found ${input.codeProbe.kind} runtime evidence, so loom will generate deployment files from the deployment source model.`;
  }
}

function providerCandidates(
  selectedProvider: DeployProvider,
  input: {
    codeProbe: DeploymentCodeProbe;
    existing: ExistingDeploymentFiles;
    policy: DeploymentProviderPolicy;
  },
): DeploymentProviderCandidate[] {
  return [
    {
      provider: "compose-existing",
      status: statusFor("compose-existing", selectedProvider, Boolean(input.existing.composePath)),
      reason: candidateReason("compose-existing", input),
      commands: [["docker", "compose", "config", "--quiet"]],
    },
    {
      provider: "dockerfile-existing",
      status: statusFor("dockerfile-existing", selectedProvider, Boolean(input.existing.dockerfilePath)),
      reason: candidateReason("dockerfile-existing", input),
      commands: [["docker", "compose", "up", "-d", "--build"]],
    },
    {
      provider: "dockerfile-template",
      status: selectedProvider === "dockerfile-template" ? "selected" : "available",
      reason: candidateReason("dockerfile-template", input),
      commands: [["docker", "compose", "up", "-d", "--build"]],
    },
  ];
}

function candidateReason(
  provider: DeployProvider,
  input: {
    codeProbe: DeploymentCodeProbe;
    existing: ExistingDeploymentFiles;
    policy: DeploymentProviderPolicy;
  },
): string {
  if (input.policy.forceGenerate && provider !== "dockerfile-template") {
    return "Skipped because provider policy forces generated Dockerfile/Compose assets.";
  }
  if (input.policy.provider && input.policy.provider !== provider) {
    return `Skipped because provider policy explicitly selected ${input.policy.provider}.`;
  }
  if (!input.policy.reuseExisting && provider !== "dockerfile-template") {
    return "Skipped because provider policy disables existing deployment asset reuse.";
  }

  switch (provider) {
    case "compose-existing":
      return input.existing.composePath
        ? "Existing Compose file found at project root."
        : "No root-level Compose file was found.";
    case "dockerfile-existing":
      return input.existing.dockerfilePath
        ? "Existing Dockerfile found at project root."
        : "No root-level Dockerfile was found.";
    case "dockerfile-template":
      return templateReason(input.codeProbe.kind);
  }
}

function statusFor(
  provider: DeployProvider,
  selectedProvider: DeployProvider,
  isAvailable: boolean,
): DeploymentProviderCandidate["status"] {
  if (provider === selectedProvider) {
    return "selected";
  }
  return isAvailable ? "available" : "skipped";
}

export function normalizeProviderPolicy(
  policy?: Partial<DeploymentProviderPolicy>,
): DeploymentProviderPolicy {
  return {
    provider: policy?.provider,
    reuseExisting: policy?.forceGenerate ? false : policy?.reuseExisting ?? true,
    forceGenerate: policy?.forceGenerate ?? false,
  };
}

function templateReason(kind: DeploymentCodeProbe["kind"]): string {
  if (kind === "unknown") {
    return "Available as a deterministic local preview so a coding agent can inspect and repair deployment files if the first attempt fails.";
  }
  return `Available because loom can model a ${kind} runtime as a deployment source service.`;
}
