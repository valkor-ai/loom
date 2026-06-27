use std::path::Path;

use contracts::{
    DeployExecutionRepairTaskResult, DeploymentErrorWindow, DeploymentFailureKind,
    DeploymentFailureOwner, DeploymentFailureReport, DeploymentRepairAction, DeploymentRepairRoute,
    DeploymentSpec,
};
use delivery_core::{
    ArtifactKind, DeployRepairAssetsNext, ExecuteEditBoundary, ExecuteTaskNext,
    ExecuteVerificationPolicy, ExecutionKind, FileSubmitInput, LoomMcpActionResult,
    LoomMcpAutoRunnableResult, LoomMcpBlockedResult, LoomMcpDoneResult, LoomMcpFailure,
    LoomMcpFailureResult, LoomMcpNextAction, LoomMcpRepairableErrorResult, PostSubmitAction,
    ReadRequestFieldsResult, RepairContext, RepairIssue, RepairOrigin, WriteMode,
};
use schemars::schema_for;
use serde_json::{json, Value};
use state::{
    paths::{from_project_relative, to_project_relative},
    store::{
        ensure_dir, now_millis, now_string, path_exists, read_json, read_json_value,
        write_json_atomic, StateError, StateResult,
    },
    write_targets::AuthorizedWriteSet,
};

use crate::{
    paths::{
        deploy_execution_repair_action_file, deploy_execution_repair_result_file, deployment_paths,
    },
    prepare::read_spec,
    run::deploy_retry_after_repair,
    DeployToolInput,
};

pub fn deploy_repair(input: DeployToolInput) -> LoomMcpActionResult {
    let project_root = Path::new(&input.project_root);
    match latest_repair_action(project_root) {
        Ok(Some(request)) => repair_next(project_root, &request),
        Ok(None) => LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: input.project_root,
            summary: "No deployment repair action is pending.".to_string(),
            details: None,
            warnings: vec![],
        }),
        Err(error) => failed(
            &input.project_root,
            "DEPLOY_REPAIR_FAILED",
            error.to_string(),
        ),
    }
}

