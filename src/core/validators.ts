import path from "node:path";
import { ZodError, type ZodSchema } from "zod";
import {
  type ArchitectureArtifactContract,
  type ContractIssue,
  type FieldShape,
  type PlanningGenerationContract,
  type ReviewRequest,
  type ReviewResult,
  type TaskExecutionRequest,
  type TaskPlan,
  type TaskResult,
  type TechnicalBaseline,
  architectureArtifactContractSchema,
  contractIssueSchema,
  planningGenerationContractSchema,
  reviewResultSchema,
  taskPlanSchema,
  taskResultSchema,
  technicalBaselineSchema,
} from "./contracts";
import { runtimeDeliveryClosureRequirementContractForRuntime } from "./runtime-delivery-closure";
import {
  buildWorkflowClosureRequirements,
  frontendSelfCheckViolatesRequiredClosure,
  taskCoversWorkflowClosure,
} from "./workflow-closure";

type IssueTemplate = {
  message: string;
  repairHint: string;
  repairability?: ContractIssue["repairability"];
};

const ISSUE_TEMPLATES: Record<string, IssueTemplate> = {
  SCHEMA_INVALID: {
    message: "Contract schema validation failed.",
    repairHint: "Repair the contract shape at the reported path and return a complete replacement candidate.",
    repairability: "agent_repairable",
  },
  SOURCE_NOT_READY: {
    message: "Source contract is not ready for this operation.",
    repairHint: "Regenerate only after the source contracts are ready.",
    repairability: "blocked",
  },
  BASELINE_MISMATCH: {
    message: "Contract source baseline does not match the upstream contract baseline.",
    repairHint: "Regenerate from the current ready source contracts.",
    repairability: "blocked",
  },
  PHASE_SOURCE_MISMATCH: {
    message: "Contract source phase does not match the selected phase scope.",
    repairHint: "Regenerate from the current phase Brainstorm contract and phase locator.",
    repairability: "blocked",
  },
  DUPLICATE_ID: {
    message: "Identifier is duplicated within the contract section.",
    repairHint: "Rename the duplicate identifier and update refs if needed.",
  },
  INVALID_SCOPE_REF: {
    message: "Referenced scope is not part of the current phase included scope.",
    repairHint: "Remove refs outside the current phase scope.",
  },
  INVALID_DEFERRED_REF: {
    message: "Referenced deferred scope does not exist in the current phase deferred scope.",
    repairHint: "Use an existing deferred scope ref or remove the invalid ref.",
  },
  INVALID_ACCEPTANCE_REF: {
    message: "Referenced acceptance does not exist in the current phase acceptance set.",
    repairHint: "Use an existing current phase acceptance ref or remove the invalid ref.",
  },
  UNKNOWN_ARTIFACT_REF: {
    message: "Referenced artifact does not exist in the corresponding AAC section.",
    repairHint: "Use an existing AAC artifact ref or remove the invalid ref.",
  },
  INVALID_PATH: {
    message: "Path is not a safe project-relative path.",
    repairHint: "Use a safe project-relative path or remove the invalid path.",
  },
  MUST_ACCEPTANCE_NOT_COVERED: {
    message: "A must acceptance is not covered by the candidate.",
    repairHint: "Cover the must acceptance using current phase refs only.",
  },
  WORKFLOW_CLOSURE_NOT_ASSIGNED: {
    message: "A frontend workflow closure requirement is not assigned to any executable task.",
    repairHint: "Repair the TaskPlan so a task references the required userFlow/interface refs, includes wire_reference_in_api_or_ui, carries frontendExperienceRequirement, and has automated_test or runtime_api_check verification evidence.",
  },
  MUST_ACCEPTANCE_REQUIRES_USER_DECISION: {
    message: "A must acceptance is not covered because the candidate declared a user decision category.",
    repairHint: "Route this candidate to user decision with the declared reason category.",
    repairability: "requires_user_decision",
  },
  READY_WITH_BLOCKING_ISSUES: {
    message: "Contract is marked ready while blocking validator issues exist.",
    repairHint: "Set the ready handoff flag to false or repair all blocking issues before marking ready.",
  },
  INVALID_DEPENDS_ON: {
    message: "Task dependency references an unknown task id.",
    repairHint: "Use an existing task id or remove the invalid dependency.",
  },
  DEPENDENCY_CYCLE: {
    message: "Task dependencies contain a cycle.",
    repairHint: "Remove or reorder dependencies so the task graph is acyclic.",
  },
  INVALID_FORBIDDEN_PATH: {
    message: "Forbidden path is not a safe project-relative path.",
    repairHint: "Use a safe project-relative forbidden path or remove the invalid path.",
  },
  INVALID_VERIFICATION_INTENT: {
    message: "Verification intent is structurally invalid or references invalid acceptance.",
    repairHint: "Fix the verification intent structure and use valid current phase acceptance refs.",
  },
  IMPLEMENTATION_INTENT_CONFLICT: {
    message: "Implementation action conflicts with the referenced entity implementation intent.",
    repairHint: "Remove the conflicting action or change the task artifact refs to match the intended scope.",
  },
  TASK_RESULT_STATUS_INCONSISTENT: {
    message: "TaskResult status is inconsistent with related result fields.",
    repairHint: "Repair only TaskResult contract fields and return a complete replacement TaskResult.",
  },
  TASK_RESULT_FAILURE_INVALID: {
    message: "TaskResult failure details are inconsistent with the result status.",
    repairHint: "Include failure details only for failed TaskResult outputs, and include them whenever status is failed.",
  },
  TASK_RESULT_SELF_REPAIR_INVALID: {
    message: "TaskResult selfRepairSummary is inconsistent with the result status or self-repair policy.",
    repairHint: "For failed TaskResult outputs, provide selfRepairSummary with either attempted=false/not_attempted or a valid attempted repair summary.",
  },
  TASK_RESULT_PATH_INVALID: {
    message: "TaskResult changedFiles contains an unsafe project-relative path.",
    repairHint: "Use safe project-relative changedFiles or remove invalid paths.",
  },
  TASK_RESULT_REF_INVALID: {
    message: "TaskResult references an unknown task, task plan, or verification intent.",
    repairHint: "Use refs from the current TaskExecutionRequest.",
  },
  TASK_RESULT_RUNTIME_CHECK_ID_INVALID: {
    message: "TaskResult runtimeDeliveryEvidence.codeLevelChecks contains a checkId that is not allowed by the current TaskExecutionRequest.",
    repairHint: "Use exactly the checkId values from outputContract.schemaShape.runtimeDeliveryEvidence.requiredCheckIds or task.runtimeDeliveryRequirement.requiredCodeLevelChecks[].checkId.",
  },
  TASK_RESULT_BLOCKED_MAPPING_INVALID: {
    message: "TaskResult blocked reason does not match the fixed blocked output mapping.",
    repairHint: "Use the fixed blocked output code, nextNode, detailsKey, and message from TaskExecutionRequest.",
  },
  TASK_RESULT_WORKFLOW_CLOSURE_INVALID: {
    message: "TaskResult frontend self-check claims a required workflow closure is satisfied without wired evidence.",
    repairHint: "Either repair the implementation/evidence so dataBinding.mode is wired and knownGaps is empty, or change frontendExperienceSelfCheck.status away from satisfied and route the remaining gap through the normal TaskResult status.",
  },
  AAC_COVERAGE_TYPE_MISMATCH: {
    message: "AAC acceptance coverage type does not match the referenced artifact kind.",
    repairHint: "Use the coverage type that matches the artifact id: state machine rules use state_rule, data constraints use data_constraint, modules use module, and so on.",
  },
  DETAIL_REF_INVALID: {
    message: "Requirement detail reference is not part of the PlanningGenerationContract requirementDetails index.",
    repairHint: "Use only detailId values from PGC requirementDetails.items.",
  },
  DETAIL_COVERAGE_INVALID: {
    message: "Requirement detail coverage is missing required artifact refs or reason.",
    repairHint: "Repair detailCoverage so covered items reference current AAC artifacts and non-covered items include a reason.",
  },
  REVIEW_RESULT_STATUS_INCONSISTENT: {
    message: "ReviewResult decision or routing is inconsistent with findings.",
    repairHint: "Repair only ReviewResult contract fields and return a complete replacement ReviewResult.",
  },
  REVIEW_RESULT_REF_INVALID: {
    message: "ReviewResult references an unknown task, acceptance, finding, or source.",
    repairHint: "Use refs from the current ReviewRequest.",
  },
  NEEDS_USER_DECISION_SHAPE_INVALID: {
    message: "User decision shape is incomplete.",
    repairHint: "Provide the required decision question, options, freeform setting, and impact fields.",
    repairability: "requires_user_decision",
  },
  BASELINE_CONFLICT_REQUIRES_USER_CONFIRMATION: {
    message: "TechnicalBaseline changes or conflicts with the previous stable baseline without entering the technical-baseline confirmation gate.",
    repairHint: "Do not silently continue after adding, replacing, or conflicting with previous baseline stack elements. Either reuse the previous TechnicalBaseline stack unchanged, or return status needs_user_confirmation with requiresUserConfirmation=true and approval.type=none so the user can confirm the technical baseline change.",
    repairability: "requires_user_decision",
  },
  GREENFIELD_BASELINE_CONFIRMATION_REQUIRED: {
    message: "Greenfield TechnicalBaseline must be the final user-confirmed technology baseline before planning can continue.",
    repairHint: "Do not submit intermediate recommendations. Complete the agent-user technology baseline confirmation first, then submit a candidate with status=confirmed, approval.type=user_confirmed, approval.confirmedAt, and requiresUserConfirmation=false or omitted.",
    repairability: "requires_user_decision",
  },
  GREENFIELD_BASELINE_TRACKS_INCOMPLETE: {
    message: "Greenfield TechnicalBaseline stack.tracks is missing required technology tracks.",
    repairHint: "Use stack.tracks with web, app, backend, persistence, dataAccess, and externalServices. Mark a track selected, not_needed, not_applicable, or user_custom, and include the final confirmed selection for each track.",
    repairability: "agent_repairable",
  },
};

let issueCounter = 0;

export function validateTechnicalBaselineCandidate(candidate: unknown, projectRoot: string): {
  value: TechnicalBaseline | null;
  issues: ContractIssue[];
} {
  const parsed = parseWithIssues(technicalBaselineSchema, normalizeOptionalEmptyFields(candidate, [
    "confirmedAt",
    "reason",
    "path",
    "requiresUserConfirmation",
    "reasoningSummary",
    "alternatives",
  ]));
  if (!parsed.value) {
    return parsed;
  }

  const issues = [...parsed.issues];
  const baseline = parsed.value;

  if (baseline.requiresUserConfirmation === true && baseline.status === "auto_accepted") {
    issues.push(issue("SOURCE_NOT_READY", "/requiresUserConfirmation", "blocked"));
  }

  for (const [index, evidence] of baseline.evidence.entries()) {
    if (evidence.path && !isSafeProjectRelativePath(evidence.path)) {
      issues.push(issue("INVALID_PATH", `/evidence/${index}/path`));
    }
    if (evidence.path && path.isAbsolute(evidence.path)) {
      issues.push(issue("INVALID_PATH", `/evidence/${index}/path`));
    }
    if (evidence.path && path.relative(projectRoot, path.resolve(projectRoot, evidence.path)).startsWith("..")) {
      issues.push(issue("INVALID_PATH", `/evidence/${index}/path`));
    }
  }

  return { value: baseline, issues };
}

