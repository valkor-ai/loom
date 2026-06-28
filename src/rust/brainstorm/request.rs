use std::path::Path;

use contracts::{
    BrainstormCandidateAgentWritable, ClarificationBlockName, UserFacingLanguageConstraint,
};
use delivery_core::{ArtifactKind, RouteAction, RouteActionKind, WriteMode};
use schemars::schema_for;
use serde_json::{json, Map, Value};
use state::paths::to_project_relative;

use crate::{gate::required_blocks, paths::brainstorm_agent_candidate_file};

pub fn build_brainstorm_request_root(
    _project_root: &Path,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    brainstorm_run_id: &str,
    user_facing_language: &UserFacingLanguageConstraint,
    context_refs: Value,
) -> serde_json::Value {
    build_brainstorm_clarification_request_root(
        request_id,
        delivery_id,
        phase_id,
        brainstorm_run_id,
        user_facing_language,
        context_refs,
        ClarificationBlockName::PhaseScope,
    )
}

pub fn build_brainstorm_clarification_request_root(
    _request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    brainstorm_run_id: &str,
    user_facing_language: &UserFacingLanguageConstraint,
    context_refs: Value,
    current_block: ClarificationBlockName,
) -> serde_json::Value {
    let (rule_key, rules, rule_group_fields) = block_rules(&current_block);
    let mut rules_object = Map::new();
    rules_object.insert(rule_key.to_string(), rules);
    rules_object.insert(
        "requirementSemanticGrounding".to_string(),
        json!({ "compactRules": requirement_semantic_compact_rules() }),
    );
    let mut groups = vec![
        json!({
            "groupId": "conversation_protocol",
            "required": true,
            "purpose": "Read the current Brainstorm block protocol before presenting anything to the user.",
            "whenToRead": "Read at the beginning of this Brainstorm block.",
            "fields": [
                "userFacingLanguage",
                "clarificationConversationProtocol.currentBlock",
                "clarificationConversationProtocol.userVisibleBlockTitle",
                "clarificationConversationProtocol.userFacingLanguageRule",
                "clarificationConversationProtocol.blockRule",
                "clarificationConversationProtocol.confirmToolRule"
            ]
        }),
        json!({
            "groupId": "requirement_context",
            "required": true,
            "purpose": "Read compact source metadata and requirement hints for the current Brainstorm block.",
            "whenToRead": "Read before forming the current block response.",
            "fields": [
                "requirementContext.sourceItems",
                "keywordHints.compact"
            ]
        }),
        json!({
            "groupId": "requirement_full_text",
            "required": false,
            "purpose": "Read the full normalized requirement text only when compact context and request-scoped knowledge are insufficient.",
            "whenToRead": "Read on demand for the current block only.",
            "fields": [
                "requirementContext.normalizedText"
            ]
        }),
        json!({
            "groupId": "current_block_rules",
            "required": true,
            "purpose": "Read only the rules for the current Brainstorm confirmation block.",
            "whenToRead": "Read before presenting the current block.",
            "fields": rule_group_fields
        }),
    ];
    if current_block != ClarificationBlockName::PhaseScope {
        groups.push(json!({
            "groupId": "confirmed_clarification_state",
            "required": true,
            "purpose": "Read the already user-confirmed Brainstorm blocks as the authority for the current block.",
            "whenToRead": "Read before forming the current block response.",
            "fields": [
                "confirmedClarificationState.blocks",
                "confirmedClarificationState.finalSummaryConfirmed"
            ]
        }));
    }
    if current_block != ClarificationBlockName::FinalSummary {
        groups.push(json!({
            "groupId": "knowledge_context_plan",
            "required": true,
            "purpose": "Read the request-scoped knowledge query plan and call loom.knowledgeBrainstormContext for the current Brainstorm block before presenting it.",
            "whenToRead": "Read before forming the current block response; call loom.knowledgeBrainstormContext for every listed executionOrder step, and for repeatMode steps run the required query set.",
            "fields": [
                "knowledgeQueryPlan.sharedRules",
                "knowledgeQueryPlan.toolContract",
                format!("knowledgeQueryPlan.blocks.{}.executionOrder", block_id(&current_block))
            ]
        }));
    }
    groups.push(json!({
        "groupId": "block_confirmation_contract",
        "required": true,
        "purpose": "Read the current block confirmation submit shape after the user visibly confirms this block.",
        "whenToRead": "Read only after the user confirms the current block in chat.",
        "fields": [
            "blockConfirmationContract.tool",
            "blockConfirmationContract.currentBlock",
            "blockConfirmationContract.summary",
            "blockConfirmationContract.confirmedDataShape"
        ]
    }));

    json!({
        "schemaVersion": "1.0",
        "requestType": "brainstorm_clarification_block",
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "brainstormRunId": brainstorm_run_id,
        "userFacingLanguage": user_facing_language,
        "contextRefs": context_refs,
        "clarificationConversationProtocol": {
            "mode": "progressive_blocks",
            "currentBlock": current_block,
            "requiredBlocks": required_blocks(),
            "userVisibleBlockTitle": user_visible_block_title(&current_block),
            "userFacingLanguageRule": user_facing_language.rule,
            "blockRule": block_rule(&current_block),
            "confirmToolRule": "After visible user confirmation, call loom.brainstormConfirmBlock with this requestRef, currentBlock, a concise user-facing summary, and current-block confirmedData. Do not write the final Brainstorm candidate in a clarification block."
        },
        "knowledgeQueryPlan": knowledge_query_plan_for_block(&current_block),
        "rules": Value::Object(rules_object),
        "blockConfirmationContract": {
            "tool": "loom.brainstormConfirmBlock",
            "currentBlock": current_block,
            "summary": "Concise user-facing summary of what the user confirmed for this block.",
            "confirmedDataShape": block_confirmed_data_shape(&current_block)
        },
        "requestReadPlan": {
            "groups": groups
        }
    })
}

