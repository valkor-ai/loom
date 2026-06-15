export function nextPhasePreviewCandidateRules(): string[] {
  return [
    "phasePlan.nextPhasePreview is a non-binding seed for the next Brainstorm, not a confirmed future-phase scope.",
    "When original requirement text, current scope.deferred, or current scope.excluded indicates remaining capabilities, keep nextPhasePreview.kind=candidate and make scopePreview list concrete source-grounded business objects, actions, or workflows.",
    "Do not use nextPhasePreview.kind=none to mean the next phase is uncertain; use kind=none only when the current confirmed delivery has no remaining phase candidate.",
    "When multiple next directions remain, keep kind=candidate and make scopePreview a concrete candidate set for the next Brainstorm to confirm or narrow.",
    "Generic continuation labels are insufficient as the primary scopePreview content unless each item also names source-derived business objects, actions, or workflows.",
    "When contextRefs.normalizedRequirementTextRef or contextRefs.keywordHintsRef is present, use them as source/advisory context for nextPhasePreview wording; keyword hints are never scope or acceptance authority.",
  ];
}

export function phaseScopeOptionComparisonRules(): string[] {
  return [
    "During the phase_scope block, when the current phase boundary has real alternative cuts, present 2-3 source-grounded phase scope options before asking for confirmation.",
    "Each phase_scope option must identify included scope, excluded/deferred boundary, the reason for that cut, and the tradeoff for delivery speed, completeness, and implementation risk.",
    "Recommend exactly one phase_scope option and explain why it is the best current-phase cut.",
    "When the current phase boundary has only one clear source-grounded cut, present that single scope and explicitly state that meaningful alternatives were not found; do not fabricate extra options.",
    "A confirmed phase_scope option must be reflected in scope.included, scope.excluded, scope.deferred, phasePlan.current, and phasePlan.nextPhasePreview using the existing BrainstormCandidate fields.",
  ];
}

export function brainstormCandidateSelfReviewRules(): string[] {
  return [
    "Before writing or submitting BrainstormCandidate, perform a self-review against the final_summary and the confirmed phase_scope option.",
    "Self-review must verify that confirmed requirement details are stored in existing BrainstormCandidate fields rather than only in chat: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding, frontendExperience/frontendExperienceDelta, and phasePlan.nextPhasePreview.",
    "Self-review must verify that every confirmed scope.included item has been considered in the concept_grounding scope-item coverage summary. If a scope item has no applicable detail, the candidate must preserve the concrete reason or unresolved note instead of silently dropping it.",
    "Self-review must check that scope items name concrete objects, actions, rules, fields, states, or boundaries when those details were confirmed.",
    "Self-review must check that acceptance statements are executable outcomes and that businessFlows summarize flow steps, preconditions, validation or blocking rules, blocking reasons, success state, and input/display/pass-through fields when applicable.",
    "Self-review must check that domain phases preserve a natural-language object-operation summary: key business objects, key field sets, supported operations, operation inputs, preconditions, validation or blocking reasons, success state changes, and user-visible feedback.",
    "Self-review must check that user-facing workflow phases store page operation paths in frontendExperience/frontendExperienceDelta: how users find or receive the target object, which view/action starts the operation, and how success, empty, error, or business-blocking results are observed.",
    "If self-review finds that a required detail is unclear or missing from the existing fields, return to the relevant Brainstorm block and ask the user before submitting; do not let PGC, AAC, TaskPlan, or TaskExecution rediscover that detail later.",
    "Do not create a separate Markdown spec, commit, or parallel requirement artifact for this self-review; the accepted BrainstormCandidate remains the requirement contract.",
  ];
}

export function scopeItemCoverageClarificationRules(): string[] {
  return [
    "The concept_grounding block must include a natural-language scope-item coverage summary before asking the user to confirm concepts.",
    "For each confirmed scope.included item, state what requirement detail is covered for that item using only applicable dimensions: object or subject, user/system action or behavior, inputs or fields, preconditions, validation or blocking conditions and reasons, success state/data/UI/API/result changes, visible or returned feedback, source refs, and unresolved notes.",
    "Do not force every dimension onto every scope item. If a dimension is not applicable to that scope item, omit it or give a short concrete reason; if it is applicable but source information is insufficient, mark it as unresolved or ask a focused clarification.",
    "Do not use a fixed capability taxonomy or test-scenario categories when presenting the coverage summary. The coverage rows should follow the confirmed scope wording and the source facts.",
    "If a scope item was confirmed in phase_scope but does not appear in the scope-item coverage summary, do not proceed to frontend_experience or final_summary. Return to concept_grounding and cover or explicitly defer that item.",
  ];
}