export function validateTaskPlanCandidate(candidate: unknown, pgc: PlanningGenerationContract, aac: ArchitectureArtifactContract, baseline: TechnicalBaseline): {
  value: TaskPlan | null;
  issues: ContractIssue[];
  status: TaskPlan["status"];
} {
  const parsed = parseWithIssues(taskPlanSchema, normalizeOptionalEmptyFields(candidate, []));
  if (!parsed.value) {
    return { ...parsed, status: "needs_candidate_repair" };
  }

  const taskPlan = parsed.value;
  const issues = [...parsed.issues];
  const includedScopeIds = new Set(pgc.phaseScope.included.map((item) => item.scopeId));
  const excludedScopeIds = new Set(pgc.phaseScope.excluded.map((item) => item.scopeId));
  const deferredScopeIds = new Set(pgc.phaseScope.deferred.map((item) => item.scopeId));
  const acceptanceById = new Map(pgc.phaseScope.acceptanceCandidates.map((item) => [item.id, item]));
  const taskIds = new Set(taskPlan.tasks.map((task) => task.taskId));
  const groupIds = new Set(taskPlan.groups.map((group) => group.groupId));

  if (pgc.status !== "ready" || aac.status !== "ready" || !aac.handoff.readyForTaskPlan || !["auto_accepted", "confirmed"].includes(baseline.status)) {
    issues.push(issue("SOURCE_NOT_READY", "/source", "blocked"));
  }
  if (
    taskPlan.source.technicalBaselineId !== baseline.technicalBaselineId ||
    pgc.source.technicalBaselineId !== baseline.technicalBaselineId ||
    aac.source.technicalBaselineId !== baseline.technicalBaselineId
  ) {
    issues.push(issue("BASELINE_MISMATCH", "/source/technicalBaselineId", "blocked"));
  }
  if (taskPlan.source.planningGenerationContractId !== pgc.planningContractId) {
    issues.push(issue("SOURCE_NOT_READY", "/source/planningGenerationContractId", "blocked"));
  }
  if (taskPlan.source.architectureArtifactContractId !== aac.architectureArtifactContractId) {
    issues.push(issue("SOURCE_NOT_READY", "/source/architectureArtifactContractId", "blocked"));
  }

  for (const duplicate of duplicates(taskPlan.tasks.map((task) => task.taskId))) {
    issues.push(issue("DUPLICATE_ID", `/tasks/${duplicate}`));
  }
  for (const duplicate of duplicates(taskPlan.groups.map((group) => group.groupId))) {
    issues.push(issue("DUPLICATE_ID", `/groups/${duplicate}`));
  }
  for (const group of taskPlan.groups) {
    for (const dependency of group.dependsOn) {
      if (!groupIds.has(dependency)) {
        issues.push(issue("INVALID_DEPENDS_ON", `/groups/${group.groupId}/dependsOn/${dependency}`));
      }
    }
  }

  for (const ref of taskPlan.scopeSnapshot.includedScopeRefs) {
    if (!includedScopeIds.has(ref)) issues.push(issue("INVALID_SCOPE_REF", `/scopeSnapshot/includedScopeRefs/${ref}`));
  }
  for (const ref of taskPlan.scopeSnapshot.excludedScopeRefs) {
    if (!excludedScopeIds.has(ref)) issues.push(issue("INVALID_SCOPE_REF", `/scopeSnapshot/excludedScopeRefs/${ref}`));
  }
  for (const ref of taskPlan.scopeSnapshot.deferredScopeRefs) {
    if (!deferredScopeIds.has(ref)) issues.push(issue("INVALID_DEFERRED_REF", `/scopeSnapshot/deferredScopeRefs/${ref}`));
  }
  for (const ref of taskPlan.scopeSnapshot.acceptanceRefs) {
    if (!acceptanceById.has(ref)) issues.push(issue("INVALID_ACCEPTANCE_REF", `/scopeSnapshot/acceptanceRefs/${ref}`));
  }

  const entityById = new Map(aac.dataModel.entities.map((entity) => [entity.entityId, entity]));
  const artifactSets = getArtifactSets(aac);
  for (const task of taskPlan.tasks) {
    for (const dependency of task.dependsOn) {
      if (!taskIds.has(dependency)) {
        issues.push(issue("INVALID_DEPENDS_ON", `/tasks/${task.taskId}/dependsOn/${dependency}`));
      }
    }
    for (const ref of task.scopeRefs) {
      if (!includedScopeIds.has(ref)) issues.push(issue("INVALID_SCOPE_REF", `/tasks/${task.taskId}/scopeRefs/${ref}`));
    }
    for (const ref of task.acceptanceRefs) {
      if (!acceptanceById.has(ref)) issues.push(issue("INVALID_ACCEPTANCE_REF", `/tasks/${task.taskId}/acceptanceRefs/${ref}`));
    }
    for (const pathValue of task.writeBoundary.forbiddenPaths) {
      if (!isSafeProjectRelativePath(pathValue) && pathValue !== ".loom") {
        issues.push(issue("INVALID_FORBIDDEN_PATH", `/tasks/${task.taskId}/writeBoundary/forbiddenPaths/${pathValue}`));
      }
    }
    checkArtifactRefs(task.writeBoundary.artifactRefs.modules, artifactSets.moduleIds, `/tasks/${task.taskId}/writeBoundary/artifactRefs/modules`, issues);
    checkArtifactRefs(task.writeBoundary.artifactRefs.entities, artifactSets.entityIds, `/tasks/${task.taskId}/writeBoundary/artifactRefs/entities`, issues);
    checkArtifactRefs(task.writeBoundary.artifactRefs.interfaces, artifactSets.interfaceIds, `/tasks/${task.taskId}/writeBoundary/artifactRefs/interfaces`, issues);
    checkArtifactRefs(task.writeBoundary.artifactRefs.userFlows, artifactSets.userFlowIds, `/tasks/${task.taskId}/writeBoundary/artifactRefs/userFlows`, issues);
    checkArtifactRefs(task.writeBoundary.artifactRefs.stateMachines, artifactSets.stateMachineIds, `/tasks/${task.taskId}/writeBoundary/artifactRefs/stateMachines`, issues);
    checkArtifactRefs(task.writeBoundary.artifactRefs.decisions, artifactSets.decisionIds, `/tasks/${task.taskId}/writeBoundary/artifactRefs/decisions`, issues);
    checkArtifactRefs(task.writeBoundary.artifactRefs.risks, artifactSets.riskIds, `/tasks/${task.taskId}/writeBoundary/artifactRefs/risks`, issues);

    for (const intent of task.verificationIntents) {
      for (const ref of intent.acceptanceRefs) {
        if (!acceptanceById.has(ref) || !task.acceptanceRefs.includes(ref)) {
          issues.push(issue("INVALID_VERIFICATION_INTENT", `/tasks/${task.taskId}/verificationIntents/${intent.verificationId}/acceptanceRefs/${ref}`));
        }
      }
      if (intent.acceptableEvidence.length === 0) {
        issues.push(issue("INVALID_VERIFICATION_INTENT", `/tasks/${task.taskId}/verificationIntents/${intent.verificationId}/acceptableEvidence`));
      }
    }

    const conceptRefs = task.conceptRefs ?? [];
    if (conceptRefs.length > 0) {
      const responsibilities = task.conceptResponsibilities ?? [];
      const responsibilityRefs = new Set(responsibilities.map((item) => item.conceptRef));
      for (const conceptRef of conceptRefs) {
        if (!responsibilityRefs.has(conceptRef)) {
          issues.push(issue("TASK_CONCEPT_RESPONSIBILITY_MISSING", `/tasks/${task.taskId}/conceptResponsibilities/${conceptRef}`));
        }
      }
      for (const responsibility of responsibilities) {
        if (!conceptRefs.includes(responsibility.conceptRef)) {
          issues.push(issue("TASK_CONCEPT_REF_INVALID", `/tasks/${task.taskId}/conceptResponsibilities/${responsibility.conceptRef}`));
        }
      }
      for (const intent of task.conceptVerificationIntents ?? []) {
        if (!conceptRefs.includes(intent.conceptRef)) {
          issues.push(issue("TASK_CONCEPT_REF_INVALID", `/tasks/${task.taskId}/conceptVerificationIntents/${intent.conceptRef}`));
        }
      }
    }

    if (task.runtimeDeliveryRequirement) {
      validateRuntimeDeliveryRequirement(task.taskId, task.runtimeDeliveryRequirement, issues);
    }

    const forbiddenReferenceActions = new Set([
      "create_entity_crud",
      "create_entity_repository",
      "create_entity_admin_page",
      "create_entity_migration",
      "implement_entity_lifecycle",
    ]);
    const referencesReferenceOnly = task.writeBoundary.artifactRefs.entities.some((entityRef) => {
      const entity = entityById.get(entityRef);
      return entity?.implementationIntent === "reference_only";
    });
    if (referencesReferenceOnly && task.implementationActions.some((action) => forbiddenReferenceActions.has(action))) {
      issues.push(issue("IMPLEMENTATION_INTENT_CONFLICT", `/tasks/${task.taskId}/implementationActions`));
    }
  }

  if (hasDependencyCycle(taskPlan)) {
    issues.push(issue("DEPENDENCY_CYCLE", "/tasks/dependsOn"));
  }

  for (const acceptance of pgc.phaseScope.acceptanceCandidates.filter((item) => item.priority === "must")) {
    if (!taskPlan.tasks.some((task) => task.acceptanceRefs.includes(acceptance.id))) {
      issues.push(issue("MUST_ACCEPTANCE_NOT_COVERED", `/tasks/acceptanceRefs/${acceptance.id}`));
    }
  }

  if (aac.frontendExperience?.required) {
    const hasFrontendTask = taskPlan.tasks.some((task) =>
      task.taskKind === "frontend_experience" ||
      task.taskKind === "ui_flow_increment" ||
      Boolean(task.frontendExperienceRequirement)
    );
    if (!hasFrontendTask) {
      issues.push(issue("MUST_ACCEPTANCE_NOT_COVERED", "/tasks/frontendExperienceRequirement", "blocked"));
    }
  }
  validateWorkflowClosureTaskAssignments(taskPlan, aac, issues);

  if (aac.runtimeDelivery?.status === "modified") {
    validateRuntimeDeliveryClosureTask(taskPlan, aac.runtimeDelivery, issues);
  }

  if (taskPlan.status === "ready" && taskPlan.handoff.readyForExecution && issues.length > 0) {
    issues.push(issue("READY_WITH_BLOCKING_ISSUES", "/handoff/readyForExecution"));
  }

  const status = issues.some((item) => item.repairability === "blocked")
    ? "blocked"
    : issues.length > 0
      ? "needs_candidate_repair"
      : "ready";
  return { value: taskPlan, issues, status };
}

function validateWorkflowClosureTaskAssignments(
  taskPlan: TaskPlan,
  aac: ArchitectureArtifactContract,
  issues: ContractIssue[],
): void {
  for (const requirement of buildWorkflowClosureRequirements(aac)) {
    if (taskPlan.tasks.some((task) => taskCoversWorkflowClosure(task, requirement))) {
      continue;
    }
    issues.push(issue("WORKFLOW_CLOSURE_NOT_ASSIGNED", `/tasks/workflowClosureRequirements/${requirement.closureId}`));
  }
}

