#!/usr/bin/env node
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const root = path.resolve(__dirname, "..");

function read(file) {
  return fs.readFileSync(path.join(root, file), "utf8");
}

function assertIncludes(file, needle, message) {
  assert.ok(read(file).includes(needle), message ?? `${file} must include ${needle}`);
}

function assertNotIncludes(file, needle, message) {
  assert.equal(read(file).includes(needle), false, message ?? `${file} must not include ${needle}`);
}

function runCli(projectRoot, args, env = {}) {
  const result = spawnSync("node", [path.join(root, "dist/cli.js"), ...args], {
    cwd: projectRoot,
    env: {
      ...process.env,
      LOOM_AGENT_PROFILE: "codex",
      LOOM_COMPACT_OUTPUT: "1",
      ...env,
    },
    encoding: "utf8",
  });
  const output = result.stdout.trim();
  let parsed = null;
  if (output.length > 0) {
    parsed = JSON.parse(output);
  }
  return { result, parsed };
}

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

const { agentActionContract, brainstormSessionAgentActionContract, normalizeAgentActionForFieldGroups } = require("../dist/core/operations/agent-action");

function assertNoCoveredFieldPaths(groups, label) {
  const requiredFields = groups
    .filter((group) => group.required)
    .flatMap((group) => group.fields);
  for (const group of groups) {
    for (const field of group.fields) {
      const coveredInGroup = group.fields.find((candidate) => candidate !== field && field.startsWith(`${candidate}.`));
      assert.equal(coveredInGroup, undefined, `${label}: ${group.groupId} must not read both ${coveredInGroup} and ${field}.`);
      if (!group.required) {
        const coveredByRequired = requiredFields.find((candidate) => field === candidate || field.startsWith(`${candidate}.`));
        assert.equal(coveredByRequired, undefined, `${label}: optional field ${field} is already covered by required field ${coveredByRequired}.`);
      }
    }
  }
}

function allFields(groups) {
  return groups.flatMap((group) => group.fields);
}

const action = agentActionContract({
  actionKind: "execute_task",
  instruction: "Execute.",
  read: {
    required: [
      "this request",
      "referencedArtifactReadGuide",
      "task",
      "task.frontendExperienceRequirement",
      "task.frontendExperienceRequirement.executionGuidance",
      "sourceContext.acceptanceSnapshot",
      "outputContract.schemaShape",
      "outputContract.schemaShape.runtimeDeliveryEvidence.requiredCheckIds",
      "taskConceptGrounding when mode=concepts_present",
    ],
    optional: [
      "sourceRefs",
      "sourceRefs.taskPlanRef",
      "task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs",
      "task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource",
      "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
      "outputContract.schemaShape.frontendExperienceSelfCheck.requirementRef",
      "outputContract.schemaShape.frontendExperienceSelfCheck.status",
      "outputContract.schemaShape.frontendExperienceSelfCheck.surfacesTouched",
      "outputContract.schemaShape.frontendExperienceSelfCheck.workflowsCovered",
      "outputContract.schemaShape.frontendExperienceSelfCheck.userActionsImplemented",
      "outputContract.schemaShape.frontendExperienceSelfCheck.interactionStatesCovered",
      "outputContract.schemaShape.frontendExperienceSelfCheck.dataBinding",
      "outputContract.schemaShape.frontendExperienceSelfCheck.knownGaps",
      "outputContract.schemaShape.frontendExperienceSelfCheck.notes",
      "outputContract.schemaShape.frontendExperienceSelfCheck.closureRequirementIds",
      "outputContract.schemaShape.frontendExperienceSelfCheck.workflowClosureEvidenceRule",
      "outputContract.schemaShape.runtimeDeliveryEvidence",
      "task.runtimeDeliveryRequirement",
    ],
    displayPolicy: "compact",
  },
  write: { rules: [] },
  submit: {
    command: { name: "record-result", argv: ["record-result"] },
    requiredArgs: [],
    placeholders: {},
    runAfter: "resultFile exists",
  },
  schema: { primary: "TaskResult", shapeLocation: "outputContract.schemaShape", enumLocation: "enumRefs" },
  stopConditions: [],
});

