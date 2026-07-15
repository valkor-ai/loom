use contracts::{BrowserVerificationProfile, CodeQualityRequirement, TaskDefinition, TaskPlan};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(crate) const FRONTEND_QUALITY_CONTRACT_READ_FIELDS: [&str; 12] = [
    "task.frontendExperienceRequirement.uiSurfaceDecisionContractRef",
    "task.frontendExperienceRequirement.uiSurfaceOwnership",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.contractRef",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.selectionMode",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.patternDecision",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.regionsInScope",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.actionsInScope",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.statesInScope",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.contentBoundary",
    "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.qualityRulesInScope",
    "task.frontendExperienceRequirement.executionGuidance.styleAssetPlan.referencePlan",
    "task.frontendExperienceRequirement.executionGuidance.styleAssetPlan.designTokenAssetPlan",
];

pub(crate) fn frontend_surface_contract_applies(task: &TaskDefinition) -> bool {
    task.frontend_experience_requirement
        .as_ref()
        .is_some_and(|requirement| {
            requirement.get("uiSurfaceDecisionContractRef").is_some()
                || requirement.get("uiSurfaceOwnership").is_some()
                || requirement
                    .pointer("/executionGuidance/uiProductionBrief/surfaceDecisionContract")
                    .is_some_and(Value::is_object)
        })
}

pub(crate) fn taskplan_outline_result_template() -> Value {
    json!({
        "status": "ready",
        "groups": [{
            "groupId": "group-current-capability",
            "title": "Current capability group",
            "objective": "Deliver one taskable current-phase capability slice.",
            "dependsOn": [],
            "scopeRefs": ["allowedRefs.scopeRefs item"],
            "acceptanceRefs": ["allowedRefs.acceptanceRefs item"],
            "taskIds": ["task-current-001"]
        }],
        "blockedReasons": []
    })
}

pub(crate) fn taskplan_group_result_template() -> Value {
    json!({
        "status": "ready",
        "group": {
            "groupId": "group-current-capability",
            "title": "Current capability group",
            "objective": "Deliver one taskable current-phase capability slice.",
            "dependsOn": [],
            "scopeRefs": ["allowedRefs.scopeRefs item"],
            "acceptanceRefs": ["allowedRefs.acceptanceRefs item"],
            "taskIds": ["task-current-001"]
        },
        "tasks": [{
            "taskId": "task-current-001",
            "groupId": "group-current-capability",
            "title": "",
            "taskKind": "feature_increment",
            "implementationActions": ["create_or_update_business_rule"],
            "objective": "",
            "dependsOn": [],
            "scopeRefs": ["allowedRefs.scopeRefs item"],
            "acceptanceRefs": ["allowedRefs.acceptanceRefs item"],
            "requirementDetailRefs": ["allowedRefs.requirementDetailIds item"],
            "writeBoundary": {
                "forbiddenPaths": [".loom"],
                "artifactRefs": {
                    "modules": [],
                    "entities": [],
                    "interfaces": [],
                    "userFlows": [],
                    "stateMachines": [],
                    "decisions": [],
                    "nfrs": [],
                    "risks": []
                }
            },
            "verificationIntents": [{
                "verificationId": "verify-task-current-001",
                "acceptanceRefs": ["allowedRefs.acceptanceRefs item"],
                "requirementDetailRefs": ["allowedRefs.requirementDetailIds item"],
                "behavior": "",
                "preferredEvidence": ["automated_test"],
                "acceptableEvidence": ["automated_test", "manual_command_output", "static_check"]
            }],
            "conceptRefs": ["contextProjection.requirementDetailTransfer.conceptRefs item"],
            "conceptResponsibilities": [{
                "conceptRef": "contextProjection.requirementDetailTransfer.conceptRefs item",
                "responsibility": "How this task preserves or implements that concept."
            }],
            "conceptVerificationIntents": [{
                "conceptRef": "contextProjection.requirementDetailTransfer.conceptRefs item",
                "evidenceType": "static_check",
                "intent": "How verification will prove this task preserved or implemented that concept."
            }]
        }],
        "blockedReasons": []
    })
}

