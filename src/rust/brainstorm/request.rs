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
            "whenToRead": "Read before forming the current block response; call loom.knowledgeBrainstormContext for every listed executionOrder step.",
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
            "blockConfirmationContract"
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
                        "confirmedClarificationState"
                    ]
                },
                {
                    "groupId": "requirement_context",
                    "required": true,
                    "purpose": "Read formal requirement source ids before writing candidate sourceRefs.",
                    "whenToRead": "Read before writing sourceRefs.",
                    "fields": [
                        "requirementContext.sourceItems",
                        "keywordHints.compact"
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
                        "enumRefs.scopeSource",
                        "enumRefs.acceptancePriority",
                        "enumRefs.conceptGroundingMode",
                        "enumRefs.clarificationBlockName",
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
            "deferred": [],
            "assumptions": []
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
                "kind": "none",
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
            "businessFlows": []
        },
        "conceptGrounding": {
            "phaseConceptGrounding": {
                "mode": "concepts_present",
                "reason": "",
                "concepts": []
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
            "audiences": [],
            "surfaces": [],
            "dataViews": [],
            "actions": [],
            "operationPaths": [],
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
        "scopeSource": ["source_explicit", "user_confirmed", "user_overridden", "model_recommended", "derived"],
        "acceptancePriority": ["must", "should", "could"],
        "conceptGroundingMode": ["concepts_present", "none_required", "not_applicable"],
        "clarificationBlockName": ["phase_scope", "concept_grounding", "frontend_experience", "final_summary"],
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
            ]
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
                        "querySubjectRule": "The subject is exactly one candidate capability unit or one closed current-phase slice.",
                        "queryConstructionRules": [
                            "Run one capability_closure query per candidate phase cut.",
                            "Keep semanticFocus inside the current unit's object, operation, rule, state, field, or flow anchors."
                        ]
                    }
                ]
            },
            "concept_grounding": {
                "executionOrder": [
                    {
                        "stepId": "concept_scope_item_grounding",
                        "queryKind": "scope_item_grounding",
                        "querySubjectRule": "The subject is one confirmed scope item or one tight group sharing the same object and flow.",
                        "queryConstructionRules": [
                            "Query only the already confirmed current-phase scope items.",
                            "Use semanticFocus to name concrete objects, operations, rules, states, and fields."
                        ]
                    }
                ]
            },
            "frontend_experience": {
                "executionOrder": [
                    {
                        "stepId": "frontend_page_operation_path",
                        "queryKind": "page_operation_path",
                        "querySubjectRule": "The subject is one confirmed page/workspace operation path or one tight group sharing the same entry and readback pattern.",
                        "queryConstructionRules": [
                            "Ask for entry surface, target discovery, action entry, feedback, blocking, and readback.",
                            "If page-specific knowledge is absent, use confirmed business operations to form the page path without inventing unsupported UI facts."
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
                "rules.frontendExperience.confirmedDataShape",
            ],
        ),
        ClarificationBlockName::FinalSummary => (
            "finalSummary",
            final_summary_rules(),
            vec![
                "rules.finalSummary.reviewGate",
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
            "Before presenting options, call loom.knowledgeBrainstormContext for every phase_scope executionOrder step in knowledge_context_plan. If the result is empty, continue with source requirements.",
            "Even when the source asks for stage priorities or phased delivery, do not ask the user to confirm a full-project roadmap in this block."
        ],
        "presentation": [
            "Present 2-3 alternatives for the active phase only, preferably as A/B/C.",
            "Each alternative must be one current-phase candidate slice, not a sequence of phases.",
            "For each alternative show included scope, deferred boundary, reason, and tradeoff.",
            "End with one recommendation and ask the user to choose A/B/C or adjust the active-phase boundary."
        ],
        "optionComparison": [
            "Present 2-3 source-grounded current-phase options with one recommendation.",
            "Each option must show included scope, deferred or not-this-phase boundary, reason, and tradeoff.",
            "The recommended option must preserve the current phase's closure and dependency purpose."
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
            "Verify the recommended option contains goal-essential and flow-support items.",
            "Do not let adjacent or downstream work occupy the current phase unless the user explicitly asks for that wider boundary."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::PhaseScope)
    })
}