pub fn write_repair_action(
    project_root: &Path,
    spec: &DeploymentSpec,
    failure_kind: DeploymentFailureKind,
    command: Vec<String>,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> StateResult<LoomMcpActionResult> {
    let paths = deployment_paths(project_root);
    ensure_dir(&paths.state_dir)?;
    ensure_dir(&paths.logs_dir)?;
    let mut log = String::new();
    if path_exists(&paths.log_file) {
        log = state::store::read_text(&paths.log_file).unwrap_or_default();
    }
    log.push_str(stdout);
    if !stdout.ends_with('\n') {
        log.push('\n');
    }
    log.push_str(stderr);
    if !stderr.ends_with('\n') {
        log.push('\n');
    }
    state::store::write_text_atomic(&paths.log_file, &log)?;
    let editable_files = editable_files_for(spec, failure_kind);
    let (failure_owner, repair_route) = classify_repair(failure_kind, editable_files.len());
    let failure_ref = if repair_route == DeploymentRepairRoute::ExecutionRepair {
        let report = create_failure_report(
            project_root,
            spec,
            failure_kind,
            failure_owner,
            repair_route,
            command.clone(),
            exit_code,
            stdout,
            stderr,
        )?;
        write_json_atomic(&paths.failure_file, &report)?;
        Some(to_project_relative(project_root, &paths.failure_file)?)
    } else {
        None
    };
    let request = DeploymentRepairAction {
        schema_version: 1,
        repair_id: format!("deploy_repair_{}", now_millis()),
        created_at: now_string(),
        project_root: project_root.to_string_lossy().into_owned(),
        spec_ref: to_project_relative(project_root, &paths.spec_file)?,
        failure_kind,
        failure_owner,
        repair_route,
        failure_ref,
        command,
        exit_code,
        full_log_ref: Some(to_project_relative(project_root, &paths.log_file)?),
        error_window: Some(error_window(stdout, stderr)),
        diagnostics: vec![],
        suggested_actions: suggested_actions(failure_kind, failure_owner),
        editable_files,
        protected_files: protected_files_for(spec),
        instruction: instruction_for(failure_kind, failure_owner),
        max_attempts: 2,
        attempts: 0,
        status: "pending".to_string(),
    };
    write_json_atomic(&paths.repair_action_file, &request)?;
    Ok(repair_next(project_root, &request))
}

pub fn repair_next(project_root: &Path, request: &DeploymentRepairAction) -> LoomMcpActionResult {
    match request.repair_route {
        DeploymentRepairRoute::DeployRepair => {
            let spec = read_spec(project_root).ok();
            LoomMcpActionResult::AutoRunnable(LoomMcpAutoRunnableResult::new(
                project_root.to_string_lossy().into_owned(),
                LoomMcpNextAction::DeployRepairAssets(DeployRepairAssetsNext {
                    repair_id: request.repair_id.clone(),
                    failure_kind: enum_string(&request.failure_kind),
                    failure_owner: enum_string(&request.failure_owner),
                    repair_route: enum_string(&request.repair_route),
                    editable_files: request.editable_files.clone(),
                    protected_files: request.protected_files.clone(),
                    source_model_ref: spec.as_ref().map(|spec| spec.source_model_ref.clone()),
                    topology_ref: spec.as_ref().map(|spec| spec.topology_ref.clone()),
                    generated_file_refs: spec
                        .as_ref()
                        .map(|spec| {
                            let mut refs = vec![
                                spec.files.compose_path.clone(),
                                spec.files.dockerignore_path.clone(),
                            ];
                            refs.extend(spec.files.dockerfile_paths.values().cloned());
                            refs.extend(spec.files.nginx_config_paths.values().cloned());
                            refs
                        })
                        .unwrap_or_default(),
                    diagnostics_ref: Some(
                        to_project_relative(
                            project_root,
                            &deployment_paths(project_root).repair_action_file,
                        )
                        .unwrap_or_else(|_| {
                            ".loom/deployment/state/repair-action.json".to_string()
                        }),
                    ),
                    error_window: request.error_window.as_ref().map(|window| {
                        delivery_core::DeploymentErrorWindow {
                            started_at: None,
                            ended_at: None,
                            lines: window.lines.clone(),
                            truncated: window.truncated,
                            total_line_count: window.total_line_count,
                            matched_patterns: window.matched_patterns.clone(),
                        }
                    }),
                    retry_tool: "loom.deployUp".to_string(),
                }),
            ))
        }
        DeploymentRepairRoute::ExecutionRepair => {
            materialize_deploy_execution_repair(project_root, request).unwrap_or_else(|error| {
                failed(
                    &project_root.to_string_lossy(),
                    "DEPLOY_EXECUTION_REPAIR_FAILED",
                    error.to_string(),
                )
            })
        }
        DeploymentRepairRoute::ManualReview => {
            LoomMcpActionResult::UserGate(delivery_core::LoomMcpUserGateResult {
                project_root: project_root.to_string_lossy().into_owned(),
                prompt: "Deployment failure needs user review before Loom can safely repair it."
                    .to_string(),
                accepted_responses: vec!["confirm".to_string()],
                request_ref: None,
                delivery_id: None,
                phase_id: None,
                gate: Some(json!(request)),
            })
        }
        DeploymentRepairRoute::None => LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: project_root.to_string_lossy().into_owned(),
            blockers: request.suggested_actions.clone(),
            recommended_tool: Some("loom.deployStatus".to_string()),
            details: Some(json!(request)),
        }),
    }
}

pub fn accept_deploy_execution_repair_file(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
) -> LoomMcpActionResult {
    match accept_deploy_execution_repair_file_inner(input, authorized) {
        Ok(result) => result,
        Err(error) => failed(
            &input.project_root,
            "DEPLOY_EXECUTION_REPAIR_SUBMIT_FAILED",
            error.to_string(),
        ),
    }
}

