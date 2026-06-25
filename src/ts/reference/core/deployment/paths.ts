import path from "node:path";
import { getLoomPaths } from "../state/paths";

export type DeploymentPaths = {
  specsDir: string;
  stateDir: string;
  logsDir: string;
  repairsDir: string;
  evidenceDir: string;
  specFile: string;
  stateFile: string;
  activeOperationFile: string;
  staleOperationFile: string;
  logFile: string;
  repairFile: string;
  failureFile: string;
  codeEvidenceFile: string;
  generatedDir: string;
  dockerfileFile: string;
  composeFile: string;
  dockerignoreFile: string;
};

export function getDeploymentPaths(projectRoot: string): DeploymentPaths {
  const paths = getLoomPaths(projectRoot);
  const deploymentDir = path.join(paths.loomDir, "deployment");
  const specsDir = path.join(deploymentDir, "specs");
  const stateDir = path.join(deploymentDir, "state");
  const logsDir = path.join(deploymentDir, "logs");
  const repairsDir = path.join(deploymentDir, "repairs");
  const evidenceDir = path.join(deploymentDir, "evidence");
  const generatedDir = path.join(specsDir, "generated");

  return {
    specsDir,
    stateDir,
    logsDir,
    repairsDir,
    evidenceDir,
    specFile: path.join(specsDir, "local.json"),
    stateFile: path.join(stateDir, "local.json"),
    activeOperationFile: path.join(stateDir, "active-operation.json"),
    staleOperationFile: path.join(stateDir, "last-stale-operation.json"),
    logFile: path.join(logsDir, "local.log"),
    repairFile: path.join(stateDir, "repair-request.json"),
    failureFile: path.join(stateDir, "latest-failure.json"),
    codeEvidenceFile: path.join(evidenceDir, "latest-code-evidence.json"),
    generatedDir,
    dockerfileFile: path.join(generatedDir, "Dockerfile"),
    composeFile: path.join(generatedDir, "compose.yaml"),
    dockerignoreFile: path.join(generatedDir, "Dockerfile.dockerignore"),
  };
}

export function getDeploymentRepairPaths(projectRoot: string, repairId: string): {
  repairDir: string;
  requestFile: string;
  taskExecutionRequestFile: string;
  resultFile: string;
} {
  const paths = getDeploymentPaths(projectRoot);
  const repairDir = path.join(paths.repairsDir, repairId);
  return {
    repairDir,
    requestFile: path.join(repairDir, "request.json"),
    taskExecutionRequestFile: path.join(repairDir, "task-execution-request.json"),
    resultFile: path.join(repairDir, "result.json"),
  };
}
