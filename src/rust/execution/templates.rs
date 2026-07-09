use contracts::{CodeQualityRequirement, TaskDefinition, TaskPlan};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub(crate) const FRONTEND_QUALITY_CONTRACT_READ_FIELDS: [&str; 23] = [
    "task.frontendExperienceRequirement.uiQualityContract.scenario",
    "task.frontendExperienceRequirement.uiQualityContract.qualityLevel",
    "task.frontendExperienceRequirement.uiQualityContract.surfacePolicy",
    "task.frontendExperienceRequirement.uiQualityContract.layoutBaseline",
    "task.frontendExperienceRequirement.uiQualityContract.density",
    "task.frontendExperienceRequirement.uiQualityContract.semanticTokenPolicy",
    "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.loadMode",
    "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.groups",
    "task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan",
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
    "task.frontendExperienceRequirement.uiQualityContract.qualityGates",
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
) -> Value {
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

pub(crate) fn task_result_schema_shape(task: &TaskDefinition) -> Value {
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
    properties.insert(
        "verificationResults".to_string(),
        json!([{
            "verificationId": "task.verificationIntents[].verificationId",
            "status": "passed | not_run | failed | inconclusive",
            "evidenceType": "one of the matching verification intent acceptableEvidence values",
            "summary": "string"
        }]),
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
                "scenarioKind": "task.frontendExperienceRequirement.uiQualityContract.scenario.kind",
                "qualityLevel": "task.frontendExperienceRequirement.uiQualityContract.qualityLevel",
                "referenceGroupsChecked": {"group": ["item"]},
                "referenceFilesChecked": ["task.frontendExperienceRequirement.uiQualityContract.referenceProfile.referenceLoadPlan[].path"],
                "statesCovered": [{
                    "state": "task.frontendExperienceRequirement.uiQualityContract.requiredUiStates[].state",
                    "status": "covered | partial | missing",
                    "evidence": "string"
                }],
                "businessUiRulesChecked": [{
                    "ruleId": "task.frontendExperienceRequirement.uiQualityContract.businessUiRules[].ruleId",
                    "status": "satisfied | partial | missing",
                    "evidence": "string"
                }],
                "forbiddenContentCheck": {
                    "checked": true,
                    "violations": ["string"]
                },
                "surfacesCovered": [{
                    "surfaceId": "task.frontendExperienceRequirement.executionGuidance.surfacesInScope[].surfaceId",
                    "surfaceRole": "app_shell | page | record_list | record_detail | form | action_panel | navigation",
                    "files": ["project-relative UI file"],
                    "states": ["covered state id"],
                    "businessActions": ["covered action id"],
                    "evidence": "string"
                }],
                "designTokenEvidence": {
                    "strategyUsed": "reuse_existing | extend_existing | create_css_tokens | create_tailwind_tokens | not_applicable",
                    "templateIdUsed": "tokens-css | tokens-tailwind | null",
                    "tokenAssetFiles": ["project-relative token asset file"],
                    "tokenConsumerFiles": ["project-relative UI file using declared tokens"],
                    "existingTokenSystemReused": true,
                    "parallelTokenSystemCreated": false,
                    "mergeSummary": "string"
                },
                "gateResults": [{
                    "gateId": "task.frontendExperienceRequirement.uiTaskQualityGates[].gateId",
                    "status": "satisfied | partial | missing | blocked_by_environment | not_applicable",
                    "files": ["project-relative file"],
                    "surfaceIds": ["surface id"],
                    "viewportsChecked": ["desktop 1280x800 | mobile 390x844"],
                    "sourceChecks": ["project-relative file"],
                    "attemptedChecks": ["command or probe"],
                    "fallbackEvidence": ["string"],
                    "blockedReason": "string or null",
                    "evidence": "string"
                }],
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
    let ui_quality_contract = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiQualityContract"))
        .unwrap_or(&Value::Null);
    let frontend_execution_guidance = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("executionGuidance"));
    let reference_groups = ui_quality_contract
        .pointer("/referenceProfile/groups")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let reference_files = ui_quality_contract
        .pointer("/referenceProfile/referenceLoadPlan")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("path").and_then(Value::as_str).map(str::to_string))
                .collect::<Vec<_>>()
        })
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
    let default_surface_states = frontend_execution_guidance
        .and_then(|guidance| guidance.pointer("/uiProductionBrief/stateContract/statesInScope"))
        .and_then(Value::as_array)
        .map(|states| {
            states
                .iter()
                .filter_map(|state| {
                    state.as_str().map(|value| json!(value)).or_else(|| {
                        state
                            .get("state")
                            .and_then(Value::as_str)
                            .map(|value| json!(value))
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let default_surface_actions = frontend_execution_guidance
        .and_then(|guidance| guidance.get("actionsInScope"))
        .and_then(Value::as_array)
        .map(|actions| {
            actions
                .iter()
                .filter_map(|action| {
                    action
                        .get("actionId")
                        .and_then(Value::as_str)
                        .or_else(|| action.get("label").and_then(Value::as_str))
                        .map(|value| json!(value))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let surfaces_covered = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.pointer("/executionGuidance/surfacesInScope"))
        .and_then(Value::as_array)
        .map(|surfaces| {
            surfaces
                .iter()
                .filter_map(|surface| {
                    let surface_id = surface.get("surfaceId").and_then(Value::as_str)?;
                    let states = surface
                        .get("stateRefs")
                        .and_then(Value::as_array)
                        .cloned()
                        .filter(|items| !items.is_empty())
                        .unwrap_or_else(|| default_surface_states.clone());
                    let business_actions = surface
                        .get("actionRefs")
                        .and_then(Value::as_array)
                        .cloned()
                        .filter(|items| !items.is_empty())
                        .unwrap_or_else(|| default_surface_actions.clone());
                    Some(json!({
                        "surfaceId": surface_id,
                        "surfaceRole": surface
                            .get("surfaceRole")
                            .or_else(|| surface.get("role"))
                            .and_then(Value::as_str)
                            .unwrap_or("page"),
                        "files": ["replace_with_ui_file_path_for_this_surface"],
                        "states": states,
                        "businessActions": business_actions,
                        "evidence": "Describe how this surface implements the business purpose, layout composition, states, and actions."
                    }))
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
    let token_asset_files = design_token_plan
        .get("targetFiles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let gate_results = frontend_quality_gate_result_template(task, ui_quality_contract);
    json!({
        "status": "partial",
        "scenarioKind": ui_quality_contract.pointer("/scenario/kind").and_then(Value::as_str).unwrap_or("custom_product_ui"),
        "qualityLevel": ui_quality_contract.get("qualityLevel").and_then(Value::as_str).unwrap_or("production_internal_product"),
        "referenceGroupsChecked": reference_groups,
        "referenceFilesChecked": reference_files,
        "statesCovered": states_covered,
        "businessUiRulesChecked": business_rules_checked,
        "forbiddenContentCheck": {
            "checked": true,
            "violations": []
        },
        "surfacesCovered": surfaces_covered,
        "designTokenEvidence": {
            "strategyUsed": token_strategy,
            "templateIdUsed": template_id,
            "tokenAssetFiles": token_asset_files,
            "tokenConsumerFiles": ["replace_with_ui_file_using_declared_tokens"],
            "existingTokenSystemReused": matches!(token_strategy, "reuse_existing" | "extend_existing"),
            "parallelTokenSystemCreated": false,
            "mergeSummary": ""
        },
        "gateResults": gate_results,
        "knownGaps": [],
        "summary": ""
    })
}

fn frontend_quality_gate_result_template(
    task: &TaskDefinition,
    ui_quality_contract: &Value,
) -> Value {
    let gates = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiTaskQualityGates"))
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| {
            ui_quality_contract
                .get("qualityGates")
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default();
    Value::Array(
        gates
            .into_iter()
            .filter_map(|gate| {
                let gate_id = gate.get("gateId").and_then(Value::as_str)?;
                Some(json!({
                    "gateId": gate_id,
                    "status": "missing",
                    "files": ["replace_with_changed_ui_file_or_source_checked_file"],
                    "surfaceIds": gate
                        .get("surfaceIds")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                    "viewportsChecked": [],
                    "sourceChecks": [],
                    "attemptedChecks": [],
                    "fallbackEvidence": [],
                    "blockedReason": Value::Null,
                    "evidence": "Replace with concrete evidence. Use status=blocked_by_environment only for render checks blocked by missing local preview/browser/dependencies, and include blockedReason plus fallbackEvidence."
                }))
            })
            .collect(),
    )
}