fn accept_deploy_execution_repair_file_inner(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
) -> StateResult<LoomMcpActionResult> {
    let target = authorized.targets.first().ok_or_else(|| {
        StateError::InvalidArgument("Deploy execution repair result target is missing.".to_string())
    })?;
    let project_root = Path::new(&input.project_root);
    let target_file = target.path.clone();
    let raw_result = match read_json_value(&from_project_relative(project_root, &target_file)?) {
        Ok(value) => value,
        Err(error) => {
            return Ok(repairable(
                input,
                authorized,
                target_file,
                vec![repair_issue(
                    "DEPLOY_REPAIR_RESULT_JSON_INVALID",
                    "$",
                    &format!("Deploy execution repair result JSON is not readable: {error}"),
                )],
            ))
        }
    };
    let result: DeployExecutionRepairTaskResult = match serde_json::from_value(raw_result) {
        Ok(result) => result,
        Err(error) => {
            return Ok(repairable(
                input,
                authorized,
                target_file,
                vec![repair_issue(
                    "DEPLOY_REPAIR_RESULT_SCHEMA_INVALID",
                    "$",
                    &format!("Deploy execution repair result has an invalid schema: {error}"),
                )],
            ))
        }
    };
    let request_fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: input.project_root.clone(),
        request_ref: input.request_ref.clone(),
        fields: vec![
            "repairContext.failedContractFields".to_string(),
            "repairContext.requiredCodeLevelChecks".to_string(),
            "editBoundary.protectedPaths".to_string(),
            "outputContract.repairId".to_string(),
            "outputContract.deploymentFailureRef".to_string(),
        ],
    })
    .map_err(|error| StateError::InvalidArgument(error.to_string()))?;
    let repair_id = request_fields
        .fields
        .get("outputContract.repairId")
        .and_then(|field| field.value.as_str())
        .ok_or_else(|| {
            StateError::StateCorrupted(
                "deploy repair action missing outputContract.repairId.".to_string(),
            )
        })?;
    let deployment_failure_ref = request_fields
        .fields
        .get("outputContract.deploymentFailureRef")
        .and_then(|field| field.value.as_str())
        .ok_or_else(|| {
            StateError::StateCorrupted(
                "deploy repair action missing deploymentFailureRef.".to_string(),
            )
        })?;
    if result.repair_id != repair_id {
        return Ok(repairable(
            input,
            authorized,
            target_file,
            vec![repair_issue(
                "DEPLOY_REPAIR_ID_MISMATCH",
                "repairId",
                "repairId must match outputContract.repairId from the active deploy repair request.",
            )],
        ));
    }
    if result.deployment_failure_ref != deployment_failure_ref {
        return Ok(repairable(
            input,
            authorized,
            target_file,
            vec![repair_issue(
                "DEPLOY_REPAIR_FAILURE_REF_MISMATCH",
                "deploymentFailureRef",
                "deploymentFailureRef must match outputContract.deploymentFailureRef from the active deploy repair request.",
            )],
        ));
    }
    let protected = request_fields
        .fields
        .get("editBoundary.protectedPaths")
        .and_then(|field| field.value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for changed in &result.changed_files {
        if protected
            .iter()
            .any(|prefix| changed == prefix || changed.starts_with(&format!("{prefix}/")))
        {
            return Ok(repairable(
                input,
                authorized,
                target_file,
                vec![repair_issue(
                    "DEPLOY_REPAIR_PROTECTED_PATH_CHANGED",
                    "changedFiles",
                    &format!("Deploy execution repair must not change protected path {changed}."),
                )],
            ));
        }
    }
    if let Err(error) = validate_runtime_delivery_evidence(&result, &request_fields) {
        return Ok(repairable(
            input,
            authorized,
            target_file,
            vec![repair_issue(
                "DEPLOY_REPAIR_RUNTIME_EVIDENCE_INVALID",
                "runtimeDeliveryEvidence",
                &error.to_string(),
            )],
        ));
    }
    if matches!(result.status.as_str(), "blocked" | "failed") {
        return Ok(LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: input.project_root.clone(),
            blockers: result.notes,
            recommended_tool: Some("loom.deployRepair".to_string()),
            details: Some(json!({ "repairId": result.repair_id, "status": result.status })),
        }));
    }
    let persisted = deployment_paths(project_root)
        .repairs_dir
        .join(&authorized.request_id)
        .join("result.json");
    write_json_atomic(&persisted, &result)?;
    Ok(deploy_retry_after_repair(project_root))
}