pub fn build_brainstorm_candidate_write_request_root(
    project_root: &Path,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    brainstorm_run_id: &str,
    user_facing_language: &UserFacingLanguageConstraint,
    context_refs: Value,
) -> serde_json::Value {
    let candidate_file = to_project_relative(
        project_root,
        &brainstorm_agent_candidate_file(project_root, request_id),
    )
    .unwrap_or_else(|_| format!(".loom/agent-writable/{request_id}/brainstorm-candidate.json"));
    let schema_shape = serde_json::to_value(schema_for!(BrainstormCandidateAgentWritable))
        .unwrap_or_else(|_| json!({ "type": "object" }));

    json!({
        "schemaVersion": "1.0",
        "requestType": "brainstorm_candidate_write",
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "brainstormRunId": brainstorm_run_id,
        "userFacingLanguage": user_facing_language,
        "contextRefs": context_refs,
        "rules": {
            "candidateWrite": candidate_write_rules(),
            "requirementSemanticGrounding": {
                "compactRules": requirement_semantic_compact_rules()
            }
        },
        "enumRefs": enum_refs(),
        "outputContract": {
            "artifactKind": ArtifactKind::BrainstormCandidate,
            "writeMode": WriteMode::SingleJson,
            "submitTool": "loom.brainstormAcceptFile",
            "writeTargets": [{
                "targetId": "candidate",
                "path": candidate_file,
                "required": true,
                "description": "Write the Brainstorm candidate JSON after final_summary is confirmed."
            }],
            "resultTemplate": candidate_result_template(phase_id),
            "schemaShape": schema_shape,
            "schemaProjection": schema_projection()
        },
        "postSubmit": {
            "nextAction": RouteAction {
                kind: RouteActionKind::TechnicalBaselineRequest,
                source: "brainstorm_accept".to_string(),
                reason: "brainstorm_confirmed".to_string(),
                prompt: None,
                accepted_responses: vec![],
                request_ref: None,
                details: None,
                target_phase_id: None
            }
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "confirmed_clarification_state",
                    "required": true,
                    "purpose": "Read the confirmed Brainstorm blocks that must be structurally preserved in the candidate.",
                    "whenToRead": "Read before writing the Brainstorm candidate.",
                    "fields": [
                        "confirmedClarificationState.blocks",
                        "confirmedClarificationState.skippedBlocks",
                        "confirmedClarificationState.finalSummaryConfirmed"
                    ]
                },
                {
                    "groupId": "source_ref_registry",
                    "required": true,
                    "purpose": "Read only legal requirement source ids for candidate sourceRefs; do not reinterpret requirements from this registry.",
                    "whenToRead": "Read before filling candidate sourceRefs.",
                    "fields": [
                        "sourceRefRegistry.sources"
                    ]
                },
                {
                    "groupId": "candidate_write_contract",
                    "required": true,
                    "purpose": "Read the compact write contract for the final Brainstorm candidate.",
                    "whenToRead": "Read immediately before writing the Brainstorm candidate.",
                    "fields": [
                        "outputContract.writeTargets",
                        "outputContract.submitTool",
                        "outputContract.resultTemplate",
                        "outputContract.schemaProjection",
                        "enumRefs.complexity",
                        "enumRefs.scopeSource",
                        "enumRefs.acceptancePriority",
                        "enumRefs.phaseStatus",
                        "enumRefs.phasePlanCurrentStatus",
                        "enumRefs.nextPhasePreviewKind",
                        "enumRefs.conceptGroundingMode",
                        "enumRefs.conceptPhaseRelevance",
                        "enumRefs.conceptPriority",
                        "enumRefs.conceptRiskFactor",
                        "enumRefs.glossaryUpdateOperation",
                        "enumRefs.clarificationBlockName",
                        "enumRefs.clarificationMode",
                        "enumRefs.frontendExperienceLevel",
                        "enumRefs.frontendTargetSelectionMode",
                        "enumRefs.frontendActionEntryPoint",
                        "enumRefs.frontendResultObservationMode",
                        "enumRefs.frontendInteractionState",
                        "rules.candidateWrite"
                    ]
                }
            ]
        }
    })
}

fn schema_projection() -> Value {
    json!({
        "requiredTopLevelFields": [
            "requestSummary",
            "scope",
            "roadmap",
            "phasePlan",
            "acceptance",
            "userConfirmation"
        ],
        "phaseScopeFields": [
            "scope.included",
            "scope.excluded",
            "scope.deferred",
            "scope.assumptions",
            "roadmap.currentPhaseId",
            "roadmap.phases",
            "phasePlan.current",
            "phasePlan.nextPhasePreview"
        ],
        "conceptGroundingFields": [
            "acceptance",
            "domainModel.businessFlows",
            "conceptGrounding",
            "conceptConfirmation"
        ],
        "frontendExperienceFields": [
            "frontendExperience.required",
            "frontendExperience.kind",
            "frontendExperience.experienceLevel",
            "frontendExperience.audiences",
            "frontendExperience.surfaces",
            "frontendExperience.dataViews",
            "frontendExperience.actions",
            "frontendExperience.operationPaths",
            "frontendExperience.mustNot",
            "frontendExperience.confirmationSummary"
        ],
        "enumFields": {
            "requestSummary.complexity": "enumRefs.complexity",
            "scope.included[].source": "enumRefs.scopeSource",
            "scope.excluded[].source": "enumRefs.scopeSource",
            "scope.deferred[].source": "enumRefs.scopeSource",
            "acceptance[].priority": "enumRefs.acceptancePriority",
            "roadmap.phases[].status": "enumRefs.phaseStatus",
            "phasePlan.current.status": "enumRefs.phasePlanCurrentStatus",
            "phasePlan.nextPhasePreview.kind": "enumRefs.nextPhasePreviewKind",
            "conceptGrounding.deliveryConceptGlossary.mode": "enumRefs.conceptGroundingMode",
            "conceptGrounding.deliveryConceptGlossary.concepts[].phaseRelevance": "enumRefs.conceptPhaseRelevance",
            "conceptGrounding.deliveryConceptGlossary.concepts[].priority": "enumRefs.conceptPriority",
            "conceptGrounding.deliveryConceptGlossary.concepts[].riskFactors[]": "enumRefs.conceptRiskFactor",
            "conceptGrounding.phaseConceptGrounding.mode": "enumRefs.conceptGroundingMode",
            "conceptGrounding.phaseConceptGrounding.concepts[].phaseRelevance": "enumRefs.conceptPhaseRelevance",
            "conceptGrounding.phaseConceptGrounding.concepts[].priority": "enumRefs.conceptPriority",
            "conceptGrounding.phaseConceptGrounding.concepts[].riskFactors[]": "enumRefs.conceptRiskFactor",
            "conceptGrounding.glossaryUpdates[].operation": "enumRefs.glossaryUpdateOperation",
            "clarificationProgress.mode": "enumRefs.clarificationMode",
            "clarificationProgress.confirmedBlocks[].block": "enumRefs.clarificationBlockName",
            "clarificationProgress.skippedBlocks[].block": "enumRefs.clarificationBlockName",
            "frontendExperience.experienceLevel": "enumRefs.frontendExperienceLevel",
            "frontendExperience.dataViews[].selectionMode": "enumRefs.frontendTargetSelectionMode",
            "frontendExperience.actions[].entryPoint": "enumRefs.frontendActionEntryPoint",
            "frontendExperience.actions[].resultObservation[]": "enumRefs.frontendResultObservationMode",
            "frontendExperience.operationPaths[].selectionMode": "enumRefs.frontendTargetSelectionMode",
            "frontendExperience.operationPaths[].requiredStates[]": "enumRefs.frontendInteractionState"
        },
        "objectShapeRules": {
            "frontendExperience.actions[].resultObservation[]": "Use only frontendResultObservationMode values; empty is not a result observation.",
            "frontendExperience.operationPaths[].requiredStates[]": "Use frontendInteractionState values; empty is valid only here for empty-list/page states."
        },
        "clarificationFields": [
            "userConfirmation.confirmed",
            "userConfirmation.confirmationSummary",
            "userConfirmation.confirmationBasis",
            "clarificationProgress"
        ],
        "clarificationProgressShape": {
            "mode": "progressive_blocks",
            "confirmedBlocks": [{
                "block": "phase_scope|concept_grounding|frontend_experience",
                "summary": "what the user confirmed in that user-facing block",
                "confirmedByUser": true
            }],
            "skippedBlocks": [{
                "block": "frontend_experience",
                "reason": "only when UI/page confirmation is explicitly not applicable"
            }],
            "finalSummaryConfirmed": true
        },
        "forbiddenClarificationProgressFields": [
            "completedBlocks",
            "currentBlock"
        ],
        "notes": [
            "Machine-owned ids, request binding, accepted status, and handoff routing are added by Loom on accept.",
            "final_summary is the gate before write, not the source of requirement detail."
        ]
    })
}