function validateRuntimeDeliveryClosureTask(
  taskPlan: TaskPlan,
  runtimeDelivery: NonNullable<ArchitectureArtifactContract["runtimeDelivery"]>,
  issues: ContractIssue[],
): void {
  const closureTasks = taskPlan.tasks.filter((task) => task.taskKind === "runtime_delivery_closure");
  if (closureTasks.length !== 1) {
    issues.push(issue("MUST_ACCEPTANCE_NOT_COVERED", "/tasks/runtimeDeliveryClosure", "blocked"));
    return;
  }
  const closure = closureTasks[0];
  const requirement = closure.runtimeDeliveryRequirement;
  if (!requirement?.appliesToThisTask) {
    issues.push(issue("SOURCE_NOT_READY", `/tasks/${closure.taskId}/runtimeDeliveryRequirement`, "blocked"));
    return;
  }
  const closureContract = runtimeDeliveryClosureRequirementContractForRuntime(runtimeDelivery);
  const requiredFields = closureContract.requiredContractFields;
  const requiredChecks = closureContract.requiredCodeLevelChecks;
  const affected = new Set(requirement.affectedContractFields ?? []);
  for (const field of requiredFields) {
    if (!affected.has(field)) {
      issues.push(issue("SOURCE_NOT_READY", `/tasks/${closure.taskId}/runtimeDeliveryRequirement/affectedContractFields/${field}`, "blocked"));
    }
  }
  const requiredFieldSet = new Set(requiredFields);
  for (const field of requirement.affectedContractFields ?? []) {
    if (!requiredFieldSet.has(field)) {
      issues.push(issue("SOURCE_NOT_READY", `/tasks/${closure.taskId}/runtimeDeliveryRequirement/affectedContractFields/${field}`, "blocked"));
    }
  }
  const checkFields = new Set((requirement.requiredCodeLevelChecks ?? []).map((check) => check.contractField));
  const checkIdsByField = new Map((requirement.requiredCodeLevelChecks ?? []).map((check) => [check.contractField, check.checkId]));
  for (const field of requiredFields) {
    if (!checkFields.has(field)) {
      issues.push(issue("SOURCE_NOT_READY", `/tasks/${closure.taskId}/runtimeDeliveryRequirement/requiredCodeLevelChecks/${field}`, "blocked"));
    }
  }
  for (const requiredCheck of requiredChecks) {
    if (checkIdsByField.get(requiredCheck.contractField) !== requiredCheck.checkId) {
      issues.push(issue("SOURCE_NOT_READY", `/tasks/${closure.taskId}/runtimeDeliveryRequirement/requiredCodeLevelChecks/${requiredCheck.checkId}`, "blocked"));
    }
  }
  const requiredCheckIdSet = new Set(requiredChecks.map((check) => check.checkId));
  for (const check of requirement.requiredCodeLevelChecks ?? []) {
    if (!requiredFieldSet.has(check.contractField) || !requiredCheckIdSet.has(check.checkId)) {
      issues.push(issue("SOURCE_NOT_READY", `/tasks/${closure.taskId}/runtimeDeliveryRequirement/requiredCodeLevelChecks/${check.checkId}`, "blocked"));
    }
  }
  const runtimeTaskIds = taskPlan.tasks
    .filter((task) => task.taskId !== closure.taskId && task.runtimeDeliveryRequirement?.appliesToThisTask === true)
    .map((task) => ({ taskId: task.taskId, groupId: task.groupId }));
  const closureGroupId = closure.groupId;
  const closureGroup = taskPlan.groups.find((group) => group.groupId === closureGroupId);
  if (!closureGroup) {
    issues.push(issue("INVALID_DEPENDS_ON", `/groups/${closureGroupId}`, "blocked"));
    return;
  }
  if (closureGroup.taskIds.length !== 1 || closureGroup.taskIds[0] !== closure.taskId) {
    issues.push(issue("INVALID_DEPENDS_ON", `/groups/${closureGroupId}/taskIds`, "blocked"));
  }
  const lastGroup = taskPlan.groups[taskPlan.groups.length - 1];
  if (lastGroup?.groupId !== closureGroupId) {
    issues.push(issue("INVALID_DEPENDS_ON", `/groups/${closureGroupId}/position`, "blocked"));
  }
  const groupsDependingOnClosure = taskPlan.groups.filter((group) => group.dependsOn.includes(closureGroupId));
  for (const group of groupsDependingOnClosure) {
    issues.push(issue("INVALID_DEPENDS_ON", `/groups/${group.groupId}/dependsOn/${closureGroupId}`, "blocked"));
  }
  const taskGroupById = new Map(taskPlan.tasks.map((task) => [task.taskId, task.groupId]));
  for (const dependency of closure.dependsOn) {
    const dependencyGroupId = taskGroupById.get(dependency);
    if (dependencyGroupId && dependencyGroupId !== closureGroupId) {
      issues.push(issue("INVALID_DEPENDS_ON", `/tasks/${closure.taskId}/dependsOn/${dependency}`, "blocked"));
    }
  }
  const closureGroupDependencies = closureGroup ? transitiveGroupDependencies(taskPlan, closureGroup.groupId) : new Set<string>();
  for (const runtimeTask of runtimeTaskIds) {
    if (runtimeTask.groupId === closureGroupId) {
      if (!closure.dependsOn.includes(runtimeTask.taskId)) {
        issues.push(issue("INVALID_DEPENDS_ON", `/tasks/${closure.taskId}/dependsOn/${runtimeTask.taskId}`, "blocked"));
      }
    } else if (!closureGroupDependencies.has(runtimeTask.groupId)) {
      issues.push(issue("INVALID_DEPENDS_ON", `/groups/${closureGroupId}/dependsOn/${runtimeTask.groupId}`, "blocked"));
    }
  }
}

function transitiveGroupDependencies(taskPlan: TaskPlan, groupId: string): Set<string> {
  const groupById = new Map(taskPlan.groups.map((group) => [group.groupId, group]));
  const visited = new Set<string>();
  const visit = (candidateId: string): void => {
    const group = groupById.get(candidateId);
    if (!group) return;
    for (const dependency of group.dependsOn) {
      if (visited.has(dependency)) continue;
      visited.add(dependency);
      visit(dependency);
    }
  };
  visit(groupId);
  return visited;
}

function validateRuntimeDeliveryRequirement(
  taskId: string,
  requirement: TaskPlan["tasks"][number]["runtimeDeliveryRequirement"],
  issues: ContractIssue[],
): void {
  if (!requirement) return;
  const basePath = `/tasks/${taskId}/runtimeDeliveryRequirement`;
  if (requirement.appliesToThisTask) {
    if (!requirement.runtimeDeliveryRef) {
      issues.push(issue("SOURCE_NOT_READY", `${basePath}/runtimeDeliveryRef`, "blocked"));
    }
    if (!requirement.affectedContractFields || requirement.affectedContractFields.length === 0) {
      issues.push(issue("SOURCE_NOT_READY", `${basePath}/affectedContractFields`, "blocked"));
    }
    if (!requirement.requiredCodeLevelChecks || requirement.requiredCodeLevelChecks.length === 0) {
      issues.push(issue("SOURCE_NOT_READY", `${basePath}/requiredCodeLevelChecks`, "blocked"));
    }
    if (!requirement.evidenceExpectedInTaskResult || requirement.evidenceExpectedInTaskResult.length === 0) {
      issues.push(issue("SOURCE_NOT_READY", `${basePath}/evidenceExpectedInTaskResult`, "blocked"));
    }
    if (!requirement.forbiddenActions || requirement.forbiddenActions.length === 0) {
      issues.push(issue("SOURCE_NOT_READY", `${basePath}/forbiddenActions`, "blocked"));
    }
    const forbiddenText = [
      ...(requirement.requiredCodeLevelChecks ?? []).map((check) => `${check.objective} ${check.contractField}`),
      ...(requirement.evidenceExpectedInTaskResult ?? []),
      ...(requirement.forbiddenActions ?? []),
    ].join(" ").toLowerCase();
    if (
      forbiddenText.includes("require clean install") ||
      forbiddenText.includes("required clean install") ||
      forbiddenText.includes("require container") ||
      forbiddenText.includes("required container") ||
      forbiddenText.includes("must run docker") ||
      forbiddenText.includes("must run deploy")
    ) {
      issues.push(issue("SOURCE_NOT_READY", `${basePath}/verificationBoundary`, "blocked"));
    }
  } else if (!requirement.reason) {
    issues.push(issue("SOURCE_NOT_READY", `${basePath}/reason`, "blocked"));
  }
}

export function validateTaskResult(candidate: unknown, request: TaskExecutionRequest): {
  value: TaskResult | null;
  issues: ContractIssue[];
} {
  const parsed = parseWithIssues(taskResultSchema, normalizeOptionalEmptyFields(candidate, ["evidenceType"]));
  if (!parsed.value) {
    return parsed;
  }

  const rawResult = parsed.value;
  const result = normalizeTaskResultVerificationResults(taskResultSchema.parse({
    ...rawResult,
    changedFiles: filterReviewChangedFiles(rawResult.changedFiles),
  }), request);
  const issues = [...parsed.issues];
  const verificationIds = new Set(request.task.verificationIntents.map((intent) => intent.verificationId));
  const acceptableEvidence = new Map(request.task.verificationIntents.map((intent) => [intent.verificationId, new Set(intent.acceptableEvidence)]));

  if (result.taskId !== request.source.taskId || result.taskPlanId !== request.source.taskPlanId) {
    issues.push(issue("TASK_RESULT_REF_INVALID", "/taskId"));
  }
  for (const file of rawResult.changedFiles) {
    if (!isSafeProjectRelativePath(file)) {
      issues.push(issue("TASK_RESULT_PATH_INVALID", `/changedFiles/${file}`));
    }
  }
  if ((result.status === "completed" || result.status === "completed_with_notes") && result.changedFiles.length === 0) {
    const allowedEmptyChange =
      request.task.taskKind === "verification_increment" ||
      result.noChangeReason?.code === "NO_CODE_CHANGE_REQUIRED" ||
      result.noChangeReason?.code === "VERIFICATION_ONLY_TASK" ||
      result.noChangeReason?.code === "ENVIRONMENT_CHECK_ONLY";
    if (!allowedEmptyChange) {
      issues.push(issue("TASK_RESULT_STATUS_INCONSISTENT", "/changedFiles"));
    }
  }
  if (result.status !== "failed" && result.failure !== null) {
    issues.push(issue("TASK_RESULT_FAILURE_INVALID", "/failure"));
  }
  if (result.status === "failed" && result.failure === null) {
    issues.push(issue("TASK_RESULT_FAILURE_INVALID", "/failure"));
  }
  if (result.status === "failed" && result.selfRepairSummary === null) {
    issues.push(issue("TASK_RESULT_SELF_REPAIR_INVALID", "/selfRepairSummary"));
  }
  if (result.selfRepairSummary?.attempted === false && (result.selfRepairSummary.attemptCount !== 0 || result.selfRepairSummary.stopReason !== "not_attempted" || result.selfRepairSummary.progressObserved !== false)) {
    issues.push(issue("TASK_RESULT_SELF_REPAIR_INVALID", "/selfRepairSummary"));
  }
  if (result.selfRepairSummary?.attempted === true && (result.selfRepairSummary.attemptCount <= 0 || result.selfRepairSummary.attemptCount > 8 || result.selfRepairSummary.stopReason === "not_attempted")) {
    issues.push(issue("TASK_RESULT_SELF_REPAIR_INVALID", "/selfRepairSummary"));
  }

  for (const verification of result.verificationResults) {
    if (!verificationIds.has(verification.verificationId)) {
      issues.push(issue("TASK_RESULT_REF_INVALID", `/verificationResults/${verification.verificationId}`));
    }
    if (verification.evidenceType && !(acceptableEvidence.get(verification.verificationId)?.has(verification.evidenceType))) {
      issues.push(issue("INVALID_VERIFICATION_INTENT", `/verificationResults/${verification.verificationId}/evidenceType`));
    }
  }
  if (result.status === "completed") {
    for (const verificationId of verificationIds) {
      const verification = result.verificationResults.find((item) => item.verificationId === verificationId);
      if (!verification || verification.status !== "passed") {
        issues.push(issue("TASK_RESULT_STATUS_INCONSISTENT", `/verificationResults/${verificationId}`));
      }
    }
  }
  if (result.status === "completed_with_notes" && result.notes.length === 0 && result.verificationResults.some((item) => item.status === "not_run" || item.status === "inconclusive")) {
    issues.push(issue("TASK_RESULT_STATUS_INCONSISTENT", "/notes"));
  }
  if (result.status !== "failed" && result.verificationResults.some((item) => item.status === "failed")) {
    issues.push(issue("TASK_RESULT_STATUS_INCONSISTENT", "/verificationResults"));
  }
  validateExecutionContinuity(result, issues);
  validateRuntimeDeliveryEvidence(result, request, issues);
  validateConceptEvidence(result, request, issues);
  validateWorkflowClosureTaskResult(result, request, issues);
  if (result.status === "blocked") {
    if (result.blockedReasons.length === 0) {
      issues.push(issue("TASK_RESULT_BLOCKED_MAPPING_INVALID", "/blockedReasons"));
    }
    for (const blocked of result.blockedReasons) {
      const expected = BLOCKED_OUTPUT_MAPPING[blocked.code];
      if (!expected || blocked.nextNode !== expected.nextNode || blocked.message !== expected.message || !(expected.detailsKey in blocked.details)) {
        issues.push(issue("TASK_RESULT_BLOCKED_MAPPING_INVALID", `/blockedReasons/${blocked.code}`));
      }
    }
  } else if (result.blockedReasons.length > 0) {
    issues.push(issue("TASK_RESULT_BLOCKED_MAPPING_INVALID", "/blockedReasons"));
  }

  return { value: result, issues };
}

function validateWorkflowClosureTaskResult(result: TaskResult, request: TaskExecutionRequest, issues: ContractIssue[]): void {
  const requirementIds = workflowClosureRequirementIdsFromTaskRequest(request);
  const violation = frontendSelfCheckViolatesRequiredClosure(result, requirementIds);
  if (violation.violates) {
    issues.push(issue("TASK_RESULT_WORKFLOW_CLOSURE_INVALID", "/frontendExperienceSelfCheck/dataBinding/mode"));
  }
}

