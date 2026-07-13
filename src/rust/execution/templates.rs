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
            }],
            "architectureQualityRequirementRefs": [],
            "apiContractRequirementRefs": [],
            "codeQualityRequirementRefs": []
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
    task_plan_id: &str,
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
                "verificationId": intent.verification_id,
                "status": if browser_checks.is_empty() { "passed" } else { "not_run" },
                "evidenceType": if browser_checks.is_empty() { "automated_test" } else { "browser_automation" },
                "summary": ""
            });
            if !browser_checks.is_empty() {
                result["browserChecks"] = Value::Array(
                    browser_checks
                        .into_iter()
                        .map(|check| {
                            json!({
                                "checkId": check.check_id,
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
    properties.insert("schemaVersion".to_string(), json!("1.0"));
    properties.insert("taskResultId".to_string(), json!("string"));
    properties.insert("taskId".to_string(), json!("string"));
    properties.insert("taskPlanId".to_string(), json!("string"));
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
        "verificationId": "task.verificationIntents[].verificationId",
        "status": "passed | not_run | failed | inconclusive",
        "evidenceType": "one of the matching verification intent acceptableEvidence values",
        "summary": "string"
    });
    if browser_profile.is_some() {
        verification_shape["browserChecks"] = json!([{
            "checkId": "sourceContext.browserVerificationContext.profile.checks[].checkId for this verificationId",
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
            "detailId": "task.requirementDetailRefs or task.verificationIntents[].requirementDetailRefs item",
            "status": "satisfied | partial | not_verified",
            "verificationIds": ["task.verificationIntents[].verificationId"],
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
    properties.insert("createdAt".to_string(), json!("ISO-8601 datetime"));
    properties.insert("updatedAt".to_string(), json!("ISO-8601 datetime"));

    if frontend_self_check_applies(task) {
        properties.insert(
            "frontendExperienceSelfCheck".to_string(),
            json!({
                "status": "satisfied | partial | blocked",
                "closureRequirementIds": ["task.frontendExperienceRequirement.executionGuidance.closureRequirementRefs[].closureId"],
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
                "surfaceDecisionContractRef": "task.frontendExperienceRequirement.uiSurfaceDecisionContractRef",
                "surfaceRegionEvidence": [{
                    "id": "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.regionsInScope[].regionId",
                    "status": "satisfied | partial | missing | blocked_by_environment",
                    "files": ["project-relative UI file"],
                    "states": ["covered state id"],
                    "actions": ["covered action id"],
                    "evidence": "string"
                }],
                "surfaceActionEvidence": [{
                    "id": "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.actionsInScope[].actionId",
                    "status": "satisfied | partial | missing | blocked_by_environment",
                    "files": ["project-relative UI file"],
                    "states": ["covered state id"],
                    "actions": ["covered action id"],
                    "evidence": "string"
                }],
                "surfaceStateEvidence": [{
                    "id": "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.statesInScope[].state",
                    "status": "satisfied | partial | missing | blocked_by_environment",
                    "files": ["project-relative UI file"],
                    "states": ["covered state id"],
                    "actions": ["covered action id"],
                    "evidence": "string"
                }],
                "surfaceQualityRuleEvidence": [{
                    "id": "task.frontendExperienceRequirement.executionGuidance.uiProductionBrief.surfaceDecisionContract.qualityRulesInScope[].ruleId",
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
                "requirementRef": "task.runtimeDeliveryRequirement.runtimeDeliveryRef",
                "checkedFields": ["task.runtimeDeliveryRequirement.affectedContractFields item"],
                "codeLevelChecks": [{
                    "checkId": "task.runtimeDeliveryRequirement.requiredCodeLevelChecks[].checkId",
                    "contractField": "string or null",
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
                "conceptRef": "task.conceptRefs item",
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
                "requirementId": "task.architectureQualityRequirementRefs item",
                "status": "satisfied | partial | not_verified",
                "verificationIds": ["task.verificationIntents[].verificationId"],
                "changedFiles": ["project-relative path"],
                "summary": "string"
            }]),
        );
    }
    if api_contract_evidence_applies(task) {
        properties.insert(
            "apiContractEvidence".to_string(),
            json!([{
                "requirementId": "task.apiContractRequirementRefs item",
                "status": "satisfied | partial | not_verified",
                "interfaceRefs": ["task.writeBoundary.artifactRefs.interfaces item"],
                "verificationIds": ["task.verificationIntents[].verificationId"],
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
                "requirementId": "task.codeQualityRequirementRefs item",
                "status": "satisfied | partial | not_verified",
                "referenceGroupsChecked": {"language_or_framework": ["group"]},
                "referenceFilesChecked": ["reference path"],
                "verificationIds": ["task.verificationIntents[].verificationId"],
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

fn architecture_quality_evidence_template(task: &TaskDefinition) -> Value {
    Value::Array(
        task.architecture_quality_requirement_refs
            .iter()
            .map(|requirement_id| {
                json!({
                    "requirementId": requirement_id,
                    "status": "satisfied",
                    "verificationIds": template_verification_ids_for_architecture_quality(task),
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
            .map(|requirement_id| {
                json!({
                    "requirementId": requirement_id,
                    "status": "satisfied",
                    "interfaceRefs": task.write_boundary.artifact_refs.interfaces.clone(),
                    "verificationIds": template_verification_ids_for_architecture_quality(task),
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
                    "requirementId": requirement_id,
                    "status": "satisfied",
                    "referenceGroupsChecked": reference_groups,
                    "referenceFilesChecked": reference_files,
                    "verificationIds": template_verification_ids_for_architecture_quality(task),
                    "changedFiles": [],
                    "commandsRun": [],
                    "knownGaps": [],
                    "summary": "Explain how the changed files followed the selected code quality references and existing repository style."
                })
            })
            .collect(),
    )
}

fn template_verification_ids_for_architecture_quality(task: &TaskDefinition) -> Vec<String> {
    if task.verification_intents.len() == 1 {
        return vec![task.verification_intents[0].verification_id.clone()];
    }
    task.verification_intents
        .iter()
        .map(|intent| intent.verification_id.clone())
        .collect()
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
    let frontend_execution_guidance = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("executionGuidance"));
    let surface_decision_ref = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiSurfaceDecisionContractRef"))
        .and_then(Value::as_str)
        .map(str::to_string);
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
                    region.get("regionId").and_then(Value::as_str).map(|id| {
                        json!({
                            "id": id,
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
                            "id": id,
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
                            "id": id,
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
                    rule.get("ruleId").and_then(Value::as_str).map(|id| {
                        json!({
                            "id": id,
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
        "surfaceDecisionContractRef": surface_decision_ref,
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

        let template =
            task_result_template_with_code_quality("taskplan", &task, &[], Some(&profile));

        assert_eq!(
            template["verificationResults"][0]["browserChecks"][0]["checkId"],
            json!("browser-ui-desktop")
        );
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
