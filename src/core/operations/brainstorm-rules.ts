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
    "During the phase_scope block, present 2-3 source-grounded phase scope options before asking for confirmation by default. Treat nextPhaseSeed as a non-binding seed for option design, not as a preselected user answer.",
    "Each phase_scope option must identify included scope, excluded/deferred boundary, the reason for that cut, and the tradeoff for delivery speed, completeness, and implementation risk.",
    "Present phase_scope options in the user's language with a consistent compact structure for every option: included scope, excluded/deferred boundary, reason, and tradeoff.",
    "For product, domain, UI, workflow, or multi-step phases, derive options from source-grounded cuts such as a balanced current-phase cut, a narrower dependency-first or lower-risk cut, and a broader adjacent-workflow cut when those cuts exist in the requirement context.",
    "For phase continuation, use nextPhaseSeed.goal, nextPhaseSeed.scopePreview, previous deferred scope, and latest confirmed user decisions as the source-grounded current-phase candidate set. They are not a preselected user answer, but they are also not optional content to drop from the recommended option without a source-grounded reason.",
    "Recommend exactly one phase_scope option and explain why it is the best current-phase cut. The recommended option must preserve the current phase's source-grounded core outcome, module closure, lifecycle coverage, and dependency purpose.",
    "A narrower option may be offered as an alternative, but do not recommend it when it defers explicit nextPhaseSeed.scopePreview items, previously deferred current-phase work, or confirmed lifecycle actions that define the module closure, unless the user has asked to reduce scope or the source/repository evidence shows the full closure is impossible for this phase.",
    "If the recommended option excludes or defers any explicit current-phase seed item, label that as a scope reduction, explain the source-grounded reason, and ask the user to confirm the reduction instead of presenting it as the default recommendation.",
    "Use a single phase_scope only for an atomic phase where no source-grounded narrower cut, broader adjacent-workflow cut, dependency/order cut, deferred lifecycle action, or alternate UI/runtime boundary exists.",
    "When using the atomic single-scope exception, explicitly state why no narrower cut, broader adjacent-workflow cut, dependency/order cut, deferred lifecycle action, or alternate UI/runtime boundary exists before asking for confirmation.",
    "For phase continuation, multiple nextPhaseSeed.scopePreview items, multiple lifecycle actions, or multiple deferred/excluded items mean the phase is not atomic and must be presented as 2-3 options.",
    "A confirmed phase_scope option must be reflected in scope.included, scope.excluded, scope.deferred, phasePlan.current, and phasePlan.nextPhasePreview using the existing BrainstormCandidate fields.",
  ];
}

export function businessScenarioConfirmationRules(): string[] {
  return [
    "The concept_grounding block must include a plain-language business scenario confirmation when the current phase has domain behavior or user operations.",
    "Use wording like 'I will summarize the current business scenario first' rather than obscure terms. Do not use the Chinese word 反讲 as user-facing wording.",
    "The scenario confirmation must name the actor or system, the business object or subject, the trigger, the operation goal, the expected result, and the boundary of what this phase will not do when applicable.",
    "If the current phase is technical-only, state the concrete reason why business scenario confirmation is not applicable and summarize the technical workflow instead.",
    "Store confirmed scenario details in existing fields: domainModel.businessFlows[].summary, scope.included[].items, acceptance[].statement, and conceptGrounding.phaseConceptGrounding.concepts[].explanation when the scenario carries high-risk meaning.",
  ];
}

export function decisionImpactOrderingRules(): string[] {
  return [
    "The concept_grounding block must identify important clarification decisions before asking for confirmation, ordered by their downstream impact.",
    "High-impact decisions are those that change phase scope, data model, business flow, frontend operation path, interface contract, acceptance outcome, runtime or delivery boundary.",
    "For each high-impact decision, state what it affects in plain language and whether the decision is confirmed, unresolved, explicitly deferred, or not applicable.",
    "Do not invent decisions only to fill a checklist. Include only decisions grounded in the source requirement, confirmed user answer, repository facts, or an explicit unresolved note.",
    "Store confirmed decision impact in existing fields: scope reason/items for scope impact, acceptance statements for acceptance impact, conceptGrounding priority/attentionRank/humanReadableReason for high-risk impact, businessFlows summaries for flow impact, and frontendExperience operation paths for frontend impact.",
  ];
}