assert.equal(action.read.primaryMethod, "inspect", "agentAction.read must prefer inspect.");
assert.equal(action.read.fallbackMethod, "request_manifest_refs", "agentAction.read must preserve manifest-ref fallback.");
assert.equal(action.read.fields, undefined, "new agentAction.read must not expose legacy fields.");
assert.ok(Array.isArray(action.read.fieldGroups), "agentAction.read.fieldGroups must be generated.");
const taskCoreGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_task_core");
const acceptanceGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_acceptance_and_concepts");
const frontendGuidanceGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_frontend_guidance");
const frontendClosureDetailsGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_frontend_closure_details");
const resultContractGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_result_contract");
const resultFrontendSelfCheckGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_result_frontend_self_check_contract");
const resultFrontendClosureGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_result_frontend_closure_contract");
const runtimeResultContractGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_runtime_result_contract");
const frontendBindingDetailsGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_frontend_binding_details");
const runtimeGuidanceGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_runtime_guidance");
const dependencyGroup = action.read.fieldGroups.find((group) => group.groupId === "execute_task_dependency_context");
assert.ok(taskCoreGroup, "TaskExecution readPlan must include task_core.");
assert.ok(acceptanceGroup, "TaskExecution readPlan must include acceptance_and_concepts.");
assert.ok(frontendGuidanceGroup, "TaskExecution readPlan must include frontend_guidance when frontend fields are explicit.");
assert.ok(frontendClosureDetailsGroup, "TaskExecution readPlan must expose frontend closure details separately.");
assert.ok(resultContractGroup, "TaskExecution readPlan must include result_contract.");
assert.ok(resultFrontendSelfCheckGroup, "TaskExecution readPlan must expose frontend self-check contract separately.");
assert.ok(resultFrontendClosureGroup, "TaskExecution readPlan must expose frontend closure result details separately.");
assert.ok(runtimeResultContractGroup, "TaskExecution readPlan must expose runtime result contract separately.");
assert.ok(frontendBindingDetailsGroup, "TaskExecution readPlan must expose frontend binding details separately.");
assert.ok(runtimeGuidanceGroup, "TaskExecution readPlan must include runtime_guidance as optional context.");
assert.ok(dependencyGroup, "TaskExecution readPlan must include dependency_context as optional context.");
assert.equal(taskCoreGroup.required, true, "task_core must be required.");
assert.equal(frontendGuidanceGroup.required, true, "frontend_guidance must be required.");
assert.equal(resultContractGroup.required, true, "result_contract must be required.");
assert.equal(frontendClosureDetailsGroup.required, false, "full workflowClosureRequirements must not be a required default read.");
assert.equal(resultFrontendSelfCheckGroup.required, false, "frontend self-check schema details must be read only before writing self-check.");
assert.equal(resultFrontendClosureGroup.required, false, "frontend closure schema details must be read only before writing satisfied closure evidence.");
assert.equal(runtimeResultContractGroup.required, false, "runtimeDeliveryEvidence schema must not be a required default read.");
assert.equal(frontendBindingDetailsGroup.required, false, "large frontendBackendBindings must not be a required default read.");
assert.deepEqual(
  taskCoreGroup.fields,
  [
    "task.taskId",
    "task.groupId",
    "task.title",
    "task.taskKind",
    "task.implementationActions",
    "task.objective",
    "task.dependsOn",
    "task.scopeRefs",
    "task.acceptanceRefs",
    "task.writeBoundary",
    "task.verificationIntents",
    "task.conceptRefs",
    "task.conceptResponsibilities",
    "task.conceptVerificationIntents",
  ],
  "TaskExecution task_core must read task subfields instead of the full task object.",
);
assert.equal(
  allFields(action.read.fieldGroups).includes("task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings"),
  true,
  "TaskExecution readPlan must keep frontendBackendBindings available.",
);
assert.equal(
  frontendGuidanceGroup.fields.includes("task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"),
  false,
  "TaskExecution frontend_guidance must not default-read workflow closure refs.",
);
assert.equal(
  frontendClosureDetailsGroup.fields.includes("task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs"),
  true,
  "TaskExecution must keep closureRequirementRefs available in an on-demand group.",
);
assert.equal(
  frontendClosureDetailsGroup.fields.includes("task.frontendExperienceRequirement.executionGuidance.workflowClosureDetailSource"),
  true,
  "TaskExecution must keep workflowClosureDetailSource available in an on-demand group.",
);
assert.equal(
  resultContractGroup.fields.includes("outputContract.schemaShape.frontendExperienceSelfCheck"),
  false,
  "TaskExecution result_contract must not default-read full frontendExperienceSelfCheck.",
);
assert.equal(
  resultContractGroup.fields.includes("outputContract.schemaShape.runtimeDeliveryEvidence"),
  false,
  "TaskExecution result_contract must not default-read full runtimeDeliveryEvidence.",
);
for (const broadField of ["task", "task.frontendExperienceRequirement", "task.frontendExperienceRequirement.executionGuidance", "outputContract.schemaShape", "outputContract.schemaShape.frontendExperienceSelfCheck"]) {
  assert.equal(allFields(action.read.fieldGroups).includes(broadField), false, `TaskExecution readPlan must not default-read broad field ${broadField}.`);
}
assertNoCoveredFieldPaths(action.read.fieldGroups, "generated read.fieldGroups");
for (const group of action.read.fieldGroups) {
  assert.equal(group.readCommand.name, "inspect", `${group.groupId}: readCommand must use inspect.`);
  assert.deepEqual(group.readCommand.argv.slice(0, 4), ["inspect", "--request", "{requestRef}", "--field"], `${group.groupId}: readCommand argv must be executable.`);
  assert.equal(group.readCommand.argv[4], group.fields.join(","), `${group.groupId}: readCommand must target the same grouped fields.`);
  assert.ok(typeof group.purpose === "string" && group.purpose.length > 0, `${group.groupId}: purpose is required.`);
  assert.ok(typeof group.whenToRead === "string" && group.whenToRead.length > 0, `${group.groupId}: whenToRead is required.`);
  assert.ok(typeof group.fallbackRule === "string" && group.fallbackRule.includes("requestManifest"), `${group.groupId}: fallbackRule must use requestManifest refs.`);
}
assert.equal(JSON.stringify(action).includes("limit"), false, "readPlan must not introduce content limits.");
assert.equal(JSON.stringify(action).includes("truncate"), false, "readPlan must not introduce content truncation.");

const normalizedLegacyAction = normalizeAgentActionForFieldGroups({
  actionKind: "execute_task",
  read: {
    required: [],
    fields: [
      { field: "task", required: true },
      { field: "task.frontendExperienceRequirement", required: true },
      { field: "outputContract", required: true },
      { field: "outputContract.schemaShape", required: true },
      { field: "sourceRefs", required: false },
      { field: "sourceRefs.taskPlanRef", required: false },
      { field: "task.runtimeDeliveryRequirement", required: false },
    ],
    displayPolicy: "compact",
  },
});
assert.equal(normalizedLegacyAction.read.fields, undefined, "legacy read.fields must be removed by CLI normalization.");
assert.ok(
  normalizedLegacyAction.read.fieldGroups.some((group) => group.groupId === "execute_task_task_core"),
  "legacy read.fields must be converted into TaskExecution task_core fieldGroup.",
);
assert.ok(
  normalizedLegacyAction.read.fieldGroups.some((group) => group.groupId === "execute_task_result_contract"),
  "legacy read.fields must be converted into TaskExecution result_contract fieldGroup.",
);
assert.equal(
  allFields(normalizedLegacyAction.read.fieldGroups).includes("task"),
  false,
  "legacy read.fields normalization must not keep broad task as a default read.",
);
assert.equal(
  allFields(normalizedLegacyAction.read.fieldGroups).includes("outputContract.schemaShape"),
  false,
  "legacy read.fields normalization must not keep broad outputContract.schemaShape as a default read.",
);
assertNoCoveredFieldPaths(normalizedLegacyAction.read.fieldGroups, "legacy-normalized read.fieldGroups");

