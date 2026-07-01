use contracts::TaskDefinition;
use serde_json::{json, Value};

pub(crate) const FRONTEND_QUALITY_CONTRACT_READ_FIELDS: [&str; 21] = [
    "task.frontendExperienceRequirement.uiQualityContract.scenario",
    "task.frontendExperienceRequirement.uiQualityContract.qualityLevel",
    "task.frontendExperienceRequirement.uiQualityContract.surfacePolicy",
    "task.frontendExperienceRequirement.uiQualityContract.layoutBaseline",
    "task.frontendExperienceRequirement.uiQualityContract.density",
    "task.frontendExperienceRequirement.uiQualityContract.semanticTokenPolicy",
    "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceIds",
    "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.loadMode",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.strategy",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.templateId",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.targetFiles",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.tailwindConfigRefs",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.tokenFileRefs",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.globalStyleRefs",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.componentThemeRefs",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.existingStyleEvidence.summary",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.mergePolicy",
    "task.frontendExperienceRequirement.uiQualityContract.designTokenAssetPlan.duplicationPolicy",
    "task.frontendExperienceRequirement.uiQualityContract.forbiddenUserVisibleContent",
    "task.frontendExperienceRequirement.uiQualityContract.requiredUiStates",
    "task.frontendExperienceRequirement.uiQualityContract.businessUiRules",
];

pub(crate) fn taskplan_outline_result_template(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Value {
    json!({
        "schemaVersion": "1.0",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
        "status": "ready",
        "taskPlanId": format!("taskplan-{phase_id}"),
        "groups": [{
            "groupId": "group-current-capability",
            "title": "Current capability group",
            "objective": "Deliver one taskable current-phase capability slice.",
            "dependsOn": [],
            "scopeRefs": ["allowedRefs.scopeRefs item"],
            "acceptanceRefs": ["allowedRefs.acceptanceRefs item"],
            "taskIds": ["task-current-001"]
        }],
        "blockedReasons": [],
        "createdAt": "ISO-8601 datetime"
    })
}

pub(crate) fn taskplan_group_result_template(
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
) -> Value {
    json!({
        "schemaVersion": "1.0",
        "requestId": request_id,
        "deliveryId": delivery_id,
        "phaseId": phase_id,
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
        "blockedReasons": [],
        "createdAt": "ISO-8601 datetime"
    })
}

pub(crate) fn runtime_delivery_requirement_template(runtime_delivery: Option<&Value>) -> Value {
    if runtime_delivery.is_none() {
        return Value::Null;
    }
    json!({
        "appliesToThisTask": true,
        "reason": "Why this task changes build, start, runtime entry, static serving, generated artifacts, or runtime surface.",
        "runtimeDeliveryRef": "sourceRefs.architectureArtifactContractRef#/runtimeDelivery",
        "affectedContractFields": ["runtimeSurfaces"],
        "requiredCodeLevelChecks": [{
            "checkId": "check-task-current-001-runtime",
            "contractField": "runtimeSurfaces",
            "objective": "Verify this task preserves the runtime delivery contract it touches.",
            "acceptableEvidence": ["manual_command_output", "runtime_api_check", "static_check"]
        }],
        "evidenceExpectedInTaskResult": [],
        "forbiddenActions": []
    })
}

pub(crate) fn task_result_template(task_plan_id: &str, task: &TaskDefinition) -> Value {
    let verification_results = task
        .verification_intents
        .iter()
        .map(|intent| {
            json!({
                "verificationId": intent.verification_id,
                "status": "passed",
                "evidenceType": "automated_test",
                "summary": ""
            })
        })
        .collect::<Vec<_>>();
    let requirement_detail_evidence = task
        .requirement_detail_refs
        .iter()
        .map(|detail_id| {
            let verification_ids = template_verification_ids_for_detail(task, detail_id);
            json!({
                "detailId": detail_id,
                "status": "satisfied",
                "verificationIds": verification_ids,
                "evidenceRefs": [],
                "summary": ""
            })
        })
        .collect::<Vec<_>>();
    let mut template = json!({
        "schemaVersion": "1.0",
        "taskResultId": format!("result-{}", task.task_id),
        "taskId": task.task_id,
        "taskPlanId": task_plan_id,
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
        "blockedReasons": [],
        "createdAt": "ISO-8601 datetime",
        "updatedAt": "ISO-8601 datetime"
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
                    .map(|concept_ref| {
                        json!({
                            "conceptRef": concept_ref,
                            "evidenceType": "code",
                            "refs": [],
                            "summary": ""
                        })
                    })
                    .collect(),
            ),
        );
    }
    template
}