export function businessLifecycleScanRules(): string[] {
  return [
    "The concept_grounding block must scan each key current-phase business object or subject for lifecycle actions that are actually relevant to this phase.",
    "Lifecycle actions include create, query/select, view, update, approve/process, state change, terminate/cancel, and blocking/exception handling; do not force every action onto every object.",
    "For each relevant lifecycle action, summarize inputs or fields, preconditions, validation or blocking reasons, success state changes, and visible or returned feedback when applicable.",
    "For lifecycle actions that are explicitly out of scope or deferred, state that boundary in user language and preserve it in scope.deferred, scope.excluded, assumptions, or nextPhasePreview as appropriate.",
    "Store confirmed lifecycle details in existing fields: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding explanations, and frontendExperience operation paths when UI applies.",
  ];
}

export function phaseScopeSelfCheckRules(): string[] {
  return [
    "Before presenting phase_scope for user confirmation, run a phase_scope self-check inside the block.",
    "The self-check must verify that included, deferred, and excluded scope are explicit and that included scope names concrete objects, actions, workflows, deliverables, or boundaries when present in the source.",
    "The self-check must verify that 2-3 source-grounded phase scope options were considered and shown before confirmation unless the atomic single-scope exception is explicitly justified.",
    "If fewer than 2 options are shown, the self-check must document why there is no narrower cut, broader adjacent-workflow cut, dependency/order cut, deferred lifecycle action, or alternate UI/runtime boundary.",
    "The self-check must reject a single preselected phase_scope when nextPhaseSeed, deferred scope, excluded scope, or the source requirement contains multiple concrete actions, workflows, UI surfaces, lifecycle changes, or deliverable boundaries.",
    "The self-check must verify that the chosen current phase cut explains why the work belongs now and what meaningful work is deferred or excluded.",
    "The self-check must verify that nextPhasePreview carries remaining deferred or future scope when such scope exists.",
    "If the phase_scope self-check finds unclear or missing current-phase boundaries, ask a focused scope question before marking phase_scope confirmed.",
  ];
}

export function conceptGroundingSelfCheckRules(): string[] {
  return [
    "Before presenting concept_grounding for user confirmation, run a concept_grounding self-check inside the block.",
    "The self-check must verify that every confirmed scope.included item is covered, explicitly unresolved, or explicitly deferred.",
    "The self-check must verify that business scenario confirmation, decision impact ordering, and lifecycle scan have been considered when the current phase has domain behavior or user operations.",
    "The self-check must verify that key objects or subjects include applicable field sets, operation inputs, preconditions, validation or blocking reasons, success states, state transitions, and visible or returned feedback.",
    "If the concept_grounding self-check finds a relevant missing detail, ask a focused concept or business-rule question before marking concept_grounding confirmed.",
  ];
}

export function frontendExperienceSelfCheckRules(): string[] {
  return [
    "Before presenting frontend_experience for user confirmation, run a frontend_experience self-check inside the block.",
    "The self-check must verify whether UI is required, not required, or deferred, and must state the reason in user language.",
    "When UI is required, the self-check must verify target discovery or selection, pagination/list behavior when relevant, grounded query criteria, action entry, input fields, success feedback, error feedback, business-blocking feedback, and refresh/readback behavior.",
    "The self-check must verify that query criteria come from confirmed fields, user wording, acceptance details, business flow details, or repository facts; do not use a hardcoded industry field list.",
    "If the frontend_experience self-check finds the operation path unclear, ask a focused frontend operation-path question before marking frontend_experience confirmed.",
  ];
}

export function finalSummaryReviewRules(): string[] {
  return [
    "The final_summary block is a review gate, not the first place to discover business details.",
    "Before presenting final_summary for user confirmation, verify that phase_scope, concept_grounding, and frontend_experience were already confirmed or explicitly skipped with reasons.",
    "The final_summary must summarize confirmed scope, business scenario, decision impacts, lifecycle actions, key field/data elements, concept or rule boundaries, frontend target or skip reason, nextPhasePreview, and explicit not-done or deferred details when applicable.",
    "When concept_grounding confirmed key field sets, final_summary must include a concise user-visible key field/data elements paragraph covering the fields the phase depends on: identity fields, input fields, display fields, relationship fields, state fields, and result or feedback fields when applicable.",
    "The final_summary may be concise, but it must not narrow, omit, or override already confirmed phase_scope, concept_grounding, or frontend_experience details unless the user explicitly corrects them.",
    "When final_summary condenses earlier details, state that the previously confirmed block details remain part of the BrainstormCandidate contract instead of asking the user to restate them.",
    "If the user corrects final_summary, do not submit BrainstormCandidate from the stale summary. Incorporate the correction into the affected existing fields and present an updated final_summary before setting finalSummaryConfirmed=true.",
    "Do not use final_summary to hide missing block-level details; return to the relevant Brainstorm block in the same agent-managed conversation when detail is missing.",
  ];
}