fn candidate_result_template(phase_id: &str) -> Value {
    json!({
        "requestSummary": {
            "title": "",
            "oneLine": "",
            "businessGoal": "",
            "complexity": "medium"
        },
        "scope": {
            "included": [{
                "id": "scope_1",
                "label": "",
                "items": [],
                "reason": "",
                "source": "user_confirmed"
            }],
            "excluded": [],
            "deferred": [{
                "id": "deferred_1",
                "label": "",
                "items": [],
                "reason": "",
                "source": "user_confirmed"
            }],
            "assumptions": [{
                "id": "assumption_1",
                "text": "",
                "requiresConfirmation": false
            }]
        },
        "roadmap": {
            "required": true,
            "currentPhaseId": phase_id,
            "phases": [{
                "phaseId": phase_id,
                "title": "",
                "name": "",
                "status": "scope_confirmed",
                "goal": ""
            }]
        },
        "phasePlan": {
            "current": {
                "phaseId": phase_id,
                "title": "",
                "goal": "",
                "scopeRefs": ["scope_1"],
                "acceptanceRefs": ["acc_1"],
                "status": "scope_confirmed"
            },
            "nextPhasePreview": {
                "kind": "candidate",
                "suggestedPhaseId": "phase-next",
                "title": "",
                "goal": "",
                "scopePreview": [],
                "reason": ""
            }
        },
        "acceptance": [{
            "id": "acc_1",
            "statement": "",
            "capabilityRefs": [],
            "sourceRefs": [],
            "priority": "must"
        }],
        "domainModel": {
            "actors": [{
                "id": "actor_1",
                "name": "",
                "description": ""
            }],
            "capabilityGroups": [{
                "id": "capability_group_1",
                "name": "",
                "description": ""
            }],
            "businessFlows": [{
                "id": "flow_1",
                "name": "",
                "actors": ["actor_1"],
                "capabilityRefs": ["scope_1"],
                "summary": ""
            }]
        },
        "conceptGrounding": {
            "phaseConceptGrounding": {
                "mode": "concepts_present",
                "reason": "",
                "concepts": [{
                    "conceptId": "concept_1",
                    "term": "",
                    "normalizedName": "",
                    "explanation": "",
                    "mustNotMisinterpretAs": [],
                    "phaseRelevance": "current",
                    "priority": "must_understand",
                    "attentionRank": 1,
                    "riskFactors": [],
                    "scopeRefs": ["scope_1"],
                    "acceptanceRefs": ["acc_1"],
                    "humanReadableReason": ""
                }]
            },
            "glossaryUpdates": []
        },
        "conceptConfirmation": {
            "shownToUser": true,
            "confirmedConceptRefs": [],
            "confirmationSummary": ""
        },
        "frontendExperience": {
            "required": true,
            "kind": "",
            "experienceLevel": "usable_internal_product",
            "audiences": [{
                "audienceId": "audience_1",
                "name": "",
                "primaryJobs": []
            }],
            "surfaces": [{
                "surfaceId": "surface_1",
                "name": "",
                "audienceRefs": ["audience_1"],
                "primaryJobs": []
            }],
            "dataViews": [{
                "viewId": "view_1",
                "name": "",
                "purpose": "",
                "targetObject": "",
                "selectionMode": "query_and_select",
                "paginationRequired": true,
                "defaultLoadsFirstPage": true,
                "searchCriteria": [{
                    "criterionId": "criterion_1",
                    "label": "",
                    "fieldRef": "",
                    "reason": "",
                    "sourceRefs": []
                }],
                "sourceRefs": []
            }],
            "actions": [{
                "actionId": "action_1",
                "label": "",
                "targetObject": "",
                "entryPoint": "navigation_entry",
                "inputFields": [],
                "resultObservation": ["response_message"],
                "refreshPolicy": "",
                "successFeedback": [],
                "blockingOrErrorFeedback": [],
                "sourceRefs": []
            }],
            "operationPaths": [{
                "pathId": "path_1",
                "name": "",
                "userGoal": "",
                "surfaceRef": "surface_1",
                "targetObject": "",
                "selectionMode": "query_and_select",
                "selectionSummary": "",
                "dataViewRefs": ["view_1"],
                "actionRefs": ["action_1"],
                "requiredStates": ["success", "business_blocking", "error"],
                "sourceRefs": []
            }],
            "mustNot": [],
            "confirmationSummary": ""
        },
        "userConfirmation": {
            "confirmed": true,
            "confirmedAt": "",
            "confirmationSummary": "",
            "confirmationBasis": {
                "initialRequestOnly": false,
                "summaryPresentedToUser": true,
                "confirmedAfterSummary": true,
                "presentedItems": [
                    "阶段范围确认",
                    "业务理解与规则确认",
                    "页面办理路径确认",
                    "提交前确认"
                ]
            }
        },
        "clarificationProgress": {
            "mode": "progressive_blocks",
            "confirmedBlocks": [
                {
                    "block": "phase_scope",
                    "summary": "",
                    "confirmedByUser": true
                },
                {
                    "block": "concept_grounding",
                    "summary": "",
                    "confirmedByUser": true
                },
                {
                    "block": "frontend_experience",
                    "summary": "",
                    "confirmedByUser": true
                }
            ],
            "skippedBlocks": [],
            "finalSummaryConfirmed": true
        }
    })
}

fn enum_refs() -> Value {
    json!({
        "complexity": ["small", "medium", "large", "unknown"],
        "scopeSource": ["source_explicit", "user_confirmed", "user_overridden", "model_recommended", "derived"],
        "acceptancePriority": ["must", "should", "could"],
        "phaseStatus": ["scope_confirmed", "proposed", "delivered", "paused", "skipped", "revised"],
        "phasePlanCurrentStatus": ["scope_confirmed"],
        "nextPhasePreviewKind": ["candidate", "none"],
        "conceptGroundingMode": ["concepts_present", "none_required", "not_applicable"],
        "conceptPhaseRelevance": ["current", "current_adjacent", "future", "deferred", "excluded"],
        "conceptPriority": ["must_understand", "should_understand", "nice_to_understand"],
        "conceptRiskFactor": ["business_invariant", "state_transition", "resource_consistency", "permission_boundary", "external_contract", "scope_confusion_risk", "user_visible_flow", "runtime_or_delivery_semantics", "frontend_experience_semantics"],
        "glossaryUpdateOperation": ["add", "replace", "remove"],
        "clarificationBlockName": ["phase_scope", "concept_grounding", "frontend_experience", "final_summary"],
        "clarificationMode": ["progressive_blocks"],
        "frontendExperienceLevel": ["none", "technical_demo", "usable_internal_product", "polished_product"],
        "frontendTargetSelectionMode": ["query_and_select", "direct_id_lookup", "preselected_context", "not_applicable"],
        "frontendActionEntryPoint": ["result_row_action", "detail_button", "form_submit", "bulk_action", "inline_action", "navigation_entry"],
        "frontendResultObservationMode": ["list_refresh", "detail_refresh", "inline_status_update", "response_message", "not_applicable"],
        "frontendInteractionState": ["loading", "success", "error", "empty", "business_blocking"]
    })
}