const normalizedExistingGroupAction = normalizeAgentActionForFieldGroups({
  actionKind: "execute_task",
  read: {
    required: [],
    fieldGroups: [
      {
        groupId: "execute_task_core",
        required: true,
        fields: [
          "task",
          "task.frontendExperienceRequirement",
          "task.frontendExperienceRequirement.executionGuidance.frontendBackendBindings",
          "sourceContext.acceptanceSnapshot",
        ],
      },
      {
        groupId: "execute_task_optional_context",
        required: false,
        fields: ["task.runtimeDeliveryRequirement", "sourceRefs", "sourceRefs.taskPlanRef"],
      },
    ],
    displayPolicy: "compact",
  },
});
assert.deepEqual(
  normalizedExistingGroupAction.read.fieldGroups.map((group) => group.groupId),
  [
    "execute_task_task_core",
    "execute_task_acceptance_and_concepts",
    "execute_task_frontend_guidance",
    "execute_task_frontend_closure_details",
    "execute_task_frontend_binding_details",
    "execute_task_runtime_guidance",
    "execute_task_dependency_context",
  ],
  "existing broad TaskExecution fieldGroups must be rewritten into layered groups.",
);
assert.equal(
  normalizedExistingGroupAction.read.fieldGroups.find((group) => group.groupId === "execute_task_frontend_binding_details")?.required,
  false,
  "existing broad TaskExecution fieldGroups must move frontendBackendBindings out of required reads.",
);
assertNoCoveredFieldPaths(normalizedExistingGroupAction.read.fieldGroups, "existing read.fieldGroups");

