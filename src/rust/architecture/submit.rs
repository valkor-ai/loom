use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    normalize_ui_surface_decision_contract_for_persist, validate_ui_surface_decision_contract,
    AcceptanceMatrixEntry, ArchitectureArtifactContract, ArchitectureArtifactSource,
    ArchitectureArtifactStatus, ArchitectureDetailCoverageEntry, ArchitectureHandoff,
    ArchitectureQuality, ArchitectureSectionCandidateAgentWritable, ArchitectureSectionGroup,
    ArchitectureSectionStatus, COVERAGE_ARTIFACT_TYPES,
};
use delivery_core::{
    DomainDispatcher, FileSubmitInput, LoomMcpActionResult, LoomMcpBlockedResult, LoomMcpFailure,
    LoomMcpFailureResult, LoomMcpRepairableErrorResult, OperationContext, ReadRequestFieldsInput,
    RouteAction, RouteActionKind, SubmitAcceptedEvent, TransitionEngine, TransitionStore,
};
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    request_index::get_request_index_entry,
    write_targets::AuthorizedWriteSet,
};

use crate::{
    paths::{
        architecture_contract_file, architecture_latest_file, architecture_section_snapshot_file,
        section_name,
    },
    request::{architecture_read_groups, required_content_keys, section_order},
};

pub fn accept_architecture_section_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher + Clone,
{
    match accept_architecture_section_file_inner(
        input,
        authorized,
        ArchitectureSubmitMode::Generation,
        dispatcher,
    ) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "ARCHITECTURE_SECTION_ACCEPT_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(8),
                domain: Some("architecture".to_string()),
                route_action: Some("architecture_section_submit".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

pub fn accept_architecture_repair_file<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    dispatcher: D,
) -> LoomMcpActionResult
where
    D: DomainDispatcher + Clone,
{
    match accept_architecture_section_file_inner(
        input,
        authorized,
        ArchitectureSubmitMode::Repair,
        dispatcher,
    ) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "ARCHITECTURE_REPAIR_ACCEPT_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(9),
                domain: Some("architecture".to_string()),
                route_action: Some("architecture_artifact_repair".to_string()),
                recovery_tool: None,
            },
        }),
    }
}

#[derive(Debug, Clone, Copy)]
enum ArchitectureSubmitMode {
    Generation,
    Repair,
}

impl ArchitectureSubmitMode {
    fn latest_ref_key(self) -> &'static str {
        match self {
            Self::Generation => "architectureRequestRef",
            Self::Repair => "activeRepairActionRef",
        }
    }

    fn resubmit_tool(self) -> &'static str {
        match self {
            Self::Generation => "loom.architectureSectionSubmitFile",
            Self::Repair => "loom.repairSubmitFile",
        }
    }

    fn fix_scope(self) -> &'static str {
        match self {
            Self::Generation => "architecture_section_candidate_only",
            Self::Repair => "architecture_repair_section_candidate_only",
        }
    }

    fn stale_code(self) -> &'static str {
        match self {
            Self::Generation => "STALE_ARCHITECTURE_REQUEST",
            Self::Repair => "STALE_ARCHITECTURE_REPAIR_REQUEST",
        }
    }

    fn target_batch(self) -> u32 {
        match self {
            Self::Generation => 8,
            Self::Repair => 9,
        }
    }

    fn artifact_source(self) -> &'static str {
        match self {
            Self::Generation => "architecture_section_submit",
            Self::Repair => "architecture_repair_section_submit",
        }
    }

    fn next_reason(self, section: ArchitectureSectionGroup) -> String {
        match self {
            Self::Generation => format!("{}_ready", section_name(section)),
            Self::Repair => format!("{}_repair_ready", section_name(section)),
        }
    }
}

fn accept_architecture_section_file_inner<D>(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    mode: ArchitectureSubmitMode,
    dispatcher: D,
) -> Result<LoomMcpActionResult, state::store::StateError>
where
    D: DomainDispatcher + Clone,
{
    let Some(target) = authorized.targets.first() else {
        return Ok(repairable(
            input,
            authorized,
            String::new(),
            vec![issue(
                "TARGET_MISSING",
                "candidate",
                "No authorized Architecture section target was written.",
            )],
            mode,
        ));
    };
    let delivery_id = authorized.delivery_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("authorized deliveryId is missing".to_string())
    })?;
    let phase_id = authorized.phase_id.clone().ok_or_else(|| {
        state::store::StateError::InvalidArgument("authorized phaseId is missing".to_string())
    })?;
    if let Some(result) = ensure_latest_request(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &input.request_ref,
        mode,
    )? {
        return Ok(result);
    }

    let project_root = Path::new(&input.project_root);
    let candidate_file = from_project_relative(project_root, &target.path)?;
    let request_root = load_request_root(&input.project_root, &authorized.request_id)?;
    let current_section = parse_section(&request_root, "/sectionState/currentSection")?;
    let mut raw = state::store::read_json_value(&candidate_file)?;
    normalize_architecture_candidate_envelope(
        &mut raw,
        &authorized.request_id,
        &delivery_id,
        &phase_id,
        current_section,
        &request_root,
    );
    let mut candidate: ArchitectureSectionCandidateAgentWritable =
        match serde_json::from_value(raw.clone()) {
            Ok(candidate) => candidate,
            Err(error) => {
                return Ok(repairable(
                    input,
                    authorized,
                    target.path.clone(),
                    vec![issue(
                        "ARCHITECTURE_SECTION_SCHEMA_INVALID",
                        "candidate",
                        &format!(
                            "Architecture section candidate JSON has an invalid schema: {error}"
                        ),
                    )],
                    mode,
                ));
            }
        };

    if matches!(candidate.status, ArchitectureSectionStatus::Blocked) {
        return Ok(LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: input.project_root.clone(),
            blockers: candidate
                .blocked_reasons
                .iter()
                .map(|reason| reason.message.clone())
                .collect(),
            recommended_tool: Some("loom.architectureSectionSubmitFile".to_string()),
            details: Some(json!({
                "section": candidate.section,
                "blockedReasons": candidate.blocked_reasons
            })),
        }));
    }

    let source_refs = read_source_refs(&input.project_root, &input.request_ref, &request_root)?;
    let section_outputs =
        parse_section_outputs(&input.project_root, &authorized.request_id, &request_root)?;
    let allowed_refs = if section_uses_allowed_refs(current_section) {
        let allowed_ref_fields = state::read_request_fields(ReadRequestFieldsInput {
            project_root: input.project_root.clone(),
            request_ref: input.request_ref.clone(),
            fields: [
                "allowedRefs.scopeRefs",
                "allowedRefs.acceptanceRefs",
                "allowedRefs.deferredScopeRefs",
                "allowedRefs.excludedScopeRefs",
                "allowedRefs.requirementDetailIds",
            ]
            .iter()
            .map(|field| field.to_string())
            .collect(),
        })?
        .fields;
        json!({
            "scopeRefs": field_value(&allowed_ref_fields, "allowedRefs.scopeRefs"),
            "acceptanceRefs": field_value(&allowed_ref_fields, "allowedRefs.acceptanceRefs"),
            "deferredScopeRefs": field_value(&allowed_ref_fields, "allowedRefs.deferredScopeRefs"),
            "excludedScopeRefs": field_value(&allowed_ref_fields, "allowedRefs.excludedScopeRefs"),
            "requirementDetailIds": field_value(&allowed_ref_fields, "allowedRefs.requirementDetailIds")
        })
    } else {
        json!({})
    };
    let mut candidate_normalized = true;
    if matches!(candidate.section, ArchitectureSectionGroup::Coverage) {
        candidate_normalized = normalize_coverage_deferred_reasons(
            project_root,
            &source_refs,
            &mut candidate.content,
        )?;
    }
    if normalize_frontend_ui_surface_decision_contract(&mut candidate.content, &request_root) {
        candidate_normalized = true;
    }
    if normalize_runtime_delivery_deployment_shape(&mut candidate.content) {
        candidate_normalized = true;
    }

    let mut issues = Vec::new();
    issues.extend(validate_section_content(&candidate));
    if section_uses_allowed_refs(current_section) {
        issues.extend(validate_allowed_refs(&candidate.content, &allowed_refs));
    }
    issues.extend(validate_frontend_rules(&candidate, &request_root));
    issues.extend(validate_runtime_rules(&candidate, &source_refs));
    if matches!(candidate.section, ArchitectureSectionGroup::Coverage) {
        issues.extend(validate_coverage_section(&candidate.content, &allowed_refs));
    }
    if !issues.is_empty() {
        return Ok(repairable(
            input,
            authorized,
            target.path.clone(),
            issues,
            mode,
        ));
    }

    let locator = DeliveryPhaseLocator {
        delivery_id: delivery_id.clone(),
        phase_id: phase_id.clone(),
    };
    let snapshot_file = architecture_section_snapshot_file(
        project_root,
        &locator,
        &authorized.request_id,
        candidate.section,
    );
    if candidate_normalized {
        state::store::write_json_atomic(&candidate_file, &candidate)?;
    }
    state::store::write_json_atomic(&snapshot_file, &candidate)?;

    let next_section = next_section(candidate.section);
    if let Some(next_section) = next_section {
        let next_output = section_outputs
            .iter()
            .find(|output| output.section == next_section)
            .cloned()
            .ok_or_else(|| {
                state::store::StateError::StateCorrupted(format!(
                    "request {} is missing next section output {}",
                    authorized.request_id,
                    section_name(next_section)
                ))
            })?;
        let include_repair_context = matches!(mode, ArchitectureSubmitMode::Repair);
        let include_repair_source_ref = include_repair_context
            && repair_context_has_source_ref(
                &input.project_root,
                &input.request_ref,
                &request_root,
            )?;
        let updated_root = update_request_for_next_section(
            request_root,
            next_section,
            &next_output,
            include_repair_context,
            include_repair_source_ref,
            &source_refs,
        )?;
        update_output_contract_ref(
            &input.project_root,
            &authorized.request_id,
            next_section,
            &next_output,
        )?;
        save_request_root(&input.project_root, &authorized.request_id, &updated_root)?;
        let engine = TransitionEngine {
            store: FileTransitionStore,
            dispatcher: dispatcher.clone(),
        };
        return engine
            .advance_after_submit(
                OperationContext {
                    project_root: input.project_root.clone(),
                },
                SubmitAcceptedEvent {
                    delivery_id,
                    phase_id,
                    source_tool: "loom.architectureSectionSubmitFile".to_string(),
                    accepted_artifact_ref: format!(
                        "{}/targets/{}",
                        input.request_ref, target.target_id
                    ),
                    next_action: Some(RouteAction {
                        kind: RouteActionKind::ArchitectureArtifactContract,
                        source: mode.artifact_source().to_string(),
                        reason: mode.next_reason(candidate.section),
                        prompt: None,
                        accepted_responses: vec![],
                        request_ref: Some(input.request_ref.clone()),
                        details: None,
                        target_phase_id: None,
                    }),
                },
            )
            .map_err(to_state_error);
    }

    let contract = assemble_architecture_contract(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &section_outputs,
        &source_refs,
    )?;
    let contract_file = architecture_contract_file(project_root, &locator);
    state::store::write_json_atomic(&contract_file, &contract)?;
    let contract_ref = to_project_relative(project_root, &contract_file)?;
    let latest_file = architecture_latest_file(project_root, &locator);
    state::store::write_json_atomic(
        &latest_file,
        &json!({
            "schemaVersion": "1.0",
            "architectureArtifactContractId": contract.architecture_artifact_contract_id,
            "contractRef": contract_ref,
            "updatedAt": contract.updated_at
        }),
    )?;

    let store = FileTransitionStore;
    let mut delivery = store
        .load_delivery_index(&input.project_root, &delivery_id)
        .map_err(to_state_error)?;
    if let Some(phase) = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
    {
        phase
            .latest_refs
            .insert("architectureArtifact".to_string(), contract_ref.clone());
        if matches!(mode, ArchitectureSubmitMode::Repair) {
            phase.latest_refs.remove("taskPlanRequestId");
            phase.latest_refs.remove("taskPlanRequestRef");
            phase.latest_refs.remove("taskPlan");
            phase.latest_refs.remove("taskPlanRun");
        }
    }
    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(&input.project_root, &delivery)
        .map_err(to_state_error)?;

    let engine = TransitionEngine {
        store: FileTransitionStore,
        dispatcher,
    };
    engine
        .advance_after_submit(
            OperationContext {
                project_root: input.project_root.clone(),
            },
            SubmitAcceptedEvent {
                delivery_id,
                phase_id,
                source_tool: "loom.architectureSectionSubmitFile".to_string(),
                accepted_artifact_ref: format!(
                    "{}/targets/{}",
                    input.request_ref, target.target_id
                ),
                next_action: Some(RouteAction {
                    kind: RouteActionKind::TaskplanGeneration,
                    source: mode.artifact_source().to_string(),
                    reason: if matches!(mode, ArchitectureSubmitMode::Repair) {
                        "architecture_repair_ready".to_string()
                    } else {
                        "architecture_ready".to_string()
                    },
                    prompt: None,
                    accepted_responses: vec![],
                    request_ref: Some(contract_ref),
                    details: None,
                    target_phase_id: None,
                }),
            },
        )
        .map_err(to_state_error)
}

