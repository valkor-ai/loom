#!/usr/bin/env node

const assert = require("node:assert/strict");

const { runCli } = require("../harness/cli");
const { readRepoFile } = require("../harness/root");
const { hydrateRequest, readProjectJson: readJson, tempProject } = require("../harness/files");

function includes(text, needle, message) {
  assert.ok(String(text).includes(needle), message);
}

const projectRoot = tempProject("loom-brainstorm-business-detail-");
runCli(["init"], projectRoot);

const started = runCli([
  "brainstorm",
  "start",
  "--request",
  "Build an internal account operations system. Staff create accounts, review account applications, block invalid operations with clear reasons, update account status, and query existing accounts before operating on them.",
], projectRoot);

const request = hydrateRequest(projectRoot, readJson(projectRoot, started.requestPath ?? started.requestRef));
assert.equal(request.requestType, "brainstorm_session");

assert.ok(
  request.firstClarificationGate.mustPresentBeforeAccept.includes("businessObjectOperationSummary"),
  "Brainstorm must require businessObjectOperationSummary before accept.",
);

const blockRules = request.clarificationConversationProtocol.blockExecutionRules.join("\n");
includes(blockRules, "map every confirmed scope.included item", "concept_grounding must map every included scope item before confirmation.");
includes(blockRules, "scope-item coverage summary", "concept_grounding must require scope-item coverage summary.");
includes(blockRules, "Do not force every dimension onto every scope item", "coverage check must not force fixed dimensions onto all scopes.");
includes(blockRules, "Do not use a fixed capability taxonomy", "coverage check must avoid test-scenario categories.");
includes(blockRules, "does not appear in the scope-item coverage summary", "missing included scope items must block progression.");
includes(blockRules, "applicable objects or subjects, actions or behaviors, inputs or fields", "concept_grounding must ask for applicable scope item details.");
includes(blockRules, "identity fields, input fields, display fields, relationship fields, state fields", "field-set categories must be explicit and generic.");
includes(blockRules, "operation input, preconditions, validation rules, blocking conditions, blocking reasons", "operation rule details must be explicit.");
includes(blockRules, "Do not present only noun definitions", "concept grounding must not degrade to noun glossary.");
includes(blockRules, "structured BrainstormCandidate fields", "confirmed details must be preserved in structured fields instead of final_summary.");

const conceptRule = request.clarificationConversationProtocol.blockConfirmationRules.concept_grounding;
includes(conceptRule, "covers every confirmed scope.included item", "concept confirmation must require every included scope item to be covered.");
includes(conceptRule, "unresolved notes", "concept confirmation must support unresolved notes instead of silent omission.");
includes(conceptRule, "inputs or fields", "concept confirmation must include applicable inputs or fields.");
includes(conceptRule, "actions or behaviors", "concept confirmation must include applicable actions or behaviors.");
includes(conceptRule, "visible feedback", "concept confirmation must include visible feedback.");

const semanticContract = request.rules.requirementSemanticGrounding.finalSummaryBusinessDetailContract;
assert.equal(
  semanticContract.confirmedBlockDetailRetentionContract.sourceOfTruth,
  "all_confirmed_brainstorm_blocks_plus_final_summary_corrections",
  "confirmed block detail retention must use all confirmed Brainstorm blocks as source.",
);
assert.ok(
  semanticContract.confirmedBlockDetailRetentionContract.candidateFields.includes("scope.included[].items"),
  "retention contract must preserve phase_scope details in existing scope fields.",
);
assert.ok(
  semanticContract.confirmedBlockDetailRetentionContract.candidateFields.includes("conceptGrounding.phaseConceptGrounding.concepts[].explanation"),
  "retention contract must preserve concept_grounding details in existing concept fields.",
);
assert.ok(
  semanticContract.confirmedBlockDetailRetentionContract.rules.some((rule) => rule.includes("For concept_grounding, preserve confirmed business scenario")),
  "retention rules must preserve concept_grounding business details.",
);
assert.ok(
  semanticContract.finalSummaryReviewContract.rules.some((rule) => rule.includes("not be required to repeat every confirmed object")),
  "final summary review rules must not require exhaustive detail repetition.",
);
assert.equal(semanticContract.scopeItemCoverageContract.owningBlock, "concept_grounding");
assert.ok(
  semanticContract.scopeItemCoverageContract.candidateFields.includes("scope.included[].items"),
  "scope-item coverage must map back to existing scope.included[].items.",
);
assert.ok(
  semanticContract.scopeItemCoverageContract.rules.some((rule) => rule.includes("Every scope.included item")),
  "scope-item coverage contract must require every included item to land in existing fields.",
);
assert.equal(semanticContract.objectOperationContract.owningBlock, "concept_grounding");
assert.ok(
  semanticContract.objectOperationContract.candidateFields.includes("conceptGrounding.phaseConceptGrounding.concepts[].explanation"),
  "object-operation details must map to existing ConceptGrounding explanation fields.",
);
assert.ok(
  semanticContract.objectOperationContract.candidateFields.includes("domainModel.businessFlows[].summary"),
  "object-operation details must map to existing business flow summaries.",
);
assert.ok(
  semanticContract.requiredUserVisibleTopicsWhenApplicable.includes("business-rule checklist from confirmed business understanding including concrete objects, relationships, operations, field-set headlines, state changes, blocking rules, success outcomes, and high-risk misunderstanding guards when applicable"),
  "final summary contract must surface concrete business-rule checklist coverage.",
);
assert.ok(
  semanticContract.requiredUserVisibleTopicsWhenApplicable.includes("explicit final_summary corrections that must be written back to structured fields"),
  "final summary contract must route corrections back to structured fields.",
);

