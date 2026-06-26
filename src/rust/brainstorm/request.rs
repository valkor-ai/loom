use std::path::Path;

use contracts::{
    BrainstormCandidateAgentWritable, ClarificationBlockName, UserFacingLanguageConstraint,
};
use delivery_core::{ArtifactKind, RouteAction, RouteActionKind, WriteMode};
use schemars::schema_for;
use serde_json::{json, Value};
use state::paths::to_project_relative;

use crate::{gate::required_blocks, paths::brainstorm_agent_candidate_file};

pub fn build_brainstorm_request_root(
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
    let phase_scope_rules = phase_scope_rules();
    let concept_rules = concept_grounding_rules();
    let frontend_rules = frontend_experience_rules();
    let final_summary_rules = final_summary_rules();
    let semantic_rules = requirement_semantic_compact_rules();

    json!({
        "schemaVersion": "1.0",
        "requestType": "brainstorm_session",
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "brainstormRunId": brainstorm_run_id,
        "userFacingLanguage": user_facing_language,
        "contextRefs": context_refs,
        "clarificationConversationProtocol": {
            "mode": "progressive_blocks",
            "currentBlock": ClarificationBlockName::PhaseScope,
            "requiredBlocks": required_blocks(),
            "blockSequenceRule": "Process phase_scope, then concept_grounding, then frontend_experience, then final_summary. Do not skip ahead to writing the Brainstorm candidate.",
            "userFacingLanguageRule": user_facing_language.rule,
            "blockExecutionRules": {
                "phase_scope": [
                    "Read conversation_protocol, requirement_context, and phase_scope_rules before presenting phase_scope.",
                    "For every phase_scope step in knowledge_context_plan, call loom.knowledgeBrainstormContext before recommending the current phase cut.",
                    "Present 2-3 source-grounded scope options and show one recommendation; do not directly ask the user to approve an unstated internal scope."
                ],
                "concept_grounding": [
                    "Read concept_grounding_rules after phase_scope is confirmed.",
                    "Use only the confirmed current-phase scope items as the concept_grounding subject set.",
                    "Show business objects, operations, fields, states, blockers, outcomes, and misunderstanding boundaries to the user before confirmation."
                ],
                "frontend_experience": [
                    "Read frontend_experience_rules only after concept_grounding is confirmed.",
                    "Confirm page or workspace operation paths from the already confirmed scope and business rules.",
                    "If UI is not applicable, state the concrete reason and mark frontend_experience as skipped rather than inventing a page target."
                ],
                "final_summary": [
                    "Do not call knowledge context for final_summary.",
                    "Use final_summary as the pre-submit coverage checklist, not as the first place where detailed requirements appear.",
                    "If the user corrects the summary, write the correction back into structured fields before confirming final_summary."
                ]
            },
            "blockConfirmationRules": {
                "phase_scope": "Wait for explicit user confirmation of the current phase cut before moving to concept_grounding.",
                "concept_grounding": "Wait for explicit user confirmation of business understanding and rules before moving to frontend_experience.",
                "frontend_experience": "Wait for explicit user confirmation of the page/workspace path, or record a concrete skip reason, before moving to final_summary.",
                "final_summary": "Write and submit the Brainstorm candidate only after the user explicitly confirms the final_summary coverage checklist."
            },
            "noPrematureSubmitRule": "Do not read candidate_write_contract or write the Brainstorm candidate before final_summary is explicitly confirmed."
        },
        "knowledgeQueryPlan": knowledge_query_plan(),
        "rules": {
            "phaseScope": phase_scope_rules,
            "conceptGrounding": concept_rules,
            "frontendExperience": frontend_rules,
            "finalSummary": final_summary_rules,
            "candidateWrite": candidate_write_rules(),
            "requirementSemanticGrounding": {
                "compactRules": semantic_rules
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
                    "groupId": "conversation_protocol",
                    "required": true,
                    "purpose": "Read the Brainstorm conversation contract and the current block discipline before asking the user anything.",
                    "whenToRead": "Read at the beginning of the Brainstorm conversation and keep it authoritative for the whole request.",
                    "fields": [
                        "userFacingLanguage",
                        "clarificationConversationProtocol.currentBlock",
                        "clarificationConversationProtocol.requiredBlocks",
                        "clarificationConversationProtocol.blockSequenceRule",
                        "clarificationConversationProtocol.userFacingLanguageRule",
                        "clarificationConversationProtocol.blockExecutionRules.phase_scope",
                        "clarificationConversationProtocol.blockExecutionRules.concept_grounding",
                        "clarificationConversationProtocol.blockExecutionRules.frontend_experience",
                        "clarificationConversationProtocol.blockExecutionRules.final_summary",
                        "clarificationConversationProtocol.blockConfirmationRules.phase_scope",
                        "clarificationConversationProtocol.blockConfirmationRules.concept_grounding",
                        "clarificationConversationProtocol.blockConfirmationRules.frontend_experience",
                        "clarificationConversationProtocol.blockConfirmationRules.final_summary",
                        "clarificationConversationProtocol.noPrematureSubmitRule"
                    ]
                },
                {
                    "groupId": "requirement_context",
                    "required": true,
                    "purpose": "Read compact source metadata and requirement hints before forming clarification options or rule summaries.",
                    "whenToRead": "Read before phase_scope and return here whenever the confirmed source authority is unclear.",
                    "fields": [
                        "requirementContext.sourceItems",
                        "keywordHints.compact"
                    ]
                },
                {
                    "groupId": "requirement_full_text",
                    "required": false,
                    "purpose": "Read the full normalized requirement text only when compact source metadata, keyword hints, and request-scoped knowledge context are insufficient for the current clarification block.",
                    "whenToRead": "Read on demand only for the active block; do not read it as the default phase_scope context.",
                    "fields": [
                        "requirementContext.normalizedText"
                    ]
                },
                {
                    "groupId": "phase_scope_rules",
                    "required": true,
                    "purpose": "Read the current-phase cut rules before presenting phase_scope options.",
                    "whenToRead": "Read immediately before presenting phase_scope.",
                    "fields": [
                        "rules.phaseScope.optionComparison",
                        "rules.phaseScope.selfCheck",
                        "rules.phaseScope.candidateFieldMapping"
                    ]
                },
                {
                    "groupId": "knowledge_context_plan",
                    "required": false,
                    "purpose": "Read the tool-bound knowledge query plan for phase_scope, concept_grounding, and frontend_experience.",
                    "whenToRead": "Read before each knowledge-enabled block. If the knowledge context tool returns failed, stop and report the tool failure instead of producing a knowledge-free clarification.",
                    "fields": [
                        "knowledgeQueryPlan.sharedRules",
                        "knowledgeQueryPlan.toolContract",
                        "knowledgeQueryPlan.blocks.phase_scope.executionOrder",
                        "knowledgeQueryPlan.blocks.concept_grounding.executionOrder",
                        "knowledgeQueryPlan.blocks.frontend_experience.executionOrder"
                    ]
                },
                {
                    "groupId": "concept_grounding_rules",
                    "required": true,
                    "purpose": "Read the business understanding and rule confirmation contract after phase_scope is confirmed.",
                    "whenToRead": "Read after phase_scope is confirmed and before presenting concept_grounding.",
                    "fields": [
                        "rules.conceptGrounding.presentation",
                        "rules.conceptGrounding.selfCheck",
                        "rules.conceptGrounding.scopeItemCoverage",
                        "rules.conceptGrounding.objectOperation",
                        "rules.conceptGrounding.candidateFieldMapping"
                    ]
                },
                {
                    "groupId": "frontend_experience_rules",
                    "required": false,
                    "purpose": "Read the page/workspace operation path contract after concept_grounding is confirmed.",
                    "whenToRead": "Read after concept_grounding is confirmed and before presenting frontend_experience.",
                    "fields": [
                        "rules.frontendExperience.presentation",
                        "rules.frontendExperience.selfCheck",
                        "rules.frontendExperience.operationPath",
                        "rules.frontendExperience.candidateFieldMapping"
                    ]
                },
                {
                    "groupId": "final_summary_rules",
                    "required": true,
                    "purpose": "Read the pre-submit coverage checklist rules before presenting final_summary.",
                    "whenToRead": "Read after prior blocks are confirmed or skipped and before presenting final_summary.",
                    "fields": [
                        "rules.finalSummary.reviewGate",
                        "rules.finalSummary.requiredUserVisibleTopics",
                        "rules.finalSummary.correctionWriteback",
                        "rules.finalSummary.detailRetention",
                        "rules.requirementSemanticGrounding.compactRules"
                    ]
                },
                {
                    "groupId": "candidate_write_contract",
                    "required": true,
                    "purpose": "Read the compact write contract only after the user explicitly confirms final_summary.",
                    "whenToRead": "Read only after final_summary is explicitly confirmed by the user.",
                    "fields": [
                        "outputContract.writeTargets",
                        "outputContract.submitTool",
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
        "notes": [
            "Machine-owned ids, request binding, accepted status, and handoff routing are added by Loom on accept.",
            "final_summary is the gate before write, not the source of requirement detail."
        ]
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
                        "querySubjectRule": "The subject is the overall dependency order across candidate capability units, not one module's closure.",
                        "queryConstructionRules": [
                            "Use dependency_order only to compare sequencing and deferred boundaries.",
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

fn phase_scope_rules() -> Value {
    json!({
        "optionComparison": [
            "Present 2-3 source-grounded current-phase options with one recommendation.",
            "Each option must show included scope, deferred or not-this-phase boundary, reason, and tradeoff.",
            "The recommended option must preserve the current phase's closure and dependency purpose."
        ],
        "selfCheck": [
            "Verify the recommended option contains goal-essential and flow-support items.",
            "Do not let adjacent or downstream work occupy the current phase unless the user explicitly asks for that wider boundary."
        ],
        "candidateFieldMapping": {
            "scope": ["scope.included", "scope.excluded", "scope.deferred", "scope.assumptions"],
            "roadmap": ["roadmap.currentPhaseId", "roadmap.phases"],
            "phasePlan": ["phasePlan.current", "phasePlan.nextPhasePreview"]
        }
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
        "candidateFieldMapping": {
            "acceptance": ["acceptance"],
            "domainModel": ["domainModel.businessFlows"],
            "concepts": ["conceptGrounding", "conceptConfirmation"],
            "scopeCoverage": ["scope.included[].items"]
        }
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
        "candidateFieldMapping": {
            "frontendExperience": [
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
            ]
        }
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
        ]
    })
}

fn candidate_write_rules() -> Value {
    json!([
        "Write only the Brainstorm candidate target after the user explicitly confirms final_summary.",
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