fn normalize_architecture_candidate_envelope(
    raw: &mut Value,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    current_section: ArchitectureSectionGroup,
    request_root: &Value,
) {
    let Some(object) = raw.as_object_mut() else {
        return;
    };
    object.insert("schemaVersion".to_string(), json!("1.0"));
    object.insert("requestId".to_string(), json!(request_id));
    object.insert("deliveryId".to_string(), json!(delivery_id));
    object.insert("phaseId".to_string(), json!(phase_id));
    object.insert("section".to_string(), json!(current_section));
    object.insert("createdAt".to_string(), json!(state::store::now_string()));
    if matches!(current_section, ArchitectureSectionGroup::Foundation) {
        if let Some(content) = object.get_mut("content").and_then(Value::as_object_mut) {
            content.insert(
                "source".to_string(),
                json!({
                    "planningGenerationContractId": request_root.pointer("/contextProjection/planningContractId").cloned().unwrap_or(Value::Null),
                    "technicalBaselineId": request_root.pointer("/contextProjection/technicalBaseline/technicalBaselineId").cloned().unwrap_or(Value::Null)
                }),
            );
        }
    }
}

fn validate_section_content(
    candidate: &ArchitectureSectionCandidateAgentWritable,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let Some(object) = candidate.content.as_object() else {
        issues.push(issue(
            "CONTENT_INVALID",
            "content",
            "Architecture section candidate content must be a JSON object.",
        ));
        return issues;
    };
    for key in required_content_keys(candidate.section) {
        if !object.contains_key(key) {
            issues.push(issue(
                "CONTENT_FIELD_REQUIRED",
                &format!("content.{key}"),
                "Architecture section content is missing a required field.",
            ));
        }
    }
    issues
}

fn section_uses_allowed_refs(section: ArchitectureSectionGroup) -> bool {
    matches!(
        section,
        ArchitectureSectionGroup::Foundation
            | ArchitectureSectionGroup::DomainContract
            | ArchitectureSectionGroup::Behavior
            | ArchitectureSectionGroup::Coverage
    )
}

fn validate_allowed_refs(content: &Value, allowed_refs: &Value) -> Vec<delivery_core::RepairIssue> {
    let scope_refs = string_set(allowed_refs.pointer("/scopeRefs"));
    let acceptance_refs = string_set(allowed_refs.pointer("/acceptanceRefs"));
    let deferred_refs = string_set(allowed_refs.pointer("/deferredScopeRefs"));
    let excluded_refs = string_set(allowed_refs.pointer("/excludedScopeRefs"));
    let mut issues = Vec::new();
    visit_refs(content, "", &mut |path, key, value| match key {
        "scopeRefs" => {
            validate_string_array(value, &scope_refs, path, "INVALID_SCOPE_REF", &mut issues)
        }
        "acceptanceRefs" => validate_string_array(
            value,
            &acceptance_refs,
            path,
            "INVALID_ACCEPTANCE_REF",
            &mut issues,
        ),
        "deferredRef" => validate_string_value(
            value,
            &deferred_refs,
            path,
            "INVALID_DEFERRED_REF",
            &mut issues,
        ),
        "deferredRefs" => validate_string_array(
            value,
            &deferred_refs,
            path,
            "INVALID_DEFERRED_REF",
            &mut issues,
        ),
        "excludedRef" => validate_string_value(
            value,
            &excluded_refs,
            path,
            "INVALID_EXCLUDED_REF",
            &mut issues,
        ),
        "excludedRefs" => validate_string_array(
            value,
            &excluded_refs,
            path,
            "INVALID_EXCLUDED_REF",
            &mut issues,
        ),
        _ => {}
    });
    issues
}

fn validate_frontend_rules(
    candidate: &ArchitectureSectionCandidateAgentWritable,
    request_root: &Value,
) -> Vec<delivery_core::RepairIssue> {
    if !matches!(
        candidate.section,
        ArchitectureSectionGroup::FrontendExperience
    ) {
        return vec![];
    }
    let mut issues = Vec::new();
    let expected = request_root
        .pointer("/frontendExperienceSource/confirmedFrontendExperienceRef")
        .and_then(Value::as_str)
        .or_else(|| {
            request_root
                .pointer("/frontendExperienceSource/currentFrontendExperienceRef")
                .and_then(Value::as_str)
        });
    if let Some(expected) = expected {
        let actual = candidate
            .content
            .pointer("/frontendExperience/sourceRefs/brainstormFrontendExperienceRef")
            .and_then(Value::as_str);
        if actual != Some(expected) {
            issues.push(issue(
                "FRONTEND_AUTHORITY_REF_REQUIRED",
                "content.frontendExperience.sourceRefs.brainstormFrontendExperienceRef",
                "frontend_experience must preserve the confirmed/current frontend authority ref.",
            ));
        }
    }
    if let Some(frontend_experience) = candidate.content.get("frontendExperience") {
        issues.extend(validate_ui_surface_decision_contract(frontend_experience));
    }
    issues
}

fn normalize_frontend_ui_surface_decision_contract(
    content: &mut Value,
    request_root: &Value,
) -> bool {
    let Some(frontend_experience) = content.get_mut("frontendExperience") else {
        return false;
    };
    let ui_quality_seed = request_root.get("uiQualitySeed").unwrap_or(&Value::Null);
    let surface_contract_changed =
        normalize_ui_surface_decision_contract_for_persist(frontend_experience, ui_quality_seed);
    let removed_legacy = frontend_experience
        .as_object_mut()
        .and_then(|object| object.remove("uiQualityContract"))
        .is_some();
    surface_contract_changed || removed_legacy
}