function workflowClosureRequirementIdsFromTaskRequest(request: TaskExecutionRequest): string[] {
  const frontendRequirement = request.task.frontendExperienceRequirement;
  if (!frontendRequirement || typeof frontendRequirement !== "object" || Array.isArray(frontendRequirement)) {
    return [];
  }
  const guidance = (frontendRequirement as Record<string, unknown>).executionGuidance;
  if (!guidance || typeof guidance !== "object" || Array.isArray(guidance)) {
    return [];
  }
  const closureRefs = (guidance as Record<string, unknown>).closureRequirementRefs;
  if (Array.isArray(closureRefs)) {
    return closureRefs
      .map((item) => {
        if (typeof item === "string") return item;
        if (typeof item === "object" && item !== null && !Array.isArray(item)) {
          const closureId = (item as Record<string, unknown>).closureId;
          return typeof closureId === "string" ? closureId : null;
        }
        return null;
      })
      .filter((item): item is string => typeof item === "string" && item.length > 0);
  }
  const requirements = (guidance as Record<string, unknown>).workflowClosureRequirements;
  if (!Array.isArray(requirements)) return [];
  return requirements
    .map((item) => typeof item === "object" && item !== null && !Array.isArray(item) ? (item as Record<string, unknown>).closureId : null)
    .filter((item): item is string => typeof item === "string" && item.length > 0);
}

function validateExecutionContinuity(result: TaskResult, issues: ContractIssue[]): void {
  if (!result.executionContinuity.taskResultSubmittedAfterVerification) {
    issues.push(issue("TASK_RESULT_STATUS_INCONSISTENT", "/executionContinuity/taskResultSubmittedAfterVerification"));
  }
  if (result.executionContinuity.agentOwnedLongRunningWork === "unknown") {
    if (result.status === "completed") {
      issues.push(issue("TASK_RESULT_STATUS_INCONSISTENT", "/executionContinuity/agentOwnedLongRunningWork"));
    }
    if (result.notes.length === 0 && result.executionContinuity.notes.length === 0) {
      issues.push(issue("TASK_RESULT_STATUS_INCONSISTENT", "/executionContinuity/notes"));
    }
  }
}

function normalizeTaskResultVerificationResults(result: TaskResult, request: TaskExecutionRequest): TaskResult {
  const intents = request.task.verificationIntents;
  if (intents.length !== 1) {
    return result;
  }

  const [intent] = intents;
  if (result.verificationResults.some((verification) => verification.verificationId === intent.verificationId)) {
    return result;
  }
  if (result.verificationResults.length === 0) {
    return result;
  }

  const acceptableEvidence = new Set(intent.acceptableEvidence);
  const statuses = new Set(result.verificationResults.map((verification) => verification.status));
  const evidenceTypes = result.verificationResults
    .map((verification) => verification.evidenceType)
    .filter((evidenceType): evidenceType is NonNullable<typeof evidenceType> => typeof evidenceType === "string" && evidenceType.length > 0);
  const evidenceType = evidenceTypes.find((candidate) => acceptableEvidence.has(candidate)) ?? intent.acceptableEvidence[0] ?? null;
  const normalizedStatus =
    statuses.has("failed") ? "failed" :
    statuses.has("inconclusive") ? "inconclusive" :
    statuses.has("not_run") ? "not_run" :
    "passed";
  const summary = result.verificationResults
    .map((verification) => `${verification.verificationId}: ${verification.summary}`)
    .join(" ");

  return {
    ...result,
    verificationResults: [{
      verificationId: intent.verificationId,
      status: normalizedStatus,
      ...(evidenceType ? { evidenceType } : {}),
      summary: summary || `Verification evidence was normalized to ${intent.verificationId}.`,
    }],
  };
}

function validateConceptEvidence(result: TaskResult, request: TaskExecutionRequest, issues: ContractIssue[]): void {
  const conceptRefs = request.task.conceptRefs ?? [];
  if (conceptRefs.length === 0) {
    return;
  }
  const evidenceRefs = new Set((result.conceptEvidence ?? []).map((item) => item.conceptRef));
  for (const conceptRef of conceptRefs) {
    if (!evidenceRefs.has(conceptRef)) {
      issues.push(issue("TASK_RESULT_REF_INVALID", `/conceptEvidence/${conceptRef}`));
    }
  }
  const allowedConceptRefs = new Set(conceptRefs);
  for (const evidence of result.conceptEvidence ?? []) {
    if (!allowedConceptRefs.has(evidence.conceptRef)) {
      issues.push(issue("TASK_RESULT_REF_INVALID", `/conceptEvidence/${evidence.conceptRef}`));
    }
  }
}

function validateRuntimeDeliveryEvidence(result: TaskResult, request: TaskExecutionRequest, issues: ContractIssue[]): void {
  const requirement = request.task.runtimeDeliveryRequirement;
  if (!requirement?.appliesToThisTask) {
    return;
  }
  if (!result.runtimeDeliveryEvidence) {
    issues.push(issue("TASK_RESULT_REF_INVALID", "/runtimeDeliveryEvidence"));
    return;
  }
  const checkedFields = new Set(result.runtimeDeliveryEvidence.checkedFields ?? []);
  for (const field of requirement.affectedContractFields ?? []) {
    if (!checkedFields.has(field)) {
      issues.push(issue("TASK_RESULT_REF_INVALID", `/runtimeDeliveryEvidence/checkedFields/${field}`));
    }
  }
  const resultCheckIds = new Set((result.runtimeDeliveryEvidence.codeLevelChecks ?? []).map((check) => check.checkId));
  const allowedCheckIds = new Set((requirement.requiredCodeLevelChecks ?? []).map((check) => check.checkId));
  for (const check of result.runtimeDeliveryEvidence.codeLevelChecks ?? []) {
    if (!allowedCheckIds.has(check.checkId)) {
      issues.push(issue("TASK_RESULT_RUNTIME_CHECK_ID_INVALID", `/runtimeDeliveryEvidence/codeLevelChecks/${check.checkId}`));
    }
  }
  for (const check of requirement.requiredCodeLevelChecks ?? []) {
    if (!resultCheckIds.has(check.checkId)) {
      issues.push(issue("TASK_RESULT_RUNTIME_CHECK_ID_INVALID", `/runtimeDeliveryEvidence/codeLevelChecks/${check.checkId}`));
    }
  }
}

export function filterReviewChangedFiles(files: string[]): string[] {
  const filtered: string[] = [];
  const seen = new Set<string>();
  for (const file of files) {
    const normalized = normalizeProjectPath(file);
    if (isWorkspaceSideEffectPath(normalized) || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    filtered.push(normalized);
  }
  return filtered;
}

function normalizeProjectPath(value: string): string {
  return value.split("\\").join("/").replace(/^\.\/+/, "");
}

function isWorkspaceSideEffectPath(value: string): boolean {
  const pathValue = normalizeProjectPath(value);
  const segments = pathValue.split("/");
  const fileName = segments.at(-1) ?? pathValue;
  const first = segments[0] ?? "";

  if (pathValue === ".DS_Store" || fileName === ".DS_Store") {
    return true;
  }
  if (pathValue === "lcov.info" || /^junit.*\.xml$/i.test(fileName) || /^coverage.*\.xml$/i.test(fileName)) {
    return true;
  }
  if (fileName.endsWith(".log")) {
    return true;
  }

  const anySegmentBlacklist = new Set([
    "node_modules",
    "bower_components",
    ".cache",
    ".gradle",
    ".m2",
    ".ivy2",
    ".npm",
    ".pnpm-store",
  ]);

  if (segments.some((segment) => anySegmentBlacklist.has(segment))) {
    return true;
  }

  const topLevelOutputBlacklist = new Set([
    "dist",
    "build",
    "out",
    "target",
    "bin",
    "obj",
    "coverage",
    ".nyc_output",
    "playwright-report",
    "test-results",
    "logs",
    "tmp",
    "temp",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".angular",
    ".vite",
    ".parcel-cache",
    "parcel-cache",
    ".turbo",
    ".vercel",
    ".netlify",
  ]);

  if (topLevelOutputBlacklist.has(first)) {
    return true;
  }
  if (first === ".yarn" && ["cache", "unplugged"].includes(segments[1] ?? "")) {
    return true;
  }
  if (pathValue === ".yarn/build-state.yml" || pathValue === ".yarn/install-state.gz") {
    return true;
  }
  if (pathValue.startsWith("vendor/bundle/") || pathValue.startsWith("Pods/") || pathValue.startsWith("Carthage/Build/")) {
    return true;
  }

  return false;
}

export function validateReviewResult(candidate: unknown, request: ReviewRequest): {
  value: ReviewResult | null;
  issues: ContractIssue[];
} {
  const parsed = parseWithIssues(reviewResultSchema, normalizeOptionalEmptyFields(candidate, [
    "evidenceRefs",
    "groupRefs",
    "targetNode",
    "targetPhaseId",
    "targetTaskIds",
    "findingRefs",
    "userVisibleState",
  ]));
  if (!parsed.value) {
    return parsed;
  }

  const result = parsed.value;
  const issues = [...parsed.issues];
  const taskIds = new Set(extractAllowedRefs(request, "taskIds"));
  const groupIds = new Set(extractAllowedRefs(request, "groupIds"));
  const acceptanceRefs = new Set(request.reviewScope.acceptanceRefs);
  const allowedReadRefs = new Set(extractAllowedRefs(request, "readRefs"));
  const changedFileRefs = changedFileRefSet(request);
  const verificationEvidenceRefs = verificationEvidenceRefSet(request);
  const readRefTypes = new Set(request.enumRefs?.readRefType ?? ["review_packet", "change_context", "diff_ref", "changed_file", "task_result", "verification_evidence"]);
  const evidenceRefTypes = new Set(request.enumRefs?.evidenceRefType ?? ["task_result", "verification_result", "diff_ref", "changed_file", "manual_note"]);
  const findingIds = new Set(result.findings.map((finding) => finding.findingId));
  const actionByFinding = new Map(result.findings.map((finding) => [finding.findingId, finding.recommendedNextAction]));

  if (
    result.source.requestId !== request.requestId ||
    result.source.phaseId !== request.source.phaseId ||
    result.source.taskPlanId !== request.source.taskPlanId ||
    result.source.taskPlanRunId !== request.source.taskPlanRunId
  ) {
    issues.push(issue("REVIEW_RESULT_REF_INVALID", "/source"));
  }

  for (const finding of result.findings) {
    if (finding.failureClass === "environment_blocker" && finding.recommendedNextAction === "execution_repair") {
      const hasProductDefect = result.findings.some((item) => item.findingId !== finding.findingId && item.failureClass === "product_defect");
      if (!hasProductDefect) {
        issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", `/findings/${finding.findingId}/failureClass`));
      }
    }
    if (finding.severityClass === "blocking") {
      const hasActionableRef =
        finding.taskRefs.length > 0 ||
        (finding.groupRefs?.length ?? 0) > 0 ||
        Object.values(finding.artifactRefs ?? {}).some((refs) => Array.isArray(refs) && refs.length > 0) ||
        Boolean(finding.location?.file);
      if (!hasActionableRef && !["manual_review", "needs_user_decision"].includes(finding.recommendedNextAction)) {
        issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/refs`));
      }
    }
    for (const taskRef of finding.taskRefs) {
      if (!taskIds.has(taskRef)) issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/taskRefs/${taskRef}`));
    }
    for (const groupRef of finding.groupRefs ?? []) {
      if (!groupIds.has(groupRef)) issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/groupRefs/${groupRef}`));
    }
    for (const acceptanceRef of finding.acceptanceRefs) {
      if (!acceptanceRefs.has(acceptanceRef)) issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/acceptanceRefs/${acceptanceRef}`));
    }
    for (const readRef of finding.readRefs) {
      if (!readRefTypes.has(readRef.type)) {
        issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/readRefs/${readRef.type}`));
      }
      if (!isAllowedReviewReadRef(readRef, allowedReadRefs, changedFileRefs, verificationEvidenceRefs)) {
        issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/readRefs/${readRef.ref}`));
      }
    }
    for (const evidenceRef of finding.evidenceRefs ?? []) {
      if (!evidenceRefTypes.has(evidenceRef.type)) {
        issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/evidenceRefs/${evidenceRef.type}`));
      } else if (!isAllowedReviewEvidenceRef(evidenceRef, allowedReadRefs, changedFileRefs, verificationEvidenceRefs)) {
        issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/evidenceRefs/${evidenceRef.ref}`));
      }
    }
    if (finding.taskRefs.length > 0 && taskIds.size === 0) {
      issues.push(issue("REVIEW_RESULT_REF_INVALID", `/findings/${finding.findingId}/taskRefs`));
    }
    if (changeSetMode(request) === "current_file_content") {
      const canImpact = finding.taskRelevance === "direct" && finding.scopeRelation === "within_task_changed_files";
      if (!canImpact && (finding.severity === "critical" || finding.severity === "major")) {
        issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", `/findings/${finding.findingId}/severity`));
      }
    }
  }

  for (const assessment of result.coverageAssessment.mustAcceptance) {
    if (!acceptanceRefs.has(assessment.acceptanceRef)) {
      issues.push(issue("REVIEW_RESULT_REF_INVALID", `/coverageAssessment/mustAcceptance/${assessment.acceptanceRef}`));
    }
    for (const taskResultRef of assessment.supportingTaskResults) {
      if (!extractAllowedRefs(request, "taskResultIds").includes(taskResultRef)) {
        issues.push(issue("REVIEW_RESULT_REF_INVALID", `/coverageAssessment/mustAcceptance/${assessment.acceptanceRef}/supportingTaskResults/${taskResultRef}`));
      }
    }
    if (assessment.status !== "satisfied") {
      const hasFinding = result.findings.some((finding) => finding.acceptanceRefs.includes(assessment.acceptanceRef));
      if (!hasFinding) {
        issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", `/coverageAssessment/mustAcceptance/${assessment.acceptanceRef}`));
      }
    }
  }

  const mustAcceptanceRefs = acceptanceRefs;
  for (const acceptanceRef of mustAcceptanceRefs) {
    if (!result.coverageAssessment.mustAcceptance.some((item) => item.acceptanceRef === acceptanceRef)) {
      issues.push(issue("REVIEW_RESULT_REF_INVALID", `/coverageAssessment/mustAcceptance/${acceptanceRef}`));
    }
  }

  for (const action of result.pendingActions) {
    for (const findingRef of action.findingRefs) {
      if (!findingIds.has(findingRef)) {
        issues.push(issue("REVIEW_RESULT_REF_INVALID", `/pendingActions/${action.type}/findingRefs/${findingRef}`));
      } else if (actionByFinding.get(findingRef) !== action.type) {
        issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", `/pendingActions/${action.type}/findingRefs/${findingRef}`));
      }
    }
    if (action.type === result.nextAction.type) {
      issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", `/pendingActions/${action.type}`));
    }
  }
  for (const findingRef of result.nextAction.findingRefs ?? []) {
    if (!findingIds.has(findingRef)) {
      issues.push(issue("REVIEW_RESULT_REF_INVALID", `/nextAction/findingRefs/${findingRef}`));
    }
  }

  if (result.decision === "approved" && (result.findings.some((finding) => isBlockingFinding(finding)) || result.pendingActions.length > 0)) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/decision"));
  }
  if (result.decision === "approved_with_notes" && (result.findings.some((finding) => isBlockingFinding(finding)) || result.pendingActions.length > 0)) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/decision"));
  }
  if (result.decision === "changes_requested" && !result.findings.some((finding) => finding.recommendedNextAction === "execution_repair" && isBlockingFinding(finding))) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/decision"));
  }
  if (result.decision === "blocked" && !result.findings.some((finding) => ["architecture_artifact_repair", "taskplan_repair", "manual_review"].includes(finding.recommendedNextAction) && isBlockingFinding(finding))) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/decision"));
  }
  if (result.decision === "needs_user_decision" && !result.findings.some((finding) => finding.recommendedNextAction === "needs_user_decision" && isBlockingFinding(finding))) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/decision"));
  }
  if ((result.decision === "approved" || result.decision === "approved_with_notes") && hasUnsatisfiedWorkflowClosureSignal(request)) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/decision"));
  }

  const expectedTopAction = expectedReviewTopAction(result, request);
  if (result.nextAction.type !== expectedTopAction) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/nextAction/type"));
  }
  if (
    result.findings.length > 0 &&
    result.findings.every((finding) => finding.severityClass === "warning") &&
    ["execution_repair", "taskplan_repair", "architecture_artifact_repair", "manual_review", "needs_user_decision"].includes(result.nextAction.type)
  ) {
    issues.push(issue("REVIEW_RESULT_STATUS_INCONSISTENT", "/nextAction/type"));
  }

  return { value: result, issues };
}

