use std::{fs, path::Path};

use contracts::{
    DeployExecutionRepairTaskResult, DeployProvider, DeploymentErrorWindow,
    DeploymentFailedContract, DeploymentFailureDiagnostic, DeploymentFailureKind,
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
    request_index::get_request_index_entry,
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
    prepare::{deployment_generated_file_refs, read_spec},
    run::deploy_retry_after_repair,
    DeployToolInput,
};

const DEFAULT_DEPLOY_REPAIR_MAX_ATTEMPTS: u32 = 2;

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

pub(crate) fn latest_repair_summary(project_root: &Path) -> Option<Value> {
    latest_repair_action(project_root)
        .ok()
        .flatten()
        .map(|request| compact_repair_summary(project_root, &request))
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
    let diagnostics = diagnose_deployment_failure(spec, stdout, stderr);
    let editable_files = editable_files_for(spec, failure_kind);
    let (failure_owner, repair_route) = classify_repair(failure_kind, editable_files.len());
    let current_error_window = error_window(stdout, stderr, &diagnostics);
    if let Some(existing) =
        reusable_pending_execution_repair_action(project_root, failure_kind, &current_error_window)?
    {
        return Ok(repair_next(project_root, &existing));
    }
    let attempts = next_repair_attempt(project_root, failure_kind, &current_error_window)?;
    let max_attempts = DEFAULT_DEPLOY_REPAIR_MAX_ATTEMPTS;
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
            &diagnostics,
            attempts.saturating_add(1),
            max_attempts,
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
        error_window: Some(current_error_window),
        diagnostics: diagnostics.clone(),
        suggested_actions: suggested_actions(failure_kind, failure_owner, &diagnostics),
        editable_files,
        protected_files: protected_files_for(spec),
        instruction: instruction_for(failure_kind, failure_owner),
        max_attempts,
        attempts,
        status: "pending".to_string(),
    };
    write_json_atomic(&paths.repair_action_file, &request)?;
    Ok(repair_next(project_root, &request))
}

