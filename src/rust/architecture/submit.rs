use std::{collections::BTreeSet, path::Path};

use contracts::{
    AcceptanceMatrixEntry, ArchitectureArtifactContract, ArchitectureArtifactSource,
    ArchitectureArtifactStatus, ArchitectureDetailCoverageEntry, ArchitectureHandoff,
    ArchitectureSectionCandidateAgentWritable, ArchitectureSectionGroup, ArchitectureSectionStatus,
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
    request::{required_content_keys, section_order},
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
    let raw = state::store::read_json_value(&candidate_file)?;
    let candidate: ArchitectureSectionCandidateAgentWritable =
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

    let request_root = load_request_root(&input.project_root, &authorized.request_id)?;
    let request_fields = read_request_fields(
        &input.project_root,
        &input.request_ref,
        &["allowedRefs", "sourceRefs"],
    )?;
    let current_section = parse_section(&request_root, "/sectionState/currentSection")?;
    let section_outputs = parse_section_outputs(&request_root)?;
    let allowed_refs = request_fields
        .get("allowedRefs")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let source_refs = request_fields
        .get("sourceRefs")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let expected_request_id = request_root
        .get("requestId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let expected_delivery_id = request_root
        .get("deliveryId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let expected_phase_id = request_root
        .get("phaseId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut issues = validate_candidate_identity(
        &candidate,
        &expected_request_id,
        &expected_delivery_id,
        &expected_phase_id,
        current_section,
    );
    issues.extend(validate_section_content(&candidate));
    issues.extend(validate_allowed_refs(&candidate.content, &allowed_refs));
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
        let updated_root =
            update_request_for_next_section(request_root, next_section, &next_output)?;
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

fn validate_candidate_identity(
    candidate: &ArchitectureSectionCandidateAgentWritable,
    request_id: &str,
    delivery_id: &str,
    phase_id: &str,
    current_section: ArchitectureSectionGroup,
) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    if candidate.request_id != request_id {
        issues.push(issue(
            "REQUEST_ID_MISMATCH",
            "requestId",
            "Architecture section candidate requestId must match the active request.",
        ));
    }
    if candidate.delivery_id != delivery_id {
        issues.push(issue(
            "DELIVERY_ID_MISMATCH",
            "deliveryId",
            "Architecture section candidate deliveryId must match the active delivery.",
        ));
    }
    if candidate.phase_id != phase_id {
        issues.push(issue(
            "PHASE_ID_MISMATCH",
            "phaseId",
            "Architecture section candidate phaseId must match the active phase.",
        ));
    }
    if candidate.section != current_section {
        issues.push(issue(
            "SECTION_ORDER_INVALID",
            "section",
            "Architecture section candidate section must match the current expected section.",
        ));
    }
    issues
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
    issues
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
    issues
}

fn validate_coverage_section(
    content: &Value,
    allowed_refs: &Value,
) -> Vec<delivery_core::RepairIssue> {
    let detail_ids = string_set(allowed_refs.pointer("/requirementDetailIds"));
    let mut issues = Vec::new();
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
                Some(Value::Array(values)) => total_refs += values.len(),
                _ => issues.push(issue(
                    "DETAIL_COVERAGE_INVALID",
                    &format!("content.detailCoverage[{index}].artifactRefs.{key}"),
                    "detailCoverage.artifactRefs fields must all be arrays.",
                )),
            }
        }
        let has_reason = entry.get("reason").and_then(Value::as_str).is_some();
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
    }
    issues
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
    root: &Value,
) -> Result<Vec<SectionStateOutput>, state::store::StateError> {
    serde_json::from_value(root.get("sectionOutputs").cloned().ok_or_else(|| {
        state::store::StateError::StateCorrupted("request is missing sectionOutputs".to_string())
    })?)
    .map_err(state::store::StateError::Json)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SectionStateOutput {
    section: ArchitectureSectionGroup,
    candidate_file: String,
    schema_ref: String,
    schema_shape: Value,
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
    root["writeTargets"] = json!([{
        "targetId": section_name(next_section),
        "path": next_output.candidate_file,
        "required": true,
        "description": format!("Write the {} Architecture section candidate JSON.", section_name(next_section))
    }]);
    Ok(root)
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
        "path": next_output.candidate_file,
        "required": true,
        "description": format!("Write the {} Architecture section candidate JSON.", section_name(next_section))
    }]);
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
        interfaces: domain
            .content
            .get("interfaces")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
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
        runtime_delivery: runtime.content.get("runtimeDelivery").cloned(),
        acceptance_matrix,
        detail_coverage,
        risks_and_decisions: coverage
            .content
            .get("risksAndDecisions")
            .cloned()
            .unwrap_or_else(|| json!({})),
        handoff,
        created_at: state::store::now_string(),
        updated_at: state::store::now_string(),
    })
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

fn needs_user_decision(coverage_content: &Value) -> bool {
    coverage_content
        .pointer("/risksAndDecisions/decisions")
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

fn read_request_fields(
    project_root: &str,
    request_ref: &str,
    fields: &[&str],
) -> Result<std::collections::BTreeMap<String, Value>, state::store::StateError> {
    let resolved = state::read_request_fields(ReadRequestFieldsInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
        fields: fields.iter().map(|field| field.to_string()).collect(),
    })?;
    Ok(resolved
        .fields
        .into_iter()
        .map(|(field, result)| (field, result.value))
        .collect())
}