function hasUnsatisfiedWorkflowClosureSignal(request: ReviewRequest): boolean {
  const signals = request.outputContract.reviewSignals;
  if (!Array.isArray(signals)) {
    return false;
  }
  return signals.some((signal) => {
    if (!signal || typeof signal !== "object" || Array.isArray(signal)) {
      return false;
    }
    const record = signal as Record<string, unknown>;
    return record.kind === "frontend_workflow_closure" && record.closureSatisfied === false;
  });
}

export function validatePlanningGenerationContract(contract: unknown, baseline: TechnicalBaseline | null): {
  value: PlanningGenerationContract | null;
  issues: ContractIssue[];
} {
  const parsed = parseWithIssues(planningGenerationContractSchema, contract);
  if (!parsed.value) {
    return parsed;
  }

  const pgc = parsed.value;
  const issues = [...parsed.issues];

  if (!baseline || !["auto_accepted", "confirmed"].includes(baseline.status)) {
    issues.push(issue("SOURCE_NOT_READY", "/technicalBaseline/status", "blocked"));
  } else if (pgc.source.technicalBaselineId !== baseline.technicalBaselineId) {
    issues.push(issue("BASELINE_MISMATCH", "/source/technicalBaselineId", "blocked"));
  } else if (baseline.projectKind === "existing_project" && !pgc.contextRefs?.repositoryContextRef) {
    issues.push(issue("SOURCE_NOT_READY", "/contextRefs/repositoryContextRef", "blocked"));
  }

  const expectedPhaseId = pgc.planningContractId.startsWith("pgc-") ? pgc.planningContractId.slice(4) : null;
  if (expectedPhaseId && pgc.source.phaseId !== expectedPhaseId) {
    issues.push(issue("PHASE_SOURCE_MISMATCH", "/source/phaseId", "blocked"));
  }

  if (pgc.status === "ready" && !pgc.handoff.readyForArchitecture) {
    issues.push(issue("READY_WITH_BLOCKING_ISSUES", "/handoff/readyForArchitecture"));
  }
  if (pgc.handoff.readyForTaskPlan) {
    issues.push(issue("READY_WITH_BLOCKING_ISSUES", "/handoff/readyForTaskPlan"));
  }

  return { value: pgc, issues };
}