fn knowledge_query_plan() -> Value {
    json!({
        "sharedRules": [
            "Use request-scoped knowledge context only for phase_scope, concept_grounding, and frontend_experience.",
            "Do not carry knowledge chunks from one Brainstorm block into another block without re-querying that block's step.",
            "For each executionOrder step, call loom.knowledgeBrainstormContext with projectRoot, requestRef, block, stepId, querySubject, naturalLanguageQuery, and semanticFocus.",
            "When an executionOrder step declares repeatMode=per_candidate_phase_cut, call loom.knowledgeBrainstormContext once per candidate phase cut with a distinct queryId before presenting options.",
            "If loom.knowledgeBrainstormContext returns status available, inspect every chunk listed in readPlan before using it in the clarification block.",
            "If loom.knowledgeBrainstormContext returns status empty, continue with source requirements and mention no knowledge match only when it affects confidence.",
            "If any knowledge tool returns state failed or an error object, stop the clarification block and report the failure; do not silently fall back to a knowledge-free answer.",
            "Use knowledge only to improve clarification quality. Do not write knowledge source ids, chunk ids, inspect output, or knowledge paths into the Brainstorm candidate."
        ],
        "toolContract": {
            "contextTool": "loom.knowledgeBrainstormContext",
            "inspectTool": "loom.knowledgeInspectChunk",
            "doNotUseAsContextCheck": [
                "loom.knowledgeList",
                "loom.knowledgePending"
            ],
            "requiredInputFields": [
                "projectRoot",
                "requestRef",
                "block",
                "stepId",
                "querySubject",
                "naturalLanguageQuery",
                "semanticFocus"
            ],
            "conditionalInputFields": {
                "queryId": "Required when executionOrder[].repeatMode is per_candidate_phase_cut. Use a stable value such as capability_closure_A, capability_closure_B, or atomic_scope.",
                "atomicScopeReason": "Required only when queryId is atomic_scope."
            }
        },
        "blocks": {
            "phase_scope": {
                "executionOrder": [
                    {
                        "stepId": "phase_scope_dependency_order",
                        "queryKind": "dependency_order",
                        "querySubjectRule": "The subject is dependency evidence used to compare active-phase candidate boundaries, not a full delivery roadmap.",
                        "queryConstructionRules": [
                            "Use dependency_order only to compare what belongs in the active phase and what must be deferred.",
                            "Do not output or confirm the overall dependency sequence as numbered project phases.",
                            "Do not let a broad system-chain query decide the current phase by itself."
                        ]
                    },
                    {
                        "stepId": "phase_scope_capability_closure",
                        "queryKind": "capability_closure",
                        "repeatMode": "per_candidate_phase_cut",
                        "minimumQueryCount": 2,
                        "queryIdRule": "Use one distinct queryId per option candidate, for example capability_closure_A, capability_closure_B, capability_closure_C. If and only if the current phase is truly atomic and no meaningful narrower, broader, dependency, lifecycle, or UI/runtime boundary exists, use queryId=atomic_scope and provide atomicScopeReason.",
                        "querySubjectRule": "The subject is exactly one candidate capability unit or one closed current-phase slice.",
                        "queryConstructionRules": [
                            "Before composing options, identify candidate capability units from the requirement and dependency-order evidence.",
                            "Run one capability_closure query per candidate phase cut with a distinct queryId.",
                            "Each capability_closure query covers exactly one module, object lifecycle, workflow, backend capability, or page-operation set.",
                            "Keep semanticFocus inside the current unit's object, operation, rule, state, field, or flow anchors.",
                            "Do not include sibling, downstream, or next-phase capability units in semanticFocus.",
                            "Use separate operation focus entries for separate operations; never combine multiple operations into one operation focus.",
                            "For connected processes, lifecycle transitions, replacement, recovery, or ordered flows, include the primary object plus each identifiable component operation as separate operation focus entries; add a flow focus only when the whole flow wording is explicit."
                        ]
                    }
                ]
            },
            "concept_grounding": {
                "executionOrder": [
                    {
                        "stepId": "concept_grounding_scope_item",
                        "queryKind": "scope_item_grounding",
                        "querySubjectRule": "The subject is one confirmed scope item or one tight group from the user's confirmed phase scope that shares the same object and operation flow.",
                        "queryConstructionRules": [
                            "Start from every confirmed current-phase included item; do not query only the easiest or highest-level object.",
                            "naturalLanguageQuery should ask for the subject's objects, fields, rules, states, validations, blockers, outcomes, feedback, and misunderstanding boundaries.",
                            "semanticFocus must stay inside the confirmed scope item or tight group, pairing object focus with relevant operation, rule, state, field, or flow anchors when those anchors are explicit.",
                            "Do not re-query the whole system or every deferred module.",
                            "Use separate operation focus entries for separate actions and lifecycle steps; do not use a single compound operation focus as a substitute for lifecycle or process component operations."
                        ]
                    }
                ]
            },
            "frontend_experience": {
                "executionOrder": [
                    {
                        "stepId": "frontend_experience_page_operation_path",
                        "queryKind": "page_operation_path",
                        "querySubjectRule": "The subject is one confirmed user/staff-visible operation path or one tight group sharing one surface, target discovery, action entry, feedback, and readback pattern.",
                        "queryConstructionRules": [
                            "naturalLanguageQuery should ask for entry surface, target discovery, query/list/detail selection, action entry, form inputs, success feedback, validation or business-blocking feedback, loading/empty/error states, and refresh/readback.",
                            "Start from confirmed current-phase operations and blockers, then translate them into page-operation retrieval anchors for how a user or staff member finds the target, starts the action, enters data, sees feedback, and reads back updated state.",
                            "Prefer page or flow focus when those labels are explicit; otherwise use operation, field, state, and object anchors from confirmed scope.",
                            "When the knowledge source has no explicit page labels, use the page-operation intent in naturalLanguageQuery and use operation, field, state, or object semanticFocus anchors from confirmed scope.",
                            "For multi-step page paths or operation workflows, include page or flow focus plus concrete operation, field, or state anchors; do not rely on a compound operation focus alone.",
                            "Do not invent page labels just to satisfy semanticFocus.",
                            "Do not query only with business object names when operation, field, or state anchors are available."
                        ]
                    }
                ]
            }
        }
    })
}

fn knowledge_query_plan_for_block(block: &ClarificationBlockName) -> Value {
    if *block == ClarificationBlockName::FinalSummary {
        return json!({
            "sharedRules": [
                "final_summary does not call knowledge context."
            ],
            "toolContract": {
                "contextTool": "loom.knowledgeBrainstormContext",
                "inspectTool": "loom.knowledgeInspectChunk"
            },
            "blocks": {}
        });
    }
    let full = knowledge_query_plan();
    let block_name = block_id(block);
    let Some(block_plan) = full.pointer(&format!("/blocks/{block_name}")).cloned() else {
        return full;
    };
    let mut blocks = Map::new();
    blocks.insert(block_name.to_string(), block_plan);
    json!({
        "sharedRules": full.get("sharedRules").cloned().unwrap_or_else(|| json!([])),
        "toolContract": full.get("toolContract").cloned().unwrap_or_else(|| json!({})),
        "blocks": Value::Object(blocks)
    })
}