fn materialize_deploy_execution_repair(
    project_root: &Path,
    request: &DeploymentRepairAction,
) -> StateResult<LoomMcpActionResult> {
    let failure_ref = request.failure_ref.clone().ok_or_else(|| {
        StateError::StateCorrupted("execution repair is missing failureRef.".to_string())
    })?;
    let failure: DeploymentFailureReport =
        read_json(&from_project_relative(project_root, &failure_ref)?)?;
    let request_id = format!("deploy_exec_repair_{}", now_millis());
    let result_file = to_project_relative(
        project_root,
        &deploy_execution_repair_result_file(project_root, &request_id),
    )?;
    let request_file = to_project_relative(
        project_root,
        &deploy_execution_repair_action_file(project_root, &request_id),
    )?;
    let protected_paths = failure.must_not_edit.clone();
    let schema_shape = serde_json::to_value(schema_for!(DeployExecutionRepairTaskResult))
        .unwrap_or_else(|_| json!({ "type": "object" }));
    let request_root = json!({
        "schemaVersion": "1.0",
        "requestType": "deploy_execution_repair",
        "requestId": request_id,
        "artifactKind": ArtifactKind::DeployExecutionRepairResult,
        "executionKind": "deploy_execution_repair",
        "repairContext": {
            "repairOrigin": "deploy_failure",
            "deploymentFailureRef": failure_ref,
            "failureKind": failure.failure_kind,
            "failureOwner": failure.failure_owner,
            "failedContractFields": failure.failed_contract_fields,
            "requiredCodeLevelChecks": failure.required_code_level_checks,
            "errorWindow": failure.error_window
        },
            "editBoundary": {
                "allowedPaths": ["."],
                "protectedPaths": protected_paths
            },
        "executionRules": {
            "scope": "Repair application source code, package scripts, or runtime wiring required by the deploy failure report.",
            "mustNotEditGeneratedAssets": true,
            "mustNotClaimDeploymentSuccess": true,
            "completionBarrier": {
                "resultFile": result_file,
                "submitTool": "loom.repairSubmitFile",
                "postSubmit": "retry_deploy"
            }
        },
        "outputContract": {
            "artifactKind": ArtifactKind::DeployExecutionRepairResult,
            "writeMode": WriteMode::RepairJson,
            "submitTool": "loom.repairSubmitFile",
            "repairId": request.repair_id,
            "deploymentFailureRef": failure_ref,
            "resultFile": result_file,
            "writeTargets": [{
                "targetId": "result",
                "path": result_file,
                "required": true,
                "description": "Write the deploy execution repair result JSON."
            }],
            "schemaShape": schema_shape,
            "resultTemplate": deploy_execution_repair_result_template(request, &failure_ref, &failure),
            "resultRules": [
                "changedFiles must not include generated Dockerfile, Compose, nginx, dockerignore, RuntimeDeliveryContract, AAC, TaskPlan, ReviewResult, or .loom.",
                "runtimeDeliveryEvidence.addressedFailedContractFields must cover failedContractFields.",
                "codeLevelChecks must cover requiredCodeLevelChecks."
            ]
        },
        "requestReadPlan": {
            "groups": [
                {
                    "groupId": "deploy_failure_context",
                    "required": true,
                    "purpose": "Read deploy failure report and edit boundary before editing application code.",
                    "whenToRead": "Read before source edits.",
                    "fields": [
                        "repairContext.repairOrigin",
                        "repairContext.deploymentFailureRef",
                        "repairContext.failureKind",
                        "repairContext.failureOwner",
                        "repairContext.failedContractFields",
                        "repairContext.requiredCodeLevelChecks",
                        "repairContext.errorWindow",
                        "editBoundary.allowedPaths",
                        "editBoundary.protectedPaths",
                        "executionRules.scope",
                        "executionRules.mustNotEditGeneratedAssets",
                        "executionRules.mustNotClaimDeploymentSuccess"
                    ]
                },
                {
                    "groupId": "deploy_repair_result_contract",
                    "required": true,
                    "purpose": "Read result path, schema shape, and submit rules before writing repair result.",
                    "whenToRead": "Read before writing the result file.",
                    "fields": [
                        "outputContract.repairId",
                        "outputContract.deploymentFailureRef",
                        "outputContract.resultFile",
                        "outputContract.resultTemplate",
                        "outputContract.schemaShape.properties.status",
                        "outputContract.schemaShape.properties.changedFiles",
                        "outputContract.schemaShape.properties.runtimeDeliveryEvidence",
                        "outputContract.schemaShape.properties.selfRepairSummary",
                        "outputContract.schemaShape.properties.notes",
                        "outputContract.resultRules",
                        "executionRules.completionBarrier"
                    ]
                }
            ]
        }
    });
    let stored = state::write_native_request(
        &project_root.to_string_lossy(),
        state::NativeRequestInput {
            request_id: request_id.clone(),
            request_kind: "deploy_execution_repair".to_string(),
            request_file: Some(request_file),
            delivery_id: None,
            phase_id: None,
            root: request_root,
        },
    )?;
    if let Some(parent) = from_project_relative(project_root, &result_file)?.parent() {
        ensure_dir(parent)?;
    }
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string_lossy().into_owned(),
        request_ref: stored.request_ref.clone(),
    })
    .map_err(|error| StateError::InvalidArgument(error.to_string()))?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string_lossy().into_owned(),
            LoomMcpNextAction::ExecuteTask(ExecuteTaskNext {
                execution_kind: ExecutionKind::DeployExecutionRepair,
                repair_origin: Some(RepairOrigin::DeployFailure),
                request_ref: stored.request_ref,
                result_file,
                task_id: "deploy-execution-repair".to_string(),
                group_id: None,
                read_groups: inspected.read_groups,
                submit_tool: "loom.repairSubmitFile".to_string(),
                edit_boundary: ExecuteEditBoundary {
                    allowed_paths: vec![".".to_string()],
                    protected_paths: failure.must_not_edit.clone(),
                },
                verification_policy: ExecuteVerificationPolicy {
                    required_commands: vec![],
                    evidence_required: true,
                },
                repair_context: Some(RepairContext {
                    repair_origin: RepairOrigin::DeployFailure,
                    source_task_id: "deploy-execution-repair".to_string(),
                    issues: vec![enum_string(&failure.failure_kind)],
                    review_result_ref: None,
                    finding_refs: vec![],
                    manual_review_resolution_ref: None,
                    user_change_summary: None,
                    failed_task_result_ref: None,
                    attempt_count: Some(failure.attempt),
                    deployment_failure_ref: Some(failure_ref),
                    failed_contract_fields: failure.failed_contract_fields,
                    required_code_level_checks: failure.required_code_level_checks,
                }),
                post_submit: PostSubmitAction::RetryDeploy,
            }),
        ),
    ))
}