fn normalize_runtime_delivery_deployment_shape(content: &mut Value) -> bool {
    let Some(runtime) = content.get_mut("runtimeDelivery") else {
        return false;
    };
    if runtime.pointer("/status").and_then(Value::as_str) != Some("modified") {
        return false;
    }
    let shape = derived_runtime_deployment_shape(runtime);
    let current = runtime
        .get("deploymentShape")
        .and_then(Value::as_str)
        .map(str::trim);
    if current == Some(shape) {
        return false;
    }
    let Some(object) = runtime.as_object_mut() else {
        return false;
    };
    object.insert("deploymentShape".to_string(), json!(shape));
    true
}

fn derived_runtime_deployment_shape(runtime: &Value) -> &'static str {
    if runtime_frontend_is_served_by_integrated_app(runtime) {
        return "single-service";
    }
    let has_frontend_endpoint = runtime_endpoint_present(runtime, "frontend");
    let has_api_endpoint = runtime_endpoint_present(runtime, "api");
    let has_frontend_surface = runtime_has_surface_kind(runtime, &["frontend", "web", "ui"]);
    let has_api_surface = runtime_has_surface_kind(runtime, &["api", "backend"]);

    if (has_frontend_endpoint && has_api_endpoint)
        || (has_api_endpoint && has_frontend_surface)
        || (has_frontend_surface && has_api_surface)
        || runtime_labeled_commands_declare_frontend_and_backend(runtime)
    {
        "frontend-and-backend"
    } else {
        "single-service"
    }
}

fn runtime_endpoint_present(runtime: &Value, field: &str) -> bool {
    let Some(endpoint) = runtime.get(field) else {
        return false;
    };
    endpoint.get("required").and_then(Value::as_bool) == Some(true)
        || endpoint.as_object().is_some_and(|object| {
            object.iter().any(|(key, value)| {
                key != "required"
                    && value
                        .as_str()
                        .map(|text| !text.trim().is_empty())
                        .unwrap_or_else(|| value.as_array().is_some_and(|items| !items.is_empty()))
            })
        })
}