const brainstormAction = brainstormSessionAgentActionContract({
  candidateFile: ".loom/tmp/brainstorm-candidate.json",
  submitCommand: { name: "brainstorm accept", argv: ["brainstorm", "accept", "--candidate-file", "{candidateFile}"] },
});
assertNoCoveredFieldPaths(brainstormAction.read.fieldGroups, "brainstorm read.fieldGroups");
const brainstormScopeCoreGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_phase_scope_core");
const brainstormScopeAuthorityGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_phase_scope_authority");
const brainstormConceptGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_concept_grounding_context");
const brainstormKeywordHintsGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_keyword_hints_advisory");
const brainstormCandidateWriteControlsGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_write_controls");
const brainstormCandidateSchemaIdentityGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_schema_identity");
const brainstormCandidateSchemaScopeGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_schema_scope");
const brainstormCandidateSchemaConceptsGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_schema_concepts");
const brainstormCandidateSchemaFrontendGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_schema_frontend");
const brainstormCandidateSchemaHandoffGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_schema_handoff");
const brainstormCandidateFinalSummaryRulesGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_final_summary_rules");
const brainstormCandidateRequirementSemanticRulesGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_requirement_semantic_rules");
const brainstormCandidateSelfReviewRulesGroup = brainstormAction.read.fieldGroups.find((group) => group.groupId === "brainstorm_session_candidate_self_review_rules");
assert.ok(brainstormScopeCoreGroup, "Brainstorm readPlan must include a phase_scope core group.");
assert.ok(brainstormScopeAuthorityGroup, "Brainstorm readPlan must include a phase_scope authority group.");
assert.ok(brainstormConceptGroup, "Brainstorm readPlan must include a concept grounding context group.");
assert.ok(brainstormKeywordHintsGroup, "Brainstorm readPlan must expose keywordHints as a separate advisory group.");
assert.ok(brainstormCandidateWriteControlsGroup, "Brainstorm readPlan must include a candidate write controls group.");
assert.ok(brainstormCandidateSchemaIdentityGroup, "Brainstorm readPlan must include a candidate schema identity group.");
assert.ok(brainstormCandidateSchemaScopeGroup, "Brainstorm readPlan must include a candidate schema scope group.");
assert.ok(brainstormCandidateSchemaConceptsGroup, "Brainstorm readPlan must include a candidate schema concepts group.");
assert.ok(brainstormCandidateSchemaFrontendGroup, "Brainstorm readPlan must include a candidate schema frontend group.");
assert.ok(brainstormCandidateSchemaHandoffGroup, "Brainstorm readPlan must include a candidate schema handoff group.");
assert.ok(brainstormCandidateFinalSummaryRulesGroup, "Brainstorm readPlan must include a candidate final summary rules group.");
assert.ok(brainstormCandidateRequirementSemanticRulesGroup, "Brainstorm readPlan must include a candidate requirement semantic rules group.");
assert.ok(brainstormCandidateSelfReviewRulesGroup, "Brainstorm readPlan must include a candidate self-review rules group.");
assert.deepEqual(
  brainstormScopeCoreGroup.fields,
  [
    "agentAction.instruction",
    "agentAction.stopConditions",
    "originalRequest",
    "contextRefs",
    "sourceFieldAccessHints",
    "firstClarificationGate",
    "clarificationConversationProtocol.mode",
    "clarificationConversationProtocol.oneTopicPerTurn",
    "clarificationConversationProtocol.maxOptionsPerQuestion",
    "clarificationConversationProtocol.avoidSchemaLanguageToUser",
    "clarificationConversationProtocol.requiredBlocks",
    "clarificationConversationProtocol.blockConfirmationRules.phase_scope",
    "rules.phaseScopeOptionComparison",
    "clarificationGuidance",
    "riskGuidance",
    "confirmationRules",
  ],
  "Brainstorm phase_scope core group must contain only current scope conversation controls.",
);
assert.deepEqual(
  brainstormScopeAuthorityGroup.fields,
  ["requirementContext.sourceItems", "requirementContext.normalizedText"],
  "Initial Brainstorm phase_scope authority group must contain original requirement authority fields.",
);
assert.deepEqual(
  brainstormConceptGroup.fields,
  [
    "conceptGroundingRequest",
    "clarificationConversationProtocol.blockExecutionRules",
    "clarificationConversationProtocol.blockConfirmationRules.concept_grounding",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.scopeItemCoverageContract",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.objectOperationContract",
  ],
  "Brainstorm concept group must hold concept-only context and narrow concept rules.",
);
assert.deepEqual(
  brainstormCandidateWriteControlsGroup.fields,
  ["agentAction.write", "agentAction.submit", "agentAction.schema", "outputContract.candidateFile", "outputContract.schemaRef", "generationProtocol", "enumRefs"],
  "Brainstorm write controls must be delayed and avoid reading the full outputContract.",
);
assert.deepEqual(
  brainstormCandidateSchemaIdentityGroup.fields,
  [
    "outputContract.schemaShape.schemaVersion",
    "outputContract.schemaShape.candidateId",
    "outputContract.schemaShape.brainstormRunId",
    "outputContract.schemaShape.deliveryId",
    "outputContract.schemaShape.phaseId",
    "outputContract.schemaShape.status",
    "outputContract.schemaShape.requestSummary",
    "outputContract.schemaShape.sources",
  ],
  "Brainstorm candidate identity schema fields must be a narrow schema projection.",
);
assert.deepEqual(
  brainstormCandidateSchemaScopeGroup.fields,
  [
    "outputContract.schemaShape.scope",
    "outputContract.schemaShape.roadmap",
    "outputContract.schemaShape.phasePlan",
    "outputContract.schemaShape.acceptance",
  ],
  "Brainstorm candidate scope schema fields must be isolated from unrelated schema sections.",
);
assert.deepEqual(
  brainstormCandidateSchemaConceptsGroup.fields,
  [
    "outputContract.schemaShape.domainModel",
    "outputContract.schemaShape.conceptGrounding",
    "outputContract.schemaShape.conceptConfirmation",
    "outputContract.schemaShape.clarificationProgress",
  ],
  "Brainstorm candidate concept schema fields must be isolated from unrelated schema sections.",
);
assert.deepEqual(
  brainstormCandidateSchemaFrontendGroup.fields,
  ["outputContract.schemaShape.frontendExperience"],
  "Brainstorm candidate frontend schema fields must be isolated from unrelated schema sections.",
);
assert.deepEqual(
  brainstormCandidateSchemaHandoffGroup.fields,
  ["outputContract.schemaShape.handoff"],
  "Brainstorm candidate handoff fields must contain only delayed handoff schema.",
);
assert.deepEqual(
  brainstormCandidateFinalSummaryRulesGroup.fields,
  [
    "clarificationConversationProtocol.blockConfirmationRules.final_summary",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.appliesWhenAgentFinds",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.requiredUserVisibleTopicsWhenApplicable",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.notApplicableRule",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.candidateFieldMapping",
    "rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract.finalSummaryReviewContract",
  ],
  "Brainstorm final summary rules must be narrow rule sections instead of the full rules object.",
);
assert.ok(
  brainstormCandidateFinalSummaryRulesGroup.whenToRead.includes("before presenting the final_summary block"),
  "Brainstorm final summary review rules must be read before presenting final_summary.",
);
assert.deepEqual(
  brainstormCandidateRequirementSemanticRulesGroup.fields,
  ["rules.requirementSemanticGrounding.compactRules"],
  "Brainstorm requirement semantic rules must default to the compact rule list.",
);
assert.deepEqual(
  brainstormCandidateSelfReviewRulesGroup.fields,
  ["rules.candidateSelfReview", "rules.nextPhasePreviewGeneration"],
  "Brainstorm self-review rules must be narrow rule sections instead of candidateRules.",
);
for (const candidateGroup of [
  brainstormCandidateWriteControlsGroup,
  brainstormCandidateSchemaIdentityGroup,
  brainstormCandidateSchemaScopeGroup,
  brainstormCandidateSchemaConceptsGroup,
  brainstormCandidateSchemaFrontendGroup,
  brainstormCandidateSchemaHandoffGroup,
  brainstormCandidateFinalSummaryRulesGroup,
  brainstormCandidateRequirementSemanticRulesGroup,
  brainstormCandidateSelfReviewRulesGroup,
]) {
  assert.ok(
    candidateGroup.whenToRead.includes("final_summary"),
    `${candidateGroup.groupId} must only be read after final_summary confirmation.`,
  );
}
assert.ok(
  brainstormCandidateWriteControlsGroup.whenToRead.includes("Do not read this group after phase_scope"),
  "Brainstorm write controls must not be read after early Brainstorm block confirmations.",
);
assert.equal(
  brainstormScopeCoreGroup.fields.includes("outputContract") ||
    brainstormScopeCoreGroup.fields.includes("agentAction") ||
    brainstormScopeCoreGroup.fields.includes("clarificationConversationProtocol") ||
    brainstormScopeCoreGroup.fields.includes("rules") ||
    brainstormScopeCoreGroup.fields.includes("generationProtocol") ||
    brainstormScopeCoreGroup.fields.includes("enumRefs"),
  false,
  "Brainstorm phase_scope core group must not read candidate write fields.",
);
assert.equal(
  allFields(brainstormAction.read.fieldGroups).includes("outputContract") ||
    allFields(brainstormAction.read.fieldGroups).includes("outputContract.schemaShape") ||
    allFields(brainstormAction.read.fieldGroups).includes("outputContract.schemaShape.candidateRules") ||
    allFields(brainstormAction.read.fieldGroups).includes("agentAction") ||
    allFields(brainstormAction.read.fieldGroups).includes("clarificationConversationProtocol") ||
    allFields(brainstormAction.read.fieldGroups).includes("requestManifest") ||
    allFields(brainstormAction.read.fieldGroups).includes("rules"),
  false,
  "Brainstorm readPlan must not read full outputContract, schemaShape, candidateRules, agentAction, protocol, requestManifest, or rules.",
);
assert.equal(
  brainstormScopeAuthorityGroup.fields.includes("keywordHints"),
  false,
  "Brainstorm phase_scope authority group must not read complete advisory keywordHints.",
);
assert.equal(brainstormKeywordHintsGroup.required, false, "Brainstorm keyword hints group must be optional.");
assert.deepEqual(brainstormKeywordHintsGroup.fields, ["keywordHints.compact"], "Brainstorm keyword hints group must read only the compact keywordHints projection.");
assert.deepEqual(
  brainstormKeywordHintsGroup.readCommand.argv,
  ["inspect", "--request", "{requestRef}", "--field", "keywordHints.compact"],
  "Brainstorm keyword hints group must have a targeted inspect command.",
);
assert.ok(
  brainstormKeywordHintsGroup.whenToRead.includes("Do not read this group by default for phase_scope"),
  "Brainstorm keyword hints group must prevent default phase_scope reads.",
);
assert.ok(
  brainstormKeywordHintsGroup.purpose.includes("never use this group as scope or acceptance authority"),
  "Brainstorm keyword hints group must preserve advisory-only semantics.",
);