pub(crate) fn runtime_delivery_requirement_template(runtime_delivery: Option<&Value>) -> Value {
    if runtime_delivery.is_none() {
        return Value::Null;
    }
    json!({
        "appliesToThisTask": true,
        "reason": "Why this task changes build, start, runtime entry, static serving, generated artifacts, or runtime surface.",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": [{
            "objective": "Verify this task preserves the runtime delivery contract it touches.",
            "acceptableEvidence": ["manual_command_output", "runtime_api_check", "static_check"]
        }],
        "evidenceExpectedInTaskResult": [],
        "forbiddenActions": []
    })
}

pub(crate) fn code_quality_requirements_for_task(
    task_plan: &TaskPlan,
    task: &TaskDefinition,
) -> Vec<CodeQualityRequirement> {
    if task.code_quality_requirement_refs.is_empty() {
        return vec![];
    }
    let refs = task
        .code_quality_requirement_refs
        .iter()
        .collect::<BTreeSet<_>>();
    task_plan
        .code_quality_requirements
        .iter()
        .filter(|requirement| refs.contains(&requirement.requirement_id))
        .cloned()
        .collect()
}

pub(crate) fn code_quality_execution_context(
    code_quality_requirements: &[CodeQualityRequirement],
) -> Value {
    Value::Array(
        code_quality_requirements
            .iter()
            .map(|requirement| {
                json!({
                    "requirementId": requirement.requirement_id,
                    "kind": requirement.kind,
                    "appliesToTaskIds": requirement.applies_to_task_ids,
                    "referenceGroups": requirement.reference_groups,
                    "referenceLoadPlan": requirement.reference_load_plan,
                    "packageNamingPolicy": requirement.package_naming_policy,
                    "focusTags": requirement.focus_tags
                })
            })
            .collect(),
    )
}