export function scopeItemCoverageCandidateRules(): string[] {
  return [
    "Store the confirmed scope-item coverage in existing BrainstormCandidate fields, not a new parallel model: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding.phaseConceptGrounding.concepts[].explanation, and frontendExperience/frontendExperienceDelta when UI applies.",
    "Every scope.included item should be represented by at least one of these existing fields with its applicable object/subject, action/behavior, inputs/fields, preconditions, blocking reasons, success changes, feedback, source refs, or unresolved note.",
    "If an included scope item has no applicable business or technical detail beyond its name, preserve the reason in scope.included[].items or assumptions so downstream PGC/AAC/TaskPlan do not silently drop it.",
  ];
}

export function businessObjectOperationClarificationRules(): string[] {
  return [
    "The concept_grounding block owns business object and operation-rule clarification for domain phases; do not wait until final_summary to first expose object fields or operation logic.",
    "When the current phase includes business objects, user operations, system operations, forms, persistence, state changes, or validation/blocking rules, present a natural-language object-operation summary before asking the user to confirm concepts.",
    "For each key business object in the current phase, list the key field set that the phase depends on: identity fields, input fields, display fields, relationship fields, state fields, and result or feedback fields. Use source-confirmed names when available; if a category is unclear, state the missing detail as a question or unresolved note instead of inventing fields.",
    "For each operation on a key object, summarize the operation input, preconditions, validation rules, blocking conditions, blocking reasons, success outcome, state changes, and user-visible feedback that the downstream implementation must preserve.",
    "Every object field, operation rule, state change, and blocking reason shown in concept_grounding must point back to original requirements, confirmed user decisions, repository facts, or an explicit unresolved clarification note. Keyword hints are advisory only.",
    "Do not present only noun definitions or broad concept summaries when business operations are in scope; the user must be able to confirm whether the object fields and operation logic are correct before frontend_experience and final_summary.",
    "If the current phase is purely technical, infrastructure, build, deployment, or non-domain work, state why object-operation clarification is not applicable and keep conceptGrounding limited to real high-risk technical concepts.",
  ];
}

export function businessObjectOperationCandidateRules(): string[] {
  return [
    "Store confirmed object-operation details in existing BrainstormCandidate fields rather than a parallel artifact: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding.phaseConceptGrounding.concepts[].explanation, frontendExperience/frontendExperienceDelta when UI applies, and phasePlan.nextPhasePreview when details are deferred.",
    "scope.included[].items should include the current phase business objects, supported operations, key field sets, validation/blocking rules, state changes, and explicit boundaries when those details were confirmed.",
    "domainModel.businessFlows[].summary should describe object operation flow steps with inputs, preconditions, validation/blocking reasons, success state changes, and visible feedback; it must not be only a flow title.",
    "conceptGrounding.phaseConceptGrounding.concepts[].explanation should capture high-risk object semantics, key field meaning, operation invariants, state transition rules, and misunderstanding boundaries that tasks must preserve.",
    "acceptance[].statement should be executable against the confirmed object-operation details, including field, rule, state, feedback, or source-ref expectations when applicable.",
  ];
}

export function frontendOperationPathClarificationRules(): string[] {
  return [
    "The frontend_experience block owns page operation path clarification; do not wait until final_summary to first ask how users find targets, trigger actions, or observe results.",
    "When the current phase has UI for existing business objects, present a natural-language default of paginated query results plus selection/action from those results unless the user has confirmed direct id entry, upstream context, login/session context, or no target object.",
    "When the operation starts from a prior page, authenticated session, notification, external link, or already selected record, describe that preselected context in user language and do not force a query page.",
    "When the operation is create-only, login-only, static content, a local developer tool, or a non-UI technical task, state why target selection is not applicable.",
    "If a search/query path is proposed, list only query criteria that are grounded in confirmed object fields, acceptance statements, business flow details, repository facts, or the user's own words; do not use a hardcoded industry field list.",
    "If confirmed fields are insufficient for meaningful filters, do not block the phase. Confirm a basic paginated result list with no advanced filters, and record the missing filter detail as a risk or note.",
    "Use natural user-facing wording in the conversation, such as '分页查询结果中选择记录并操作' or '从登录上下文带入当前对象'. Do not show internal enum values like query_and_select, direct_id_lookup, preselected_context, not_applicable, dataViews, actions, or operationPaths to the user.",
  ];
}