export function validateArchitectureArtifactCandidate(candidate: unknown, pgc: PlanningGenerationContract, baseline: TechnicalBaseline): {
  value: ArchitectureArtifactContract | null;
  issues: ContractIssue[];
  status: ArchitectureArtifactContract["status"];
} {
  const parsed = parseWithIssues(architectureArtifactContractSchema, normalizeArchitectureCandidateForParse(normalizeOptionalEmptyFields(candidate, [
    "brainstormContractId",
    "roadmapId",
    "appId",
    "artifactIntent",
    "creationPolicy",
    "method",
    "path",
    "requestSchema",
    "responseSchema",
    "errorSchema",
    "label",
    "actor",
    "systemResponse",
    "errorCode",
    "reason",
    "reasonCategory",
    "repairability",
    "decisionCategory",
    "decisionQuestion",
    "repositoryContextRef",
    "options",
    "allowFreeform",
    "impact",
    "mitigation",
  ])));
  if (!parsed.value) {
    return { ...parsed, status: "needs_candidate_repair" };
  }

  const aac = parsed.value;
  const issues = [...parsed.issues];
  const includedScopeIds = new Set(pgc.phaseScope.included.map((item) => item.scopeId));
  const deferredScopeIds = new Set(pgc.phaseScope.deferred.map((item) => item.scopeId));
  const acceptanceById = new Map(pgc.phaseScope.acceptanceCandidates.map((item) => [item.id, item]));
  const moduleIds = new Set(aac.modules.map((item) => item.moduleId));
  const boundaryModuleIds = new Set(aac.engineeringBoundary.modules.map((item) => item.moduleId));
  const entityIds = new Set(aac.dataModel.entities.map((item) => item.entityId));
  const relationshipIds = new Set(aac.dataModel.relationships.map((item) => item.relationshipId));
  const dataConstraintIds = new Set([
    ...aac.dataModel.constraints.map((item) => item.constraintId),
    ...aac.dataModel.entities.flatMap((entity) => entity.constraints.map((constraint) => constraint.constraintId)),
  ]);
  const interfaceIds = new Set(aac.interfaces.map((item) => item.interfaceId));
  const userFlowIds = new Set(aac.userFlows.map((item) => item.flowId));
  const stateMachineIds = new Set(aac.stateMachines.map((item) => item.stateMachineId));
  const stateRuleIds = new Set(aac.stateMachines.flatMap((machine) => machine.rules.map((rule) => rule.ruleId)));
  const decisionIds = new Set(aac.risksAndDecisions.decisions.map((item) => item.decisionId));
  const riskIds = new Set(aac.risksAndDecisions.risks.map((item) => item.riskId));
  const fieldIds = new Set([
    ...aac.dataModel.entities.flatMap((entity) => entity.fields.flatMap((field) => collectFieldShapeIds(field))),
    ...aac.interfaces.flatMap((item) => [
      ...(item.requestSchema ?? []).flatMap((field) => collectFieldShapeIds(field)),
      ...(item.responseSchema ?? []).flatMap((field) => collectFieldShapeIds(field)),
      ...(item.errorSchema ?? []).flatMap((field) => collectFieldShapeIds(field)),
    ]),
  ]);
  const frontendDataViewIds = new Set(aac.frontendExperience?.dataViews?.map((item) => item.viewId) ?? []);
  const frontendActionIds = new Set(aac.frontendExperience?.actions?.map((item) => item.actionId) ?? []);
  const frontendOperationPathIds = new Set(aac.frontendExperience?.operationPaths?.map((item) => item.pathId) ?? []);

  if (aac.source.planningGenerationContractId !== pgc.planningContractId) {
    issues.push(issue("SOURCE_NOT_READY", "/source/planningGenerationContractId", "blocked"));
  }
  if (aac.source.technicalBaselineId !== baseline.technicalBaselineId || baseline.technicalBaselineId !== pgc.source.technicalBaselineId) {
    issues.push(issue("BASELINE_MISMATCH", "/source/technicalBaselineId", "blocked"));
  }
  if (pgc.status !== "ready" || !pgc.handoff.readyForArchitecture) {
    issues.push(issue("SOURCE_NOT_READY", "/source/planningGenerationContractId", "blocked"));
  }

  for (const duplicate of duplicates(aac.modules.map((item) => item.moduleId))) {
    issues.push(issue("DUPLICATE_ID", `/modules/${duplicate}`));
  }
  for (const duplicate of duplicates(aac.dataModel.entities.map((item) => item.entityId))) {
    issues.push(issue("DUPLICATE_ID", `/dataModel/entities/${duplicate}`));
  }
  for (const duplicate of duplicates(aac.interfaces.map((item) => item.interfaceId))) {
    issues.push(issue("DUPLICATE_ID", `/interfaces/${duplicate}`));
  }

  for (const app of aac.engineeringBoundary.applications) {
    validatePathList([app.root], "/engineeringBoundary/applications/root", issues);
  }
  for (const mod of aac.engineeringBoundary.modules) {
    if (!moduleIds.has(mod.moduleId)) {
      issues.push(issue("UNKNOWN_ARTIFACT_REF", `/engineeringBoundary/modules/${mod.moduleId}`));
    }
    validatePathList(mod.paths, `/engineeringBoundary/modules/${mod.moduleId}/paths`, issues);
    for (const mapping of mod.layerMappings ?? []) {
      validatePathList(mapping.paths, `/engineeringBoundary/modules/${mod.moduleId}/layerMappings/${mapping.layer}/paths`, issues);
    }
  }

  const checkScopeRefs = (refs: string[], pointer: string): void => {
    for (const ref of refs) {
      if (!includedScopeIds.has(ref)) {
        issues.push(issue("INVALID_SCOPE_REF", `${pointer}/${ref}`));
      }
    }
  };
  const checkAcceptanceRefs = (refs: string[], pointer: string): void => {
    for (const ref of refs) {
      if (!acceptanceById.has(ref)) {
        issues.push(issue("INVALID_ACCEPTANCE_REF", `${pointer}/${ref}`));
      }
    }
  };
  const checkModuleRefs = (refs: string[], pointer: string): void => {
    for (const ref of refs) {
      if (!moduleIds.has(ref)) {
        issues.push(issue("UNKNOWN_ARTIFACT_REF", `${pointer}/${ref}`));
      }
    }
  };
  const checkEntityRefs = (refs: string[], pointer: string): void => {
    for (const ref of refs) {
      if (!entityIds.has(ref)) {
        issues.push(issue("UNKNOWN_ARTIFACT_REF", `${pointer}/${ref}`));
      }
    }
  };

  for (const mod of aac.modules) {
    checkScopeRefs(mod.scopeRefs, `/modules/${mod.moduleId}/scopeRefs`);
    checkAcceptanceRefs(mod.acceptanceRefs, `/modules/${mod.moduleId}/acceptanceRefs`);
    checkModuleRefs(mod.dependsOn, `/modules/${mod.moduleId}/dependsOn`);
  }
  for (const entity of aac.dataModel.entities) {
    checkModuleRefs(entity.moduleRefs, `/dataModel/entities/${entity.entityId}/moduleRefs`);
    checkScopeRefs(entity.scopeRefs, `/dataModel/entities/${entity.entityId}/scopeRefs`);
    checkAcceptanceRefs(entity.acceptanceRefs, `/dataModel/entities/${entity.entityId}/acceptanceRefs`);
  }
  for (const relationship of aac.dataModel.relationships) {
    checkEntityRefs([relationship.fromEntityRef, relationship.toEntityRef], `/dataModel/relationships/${relationship.relationshipId}`);
    checkModuleRefs(relationship.moduleRefs, `/dataModel/relationships/${relationship.relationshipId}/moduleRefs`);
    checkScopeRefs(relationship.scopeRefs, `/dataModel/relationships/${relationship.relationshipId}/scopeRefs`);
    checkAcceptanceRefs(relationship.acceptanceRefs, `/dataModel/relationships/${relationship.relationshipId}/acceptanceRefs`);
  }
  for (const constraint of aac.dataModel.constraints) {
    checkEntityRefs(constraint.entityRefs, `/dataModel/constraints/${constraint.constraintId}/entityRefs`);
    checkAcceptanceRefs(constraint.acceptanceRefs, `/dataModel/constraints/${constraint.constraintId}/acceptanceRefs`);
  }
  for (const contract of aac.interfaces) {
    checkModuleRefs(contract.moduleRefs, `/interfaces/${contract.interfaceId}/moduleRefs`);
    checkEntityRefs(contract.entityRefs, `/interfaces/${contract.interfaceId}/entityRefs`);
    checkScopeRefs(contract.scopeRefs, `/interfaces/${contract.interfaceId}/scopeRefs`);
    checkAcceptanceRefs(contract.acceptanceRefs, `/interfaces/${contract.interfaceId}/acceptanceRefs`);
  }
  for (const flow of aac.userFlows) {
    checkModuleRefs(flow.moduleRefs, `/userFlows/${flow.flowId}/moduleRefs`);
    checkEntityRefs(flow.entityRefs, `/userFlows/${flow.flowId}/entityRefs`);
    checkScopeRefs(flow.scopeRefs, `/userFlows/${flow.flowId}/scopeRefs`);
    checkAcceptanceRefs(flow.acceptanceRefs, `/userFlows/${flow.flowId}/acceptanceRefs`);
    for (const ref of flow.interfaceRefs) {
      if (!interfaceIds.has(ref)) issues.push(issue("UNKNOWN_ARTIFACT_REF", `/userFlows/${flow.flowId}/interfaceRefs/${ref}`));
    }
    for (const step of flow.steps) {
      for (const ref of step.interfaceRefs) {
        if (!interfaceIds.has(ref)) issues.push(issue("UNKNOWN_ARTIFACT_REF", `/userFlows/${flow.flowId}/steps/${step.stepId}/interfaceRefs/${ref}`));
      }
      for (const ref of step.stateMachineRefs) {
        if (!stateMachineIds.has(ref)) issues.push(issue("UNKNOWN_ARTIFACT_REF", `/userFlows/${flow.flowId}/steps/${step.stepId}/stateMachineRefs/${ref}`));
      }
    }
  }
  for (const machine of aac.stateMachines) {
    if (machine.entityRef && !entityIds.has(machine.entityRef)) {
      issues.push(issue("UNKNOWN_ARTIFACT_REF", `/stateMachines/${machine.stateMachineId}/entityRef`));
    }
    if (machine.entityRef && !machine.entityRefs.includes(machine.entityRef)) {
      issues.push(issue("UNKNOWN_ARTIFACT_REF", `/stateMachines/${machine.stateMachineId}/entityRefs`));
    }
    checkEntityRefs(machine.entityRefs, `/stateMachines/${machine.stateMachineId}/entityRefs`);
    checkModuleRefs(machine.moduleRefs, `/stateMachines/${machine.stateMachineId}/moduleRefs`);
    checkScopeRefs(machine.scopeRefs, `/stateMachines/${machine.stateMachineId}/scopeRefs`);
    checkAcceptanceRefs(machine.acceptanceRefs, `/stateMachines/${machine.stateMachineId}/acceptanceRefs`);
    const states = new Set(machine.states.map((state) => state.stateId));
    const events = new Set(machine.events.map((event) => event.eventId));
    if (!states.has(machine.initialState)) {
      issues.push(issue("UNKNOWN_ARTIFACT_REF", `/stateMachines/${machine.stateMachineId}/initialState`));
    }
    for (const transition of machine.transitions) {
      if (!states.has(transition.from)) issues.push(issue("UNKNOWN_ARTIFACT_REF", `/stateMachines/${machine.stateMachineId}/transitions/${transition.transitionId}/from`));
      if (!states.has(transition.to)) issues.push(issue("UNKNOWN_ARTIFACT_REF", `/stateMachines/${machine.stateMachineId}/transitions/${transition.transitionId}/to`));
      if (!events.has(transition.event)) issues.push(issue("UNKNOWN_ARTIFACT_REF", `/stateMachines/${machine.stateMachineId}/transitions/${transition.transitionId}/event`));
    }
    for (const rule of machine.rules) {
      checkAcceptanceRefs(rule.acceptanceRefs, `/stateMachines/${machine.stateMachineId}/rules/${rule.ruleId}/acceptanceRefs`);
    }
  }

  for (const entry of aac.acceptanceMatrix) {
    const acceptance = acceptanceById.get(entry.acceptanceId);
    if (!acceptance) {
      issues.push(issue("INVALID_ACCEPTANCE_REF", `/acceptanceMatrix/${entry.acceptanceId}`));
    } else {
      if (entry.statement !== acceptance.statement || entry.priority !== acceptance.priority) {
        issues.push(issue("INVALID_ACCEPTANCE_REF", `/acceptanceMatrix/${entry.acceptanceId}`));
      }
      if (acceptance.priority === "must" && entry.coverageStatus !== "covered") {
        const repairability = entry.repairability ?? "agent_repairable";
        const code = entry.reasonCategory === "scope_conflict" || entry.reasonCategory === "baseline_conflict" || entry.reasonCategory === "user_tradeoff"
          ? "MUST_ACCEPTANCE_REQUIRES_USER_DECISION"
          : "MUST_ACCEPTANCE_NOT_COVERED";
        issues.push(issue(code, `/acceptanceMatrix/${entry.acceptanceId}`, repairability));
      }
    }
    if (entry.coverageStatus !== "covered" && !entry.reason) {
      issues.push(issue("MUST_ACCEPTANCE_NOT_COVERED", `/acceptanceMatrix/${entry.acceptanceId}/reason`));
    }
    for (const coverage of entry.coverage) {
      for (const ref of coverage.refs) {
        const actualType = coverageRefActualType(ref, {
          moduleIds,
          entityIds,
          dataConstraintIds,
          relationshipIds,
          interfaceIds,
          userFlowIds,
          stateMachineIds,
          stateRuleIds,
          decisionIds,
          riskIds,
        });
        if (!actualType) {
          issues.push(issue("UNKNOWN_ARTIFACT_REF", `/acceptanceMatrix/${entry.acceptanceId}/coverage/${coverage.type}/${ref}`));
        } else if (actualType !== coverage.type) {
          issues.push(issue("AAC_COVERAGE_TYPE_MISMATCH", `/acceptanceMatrix/${entry.acceptanceId}/coverage/${coverage.type}/${ref}`));
        }
      }
    }
  }
  for (const acceptance of pgc.phaseScope.acceptanceCandidates.filter((item) => item.priority === "must")) {
    const entry = aac.acceptanceMatrix.find((item) => item.acceptanceId === acceptance.id);
    if (!entry) {
      issues.push(issue("MUST_ACCEPTANCE_NOT_COVERED", `/acceptanceMatrix/${acceptance.id}`));
    }
  }

  const requirementDetailIds = new Set(pgc.requirementDetails?.items.map((item) => item.detailId) ?? []);
  if (pgc.requirementDetails && requirementDetailIds.size > 0) {
    for (const detail of pgc.requirementDetails.items.filter((item) => item.requiredForCurrentPhase)) {
      if (!(aac.detailCoverage ?? []).some((entry) => entry.detailId === detail.detailId)) {
        issues.push(issue("DETAIL_COVERAGE_INVALID", `/detailCoverage/${detail.detailId}`));
      }
    }
    for (const entry of aac.detailCoverage ?? []) {
      if (!requirementDetailIds.has(entry.detailId)) {
        issues.push(issue("DETAIL_REF_INVALID", `/detailCoverage/${entry.detailId}`));
      }
      const allRefs = Object.values(entry.artifactRefs).flat();
      if (entry.coverageStatus === "covered" && allRefs.length === 0) {
        issues.push(issue("DETAIL_COVERAGE_INVALID", `/detailCoverage/${entry.detailId}/artifactRefs`));
      }
      if (entry.coverageStatus !== "covered" && !entry.reason) {
        issues.push(issue("DETAIL_COVERAGE_INVALID", `/detailCoverage/${entry.detailId}/reason`));
      }
      validateDetailCoverageRefs(entry, {
        moduleIds,
        entityIds,
        fieldIds,
        dataConstraintIds,
        interfaceIds,
        userFlowIds,
        stateMachineIds,
        frontendDataViewIds,
        frontendActionIds,
        frontendOperationPathIds,
        acceptanceIds: new Set(aac.acceptanceMatrix.map((item) => item.acceptanceId)),
      }, issues);
    }
  }

  for (const decision of aac.risksAndDecisions.decisions) {
    checkScopeRefs(decision.scopeRefs, `/risksAndDecisions/decisions/${decision.decisionId}/scopeRefs`);
    checkAcceptanceRefs(decision.acceptanceRefs, `/risksAndDecisions/decisions/${decision.decisionId}/acceptanceRefs`);
    if (decision.status === "needs_user_decision") {
      if (!decision.decisionCategory || !decision.decisionQuestion || decision.allowFreeform === undefined || !decision.impact || (decision.options?.length ?? 0) < 2) {
        issues.push(issue("NEEDS_USER_DECISION_SHAPE_INVALID", `/risksAndDecisions/decisions/${decision.decisionId}`, "requires_user_decision"));
      }
    }
  }
  for (const risk of aac.risksAndDecisions.risks) {
    checkScopeRefs(risk.scopeRefs, `/risksAndDecisions/risks/${risk.riskId}/scopeRefs`);
    checkAcceptanceRefs(risk.acceptanceRefs, `/risksAndDecisions/risks/${risk.riskId}/acceptanceRefs`);
    if ((risk.severity === "high" || risk.severity === "blocking") && !risk.mitigation) {
      issues.push(issue("MUST_ACCEPTANCE_NOT_COVERED", `/risksAndDecisions/risks/${risk.riskId}/mitigation`));
    }
    if (risk.severity === "blocking" && risk.status === "open") {
      issues.push(issue("SOURCE_NOT_READY", `/risksAndDecisions/risks/${risk.riskId}`, "blocked"));
    }
  }
  for (const assumption of aac.risksAndDecisions.assumptions) {
    checkScopeRefs(assumption.scopeRefs, `/risksAndDecisions/assumptions/${assumption.assumptionId}/scopeRefs`);
    checkEntityRefs(assumption.entityRefs, `/risksAndDecisions/assumptions/${assumption.assumptionId}/entityRefs`);
  }
  for (const note of aac.risksAndDecisions.deferredNotes) {
    if (!deferredScopeIds.has(note.deferredRef)) {
      issues.push(issue("INVALID_DEFERRED_REF", `/risksAndDecisions/deferredNotes/${note.deferredRef}`));
    }
  }

  const confirmedFrontendRef = pgc.contextRefs?.confirmedFrontendExperienceRef ?? pgc.contextRefs?.currentFrontendExperienceRef;
  if (confirmedFrontendRef && !aac.frontendExperience) {
    issues.push(issue("SOURCE_NOT_READY", "/frontendExperience", "blocked"));
  }

  if (aac.frontendExperience) {
    const frontend = aac.frontendExperience;
    if (confirmedFrontendRef && frontend.sourceRefs?.brainstormFrontendExperienceRef !== confirmedFrontendRef) {
      issues.push(issue("SOURCE_NOT_READY", "/frontendExperience/sourceRefs/brainstormFrontendExperienceRef", "blocked"));
    }
    if (frontend.required && frontend.experienceLevel === "none") {
      issues.push(issue("SOURCE_NOT_READY", "/frontendExperience/experienceLevel", "blocked"));
    }
    if (frontend.required && frontend.navigation.required && frontend.surfaces.length > 1 && frontend.navigation.items.length === 0) {
      issues.push(issue("SOURCE_NOT_READY", "/frontendExperience/navigation/items", "blocked"));
    }
  }

  if (aac.runtimeDelivery) {
    const runtime = aac.runtimeDelivery;
    if (runtime.status === "modified") {
      if (!runtime.basis.technicalBaselineRef) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/basis/technicalBaselineRef", "blocked"));
      }
      if (!runtime.build?.command) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/build/command", "blocked"));
      }
      if (runtime.build && !Array.isArray(runtime.build.codeLevelExpectations)) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/build/codeLevelExpectations", "blocked"));
      }
      if (!runtime.start?.command && (!runtime.runtimeSurfaces || runtime.runtimeSurfaces.length === 0)) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/start/command", "blocked"));
      }
      if (runtime.start && !Array.isArray(runtime.start.codeLevelExpectations)) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/start/codeLevelExpectations", "blocked"));
      }
      if (!runtime.runtimeSurfaces || runtime.runtimeSurfaces.length === 0) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/runtimeSurfaces", "blocked"));
      }
      if (!runtime.taskPlanningGuidance) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/taskPlanningGuidance", "blocked"));
      } else {
        if (runtime.taskPlanningGuidance.verificationBoundary !== "code_level_only") {
          issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/taskPlanningGuidance/verificationBoundary", "blocked"));
        }
        if (!runtime.taskPlanningGuidance.doNotRequireCleanInstallOrContainerBuild) {
          issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/taskPlanningGuidance/doNotRequireCleanInstallOrContainerBuild", "blocked"));
        }
      }
      if (!runtime.httpProbes?.previewPath) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/httpProbes/previewPath", "blocked"));
      }
      if (runtime.httpProbes && runtime.httpProbes.expectedStatus !== "2xx_or_3xx") {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/httpProbes/expectedStatus", "blocked"));
      }
      if (runtime.frontend?.required && (!runtime.frontend.outputDir || !runtime.frontend.servedBy)) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/frontend", "blocked"));
      }
      if (runtime.deliveryMechanics?.codegen && !Array.isArray(runtime.deliveryMechanics.codegen.codeLevelExpectations)) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/deliveryMechanics/codegen/codeLevelExpectations", "blocked"));
      }
      if (runtime.api?.required && !runtime.api.entry && runtime.api.probePaths.length === 0) {
        issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/api", "blocked"));
      }
    }
    if (runtime.status === "unchanged" && !runtime.basis.previousRuntimeDeliveryRef) {
      issues.push(issue("SOURCE_NOT_READY", "/runtimeDelivery/basis/previousRuntimeDeliveryRef", "blocked"));
    }
  }

  const status = computeArchitectureStatus(aac, issues);
  if (status === "ready" && !aac.handoff.readyForTaskPlan) {
    issues.push(issue("READY_WITH_BLOCKING_ISSUES", "/handoff/readyForTaskPlan"));
  }

  return { value: aac, issues, status: computeArchitectureStatus(aac, issues) };
}