fn runtime_frontend_is_served_by_integrated_app(runtime: &Value) -> bool {
    [
        "/frontend/servedBy",
        "/frontend/servedByRef",
        "/deliveryMechanics/staticAssets/servedBy",
    ]
    .iter()
    .filter_map(|pointer| runtime.pointer(pointer).and_then(Value::as_str))
    .any(|value| {
        let normalized = value.to_ascii_lowercase().replace(['_', '-'], "");
        [
            "springbootstatic",
            "backendstatic",
            "serverstatic",
            "servicestatic",
            "appstatic",
            "sameprocess",
            "sameapp",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
    })
}

fn runtime_has_surface_kind(runtime: &Value, accepted: &[&str]) -> bool {
    runtime
        .get("runtimeSurfaces")
        .and_then(Value::as_array)
        .map(|surfaces| {
            surfaces.iter().any(|surface| {
                surface
                    .get("kind")
                    .and_then(Value::as_str)
                    .map(|kind| {
                        let normalized = kind.to_ascii_lowercase().replace(['_', '-'], "");
                        accepted
                            .iter()
                            .any(|item| normalized.contains(&item.replace(['_', '-'], "")))
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn runtime_labeled_commands_declare_frontend_and_backend(runtime: &Value) -> bool {
    let mut labels = Vec::new();
    for command in [
        runtime.pointer("/build/command").and_then(Value::as_str),
        runtime.pointer("/buildCommand").and_then(Value::as_str),
        runtime.pointer("/start/command").and_then(Value::as_str),
        runtime.pointer("/startCommand").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    {
        labels.extend(runtime_labeled_command_segments(command));
    }
    let has_frontend = labels
        .iter()
        .any(|label| matches!(label.as_str(), "frontend" | "web" | "client" | "ui"));
    let has_backend = labels
        .iter()
        .any(|label| matches!(label.as_str(), "backend" | "api" | "service" | "server"));
    has_frontend && has_backend
}

fn runtime_labeled_command_segments(command: &str) -> Vec<String> {
    command
        .split(';')
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .filter_map(|part| {
            let trimmed = part.trim();
            let (label, rest) = trimmed.split_once(':')?;
            let label = label.trim();
            if rest.trim().is_empty()
                || label.is_empty()
                || label.contains(char::is_whitespace)
                || !label
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            {
                return None;
            }
            Some(label.to_ascii_lowercase())
        })
        .collect()
}

fn validate_runtime_rules(
    candidate: &ArchitectureSectionCandidateAgentWritable,
    source_refs: &Value,
) -> Vec<delivery_core::RepairIssue> {
    if !matches!(candidate.section, ArchitectureSectionGroup::RuntimeDelivery) {
        return vec![];
    }
    let mut issues = Vec::new();
    let status = candidate
        .content
        .pointer("/runtimeDelivery/status")
        .and_then(Value::as_str);
    if status == Some("unchanged") {
        let expected = source_refs
            .get("previousRuntimeDeliveryRef")
            .and_then(Value::as_str);
        let actual = candidate
            .content
            .pointer("/runtimeDelivery/basis/previousRuntimeDeliveryRef")
            .and_then(Value::as_str);
        if expected.is_none() || actual != expected {
            issues.push(issue(
                "RUNTIME_PREVIOUS_REF_REQUIRED",
                "content.runtimeDelivery.basis.previousRuntimeDeliveryRef",
                "runtime_delivery status=unchanged requires sourceRefs.previousRuntimeDeliveryRef and must copy it exactly.",
            ));
        }
    }
    if status == Some("modified") {
        let Some(runtime) = candidate.content.get("runtimeDelivery") else {
            return issues;
        };
        require_runtime_string(
            runtime,
            "/basis/technicalBaselineRef",
            "content.runtimeDelivery.basis.technicalBaselineRef",
            &mut issues,
        );
        require_runtime_string(
            runtime,
            "/build/command",
            "content.runtimeDelivery.build.command",
            &mut issues,
        );
        require_runtime_array(
            runtime,
            "/build/codeLevelExpectations",
            "content.runtimeDelivery.build.codeLevelExpectations",
            &mut issues,
        );
        if runtime.get("start").is_some() {
            require_runtime_array(
                runtime,
                "/start/codeLevelExpectations",
                "content.runtimeDelivery.start.codeLevelExpectations",
                &mut issues,
            );
            if runtime
                .pointer("/start/port")
                .is_some_and(|port| !port.is_number())
            {
                issues.push(issue(
                    "RUNTIME_FIELD_INVALID",
                    "content.runtimeDelivery.start.port",
                    "runtime_delivery start.port must be a number when present; omit it when unknown.",
                ));
            }
        }
        if runtime
            .pointer("/start/command")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
            && runtime
                .get("runtimeSurfaces")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
        {
            issues.push(issue(
                "RUNTIME_START_OR_SURFACE_REQUIRED",
                "content.runtimeDelivery.start.command",
                "runtime_delivery status=modified requires start.command or at least one runtimeSurfaces entry.",
            ));
        }
        require_runtime_non_empty_array(
            runtime,
            "/runtimeSurfaces",
            "content.runtimeDelivery.runtimeSurfaces",
            &mut issues,
        );
        require_runtime_string(
            runtime,
            "/httpProbes/previewPath",
            "content.runtimeDelivery.httpProbes.previewPath",
            &mut issues,
        );
        if runtime
            .pointer("/httpProbes/expectedStatus")
            .and_then(Value::as_str)
            != Some("2xx_or_3xx")
        {
            issues.push(issue(
                "RUNTIME_HTTP_PROBE_STATUS_INVALID",
                "content.runtimeDelivery.httpProbes.expectedStatus",
                "runtime_delivery httpProbes.expectedStatus must be 2xx_or_3xx.",
            ));
        }
        let guidance = runtime.get("taskPlanningGuidance").unwrap_or(&Value::Null);
        require_runtime_non_empty_array(
            guidance,
            "/requireRuntimeDeliveryRequirementWhenTaskTouches",
            "content.runtimeDelivery.taskPlanningGuidance.requireRuntimeDeliveryRequirementWhenTaskTouches",
            &mut issues,
        );
        if guidance.get("verificationBoundary").and_then(Value::as_str) != Some("code_level_only") {
            issues.push(issue(
                "RUNTIME_VERIFICATION_BOUNDARY_INVALID",
                "content.runtimeDelivery.taskPlanningGuidance.verificationBoundary",
                "runtime_delivery taskPlanningGuidance.verificationBoundary must be code_level_only.",
            ));
        }
        if guidance
            .get("doNotRequireCleanInstallOrContainerBuild")
            .and_then(Value::as_bool)
            != Some(true)
        {
            issues.push(issue(
                "RUNTIME_CLEAN_INSTALL_BOUNDARY_INVALID",
                "content.runtimeDelivery.taskPlanningGuidance.doNotRequireCleanInstallOrContainerBuild",
                "runtime_delivery must keep AAC verification at code level and not require clean install, container build, registry, or deploy success.",
            ));
        }
        if runtime
            .pointer("/frontend/required")
            .and_then(Value::as_bool)
            == Some(true)
        {
            require_runtime_string(
                runtime,
                "/frontend/outputDir",
                "content.runtimeDelivery.frontend.outputDir",
                &mut issues,
            );
            require_runtime_string(
                runtime,
                "/frontend/servedBy",
                "content.runtimeDelivery.frontend.servedBy",
                &mut issues,
            );
        }
        if runtime.pointer("/deliveryMechanics/codegen").is_some() {
            require_runtime_array(
                runtime,
                "/deliveryMechanics/codegen/codeLevelExpectations",
                "content.runtimeDelivery.deliveryMechanics.codegen.codeLevelExpectations",
                &mut issues,
            );
        }
    }
    issues
}

fn require_runtime_string(
    root: &Value,
    pointer: &str,
    field_path: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if root
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push(issue(
            "RUNTIME_FIELD_REQUIRED",
            field_path,
            "runtime_delivery status=modified requires this field for TaskPlan, Execution, Deploy, and Repair.",
        ));
    }
}

fn require_runtime_array(
    root: &Value,
    pointer: &str,
    field_path: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if root.pointer(pointer).and_then(Value::as_array).is_none() {
        issues.push(issue(
            "RUNTIME_FIELD_REQUIRED",
            field_path,
            "runtime_delivery status=modified requires this array field for code-level verification planning.",
        ));
    }
}

fn require_runtime_non_empty_array(
    root: &Value,
    pointer: &str,
    field_path: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if root
        .pointer(pointer)
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        issues.push(issue(
            "RUNTIME_FIELD_REQUIRED",
            field_path,
            "runtime_delivery status=modified requires at least one item here.",
        ));
    }
}

fn validate_coverage_section(
    content: &Value,
    allowed_refs: &Value,
) -> Vec<delivery_core::RepairIssue> {
    let detail_ids = string_set(allowed_refs.pointer("/requirementDetailIds"));
    let mut issues = Vec::new();
    let allowed_coverage_statuses = std::collections::BTreeSet::from([
        "covered",
        "partial",
        "not_applicable",
        "deferred",
        "uncovered",
    ]);
    let allowed_acceptance_priorities =
        std::collections::BTreeSet::from(["must", "should", "could"]);
    let allowed_coverage_types = std::collections::BTreeSet::from(COVERAGE_ARTIFACT_TYPES);
    let Some(detail_coverage) = content.get("detailCoverage").and_then(Value::as_array) else {
        issues.push(issue(
            "DETAIL_COVERAGE_REQUIRED",
            "content.detailCoverage",
            "coverage section must include detailCoverage.",
        ));
        return issues;
    };
    for (index, entry) in detail_coverage.iter().enumerate() {
        let detail_id = entry
            .get("detailId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !detail_ids.contains(detail_id) {
            issues.push(issue(
                "DETAIL_REF_INVALID",
                &format!("content.detailCoverage[{index}].detailId"),
                "detailCoverage.detailId must come from allowedRefs.requirementDetailIds.",
            ));
        }
        let coverage_status = entry
            .get("coverageStatus")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !allowed_coverage_statuses.contains(coverage_status) {
            issues.push(issue(
                "DETAIL_COVERAGE_INVALID",
                &format!("content.detailCoverage[{index}].coverageStatus"),
                "detailCoverage.coverageStatus must be one of covered, partial, not_applicable, deferred, or uncovered.",
            ));
        }
        let Some(artifact_refs) = entry.get("artifactRefs").and_then(Value::as_object) else {
            issues.push(issue(
                "DETAIL_COVERAGE_INVALID",
                &format!("content.detailCoverage[{index}].artifactRefs"),
                "detailCoverage.artifactRefs must be a JSON object.",
            ));
            continue;
        };
        let expected_keys = [
            "modules",
            "entities",
            "fields",
            "constraints",
            "interfaces",
            "userFlows",
            "stateMachines",
            "frontendDataViews",
            "frontendActions",
            "frontendOperationPaths",
            "acceptanceMatrix",
        ];
        let mut total_refs = 0usize;
        for key in expected_keys {
            match artifact_refs.get(key) {
                Some(Value::Array(values)) => {
                    total_refs += values.len();
                    validate_array_entries_are_strings(
                        values,
                        &format!("content.detailCoverage[{index}].artifactRefs.{key}"),
                        "DETAIL_COVERAGE_INVALID",
                        &mut issues,
                    );
                }
                _ => issues.push(issue(
                    "DETAIL_COVERAGE_INVALID",
                    &format!("content.detailCoverage[{index}].artifactRefs.{key}"),
                    "detailCoverage.artifactRefs fields must all be arrays.",
                )),
            }
        }
        let has_reason = entry
            .get("reason")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if coverage_status == "covered" && total_refs == 0 {
            issues.push(issue(
                "DETAIL_COVERAGE_INVALID",
                &format!("content.detailCoverage[{index}].artifactRefs"),
                "covered detailCoverage entries must reference at least one AAC artifact.",
            ));
        }
        if coverage_status != "covered" && !has_reason {
            issues.push(issue(
                "DETAIL_COVERAGE_INVALID",
                &format!("content.detailCoverage[{index}].reason"),
                "non-covered detailCoverage entries must explain the reason.",
            ));
        }
        validate_optional_string(
            entry.get("reason"),
            &format!("content.detailCoverage[{index}].reason"),
            "DETAIL_COVERAGE_INVALID",
            &mut issues,
        );
    }
    let Some(acceptance_matrix) = content.get("acceptanceMatrix").and_then(Value::as_array) else {
        issues.push(issue(
            "ACCEPTANCE_MATRIX_REQUIRED",
            "content.acceptanceMatrix",
            "coverage section must include acceptanceMatrix.",
        ));
        return issues;
    };
    let acceptance_refs = string_set(allowed_refs.pointer("/acceptanceRefs"));
    for (index, entry) in acceptance_matrix.iter().enumerate() {
        let acceptance_id = entry
            .get("acceptanceId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !acceptance_refs.contains(acceptance_id) {
            issues.push(issue(
                "INVALID_ACCEPTANCE_REF",
                &format!("content.acceptanceMatrix[{index}].acceptanceId"),
                "acceptanceMatrix.acceptanceId must come from allowedRefs.acceptanceRefs.",
            ));
        }
        let priority = entry
            .get("priority")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !allowed_acceptance_priorities.contains(priority) {
            issues.push(issue(
                "ACCEPTANCE_MATRIX_INVALID",
                &format!("content.acceptanceMatrix[{index}].priority"),
                "acceptanceMatrix.priority must be one of must, should, or could.",
            ));
        }
        let coverage_status = entry
            .get("coverageStatus")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !allowed_coverage_statuses.contains(coverage_status) {
            issues.push(issue(
                "ACCEPTANCE_MATRIX_INVALID",
                &format!("content.acceptanceMatrix[{index}].coverageStatus"),
                "acceptanceMatrix.coverageStatus must be one of covered, partial, not_applicable, deferred, or uncovered.",
            ));
        }
        if entry
            .get("statement")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            issues.push(issue(
                "ACCEPTANCE_MATRIX_INVALID",
                &format!("content.acceptanceMatrix[{index}].statement"),
                "acceptanceMatrix.statement is required and must preserve the acceptance statement.",
            ));
        }
        if entry.get("artifactRefs").is_some() {
            issues.push(issue(
                "ACCEPTANCE_MATRIX_INVALID",
                &format!("content.acceptanceMatrix[{index}].artifactRefs"),
                "acceptanceMatrix must use coverage[] entries, not artifactRefs.",
            ));
        }
        let Some(coverage) = entry.get("coverage").and_then(Value::as_array) else {
            issues.push(issue(
                "ACCEPTANCE_MATRIX_INVALID",
                &format!("content.acceptanceMatrix[{index}].coverage"),
                "acceptanceMatrix.coverage must be an array of artifact coverage entries.",
            ));
            continue;
        };
        for (coverage_index, coverage_entry) in coverage.iter().enumerate() {
            let coverage_type = coverage_entry
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if coverage_type.is_empty() {
                issues.push(issue(
                    "ACCEPTANCE_MATRIX_INVALID",
                    &format!("content.acceptanceMatrix[{index}].coverage[{coverage_index}].type"),
                    "acceptanceMatrix.coverage[].type is required.",
                ));
            } else if !allowed_coverage_types.contains(coverage_type) {
                issues.push(issue(
                    "ACCEPTANCE_MATRIX_INVALID",
                    &format!("content.acceptanceMatrix[{index}].coverage[{coverage_index}].type"),
                    "acceptanceMatrix.coverage[].type must be one of currentSectionContract.enumRefs.coverageArtifactType.",
                ));
            }
            if !coverage_entry
                .get("refs")
                .map(Value::is_array)
                .unwrap_or(false)
            {
                issues.push(issue(
                    "ACCEPTANCE_MATRIX_INVALID",
                    &format!("content.acceptanceMatrix[{index}].coverage[{coverage_index}].refs"),
                    "acceptanceMatrix.coverage[].refs must be an array.",
                ));
            } else if let Some(values) = coverage_entry.get("refs").and_then(Value::as_array) {
                validate_array_entries_are_strings(
                    values,
                    &format!("content.acceptanceMatrix[{index}].coverage[{coverage_index}].refs"),
                    "ACCEPTANCE_MATRIX_INVALID",
                    &mut issues,
                );
            }
            if coverage_entry
                .get("description")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                issues.push(issue(
                    "ACCEPTANCE_MATRIX_INVALID",
                    &format!(
                        "content.acceptanceMatrix[{index}].coverage[{coverage_index}].description"
                    ),
                    "acceptanceMatrix.coverage[].description is required.",
                ));
            }
        }
        match entry.get("verificationHints") {
            Some(Value::Array(hints)) => {
                for (hint_index, hint) in hints.iter().enumerate() {
                    let Some(hint_object) = hint.as_object() else {
                        issues.push(issue(
                            "ACCEPTANCE_MATRIX_INVALID",
                            &format!(
                                "content.acceptanceMatrix[{index}].verificationHints[{hint_index}]"
                            ),
                            "acceptanceMatrix.verificationHints[] must be objects with kind and description.",
                        ));
                        continue;
                    };
                    if hint_object
                        .get("kind")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default()
                        .is_empty()
                    {
                        issues.push(issue(
                            "ACCEPTANCE_MATRIX_INVALID",
                            &format!(
                                "content.acceptanceMatrix[{index}].verificationHints[{hint_index}].kind"
                            ),
                            "acceptanceMatrix.verificationHints[].kind is required.",
                        ));
                    }
                    if hint_object
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .unwrap_or_default()
                        .is_empty()
                    {
                        issues.push(issue(
                            "ACCEPTANCE_MATRIX_INVALID",
                            &format!(
                                "content.acceptanceMatrix[{index}].verificationHints[{hint_index}].description"
                            ),
                            "acceptanceMatrix.verificationHints[].description is required.",
                        ));
                    }
                }
            }
            Some(_) => issues.push(issue(
                "ACCEPTANCE_MATRIX_INVALID",
                &format!("content.acceptanceMatrix[{index}].verificationHints"),
                "acceptanceMatrix.verificationHints must be an array.",
            )),
            None => {}
        }
        let has_reason = entry
            .get("reason")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if coverage_status != "covered" && !has_reason {
            issues.push(issue(
                "ACCEPTANCE_MATRIX_INVALID",
                &format!("content.acceptanceMatrix[{index}].reason"),
                "non-covered acceptanceMatrix entries must explain the reason.",
            ));
        }
        validate_optional_string(
            entry.get("reason"),
            &format!("content.acceptanceMatrix[{index}].reason"),
            "ACCEPTANCE_MATRIX_INVALID",
            &mut issues,
        );
    }
    validate_coverage_handoff(content.get("handoff"), &mut issues);
    validate_architecture_quality(
        content.get("architectureQuality"),
        allowed_refs,
        &mut issues,
    );
    issues
}

fn normalize_coverage_deferred_reasons(
    project_root: &Path,
    source_refs: &Value,
    content: &mut Value,
) -> Result<bool, state::store::StateError> {
    let Some(planning_ref) = source_refs
        .get("planningContractRef")
        .and_then(Value::as_str)
    else {
        return Ok(false);
    };
    let planning: contracts::PlanningGenerationContract =
        read_project_json(project_root, planning_ref)?;
    let reason_by_detail = planning
        .requirement_details
        .items
        .iter()
        .filter(|detail| {
            !detail.required_for_current_phase
                || detail.kind == "deferred_or_excluded_boundary"
                || detail.lifecycle_stage == "deferred"
        })
        .map(|detail| {
            (
                detail.detail_id.clone(),
                format!(
                    "Deferred by the confirmed phase boundary: {}",
                    detail.summary
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let Some(rows) = content
        .get_mut("detailCoverage")
        .and_then(Value::as_array_mut)
    else {
        return Ok(false);
    };
    let mut normalized = false;
    for row in rows {
        if row.get("coverageStatus").and_then(Value::as_str) != Some("deferred") {
            continue;
        }
        let has_reason = row
            .get("reason")
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if has_reason {
            continue;
        }
        let Some(detail_id) = row.get("detailId").and_then(Value::as_str) else {
            continue;
        };
        let Some(reason) = reason_by_detail.get(detail_id) else {
            continue;
        };
        if let Some(object) = row.as_object_mut() {
            object.insert("reason".to_string(), Value::String(reason.clone()));
            normalized = true;
        }
    }
    Ok(normalized)
}

fn validate_architecture_quality(
    value: Option<&Value>,
    allowed_refs: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(value) = value else {
        issues.push(issue(
            "ARCHITECTURE_QUALITY_REQUIRED",
            "content.architectureQuality",
            "coverage section must include architectureQuality.",
        ));
        return;
    };
    let Ok(model) = serde_json::from_value::<ArchitectureQuality>(value.clone()) else {
        issues.push(issue(
            "ARCHITECTURE_QUALITY_INVALID",
            "content.architectureQuality",
            "architectureQuality must follow the declared decisions, nfrs, and risks object shape.",
        ));
        return;
    };
    if model.decisions.is_empty() {
        issues.push(issue(
            "ARCHITECTURE_QUALITY_INCOMPLETE",
            "content.architectureQuality.decisions",
            "architectureQuality.decisions must include at least one current-phase architecture decision.",
        ));
    }
    if model.nfrs.is_empty() {
        issues.push(issue(
            "ARCHITECTURE_QUALITY_INCOMPLETE",
            "content.architectureQuality.nfrs",
            "architectureQuality.nfrs must include at least one current-phase non-functional requirement.",
        ));
    }
    if model.risks.is_empty() {
        issues.push(issue(
            "ARCHITECTURE_QUALITY_INCOMPLETE",
            "content.architectureQuality.risks",
            "architectureQuality.risks must include at least one current-phase architecture risk or failure mode.",
        ));
    }
    let allowed_decision_categories = BTreeSet::from([
        "architecture_style",
        "module_boundary",
        "data_boundary",
        "integration_boundary",
        "runtime_boundary",
        "security_boundary",
        "operability",
    ]);
    let allowed_nfr_categories = BTreeSet::from([
        "performance",
        "reliability",
        "security",
        "maintainability",
        "observability",
        "cost",
    ]);
    let allowed_risk_categories = BTreeSet::from([
        "data_integrity",
        "integration",
        "runtime",
        "security",
        "operability",
        "maintainability",
    ]);
    let allowed_statuses = BTreeSet::from(["accepted", "needs_user_decision"]);
    let allowed_severities = BTreeSet::from(["low", "medium", "high", "critical"]);
    let allowed_likelihoods = BTreeSet::from(["low", "medium", "high"]);
    let allowed_scope_refs = string_set(allowed_refs.pointer("/scopeRefs"));
    let allowed_acceptance_refs = string_set(allowed_refs.pointer("/acceptanceRefs"));
    let allowed_detail_refs = string_set(allowed_refs.pointer("/requirementDetailIds"));
    let decision_ids = model
        .decisions
        .iter()
        .map(|decision| decision.decision_id.clone())
        .collect::<BTreeSet<_>>();
    let nfr_ids = model
        .nfrs
        .iter()
        .map(|nfr| nfr.nfr_id.clone())
        .collect::<BTreeSet<_>>();
    let risk_ids = model
        .risks
        .iter()
        .map(|risk| risk.risk_id.clone())
        .collect::<BTreeSet<_>>();
    for (index, decision) in model.decisions.iter().enumerate() {
        validate_non_empty(
            &decision.decision_id,
            &format!("content.architectureQuality.decisions[{index}].decisionId"),
            issues,
        );
        validate_non_empty(
            &decision.title,
            &format!("content.architectureQuality.decisions[{index}].title"),
            issues,
        );
        validate_non_empty(
            &decision.context,
            &format!("content.architectureQuality.decisions[{index}].context"),
            issues,
        );
        validate_non_empty(
            &decision.decision,
            &format!("content.architectureQuality.decisions[{index}].decision"),
            issues,
        );
        if !allowed_decision_categories.contains(decision.category.as_str()) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.decisions[{index}].category"),
                "decision category must come from enumRefs.architectureQuality.decisionCategory.",
            ));
        }
        if !allowed_statuses.contains(decision.status.as_str()) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.decisions[{index}].status"),
                "decision status must be accepted or needs_user_decision.",
            ));
        }
        if decision.alternatives_considered.is_empty() {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INCOMPLETE",
                &format!("content.architectureQuality.decisions[{index}].alternativesConsidered"),
                "architecture decisions must include at least one alternative with a rejectedBecause reason.",
            ));
        }
        if decision.verification_hints.is_empty() {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INCOMPLETE",
                &format!("content.architectureQuality.decisions[{index}].verificationHints"),
                "architecture decisions must include verificationHints for downstream tasks or review.",
            ));
        }
        validate_ref_members(
            &decision.source_refs.scope_refs,
            &allowed_scope_refs,
            &format!("content.architectureQuality.decisions[{index}].sourceRefs.scopeRefs"),
            issues,
        );
        validate_ref_members(
            &decision.source_refs.acceptance_refs,
            &allowed_acceptance_refs,
            &format!("content.architectureQuality.decisions[{index}].sourceRefs.acceptanceRefs"),
            issues,
        );
        validate_ref_members(
            &decision.source_refs.requirement_detail_refs,
            &allowed_detail_refs,
            &format!(
                "content.architectureQuality.decisions[{index}].sourceRefs.requirementDetailRefs"
            ),
            issues,
        );
    }
    for (index, nfr) in model.nfrs.iter().enumerate() {
        validate_non_empty(
            &nfr.nfr_id,
            &format!("content.architectureQuality.nfrs[{index}].nfrId"),
            issues,
        );
        validate_non_empty(
            &nfr.target,
            &format!("content.architectureQuality.nfrs[{index}].target"),
            issues,
        );
        validate_non_empty(
            &nfr.rationale,
            &format!("content.architectureQuality.nfrs[{index}].rationale"),
            issues,
        );
        validate_non_empty(
            &nfr.verification_strategy,
            &format!("content.architectureQuality.nfrs[{index}].verificationStrategy"),
            issues,
        );
        if !allowed_nfr_categories.contains(nfr.category.as_str()) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.nfrs[{index}].category"),
                "nfr category must come from enumRefs.architectureQuality.nfrCategory.",
            ));
        }
        validate_ref_members(
            &nfr.architecture_refs.decisions,
            &decision_ids,
            &format!("content.architectureQuality.nfrs[{index}].architectureRefs.decisions"),
            issues,
        );
        validate_ref_members(
            &nfr.architecture_refs.risks,
            &risk_ids,
            &format!("content.architectureQuality.nfrs[{index}].architectureRefs.risks"),
            issues,
        );
    }
    for (index, risk) in model.risks.iter().enumerate() {
        validate_non_empty(
            &risk.risk_id,
            &format!("content.architectureQuality.risks[{index}].riskId"),
            issues,
        );
        validate_non_empty(
            &risk.impact,
            &format!("content.architectureQuality.risks[{index}].impact"),
            issues,
        );
        validate_non_empty(
            &risk.mitigation,
            &format!("content.architectureQuality.risks[{index}].mitigation"),
            issues,
        );
        if !allowed_risk_categories.contains(risk.category.as_str()) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.risks[{index}].category"),
                "risk category must come from enumRefs.architectureQuality.riskCategory.",
            ));
        }
        if !allowed_severities.contains(risk.severity.as_str()) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.risks[{index}].severity"),
                "risk severity must be low, medium, high, or critical.",
            ));
        }
        if !allowed_likelihoods.contains(risk.likelihood.as_str()) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.risks[{index}].likelihood"),
                "risk likelihood must be low, medium, or high.",
            ));
        }
        validate_ref_members(
            &risk.owner_artifact_refs.decisions,
            &decision_ids,
            &format!("content.architectureQuality.risks[{index}].ownerArtifactRefs.decisions"),
            issues,
        );
        validate_ref_members(
            &risk.owner_artifact_refs.nfrs,
            &nfr_ids,
            &format!("content.architectureQuality.risks[{index}].ownerArtifactRefs.nfrs"),
            issues,
        );
        if risk.verification_hints.is_empty() {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INCOMPLETE",
                &format!("content.architectureQuality.risks[{index}].verificationHints"),
                "architecture risks must include verificationHints for downstream tasks or review.",
            ));
        }
    }
}