fn block_rules(block: &ClarificationBlockName) -> (&'static str, Value, Vec<&'static str>) {
    match block {
        ClarificationBlockName::PhaseScope => (
            "phaseScope",
            phase_scope_rules(),
            vec![
                "rules.phaseScope.blockMission",
                "rules.phaseScope.presentation",
                "rules.phaseScope.optionComparison",
                "rules.phaseScope.forbiddenOutput",
                "rules.phaseScope.selfCheck",
                "rules.phaseScope.confirmedDataShape",
            ],
        ),
        ClarificationBlockName::ConceptGrounding => (
            "conceptGrounding",
            concept_grounding_rules(),
            vec![
                "rules.conceptGrounding.presentation",
                "rules.conceptGrounding.selfCheck",
                "rules.conceptGrounding.scopeItemCoverage",
                "rules.conceptGrounding.objectOperation",
                "rules.conceptGrounding.businessScenario",
                "rules.conceptGrounding.decisionImpactOrdering",
                "rules.conceptGrounding.businessLifecycleScan",
                "rules.conceptGrounding.confirmedDataShape",
            ],
        ),
        ClarificationBlockName::FrontendExperience => (
            "frontendExperience",
            frontend_experience_rules(),
            vec![
                "rules.frontendExperience.presentation",
                "rules.frontendExperience.selfCheck",
                "rules.frontendExperience.operationPath",
                "rules.frontendExperience.candidateCarryForward",
                "rules.frontendExperience.confirmedDataShape",
            ],
        ),
        ClarificationBlockName::FinalSummary => (
            "finalSummary",
            final_summary_rules(),
            vec![
                "rules.finalSummary.reviewGate",
                "rules.finalSummary.presentation",
                "rules.finalSummary.checklistCoverage",
                "rules.finalSummary.requiredUserVisibleTopics",
                "rules.finalSummary.correctionWriteback",
                "rules.finalSummary.detailRetention",
                "rules.finalSummary.confirmedDataShape",
                "rules.requirementSemanticGrounding.compactRules",
            ],
        ),
    }
}

fn block_id(block: &ClarificationBlockName) -> &'static str {
    match block {
        ClarificationBlockName::PhaseScope => "phase_scope",
        ClarificationBlockName::ConceptGrounding => "concept_grounding",
        ClarificationBlockName::FrontendExperience => "frontend_experience",
        ClarificationBlockName::FinalSummary => "final_summary",
    }
}

fn user_visible_block_title(block: &ClarificationBlockName) -> &'static str {
    match block {
        ClarificationBlockName::PhaseScope => "阶段范围确认",
        ClarificationBlockName::ConceptGrounding => "业务理解与规则确认",
        ClarificationBlockName::FrontendExperience => "页面办理路径确认",
        ClarificationBlockName::FinalSummary => "提交前确认",
    }
}

fn block_rule(block: &ClarificationBlockName) -> &'static str {
    match block {
        ClarificationBlockName::PhaseScope => {
            "Confirm only the active phase boundary: first query request-scoped knowledge for this block, then present 2-3 current-phase options, not a full multi-stage project roadmap, and wait for explicit user confirmation."
        }
        ClarificationBlockName::ConceptGrounding => {
            "Use only the confirmed current-stage scope as the subject set; first query request-scoped knowledge for this block, then wait for explicit user confirmation."
        }
        ClarificationBlockName::FrontendExperience => {
            "Use confirmed business operations; first query request-scoped knowledge for this block, then confirm the page or workspace path, or record a concrete skip reason."
        }
        ClarificationBlockName::FinalSummary => {
            "Summarize already confirmed blocks for final confirmation; do not introduce new requirement detail here."
        }
    }
}

fn block_confirmed_data_shape(block: &ClarificationBlockName) -> Value {
    match block {
        ClarificationBlockName::PhaseScope => json!({
            "scope": {
                "included": ["current-stage capability items confirmed by the user"],
                "deferred": ["out-of-current-stage boundary items"],
                "excluded": ["not-this-delivery items when applicable"]
            },
            "recommendation": {
                "label": "confirmed current stage",
                "reason": "why this stage boundary was confirmed"
            },
            "nextPhasePreview": "short user-facing next-phase preview when useful"
        }),
        ClarificationBlockName::ConceptGrounding => json!({
            "scopeCoverage": ["coverage notes for each confirmed scope item"],
            "objects": ["business objects or subjects"],
            "operations": ["business operations or workflows"],
            "fields": ["important fields or inputs"],
            "states": ["important states"],
            "rules": ["validation, blocking, outcome, or invariant rules"],
            "boundaries": ["misunderstanding boundaries or deferred rules"]
        }),
        ClarificationBlockName::FrontendExperience => json!({
            "required": true,
            "surfaces": ["page or workspace names"],
            "targetDiscovery": ["query, list, selection, or preselected context"],
            "operationPaths": ["entry, inputs, success feedback, blocking feedback, and readback"],
            "mustNot": ["unacceptable page interaction forms"]
        }),
        ClarificationBlockName::FinalSummary => json!({
            "coverageChecklist": ["already confirmed scope, rules, page path, and deferred boundary"],
            "corrections": ["user corrections written back to prior block data"],
            "readyToWriteCandidate": true
        }),
    }
}