export function normalizeOptionalEmptyFields(candidate: unknown, optionalFieldNames: string[]): unknown {
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
    return candidate;
  }
  const copy = structuredClone(candidate);
  removeOptionalEmptyFieldsDeep(copy, new Set(optionalFieldNames));
  return copy;
}

function removeOptionalEmptyFieldsDeep(value: unknown, optionalFieldNames: Set<string>): void {
  if (!value || typeof value !== "object") {
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item) => removeOptionalEmptyFieldsDeep(item, optionalFieldNames));
    return;
  }
  const record = value as Record<string, unknown>;
  for (const [key, fieldValue] of Object.entries(record)) {
    if (optionalFieldNames.has(key) && (fieldValue === null || fieldValue === "")) {
      delete record[key];
    } else {
      removeOptionalEmptyFieldsDeep(fieldValue, optionalFieldNames);
    }
  }
}

export function isSafeProjectRelativePath(value: string): boolean {
  if (!value || path.isAbsolute(value)) {
    return false;
  }
  const normalized = value.split("\\").join("/");
  if (normalized.startsWith("~") || normalized.includes("\0")) {
    return false;
  }
  const parts = normalized.split("/");
  if (parts.includes("..")) {
    return false;
  }
  const forbidden = new Set([".loom"]);
  return !parts.some((part) => forbidden.has(part));
}

export const BLOCKED_OUTPUT_MAPPING = {
  DESIGN_INSUFFICIENT: {
    nextNode: "architecture_artifact_repair",
    detailsKey: "missingDesignSignals",
    message: "Task execution is blocked because required design information is insufficient.",
  },
  TASKPLAN_INVALID: {
    nextNode: "taskplan_repair",
    detailsKey: "taskPlanExecutionIssues",
    message: "Task execution is blocked because the task contract is invalid or inconsistent.",
  },
  DEPENDENCY_NOT_READY: {
    nextNode: "wait_dependency",
    detailsKey: "dependencyIssues",
    message: "Task execution is blocked because one or more task dependencies are not ready.",
  },
} as const;

export const REVIEW_CATEGORY_TO_ACTION = {
  functional_correctness: "execution_repair",
  integration_risk: "execution_repair",
  test_gap: "execution_repair",
  evidence_insufficient: "execution_repair",
  acceptance_not_satisfied: "execution_repair",
  reference_only_boundary_violation: "execution_repair",
  technical_baseline_violation: "execution_repair",
  forbidden_path_violation: "execution_repair",
  contract_modified: "execution_repair",
  architecture_design_gap: "architecture_artifact_repair",
  acceptance_design_mismatch: "architecture_artifact_repair",
  state_model_gap: "architecture_artifact_repair",
  interface_contract_gap: "architecture_artifact_repair",
  data_model_gap: "architecture_artifact_repair",
  task_scope_mismatch: "taskplan_repair",
  task_dependency_issue: "taskplan_repair",
  task_verification_mapping_issue: "taskplan_repair",
  task_artifact_mapping_issue: "taskplan_repair",
  scope_decision_required: "needs_user_decision",
  baseline_decision_required: "needs_user_decision",
  acceptance_tradeoff_required: "needs_user_decision",
  product_behavior_decision_required: "needs_user_decision",
  environment_or_dependency: "manual_review",
  review_limitation: "manual_review",
  external_system_unavailable: "manual_review",
} as const;

export function issue(
  code: string,
  pointer: string,
  repairability?: ContractIssue["repairability"],
): ContractIssue {
  issueCounter += 1;
  const template = ISSUE_TEMPLATES[code] ?? ISSUE_TEMPLATES.SCHEMA_INVALID;
  return contractIssueSchema.parse({
    issueId: `issue-${issueCounter.toString().padStart(4, "0")}`,
    code,
    severity: "blocking",
    path: pointer,
    message: template.message,
    repairability: repairability ?? template.repairability ?? "agent_repairable",
    repairHint: template.repairHint,
  });
}

export function resetIssueCounter(): void {
  issueCounter = 0;
}

function parseWithIssues<T>(schema: ZodSchema<T>, value: unknown): {
  value: T | null;
  issues: ContractIssue[];
} {
  try {
    return { value: schema.parse(value), issues: [] };
  } catch (error) {
    if (error instanceof ZodError) {
      return {
        value: null,
        issues: error.issues.map(schemaIssueFromZodIssue),
      };
    }
    throw error;
  }
}

function schemaIssueFromZodIssue(zodIssue: ZodError["issues"][number]): ContractIssue {
  const pointer = `/${zodIssue.path.map(String).join("/") || ""}`;
  const base = issue("SCHEMA_INVALID", pointer);
  const details = zodIssueDetails(zodIssue, pointer);
  const allowedText = details.allowedValues && details.allowedValues.length > 0
    ? ` Allowed values: ${details.allowedValues.join(", ")}.`
    : "";
  const receivedText = details.received ? ` Received: ${details.received}.` : "";
  const repairHint = details.allowedValues && details.allowedValues.length > 0
    ? enumSchemaRepairHint(pointer, details.allowedValues)
    : `Repair the contract field at ${pointer}: ${zodIssue.message}.`;
  return contractIssueSchema.parse({
    ...base,
    message: `Contract schema validation failed at ${pointer}: ${zodIssue.message}.${allowedText}${receivedText}`,
    repairHint,
    schemaError: details,
  });
}

function zodIssueDetails(zodIssue: ZodError["issues"][number], pointer: string): {
  source: "zod";
  code: string;
  path: string;
  message: string;
  allowedValues?: string[];
  expected?: string;
  received?: string;
} {
  const rawIssue = zodIssue as unknown as Record<string, unknown>;
  const allowedValues = zodIssueAllowedValues(rawIssue);
  return {
    source: "zod",
    code: String(rawIssue.code ?? "unknown"),
    path: pointer,
    message: zodIssue.message,
    ...(allowedValues.length > 0 ? { allowedValues } : {}),
    ...optionalStringDetail("expected", rawIssue.expected),
    ...optionalStringDetail("received", rawIssue.received ?? rawIssue.input),
  };
}

function enumSchemaRepairHint(pointer: string, allowedValues: string[]): string {
  const allowed = allowedValues.join(", ");
  const fieldSpecific = enumFieldSpecificRepairRule(pointer);
  return [
    `Use exactly one allowed value for ${pointer}: ${allowed}.`,
    "Choose the value from the field's source contract, generation rules, and current request facts; do not choose arbitrarily just to satisfy schema.",
    fieldSpecific,
    "Do not copy pipe-joined examples or prose into enum fields.",
    "If the correct value cannot be determined from the current request and source refs, repair the candidate as blocked or return the appropriate blocked/needs-decision path instead of guessing.",
  ].filter((item): item is string => Boolean(item)).join(" ");
}

function enumFieldSpecificRepairRule(pointer: string): string | null {
  if (pointer === "/runtimeDelivery/status") {
    return "For runtimeDelivery.status, use modified when this phase changes runtime/code/script/API shape, unchanged only when sourceRefs.previousRuntimeDeliveryRef exists and the current phase truly preserves runtime delivery, and not_applicable only when this phase has no runtime delivery surface.";
  }
  if (pointer === "/status") {
    return "For top-level candidate status, use ready only when the whole candidate is ready for the next handoff; use the schema's blocked/repair status only when the candidate records blocking reasons.";
  }
  if (pointer === "/frontendExperience/experienceLevel") {
    return "For frontendExperience.experienceLevel, choose the level from the confirmed frontend experience source and current phase UI responsibilities; do not downgrade a user-confirmed product target.";
  }
  return null;
}