fn validate_non_empty(value: &str, field_path: &str, issues: &mut Vec<delivery_core::RepairIssue>) {
    if value.trim().is_empty() {
        issues.push(issue(
            "ARCHITECTURE_QUALITY_INCOMPLETE",
            field_path,
            "architectureQuality string fields must be non-empty.",
        ));
    }
}

fn validate_ref_members(
    values: &[String],
    allowed: &BTreeSet<String>,
    field_path: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    for (index, value) in values.iter().enumerate() {
        if !allowed.contains(value) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_REF_INVALID",
                &format!("{field_path}[{index}]"),
                "architectureQuality refs must come from the current AAC or allowedRefs.",
            ));
        }
    }
}

fn validate_array_entries_are_strings(
    values: &[Value],
    field_path: &str,
    code: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    for (index, value) in values.iter().enumerate() {
        if !value.is_string() {
            issues.push(issue(
                code,
                &format!("{field_path}[{index}]"),
                "array entries must be strings.",
            ));
        }
    }
}

fn validate_optional_string(
    value: Option<&Value>,
    field_path: &str,
    code: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    if let Some(value) = value {
        if !(value.is_null() || value.is_string()) {
            issues.push(issue(
                code,
                field_path,
                "field must be a string when present.",
            ));
        }
    }
}