fn deploy_execution_repair_result_template(
    request: &DeploymentRepairAction,
    failure_ref: &str,
    failure: &DeploymentFailureReport,
) -> Value {
    json!({
        "schemaVersion": "1.0",
        "repairId": request.repair_id,
        "status": "completed",
        "deploymentFailureRef": failure_ref,
        "changedFiles": [],
        "runtimeDeliveryEvidence": {
            "addressedFailedContractFields": failure.failed_contract_fields,
            "codeLevelChecks": failure.required_code_level_checks.iter().map(|check| {
                json!({
                    "checkId": check,
                    "status": "passed",
                    "evidence": ""
                })
            }).collect::<Vec<_>>(),
            "commandsRun": [],
            "unverifiedItems": []
        },
        "selfRepairSummary": {
            "attempted": true,
            "attemptCount": failure.attempt,
            "stopReason": "verification_passed",
            "progressObserved": true
        },
        "notes": []
    })
}

fn latest_repair_action(project_root: &Path) -> StateResult<Option<DeploymentRepairAction>> {
    let path = deployment_paths(project_root).repair_action_file;
    if !path_exists(&path) {
        return Ok(None);
    }
    read_json(&path).map(Some)
}

fn validate_runtime_delivery_evidence(
    result: &DeployExecutionRepairTaskResult,
    request_fields: &ReadRequestFieldsResult,
) -> StateResult<()> {
    let expected_fields = request_fields
        .fields
        .get("repairContext.failedContractFields")
        .and_then(|field| field.value.as_array())
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let addressed_fields = result
        .runtime_delivery_evidence
        .get("addressedFailedContractFields")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    for expected in expected_fields {
        if !addressed_fields.contains(&expected) {
            return Err(StateError::InvalidArgument(format!(
                "runtimeDeliveryEvidence.addressedFailedContractFields is missing {expected}."
            )));
        }
    }
    let expected_checks = request_fields
        .fields
        .get("repairContext.requiredCodeLevelChecks")
        .and_then(|field| field.value.as_array())
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let actual_checks = result
        .runtime_delivery_evidence
        .get("codeLevelChecks")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("checkId").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for expected in expected_checks {
        if !actual_checks.contains(&expected) {
            return Err(StateError::InvalidArgument(format!(
                "runtimeDeliveryEvidence.codeLevelChecks is missing {expected}."
            )));
        }
    }
    if result.self_repair_summary.is_null() {
        return Err(StateError::InvalidArgument(
            "selfRepairSummary is required.".to_string(),
        ));
    }
    Ok(())
}