fn concept_grounding_rules() -> Value {
    json!({
        "presentation": [
            "Show the current business scenario, scope-by-scope coverage, key objects and operation rules, then one confirmation instruction.",
            "Cover applicable objects, fields, operations, preconditions, validation or blockers, outcomes, and misunderstanding boundaries."
        ],
        "selfCheck": [
            "Every confirmed scope item must be covered, explicitly unresolved, or explicitly deferred.",
            "Do not let final_summary become the first place where business rules appear."
        ],
        "scopeItemCoverage": [
            "For each confirmed scope item, show object or subject, action or behavior, inputs or fields, blockers, outcomes, and unresolved notes when applicable."
        ],
        "objectOperation": [
            "The concept_grounding block owns object-operation clarification for domain phases.",
            "Do not present only noun definitions when business operations are in scope."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::ConceptGrounding)
    })
}

fn frontend_experience_rules() -> Value {
    json!({
        "presentation": [
            "Confirm the page or workspace operation path in user language.",
            "Cover entry surface, target discovery, query or selection, action entry, input fields, success feedback, blocking feedback, and readback."
        ],
        "selfCheck": [
            "State clearly whether UI is required, skipped, or not applicable.",
            "Do not invent a page path when the phase is non-UI work."
        ],
        "operationPath": [
            "When target discovery exists, prefer paginated query and selection unless the user confirmed direct id lookup or preselected context.",
            "Use concrete confirmed operations, fields, and states to define the path."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::FrontendExperience)
    })
}

fn final_summary_rules() -> Value {
    json!({
        "reviewGate": [
            "final_summary is the pre-submit coverage checklist. It is not the only detail source.",
            "Do not use final_summary to introduce new requirements that were not confirmed in earlier blocks."
        ],
        "requiredUserVisibleTopics": [
            "current phase submission goal",
            "coverage checklist for confirmed current-phase scope and deferred boundaries",
            "business-rule checklist for confirmed objects, rules, blockers, and outcomes when applicable",
            "page-operation checklist for confirmed UI/workspace path when applicable",
            "explicit user corrections that must be written back into structured fields",
            "next phase preview in user language"
        ],
        "correctionWriteback": [
            "If the user corrects final_summary, update the corresponding structured fields first, then present an updated summary."
        ],
        "detailRetention": [
            "Keep confirmed details from phase_scope, concept_grounding, and frontend_experience in structured fields even when final_summary is concise."
        ],
        "confirmedDataShape": block_confirmed_data_shape(&ClarificationBlockName::FinalSummary)
    })
}

fn candidate_write_rules() -> Value {
    json!([
        "Write only the Brainstorm candidate target after the user explicitly confirms final_summary.",
        "Use outputContract.resultTemplate as the concrete field shape when writing the candidate.",
        "clarificationProgress must use confirmedBlocks/skippedBlocks/finalSummaryConfirmed. Do not write completedBlocks or currentBlock.",
        "In user-facing summaries and confirmation text, use user-visible block names rather than internal block ids.",
        "Keep knowledge metadata out of candidate sourceRefs and summary fields.",
        "Preserve all confirmed block details in scope, acceptance, domainModel.businessFlows, conceptGrounding, and frontendExperience instead of relying on final_summary text."
    ])
}

fn requirement_semantic_compact_rules() -> Value {
    json!([
        "Preserve the confirmed current-phase semantics in existing Brainstorm candidate fields; avoid vague labels.",
        "When business detail applies, confirm and write objects, operations, rules, fields, blockers, outcomes, and page paths in the owning blocks.",
        "When business detail does not apply, state the concrete non-domain reason rather than fabricating domain rules.",
        "If a required semantic detail is unclear after reading the requirement and inspected knowledge, ask the user before accept."
    ])
}