fn validate_coverage_handoff(value: Option<&Value>, issues: &mut Vec<delivery_core::RepairIssue>) {
    let Some(value) = value else {
        return;
    };
    let Some(object) = value.as_object() else {
        issues.push(issue(
            "COVERAGE_HANDOFF_INVALID",
            "content.handoff",
            "coverage handoff must be an object.",
        ));
        return;
    };
    if !object
        .get("readyForTaskPlan")
        .map(Value::is_boolean)
        .unwrap_or(false)
    {
        issues.push(issue(
            "COVERAGE_HANDOFF_INVALID",
            "content.handoff.readyForTaskPlan",
            "coverage handoff.readyForTaskPlan must be a boolean.",
        ));
    }
    match object.get("blockingReasons") {
        Some(Value::Array(values)) => validate_array_entries_are_strings(
            values,
            "content.handoff.blockingReasons",
            "COVERAGE_HANDOFF_INVALID",
            issues,
        ),
        Some(_) => issues.push(issue(
            "COVERAGE_HANDOFF_INVALID",
            "content.handoff.blockingReasons",
            "coverage handoff.blockingReasons must be an array.",
        )),
        None => issues.push(issue(
            "COVERAGE_HANDOFF_INVALID",
            "content.handoff.blockingReasons",
            "coverage handoff.blockingReasons is required.",
        )),
    }
    if object
        .get("nextNode")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .is_empty()
    {
        issues.push(issue(
            "COVERAGE_HANDOFF_INVALID",
            "content.handoff.nextNode",
            "coverage handoff.nextNode is required.",
        ));
    }
}

fn read_source_refs(
    project_root: &str,
    request_ref: &str,
    request_root: &Value,
) -> Result<Value, state::store::StateError> {
    let fields = read_plan_fields(request_root)
        .into_iter()
        .filter(|field| field.starts_with("sourceRefs."))
        .collect::<Vec<_>>();
    if fields.is_empty() {
        return Ok(request_root
            .get("sourceRefs")
            .cloned()
            .unwrap_or_else(|| json!({})));
    }
    let resolved = state::read_request_fields(ReadRequestFieldsInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
        fields,
    })?;
    let mut source_refs = serde_json::Map::new();
    for (field, result) in resolved.fields {
        if let Some(key) = field.strip_prefix("sourceRefs.") {
            if !result.value.is_null() {
                source_refs.insert(key.to_string(), result.value);
            }
        }
    }
    Ok(Value::Object(source_refs))
}

fn read_plan_fields(request_root: &Value) -> Vec<String> {
    request_root
        .pointer("/requestReadPlan/groups")
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .flat_map(|group| {
                    serde_json::from_value::<Vec<delivery_core::ReadSelector>>(
                        group
                            .get("selectors")
                            .cloned()
                            .unwrap_or_else(|| Value::Array(vec![])),
                    )
                    .map(|selectors| delivery_core::expand_read_selectors(&selectors))
                    .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn load_request_root(
    project_root: &str,
    request_id: &str,
) -> Result<Value, state::store::StateError> {
    let index = get_request_index_entry(project_root, request_id)?;
    let paths = state::paths::project_paths(project_root)?;
    let request_file = state::paths::from_project_relative(&paths.root, &index.request_file)?;
    state::store::read_json_value(&request_file)
}

fn save_request_root(
    project_root: &str,
    request_id: &str,
    root: &Value,
) -> Result<(), state::store::StateError> {
    let index = get_request_index_entry(project_root, request_id)?;
    let paths = state::paths::project_paths(project_root)?;
    let request_file = state::paths::from_project_relative(&paths.root, &index.request_file)?;
    state::store::write_json_atomic(&request_file, root)
}

fn parse_section(
    root: &Value,
    pointer: &str,
) -> Result<ArchitectureSectionGroup, state::store::StateError> {
    serde_json::from_value(root.pointer(pointer).cloned().ok_or_else(|| {
        state::store::StateError::StateCorrupted(format!("request is missing {pointer}"))
    })?)
    .map_err(state::store::StateError::Json)
}

fn parse_section_outputs(
    project_root: &str,
    request_id: &str,
    root: &Value,
) -> Result<Vec<SectionStateOutput>, state::store::StateError> {
    let value = if let Some(value) = root.get("sectionOutputs").cloned() {
        value
    } else {
        let paths = state::paths::project_paths(project_root)?;
        let relative = state::request_manifest::request_storage_ref(
            &paths.root,
            request_id,
            "sectionOutputs",
        )?
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "request {request_id} is missing private sectionOutputs storage"
            ))
        })?;
        let ref_file = state::paths::from_project_relative(&paths.root, &relative)?;
        state::store::read_json_value(&ref_file)?
    };
    serde_json::from_value(value).map_err(state::store::StateError::Json)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SectionStateOutput {
    section: ArchitectureSectionGroup,
    candidate_file: String,
    schema_ref: String,
    schema_shape: Value,
    result_template: Value,
    enum_refs: Value,
    generation_rules: Vec<String>,
}

fn next_section(section: ArchitectureSectionGroup) -> Option<ArchitectureSectionGroup> {
    let order = section_order();
    let index = order.iter().position(|item| *item == section)?;
    order.get(index + 1).copied()
}

