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

export function phaseScopeItemClassificationRules(): string[] {
  return [
    "Before writing phase_scope options, internally decompose the source-grounded current-phase candidate work into scope items and classify each item before composing options.",
    "Use only these internal scope item categories: goal-essential item, flow-support item, current-object lifecycle item, experience or management extension item, cross-phase item, explicitly excluded item, and unresolved classification item.",
    "A goal-essential item is required for the current phase goal to be true; do not omit it from the recommended option.",
    "A flow-support item is not the headline goal but is required for in-scope operations to be usable, selectable, submitted, validated, read back, or verified; do not move it only to the broad option.",
    "If any alternate option includes a support-like item that is required to find, select, operate on, validate, read back, or verify work in the recommended option, the recommended option must include that support item explicitly instead of relying on implication.",
    "A current-object lifecycle item belongs to the lifecycle of the current phase's core object or subject. If the phase goal is a closed loop or lifecycle closure, the recommended option must include the applicable current-object lifecycle items unless there is a source-grounded reason to ask the user to reduce scope.",
    "An experience or management extension item improves experience, administration, observability, approval depth, reporting, or adjacent convenience, but is not required for the current phase goal or flow support.",
    "A cross-phase item depends on a later module, another product surface, another subsystem, or a different core object lifecycle; keep it out of the recommended option unless the user explicitly asks to cross phase boundaries.",
    "An explicitly excluded item must not appear in any option's included scope.",
    "If classification is uncertain, mark the item as unresolved in natural language and ask a focused scope question instead of silently placing it into A/B/C.",
    "Do not expose these internal category names to the user. Use them only to make the options coherent.",
  ];
}

export function phaseScopeOptionComparisonRules(): string[] {
  return [
    "During the phase_scope block, present 2-3 source-grounded phase scope options before asking for confirmation by default. Treat nextPhaseSeed as a non-binding seed for option design, not as a preselected user answer.",
    "Use a user-facing title such as phase scope confirmation, 阶段范围确认, or 当前阶段范围确认. Do not show internal names such as phase_scope, scope.included, scope.excluded, scope.deferred, nextPhaseSeed, or nextPhasePreview to the user.",
    ...phaseScopeItemClassificationRules(),
    "Each phase_scope option must identify included scope, excluded/deferred boundary, the reason for that cut, and the tradeoff for delivery speed, completeness, and implementation risk.",
    "Present phase_scope options in the user's language with a consistent compact structure for every option: included scope, excluded/deferred boundary, reason, and tradeoff.",
    "Use one separate visual block per phase_scope option. Do not write an option as a single run-on paragraph containing included/deferred/reason/tradeoff labels.",
    "Each option block must use a stable multi-line template: option letter and short title on the first line, then separate labeled lines for included scope, not-this-phase or deferred scope, reason, and tradeoff. In Chinese, use user-facing labels equivalent to 包含, 本阶段不做或延后, 原因, and 取舍.",
    "Keep each labeled line compact and scannable. If included scope contains many actions, use semicolon-separated action names or short bullets under that label instead of one long sentence.",
    "The recommendation should be shown in the option title, such as A（推荐） or Recommended, not buried inside the option paragraph.",
    "After the options, provide one short recommendation sentence and one short user reply instruction. Do not repeat the full option details again in a prose paragraph.",
    "For product, domain, UI, workflow, or multi-step phases, derive options from source-grounded cuts such as a balanced current-phase cut, a narrower dependency-first or lower-risk cut, and a broader adjacent-workflow cut when those cuts exist in the requirement context.",
    "For phase continuation, use nextPhaseSeed.goal, nextPhaseSeed.scopePreview, previous deferred scope, and latest confirmed user decisions as the source-grounded current-phase candidate set. They are not a preselected user answer, but they are also not optional content to drop from the recommended option without a source-grounded reason.",
    "Generate the recommended option from all goal-essential items plus all flow-support items, and include current-object lifecycle items when the current phase goal is a closed loop or lifecycle closure.",
    "Generate the narrower option by reducing current-object lifecycle items or experience/management extension items only; do not remove goal-essential or flow-support items from the narrow option unless you label it as a deliberate scope reduction and ask the user to confirm that reduction.",
    "Generate the broader option by adding real experience/management extension items or a clearly labeled adjacent capability; do not make the broad option the only place where goal-essential or flow-support items appear.",
    "Before presenting options, compare the included-scope line of every alternate option against the recommended option. If an alternate contains an item that is needed to use, select, validate, read back, or verify the recommended option, add it to the recommended option or ask a focused question.",
    "Do not rewrite source-grounded or previously confirmed object relationships, operations, ownership, or state transitions while composing options. Do not turn one object's operation into another object's operation just to create an option.",
    "For high-risk object relationship work, phase_scope should name the capability boundary in source-grounded terms and defer detailed relationship semantics to concept_grounding unless the source or prior confirmation already fixes those semantics.",
    "Do not introduce vague or new relationship endpoints in phase_scope wording. Avoid implying that one object is replaced, relinked, inherited, transferred, frozen, or restored unless that exact relationship effect is source-grounded or previously confirmed.",
    "Recommend exactly one phase_scope option and explain why it is the best current-phase cut. The recommended option must preserve the current phase's source-grounded core outcome, module closure, lifecycle coverage, and dependency purpose.",
    "A narrower option may be offered as an alternative, but do not recommend it when it defers explicit nextPhaseSeed.scopePreview items, previously deferred current-phase work, or confirmed lifecycle actions that define the module closure, unless the user has asked to reduce scope or the source/repository evidence shows the full closure is impossible for this phase.",
    "If the recommended option excludes or defers any explicit current-phase seed item, label that as a scope reduction, explain the source-grounded reason, and ask the user to confirm the reduction instead of presenting it as the default recommendation.",
    "Use a single phase_scope only for an atomic phase where no source-grounded narrower cut, broader adjacent-workflow cut, dependency/order cut, deferred lifecycle action, or alternate UI/runtime boundary exists.",
    "When using the atomic single-scope exception, explicitly state why no narrower cut, broader adjacent-workflow cut, dependency/order cut, deferred lifecycle action, or alternate UI/runtime boundary exists before asking for confirmation.",
    "For phase continuation, multiple nextPhaseSeed.scopePreview items, multiple lifecycle actions, or multiple deferred/excluded items mean the phase is not atomic and must be presented as 2-3 options.",
    "A confirmed phase_scope option must be reflected in scope.included, scope.excluded, scope.deferred, phasePlan.current, and phasePlan.nextPhasePreview using the existing BrainstormCandidate fields.",
  ];
}