pub(crate) fn task_result_template_with_code_quality(
    task: &TaskDefinition,
    code_quality_requirements: &[CodeQualityRequirement],
    browser_profile: Option<&BrowserVerificationProfile>,
) -> Value {
    let verification_results = task
        .verification_intents
        .iter()
        .map(|intent| {
            let browser_checks = browser_profile
                .map(|profile| {
                    profile
                        .checks
                        .iter()
                        .filter(|check| check.verification_id == intent.verification_id)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut result = json!({
                "status": if browser_checks.is_empty() { "passed" } else { "not_run" },
                "evidenceType": if browser_checks.is_empty() { "automated_test" } else { "browser_automation" },
                "summary": ""
            });
            if !browser_checks.is_empty() {
                result["browserChecks"] = Value::Array(
                    browser_checks
                        .into_iter()
                        .map(|_| {
                            json!({
                                "status": "not_run",
                                "command": "",
                                "attempts": 0,
                                "artifactRefs": [],
                                "observedOutcome": "",
                                "blockedReason": null
                            })
                        })
                        .collect(),
                );
            }
            result
        })
        .collect::<Vec<_>>();
    let requirement_detail_evidence = task
        .requirement_detail_refs
        .iter()
        .map(|_| {
            json!({
                "status": "satisfied",
                "evidenceRefs": [],
                "summary": ""
            })
        })
        .collect::<Vec<_>>();
    let mut template = json!({
        "status": "completed",
        "changedFiles": [],
        "noChangeReason": null,
        "verificationResults": verification_results,
        "selfRepairSummary": {
            "attempted": false,
            "attemptCount": 0,
            "stopReason": "not_attempted",
            "progressObserved": false
        },
        "failure": null,
        "executionContinuity": {
            "taskResultSubmittedAfterVerification": true,
            "agentOwnedLongRunningWork": "none",
            "notes": []
        },
        "notes": [],
        "requirementDetailEvidence": requirement_detail_evidence,
        "blockedReasons": []
    });
    let Some(object) = template.as_object_mut() else {
        return template;
    };
    if frontend_self_check_applies(task) {
        object.insert(
            "frontendExperienceSelfCheck".to_string(),
            frontend_experience_self_check_template(task),
        );
    }
    if frontend_quality_self_check_applies(task) {
        object.insert(
            "frontendQualitySelfCheck".to_string(),
            frontend_quality_self_check_template(task),
        );
    }
    if runtime_delivery_evidence_applies(task) {
        object.insert(
            "runtimeDeliveryEvidence".to_string(),
            runtime_delivery_evidence_template(task),
        );
    }
    if !task.concept_refs.is_empty() {
        object.insert(
            "conceptEvidence".to_string(),
            Value::Array(
                task.concept_refs
                    .iter()
                    .map(|_| {
                        json!({
                            "evidenceType": "code",
                            "refs": [],
                            "summary": ""
                        })
                    })
                    .collect(),
            ),
        );
    }
    if architecture_quality_evidence_applies(task) {
        object.insert(
            "architectureQualityEvidence".to_string(),
            architecture_quality_evidence_template(task),
        );
    }
    if api_contract_evidence_applies(task) {
        object.insert(
            "apiContractEvidence".to_string(),
            api_contract_evidence_template(task),
        );
    }
    if code_quality_evidence_applies(task) {
        object.insert(
            "codeQualityEvidence".to_string(),
            code_quality_evidence_template(task, code_quality_requirements),
        );
    }
    template
}

pub(crate) fn task_result_schema_shape(
    task: &TaskDefinition,
    browser_profile: Option<&BrowserVerificationProfile>,
) -> Value {
    let mut properties = serde_json::Map::new();
    properties.insert(
        "status".to_string(),
        json!("completed | completed_with_notes | blocked | failed"),
    );
    properties.insert("changedFiles".to_string(), json!(["project-relative path"]));
    properties.insert(
        "noChangeReason".to_string(),
        json!({
            "shape": "object or null",
            "code": "string",
            "summary": "string"
        }),
    );
    let mut verification_shape = json!({
        "status": "passed | not_run | failed | inconclusive",
        "evidenceType": "one of the matching verification intent acceptableEvidence values",
        "summary": "string"
    });
    if browser_profile.is_some() {
        verification_shape["browserChecks"] = json!([{
            "status": "passed | failed | blocked | not_run",
            "command": "exact command used for this check",
            "attempts": "non-negative integer; passed requires at least 1",
            "artifactRefs": ["project-relative trace, screenshot, or report ref"],
            "observedOutcome": "concise visible or behavioral outcome",
            "blockedReason": "concrete string when status=blocked, otherwise null"
        }]);
    }
    properties.insert(
        "verificationResults".to_string(),
        Value::Array(vec![verification_shape]),
    );
    properties.insert(
        "selfRepairSummary".to_string(),
        json!({
            "attempted": false,
            "attemptCount": 0,
            "stopReason": "not_attempted | verification_passed | blocked_condition_detected | same_failure_repeated_without_progress | hard_attempt_limit_reached | repair_requires_contract_change | repair_requires_scope_expansion",
            "progressObserved": false
        }),
    );
    properties.insert(
        "failure".to_string(),
        json!({
            "shape": "object or null",
            "code": "required only when status=failed",
            "summary": "required only when status=failed"
        }),
    );
    properties.insert(
        "executionContinuity".to_string(),
        json!({
            "taskResultSubmittedAfterVerification": true,
            "agentOwnedLongRunningWork": "none | stopped | handed_off",
            "notes": ["string"]
        }),
    );
    properties.insert("notes".to_string(), json!(["string"]));
    properties.insert(
        "requirementDetailEvidence".to_string(),
        json!([{
            "status": "satisfied | partial | not_verified",
            "evidenceRefs": ["project-relative evidence ref"],
            "summary": "string"
        }]),
    );
    properties.insert(
        "blockedReasons".to_string(),
        json!([{
            "code": "outputContract.blockedReasonOptions[].code",
            "nextNode": "outputContract.blockedReasonOptions[].nextNode",
            "message": "string",
            "details": {}
        }]),
    );

    if frontend_self_check_applies(task) {
        properties.insert(
            "frontendExperienceSelfCheck".to_string(),
            json!({
                "status": "satisfied | partial | blocked",
                "dataBinding": {
                    "mode": "wired | partial | not_applicable",
                    "knownGaps": ["string"]
                },
                "evidenceRefs": ["project-relative evidence ref"],
                "summary": "string"
            }),
        );
    }
    if frontend_quality_self_check_applies(task) {
        properties.insert(
            "frontendQualitySelfCheck".to_string(),
            json!({
                "status": "satisfied | partial | missing | blocked_by_environment",
                "surfaceRegionEvidence": [{
                    "status": "satisfied | partial | missing | blocked_by_environment",
                    "files": ["project-relative UI file"],
                    "states": ["covered state id"],
                    "actions": ["covered action id"],
                    "evidence": "string"
                }],
                "surfaceActionEvidence": [{
                    "status": "satisfied | partial | missing | blocked_by_environment",
                    "files": ["project-relative UI file"],
                    "states": ["covered state id"],
                    "actions": ["covered action id"],
                    "evidence": "string"
                }],
                "surfaceStateEvidence": [{
                    "status": "satisfied | partial | missing | blocked_by_environment",
                    "files": ["project-relative UI file"],
                    "states": ["covered state id"],
                    "actions": ["covered action id"],
                    "evidence": "string"
                }],
                "surfaceQualityRuleEvidence": [{
                    "status": "satisfied | partial | missing | blocked_by_environment",
                    "files": ["project-relative UI file or check"],
                    "states": ["covered state id"],
                    "actions": ["covered action id"],
                    "evidence": "string"
                }],
                "contentBoundaryEvidence": {
                    "checked": true,
                    "allowedContentExamples": ["string"],
                    "forbiddenContentViolations": ["string"],
                    "evidence": "string"
                },
                "referencePlanFilesChecked": ["task.frontendExperienceRequirement.executionGuidance.styleAssetPlan.referencePlan[].path"],
                "designTokenEvidence": {
                    "strategyUsed": "reuse_existing | extend_existing | create_css_tokens | create_tailwind_tokens | not_applicable",
                    "templateIdUsed": "tokens-css | tokens-tailwind | null",
                    "tokenAssetFiles": ["project-relative token asset file"],
                    "tokenConsumerFiles": ["project-relative UI file using declared tokens"],
                    "existingTokenSystemReused": true,
                    "parallelTokenSystemCreated": false,
                    "mergeSummary": "string"
                },
                "knownGaps": ["string"],
                "summary": "string"
            }),
        );
    }
    if runtime_delivery_evidence_applies(task) {
        properties.insert(
            "runtimeDeliveryEvidence".to_string(),
            json!({
                "codeLevelChecks": [{
                    "status": "passed | failed | blocked | not_applicable",
                    "evidence": "string"
                }],
                "commandsRun": ["command"],
                "unverifiedItems": ["string"],
                "runtimeProbeCleanup": "string or null"
            }),
        );
    }
    if !task.concept_refs.is_empty() {
        properties.insert(
            "conceptEvidence".to_string(),
            json!([{
                "evidenceType": "code | test | runtime | manual",
                "refs": ["project-relative ref"],
                "summary": "string"
            }]),
        );
    }
    if architecture_quality_evidence_applies(task) {
        properties.insert(
            "architectureQualityEvidence".to_string(),
            json!([{
                "status": "satisfied | partial | not_verified",
                "changedFiles": ["project-relative path"],
                "summary": "string"
            }]),
        );
    }
    if api_contract_evidence_applies(task) {
        properties.insert(
            "apiContractEvidence".to_string(),
            json!([{
                "status": "satisfied | partial | not_verified",
                "changedFiles": ["project-relative path"],
                "successPaths": ["HTTP method/path or operation id"],
                "errorPaths": ["HTTP method/path or operation id"],
                "paginationPaths": ["HTTP method/path or operation id"],
                "contractFileRefs": ["project-relative contract file"],
                "knownGaps": ["string"],
                "summary": "string"
            }]),
        );
    }
    if code_quality_evidence_applies(task) {
        properties.insert(
            "codeQualityEvidence".to_string(),
            json!([{
                "status": "satisfied | partial | not_verified",
                "referenceGroupsChecked": {"language_or_framework": ["group"]},
                "referenceFilesChecked": ["reference path"],
                "changedFiles": ["project-relative path"],
                "commandsRun": ["command"],
                "knownGaps": ["string"],
                "summary": "string"
            }]),
        );
    }

    json!({
        "type": "object",
        "required": task_result_required_top_level_fields(task),
        "properties": properties,
        "additionalProperties": false
    })
}

pub(crate) fn task_result_required_top_level_fields(task: &TaskDefinition) -> Vec<&'static str> {
    let mut fields = vec![
        "status",
        "changedFiles",
        "noChangeReason",
        "verificationResults",
        "selfRepairSummary",
        "failure",
        "executionContinuity",
        "notes",
        "requirementDetailEvidence",
        "blockedReasons",
    ];
    if frontend_self_check_applies(task) {
        fields.push("frontendExperienceSelfCheck");
    }
    if frontend_quality_self_check_applies(task) {
        fields.push("frontendQualitySelfCheck");
    }
    if runtime_delivery_evidence_applies(task) {
        fields.push("runtimeDeliveryEvidence");
    }
    if !task.concept_refs.is_empty() {
        fields.push("conceptEvidence");
    }
    if architecture_quality_evidence_applies(task) {
        fields.push("architectureQualityEvidence");
    }
    if api_contract_evidence_applies(task) {
        fields.push("apiContractEvidence");
    }
    if code_quality_evidence_applies(task) {
        fields.push("codeQualityEvidence");
    }
    fields
}

pub(crate) fn architecture_quality_evidence_applies(task: &TaskDefinition) -> bool {
    !task.architecture_quality_requirement_refs.is_empty()
}

pub(crate) fn api_contract_evidence_applies(task: &TaskDefinition) -> bool {
    !task.api_contract_requirement_refs.is_empty()
}

pub(crate) fn code_quality_evidence_applies(task: &TaskDefinition) -> bool {
    !task.code_quality_requirement_refs.is_empty()
}

pub(crate) fn runtime_delivery_evidence_applies(task: &TaskDefinition) -> bool {
    task.runtime_delivery_requirement
        .as_ref()
        .map(|requirement| requirement.applies_to_this_task)
        .unwrap_or(false)
}

pub(crate) fn frontend_self_check_applies(task: &TaskDefinition) -> bool {
    task.frontend_experience_requirement.is_some() && !runtime_delivery_evidence_applies(task)
}

pub(crate) fn frontend_quality_self_check_applies(task: &TaskDefinition) -> bool {
    frontend_self_check_applies(task) && frontend_surface_contract_applies(task)
}

fn runtime_delivery_evidence_template(task: &TaskDefinition) -> Value {
    let Some(requirement) = &task.runtime_delivery_requirement else {
        return Value::Null;
    };
    if !requirement.applies_to_this_task {
        return Value::Null;
    }
    let code_level_checks = requirement
        .required_code_level_checks
        .iter()
        .map(|_| {
            json!({
                "status": "passed",
                "evidence": ""
            })
        })
        .collect::<Vec<_>>();
    json!({
        "codeLevelChecks": code_level_checks,
        "commandsRun": [],
        "unverifiedItems": [],
        "runtimeProbeCleanup": null
    })
}

fn architecture_quality_evidence_template(task: &TaskDefinition) -> Value {
    Value::Array(
        task.architecture_quality_requirement_refs
            .iter()
            .map(|_| {
                json!({
                    "status": "satisfied",
                    "changedFiles": [],
                    "summary": ""
                })
            })
            .collect(),
    )
}

fn api_contract_evidence_template(task: &TaskDefinition) -> Value {
    Value::Array(
        task.api_contract_requirement_refs
            .iter()
            .map(|_| {
                json!({
                    "status": "satisfied",
                    "changedFiles": [],
                    "successPaths": [],
                    "errorPaths": [],
                    "paginationPaths": [],
                    "contractFileRefs": [],
                    "knownGaps": [],
                    "summary": ""
                })
            })
            .collect(),
    )
}

fn code_quality_evidence_template(
    task: &TaskDefinition,
    code_quality_requirements: &[CodeQualityRequirement],
) -> Value {
    Value::Array(
        task.code_quality_requirement_refs
            .iter()
            .map(|requirement_id| {
                let reference_groups = code_quality_requirements
                    .iter()
                    .find(|requirement| &requirement.requirement_id == requirement_id)
                    .map(|requirement| json!(requirement.reference_groups))
                    .unwrap_or_else(|| json!({}));
                let reference_files = code_quality_requirements
                    .iter()
                    .find(|requirement| &requirement.requirement_id == requirement_id)
                    .map(|requirement| {
                        requirement
                            .reference_load_plan
                            .iter()
                            .map(|item| item.path.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                json!({
                    "status": "satisfied",
                    "referenceGroupsChecked": reference_groups,
                    "referenceFilesChecked": reference_files,
                    "changedFiles": [],
                    "commandsRun": [],
                    "knownGaps": [],
                    "summary": "Explain how the changed files followed the selected code quality references and existing repository style."
                })
            })
            .collect(),
    )
}

fn frontend_experience_self_check_template(task: &TaskDefinition) -> Value {
    if task.frontend_experience_requirement.is_none() {
        return Value::Null;
    }
    json!({
        "status": "satisfied",
        "dataBinding": {
            "mode": "wired",
            "knownGaps": []
        },
        "evidenceRefs": [],
        "summary": ""
    })
}

fn frontend_quality_self_check_template(task: &TaskDefinition) -> Value {
    let frontend_execution_guidance = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("executionGuidance"));
    let surface_decision_contract = frontend_execution_guidance
        .and_then(|guidance| guidance.pointer("/uiProductionBrief/surfaceDecisionContract"))
        .unwrap_or(&Value::Null);
    let reference_plan_files = frontend_execution_guidance
        .and_then(|guidance| guidance.pointer("/styleAssetPlan/referencePlan"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("path").and_then(Value::as_str).map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let surface_state_ids = surface_decision_contract
        .get("statesInScope")
        .and_then(Value::as_array)
        .map(|states| {
            states
                .iter()
                .filter_map(|state| state.get("state").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let surface_region_evidence = surface_decision_contract
        .get("regionsInScope")
        .and_then(Value::as_array)
        .map(|regions| {
            regions
                .iter()
                .filter_map(|region| {
                    region.get("regionId").and_then(Value::as_str).map(|_| {
                        json!({
                            "status": "satisfied",
                            "files": ["replace_with_ui_file_path_for_this_region"],
                            "states": merged_state_refs(region, &surface_state_ids),
                            "actions": region.get("actionRefs").cloned().unwrap_or_else(|| json!([])),
                            "evidence": ""
                        })
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let surface_action_evidence = surface_decision_contract
        .get("actionsInScope")
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter_map(|action| {
                    action.get("actionId").and_then(Value::as_str).map(|id| {
                        json!({
                            "status": "satisfied",
                            "files": ["replace_with_ui_file_path_for_this_action"],
                            "states": [],
                            "actions": [id],
                            "evidence": ""
                        })
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let surface_state_evidence = surface_decision_contract
        .get("statesInScope")
        .and_then(Value::as_array)
        .map(|states| {
            states
                .iter()
                .filter_map(|state| {
                    state.get("state").and_then(Value::as_str).map(|id| {
                        json!({
                            "status": "satisfied",
                            "files": ["replace_with_ui_file_path_for_this_state"],
                            "states": [id],
                            "actions": [],
                            "evidence": ""
                        })
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let surface_quality_rule_evidence = surface_decision_contract
        .get("qualityRulesInScope")
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| {
                    rule.get("ruleId").and_then(Value::as_str).map(|_| {
                        json!({
                            "status": "satisfied",
                            "files": ["replace_with_ui_file_path_or_check_for_this_rule"],
                            "states": [],
                            "actions": [],
                            "evidence": ""
                        })
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let design_token_plan = frontend_execution_guidance
        .and_then(|guidance| guidance.pointer("/styleAssetPlan/designTokenAssetPlan"))
        .cloned()
        .unwrap_or(Value::Null);
    let token_strategy = design_token_plan
        .get("strategy")
        .and_then(Value::as_str)
        .unwrap_or("not_applicable");
    let template_id = design_token_plan
        .get("templateId")
        .cloned()
        .unwrap_or(Value::Null);
    let token_asset_files = design_token_plan
        .get("targetFiles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    json!({
        "status": "partial",
        "surfaceRegionEvidence": surface_region_evidence,
        "surfaceActionEvidence": surface_action_evidence,
        "surfaceStateEvidence": surface_state_evidence,
        "surfaceQualityRuleEvidence": surface_quality_rule_evidence,
        "contentBoundaryEvidence": {
            "checked": true,
            "allowedContentExamples": [],
            "forbiddenContentViolations": [],
            "evidence": ""
        },
        "referencePlanFilesChecked": reference_plan_files,
        "designTokenEvidence": {
            "strategyUsed": token_strategy,
            "templateIdUsed": template_id,
            "tokenAssetFiles": token_asset_files,
            "tokenConsumerFiles": ["replace_with_ui_file_using_declared_tokens"],
            "existingTokenSystemReused": matches!(token_strategy, "reuse_existing" | "extend_existing"),
            "parallelTokenSystemCreated": false,
            "mergeSummary": ""
        },
        "knownGaps": [],
        "summary": ""
    })
}

fn merged_state_refs(region: &Value, surface_state_ids: &[String]) -> Value {
    let mut states = region
        .get("stateRefs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    states.extend(surface_state_ids.iter().cloned());
    Value::Array(states.into_iter().map(Value::String).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frontend_task(requirement: Value) -> TaskDefinition {
        serde_json::from_value(json!({
            "taskId": "task-ui-001",
            "groupId": "group-ui",
            "title": "UI task",
            "taskKind": "frontend_experience",
            "implementationActions": ["implement_frontend_experience_contract"],
            "objective": "Implement the UI surface.",
            "dependsOn": [],
            "scopeRefs": [],
            "acceptanceRefs": [],
            "requirementDetailRefs": [],
            "writeBoundary": {
                "forbiddenPaths": [],
                "artifactRefs": {}
            },
            "verificationIntents": [],
            "conceptRefs": [],
            "conceptResponsibilities": [],
            "conceptVerificationIntents": [],
            "frontendExperienceRequirement": requirement,
            "architectureQualityRequirementRefs": [],
            "apiContractRequirementRefs": [],
            "codeQualityRequirementRefs": []
        }))
        .expect("valid task")
    }

    #[test]
    fn surface_contract_read_fields_do_not_include_legacy_quality_contract() {
        let task = frontend_task(json!({
            "uiSurfaceDecisionContractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
            "executionGuidance": {
                "uiProductionBrief": {
                    "surfaceDecisionContract": {
                        "contractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract"
                    }
                }
            }
        }));

        let fields = FRONTEND_QUALITY_CONTRACT_READ_FIELDS;

        assert!(frontend_quality_self_check_applies(&task));
        assert!(fields.contains(
            &"task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.contractRef"
        ));
        assert!(
            fields
                .iter()
                .all(|field| !field.contains("uiQualityContract")
                    && !field.contains("uiTaskQualityGates")),
            "{fields:#?}"
        );
    }

    #[test]
    fn frontend_quality_template_does_not_emit_legacy_self_check_fields() {
        let task = frontend_task(json!({
            "uiSurfaceDecisionContractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
            "executionGuidance": {
                "uiProductionBrief": {
                    "surfaceDecisionContract": {
                        "contractRef": "sourceRefs.architectureArtifactContractRef#/frontendExperience/uiSurfaceDecisionContract",
                        "regionsInScope": [{"regionId": "region_primary"}],
                        "actionsInScope": [{"actionId": "action_create"}],
                        "statesInScope": [{"state": "loading"}],
                        "qualityRulesInScope": [{"ruleId": "surface.contract.evidence_coverage"}]
                    }
                },
                "styleAssetPlan": {
                    "referencePlan": [{"path": "uix/core.md"}],
                    "designTokenAssetPlan": {
                        "strategy": "create_css_tokens",
                        "templateId": "tokens-css",
                        "targetFiles": ["src/styles/tokens.css"]
                    }
                }
            }
        }));
        let template = frontend_quality_self_check_template(&task);

        for key in [
            "scenarioKind",
            "qualityLevel",
            "referenceGroupsChecked",
            "referenceFilesChecked",
            "statesCovered",
            "businessUiRulesChecked",
            "forbiddenContentCheck",
            "surfacesCovered",
            "gateResults",
        ] {
            assert!(
                template.get(key).is_none(),
                "frontend quality template must not emit legacy field {key}: {template:#}"
            );
        }
    }

    #[test]
    fn task_result_template_exposes_browser_outcomes_without_agent_authored_linkage() {
        let mut task = frontend_task(json!({
            "uiSurfaceDecisionContractRef": "surface-contract",
            "executionGuidance": {
                "uiProductionBrief": {
                    "surfaceDecisionContract": {"contractRef": "surface-contract"}
                }
            }
        }));
        task.verification_intents = serde_json::from_value(json!([{
            "verificationId": "verify-ui",
            "requirementDetailRefs": [],
            "behavior": "Verify the UI.",
            "preferredEvidence": ["automated_test"],
            "acceptableEvidence": ["automated_test"]
        }]))
        .expect("verification intents");
        let profile = BrowserVerificationProfile {
            profile_id: "browser-task-ui-001".to_string(),
            task_id: task.task_id.clone(),
            mode: contracts::BrowserVerificationMode::RenderedInspection,
            runner_source: contracts::BrowserRunnerSource::LoomManaged,
            installation_id: None,
            verification_ids: vec!["verify-ui".to_string()],
            surface_refs: vec![],
            workflow_refs: vec![],
            region_refs: vec![],
            action_refs: vec![],
            state_refs: vec![],
            quality_rule_refs: vec![],
            checks: vec![contracts::BrowserVerificationCheck {
                check_id: "browser-ui-desktop".to_string(),
                verification_id: "verify-ui".to_string(),
                source_task_id: "task-ui".to_string(),
                source_verification_id: "verify-ui".to_string(),
                enforcement: contracts::BrowserEvidenceEnforcement::Supplemental,
                viewport_ref: "desktop_primary".to_string(),
                backend_mode: contracts::BrowserBackendMode::NotApplicable,
            }],
            reference_load_plan: vec![],
        };

        let template = task_result_template_with_code_quality(&task, &[], Some(&profile));

        assert!(template["verificationResults"][0]["browserChecks"]
            .get("checkId")
            .is_none());
        assert_eq!(
            template["verificationResults"][0]["browserChecks"][0]["status"],
            json!("not_run")
        );
        assert_eq!(
            template["verificationResults"][0]["status"],
            json!("not_run")
        );
        assert_eq!(
            template["verificationResults"][0]["evidenceType"],
            json!("browser_automation")
        );
        assert!(template["frontendQualitySelfCheck"]
            .get("browserCheckRefs")
            .is_none());
    }
}