fn update_request_for_next_section(
    mut root: Value,
    next_section: ArchitectureSectionGroup,
    next_output: &SectionStateOutput,
    include_repair_context: bool,
    include_repair_source_ref: bool,
    source_refs: &Value,
) -> Result<Value, state::store::StateError> {
    let completed_section = parse_section(&root, "/sectionState/currentSection")?;
    root["sectionState"]["currentSection"] =
        serde_json::to_value(next_section).map_err(state::store::StateError::Json)?;
    let completed = root["sectionState"]["completedSections"]
        .as_array_mut()
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "request sectionState.completedSections is invalid".to_string(),
            )
        })?;
    completed
        .push(serde_json::to_value(completed_section).map_err(state::store::StateError::Json)?);
    root["currentSectionContract"] =
        serde_json::to_value(next_output).map_err(state::store::StateError::Json)?;
    let write_targets = json!([{
        "targetId": section_name(next_section),
        "path": next_output.candidate_file.clone(),
        "required": true,
        "description": format!("Write the {} Architecture section candidate JSON.", section_name(next_section))
    }]);
    root["writeTargets"] = write_targets.clone();
    if root.get("outputContract").is_some() {
        root["outputContract"]["writeTargets"] = write_targets;
        root["outputContract"]["schemaShape"] = next_output.schema_shape.clone();
        root["outputContract"]["schemaProjection"]["requiredContentKeys"] =
            serde_json::to_value(crate::request::required_content_keys(next_section))
                .map_err(state::store::StateError::Json)?;
    }
    let frontend_experience_source = root
        .get("frontendExperienceSource")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let api_quality_seed = root.get("apiQualitySeed").cloned().unwrap_or(Value::Null);
    root["requestReadPlan"]["groups"] = architecture_read_groups(
        next_section,
        include_repair_context,
        include_repair_source_ref,
        source_refs,
        &frontend_experience_source,
        &api_quality_seed,
    );
    Ok(root)
}

fn repair_context_has_source_ref(
    project_root: &str,
    request_ref: &str,
    request_root: &Value,
) -> Result<bool, state::store::StateError> {
    if !read_plan_fields(request_root)
        .iter()
        .any(|field| field == "repairContext.sourceRef")
    {
        return Ok(false);
    }
    let fields = state::read_request_fields(ReadRequestFieldsInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
        fields: vec!["repairContext.sourceRef".to_string()],
    })?
    .fields;
    Ok(fields
        .get("repairContext.sourceRef")
        .is_some_and(|field| !field.value.is_null()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{
        build_ui_quality_seed, ui_surface_decision_candidate_template,
        validate_ui_surface_decision_contract,
    };

    #[test]
    fn frontend_submit_normalization_writes_surface_decision_contract() {
        let ui_quality_seed = build_ui_quality_seed(None, None);
        let mut candidate = ui_surface_decision_candidate_template();
        candidate["patternRankings"][0]["score"] = json!(0.8);
        candidate["patternRankings"][0]["matchedSignals"] =
            json!(["record collection", "business action"]);
        candidate["selectedPattern"]["rationale"] =
            json!("The UI surface is a record workbench with scan, compare, and create actions.");
        candidate["semanticFacts"]["userJobs"] = json!(["browse", "compare", "create"]);
        candidate["semanticFacts"]["informationShapes"] =
            json!(["record_collection", "record_detail"]);
        candidate["semanticFacts"]["operationModels"] =
            json!(["filter_sort_paginate", "create_update"]);
        candidate["semanticFacts"]["riskFactors"] = json!(["none"]);
        candidate["layoutModel"]["desktop"]["layoutIntent"] =
            json!("Keep the working record region and primary action visible together.");
        candidate["regionModel"][0]["purpose"] =
            json!("Primary record work region for scanning and acting on business items.");
        candidate["informationModel"]["primaryObjects"] = json!(["request"]);
        candidate["informationModel"]["fields"] = json!(["id", "status"]);
        candidate["actionModel"][0]["label"] = json!("Create request");
        candidate["stateModel"][0]["placementRule"] =
            json!("Place loading and errors near the affected record work region.");

        let mut content = json!({
            "frontendExperience": {
                "required": true,
                "surfaceDecisionCandidate": candidate
            }
        });
        let request_root = json!({
            "uiQualitySeed": ui_quality_seed
        });

        assert!(
            normalize_frontend_ui_surface_decision_contract(&mut content, &request_root),
            "submit normalization must write derived frontend quality fields"
        );
        let frontend = content
            .get("frontendExperience")
            .expect("frontendExperience must remain present");
        assert!(
            frontend.get("uiSurfaceDecisionContract").is_some(),
            "submit normalization must derive uiSurfaceDecisionContract"
        );
        assert_eq!(
            frontend
                .pointer("/uiSurfaceDecisionContract/patternDecision/knownPattern")
                .and_then(Value::as_str),
            Some("collection_workbench"),
            "uiSurfaceDecisionContract pattern must be derived from surfaceDecisionCandidate"
        );
        assert!(
            frontend.get("uiQualityContract").is_none(),
            "submit normalization must not persist legacy uiQualityContract"
        );
        let issues = validate_ui_surface_decision_contract(frontend);
        assert!(
            issues.is_empty(),
            "derived surface decision contract should validate cleanly: {issues:?}"
        );
    }
}

fn update_output_contract_ref(
    project_root: &str,
    request_id: &str,
    next_section: ArchitectureSectionGroup,
    next_output: &SectionStateOutput,
) -> Result<(), state::store::StateError> {
    let output_contract_file = output_contract_ref_file(project_root, request_id)?;
    let mut output_contract = state::store::read_json_value(&output_contract_file)?;
    output_contract["writeTargets"] = json!([{
        "targetId": section_name(next_section),
        "path": next_output.candidate_file.clone(),
        "required": true,
        "description": format!("Write the {} Architecture section candidate JSON.", section_name(next_section))
    }]);
    output_contract["schemaShape"] = next_output.schema_shape.clone();
    output_contract["schemaProjection"]["requiredContentKeys"] =
        serde_json::to_value(crate::request::required_content_keys(next_section))
            .map_err(state::store::StateError::Json)?;
    state::store::write_json_atomic(&output_contract_file, &output_contract)
}

fn output_contract_ref_file(
    project_root: &str,
    request_id: &str,
) -> Result<std::path::PathBuf, state::store::StateError> {
    let paths = state::paths::project_paths(project_root)?;
    let manifest_file = state::paths::request_storage_manifest_file(&paths.root, request_id);
    let manifest = state::store::read_json_value(&manifest_file)?;
    let relative = manifest
        .pointer("/refs/outputContract/ref")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "request {} is missing outputContract storage ref",
                request_id
            ))
        })?;
    state::paths::from_project_relative(&paths.root, relative)
}