export function conceptGroundingPresentationRules(): string[] {
  return [
    "Use a user-facing title such as business understanding and rules confirmation, 业务理解与规则确认, or 业务规则确认. Do not show internal names such as concept_grounding, conceptGrounding, domainModel, businessFlows, riskFactor, semantic grounding, scope.included, or acceptance to the user.",
    "The concept_grounding block must use a stable user-visible section order when domain behavior applies: current business scenario, scope-by-scope coverage, key objects and operation rules, unresolved or deferred rules when any exist, and one confirmation instruction.",
    "Keep the current business scenario as a short plain-language paragraph, then use bullets or compact mini-blocks for details. Do not collapse scenario, concepts, object fields, operations, blocking rules, and state changes into one long paragraph.",
    "For scope-by-scope coverage, show one separate bullet or mini-block per confirmed current-phase scope item. Each item should name only applicable details: object or subject, action or behavior, key fields or inputs, preconditions, validation or blocking reasons, success state or visible feedback, and unresolved or deferred note.",
    "For key objects and operation rules, show one separate bullet or mini-block per important object or operation. Use user-facing labels equivalent to 对象/关系, 关键字段, 操作与前置条件, 校验/阻断, 状态/成功结果, and 未决/递延 when those details apply.",
    "If the phase is technical-only or a detail category is not applicable, state the concrete reason in user language instead of filling a checklist with generic business labels.",
    "End the concept_grounding block with one concise confirmation instruction. Do not ask the user to separately confirm every section unless a specific unresolved question blocks progress.",
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
    "The self-check must verify that source-grounded candidate work was decomposed into scope items and internally classified before options were written.",
    "The self-check must verify that the recommended option includes every goal-essential item and flow-support item, and includes current-object lifecycle items when the current phase goal is a closed loop or lifecycle closure.",
    "The self-check must verify that narrower options do not silently remove goal-essential or flow-support items; if they do, the option must be labeled as a deliberate scope reduction requiring user confirmation.",
    "The self-check must verify that broader options add only real experience/management extension items, adjacent capabilities, or clearly labeled cross-phase items; broader options must not be the only place where goal-essential or flow-support items appear.",
    "The self-check must compare alternate included-scope lines against the recommended option and reject any option set where a support item needed by the recommended option appears only in an alternate option.",
    "The self-check must verify that no option rewrites source-grounded or previously confirmed object relationships, object ownership, operation ownership, or state transitions.",
    "The self-check must verify that relationship wording names only source-grounded endpoints and does not imply replacement, relinking, inheritance, transfer, freezing, or restoration semantics that have not been source-grounded or previously confirmed.",
    "The self-check must verify that explicitly excluded items do not appear in any option's included scope, and unresolved classification items are surfaced as focused questions instead of being silently included.",
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

export function frontendExperiencePresentationRules(): string[] {
  return [
    "Use a user-facing title such as page operation path confirmation, 页面办理路径确认, or 页面操作路径确认. Do not show internal names such as frontend_experience, frontendExperience, targetDiscovery, search_then_select, list_browse, query_and_select, direct_id_lookup, preselected_context, not_applicable, dataViews, actions, or operationPaths to the user.",
    "The frontend_experience block must use a stable user-visible section order when UI applies: page or workspace surface, target discovery or query-selection path, operation entries and inputs, result feedback and refresh/readback, unacceptable page shapes when any apply, and one confirmation instruction.",
    "If users must query or select an existing object, show pagination/list behavior and query criteria as a separate labeled line such as 查询条件. Do not bury query criteria inside a prose paragraph.",
    "For multiple operations, show one separate bullet or compact mini-block per operation or operation group. Each operation item should name where it starts, what the user enters, what success looks like, what blocking/error feedback looks like, and what refresh/readback happens.",
    "If UI is not required, deferred, inherited without change, or driven by login/session/preselected context, state the concrete reason and still avoid internal enum labels.",
    "End the frontend_experience block with one concise confirmation instruction. Do not ask the user to separately confirm each operation unless a specific unresolved path blocks progress.",
  ];
}

export function finalSummaryReviewRules(): string[] {
  return [
    "The final_summary block is a pre-submit checklist gate, not the source of detailed requirements.",
    "Use a user-facing title such as pre-submit checklist, submit confirmation, 提交前核对, or 提交前确认. Do not show internal names such as final_summary, phase_scope, concept_grounding, frontend_experience, BrainstormCandidate, dataViews, actions, or operationPaths to the user.",
    "Before presenting final_summary for user confirmation, verify that phase_scope, concept_grounding, and frontend_experience were already confirmed or explicitly skipped with reasons.",
    "The final_summary must project the confirmed prior blocks into one user-visible coverage checklist with exactly one confirmation action, not three separate confirmations.",
    "The checklist must include user-facing sections for the current phase submission goal, current phase coverage, confirmed business rules or skip reason, confirmed page operation path or skip reason, explicit not-done/deferred boundaries, next phase preview in user language, and any corrections.",
    "Use a stable user-facing section order equivalent to: current phase to submit, current phase coverage, confirmed business rules, confirmed page operation path, not done in this phase, next phase preview, confirmation instruction.",
    "For each applicable checklist section other than the single current phase goal and next phase preview, include at least two concrete checklist items when two or more items were confirmed earlier.",
    "The current phase coverage section must list concrete included capabilities or actions from the confirmed phase scope; do not collapse the scope to a single abstract sentence when multiple capabilities were confirmed.",
    "The business rules section must list concrete business objects, relationships, operation names, field-set headlines, state changes, blocking rules, success outcomes, or high-risk misunderstanding guards that were confirmed when they apply.",
    "The page operation path section must list concrete surface/entry, target discovery or query-selection path, pagination/query criteria when confirmed, action entry, result feedback, and refresh/readback behavior when UI applies.",
    "If a section is not applicable, write a concrete user-language reason instead of fabricating checklist items.",
    "If the agent cannot extract concrete checklist items from the confirmed prior blocks, do not invent them and do not submit; return to the relevant Brainstorm block or repair the corresponding structured candidate fields first.",
    "The final_summary must not be required to repeat every confirmed object, field, operation, rule, state change, blocking reason, feedback path, or frontend operation path from earlier blocks.",
    "A checklist-style final_summary does not narrow, omit, override, or compress already confirmed phase_scope, concept_grounding, or frontend_experience details.",
    "Previously confirmed block details remain part of the BrainstormCandidate contract through structured fields, not through final_summary text.",
    "If the user corrects final_summary, do not submit BrainstormCandidate from the stale summary. Incorporate the correction into the affected existing fields and present an updated final_summary before setting finalSummaryConfirmed=true.",
    "Do not fix missing structured detail by expanding final_summary. Return to the relevant Brainstorm block or repair the corresponding structured candidate fields before submit.",
  ];
}

export function finalSummaryRequiredUserVisibleTopicsWhenApplicable(): string[] {
  return [
    "current phase submission goal",
    "coverage checklist from confirmed phase scope including concrete included work and deferred or not-done boundaries",
    "business-rule checklist from confirmed business understanding including concrete objects, relationships, operations, field-set headlines, state changes, blocking rules, success outcomes, and high-risk misunderstanding guards when applicable",
    "page-operation checklist from confirmed frontend path including surface or entry, target discovery or query selection, pagination and query criteria when confirmed, action entry, feedback, and refresh or readback when applicable",
    "explicit final_summary corrections that must be written back to structured fields",
    "next phase preview in user language",
  ];
}

export function confirmedBlockDetailRetentionRules(): string[] {
  return [
    "The accepted BrainstormCandidate must be built from all user-confirmed Brainstorm blocks, not from final_summary alone.",
    "For phase_scope, preserve the confirmed option's included scope, excluded scope, deferred scope, reasons, tradeoffs, and nextPhasePreview direction in scope, roadmap, phasePlan, assumptions, or acceptance as appropriate.",
    "For concept_grounding, preserve confirmed business scenario, high-risk concepts, business objects or subjects, key field sets, supported operations, operation inputs, preconditions, validation or blocking reasons, state transitions, success outcomes, visible or returned feedback, unresolved notes, and must-not-misinterpret boundaries in scope, acceptance, domainModel.businessFlows, and conceptGrounding.",
    "For frontend_experience, preserve confirmed UI need or skip reason, surfaces, data views, target discovery or selection path, pagination, confirmed query criteria, action entry points, input fields, success feedback, error feedback, business-blocking feedback, empty/loading states, refresh/readback policy, and unacceptable UI shapes in frontendExperience.",
    "For final_summary, preserve user corrections and final scope decisions, but do not treat a checklist-style final_summary as permission to drop details that were confirmed in earlier blocks.",
    "If a confirmed detail cannot fit a precise structured field, preserve it in the closest existing field's natural-language summary or notes rather than omitting it.",
  ];
}

export function brainstormCandidateSelfReviewRules(): string[] {
  return [
    "Before writing or submitting BrainstormCandidate, perform a self-review against each confirmed Brainstorm block; final_summary is only the last confirmation surface and correction source.",
    "Self-review must verify that confirmed requirement details are stored in existing BrainstormCandidate fields rather than only in chat: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding, frontendExperience, and phasePlan.nextPhasePreview.",
    "Self-review must verify that the candidate preserves confirmed details from every confirmed block. A checklist-style final_summary does not make earlier phase_scope, concept_grounding, or frontend_experience details optional.",
    "Self-review must verify that every confirmed scope.included item has been considered in the concept_grounding scope-item coverage summary. If a scope item has no applicable detail, the candidate must preserve the concrete reason or unresolved note instead of silently dropping it.",
    "Self-review must verify that business scenario confirmation, decision impact ordering, and lifecycle scan details are present in existing candidate fields when they applied to the current phase.",
    "Self-review must check that scope items name concrete objects, actions, rules, fields, states, or boundaries when those details were confirmed.",
    "Self-review must check that acceptance statements are executable outcomes and that businessFlows summarize flow steps, preconditions, validation or blocking rules, blocking reasons, success state, and input/display/pass-through fields when applicable.",
    "Self-review must check that domain phases preserve a natural-language object-operation summary: key business objects, key field sets, supported operations, operation inputs, preconditions, validation or blocking reasons, success state changes, and user-visible feedback.",
    "Self-review must check that user-facing workflow phases store page operation paths in frontendExperience: how users find or receive the target object, pagination and query criteria when confirmed, which view/action starts the operation, input fields, refresh/readback policy, and how success, empty, loading, error, or business-blocking results are observed.",
    "If self-review finds that a required detail is unclear or missing from the existing fields, return to the relevant Brainstorm block and ask the user before submitting; do not let PGC, AAC, TaskPlan, or TaskExecution rediscover that detail later.",
    "If the final_summary checklist is shorter than previous confirmed blocks, keep the checklist focused and preserve the detail in structured fields; do not ask the user to reconfirm all details just to make final_summary exhaustive.",
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
    "Store the confirmed scope-item coverage in existing BrainstormCandidate fields, not a new parallel model: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding.phaseConceptGrounding.concepts[].explanation, and frontendExperience when UI applies.",
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
    "Store confirmed object-operation details in existing BrainstormCandidate fields rather than a parallel artifact: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding.phaseConceptGrounding.concepts[].explanation, frontendExperience when UI applies, and phasePlan.nextPhasePreview when details are deferred.",
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
    "When frontendExperience is present, store confirmed page operation paths in dataViews, actions, and operationPaths instead of only in confirmationSummary.",
    "For query-and-select workflows, set dataViews[].paginationRequired=true and defaultLoadsFirstPage=true. searchCriteria is optional only when no query criteria were confirmed; when the user confirmed query criteria in frontend_experience, preserve them in dataViews[].searchCriteria.",
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
    "The Agent, not the CLI, decides whether the current phase involves business flows, user operations, state changes, forms/fields, validation/blocking rules, frontend/backend interaction, or user-facing operation paths. If it does, those details must be confirmed in the owning blocks and written into structured BrainstormCandidate fields; final_summary should only present a user-facing coverage checklist and any corrections.",
    "If those business-detail categories do not apply to the current phase, the final_summary block must state the concrete not-applicable reason, such as this phase only changing build configuration, test harnesses, deployment files, or other non-domain technical work.",
    "When business detail applies, write the confirmed details into existing BrainstormCandidate fields: scope.included[].items for modules/actions/rules/fields/boundaries; acceptance[].statement for verifiable business outcomes; domainModel.businessFlows[].summary for flow steps, preconditions, validation/blocking, and success state; conceptGrounding for high-risk concepts, object operations, hard rules, state changes, and misunderstanding boundaries; frontendExperience for target discovery, selection, input, display, action entry, refresh, and feedback expectations.",
    "For correction, completion, or optimization phases, describe the expected behavior from original/confirmed requirements, the current implemented behavior from latestRepositoryContext, the confirmed delta for this phase, and the target behavior after correction using the same existing fields.",
    "For technical or non-domain phases, do not fabricate domain rules; instead express technical workflow, constraints, boundaries, expected behavior, and verification responsibilities in scope, acceptance, domainModel.businessFlows when useful, and conceptGrounding only when there are real high-risk concepts.",
    "Every current-phase acceptance statement must be source-grounded: cite sourceRefs from original requirements, confirmed decisions, user confirmation, or repository facts as appropriate; keywordHints are never acceptance authority.",
    "If a required semantic detail for the confirmed current phase is unclear after reading the provided refs, ask the user in the relevant Brainstorm block before accepting; do not let downstream PGC/AAC/TaskPlan rediscover missing requirement rules from scratch.",
    "concept_grounding must cover confirmed business objects, key object field sets, operations on those objects, operation inputs, key flow logic, rule boundaries, state transitions, blocking reasons, and user-visible feedback when those details are relevant; it must not become only a glossary of nouns.",
    "frontendExperience is required only for UI or user-visible workflow phases; conceptGrounding may be none_required or not_applicable only with a concrete reason.",
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
    "When business detail applies, confirm flows, objects, operations, fields, preconditions, validation/blocking, success states, frontend operation paths, deferred details, and source refs in the owning blocks.",
    "When business detail does not apply, state the concrete technical/non-domain reason instead of fabricating domain rules.",
    "Write confirmed details into scope, acceptance, domainModel.businessFlows, conceptGrounding, and frontendExperience when applicable; final_summary is not a detail source.",
    "Acceptance statements must be source-grounded; keyword hints are advisory and never authority.",
    "If a required semantic detail is unclear after reading refs, ask the user before accepting.",
  ];
}