const normalizedOldBrainstormAction = normalizeAgentActionForFieldGroups({
  actionKind: "brainstorm_session",
  read: {
    required: [],
    fieldGroups: [
      {
        groupId: "brainstorm_session_core",
        required: true,
        fields: ["agentAction", "contextRefs"],
      },
      {
        groupId: "brainstorm_session_optional_context",
        required: false,
        fields: ["phaseContinuationContext", "keywordHints", "latestRepositoryContext"],
      },
    ],
    displayPolicy: "compact",
  },
});
assert.deepEqual(
  normalizedOldBrainstormAction.read.fieldGroups.map((group) => ({ groupId: group.groupId, fields: group.fields })),
  [
    { groupId: "brainstorm_session_phase_scope_core", fields: ["agentAction.instruction", "agentAction.stopConditions", "contextRefs"] },
    {
      groupId: "brainstorm_session_phase_scope_authority",
      fields: [
        "phaseContinuationContext",
        "latestRepositoryContext.requestLens",
        "latestRepositoryContext.repoOverview",
        "latestRepositoryContext.existingCapabilities",
        "latestRepositoryContext.roadmapImplications",
        "latestRepositoryContext.warnings",
        "latestRepositoryContext.contextQuality",
      ],
    },
    { groupId: "brainstorm_session_keyword_hints_advisory", fields: ["keywordHints.compact"] },
    {
      groupId: "brainstorm_session_candidate_write_controls",
      fields: ["agentAction.write", "agentAction.submit", "agentAction.schema"],
    },
  ],
  "CLI normalization must rewrite old Brainstorm groups into gate-scoped groups.",
);

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "loom-inspect-"));
const projectRoot = tmp;
const initRoot = fs.mkdtempSync(path.join(os.tmpdir(), "loom-inspect-init-"));