fn assemble_architecture_contract(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    outputs: &[SectionStateOutput],
    source_refs: &Value,
) -> Result<ArchitectureArtifactContract, state::store::StateError> {
    let root = Path::new(project_root);
    let mut by_section = std::collections::BTreeMap::new();
    for output in outputs {
        let absolute = from_project_relative(root, &output.candidate_file)?;
        let candidate: ArchitectureSectionCandidateAgentWritable =
            state::store::read_json(&absolute)?;
        by_section.insert(output.section, candidate);
    }
    let foundation = by_section
        .get(&ArchitectureSectionGroup::Foundation)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted("missing foundation section".to_string())
        })?;
    let domain = by_section
        .get(&ArchitectureSectionGroup::DomainContract)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted("missing domain_contract section".to_string())
        })?;
    let behavior = by_section
        .get(&ArchitectureSectionGroup::Behavior)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted("missing behavior section".to_string())
        })?;
    let frontend = by_section
        .get(&ArchitectureSectionGroup::FrontendExperience)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "missing frontend_experience section".to_string(),
            )
        })?;
    let runtime = by_section
        .get(&ArchitectureSectionGroup::RuntimeDelivery)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted("missing runtime_delivery section".to_string())
        })?;
    let coverage = by_section
        .get(&ArchitectureSectionGroup::Coverage)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted("missing coverage section".to_string())
        })?;
    let source = ArchitectureArtifactSource {
        planning_generation_contract_id: foundation
            .content
            .pointer("/source/planningGenerationContractId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        technical_baseline_id: foundation
            .content
            .pointer("/source/technicalBaselineId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        brainstorm_contract_ref: source_refs
            .get("brainstormContractRef")
            .and_then(Value::as_str)
            .map(str::to_string),
        repository_context_ref: source_refs
            .get("repositoryContextRef")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    let acceptance_matrix: Vec<AcceptanceMatrixEntry> = serde_json::from_value(
        coverage
            .content
            .get("acceptanceMatrix")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .map_err(state::store::StateError::Json)?;
    let detail_coverage: Vec<ArchitectureDetailCoverageEntry> = serde_json::from_value(
        coverage
            .content
            .get("detailCoverage")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .map_err(state::store::StateError::Json)?;
    let handoff = parse_handoff(coverage.content.get("handoff").cloned())?;
    let architecture_quality: ArchitectureQuality = serde_json::from_value(
        coverage
            .content
            .get("architectureQuality")
            .cloned()
            .unwrap_or_else(|| json!({ "decisions": [], "nfrs": [], "risks": [] })),
    )
    .map_err(state::store::StateError::Json)?;
    let mut interfaces = domain
        .content
        .get("interfaces")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    normalize_http_interface_paths(&mut interfaces);
    let mut runtime_delivery = runtime.content.get("runtimeDelivery").cloned();
    normalize_runtime_api_probe_paths(&mut runtime_delivery, &interfaces);
    let api_contract = normalize_api_contract(
        domain.content.get("apiContract"),
        &interfaces,
        runtime_delivery.as_ref(),
    );
    Ok(ArchitectureArtifactContract {
        schema_version: "1.0".to_string(),
        architecture_artifact_contract_id: format!(
            "aac_{}_{}",
            phase_id,
            state::store::now_millis()
        ),
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
        status: if needs_user_decision(&coverage.content) {
            ArchitectureArtifactStatus::NeedsUserDecision
        } else {
            ArchitectureArtifactStatus::Ready
        },
        source,
        engineering_boundary: foundation
            .content
            .get("engineeringBoundary")
            .cloned()
            .unwrap_or_else(|| json!({})),
        modules: foundation
            .content
            .get("modules")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        data_model: domain
            .content
            .get("dataModel")
            .cloned()
            .unwrap_or_else(|| json!({})),
        interfaces,
        api_contract,
        user_flows: behavior
            .content
            .get("userFlows")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        state_machines: behavior
            .content
            .get("stateMachines")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        frontend_experience: frontend.content.get("frontendExperience").cloned(),
        runtime_delivery,
        acceptance_matrix,
        detail_coverage,
        architecture_quality,
        handoff,
        created_at: state::store::now_string(),
        updated_at: state::store::now_string(),
    })
}

fn normalize_api_contract(
    candidate: Option<&Value>,
    interfaces: &[Value],
    runtime_delivery: Option<&Value>,
) -> Option<Value> {
    let has_http_interface = interfaces
        .iter()
        .any(|interface| interface.get("type").and_then(Value::as_str) == Some("http_api"));
    if !has_http_interface {
        return None;
    }
    let candidate_base = candidate
        .and_then(|value| value.pointer("/publicExposure/basePath"))
        .and_then(Value::as_str);
    let runtime_base = runtime_delivery
        .and_then(|value| value.pointer("/api/basePath"))
        .and_then(Value::as_str);
    let base_path = candidate_base
        .or(runtime_base)
        .map(normalize_api_path)
        .filter(|value| value != "/")
        .or_else(|| {
            interfaces
                .iter()
                .filter(|interface| {
                    interface.get("type").and_then(Value::as_str) == Some("http_api")
                })
                .filter_map(|interface| interface.get("path").and_then(Value::as_str))
                .find_map(first_api_path_segment)
        })
        .unwrap_or_else(|| "/api".to_string());
    let preserve_path = candidate
        .and_then(|value| value.pointer("/publicExposure/preservePath"))
        .and_then(Value::as_bool)
        .or_else(|| {
            runtime_delivery
                .and_then(|value| value.pointer("/api/preservePath"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(true);
    let browser_mode = candidate
        .and_then(|value| value.pointer("/browserBinding/mode"))
        .and_then(Value::as_str)
        .or_else(|| {
            runtime_delivery
                .and_then(|value| value.pointer("/api/browserBinding/mode"))
                .and_then(Value::as_str)
        })
        .unwrap_or("same_origin");
    let browser_base_url = candidate
        .and_then(|value| value.pointer("/browserBinding/baseUrl"))
        .and_then(Value::as_str)
        .or_else(|| {
            runtime_delivery
                .and_then(|value| value.pointer("/api/browserBinding/baseUrl"))
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    Some(json!({
        "publicExposure": {
            "basePath": base_path,
            "preservePath": preserve_path
        },
        "browserBinding": {
            "mode": browser_mode,
            "baseUrl": browser_base_url,
            "pathOwnership": "interface_path"
        }
    }))
}

fn normalize_http_interface_paths(interfaces: &mut [Value]) {
    for interface in interfaces {
        if interface.get("type").and_then(Value::as_str) != Some("http_api") {
            continue;
        }
        let Some(path) = interface.get("path").and_then(Value::as_str) else {
            continue;
        };
        let normalized = normalize_api_path(path);
        if let Some(object) = interface.as_object_mut() {
            object.insert("path".to_string(), Value::String(normalized));
        }
    }
}

fn normalize_runtime_api_probe_paths(runtime_delivery: &mut Option<Value>, interfaces: &[Value]) {
    let api_paths = interfaces
        .iter()
        .filter(|interface| interface.get("type").and_then(Value::as_str) == Some("http_api"))
        .filter_map(|interface| interface.get("path").and_then(Value::as_str))
        .map(normalize_api_path)
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    if api_paths.is_empty() {
        return;
    }
    let Some(runtime) = runtime_delivery.as_mut().and_then(Value::as_object_mut) else {
        return;
    };
    if let Some(api) = runtime.get_mut("api").and_then(Value::as_object_mut) {
        api.remove("probePaths");
    }
    let probes = runtime
        .entry("httpProbes")
        .or_insert_with(|| json!({}))
        .as_object_mut();
    let Some(probes) = probes else {
        return;
    };
    probes.insert("apiPaths".to_string(), json!(api_paths));
}

fn normalize_api_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        return "/".to_string();
    }
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    if path.len() == 1 {
        path
    } else {
        path.trim_end_matches('/').to_string()
    }
}

fn first_api_path_segment(path: &str) -> Option<String> {
    path.split('/')
        .find(|segment| !segment.is_empty() && !segment.starts_with('{'))
        .map(|segment| format!("/{segment}"))
}

fn parse_handoff(value: Option<Value>) -> Result<ArchitectureHandoff, state::store::StateError> {
    let Some(value) = value else {
        return Ok(ArchitectureHandoff {
            ready_for_task_plan: true,
            blocking_reasons: vec![],
            next_node: "task_plan".to_string(),
        });
    };
    serde_json::from_value(value).map_err(state::store::StateError::Json)
}

fn read_project_json<T: serde::de::DeserializeOwned>(
    project_root: &Path,
    relative: &str,
) -> Result<T, state::store::StateError> {
    let path = from_project_relative(project_root, relative)?;
    state::store::read_json(&path)
}

fn needs_user_decision(coverage_content: &Value) -> bool {
    coverage_content
        .pointer("/architectureQuality/decisions")
        .and_then(Value::as_array)
        .map(|decisions| {
            decisions.iter().any(|decision| {
                decision.get("status").and_then(Value::as_str) == Some("needs_user_decision")
            })
        })
        .unwrap_or(false)
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default()
}

fn visit_refs<F>(value: &Value, path: &str, callback: &mut F)
where
    F: FnMut(&str, &str, &Value),
{
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                callback(&child_path, key, child);
                visit_refs(child, &child_path, callback);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                visit_refs(child, &child_path, callback);
            }
        }
        _ => {}
    }
}

fn validate_string_array(
    value: &Value,
    allowed: &BTreeSet<String>,
    path: &str,
    code: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(items) = value.as_array() else {
        issues.push(issue(code, path, "reference list must be an array."));
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let Some(text) = item.as_str() else {
            issues.push(issue(
                code,
                &format!("{path}[{index}]"),
                "reference value must be a string.",
            ));
            continue;
        };
        if !allowed.contains(text) {
            issues.push(issue(
                code,
                &format!("{path}[{index}]"),
                "reference value is not allowed in this request.",
            ));
        }
    }
}

fn validate_string_value(
    value: &Value,
    allowed: &BTreeSet<String>,
    path: &str,
    code: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(text) = value.as_str() else {
        issues.push(issue(code, path, "reference value must be a string."));
        return;
    };
    if !allowed.contains(text) {
        issues.push(issue(
            code,
            path,
            "reference value is not allowed in this request.",
        ));
    }
}

fn repairable(
    input: &FileSubmitInput,
    authorized: &AuthorizedWriteSet,
    target_file: String,
    issues: Vec<delivery_core::RepairIssue>,
    mode: ArchitectureSubmitMode,
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
        resubmit_tool: mode.resubmit_tool().to_string(),
        fix_scope: Some(mode.fix_scope().to_string()),
        read_groups: authorized.read_groups.clone(),
    })
}

fn ensure_latest_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_ref: &str,
    mode: ArchitectureSubmitMode,
) -> Result<Option<LoomMcpActionResult>, state::store::StateError> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    if delivery.active_phase_id != phase_id {
        return Ok(Some(stale_failure(
            project_root,
            "Architecture submit must bind to the active phase.".to_string(),
            mode,
        )));
    }
    let Some(phase) = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
    else {
        return Ok(Some(stale_failure(
            project_root,
            format!("delivery {} is missing phase {}", delivery_id, phase_id),
            mode,
        )));
    };
    if phase
        .latest_refs
        .get(mode.latest_ref_key())
        .map(String::as_str)
        != Some(request_ref)
    {
        return Ok(Some(stale_failure(
            project_root,
            "Architecture submit must use the active phase latest requestRef.".to_string(),
            mode,
        )));
    }
    Ok(None)
}

fn stale_failure(
    project_root: &str,
    message: String,
    mode: ArchitectureSubmitMode,
) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: project_root.to_string(),
        error: LoomMcpFailure {
            code: mode.stale_code().to_string(),
            message,
            target_batch: Some(mode.target_batch()),
            domain: Some("architecture".to_string()),
            route_action: Some(mode.artifact_source().to_string()),
            recovery_tool: Some("loom.continue".to_string()),
        },
    })
}

fn issue(code: &str, field_path: &str, message: &str) -> delivery_core::RepairIssue {
    delivery_core::RepairIssue {
        code: code.to_string(),
        message: message.to_string(),
        target_id: Some("candidate".to_string()),
        field_path: Some(field_path.to_string()),
    }
}

fn to_state_error(error: delivery_core::LoomCoreError) -> state::store::StateError {
    state::store::StateError::StateCorrupted(error.to_string())
}

fn field_value(
    fields: &std::collections::BTreeMap<String, delivery_core::FieldReadResult>,
    field: &str,
) -> Value {
    fields
        .get(field)
        .map(|result| result.value.clone())
        .unwrap_or(Value::Null)
}
