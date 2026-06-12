import path from "node:path";

export const LOOM_DIR = ".loom";

export type LoomPaths = {
  root: string;
  loomDir: string;
  configFile: string;
  statusFile: string;
  metricsDir: string;
  tokenSavingTelemetryFile: string;
  gitignoreFile: string;
  directories: string[];
};

export type DeliveryPhaseLocator = {
  deliveryId: string;
  phaseId: string;
};

export function resolveProjectRoot(projectRoot: string): string {
  return path.resolve(projectRoot);
}

export function getLoomPaths(projectRoot: string): LoomPaths {
  const root = resolveProjectRoot(projectRoot);
  const loomDir = path.join(root, LOOM_DIR);
  const metricsDir = path.join(loomDir, "metrics");
  const relativeDirs = [
    path.join("contracts", "repo-signals"),
    "deliveries",
    "metrics",
    "tmp",
  ];

  return {
    root,
    loomDir,
    configFile: path.join(loomDir, "config.json"),
    statusFile: path.join(loomDir, "status.json"),
    metricsDir,
    tokenSavingTelemetryFile: path.join(metricsDir, "token-saving.json"),
    gitignoreFile: path.join(loomDir, ".gitignore"),
    directories: relativeDirs.map((dir) => path.join(loomDir, dir)),
  };
}

export function toProjectRelative(projectRoot: string, absolutePath: string): string {
  return path.relative(resolveProjectRoot(projectRoot), absolutePath).split(path.sep).join("/");
}

export function fromProjectRelative(projectRoot: string, relativePath: string): string {
  return path.join(resolveProjectRoot(projectRoot), relativePath.split("/").join(path.sep));
}

export function deliveryDir(projectRoot: string, deliveryId: string): string {
  return path.join(resolveProjectRoot(projectRoot), LOOM_DIR, "deliveries", deliveryId);
}

export function deliveryIndexPath(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "index.json");
}

export function phaseTmpDir(projectRoot: string, locator: DeliveryPhaseLocator): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tmp", locator.phaseId);
}

export function workspaceDir(projectRoot: string, locator: DeliveryPhaseLocator): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "workspace", locator.phaseId);
}

export function repositoryContextRequestPath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(workspaceDir(projectRoot, locator), "repository-context-requests", `${requestId}.json`);
}

export function repositoryContextCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "repository-context", requestId, "candidate.json");
}

export function repositoryContextPath(projectRoot: string, locator: DeliveryPhaseLocator): string {
  return path.join(workspaceDir(projectRoot, locator), "repository-context.json");
}

export function workspaceLatestPath(projectRoot: string, locator: DeliveryPhaseLocator): string {
  return path.join(workspaceDir(projectRoot, locator), "latest.json");
}

export function brainstormContractPath(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "brainstorms", "contract.json");
}

export function brainstormSessionRequestPath(projectRoot: string, deliveryId: string, requestId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "brainstorms", "requests", `${requestId}.json`);
}

export function brainstormCandidatePath(projectRoot: string, deliveryId: string, brainstormRunId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "tmp", "brainstorm", brainstormRunId, "candidate.json");
}

export function brainstormRequestCandidatePath(projectRoot: string, deliveryId: string, phaseId: string, requestId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "tmp", "brainstorm", phaseId, requestId, "candidate.json");
}

export function brainstormPhaseCandidatePath(projectRoot: string, deliveryId: string, phaseId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "tmp", "brainstorm", phaseId, "candidate.json");
}

export function brainstormLatestPath(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "brainstorms", "latest.json");
}

export function brainstormDecisionPath(projectRoot: string, deliveryId: string, phaseId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "brainstorms", "decisions", `${phaseId}.json`);
}

export function brainstormDecisionsIndexPath(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "brainstorms", "decisions", "index.json");
}

export function deliveryConceptGlossaryPath(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "concepts", "delivery-glossary.json");
}

export function requirementsDir(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "requirements");
}

export function requirementContextPath(projectRoot: string, deliveryId: string): string {
  return path.join(requirementsDir(projectRoot, deliveryId), "context.json");
}

export function requirementNormalizedTextPath(projectRoot: string, deliveryId: string): string {
  return path.join(requirementsDir(projectRoot, deliveryId), "normalized.txt");
}

export function requirementKeywordHintsPath(projectRoot: string, deliveryId: string): string {
  return path.join(requirementsDir(projectRoot, deliveryId), "keyword-hints.json");
}

export function requirementInputTextPath(projectRoot: string, deliveryId: string, itemId: string): string {
  return path.join(requirementsDir(projectRoot, deliveryId), "inputs", `${itemId}.txt`);
}

export function requirementExtractedTextPath(projectRoot: string, deliveryId: string, itemId: string): string {
  return path.join(requirementsDir(projectRoot, deliveryId), "extracted", `${itemId}.txt`);
}

export function phaseConceptGroundingPath(projectRoot: string, deliveryId: string, phaseId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "concepts", phaseId, "phase-concepts.json");
}

export function confirmedFrontendExperienceTargetPath(projectRoot: string, deliveryId: string, phaseId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "frontend-experience", phaseId, "confirmed-target.json");
}

export function currentFrontendExperienceTargetPath(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "frontend-experience", "current.json");
}

export function technicalBaselinePath(projectRoot: string, deliveryId?: string): string {
  if (!deliveryId) {
    throw new Error("technicalBaselinePath requires deliveryId.");
  }
  return path.join(deliveryDir(projectRoot, deliveryId), "contracts", "technical-baseline.json");
}

export function repoSignalSetPath(projectRoot: string, signalSetId: string): string {
  return path.join(resolveProjectRoot(projectRoot), LOOM_DIR, "contracts", "repo-signals", `${signalSetId}.json`);
}