fn classify_repair(
    failure_kind: DeploymentFailureKind,
    editable_file_count: usize,
) -> (DeploymentFailureOwner, DeploymentRepairRoute) {
    match failure_kind {
        DeploymentFailureKind::DockerUnavailable => (
            DeploymentFailureOwner::Environment,
            DeploymentRepairRoute::None,
        ),
        DeploymentFailureKind::RegistryNetwork => (
            DeploymentFailureOwner::ExternalSystem,
            DeploymentRepairRoute::None,
        ),
        DeploymentFailureKind::BuildCommandFailed
        | DeploymentFailureKind::StartCommandFailed
        | DeploymentFailureKind::ApplicationStartupFailed
        | DeploymentFailureKind::HttpProbeFailed
        | DeploymentFailureKind::PreviewNotVerified => (
            DeploymentFailureOwner::ApplicationCode,
            DeploymentRepairRoute::ExecutionRepair,
        ),
        DeploymentFailureKind::Healthcheck if editable_file_count == 0 => (
            DeploymentFailureOwner::ApplicationCode,
            DeploymentRepairRoute::ManualReview,
        ),
        _ if editable_file_count > 0 => (
            DeploymentFailureOwner::DeploymentAssets,
            DeploymentRepairRoute::DeployRepair,
        ),
        _ => (
            DeploymentFailureOwner::Unknown,
            DeploymentRepairRoute::ManualReview,
        ),
    }
}

fn editable_files_for(spec: &DeploymentSpec, failure_kind: DeploymentFailureKind) -> Vec<String> {
    match failure_kind {
        DeploymentFailureKind::DockerUnavailable
        | DeploymentFailureKind::RegistryNetwork
        | DeploymentFailureKind::RuntimeContractMissing
        | DeploymentFailureKind::RuntimeContractNotApplicable
        | DeploymentFailureKind::RuntimeContractMismatch
        | DeploymentFailureKind::BuildCommandFailed
        | DeploymentFailureKind::StartCommandFailed
        | DeploymentFailureKind::ApplicationStartupFailed
        | DeploymentFailureKind::HttpProbeFailed
        | DeploymentFailureKind::PreviewNotVerified => vec![],
        _ => {
            let mut files = vec![
                spec.files.compose_path.clone(),
                spec.files.dockerignore_path.clone(),
            ];
            files.extend(spec.files.dockerfile_paths.values().cloned());
            files.extend(spec.files.nginx_config_paths.values().cloned());
            files.sort();
            files.dedup();
            files
        }
    }
}

fn protected_files_for(spec: &DeploymentSpec) -> Vec<String> {
    spec.files.reused.clone()
}