fn phase_scope_rules() -> Value {
    json!({
        "blockMission": [
            "This block confirms only the active phase implementation boundary.",
            "Use module dependency order only to compare active-phase candidate boundaries and deferred items.",
            "Before presenting options, call loom.knowledgeBrainstormContext for dependency_order and for every required per-candidate capability_closure query in knowledge_context_plan. If the result is empty, continue with source requirements.",
            "Even when the source asks for stage priorities or phased delivery, do not ask the user to confirm a full-project roadmap in this block."
        ],
        "presentation": [
            "Present 2-3 alternatives for the active phase only, preferably as A/B/C.",
            "Each alternative must be one current-phase candidate slice, not a sequence of phases.",
            "For each alternative show included scope, deferred boundary, reason, and tradeoff.",
            "End with one recommendation and ask the user to choose A/B/C or adjust the active-phase boundary."
        ],
        "optionComparison": [
            "During the phase_scope block, present 2-3 source-grounded phase scope options before asking for confirmation by default. Treat any next phase seed or dependency order as a non-binding seed for option design, not as a preselected user answer.",
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
            "Generate the recommended option from all goal-essential items plus all flow-support items, and include current-object lifecycle items when the current phase goal is a closed loop or lifecycle closure.",
            "Generate the narrower option by reducing current-object lifecycle items or experience/management extension items only; do not remove goal-essential or flow-support items from the narrow option unless you label it as a deliberate scope reduction and ask the user to confirm that reduction.",
            "Generate the broader option by adding real experience/management extension items or a clearly labeled adjacent capability; do not make the broad option the only place where goal-essential or flow-support items appear.",
            "Before presenting options, compare the included-scope line of every alternate option against the recommended option. If an alternate contains an item that is needed to use, select, validate, read back, or verify the recommended option, add it to the recommended option or ask a focused question.",
            "Each option must show included scope, deferred or not-this-phase boundary, reason, and tradeoff.",
            "Each option block must use a stable multi-line template: option letter and short title on the first line, then separate labeled lines for included scope, not-this-phase or deferred scope, reason, and tradeoff. In Chinese, use user-facing labels equivalent to 包含, 本阶段不做或延后, 原因, and 取舍.",
            "The recommendation should be shown in the option title, such as A（推荐） or Recommended, not buried inside the option paragraph.",
            "Recommend exactly one phase_scope option and explain why it is the best current-phase cut. The recommended option must preserve the current phase's source-grounded core outcome, module closure, lifecycle coverage, and dependency purpose.",
            "Do not rewrite source-grounded or previously confirmed object relationships, operations, ownership, or state transitions while composing options. Do not turn one object's operation into another object's operation just to create an option.",
            "For high-risk object relationship work, phase_scope should name the capability boundary in source-grounded terms and defer detailed relationship semantics to concept_grounding unless the source or prior confirmation already fixes those semantics.",
            "Do not introduce vague or new relationship endpoints in phase_scope wording. Avoid implying that one object is replaced, relinked, inherited, transferred, frozen, or restored unless that exact relationship effect is source-grounded or previously confirmed.",
            "A narrower option may be offered as an alternative, but do not recommend it when it defers explicit current-phase seed items, previously deferred current-phase work, or confirmed lifecycle actions that define the module closure, unless the user has asked to reduce scope or the source/repository evidence shows the full closure is impossible for this phase.",
            "If the recommended option excludes or defers any explicit current-phase seed item, label that as a scope reduction, explain the source-grounded reason, and ask the user to confirm the reduction instead of presenting it as the default recommendation.",
            "Use a single phase_scope only for an atomic phase where no source-grounded narrower cut, broader adjacent-workflow cut, dependency/order cut, deferred lifecycle action, or alternate UI/runtime boundary exists.",
            "When multiple concrete actions, workflows, UI surfaces, lifecycle changes, or deliverable boundaries exist, the phase is not atomic and must be presented as 2-3 options."
        ],
        "forbiddenOutput": [
            "Do not output numbered full-project phases such as 1..N.",
            "Do not ask the user to confirm the entire dependency sequence.",
            "Do not split downstream modules into their own confirmed implementation phases here.",
            "Mention downstream work only inside deferred boundary or a short next-phase preview."
        ],
        "selfCheck": [
            "Before responding, check that every option is an active-phase boundary candidate.",
            "Before responding, check that the current block's knowledge_context_plan was used; if the knowledge result was empty, the response may proceed from source requirements.",
            "Before responding, check that the message is not a full multi-stage roadmap.",
            "Verify that source-grounded candidate work was decomposed into scope items and internally classified before options were written.",
            "Verify the recommended option includes every goal-essential item and flow-support item, and includes current-object lifecycle items when the current phase goal is a closed loop or lifecycle closure.",
            "Verify that narrower options do not silently remove goal-essential or flow-support items; if they do, the option must be labeled as a deliberate scope reduction requiring user confirmation.",
            "Verify that broader options add only real experience/management extension items, adjacent capabilities, or clearly labeled cross-phase items; broader options must not be the only place where goal-essential or flow-support items appear.",
            "Compare alternate included-scope lines against the recommended option and reject any option set where a support item needed by the recommended option appears only in an alternate option.",
            "Verify that no option rewrites source-grounded or previously confirmed object relationships, object ownership, operation ownership, or state transitions.",
            "Verify that explicitly excluded items do not appear in any option's included scope, and unresolved classification items are surfaced as focused questions instead of being silently included.",
            "Verify that included, deferred, and excluded scope are explicit and that included scope names concrete objects, actions, workflows, deliverables, or boundaries when present in the source.",
            "If fewer than 2 options are shown, document why there is no narrower cut, broader adjacent-workflow cut, dependency/order cut, deferred lifecycle action, or alternate UI/runtime boundary before asking for confirmation.",
            "Do not let adjacent or downstream work occupy the current phase unless the user explicitly asks for that wider boundary."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::PhaseScope)
    })
}

fn concept_grounding_rules() -> Value {
    json!({
        "presentation": [
            "Use a user-facing title such as business understanding and rules confirmation, 业务理解与规则确认, or 业务规则确认. Do not show internal names such as concept_grounding, conceptGrounding, domainModel, businessFlows, riskFactor, semantic grounding, scope.included, or acceptance to the user.",
            "Use a stable user-visible section order when domain behavior applies: current business scenario, scope-by-scope coverage, key objects and operation rules, unresolved or deferred rules when any exist, and one confirmation instruction.",
            "Keep the current business scenario as a short plain-language paragraph, then use bullets or compact mini-blocks for details. Do not collapse scenario, concepts, object fields, operations, blocking rules, and state changes into one long paragraph.",
            "For scope-by-scope coverage, show one separate bullet or mini-block per confirmed current-phase scope item. Each item should name only applicable details: object or subject, action or behavior, key fields or inputs, preconditions, validation or blocking reasons, success state or visible feedback, and unresolved or deferred note.",
            "For key objects and operation rules, show one separate bullet or mini-block per important object or operation. Use user-facing labels equivalent to 对象/关系, 关键字段, 操作与前置条件, 校验/阻断, 状态/成功结果, and 未决/递延 when those details apply.",
            "If the phase is technical-only or a detail category is not applicable, state the concrete reason in user language instead of filling a checklist with generic business labels.",
            "End with one concise confirmation instruction. Do not ask the user to separately confirm every section unless a specific unresolved question blocks progress."
        ],
        "selfCheck": [
            "Before presenting concept_grounding for user confirmation, run a concept_grounding self-check inside the block.",
            "Verify that every confirmed scope.included item is covered, explicitly unresolved, or explicitly deferred.",
            "Verify that business scenario confirmation, decision impact ordering, and lifecycle scan have been considered when the current phase has domain behavior or user operations.",
            "Verify that key objects or subjects include applicable field sets, operation inputs, preconditions, validation or blocking reasons, success states, state transitions, and visible or returned feedback.",
            "If a relevant detail is unclear or missing, ask a focused concept or business-rule question before marking concept_grounding confirmed.",
            "Do not let final_summary become the first place where business rules appear."
        ],
        "scopeItemCoverage": [
            "Include a natural-language scope-item coverage summary before asking the user to confirm concepts.",
            "For each confirmed scope.included item, state what requirement detail is covered using only applicable dimensions: object or subject, user/system action or behavior, inputs or fields, preconditions, validation or blocking conditions and reasons, success state/data/UI/API/result changes, visible or returned feedback, source refs, and unresolved notes.",
            "Do not force every dimension onto every scope item. If a dimension is not applicable, omit it or give a short concrete reason; if it is applicable but source information is insufficient, mark it unresolved or ask a focused clarification.",
            "Do not use a fixed capability taxonomy or test-scenario categories when presenting the coverage summary. Coverage rows should follow the confirmed scope wording and source facts.",
            "If a confirmed scope item does not appear in the scope-item coverage summary, do not proceed to frontend_experience or final_summary; cover or explicitly defer that item first."
        ],
        "objectOperation": [
            "The concept_grounding block owns object-operation clarification for domain phases.",
            "When the current phase includes business objects, user operations, system operations, forms, persistence, state changes, or validation/blocking rules, present a natural-language object-operation summary before asking the user to confirm concepts.",
            "For each key business object in the current phase, list the key field set that the phase depends on: identity fields, input fields, display fields, relationship fields, state fields, and result or feedback fields. Use source-confirmed names when available; if a category is unclear, state the missing detail as a question or unresolved note instead of inventing fields.",
            "For each operation on a key object, summarize the operation input, preconditions, validation rules, blocking conditions, blocking reasons, success outcome, state changes, and user-visible feedback that downstream implementation must preserve.",
            "Every object field, operation rule, state change, and blocking reason shown in concept_grounding must point back to original requirements, confirmed user decisions, repository facts, or an explicit unresolved clarification note. Keyword hints are advisory only.",
            "Do not present only noun definitions or broad concept summaries when business operations are in scope.",
            "If the current phase is purely technical, infrastructure, build, deployment, or non-domain work, state why object-operation clarification is not applicable and keep concept grounding limited to real high-risk technical concepts."
        ],
        "businessScenario": [
            "Include a plain-language business scenario confirmation when the current phase has domain behavior or user operations.",
            "The scenario confirmation must name the actor or system, the business object or subject, the trigger, the operation goal, the expected result, and the boundary of what this phase will not do when applicable.",
            "If the current phase is technical-only, state the concrete reason why business scenario confirmation is not applicable and summarize the technical workflow instead."
        ],
        "decisionImpactOrdering": [
            "Identify important clarification decisions before asking for confirmation, ordered by downstream impact.",
            "High-impact decisions are those that change phase scope, data model, business flow, frontend operation path, interface contract, acceptance outcome, runtime, or delivery boundary.",
            "For each high-impact decision, state what it affects in plain language and whether it is confirmed, unresolved, explicitly deferred, or not applicable.",
            "Do not invent decisions only to fill a checklist. Include only decisions grounded in the source requirement, confirmed user answer, repository facts, or an explicit unresolved note."
        ],
        "businessLifecycleScan": [
            "Scan each key current-phase business object or subject for lifecycle actions that are actually relevant to this phase.",
            "Lifecycle actions include create, query/select, view, update, approve/process, state change, terminate/cancel, and blocking/exception handling; do not force every action onto every object.",
            "For each relevant lifecycle action, summarize inputs or fields, preconditions, validation or blocking reasons, success state changes, and visible or returned feedback when applicable.",
            "For lifecycle actions explicitly out of scope or deferred, state that boundary in user language and preserve it in deferred, excluded, assumptions, or next phase preview as appropriate."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::ConceptGrounding)
    })
}