pub fn repair_next(project_root: &Path, request: &DeploymentRepairAction) -> LoomMcpActionResult {
    if repair_attempt_limit_reached(request) {
        return repair_attempt_limit_result(project_root, request);
    }
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
                    primary_reason: primary_repair_reason(request),
                    diagnostics: compact_next_diagnostics(&request.diagnostics),
                    suggested_actions: request
                        .suggested_actions
                        .iter()
                        .take(6)
                        .cloned()
                        .collect::<Vec<_>>(),
                    editable_files: request.editable_files.clone(),
                    protected_files: request.protected_files.clone(),
                    source_model_ref: spec.as_ref().map(|spec| spec.source_model_ref.clone()),
                    topology_ref: spec.as_ref().map(|spec| spec.topology_ref.clone()),
                    generated_file_refs: spec
                        .as_ref()
                        .map(deployment_generated_file_refs)
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
                    error_window: request.error_window.as_ref().map(compact_next_error_window),
                    read_policy: delivery_core::DeploymentRepairReadPolicy {
                        first_read: "Use next.primaryReason, next.diagnostics, and next.errorWindow before reading refs.".to_string(),
                        diagnostics_ref: "Read next.diagnosticsRef only when compact diagnostics and errorWindow are insufficient.".to_string(),
                        full_log_ref: "Read full logs only after diagnosticsRef is still insufficient or the retry returns a new failure.".to_string(),
                    },
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
                gate: Some(json!({
                    "repairRef": to_project_relative(
                        project_root,
                        &deployment_paths(project_root).repair_action_file
                    ).ok(),
                    "repairSummary": compact_repair_summary(project_root, request)
                })),
            })
        }
        DeploymentRepairRoute::None => LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: project_root.to_string_lossy().into_owned(),
            blockers: request.suggested_actions.clone(),
            recommended_tool: Some("loom.deployStatus".to_string()),
            details: Some(json!({
                "repairRef": to_project_relative(
                    project_root,
                    &deployment_paths(project_root).repair_action_file
                ).ok(),
                "repairSummary": compact_repair_summary(project_root, request)
            })),
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
                "repairId must match outputContract.repairId from the active deploy repair action.",
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
                "deploymentFailureRef must match outputContract.deploymentFailureRef from the active deploy repair action.",
            )],
        ));
    }
    if !is_valid_deploy_repair_status(&result.status) {
        return Ok(repairable(
            input,
            authorized,
            target_file,
            vec![repair_issue(
                "DEPLOY_REPAIR_STATUS_INVALID",
                "status",
                "Deploy execution repair status must be completed, completed_with_notes, blocked, or failed.",
            )],
        ));
    }
    if is_completed_deploy_repair_status(&result.status) && result.changed_files.is_empty() {
        return Ok(repairable(
            input,
            authorized,
            target_file,
            vec![repair_issue(
                "DEPLOY_REPAIR_CHANGED_FILES_REQUIRED",
                "changedFiles",
                "Completed deploy execution repair requires at least one changed application code, package script, or runtime wiring file.",
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
    if let Some(request_id) =
        existing_materialized_execution_repair(project_root, &request.repair_id, &failure_ref)?
    {
        let config = state::read_project_config(&project_root.to_string_lossy())?;
        let request_ref = state::request_manifest::request_ref(&config.project_id, &request_id);
        let result_file = to_project_relative(
            project_root,
            &deploy_execution_repair_result_file(project_root, &request_id),
        )?;
        return deploy_execution_repair_next(
            project_root,
            &failure,
            failure_ref,
            request_ref,
            result_file,
        );
    }
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
    let mut repair_context = json!({
        "repairOrigin": "deploy_failure",
        "deploymentFailureRef": failure_ref,
        "failureKind": failure.failure_kind,
        "failureOwner": failure.failure_owner,
        "failedContractFields": failure.failed_contract_fields,
        "requiredCodeLevelChecks": failure.required_code_level_checks,
        "errorWindow": failure.error_window
    });
    if let Some(failed_at) = &failure.failed_at {
        repair_context["failedAt"] = json!(failed_at);
    }
    if let Some(failed_contract) = &failure.failed_contract {
        repair_context["failedContract"] = json!(failed_contract);
    }
    if !failure.deploy_command.is_empty() {
        repair_context["deployCommand"] = json!(failure.deploy_command);
    }
    if let Some(exit_code) = failure.exit_code {
        repair_context["exitCode"] = json!(exit_code);
    }
    if let Some(full_log_ref) = &failure.full_log_ref {
        repair_context["fullLogRef"] = json!(full_log_ref);
    }
    let mut deploy_failure_fields = vec![
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
        "executionRules.mustNotClaimDeploymentSuccess",
    ];
    for (key, field) in [
        ("failedAt", "repairContext.failedAt"),
        ("deployCommand", "repairContext.deployCommand"),
        ("exitCode", "repairContext.exitCode"),
        ("fullLogRef", "repairContext.fullLogRef"),
    ] {
        if repair_context.get(key).is_some() {
            deploy_failure_fields.push(field);
        }
    }
    if let Some(failed_contract) = repair_context
        .get("failedContract")
        .and_then(Value::as_object)
    {
        deploy_failure_fields.push("repairContext.failedContract.field");
        if failed_contract.get("command").is_some() {
            deploy_failure_fields.push("repairContext.failedContract.command");
        }
        deploy_failure_fields.push("repairContext.failedContract.workingDirectory");
    }
    let request_root = json!({
        "schemaVersion": "1.0",
        "requestType": "deploy_execution_repair",
        "requestId": request_id,
        "artifactKind": ArtifactKind::DeployExecutionRepairResult,
        "executionKind": "deploy_execution_repair",
        "repairContext": repair_context,
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
                    "fields": deploy_failure_fields
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
    deploy_execution_repair_next(
        project_root,
        &failure,
        failure_ref,
        stored.request_ref,
        result_file,
    )
}

fn deploy_execution_repair_next(
    project_root: &Path,
    failure: &DeploymentFailureReport,
    failure_ref: String,
    request_ref: String,
    result_file: String,
) -> StateResult<LoomMcpActionResult> {
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string_lossy().into_owned(),
        request_ref: request_ref.clone(),
    })
    .map_err(|error| StateError::InvalidArgument(error.to_string()))?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string_lossy().into_owned(),
            LoomMcpNextAction::ExecuteTask(ExecuteTaskNext {
                execution_kind: ExecutionKind::DeployExecutionRepair,
                repair_origin: Some(RepairOrigin::DeployFailure),
                request_ref,
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
                    failed_contract_fields: failure.failed_contract_fields.clone(),
                    required_code_level_checks: failure.required_code_level_checks.clone(),
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
        "changedFiles": ["project-relative/source-or-config-file"],
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

fn same_pending_repair_action(
    project_root: &Path,
    failure_kind: DeploymentFailureKind,
    window: &DeploymentErrorWindow,
) -> StateResult<Option<DeploymentRepairAction>> {
    let path = deployment_paths(project_root).repair_action_file;
    if !path_exists(&path) {
        return Ok(None);
    }
    let existing: DeploymentRepairAction = read_json(&path)?;
    if existing.status == "pending"
        && existing.failure_kind == failure_kind
        && existing
            .error_window
            .as_ref()
            .map(|existing| existing.lines.as_slice() == window.lines.as_slice())
            .unwrap_or(false)
    {
        return Ok(Some(existing));
    }
    Ok(None)
}

fn reusable_pending_execution_repair_action(
    project_root: &Path,
    failure_kind: DeploymentFailureKind,
    window: &DeploymentErrorWindow,
) -> StateResult<Option<DeploymentRepairAction>> {
    let Some(existing) = same_pending_repair_action(project_root, failure_kind, window)? else {
        return Ok(None);
    };
    if existing.repair_route != DeploymentRepairRoute::ExecutionRepair {
        return Ok(None);
    }
    let Some(failure_ref) = existing.failure_ref.as_deref() else {
        return Ok(Some(existing));
    };
    if existing_materialized_execution_repair(project_root, &existing.repair_id, failure_ref)?
        .is_some()
    {
        return Ok(Some(existing));
    }
    Ok(None)
}

fn next_repair_attempt(
    project_root: &Path,
    failure_kind: DeploymentFailureKind,
    window: &DeploymentErrorWindow,
) -> StateResult<u32> {
    let Some(existing) = same_pending_repair_action(project_root, failure_kind, window)? else {
        return Ok(0);
    };
    Ok(existing.attempts.saturating_add(1))
}

fn repair_attempt_limit_reached(request: &DeploymentRepairAction) -> bool {
    !matches!(request.repair_route, DeploymentRepairRoute::None)
        && request.max_attempts > 0
        && request.attempts >= request.max_attempts
}

fn repair_attempt_limit_result(
    project_root: &Path,
    request: &DeploymentRepairAction,
) -> LoomMcpActionResult {
    LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
        project_root: project_root.to_string_lossy().into_owned(),
        blockers: vec![format!(
            "Deployment repair attempt limit reached for {} after {} automatic repair attempts.",
            enum_string(&request.failure_kind),
            request.attempts
        )],
        recommended_tool: Some("loom.deployInspect".to_string()),
        details: Some(json!({
            "repairRef": to_project_relative(
                project_root,
                &deployment_paths(project_root).repair_action_file
            ).ok(),
            "repairSummary": compact_repair_summary(project_root, request)
        })),
    })
}

fn compact_repair_summary(project_root: &Path, request: &DeploymentRepairAction) -> Value {
    json!({
        "hasRepairRequest": true,
        "repairId": request.repair_id.clone(),
        "failureKind": enum_string(&request.failure_kind),
        "failureOwner": enum_string(&request.failure_owner),
        "repairRoute": enum_string(&request.repair_route),
        "primaryReason": primary_repair_reason(request),
        "nextAction": repair_next_action(request),
        "diagnostics": compact_diagnostics(&request.diagnostics),
        "errorWindow": request.error_window.as_ref().map(compact_error_window),
        "suggestedActions": request.suggested_actions.iter().take(6).cloned().collect::<Vec<_>>(),
        "editableFiles": request.editable_files.clone(),
        "protectedFiles": request.protected_files.clone(),
        "failureRef": request.failure_ref.clone(),
        "fullLogRef": request.full_log_ref.clone(),
        "attempts": request.attempts,
        "maxAttempts": request.max_attempts,
        "readPolicy": {
            "firstRead": "repairSummary.diagnostics and repairSummary.errorWindow",
            "fullLogRef": "Read only when compact diagnostics and errorWindow are insufficient."
        },
        "sourceRefs": {
            "repairActionRef": to_project_relative(
                project_root,
                &deployment_paths(project_root).repair_action_file
            ).ok()
        }
    })
}

fn primary_repair_reason(request: &DeploymentRepairAction) -> String {
    if let Some(diagnostic) = request.diagnostics.first() {
        return format!("{}: {}", diagnostic.code, diagnostic.message);
    }
    if let Some(line) = request.error_window.as_ref().and_then(|window| {
        window
            .lines
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
    }) {
        return line.clone();
    }
    if let Some(action) = request.suggested_actions.first() {
        return action.clone();
    }
    format!(
        "{} owned by {}",
        enum_string(&request.failure_kind),
        enum_string(&request.failure_owner)
    )
}

fn repair_next_action(request: &DeploymentRepairAction) -> &'static str {
    if repair_attempt_limit_reached(request) {
        return "inspect_attempt_limit";
    }
    match request.repair_route {
        DeploymentRepairRoute::DeployRepair => "repair_deployment_assets",
        DeploymentRepairRoute::ExecutionRepair => "repair_application_code",
        DeploymentRepairRoute::ManualReview => "manual_review",
        DeploymentRepairRoute::None => match request.failure_owner {
            DeploymentFailureOwner::Environment => "fix_environment_then_retry",
            DeploymentFailureOwner::ExternalSystem => "fix_external_system_then_retry",
            _ => "inspect_blocker",
        },
    }
}

fn compact_diagnostics(diagnostics: &[DeploymentFailureDiagnostic]) -> Vec<Value> {
    diagnostics
        .iter()
        .take(5)
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code.clone(),
                "severity": diagnostic.severity.clone(),
                "message": diagnostic.message.clone(),
                "evidence": diagnostic.evidence.iter().take(3).cloned().collect::<Vec<_>>(),
                "suggestedAction": diagnostic.suggested_action.clone()
            })
        })
        .collect()
}