fn create_failure_report(
    project_root: &Path,
    spec: &DeploymentSpec,
    failure_kind: DeploymentFailureKind,
    failure_owner: DeploymentFailureOwner,
    repair_route: DeploymentRepairRoute,
    command: Vec<String>,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> StateResult<DeploymentFailureReport> {
    let fields = failed_contract_fields(failure_kind);
    let required_checks = fields
        .iter()
        .map(|field| format!("check_{field}").replace('.', "_"))
        .collect::<Vec<_>>();
    let mut must_not_edit = vec![".loom".to_string(), spec.runtime_contract_ref.clone()];
    must_not_edit.extend(spec.files.dockerfile_paths.values().cloned());
    must_not_edit.extend(spec.files.nginx_config_paths.values().cloned());
    must_not_edit.push(spec.files.compose_path.clone());
    must_not_edit.push(spec.files.dockerignore_path.clone());
    must_not_edit.sort();
    must_not_edit.dedup();
    let _ = (command, exit_code);
    Ok(DeploymentFailureReport {
        schema_version: "1.0".to_string(),
        failure_id: format!("deploy_failure_{}", now_millis()),
        source: "deploy".to_string(),
        created_at: now_string(),
        deployment_attempt_id: format!("deploy_attempt_{}", now_millis()),
        failure_kind,
        failure_owner,
        repair_route,
        runtime_delivery_ref: spec.runtime_contract.r#ref.clone(),
        deployment_spec_ref: to_project_relative(
            project_root,
            &deployment_paths(project_root).spec_file,
        )?,
        failed_contract_fields: fields,
        required_code_level_checks: required_checks,
        error_window: error_window(stdout, stderr),
        must_not_edit,
        attempt: 1,
        max_attempts: 2,
    })
}

fn failed_contract_fields(failure_kind: DeploymentFailureKind) -> Vec<String> {
    match failure_kind {
        DeploymentFailureKind::BuildCommandFailed => vec!["build.command".to_string()],
        DeploymentFailureKind::StartCommandFailed => vec!["start.command".to_string()],
        DeploymentFailureKind::ApplicationStartupFailed => vec!["runtime.startup".to_string()],
        DeploymentFailureKind::HttpProbeFailed | DeploymentFailureKind::PreviewNotVerified => {
            vec!["httpProbes.previewPath".to_string()]
        }
        DeploymentFailureKind::ApiRouteNotVerified => {
            vec!["deploymentTopology.apiRoutes".to_string()]
        }
        DeploymentFailureKind::Healthcheck => vec!["httpProbes.healthPath".to_string()],
        _ => vec!["runtime.delivery".to_string()],
    }
}

fn error_window(stdout: &str, stderr: &str) -> DeploymentErrorWindow {
    let lines = [stdout, stderr]
        .join("\n")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let total = lines.len() as u32;
    let selected = lines
        .into_iter()
        .rev()
        .take(80)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    let patterns = selected
        .iter()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error") || lower.contains("failed") || lower.contains("exception")
        })
        .map(|_| "error".to_string())
        .collect::<Vec<_>>();
    DeploymentErrorWindow {
        truncated: total > selected.len() as u32,
        total_line_count: total,
        lines: selected,
        matched_patterns: patterns,
    }
}

fn suggested_actions(
    failure_kind: DeploymentFailureKind,
    owner: DeploymentFailureOwner,
) -> Vec<String> {
    match owner {
        DeploymentFailureOwner::Environment => {
            vec!["Start Docker Desktop or Docker daemon, then rerun loom.deployUp.".to_string()]
        }
        DeploymentFailureOwner::ExternalSystem => {
            vec!["Fix Docker registry or network access, then rerun loom.deployUp.".to_string()]
        }
        DeploymentFailureOwner::ApplicationCode => vec![
            "Repair application code or runtime wiring through deploy execution repair."
                .to_string(),
        ],
        DeploymentFailureOwner::DeploymentAssets => vec![format!(
            "Repair generated deployment assets for {}.",
            enum_string(&failure_kind)
        )],
        DeploymentFailureOwner::Unknown => vec!["Review deployment failure manually.".to_string()],
    }
}

fn instruction_for(failure_kind: DeploymentFailureKind, owner: DeploymentFailureOwner) -> String {
    match owner {
        DeploymentFailureOwner::DeploymentAssets => {
            if failure_kind == DeploymentFailureKind::ApiRouteNotVerified {
                "Repair only generated deployment assets. Preserve public frontend entry and API proxy route; do not delete apiPaths or bypass validation.".to_string()
            } else {
                "Repair only generated deployment assets listed in editableFiles. Do not edit application code or Loom contracts.".to_string()
            }
        }
        DeploymentFailureOwner::ApplicationCode => {
            "Repair application code or runtime wiring through deploy execution repair. Do not edit generated deployment assets.".to_string()
        }
        _ => "Do not edit files for this deployment failure until the blocker is resolved.".to_string(),
    }
}

fn enum_string<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn repair_issue(code: &str, field_path: &str, message: &str) -> RepairIssue {
    RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("result".to_string()),
        field_path: Some(field_path.to_string()),
    }
}

fn repairable(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<RepairIssue>,
) -> LoomMcpActionResult {
    LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
        project_root: input.project_root.clone(),
        target_file,
        target_ids: authorized
            .targets
            .iter()
            .map(|target| target.target_id.clone())
            .collect(),
        issues,
        resubmit_tool: "loom.repairSubmitFile".to_string(),
        fix_scope: Some("deploy_execution_repair_result_only".to_string()),
        read_groups: authorized.read_groups.clone(),
    })
}

fn failed(project_root: &str, code: &str, message: String) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: code.to_string(),
            message,
            target_batch: Some(10),
            domain: Some("deploy".to_string()),
            route_action: None,
            recovery_tool: Some("loom.deployInspect".to_string()),
        },
    })
}