fn template_verification_ids_for_detail(task: &TaskDefinition, detail_id: &str) -> Vec<String> {
    let direct = task
        .verification_intents
        .iter()
        .filter(|intent| {
            intent
                .requirement_detail_refs
                .iter()
                .any(|id| id == detail_id)
        })
        .map(|intent| intent.verification_id.clone())
        .collect::<Vec<_>>();
    if !direct.is_empty() {
        return direct;
    }
    if task.verification_intents.len() == 1 {
        return vec![task.verification_intents[0].verification_id.clone()];
    }
    Vec::new()
}

pub(crate) fn task_result_required_top_level_fields(task: &TaskDefinition) -> Vec<&'static str> {
    let mut fields = vec![
        "schemaVersion",
        "taskResultId",
        "taskId",
        "taskPlanId",
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
        "createdAt",
        "updatedAt",
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
    fields
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
    frontend_self_check_applies(task)
        && task
            .frontend_experience_requirement
            .as_ref()
            .and_then(|requirement| requirement.get("uiQualityContract"))
            .is_some()
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
        .map(|check| {
            json!({
                "checkId": check.check_id,
                "contractField": check.contract_field,
                "status": "passed",
                "evidence": ""
            })
        })
        .collect::<Vec<_>>();
    json!({
        "requirementRef": requirement.runtime_delivery_ref,
        "checkedFields": requirement.affected_contract_fields,
        "codeLevelChecks": code_level_checks,
        "commandsRun": [],
        "unverifiedItems": [],
        "runtimeProbeCleanup": null
    })
}

fn frontend_experience_self_check_template(task: &TaskDefinition) -> Value {
    if task.frontend_experience_requirement.is_none() {
        return Value::Null;
    }
    let closure_requirement_ids = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("executionGuidance"))
        .and_then(|guidance| guidance.get("closureRequirementRefs"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        item.get("closureId")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "status": "satisfied",
        "closureRequirementIds": closure_requirement_ids,
        "dataBinding": {
            "mode": "wired",
            "knownGaps": []
        },
        "evidenceRefs": [],
        "summary": ""
    })
}

fn frontend_quality_self_check_template(task: &TaskDefinition) -> Value {
    let ui_quality_contract = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiQualityContract"))
        .unwrap_or(&Value::Null);
    let reference_ids = ui_quality_contract
        .pointer("/referenceProfile/referenceIds")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let states_covered = ui_quality_contract
        .get("requiredUiStates")
        .and_then(Value::as_array)
        .map(|states| {
            states
                .iter()
                .filter_map(|state| {
                    state
                        .get("state")
                        .and_then(Value::as_str)
                        .map(|state_name| {
                            json!({
                                "state": state_name,
                                "status": "covered",
                                "evidence": ""
                            })
                        })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let business_rules_checked = ui_quality_contract
        .get("businessUiRules")
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| {
                    rule.get("ruleId").and_then(Value::as_str).map(|rule_id| {
                        json!({
                            "ruleId": rule_id,
                            "status": "satisfied",
                            "evidence": ""
                        })
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let design_token_plan = ui_quality_contract
        .get("designTokenAssetPlan")
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
    json!({
        "status": "satisfied",
        "scenarioKind": ui_quality_contract.pointer("/scenario/kind").and_then(Value::as_str).unwrap_or("custom_product_ui"),
        "qualityLevel": ui_quality_contract.get("qualityLevel").and_then(Value::as_str).unwrap_or("production_internal_product"),
        "referenceIdsChecked": reference_ids,
        "statesCovered": states_covered,
        "businessUiRulesChecked": business_rules_checked,
        "forbiddenContentCheck": {
            "checked": true,
            "violations": []
        },
        "designTokenEvidence": {
            "strategyUsed": token_strategy,
            "templateIdUsed": template_id,
            "tokenAssetFiles": [],
            "tokenConsumerFiles": [],
            "existingTokenSystemReused": matches!(token_strategy, "reuse_existing" | "extend_existing"),
            "parallelTokenSystemCreated": false,
            "mergeSummary": ""
        },
        "knownGaps": [],
        "summary": ""
    })
}