export function frontendOperationPathCandidateRules(): string[] {
  return [
    "When frontendExperience/frontendExperienceDelta is present, store confirmed page operation paths in dataViews, actions, and operationPaths instead of only in confirmationSummary.",
    "For query-and-select workflows, set dataViews[].paginationRequired=true and defaultLoadsFirstPage=true; searchCriteria is optional and must come from confirmed fields or source refs.",
    "For direct id lookup workflows, explain why direct id entry is user-confirmed or operationally appropriate; do not use it as the default for existing-object back-office operations.",
    "For preselected context workflows, make operationPaths[].selectionSummary identify the upstream context such as prior page, session, notification, or selected parent record.",
    "Each action must name its entry point, input fields when applicable, success feedback, blocking/error feedback, and refresh policy so AAC can project the interface, UI state, and verification responsibility.",
    "Each operationPath must connect a user goal to dataViewRefs/actionRefs and requiredStates so PGC, AAC, TaskPlan, and TaskExecution inherit the confirmed user experience target.",
  ];
}

export function brainstormRequirementSemanticRules(): string[] {
  return [
    "Brainstorm must read the original requirement refs and any confirmed requirement decision refs before presenting a final_summary or writing BrainstormCandidate.",
    "For the user-confirmed current phase, preserve requirement semantics in existing BrainstormCandidate fields; do not reduce the phase to a vague label such as implement feature, fix bug, continue expansion, or optimize page.",
    "The Agent, not the CLI, decides whether the current phase involves business flows, user operations, state changes, forms/fields, validation/blocking rules, frontend/backend interaction, or user-facing operation paths. If it does, the final_summary block must show a business-detail confirmation covering current-phase flows, preconditions, validation rules, blocking rules and reasons, success conditions and state changes, fields to input/display/pass through, user operation path, deferred or not-done details, and source refs.",
    "If those business-detail categories do not apply to the current phase, the final_summary block must state the concrete not-applicable reason, such as this phase only changing build configuration, test harnesses, deployment files, or other non-domain technical work.",
    "When business-detail confirmation applies, write the confirmed details into existing BrainstormCandidate fields: scope.included[].items for modules/actions/rules/fields/boundaries; acceptance[].statement for verifiable business outcomes; domainModel.businessFlows[].summary for flow steps, preconditions, validation/blocking, and success state; conceptGrounding for high-risk concepts, object operations, hard rules, state changes, and misunderstanding boundaries; frontendExperience/frontendExperienceDelta for target discovery, selection, input, display, action entry, refresh, and feedback expectations.",
    "For correction, completion, or optimization phases, describe the expected behavior from original/confirmed requirements, the current implemented behavior from latestRepositoryContext, the confirmed delta for this phase, and the target behavior after correction using the same existing fields.",
    "For technical or non-domain phases, do not fabricate domain rules; instead express technical workflow, constraints, boundaries, expected behavior, and verification responsibilities in scope, acceptance, domainModel.businessFlows when useful, and conceptGrounding only when there are real high-risk concepts.",
    "Every current-phase acceptance statement must be source-grounded: cite sourceRefs from original requirements, confirmed decisions, user confirmation, or repository facts as appropriate; keywordHints are never acceptance authority.",
    "If a required semantic detail for the confirmed current phase is unclear after reading the provided refs, ask the user in the relevant Brainstorm block before accepting; do not let downstream PGC/AAC/TaskPlan rediscover missing requirement rules from scratch.",
    "concept_grounding must cover confirmed business objects, key object field sets, operations on those objects, operation inputs, key flow logic, rule boundaries, state transitions, blocking reasons, and user-visible feedback when those details are relevant; it must not become only a glossary of nouns.",
    "frontendExperience/frontendExperienceDelta is required only for UI or user-visible workflow phases; conceptGrounding may be none_required or not_applicable only with a concrete reason.",
    ...scopeItemCoverageClarificationRules(),
    ...scopeItemCoverageCandidateRules(),
    ...businessObjectOperationClarificationRules(),
    ...businessObjectOperationCandidateRules(),
    ...frontendOperationPathClarificationRules(),
    ...frontendOperationPathCandidateRules(),
  ];
}

export function brainstormRequirementSemanticCompactRules(): string[] {
  return [
    "Read original requirement refs and confirmed decision refs before final_summary or BrainstormCandidate submit.",
    "Preserve the confirmed current-phase semantics in existing BrainstormCandidate fields; avoid vague labels.",
    "When business detail applies, confirm flows, objects, operations, fields, preconditions, validation/blocking, success states, frontend operation paths, deferred details, and source refs.",
    "When business detail does not apply, state the concrete technical/non-domain reason instead of fabricating domain rules.",
    "Write confirmed details into scope, acceptance, domainModel.businessFlows, conceptGrounding, and frontendExperience/frontendExperienceDelta when applicable.",
    "Acceptance statements must be source-grounded; keyword hints are advisory and never authority.",
    "If a required semantic detail is unclear after reading refs, ask the user before accepting.",
  ];
}
