import type { ArchitectureArtifactContract } from "./contracts";

export type RuntimeDeliveryClosureRequirementContract = {
  required: true;
  taskKind: "runtime_delivery_closure";
  runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery";
  requiredContractFields: string[];
  requiredCodeLevelChecks: Array<{
    checkId: string;
    contractField: string;
    objective: string;
    acceptableEvidence: Array<"static_check" | "manual_command_output" | "runtime_api_check">;
  }>;
  requiredClosureGroupShape: {
    groupIdSuggestion: string;
    position: "final_group";
    taskIds: ["task-runtime-delivery-closure"];
    dependsOn: {
      source: "outline.groups[].groupId";
      mustInclude: "every group containing a task with runtimeDeliveryRequirement.appliesToThisTask=true, excluding the closure group";
      mustNotUse: "cross-group task.dependsOn";
    };
    allowedExtraTasks: false;
  };
  requiredClosureTaskShape: {
    taskIdSuggestion: "task-runtime-delivery-closure";
    taskKind: "runtime_delivery_closure";
    groupId: "same as requiredClosureGroupShape.groupIdSuggestion or the final closure group id";
    dependsOn: {
      allowedWithinSameGroupOnly: true;
      crossGroupTaskDependsOnAllowed: false;
      rule: "Use outline.groups[].dependsOn for cross-group ordering.";
    };
    runtimeDeliveryRequirement: {
      appliesToThisTask: true;
      runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery";
      affectedContractFields: "exactly requiredContractFields";
      requiredCodeLevelChecks: "exactly requiredCodeLevelChecks";
      evidenceExpectedInTaskResult: string[];
      forbiddenActions: string[];
    };
  };
  generationOrder: string[];
  dependencyRule: string;
  taskRequirementRule: string;
  verificationBoundary: "code_level_only";
};

export function runtimeDeliveryClosureRequirementContract(
  aac?: ArchitectureArtifactContract,
): RuntimeDeliveryClosureRequirementContract | null {
  const runtime = aac?.runtimeDelivery;
  if (!runtime || runtime.status !== "modified") return null;
  return runtimeDeliveryClosureRequirementContractForRuntime(runtime);
}

export function runtimeDeliveryClosureRequirementContractForRuntime(
  runtime: NonNullable<ArchitectureArtifactContract["runtimeDelivery"]>,
): RuntimeDeliveryClosureRequirementContract {
  const requiredContractFields = runtimeDeliveryClosureFields(runtime);
  const requiredCodeLevelChecks = requiredContractFields.map((field) => ({
    checkId: runtimeDeliveryClosureCheckId(field),
    contractField: field,
    objective: `Confirm ${field} is closed at code level against RuntimeDeliveryContract.`,
    acceptableEvidence: acceptableEvidenceForRuntimeClosureField(field),
  }));
  return {
    required: true,
    taskKind: "runtime_delivery_closure",
    runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
    requiredContractFields,
    requiredCodeLevelChecks,
    requiredClosureGroupShape: {
      groupIdSuggestion: "group-runtime-delivery-closure",
      position: "final_group",
      taskIds: ["task-runtime-delivery-closure"],
      dependsOn: {
        source: "outline.groups[].groupId",
        mustInclude: "every group containing a task with runtimeDeliveryRequirement.appliesToThisTask=true, excluding the closure group",
        mustNotUse: "cross-group task.dependsOn",
      },
      allowedExtraTasks: false,
    },
    requiredClosureTaskShape: {
      taskIdSuggestion: "task-runtime-delivery-closure",
      taskKind: "runtime_delivery_closure",
      groupId: "same as requiredClosureGroupShape.groupIdSuggestion or the final closure group id",
      dependsOn: {
        allowedWithinSameGroupOnly: true,
        crossGroupTaskDependsOnAllowed: false,
        rule: "Use outline.groups[].dependsOn for cross-group ordering.",
      },
      runtimeDeliveryRequirement: {
        appliesToThisTask: true,
        runtimeDeliveryRef: "architectureArtifactContractRef#/runtimeDelivery",
        affectedContractFields: "exactly requiredContractFields",
        requiredCodeLevelChecks: "exactly requiredCodeLevelChecks",
        evidenceExpectedInTaskResult: [
          "runtimeDeliveryEvidence.checkedFields covers every requiredContractFields entry.",
          "runtimeDeliveryEvidence.codeLevelChecks reports every requiredCodeLevelChecks entry using the exact checkId.",
          "commandsRun records only code-level checks actually run; environment blockers become unverifiedItems.",
        ],
        forbiddenActions: [
          "do_not_create_or_edit_deploy_generated_files",
          "do_not_require_clean_install_or_container_build_for_this_task",
          "do_not_require_docker_or_registry_or_full_deploy_for_this_task",
          "do_not_claim_deploy_success_from_code_level_checks_only",
        ],
      },
    },
    generationOrder: [
      "Create all implementation/runtime-affecting groups first.",
      "Create one final closure group after them.",
      "Set the final closure group dependsOn to every runtime-affecting group id.",
      "Put exactly one runtime_delivery_closure task in the final closure group.",
      "Do not add cross-group task.dependsOn; use group dependsOn for cross-group ordering.",
    ],
    dependencyRule: "Put runtime_delivery_closure in a final closure group. That closure group must depend on every group containing runtime-affecting tasks. Do not use cross-group task.dependsOn.",
    taskRequirementRule: "runtimeDeliveryRequirement.affectedContractFields and requiredCodeLevelChecks must exactly match this contract's requiredContractFields and requiredCodeLevelChecks.",
    verificationBoundary: "code_level_only",
  };
}