fn frontend_experience_rules() -> Value {
    json!({
        "presentation": [
            "Use a user-facing title such as page operation path confirmation, 页面办理路径确认, or 页面操作路径确认. Do not show internal names such as frontend_experience, frontendExperience, targetDiscovery, search_then_select, list_browse, query_and_select, direct_id_lookup, preselected_context, not_applicable, dataViews, actions, or operationPaths to the user.",
            "Use a stable user-visible section order when UI applies: page or workspace surface, target discovery or query-selection path, operation entries and inputs, result feedback and refresh/readback, unacceptable page shapes when any apply, and one confirmation instruction.",
            "If users must query or select an existing object, show pagination/list behavior and query criteria as a separate labeled line such as 查询条件. Do not bury query criteria inside a prose paragraph.",
            "For multiple operations, show one separate bullet or compact mini-block per operation or operation group. Each operation item should name where it starts, what the user enters, what success looks like, what blocking/error feedback looks like, and what refresh/readback happens.",
            "If UI is not required, deferred, inherited without change, or driven by login/session/preselected context, state the concrete reason and still avoid internal enum labels.",
            "End with one concise confirmation instruction. Do not ask the user to separately confirm each operation unless a specific unresolved path blocks progress."
        ],
        "selfCheck": [
            "Before presenting frontend_experience for user confirmation, run a frontend_experience self-check inside the block.",
            "Verify whether UI is required, not required, or deferred, and state the reason in user language.",
            "When UI is required, verify target discovery or selection, pagination/list behavior when relevant, grounded query criteria, action entry, input fields, success feedback, error feedback, business-blocking feedback, and refresh/readback behavior.",
            "Verify that query criteria come from confirmed fields, user wording, acceptance details, business flow details, or repository facts; do not use a hardcoded industry field list.",
            "If the operation path is unclear, ask a focused frontend operation-path question before marking frontend_experience confirmed.",
            "Do not invent a page path when the phase is non-UI work."
        ],
        "operationPath": [
            "The frontend_experience block owns page operation path clarification; do not wait until final_summary to first ask how users find targets, trigger actions, or observe results.",
            "When the current phase has UI for existing business objects, present a natural-language default of paginated query results plus selection/action from those results unless the user has confirmed direct id entry, upstream context, login/session context, or no target object.",
            "When the operation starts from a prior page, authenticated session, notification, external link, or already selected record, describe that preselected context in user language and do not force a query page.",
            "When the operation is create-only, login-only, static content, a local developer tool, or a non-UI technical task, state why target selection is not applicable.",
            "If a search/query path is proposed, list only query criteria that are grounded in confirmed object fields, acceptance statements, business flow details, repository facts, or the user's own words; do not use a hardcoded industry field list.",
            "If confirmed fields are insufficient for meaningful filters, do not block the phase. Confirm a basic paginated result list with no advanced filters, and record the missing filter detail as a risk or note.",
            "Use natural user-facing wording in the conversation, such as 分页查询结果中选择记录并操作 or 从登录上下文带入当前对象. Do not show internal enum values like query_and_select, direct_id_lookup, preselected_context, not_applicable, dataViews, actions, or operationPaths to the user."
        ],
        "candidateCarryForward": [
            "When frontendExperience is present, store confirmed page operation paths in dataViews, actions, and operationPaths instead of only in confirmationSummary.",
            "For query-and-select workflows, set dataViews pagination and first-page loading expectations. Search criteria are optional only when no query criteria were confirmed; when query criteria were confirmed, preserve them.",
            "For direct id lookup workflows, explain why direct id entry is user-confirmed or operationally appropriate; do not use it as the default for existing-object back-office operations.",
            "Each action must name its entry point, input fields when applicable, success feedback, blocking/error feedback, and refresh policy so downstream architecture and task execution inherit the confirmed user experience target."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::FrontendExperience)
    })
}