export function technicalBaselineRequestPath(projectRoot: string, requestId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("technicalBaselineRequestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "contracts", "technical-baseline-requests", `${requestId}.json`);
}

export function technicalBaselineCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "technical-baseline", requestId, "candidate.json");
}

export function planningContractPath(projectRoot: string, planningContractId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`planningContractPath requires delivery/phase locator for ${planningContractId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "contracts", "planning", locator.phaseId, "pgc.json");
}

export function planningLatestPath(projectRoot: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("planningLatestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "contracts", "planning", locator.phaseId, "latest.json");
}

export function architectureRequestPath(projectRoot: string, requestId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("architectureRequestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "artifacts", "architecture", locator.phaseId, "requests", `${requestId}.json`);
}

export function architectureSectionCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string, section: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "architecture", requestId, "sections", `${section}.json`);
}

export function architectureSectionVersionPath(projectRoot: string, locator: DeliveryPhaseLocator, section: string, version: number): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "artifacts", "architecture", locator.phaseId, "sections", section, `v${version}.json`);
}

export function architectureSessionPath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "artifacts", "architecture", locator.phaseId, "sessions", `${requestId}.json`);
}

export function architectureCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "architecture", requestId, "candidate.json");
}

export function architectureContractPath(projectRoot: string, architectureArtifactContractId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`architectureContractPath requires delivery/phase locator for ${architectureArtifactContractId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "artifacts", "architecture", locator.phaseId, "aac.json");
}

export function architectureLatestPath(projectRoot: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("architectureLatestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "artifacts", "architecture", locator.phaseId, "latest.json");
}

export function taskPlanRequestPath(projectRoot: string, requestId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("taskPlanRequestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tasks", locator.phaseId, "requests", `${requestId}.json`);
}

export function taskPlanOutlineCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "task-plan", requestId, "outline.json");
}

export function taskPlanGroupCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string, groupId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "task-plan", requestId, "groups", `${groupId}.json`);
}

export function taskPlanCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "task-plan", requestId, "candidate.json");
}

export function taskPlanPath(projectRoot: string, taskPlanId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`taskPlanPath requires delivery/phase locator for ${taskPlanId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tasks", locator.phaseId, "taskplans", `${taskPlanId}.json`);
}

export function taskPlanLatestPath(projectRoot: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("taskPlanLatestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tasks", locator.phaseId, "taskplans", "latest.json");
}

export function taskPlanRunPath(projectRoot: string, taskPlanRunId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`taskPlanRunPath requires delivery/phase locator for ${taskPlanRunId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tasks", locator.phaseId, "runs", `${taskPlanRunId}.json`);
}

export function taskPlanRunLatestPath(projectRoot: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("taskPlanRunLatestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tasks", locator.phaseId, "runs", "latest.json");
}

export function taskExecutionRequestPath(projectRoot: string, requestId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("taskExecutionRequestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tasks", locator.phaseId, "execution-requests", `${requestId}.json`);
}

export function taskExecutionResultCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "task-results", requestId, "result.json");
}

export function taskResultPath(projectRoot: string, taskPlanRunId: string, taskId: string, taskResultId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`taskResultPath requires delivery/phase locator for ${taskResultId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "tasks", locator.phaseId, "results", taskPlanRunId, taskId, `${taskResultId}.json`);
}

export function reviewRequestPath(projectRoot: string, requestId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("reviewRequestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "requests", `${requestId}.json`);
}

export function reviewPacketPath(projectRoot: string, requestId: string, locator: DeliveryPhaseLocator): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "packets", `${requestId}.review-packet.json`);
}

export function reviewChangeContextPath(projectRoot: string, requestId: string, locator: DeliveryPhaseLocator): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "packets", `${requestId}.change-context.json`);
}

export function reviewResultCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "review", requestId, "result.json");
}

export function reviewResultPath(projectRoot: string, reviewId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`reviewResultPath requires delivery/phase locator for ${reviewId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "results", `${reviewId}.json`);
}

export function reviewResolutionPath(projectRoot: string, resolutionId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`reviewResolutionPath requires delivery/phase locator for ${resolutionId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "manual", "resolutions", `${resolutionId}.json`);
}

export function manualReviewRequestPath(projectRoot: string, requestId: string, locator: DeliveryPhaseLocator): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "manual", "requests", `${requestId}.json`);
}

export function reviewArtifactsDir(projectRoot: string, reviewId: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error(`reviewArtifactsDir requires delivery/phase locator for ${reviewId}.`);
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "artifacts", reviewId);
}

export function reviewLatestPath(projectRoot: string, locator?: DeliveryPhaseLocator): string {
  if (!locator) {
    throw new Error("reviewLatestPath requires delivery/phase locator.");
  }
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "reviews", locator.phaseId, "latest.json");
}

export function operationLeasePath(projectRoot: string, deliveryId: string): string {
  return path.join(deliveryDir(projectRoot, deliveryId), "operations", "active-lease.json");
}

export function continueDecisionLatestPath(projectRoot: string, locator: DeliveryPhaseLocator): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "control", locator.phaseId, "continue-latest.json");
}

export function repairRequestPath(projectRoot: string, locator: DeliveryPhaseLocator, repairType: string, requestId: string): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "repairs", locator.phaseId, repairType, "requests", `${requestId}.json`);
}

export function repairCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "repairs", requestId, "candidate.json");
}

export function userDecisionRequestPath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(deliveryDir(projectRoot, locator.deliveryId), "decisions", locator.phaseId, "requests", `${requestId}.json`);
}

export function userDecisionCandidatePath(projectRoot: string, locator: DeliveryPhaseLocator, requestId: string): string {
  return path.join(phaseTmpDir(projectRoot, locator), "decisions", requestId, "resolution.json");
}
