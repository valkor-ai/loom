import type { ArchitectureArtifactContract, Task, TaskResult } from "./contracts";

export type WorkflowClosureRequirement = {
  closureId: string;
  workflowRef: string;
  workflowName: string;
  surfaceRefs: string[];
  operationPathRefs: string[];
  dataViewRefs: string[];
  actionRefs: string[];
  moduleRefs: string[];
  acceptanceRefs: string[];
  interfaceRefs: string[];
  stateMachineRefs: string[];
  stepRefs: string[];
  entry: ArchitectureArtifactContract["userFlows"][number]["entry"];
  derivation: {
    source: "aac_frontend_surface_userflow_interface";
    rule: string;
  };
  requiredDataBindingMode: "wired";
  satisfiedDataBindingModes: ["wired"];
  staticModePolicy: "not_satisfied";
  knownGapPolicy: "not_satisfied_when_required_closure";
  requiredEvidence: [
    "user_action",
    "declared_interface_invocation",
    "state_or_persistence_change",
    "success_or_blocking_feedback",
  ];
  interfaces: Array<{
    interfaceId: string;
    name: string;
    type: ArchitectureArtifactContract["interfaces"][number]["type"];
    role?: ArchitectureArtifactContract["interfaces"][number]["role"];
    method?: string;
    path?: string;
    requestSchema: ArchitectureArtifactContract["interfaces"][number]["requestSchema"];
    responseSchema: ArchitectureArtifactContract["interfaces"][number]["responseSchema"];
    errorSchema: ArchitectureArtifactContract["interfaces"][number]["errorSchema"];
  }>;
};

const executableInterfaceTypes = new Set([
  "http_api",
  "service_method",
  "cli_command",
  "event",
  "job",
  "external_adapter",
]);

export function buildWorkflowClosureRequirements(aac: ArchitectureArtifactContract): WorkflowClosureRequirement[] {
  const frontend = aac.frontendExperience;
  if (!frontend?.required) return [];

  const flowById = new Map(aac.userFlows.map((flow) => [flow.flowId, flow]));
  const interfaceById = new Map(aac.interfaces.map((contract) => [contract.interfaceId, contract]));
  const surfaceRefsByFlow = new Map<string, string[]>();
  for (const surface of frontend.surfaces) {
    for (const workflowRef of surface.workflowRefs) {
      const refs = surfaceRefsByFlow.get(workflowRef) ?? [];
      refs.push(surface.surfaceId);
      surfaceRefsByFlow.set(workflowRef, uniqueRefs(refs));
    }
  }

  const requirements: WorkflowClosureRequirement[] = [];
  for (const [workflowRef, surfaceRefs] of surfaceRefsByFlow) {
    const flow = flowById.get(workflowRef);
    if (!flow || flow.kind !== "user_interaction" || flow.steps.length === 0) continue;
    const operationPaths = (frontend.operationPaths ?? []).filter((operationPath) =>
      operationPath.workflowRef === workflowRef ||
      (operationPath.surfaceRef ? surfaceRefs.includes(operationPath.surfaceRef) : false)
    );
    const operationPathRefs = uniqueRefs(operationPaths.map((operationPath) => operationPath.pathId));
    const dataViewRefs = uniqueRefs(operationPaths.flatMap((operationPath) => operationPath.dataViewRefs));
    const actionRefs = uniqueRefs(operationPaths.flatMap((operationPath) => operationPath.actionRefs));

    for (const step of flow.steps) {
      const candidateInterfaceRefs = uniqueRefs(step.interfaceRefs.length > 0 ? step.interfaceRefs : flow.interfaceRefs);
      const executableInterfaces: Array<ArchitectureArtifactContract["interfaces"][number]> = [];
      for (const ref of candidateInterfaceRefs) {
        const contract = interfaceById.get(ref);
        if (contract && executableInterfaceTypes.has(contract.type) && hasInterfaceShape(contract)) {
          executableInterfaces.push(contract);
        }
      }
      if (executableInterfaces.length === 0) continue;

      const interfaceRefs = uniqueRefs(executableInterfaces.map((contract) => contract.interfaceId));
      requirements.push({
        closureId: `closure:${flow.flowId}:${step.stepId}`,
        workflowRef: flow.flowId,
        workflowName: flow.name,
        surfaceRefs,
        operationPathRefs,
        dataViewRefs,
        actionRefs,
        moduleRefs: flow.moduleRefs,
        acceptanceRefs: flow.acceptanceRefs,
        interfaceRefs,
        stateMachineRefs: uniqueRefs(step.stateMachineRefs),
        stepRefs: [step.stepId],
        entry: flow.entry,
        derivation: {
          source: "aac_frontend_surface_userflow_interface",
          rule: "Generated only from AAC frontendExperience.surfaces[].workflowRefs -> userFlows[kind=user_interaction].steps[].interfaceRefs or flow.interfaceRefs fallback -> executable interfaces with request/response shape.",
        },
        requiredDataBindingMode: "wired",
        satisfiedDataBindingModes: ["wired"],
        staticModePolicy: "not_satisfied",
        knownGapPolicy: "not_satisfied_when_required_closure",
        requiredEvidence: [
          "user_action",
          "declared_interface_invocation",
          "state_or_persistence_change",
          "success_or_blocking_feedback",
        ],
        interfaces: executableInterfaces.map((contract) => ({
          interfaceId: contract.interfaceId,
          name: contract.name,
          type: contract.type,
          ...(contract.role ? { role: contract.role } : {}),
          ...(contract.method ? { method: contract.method } : {}),
          ...(contract.path ? { path: contract.path } : {}),
          requestSchema: contract.requestSchema ?? [],
          responseSchema: contract.responseSchema ?? [],
          errorSchema: contract.errorSchema ?? [],
        })),
      });
    }
  }

  return requirements;
}