fn final_summary_rules() -> Value {
    json!({
        "reviewGate": [
            "final_summary is the pre-submit coverage checklist gate, not the source of detailed requirements.",
            "Before presenting final_summary for user confirmation, verify that phase_scope, concept_grounding, and frontend_experience were already confirmed or explicitly skipped with concrete reasons.",
            "Do not use final_summary to introduce new requirements that were not confirmed in earlier blocks.",
            "Do not fix missing structured detail by expanding final_summary. Return to the relevant Brainstorm block or repair the corresponding structured candidate fields before submit."
        ],
        "presentation": [
            "Use a user-facing title such as pre-submit checklist, submit confirmation, 提交前核对, or 提交前确认. Do not show internal names such as final_summary, phase_scope, concept_grounding, frontend_experience, BrainstormCandidate, dataViews, actions, or operationPaths to the user.",
            "Project the confirmed prior blocks into one user-visible coverage checklist with exactly one confirmation action, not three separate confirmations.",
            "Use a stable user-facing section order equivalent to: current phase to submit, current phase coverage, confirmed business rules, confirmed page operation path, not done in this phase, next phase preview, confirmation instruction.",
            "End with one confirmation instruction. Do not ask the user to reconfirm each previous block separately."
        ],
        "checklistCoverage": [
            "For each applicable checklist section other than the single current phase goal and next phase preview, include at least two concrete checklist items when two or more items were confirmed earlier.",
            "The current phase coverage section must list concrete included capabilities or actions from the confirmed phase scope; do not collapse the scope to a single abstract sentence when multiple capabilities were confirmed.",
            "The business rules section must list concrete business objects, relationships, operation names, field-set headlines, state changes, blocking rules, success outcomes, or high-risk misunderstanding guards that were confirmed when they apply.",
            "The page operation path section must list concrete surface/entry, target discovery or query-selection path, pagination/query criteria when confirmed, action entry, result feedback, and refresh/readback behavior when UI applies.",
            "If a section is not applicable, write a concrete user-language reason instead of fabricating checklist items.",
            "If concrete checklist items cannot be extracted from confirmed prior blocks, do not invent them and do not submit; return to the relevant Brainstorm block or repair the corresponding structured candidate fields first."
        ],
        "requiredUserVisibleTopics": [
            "current phase submission goal",
            "coverage checklist from confirmed phase scope including concrete included work and deferred or not-done boundaries",
            "business-rule checklist from confirmed business understanding including concrete objects, relationships, operations, field-set headlines, state changes, blocking rules, success outcomes, and high-risk misunderstanding guards when applicable",
            "page-operation checklist from confirmed frontend path including surface or entry, target discovery or query selection, pagination and query criteria when confirmed, action entry, feedback, and refresh or readback when applicable",
            "explicit final_summary corrections that must be written back to structured fields",
            "next phase preview in user language"
        ],
        "correctionWriteback": [
            "If the user corrects final_summary, do not submit BrainstormCandidate from the stale summary.",
            "Incorporate the correction into the affected existing fields and present an updated final_summary before setting finalSummaryConfirmed=true.",
            "A correction can update phase scope, concept grounding, frontend experience, deferred boundaries, or next phase preview; do not store it only in the final_summary text."
        ],
        "detailRetention": [
            "The final_summary must not be required to repeat every confirmed object, field, operation, rule, state change, blocking reason, feedback path, or frontend operation path from earlier blocks.",
            "A checklist-style final_summary does not narrow, omit, override, or compress already confirmed phase_scope, concept_grounding, or frontend_experience details.",
            "Previously confirmed block details remain part of the BrainstormCandidate contract through structured fields, not through final_summary text.",
            "Keep confirmed details from phase_scope, concept_grounding, and frontend_experience in structured fields even when final_summary is concise."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::FinalSummary)
    })
}

fn candidate_write_rules() -> Value {
    json!([
        "Write only the Brainstorm candidate target after the user explicitly confirms final_summary.",
        "The accepted BrainstormCandidate must be built from all user-confirmed Brainstorm blocks plus final_summary corrections, not from final_summary alone.",
        "Use outputContract.resultTemplate as the concrete field shape when writing the candidate.",
        "Arrays in outputContract.resultTemplate show object shapes. Keep arrays empty only when the confirmed requirement has no item for that field; never replace typed object arrays with string arrays.",
        "When scope.deferred is non-empty, phasePlan.nextPhasePreview must use kind=candidate with suggestedPhaseId, title, goal, scopePreview, and reason. Use kind=none only when scope.deferred is empty.",
        "clarificationProgress must use confirmedBlocks/skippedBlocks/finalSummaryConfirmed. Do not write completedBlocks or currentBlock.",
        "In user-facing summaries and confirmation text, use user-visible block names rather than internal block ids.",
        "Keep knowledge metadata out of candidate sourceRefs and summary fields.",
        "Preserve all confirmed block details in scope, acceptance, domainModel.businessFlows, conceptGrounding, and frontendExperience instead of relying on final_summary text.",
        "For phase_scope, preserve the confirmed option's included scope, excluded scope, deferred scope, reasons, tradeoffs, and nextPhasePreview direction in scope, roadmap, phasePlan, assumptions, or acceptance as appropriate.",
        "For concept_grounding, preserve confirmed business scenario, high-risk concepts, business objects or subjects, key field sets, supported operations, operation inputs, preconditions, validation or blocking reasons, state transitions, success outcomes, visible or returned feedback, unresolved notes, and must-not-misinterpret boundaries in scope, acceptance, domainModel.businessFlows, and conceptGrounding.",
        "For frontend_experience, preserve confirmed UI need or skip reason, surfaces, data views, target discovery or selection path, pagination, confirmed query criteria, action entry points, input fields, success feedback, error feedback, business-blocking feedback, empty/loading states, refresh/readback policy, and unacceptable UI shapes in frontendExperience.",
        "For final_summary, preserve user corrections and final scope decisions, but do not treat a checklist-style final_summary as permission to drop details that were confirmed in earlier blocks.",
        "If a confirmed detail cannot fit a precise structured field, preserve it in the closest existing field's natural-language summary or notes rather than omitting it.",
        "Before writing or submitting BrainstormCandidate, perform a self-review against each confirmed Brainstorm block; final_summary is only the last confirmation surface and correction source.",
        "Self-review must verify that confirmed requirement details are stored in existing BrainstormCandidate fields rather than only in chat: scope.included[].items, acceptance[].statement, domainModel.businessFlows[].summary, conceptGrounding, frontendExperience, and phasePlan.nextPhasePreview.",
        "Self-review must verify that every confirmed scope.included item has been considered in the concept_grounding scope-item coverage summary. If a scope item has no applicable detail, preserve the concrete reason or unresolved note instead of silently dropping it.",
        "Self-review must check that user-facing workflow phases store page operation paths in frontendExperience: how users find or receive the target object, pagination and query criteria when confirmed, which view/action starts the operation, input fields, refresh/readback policy, and how success, empty, loading, error, or business-blocking results are observed.",
        "If self-review finds that a required detail is unclear or missing from the existing fields, return to the relevant Brainstorm block and ask the user before submitting; do not let PGC, AAC, TaskPlan, or TaskExecution rediscover that detail later.",
        "Do not create a separate Markdown spec, commit, or parallel requirement artifact for this self-review; the accepted BrainstormCandidate remains the requirement contract."
    ])
}

fn requirement_semantic_compact_rules() -> Value {
    json!([
        "Preserve the confirmed current-phase semantics in existing Brainstorm candidate fields; avoid vague labels.",
        "When business detail applies, confirm and write objects, operations, rules, fields, blockers, outcomes, and page paths in the owning blocks.",
        "final_summary is only the final coverage checklist and correction source; do not use it as the only detail source for the candidate.",
        "When business detail does not apply, state the concrete non-domain reason rather than fabricating domain rules.",
        "If a required semantic detail is unclear after reading the requirement and inspected knowledge, ask the user before accept."
    ])
}