fn compact_next_diagnostics(
    diagnostics: &[DeploymentFailureDiagnostic],
) -> Vec<delivery_core::DeploymentFailureDiagnostic> {
    diagnostics
        .iter()
        .take(5)
        .map(|diagnostic| delivery_core::DeploymentFailureDiagnostic {
            code: diagnostic.code.clone(),
            severity: diagnostic.severity.clone(),
            message: diagnostic.message.clone(),
            evidence: diagnostic.evidence.iter().take(3).cloned().collect(),
            suggested_action: diagnostic.suggested_action.clone(),
        })
        .collect()
}

fn compact_next_error_window(
    window: &DeploymentErrorWindow,
) -> delivery_core::DeploymentErrorWindow {
    let max_lines = 24usize;
    let lines = window
        .lines
        .iter()
        .rev()
        .take(max_lines)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    delivery_core::DeploymentErrorWindow {
        started_at: None,
        ended_at: None,
        lines,
        truncated: window.truncated || window.lines.len() > max_lines,
        total_line_count: window.total_line_count,
        matched_patterns: window.matched_patterns.clone(),
    }
}

fn compact_error_window(window: &DeploymentErrorWindow) -> Value {
    let max_lines = 24usize;
    let lines = window
        .lines
        .iter()
        .rev()
        .take(max_lines)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    json!({
        "lines": lines,
        "truncated": window.truncated || window.lines.len() > max_lines,
        "totalLineCount": window.total_line_count,
        "matchedPatterns": window.matched_patterns.clone()
    })
}