{
  const { result, parsed } = runCli(initRoot, [
    "init",
    "--project-root", initRoot,
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim().includes("\n"), false, "compact non-inspect output must also be minified single-line JSON.");
  assert.equal(parsed.ok, true);
  assert.equal(parsed.command, "init");
}

writeJson(path.join(tmp, ".loom/requests/request.refs/task.json"), {
  taskId: "task-1",
  groupId: "group-1",
  title: "Task one",
  taskKind: "feature_increment",
  implementationActions: ["modify_existing"],
  objective: "Full task objective text.",
  dependsOn: [],
  scopeRefs: ["scope-1"],
  acceptanceRefs: ["AC-1"],
  writeBoundary: {
    forbiddenPaths: [".loom"],
    artifactRefs: {
      modules: ["module-1"],
      entities: [],
      interfaces: [],
      userFlows: [],
      stateMachines: [],
      decisions: [],
      risks: [],
    },
  },
  verificationIntents: [{
    verificationId: "VI-1",
    acceptanceRefs: ["AC-1"],
    behavior: "Verify task one.",
    preferredEvidence: ["static_check"],
    acceptableEvidence: ["static_check"],
  }],
  conceptRefs: [],
  conceptResponsibilities: [],
  conceptVerificationIntents: [],
});
writeJson(path.join(tmp, ".loom/requests/request.refs/source-context.json"), {
  acceptanceSnapshot: [
    { acceptanceId: "AC-1", statement: "Full acceptance statement." },
  ],
  extra: "must not be returned for acceptanceSnapshot field",
});
writeJson(path.join(tmp, ".loom/requests/request.refs/output-contract.json"), {
  schemaShape: {
    requiredTopLevelFields: ["schemaVersion", "taskId"],
    runtimeDeliveryEvidence: {
      requiredCheckIds: ["rd-check-runtime-surface"],
    },
  },
  largeSibling: "must not be returned for schemaShape field",
});
writeJson(path.join(tmp, ".loom/requests/request.refs/task-concept-grounding.json"), {
  concepts: [
    { conceptId: "concept-1", meaning: "Full concept grounding text." },
  ],
});
writeJson(path.join(tmp, ".loom/requests/request.refs/agent-action.json"), action);
const technicalBaselineAction = agentActionContract({
  actionKind: "generate_candidate",
  instruction: "Generate TechnicalBaseline.",
  read: {
    required: ["contextRefs.brainstormContractRef", "currentPhaseLens", "enumRefs", "outputContract.schemaShape"],
    optional: ["contextRefs.latestRepositoryContextRef", "contextRefs.previousTechnicalBaselineRef"],
    displayPolicy: "compact",
  },
  write: { rules: [] },
  submit: {
    command: { name: "technical-baseline accept", argv: ["technical-baseline", "accept"] },
    requiredArgs: [],
    placeholders: {},
    runAfter: "candidateFile exists",
  },
  schema: { primary: "TechnicalBaseline", shapeLocation: "outputContract.schemaShape", enumLocation: "enumRefs" },
  stopConditions: [],
});
writeJson(path.join(tmp, ".loom/requests/tbr.refs/agent-action.json"), technicalBaselineAction);

const repositoryContextAction = agentActionContract({
  actionKind: "repository_context",
  instruction: "Generate RepositoryContext.",
  read: {
    required: ["this request", "referencedArtifactReadGuide", "scanPurpose", "generationRules", "enumRefs", "outputContract.schemaShape", "outputContract.referenceRules"],
    optional: ["source.brainstormContractRef", "source.technicalBaselineRef", "project files"],
    displayPolicy: "compact",
  },
  write: { rules: [] },
  submit: {
    command: { name: "repository-context accept", argv: ["repository-context", "accept"] },
    requiredArgs: [],
    placeholders: {},
    runAfter: "candidateFile exists",
  },
  schema: { primary: "RepositoryContext", shapeLocation: "outputContract.schemaShape", enumLocation: "enumRefs" },
  stopConditions: [],
});
{
  const groups = repositoryContextAction.read.fieldGroups;
  const fields = groups.flatMap((group) => group.fields);
  assert.ok(groups.some((group) => group.groupId === "repository_context_scan_scope"), "RepositoryContext must expose scan_scope group.");
  assert.ok(groups.some((group) => group.groupId === "repository_context_generation_rules"), "RepositoryContext must expose generation_rules group.");
  assert.ok(groups.some((group) => group.groupId === "repository_context_candidate_contract"), "RepositoryContext must expose candidate_contract group.");
  assert.equal(fields.includes("project files"), false, "Natural-language labels must not become inspect fields.");
  assert.equal(fields.includes("this request"), false, "Self request labels must not become inspect fields.");
  assertNoCoveredFieldPaths(groups, "RepositoryContext read plan");
}
writeJson(path.join(tmp, ".loom/context/requirement-context.json"), {
  sourceItems: [
    { itemId: "req-file-001", kind: "file", path: "/tmp/request.pdf" },
  ],
  largeSibling: "must not be returned for sourceItems field",
});
writeJson(path.join(tmp, ".loom/context/delivery-context.json"), {
  sources: [
    { sourceId: "req-001", type: "pdf", path: "/tmp/request.pdf" },
  ],
  largeSibling: "must not be returned for sources field",
});
writeJson(path.join(tmp, ".loom/context/keyword-hints.json"), {
  global: [{ term: "settlement", score: 9 }],
});
writeJson(path.join(tmp, ".loom/context/repository-context.json"), {
  repoOverview: { summary: "Current repository summary." },
  existingCapabilities: [{ capabilityId: "cap-1", summary: "Implemented capability." }],
  roadmapImplications: [{ implicationId: "roadmap-1", summary: "Follow-up implication." }],
  warnings: [],
  contextQuality: { coverage: "focused", confidence: "medium", warnings: [] },
  structureSignals: { largeSibling: "must not be returned for repoOverview field" },
});
fs.writeFileSync(path.join(tmp, ".loom/context/normalized.txt"), "Full normalized requirement text.\n");
writeJson(path.join(tmp, ".loom/requests/request.json"), {
  schemaVersion: "1.0",
  requestId: "request-1",
  contextRefs: {
    requirementContextRef: ".loom/context/requirement-context.json",
    normalizedRequirementTextRef: ".loom/context/normalized.txt",
    keywordHintsRef: ".loom/context/keyword-hints.json",
    deliveryContextRef: ".loom/context/delivery-context.json",
    latestRepositoryContextRef: ".loom/context/repository-context.json",
  },
  requestManifest: {
    schemaVersion: "1.0",
    refFirst: true,
    protocolAuthority: "request_manifest_refs",
    refs: {
      task: { refKey: "taskRef", ref: ".loom/requests/request.refs/task.json" },
      sourceContext: { refKey: "sourceContextRef", ref: ".loom/requests/request.refs/source-context.json" },
      outputContract: { refKey: "outputContractRef", ref: ".loom/requests/request.refs/output-contract.json" },
      taskConceptGrounding: { refKey: "taskConceptGroundingRef", ref: ".loom/requests/request.refs/task-concept-grounding.json" },
      agentAction: { refKey: "agentActionRef", ref: ".loom/requests/request.refs/agent-action.json" },
    },
  },
});
writeJson(path.join(tmp, ".loom/requests/tbr.json"), {
  schemaVersion: "1.0",
  requestId: "tbr-1",
  contextRefs: {
    brainstormContractRef: ".loom/context/brainstorm-contract.json",
  },
  currentPhaseLens: { phaseId: "phase-1" },
  requestManifest: {
    schemaVersion: "1.0",
    refFirst: true,
    protocolAuthority: "request_manifest_refs",
    refs: {
      agentAction: { refKey: "agentActionRef", ref: ".loom/requests/tbr.refs/agent-action.json" },
      outputContract: { refKey: "outputContractRef", ref: ".loom/requests/request.refs/output-contract.json" },
    },
  },
});

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "task",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim().includes("\n"), false, "compact inspect output must be minified single-line JSON.");
  assert.equal(parsed.ok, true);
  assert.equal(parsed.data.fields.task.value.taskId, "task-1");
  assert.equal(parsed.data.fields.task.value.objective, "Full task objective text.");
  assert.equal(parsed.data.fields.task.value.writeBoundary.artifactRefs.modules[0], "module-1");
  assert.equal(parsed.data.fields.task.fieldRead.resolvedRefKey, "task");
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--values-only",
    "--field", "task,sourceContext.acceptanceSnapshot",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(parsed.ok, true);
  assert.equal(parsed.data.requestedFields, undefined, "values-only inspect must omit requestedFields metadata.");
  assert.equal(parsed.data.fields.task.value, undefined, "values-only inspect must omit field wrapper objects.");
  assert.equal(parsed.data.fields.task.fieldRead, undefined, "values-only inspect must omit fieldRead metadata.");
  assert.equal(parsed.data.fields.task.taskId, "task-1");
  assert.deepEqual(parsed.data.fields["sourceContext.acceptanceSnapshot"], [
    { acceptanceId: "AC-1", statement: "Full acceptance statement." },
  ]);
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/tbr.json",
    "--field", "agentAction",
  ]);
  assert.equal(result.status, 0, result.stderr);
  const fields = parsed.data.fields.agentAction.value.read.fieldGroups.flatMap((group) => group.fields);
  assert.equal(fields.includes("contextRefs.latestRepositoryContextRef"), false, "request-aware agentAction inspect must remove unavailable optional latestRepositoryContextRef.");
  assert.equal(fields.includes("contextRefs.previousTechnicalBaselineRef"), false, "request-aware agentAction inspect must remove unavailable optional previousTechnicalBaselineRef.");
  assert.ok(fields.includes("contextRefs.brainstormContractRef"), "request-aware agentAction inspect must preserve required present contextRefs.");
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/tbr.json",
    "--field", "contextRefs.latestRepositoryContextRef,contextRefs.previousTechnicalBaselineRef",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(parsed.data.fields["contextRefs.latestRepositoryContextRef"].status, "not_available");
  assert.equal(parsed.data.fields["contextRefs.latestRepositoryContextRef"].value, null);
  assert.equal(parsed.data.fields["contextRefs.previousTechnicalBaselineRef"].status, "not_available");
  assert.equal(parsed.data.fields["contextRefs.previousTechnicalBaselineRef"].fieldRead.unavailableReason, "contextRef is not present on this request.");
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "task",
    "--json",
  ], { LOOM_COMPACT_OUTPUT: "" });
  assert.equal(result.status, 0, result.stderr);
  assert.ok(result.stdout.trim().includes("\n"), "non-compact inspect --json output must remain pretty-printed for human debugging.");
  assert.equal(parsed.ok, true);
  assert.equal(parsed.data.fields.task.value.objective, "Full task objective text.");
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "task,sourceContext.acceptanceSnapshot,outputContract.schemaShape",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(parsed.ok, true);
  assert.deepEqual(parsed.data.requestedFields, ["task", "sourceContext.acceptanceSnapshot", "outputContract.schemaShape"]);
  assert.equal(parsed.data.field, undefined, "inspect must not return legacy top-level field.");
  assert.equal(parsed.data.value, undefined, "inspect must not return legacy top-level value.");
  assert.equal(parsed.data.fieldRead, undefined, "inspect must not return legacy top-level fieldRead.");
  assert.equal(parsed.data.fields.task.value.objective, "Full task objective text.");
  assert.deepEqual(parsed.data.fields["sourceContext.acceptanceSnapshot"].value, [
    { acceptanceId: "AC-1", statement: "Full acceptance statement." },
  ]);
  assert.deepEqual(parsed.data.fields["outputContract.schemaShape"].value.runtimeDeliveryEvidence.requiredCheckIds, ["rd-check-runtime-surface"]);
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "requirementContext.sourceItems",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(parsed.data.fields["requirementContext.sourceItems"].value, [
    { itemId: "req-file-001", kind: "file", path: "/tmp/request.pdf" },
  ]);
  assert.equal(parsed.data.fields["requirementContext.sourceItems"].fieldRead.resolvedRefKey, "contextRefs.requirementContextRef");
  assert.equal(JSON.stringify(parsed.data.fields["requirementContext.sourceItems"].value).includes("largeSibling"), false);
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "deliveryContext.sources",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(parsed.data.fields["deliveryContext.sources"].value, [
    { sourceId: "req-001", type: "pdf", path: "/tmp/request.pdf" },
  ]);
  assert.equal(parsed.data.fields["deliveryContext.sources"].fieldRead.resolvedRefKey, "contextRefs.deliveryContextRef");
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "requirementContext.normalizedText",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(parsed.data.fields["requirementContext.normalizedText"].value, "Full normalized requirement text.\n");
  assert.equal(parsed.data.fields["requirementContext.normalizedText"].fieldRead.resolvedRefKey, "contextRefs.normalizedRequirementTextRef");
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "latestRepositoryContext.repoOverview,latestRepositoryContext.existingCapabilities,latestRepositoryContext.warnings",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(parsed.data.fields["latestRepositoryContext.repoOverview"].value, { summary: "Current repository summary." });
  assert.deepEqual(parsed.data.fields["latestRepositoryContext.existingCapabilities"].value, [{ capabilityId: "cap-1", summary: "Implemented capability." }]);
  assert.deepEqual(parsed.data.fields["latestRepositoryContext.warnings"].value, []);
  assert.equal(parsed.data.fields["latestRepositoryContext.repoOverview"].fieldRead.resolvedRefKey, "contextRefs.latestRepositoryContextRef");
  assert.equal(parsed.data.fields["latestRepositoryContext.repoOverview"].fieldRead.selector, ".repoOverview");
  assert.equal(JSON.stringify(parsed.data.fields["latestRepositoryContext.repoOverview"].value).includes("largeSibling"), false);
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "sourceContext.acceptanceSnapshot",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(parsed.data.fields["sourceContext.acceptanceSnapshot"].value, [
    { acceptanceId: "AC-1", statement: "Full acceptance statement." },
  ]);
  assert.equal(JSON.stringify(parsed.data.fields["sourceContext.acceptanceSnapshot"].value).includes("largeSibling"), false);
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "phaseScopePrompt",
  ]);
  assert.notEqual(result.status, 0, "inspect with an unknown legacy field must fail.");
  assert.equal(parsed.ok, false);
  assert.equal(parsed.error.code, "INVALID_ARGUMENT");
  const recovery = parsed.error.details.inspectRecovery;
  assert.equal(recovery.status, "field_not_found_use_request_read_plan");
  assert.equal(recovery.agentActionSource, "request_manifest_ref");
  assert.equal(recovery.agentActionRef, ".loom/requests/request.refs/agent-action.json");
  assert.ok(Array.isArray(recovery.availableFieldGroups) && recovery.availableFieldGroups.length > 0, "inspect recovery must expose available fieldGroups.");
  const firstGroup = recovery.availableFieldGroups[0];
  assert.equal(firstGroup.groupId, "execute_task_task_core");
  assert.equal(firstGroup.fields.includes("task.objective"), true, "inspect recovery must recommend task subfields.");
  assert.equal(firstGroup.fields.includes("task"), false, "inspect recovery must not recommend broad task reads.");
  assert.deepEqual(
    firstGroup.readCommand.argv,
    ["inspect", "--request", ".loom/requests/request.json", "--field", firstGroup.fields.join(",")],
    "inspect recovery availableFieldGroups must preserve readCommand with the real requestRef.",
  );
  assert.deepEqual(
    recovery.recommendedNextRead.commandInvocation.argv,
    ["inspect", "--request", ".loom/requests/request.json", "--field", firstGroup.fields.join(",")],
    "inspect recovery must return an executable grouped inspect command with the real requestRef.",
  );
  assert.ok(
    recovery.doNot.some((rule) => rule.includes("Do not guess old wrapper fields")),
    "inspect recovery must stop agents from guessing legacy wrapper fields.",
  );
  assert.ok(
    recovery.doNot.some((rule) => rule.includes("Do not run broad searches")),
    "inspect recovery must stop broad recovery searches.",
  );
  assert.equal(JSON.stringify(parsed).includes("Full task objective text."), false, "field-not-found recovery must not leak request content.");

  const recommended = runCli(projectRoot, [
    ...recovery.recommendedNextRead.commandInvocation.argv,
    "--project-root", projectRoot,
  ]);
  assert.equal(recommended.result.status, 0, recommended.result.stderr);
  assert.equal(recommended.parsed.ok, true);
  assert.equal(recommended.parsed.data.fields["task.objective"].value, "Full task objective text.");
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
    "--field", "outputContract.schemaShape.runtimeDeliveryEvidence.requiredCheckIds",
  ]);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(parsed.data.fields["outputContract.schemaShape.runtimeDeliveryEvidence.requiredCheckIds"].value, ["rd-check-runtime-surface"]);
}