function zodIssueAllowedValues(rawIssue: Record<string, unknown>): string[] {
  const values = Array.isArray(rawIssue.options)
    ? rawIssue.options
    : Array.isArray(rawIssue.values)
      ? rawIssue.values
      : [];
  return values
    .map((value) => serializeIssueValue(value))
    .filter((value): value is string => typeof value === "string" && value.length > 0);
}

function optionalStringDetail(key: "expected" | "received", value: unknown): Record<string, string> {
  const serialized = serializeIssueValue(value);
  return serialized ? { [key]: serialized } : {};
}

function serializeIssueValue(value: unknown): string | undefined {
  if (value === undefined) {
    return undefined;
  }
  if (typeof value === "string") {
    return value;
  }
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function normalizeArchitectureCandidateForParse(candidate: unknown): unknown {
  if (!candidate || typeof candidate !== "object" || Array.isArray(candidate)) {
    return candidate;
  }
  const cloned = JSON.parse(JSON.stringify(candidate)) as Record<string, unknown>;
  const userFlows = Array.isArray(cloned.userFlows) ? cloned.userFlows : [];
  for (const flow of userFlows) {
    if (!flow || typeof flow !== "object" || Array.isArray(flow)) {
      continue;
    }
    const steps = Array.isArray((flow as { steps?: unknown }).steps) ? (flow as { steps: unknown[] }).steps : [];
    for (const step of steps) {
      if (step && typeof step === "object" && !Array.isArray(step) && !("stateMachineRefs" in step)) {
        (step as { stateMachineRefs: string[] }).stateMachineRefs = [];
      }
    }
  }
  return cloned;
}

function validatePathList(paths: string[], pointer: string, issues: ContractIssue[]): void {
  for (const item of paths) {
    if (!isSafeProjectRelativePath(item)) {
      issues.push(issue("INVALID_PATH", `${pointer}/${item}`));
    }
  }
}

function duplicates(values: string[]): string[] {
  const seen = new Set<string>();
  const dupes = new Set<string>();
  for (const value of values) {
    if (seen.has(value)) {
      dupes.add(value);
    }
    seen.add(value);
  }
  return [...dupes];
}

function getArtifactSets(aac: ArchitectureArtifactContract): {
  moduleIds: Set<string>;
  entityIds: Set<string>;
  interfaceIds: Set<string>;
  userFlowIds: Set<string>;
  stateMachineIds: Set<string>;
  decisionIds: Set<string>;
  riskIds: Set<string>;
} {
  return {
    moduleIds: new Set(aac.modules.map((item) => item.moduleId)),
    entityIds: new Set(aac.dataModel.entities.map((item) => item.entityId)),
    interfaceIds: new Set(aac.interfaces.map((item) => item.interfaceId)),
    userFlowIds: new Set(aac.userFlows.map((item) => item.flowId)),
    stateMachineIds: new Set(aac.stateMachines.map((item) => item.stateMachineId)),
    decisionIds: new Set(aac.risksAndDecisions.decisions.map((item) => item.decisionId)),
    riskIds: new Set(aac.risksAndDecisions.risks.map((item) => item.riskId)),
  };
}

function checkArtifactRefs(refs: string[], allowed: Set<string>, pointer: string, issues: ContractIssue[]): void {
  for (const ref of refs) {
    if (!allowed.has(ref)) {
      issues.push(issue("UNKNOWN_ARTIFACT_REF", `${pointer}/${ref}`));
    }
  }
}

function hasDependencyCycle(taskPlan: TaskPlan): boolean {
  const visiting = new Set<string>();
  const visited = new Set<string>();
  const byId = new Map(taskPlan.tasks.map((task) => [task.taskId, task]));
  const visit = (taskId: string): boolean => {
    if (visited.has(taskId)) return false;
    if (visiting.has(taskId)) return true;
    visiting.add(taskId);
    const task = byId.get(taskId);
    for (const dependency of task?.dependsOn ?? []) {
      if (visit(dependency)) return true;
    }
    visiting.delete(taskId);
    visited.add(taskId);
    return false;
  };
  return taskPlan.tasks.some((task) => visit(task.taskId));
}

function changeSetMode(request: ReviewRequest): string | null {
  const mode = request.changeSet?.mode ?? request.outputContract.changeContextMode;
  return typeof mode === "string" ? mode : null;
}

function isAllowedReviewReadRef(
  ref: { type: string; ref: string },
  allowedReadRefs: Set<string>,
  changedFileRefs: Set<string>,
  verificationEvidenceRefs: Set<string>,
): boolean {
  if (allowedReadRefs.has(ref.ref)) {
    return true;
  }
  if (ref.type === "changed_file") {
    return changedFileRefs.has(normalizeReviewFileRef(ref.ref));
  }
  if (ref.type === "verification_evidence") {
    return verificationEvidenceRefs.has(ref.ref);
  }
  return false;
}

function isAllowedReviewEvidenceRef(
  ref: { type: string; ref: string },
  allowedReadRefs: Set<string>,
  changedFileRefs: Set<string>,
  verificationEvidenceRefs: Set<string>,
): boolean {
  if (ref.type === "manual_note") {
    return true;
  }
  if (allowedReadRefs.has(ref.ref)) {
    return true;
  }
  if (ref.type === "changed_file") {
    return changedFileRefs.has(normalizeReviewFileRef(ref.ref));
  }
  if (ref.type === "verification_result") {
    return verificationEvidenceRefs.has(ref.ref);
  }
  return false;
}

function changedFileRefSet(request: ReviewRequest): Set<string> {
  const refs = new Set<string>();
  const add = (value: unknown): void => {
    if (typeof value === "string" && value.length > 0) {
      refs.add(normalizeReviewFileRef(value));
    }
  };
  for (const file of extractAllowedRefs(request, "changedFilePaths")) {
    add(file);
  }
  const changeFiles = Array.isArray((request.changeSet as { files?: unknown } | undefined)?.files)
    ? (request.changeSet as { files: unknown[] }).files
    : [];
  for (const file of changeFiles) {
    if (typeof file === "string") {
      add(file);
    } else if (typeof file === "object" && file !== null) {
      add((file as { path?: unknown }).path);
    }
  }
  for (const task of request.executionArtifacts?.taskResults ?? []) {
    for (const file of task.changedFiles) {
      add(file);
    }
  }
  return refs;
}

function verificationEvidenceRefSet(request: ReviewRequest): Set<string> {
  const refs = new Set<string>(extractAllowedRefs(request, "verificationEvidenceRefs"));
  for (const taskResult of request.executionArtifacts?.taskResults ?? []) {
    for (const verification of taskResult.verificationResults) {
      refs.add(verification.verificationId);
      refs.add(`${taskResult.taskResultId}:${verification.verificationId}`);
      refs.add(`${taskResult.taskId}:${verification.verificationId}`);
    }
  }
  return refs;
}

function normalizeReviewFileRef(value: string): string {
  return value
    .replace(/^changed_file:/, "")
    .replace(/^file:/, "")
    .replace(/^\.\//, "")
    .replace(/\\/g, "/");
}

function extractAllowedRefs(request: ReviewRequest, key: string): string[] {
  const allowedRefs = request.outputContract.allowedRefs;
  if (typeof allowedRefs !== "object" || allowedRefs === null || Array.isArray(allowedRefs)) {
    return key === "taskIds" ? request.reviewScope.taskIds ?? [] : [];
  }
  const value = (allowedRefs as Record<string, unknown>)[key];
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function expectedReviewTopAction(result: ReviewResult, request: ReviewRequest): ReviewResult["nextAction"]["type"] {
  if (result.findings.some((finding) => finding.recommendedNextAction === "needs_user_decision" && isBlockingFinding(finding))) {
    return "needs_user_decision";
  }
  const hasBlockingManual = result.findings.some((finding) =>
    finding.recommendedNextAction === "manual_review" &&
    isBlockingFinding(finding) &&
    (
      finding.category === "review_limitation" ||
      finding.category === "environment_or_dependency" ||
      finding.failureClass === "environment_blocker" ||
      finding.severityClass === "manual_review"
    ),
  );
  if (hasBlockingManual) {
    return "manual_review";
  }
  for (const action of ["architecture_artifact_repair", "taskplan_repair", "execution_repair"] as const) {
    if (result.findings.some((finding) => finding.recommendedNextAction === action && isBlockingFinding(finding))) {
      return action;
    }
  }
  return hasNextPhase(request) ? "continue_to_next_phase" : "done";
}

function isBlockingFinding(finding: ReviewResult["findings"][number]): boolean {
  if (finding.severityClass) {
    return finding.severityClass === "blocking" || finding.severityClass === "manual_review";
  }
  return finding.severity === "critical" || finding.severity === "major";
}

function hasNextPhase(request: ReviewRequest): boolean {
  return request.reviewScope.nextPhasePreview?.kind === "candidate";
}

function coverageRefActualType(
  ref: string,
  ids: {
    moduleIds: Set<string>;
    entityIds: Set<string>;
    dataConstraintIds: Set<string>;
    relationshipIds: Set<string>;
    interfaceIds: Set<string>;
    userFlowIds: Set<string>;
    stateMachineIds: Set<string>;
    stateRuleIds: Set<string>;
    decisionIds: Set<string>;
    riskIds: Set<string>;
  },
): ArchitectureArtifactContract["acceptanceMatrix"][number]["coverage"][number]["type"] | null {
  if (ids.moduleIds.has(ref)) return "module";
  if (ids.entityIds.has(ref)) return "data_entity";
  if (ids.dataConstraintIds.has(ref)) return "data_constraint";
  if (ids.relationshipIds.has(ref)) return "relationship";
  if (ids.interfaceIds.has(ref)) return "interface";
  if (ids.userFlowIds.has(ref)) return "user_flow";
  if (ids.stateMachineIds.has(ref)) return "state_machine";
  if (ids.stateRuleIds.has(ref)) return "state_rule";
  if (ids.decisionIds.has(ref)) return "decision";
  if (ids.riskIds.has(ref)) return "risk";
  return null;
}

function validateDetailCoverageRefs(
  entry: NonNullable<ArchitectureArtifactContract["detailCoverage"]>[number],
  ids: {
    moduleIds: Set<string>;
    entityIds: Set<string>;
    fieldIds: Set<string>;
    dataConstraintIds: Set<string>;
    interfaceIds: Set<string>;
    userFlowIds: Set<string>;
    stateMachineIds: Set<string>;
    frontendDataViewIds: Set<string>;
    frontendActionIds: Set<string>;
    frontendOperationPathIds: Set<string>;
    acceptanceIds: Set<string>;
  },
  issues: ContractIssue[],
): void {
  const checks: Array<[keyof typeof entry.artifactRefs, Set<string>]> = [
    ["modules", ids.moduleIds],
    ["entities", ids.entityIds],
    ["fields", ids.fieldIds],
    ["constraints", ids.dataConstraintIds],
    ["interfaces", ids.interfaceIds],
    ["userFlows", ids.userFlowIds],
    ["stateMachines", ids.stateMachineIds],
    ["frontendDataViews", ids.frontendDataViewIds],
    ["frontendActions", ids.frontendActionIds],
    ["frontendOperationPaths", ids.frontendOperationPathIds],
    ["acceptanceMatrix", ids.acceptanceIds],
  ];
  for (const [key, validIds] of checks) {
    for (const ref of entry.artifactRefs[key]) {
      if (!validIds.has(ref)) {
        issues.push(issue("UNKNOWN_ARTIFACT_REF", `/detailCoverage/${entry.detailId}/artifactRefs/${key}/${ref}`));
      }
    }
  }
}

function collectFieldShapeIds(field: FieldShape): string[] {
  return [
    field.fieldId,
    ...(field.items ? collectFieldShapeIds(field.items) : []),
    ...(field.fields?.flatMap((nested) => collectFieldShapeIds(nested)) ?? []),
  ];
}

function computeArchitectureStatus(
  candidate: ArchitectureArtifactContract,
  issues: ContractIssue[],
): ArchitectureArtifactContract["status"] {
  if (issues.some((item) => item.repairability === "blocked")) {
    return "blocked";
  }
  if (
    candidate.risksAndDecisions.decisions.some((item) => item.status === "needs_user_decision") ||
    issues.some((item) => item.repairability === "requires_user_decision")
  ) {
    return "needs_user_decision";
  }
  if (issues.length > 0) {
    return "needs_candidate_repair";
  }
  return "ready";
}