export function runtimeDeliveryClosureFields(
  runtimeDelivery: NonNullable<ArchitectureArtifactContract["runtimeDelivery"]>,
): string[] {
  const fields: string[] = [];
  if (runtimeDelivery.build?.command) {
    fields.push("build.command");
  }
  if (runtimeDelivery.start?.command) {
    fields.push("start.command");
  }
  if ((runtimeDelivery.runtimeSurfaces ?? []).length > 0) {
    fields.push("runtimeSurfaces");
  }
  if (runtimeDelivery.httpProbes) {
    fields.push("httpProbes");
  }
  if (runtimeDelivery.deliveryMechanics?.staticAssets) {
    fields.push("deliveryMechanics.staticAssets");
  }
  if (runtimeDelivery.deliveryMechanics?.api) {
    fields.push("deliveryMechanics.api");
  }
  if (runtimeDelivery.frontend) {
    fields.push("frontend");
  }
  if (runtimeDelivery.api) {
    fields.push("api");
  }
  if (runtimeDelivery.environment) {
    fields.push("environment");
  }
  if (runtimeDeliveryHasCodegen(runtimeDelivery)) {
    fields.push("deliveryMechanics.codegen");
  }
  return fields;
}

export function runtimeDeliveryClosureCheckId(contractField: string): string {
  return `rd-closure-${contractField.replace(/[^a-z0-9]+/gi, "-").replace(/^-|-$/g, "").toLowerCase()}`;
}

function acceptableEvidenceForRuntimeClosureField(
  field: string,
): Array<"static_check" | "manual_command_output" | "runtime_api_check"> {
  if (field === "httpProbes" || field === "runtimeSurfaces" || field === "api" || field === "frontend") {
    return ["static_check", "runtime_api_check", "manual_command_output"];
  }
  return ["static_check", "manual_command_output"];
}

function runtimeDeliveryHasCodegen(runtimeDelivery: NonNullable<ArchitectureArtifactContract["runtimeDelivery"]>): boolean {
  const codegen = runtimeDelivery.deliveryMechanics?.codegen;
  if (!codegen) {
    return false;
  }
  return codegen.required !== "no" || codegen.commands.length > 0;
}