{
  const { result, parsed } = runCli(projectRoot, [
    "inspect",
    "--project-root", projectRoot,
    "--request", ".loom/requests/request.json",
  ]);
  assert.notEqual(result.status, 0, "inspect without --field must fail instead of returning a full request.");
  assert.equal(parsed.ok, false);
  assert.equal(parsed.error.code, "INVALID_ARGUMENT");
  assert.equal(JSON.stringify(parsed).includes("Full task objective text."), false, "missing-field error must not leak request content.");
}

for (const file of [
  "plugins/codex/skills/loom/SKILL.md",
  "plugins/claude-code/skills/loom/SKILL.md",
  "plugins/opencode/.opencode/commands/loom.md",
  "plugins/codex/skills/loom-deploy/SKILL.md",
  "plugins/claude-code/skills/loom-deploy/SKILL.md",
  "plugins/opencode/.opencode/commands/loom-deploy.md",
]) {
  assertIncludes(file, "agentAction.read.fieldGroups", `${file}: adapter must mention structured read field groups.`);
  assertIncludes(file, "inspect", `${file}: adapter must mention inspect.`);
  assertIncludes(file, "fallback", `${file}: adapter must mention fallback.`);
}

assertIncludes("src/core/operations/output-policy.ts", "agentAction.read.fieldGroups", "output policy must route agents through read field groups.");
assertIncludes("src/core/operations/output-policy.ts", "inspect readCommand", "output policy must mention inspect readCommand.");
assertNotIncludes("src/core/operations/output-policy.ts", "--limit", "request read protocol must not use content limits.");
assertIncludes("src/core/operations/tasks.ts", "frontendImplementationOrganizationRules", "TaskPlan/TaskExecution must expose frontend responsibility-boundary rules.");
assertIncludes("src/core/operations/tasks.ts", "UI/view", "frontend organization rules must name UI/view responsibility.");
assertIncludes("src/core/operations/tasks.ts", "API/service interaction", "frontend organization rules must name API/service responsibility.");
assertIncludes("src/core/operations/tasks.ts", "state/feedback handling", "frontend organization rules must name state/feedback responsibility.");
assertIncludes("src/core/operations/tasks.ts", "verification/test evidence", "frontend organization rules must name verification/test responsibility.");

console.log("readPlan + inspect verification passed");