export function confirmedBlockDetailRetentionRules(): string[] {
  return [
    "The accepted BrainstormCandidate must be built from all user-confirmed Brainstorm blocks, not from final_summary alone.",
    "For phase_scope, preserve the confirmed option's included scope, excluded scope, deferred scope, reasons, tradeoffs, and nextPhasePreview direction in scope, roadmap, phasePlan, assumptions, or acceptance as appropriate.",
    "For concept_grounding, preserve confirmed business scenario, high-risk concepts, business objects or subjects, key field sets, supported operations, operation inputs, preconditions, validation or blocking reasons, state transitions, success outcomes, visible or returned feedback, unresolved notes, and must-not-misinterpret boundaries in scope, acceptance, domainModel.businessFlows, and conceptGrounding.",
    "For frontend_experience, preserve confirmed UI need or skip reason, surfaces, data views, target discovery or selection path, pagination, confirmed query criteria, action entry points, input fields, success feedback, error feedback, business-blocking feedback, empty/loading states, refresh/readback policy, and unacceptable UI shapes in frontendExperience or frontendExperienceDelta.",
    "For final_summary, preserve user corrections and final scope decisions, but do not treat a concise final_summary as permission to drop details that were confirmed in earlier blocks.",
    "If a confirmed detail cannot fit a precise structured field, preserve it in the closest existing field's natural-language summary or notes rather than omitting it.",
  ];
}

export function brainstormCandidateSelfReviewRules(): string[] {
  return [
    "Before writing or submitting BrainstormCandidate, perform a self-review against the final_summary and each confirmed Brainstorm block.",
    "Self-review must verify that confirmed requirement details are stored in existing BrainstormCandidate fields rather than only in chat: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding, frontendExperience/frontendExperienceDelta, and phasePlan.nextPhasePreview.",
    "Self-review must verify that the candidate preserves confirmed details from every confirmed block. A concise final_summary does not make earlier phase_scope, concept_grounding, or frontend_experience details optional.",
    "Self-review must verify that every confirmed scope.included item has been considered in the concept_grounding scope-item coverage summary. If a scope item has no applicable detail, the candidate must preserve the concrete reason or unresolved note instead of silently dropping it.",
    "Self-review must verify that business scenario confirmation, decision impact ordering, and lifecycle scan details are present in existing candidate fields when they applied to the current phase.",
    "Self-review must check that scope items name concrete objects, actions, rules, fields, states, or boundaries when those details were confirmed.",
    "Self-review must check that acceptance statements are executable outcomes and that businessFlows summarize flow steps, preconditions, validation or blocking rules, blocking reasons, success state, and input/display/pass-through fields when applicable.",
    "Self-review must check that domain phases preserve a natural-language object-operation summary: key business objects, key field sets, supported operations, operation inputs, preconditions, validation or blocking reasons, success state changes, and user-visible feedback.",
    "Self-review must check that user-facing workflow phases store page operation paths in frontendExperience/frontendExperienceDelta: how users find or receive the target object, pagination and query criteria when confirmed, which view/action starts the operation, input fields, refresh/readback policy, and how success, empty, loading, error, or business-blocking results are observed.",
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
    "For query-and-select workflows, set dataViews[].paginationRequired=true and defaultLoadsFirstPage=true. searchCriteria is optional only when no query criteria were confirmed; when the user confirmed query criteria in frontend_experience, preserve them in dataViews[].searchCriteria or dataViewDeltas[].searchCriteria.",
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
    ...confirmedBlockDetailRetentionRules(),
    ...phaseScopeSelfCheckRules(),
    ...scopeItemCoverageClarificationRules(),
    ...scopeItemCoverageCandidateRules(),
    ...businessScenarioConfirmationRules(),
    ...decisionImpactOrderingRules(),
    ...businessLifecycleScanRules(),
    ...conceptGroundingSelfCheckRules(),
    ...businessObjectOperationClarificationRules(),
    ...businessObjectOperationCandidateRules(),
    ...frontendExperienceSelfCheckRules(),
    ...frontendOperationPathClarificationRules(),
    ...frontendOperationPathCandidateRules(),
    ...finalSummaryReviewRules(),
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