const candidateRules = request.outputContract.schemaShape.candidateRules.join("\n");
includes(candidateRules, "The accepted BrainstormCandidate must be built from all user-confirmed Brainstorm blocks", "candidate rules must not use final_summary as the only source.");
includes(candidateRules, "For phase_scope, preserve the confirmed option's included scope", "candidate rules must preserve phase_scope details.");
includes(candidateRules, "For concept_grounding, preserve confirmed business scenario", "candidate rules must preserve concept_grounding details.");
includes(candidateRules, "Every scope.included item should be represented", "candidate rules must require each included scope item to be represented.");
includes(candidateRules, "confirmed scope.included item has been considered", "candidate self-review must check scope item coverage.");
includes(candidateRules, "Store confirmed object-operation details in existing BrainstormCandidate fields", "candidate rules must store details in existing fields.");
includes(candidateRules, "domainModel.businessFlows[].summary should describe object operation flow steps", "businessFlows must carry operation details.");
includes(candidateRules, "conceptGrounding.phaseConceptGrounding.concepts[].explanation should capture high-risk object semantics", "ConceptGrounding must carry object semantics.");

const conceptShape = request.outputContract.schemaShape.conceptGrounding.phaseConceptGrounding.concepts[0].explanation;
includes(conceptShape, "key field meaning", "schema shape must guide field meaning in concept explanation.");
includes(conceptShape, "inputs or fields", "schema shape must guide applicable inputs or fields in concept explanation.");
includes(conceptShape, "visible feedback", "schema shape must guide visible feedback in concept explanation.");

const contractsSource = readRepoFile("core/operations/contracts.ts");
includes(contractsSource, "objectOperationDetailRules", "AAC requirement transfer must include objectOperationDetailRules.");
includes(contractsSource, "Every currentPhaseScope.included item should remain traceable", "AAC transfer must preserve scope item traceability.");
includes(contractsSource, "Represent business objects as entities or reference projections", "AAC domain_contract mapping must preserve business objects.");
includes(contractsSource, "Represent object operations as userFlows/stateMachines", "AAC behavior mapping must preserve operations.");

const tasksSource = readRepoFile("core/operations/tasks.ts");
includes(tasksSource, "objectOperationDetailRules", "TaskPlan requirement transfer must include objectOperationDetailRules.");
includes(tasksSource, "Every currentPhaseScope.included item should remain traceable into TaskPlan tasks", "TaskPlan transfer must preserve scope item traceability.");
includes(tasksSource, "taskAssignmentRule", "TaskPlan must assign object-operation details to tasks.");
includes(tasksSource, "field meaning, operation invariant, validation/blocking reason", "TaskExecution concept evidence must cover object-operation detail.");
includes(tasksSource, "verificationResults and conceptEvidence must mention the matching implemented or verified behavior", "TaskResult rules must require concrete behavior evidence.");

console.log("Brainstorm business-detail grounding protocol verification passed.");