fn existing_materialized_execution_repair(
    project_root: &Path,
    repair_id: &str,
    failure_ref: &str,
) -> StateResult<Option<String>> {
    let repairs_dir = deployment_paths(project_root).repairs_dir;
    if !path_exists(&repairs_dir) {
        return Ok(None);
    }
    let mut matches = vec![];
    for entry in fs::read_dir(&repairs_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let request_id = entry.file_name().to_string_lossy().into_owned();
        if !request_id.starts_with("deploy_exec_repair_") {
            continue;
        }
        if path_exists(&entry.path().join("result.json")) {
            continue;
        }
        let output_contract_file = entry.path().join("request.refs/output-contract.json");
        if !path_exists(&output_contract_file) {
            continue;
        }
        let output_contract = read_json_value(&output_contract_file)?;
        let same_repair = output_contract
            .get("repairId")
            .and_then(serde_json::Value::as_str)
            == Some(repair_id);
        let same_failure = output_contract
            .get("deploymentFailureRef")
            .and_then(serde_json::Value::as_str)
            == Some(failure_ref);
        if same_repair && same_failure {
            if get_request_index_entry(&project_root.to_string_lossy(), &request_id).is_ok() {
                matches.push(request_id);
            }
        }
    }
    matches.sort();
    Ok(matches.into_iter().last())
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
    if is_completed_deploy_repair_status(&result.status) {
        let failed_or_blocked_checks = result
            .runtime_delivery_evidence
            .get("codeLevelChecks")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        let status = item.get("status").and_then(Value::as_str)?;
                        matches!(status, "failed" | "blocked").then(|| {
                            item.get("checkId")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                                .to_string()
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !failed_or_blocked_checks.is_empty() {
            return Err(StateError::InvalidArgument(format!(
                "completed deploy execution repair cannot contain failed or blocked runtime code-level checks: {}.",
                failed_or_blocked_checks.join(", ")
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

fn is_valid_deploy_repair_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "completed_with_notes" | "blocked" | "failed"
    )
}

fn is_completed_deploy_repair_status(status: &str) -> bool {
    matches!(status, "completed" | "completed_with_notes")
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
            DeploymentRepairRoute::ExecutionRepair,
        ),
        _ if editable_file_count > 0 => (
            DeploymentFailureOwner::DeploymentAssets,
            DeploymentRepairRoute::DeployRepair,
        ),
        _ => (DeploymentFailureOwner::Unknown, DeploymentRepairRoute::None),
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
        _ if spec.provider == DeployProvider::ComposeExisting => vec![],
        _ => deployment_generated_file_refs(spec),
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
    diagnostics: &[DeploymentFailureDiagnostic],
    attempt: u32,
    max_attempts: u32,
) -> StateResult<DeploymentFailureReport> {
    let fields = failed_contract_fields(failure_kind);
    let failed_contract = failed_contract_for(spec, failure_kind);
    let required_checks = fields
        .iter()
        .map(|field| format!("check_{field}").replace('.', "_"))
        .collect::<Vec<_>>();
    let window = error_window(stdout, stderr, diagnostics);
    let mut must_not_edit = vec![".loom".to_string(), spec.runtime_contract_ref.clone()];
    must_not_edit.extend(spec.files.reused.clone());
    if repair_route == DeploymentRepairRoute::ExecutionRepair {
        must_not_edit.extend(deployment_generated_file_refs(spec));
    }
    must_not_edit.sort();
    must_not_edit.dedup();
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
        failed_at: Some(failed_at_for(failure_kind).to_string()),
        failed_contract: Some(failed_contract),
        deploy_command: command,
        exit_code: Some(exit_code),
        full_log_ref: Some(to_project_relative(
            project_root,
            &deployment_paths(project_root).log_file,
        )?),
        failed_contract_fields: fields,
        required_code_level_checks: required_checks,
        error_window: window,
        must_not_edit,
        attempt,
        max_attempts,
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

fn failed_contract_for(
    spec: &DeploymentSpec,
    failure_kind: DeploymentFailureKind,
) -> DeploymentFailedContract {
    let primary = spec
        .source_model
        .services
        .iter()
        .find(|service| service.service_id == spec.source_model.primary_service_id)
        .or_else(|| spec.source_model.services.first());
    let working_directory = primary
        .and_then(|service| service.working_directory.clone())
        .unwrap_or_else(|| ".".to_string());
    match failure_kind {
        DeploymentFailureKind::BuildCommandFailed => DeploymentFailedContract {
            field: "build.command".to_string(),
            command: spec.runtime_contract.build_command.clone(),
            working_directory,
        },
        DeploymentFailureKind::StartCommandFailed => DeploymentFailedContract {
            field: "start.command".to_string(),
            command: spec.runtime_contract.start_command.clone(),
            working_directory,
        },
        DeploymentFailureKind::ApplicationStartupFailed => DeploymentFailedContract {
            field: "runtime.startup".to_string(),
            command: spec
                .runtime_contract
                .start_command
                .clone()
                .or_else(|| primary.and_then(|service| service.start_command.clone())),
            working_directory,
        },
        DeploymentFailureKind::HttpProbeFailed | DeploymentFailureKind::PreviewNotVerified => {
            DeploymentFailedContract {
                field: "httpProbes.previewPath".to_string(),
                command: None,
                working_directory,
            }
        }
        DeploymentFailureKind::ApiRouteNotVerified => DeploymentFailedContract {
            field: "deploymentTopology.apiRoutes".to_string(),
            command: None,
            working_directory,
        },
        DeploymentFailureKind::Healthcheck => DeploymentFailedContract {
            field: "httpProbes.healthPath".to_string(),
            command: None,
            working_directory,
        },
        _ => DeploymentFailedContract {
            field: "runtime.delivery".to_string(),
            command: None,
            working_directory,
        },
    }
}

fn failed_at_for(failure_kind: DeploymentFailureKind) -> &'static str {
    match failure_kind {
        DeploymentFailureKind::BuildCommandFailed => "runtime_build_command",
        DeploymentFailureKind::StartCommandFailed => "runtime_start_command",
        DeploymentFailureKind::ApplicationStartupFailed => "runtime_application_startup",
        DeploymentFailureKind::HttpProbeFailed => "runtime_http_probe",
        DeploymentFailureKind::PreviewNotVerified => "runtime_preview_probe",
        DeploymentFailureKind::ApiRouteNotVerified => "runtime_api_route_probe",
        DeploymentFailureKind::Healthcheck => "runtime_healthcheck",
        _ => "deployment_runtime_validation",
    }
}

fn error_window(
    stdout: &str,
    stderr: &str,
    diagnostics: &[DeploymentFailureDiagnostic],
) -> DeploymentErrorWindow {
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
    let mut patterns = selected
        .iter()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("error") || lower.contains("failed") || lower.contains("exception")
        })
        .map(|_| "error".to_string())
        .collect::<Vec<_>>();
    patterns.extend(diagnostics.iter().map(|diagnostic| diagnostic.code.clone()));
    patterns.sort();
    patterns.dedup();
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
    diagnostics: &[DeploymentFailureDiagnostic],
) -> Vec<String> {
    let diagnostic_actions = diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.suggested_action))
        .collect::<Vec<_>>();
    let mut actions = match owner {
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
    };
    actions.extend(diagnostic_actions);
    actions
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

fn diagnose_deployment_failure(
    spec: &DeploymentSpec,
    stdout: &str,
    stderr: &str,
) -> Vec<DeploymentFailureDiagnostic> {
    let lines = [stdout, stderr]
        .join("\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let lower = lines
        .iter()
        .map(|line| line.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    for rule in DEPLOYMENT_DIAGNOSTIC_RULES {
        push_diagnostic_if(&mut diagnostics, &lines, &lower, rule);
    }
    if !spec.environment.missing.is_empty()
        && lower
            .iter()
            .any(|line| contains_any(line, &["env", "secret", "config", "credential"]))
    {
        diagnostics.push(DeploymentFailureDiagnostic {
            code: "spec_missing_env".to_string(),
            severity: "warning".to_string(),
            message: "DeploymentSpec already contains missing environment diagnostics.".to_string(),
            evidence: spec
                .environment
                .missing
                .iter()
                .map(|variable| variable.name.clone())
                .take(12)
                .collect(),
            suggested_action:
                "Review environment.missing before changing Dockerfile or Compose commands."
                    .to_string(),
        });
    }
    if !spec.bootstrap.tasks.is_empty()
        && lower.iter().any(|line| {
            contains_any(
                line,
                &["migration", "migrate", "relation", "table", "schema"],
            )
        })
    {
        diagnostics.push(DeploymentFailureDiagnostic {
            code: "bootstrap_task_relevant".to_string(),
            severity: "warning".to_string(),
            message: "Detected bootstrap tasks may be relevant to this failure.".to_string(),
            evidence: spec
                .bootstrap
                .tasks
                .iter()
                .take(12)
                .map(|task| format!("{}: {}", task.kind, task.command))
                .collect(),
            suggested_action:
                "Ask before running bootstrap or migration commands; use them as diagnosis first."
                    .to_string(),
        });
    }
    dedupe_diagnostics(diagnostics)
}

struct DiagnosticRule {
    code: &'static str,
    severity: &'static str,
    needles: &'static [&'static str],
    message: &'static str,
    suggested_action: &'static str,
}

const DEPLOYMENT_DIAGNOSTIC_RULES: &[DiagnosticRule] = &[
    DiagnosticRule {
        code: "registry_network",
        severity: "error",
        needles: &[
            "failed to fetch oauth token",
            "failed to authorize",
            "deadlineexceeded",
            "i/o timeout",
            "tls handshake timeout",
            "temporary failure in name resolution",
            "no such host",
            "connection timed out",
            "network is unreachable",
            "registry-1.docker.io",
            "auth.docker.io",
        ],
        message: "Docker could not reach or authenticate with the container registry.",
        suggested_action: "Fix Docker registry or network access, configure a registry mirror, pre-pull the base image, or retry when registry access is healthy.",
    },
    DiagnosticRule {
        code: "missing_module",
        severity: "error",
        needles: &["cannot find module", "module_not_found", "no module named"],
        message: "The app could not load a required runtime module.",
        suggested_action: "Check dependency installation, package lockfiles, optional native packages, and production/runtime dependency pruning.",
    },
    DiagnosticRule {
        code: "native_optional_dependency",
        severity: "error",
        needles: &[
            "lightningcss",
            "sharp",
            "esbuild",
            "rollup",
            "@next/swc",
            "oxide",
            "linux-arm64",
            "linux-x64",
            "gnu.node",
            "musl.node",
        ],
        message: "A platform-specific native optional dependency may be missing in the container image.",
        suggested_action: "Repair the install step or lockfile so the Linux container receives the required native package.",
    },
    DiagnosticRule {
        code: "port_in_use",
        severity: "error",
        needles: &[
            "eaddrinuse",
            "address already in use",
            "port is already allocated",
            "bind: address already in use",
        ],
        message: "A configured deployment port is already in use.",
        suggested_action: "Change the generated host port or stop the conflicting local/container process before retrying.",
    },
    DiagnosticRule {
        code: "database_schema",
        severity: "error",
        needles: &[
            "relation ",
            "table ",
            "no such table",
            "pending migrations",
            "pendingmigrationerror",
            "migration pending",
        ],
        message: "The app likely needs a database schema or migration step before serving traffic.",
        suggested_action: "Use bootstrap or migration commands only as explicit evidence; do not run migrations automatically without user approval.",
    },
    DiagnosticRule {
        code: "framework_startup_failed",
        severity: "error",
        needles: &[
            "application failed to start",
            "beancreationexception",
            "unsatisfieddependencyexception",
            "applicationcontextexception",
            "webserverexception",
            "flywayexception",
            "liquibaseexception",
            "hibernateexception",
            "schemamanagementexception",
            "psqlexception",
            "communications link failure",
            "unable to obtain jdbc connection",
            "django.db.utils",
            "improperlyconfigured",
            "sqlstate[",
        ],
        message: "The application framework failed during startup.",
        suggested_action: "Route through deploy execution repair and inspect dependencies, migrations, runtime configuration, and startup code before editing generated deployment assets.",
    },
    DiagnosticRule {
        code: "missing_env",
        severity: "error",
        needles: &[
            "required environment",
            "environment variable",
            "secret missing",
            "secret not set",
            "database_url",
            "app_key",
            "secret_key_base",
        ],
        message: "The app reported a missing or invalid environment variable.",
        suggested_action: "Compare logs with DeploymentSpec.environment.missing and add safe local placeholders only when appropriate.",
    },
    DiagnosticRule {
        code: "permission_denied",
        severity: "error",
        needles: &["permission denied", "eacces", "operation not permitted"],
        message: "The container hit a filesystem or executable permission problem.",
        suggested_action: "Repair generated Dockerfile ownership, chmod executable scripts, or adjust writable runtime directories.",
    },
];

fn push_diagnostic_if(
    diagnostics: &mut Vec<DeploymentFailureDiagnostic>,
    lines: &[String],
    lower_lines: &[String],
    rule: &DiagnosticRule,
) {
    if !lower_lines
        .iter()
        .any(|line| contains_any(line, rule.needles))
    {
        return;
    }
    diagnostics.push(DeploymentFailureDiagnostic {
        code: rule.code.to_string(),
        severity: rule.severity.to_string(),
        message: rule.message.to_string(),
        evidence: evidence_lines(lines, lower_lines, rule.needles),
        suggested_action: rule.suggested_action.to_string(),
    });
}

fn evidence_lines(lines: &[String], lower_lines: &[String], needles: &[&str]) -> Vec<String> {
    lines
        .iter()
        .zip(lower_lines)
        .filter(|(_, lower)| contains_any(lower, needles))
        .map(|(line, _)| line.clone())
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn dedupe_diagnostics(
    diagnostics: Vec<DeploymentFailureDiagnostic>,
) -> Vec<DeploymentFailureDiagnostic> {
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for diagnostic in diagnostics {
        if seen.insert(diagnostic.code.clone()) {
            result.push(diagnostic);
        }
    }
    result
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