export function closureRequirementsForTask(task: Task, aac: ArchitectureArtifactContract): WorkflowClosureRequirement[] {
  return buildWorkflowClosureRequirements(aac).filter((requirement) => taskCoversWorkflowClosure(task, requirement));
}

export function taskCoversWorkflowClosure(task: Task, requirement: WorkflowClosureRequirement): boolean {
  const artifactRefs = task.writeBoundary.artifactRefs;
  if (!artifactRefs.userFlows.includes(requirement.workflowRef)) return false;
  if (!requirement.interfaceRefs.every((ref) => artifactRefs.interfaces.includes(ref))) return false;
  if (!requirement.acceptanceRefs.every((ref) => task.acceptanceRefs.includes(ref))) return false;
  if (!task.frontendExperienceRequirement) return false;
  if (!task.implementationActions.includes("wire_reference_in_api_or_ui")) return false;
  return task.verificationIntents.some((intent) =>
    requirement.acceptanceRefs.every((ref) => intent.acceptanceRefs.includes(ref)) &&
    intent.acceptableEvidence.some((evidence) => evidence === "runtime_api_check" || evidence === "automated_test")
  );
}

export function frontendSelfCheckViolatesRequiredClosure(result: TaskResult, requirementIds: string[]): {
  violates: boolean;
  actualMode: string | null;
  status: string | null;
  knownGapCount: number;
} {
  const selfCheck = result.frontendExperienceSelfCheck;
  if (!isRecord(selfCheck) || requirementIds.length === 0) {
    return { violates: false, actualMode: null, status: null, knownGapCount: 0 };
  }
  const status = typeof selfCheck.status === "string" ? selfCheck.status : null;
  const dataBinding = isRecord(selfCheck.dataBinding) ? selfCheck.dataBinding : {};
  const actualMode = typeof dataBinding.mode === "string" ? dataBinding.mode : null;
  const knownGapCount = Array.isArray(selfCheck.knownGaps) ? selfCheck.knownGaps.length : 0;
  const knownGapsExplicitlyClosed = Array.isArray(selfCheck.knownGaps) && knownGapCount === 0;
  return {
    violates: status === "satisfied" && (actualMode !== "wired" || !knownGapsExplicitlyClosed),
    actualMode,
    status,
    knownGapCount,
  };
}

function hasInterfaceShape(contract: ArchitectureArtifactContract["interfaces"][number]): boolean {
  return (contract.requestSchema?.length ?? 0) > 0 && (contract.responseSchema?.length ?? 0) > 0;
}

function uniqueRefs(refs: string[]): string[] {
  return [...new Set(refs.filter((ref) => typeof ref === "string" && ref.length > 0))];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
