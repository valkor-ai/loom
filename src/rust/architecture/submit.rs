use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use contracts::{
    build_api_quality_seed_from_foundation, normalize_ui_surface_decision_contract_for_persist,
    validate_ui_surface_decision_contract, AcceptanceMatrixEntry, ArchitectureArtifactContract,
    ArchitectureArtifactSource, ArchitectureArtifactStatus, ArchitectureDetailCoverageEntry,
    ArchitectureHandoff, ArchitectureQuality, ArchitectureSectionCandidateAgentWritable,
    ArchitectureSectionGroup, ArchitectureSectionStatus, COVERAGE_ARTIFACT_TYPES,
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
        architecture_contract_file, architecture_latest_file, project_api_contract_file,
        section_name,
    },
    request::{
        architecture_quality_template_from_candidate_plan, architecture_read_groups,
        build_architecture_quality_seed, required_content_keys, section_enum_refs,
        section_generation_rules, section_order, section_result_template, section_schema_shape,
    },
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
    let mut section_outputs =
        parse_section_outputs(&input.project_root, &authorized.request_id, &request_root)?;
    let mut allowed_refs = if section_uses_allowed_refs(current_section) {
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
    if matches!(current_section, ArchitectureSectionGroup::Coverage) {
        enrich_architecture_artifact_refs(project_root, &section_outputs, &mut allowed_refs)?;
    }
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
    issues.extend(validate_architecture_section_semantics(&candidate));
    issues.extend(validate_structured_communication_boundaries(&candidate));
    issues.extend(validate_http_interfaces(&candidate, &request_root));
    if section_uses_allowed_refs(current_section) {
        issues.extend(validate_allowed_refs(&candidate.content, &allowed_refs));
    }
    issues.extend(validate_frontend_rules(&candidate, &request_root));
    issues.extend(validate_runtime_rules(
        &candidate,
        &source_refs,
        &request_root,
    ));
    if matches!(candidate.section, ArchitectureSectionGroup::Coverage) {
        issues.extend(validate_coverage_section(&candidate.content, &allowed_refs));
        issues.extend(validate_architecture_quality_candidate_plan(
            &candidate.content,
            request_root.pointer("/architectureQualitySeed/candidatePlan"),
        ));
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
    if candidate_normalized {
        state::store::write_json_atomic(&candidate_file, &candidate)?;
    }

    let next_section = next_section(candidate.section);
    if let Some(next_section) = next_section {
        let mut request_root = request_root;
        let mut next_output = section_outputs
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
        if matches!(candidate.section, ArchitectureSectionGroup::Foundation)
            && matches!(next_section, ArchitectureSectionGroup::DomainContract)
        {
            let existing_api_contract = load_project_api_contract(project_root, &delivery_id)?;
            let api_quality_seed = build_api_quality_seed_from_foundation(
                &candidate.content,
                existing_api_contract.as_ref(),
            );
            apply_api_quality_seed(&mut request_root, &api_quality_seed);
            rebuild_domain_contract_output(
                project_root,
                &source_refs,
                &request_root,
                &api_quality_seed,
                &mut next_output,
            )?;
        }
        let architecture_candidate_plan =
            if matches!(next_section, ArchitectureSectionGroup::Coverage) {
                Some(build_architecture_quality_candidate_plan(
                    project_root,
                    &section_outputs,
                )?)
            } else {
                None
            };
        let architecture_quality_seed =
            build_architecture_quality_seed(next_section, architecture_candidate_plan.as_ref());
        apply_architecture_quality_seed(&mut request_root, &architecture_quality_seed);
        if let Some(candidate_plan) = architecture_candidate_plan.as_ref() {
            next_output.result_template["content"]["architectureQuality"] =
                architecture_quality_template_from_candidate_plan(candidate_plan);
        }
        let include_repair_context = matches!(mode, ArchitectureSubmitMode::Repair);
        let include_repair_source_ref = include_repair_context
            && repair_context_has_source_ref(
                &input.project_root,
                &input.request_ref,
                &request_root,
            )?;
        let updated_root = update_request_for_next_section(
            &input.project_root,
            &authorized.request_id,
            request_root,
            next_section,
            &next_output,
            include_repair_context,
            include_repair_source_ref,
            &source_refs,
            &mut section_outputs,
        )?;
        let agent_field_policies = delivery_core::derive_agent_field_policies(&updated_root);
        update_output_contract_ref(
            &input.project_root,
            &authorized.request_id,
            next_section,
            &next_output,
            &agent_field_policies,
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

    let mut contract = assemble_architecture_contract(
        &input.project_root,
        &delivery_id,
        &phase_id,
        &section_outputs,
        &source_refs,
    )?;
    let api_contract = materialize_project_api_contract(
        project_root,
        &delivery_id,
        &phase_id,
        &contract.interfaces,
        &section_outputs,
    )?;
    contract.api_contract_ref = api_contract
        .as_ref()
        .map(|(contract_ref, _)| contract_ref.clone());
    contract.current_phase_interface_refs = contract
        .interfaces
        .iter()
        .filter(|interface| interface.get("type").and_then(Value::as_str) == Some("http_api"))
        .filter_map(|interface| interface.get("interfaceId").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    if let Some((_, value)) = api_contract {
        state::store::write_json_atomic(
            &project_api_contract_file(project_root, &delivery_id),
            &value,
        )?;
    }
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
    if matches!(current_section, ArchitectureSectionGroup::Coverage) {
        normalize_architecture_quality_candidate_fields(
            object.get_mut("content"),
            request_root.pointer("/architectureQualitySeed/candidatePlan"),
        );
    }
}

fn normalize_architecture_quality_candidate_fields(
    content: Option<&mut Value>,
    candidate_plan: Option<&Value>,
) {
    let (Some(quality), Some(plan)) = (
        content
            .and_then(|content| content.get_mut("architectureQuality"))
            .and_then(Value::as_object_mut),
        candidate_plan,
    ) else {
        return;
    };
    normalize_architecture_quality_collection(
        quality.get_mut("decisions"),
        plan.get("decisionCandidates"),
        plan,
        "decisionId",
        |item, candidate, _| {
            overlay_candidate_field(item, candidate, "decisionId");
            overlay_candidate_field(item, candidate, "category");
            overlay_candidate_field(item, candidate, "sourceRefs");
            overlay_candidate_field(item, candidate, "ownerArtifactRefs");
        },
    );
    normalize_architecture_quality_collection(
        quality.get_mut("nfrs"),
        plan.get("nfrCandidates"),
        plan,
        "nfrId",
        |item, candidate, plan| {
            overlay_candidate_field(item, candidate, "nfrId");
            overlay_candidate_field(item, candidate, "category");
            overlay_candidate_field(item, candidate, "source");
            overlay_candidate_field(item, candidate, "sourceRefs");
            overlay_candidate_field(item, candidate, "ownerArtifactRefs");
            let risk_refs = candidate_refs_targeting_id(
                plan.get("riskCandidates"),
                "nfrRefs",
                "riskId",
                candidate
                    .get("nfrId")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            );
            set_object_field(
                item,
                "architectureRefs",
                json!({
                    "decisions": candidate.get("decisionRefs").cloned().unwrap_or_else(|| json!([])),
                    "risks": risk_refs
                }),
            );
        },
    );
    normalize_architecture_quality_collection(
        quality.get_mut("risks"),
        plan.get("riskCandidates"),
        plan,
        "riskId",
        |item, candidate, _| {
            overlay_candidate_field(item, candidate, "riskId");
            overlay_candidate_field(item, candidate, "category");
            let mut owners = candidate
                .get("ownerArtifactRefs")
                .cloned()
                .unwrap_or_else(|| json!({"modules": [], "interfaces": []}));
            owners["decisions"] = candidate
                .get("decisionRefs")
                .cloned()
                .unwrap_or_else(|| json!([]));
            owners["nfrs"] = candidate
                .get("nfrRefs")
                .cloned()
                .unwrap_or_else(|| json!([]));
            set_object_field(item, "ownerArtifactRefs", owners);
        },
    );
}

fn normalize_architecture_quality_collection(
    actual: Option<&mut Value>,
    expected: Option<&Value>,
    plan: &Value,
    id_field: &str,
    overlay: impl Fn(&mut Value, &Value, &Value),
) {
    let (Some(actual), Some(expected)) = (
        actual.and_then(Value::as_array_mut),
        expected.and_then(Value::as_array),
    ) else {
        return;
    };
    let original = std::mem::take(actual);
    let expected_ids = expected
        .iter()
        .filter_map(|candidate| candidate.get(id_field).and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    let mut consumed = BTreeSet::new();
    for (expected_index, candidate) in expected.iter().enumerate() {
        let candidate_id = candidate.get(id_field).and_then(Value::as_str);
        let matching_index = candidate_id.and_then(|candidate_id| {
            original.iter().enumerate().find_map(|(index, item)| {
                (!consumed.contains(&index)
                    && item.get(id_field).and_then(Value::as_str) == Some(candidate_id))
                .then_some(index)
            })
        });
        let source_index = matching_index.or_else(|| {
            (expected_index < original.len() && !consumed.contains(&expected_index))
                .then_some(expected_index)
        });
        let Some(source_index) = source_index else {
            continue;
        };
        consumed.insert(source_index);
        let mut item = original[source_index].clone();
        overlay(&mut item, candidate, plan);
        actual.push(item);
    }
    actual.extend(
        original
            .into_iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if consumed.contains(&index)
                    || item
                        .get(id_field)
                        .and_then(Value::as_str)
                        .is_some_and(|id| expected_ids.contains(id))
                {
                    None
                } else {
                    Some(item)
                }
            }),
    );
}

fn overlay_candidate_field(item: &mut Value, candidate: &Value, field: &str) {
    if let Some(value) = candidate.get(field) {
        set_object_field(item, field, value.clone());
    }
}

fn set_object_field(item: &mut Value, field: &str, value: Value) {
    if let Some(object) = item.as_object_mut() {
        object.insert(field.to_string(), value);
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

fn validate_structured_communication_boundaries(
    candidate: &ArchitectureSectionCandidateAgentWritable,
) -> Vec<delivery_core::RepairIssue> {
    if !matches!(candidate.section, ArchitectureSectionGroup::Foundation) {
        return vec![];
    }
    let Some(interactions) = candidate
        .content
        .pointer("/engineeringBoundary/applicationInteractions")
        .and_then(Value::as_array)
    else {
        return vec![issue(
            "APPLICATION_INTERACTIONS_REQUIRED",
            "content.engineeringBoundary.applicationInteractions",
            "Foundation must declare applicationInteractions as structured objects, including an empty array when there are no cross-boundary interactions.",
        )];
    };
    let allowed = BTreeSet::from([
        "http_api",
        "service_method",
        "external_adapter",
        "event",
        "job",
        "cli_command",
    ]);
    let mut issues = Vec::new();
    for (index, interaction) in interactions.iter().enumerate() {
        let path = format!("content.engineeringBoundary.applicationInteractions[{index}]");
        let Some(object) = interaction.as_object() else {
            issues.push(issue(
                "APPLICATION_INTERACTION_OBJECT_REQUIRED",
                &path,
                "Each application interaction must be an object; prose does not activate API applicability.",
            ));
            continue;
        };
        let interaction_type = object
            .get("interactionType")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !allowed.contains(interaction_type) {
            issues.push(issue(
                "APPLICATION_INTERACTION_TYPE_INVALID",
                &format!("{path}.interactionType"),
                "interactionType must use the declared structured communication enum.",
            ));
        }
        for key in [
            "interactionId",
            "providerApplicationRef",
            "providerModuleRef",
        ] {
            if object
                .get(key)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                issues.push(issue(
                    "APPLICATION_INTERACTION_FIELD_REQUIRED",
                    &format!("{path}.{key}"),
                    "Structured application interactions must identify their owner and stable id.",
                ));
            }
        }
        if interaction_type == "http_api" {
            let Some(traits) = object.get("qualityTraits").and_then(Value::as_object) else {
                issues.push(issue(
                    "HTTP_INTERACTION_QUALITY_TRAITS_REQUIRED",
                    &format!("{path}.qualityTraits"),
                    "HTTP interactions must declare structured qualityTraits for API contract generation.",
                ));
                continue;
            };
            let auth_requirement = traits
                .get("authRequirement")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !matches!(
                auth_requirement,
                "not_applicable" | "required" | "optional" | "deferred_with_risk"
            ) {
                issues.push(issue(
                    "HTTP_INTERACTION_AUTH_REQUIREMENT_INVALID",
                    &format!("{path}.qualityTraits.authRequirement"),
                    "authRequirement must be not_applicable, required, optional, or deferred_with_risk.",
                ));
            }
            for key in [
                "paginationRequired",
                "contractArtifactRequired",
                "compatibilityRequired",
            ] {
                if !traits.get(key).is_some_and(Value::is_boolean) {
                    issues.push(issue(
                        "HTTP_INTERACTION_QUALITY_TRAIT_BOOLEAN_REQUIRED",
                        &format!("{path}.qualityTraits.{key}"),
                        &format!("{key} must be an explicit boolean."),
                    ));
                }
            }
            let allowed_policies =
                BTreeSet::from(["idempotency", "cache", "retry", "rate_limit", "request_id"]);
            let Some(policies) = traits.get("operationalPolicies").and_then(Value::as_array) else {
                issues.push(issue(
                    "HTTP_INTERACTION_OPERATIONAL_POLICIES_REQUIRED",
                    &format!("{path}.qualityTraits.operationalPolicies"),
                    "operationalPolicies must be an array, including an empty array when no operational API policy applies.",
                ));
                continue;
            };
            for (policy_index, policy) in policies.iter().enumerate() {
                if policy
                    .as_str()
                    .is_none_or(|policy| !allowed_policies.contains(policy))
                {
                    issues.push(issue(
                        "HTTP_INTERACTION_OPERATIONAL_POLICY_INVALID",
                        &format!(
                            "{path}.qualityTraits.operationalPolicies[{policy_index}]"
                        ),
                        "Operational policy must be idempotency, cache, retry, rate_limit, or request_id.",
                    ));
                }
            }
        }
    }
    issues
}

fn validate_architecture_section_semantics(
    candidate: &ArchitectureSectionCandidateAgentWritable,
) -> Vec<delivery_core::RepairIssue> {
    match candidate.section {
        ArchitectureSectionGroup::Foundation => {
            let mut issues = validate_pattern_decision(&candidate.content);
            issues.extend(validate_foundation_modules(&candidate.content));
            issues
        }
        ArchitectureSectionGroup::DomainContract => validate_data_architecture(&candidate.content),
        ArchitectureSectionGroup::Behavior => validate_behavior_contract(&candidate.content),
        _ => vec![],
    }
}

fn validate_foundation_modules(content: &Value) -> Vec<delivery_core::RepairIssue> {
    let Some(modules) = content.get("modules").and_then(Value::as_array) else {
        return vec![issue(
            "ARCHITECTURE_MODULES_REQUIRED",
            "content.modules",
            "Foundation must declare the current-phase implementation modules as an array.",
        )];
    };
    if modules.is_empty() {
        return vec![issue(
            "ARCHITECTURE_MODULES_REQUIRED",
            "content.modules",
            "Foundation must identify at least one implementation module that can own architecture decisions and tasks.",
        )];
    }
    let mut issues = Vec::new();
    let mut module_ids = BTreeSet::new();
    for (index, module) in modules.iter().enumerate() {
        let path = format!("content.modules[{index}]");
        let Some(module) = module.as_object() else {
            issues.push(issue(
                "ARCHITECTURE_MODULE_OBJECT_REQUIRED",
                &path,
                "Each architecture module must be a structured object.",
            ));
            continue;
        };
        for field in ["moduleId", "name"] {
            if module
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                issues.push(issue(
                    "ARCHITECTURE_MODULE_FIELD_REQUIRED",
                    &format!("{path}.{field}"),
                    "Architecture modules must have a stable id and descriptive name.",
                ));
            }
        }
        if let Some(id) = module.get("moduleId").and_then(Value::as_str) {
            if !id.trim().is_empty() && !module_ids.insert(id) {
                issues.push(issue(
                    "ARCHITECTURE_MODULE_ID_DUPLICATE",
                    &format!("{path}.moduleId"),
                    "Architecture module ids must be unique within the current Foundation.",
                ));
            }
        }
        let has_responsibility = ["responsibility", "summary"].iter().any(|field| {
            module
                .get(*field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        });
        if !has_responsibility {
            issues.push(issue(
                "ARCHITECTURE_MODULE_FIELD_REQUIRED",
                &format!("{path}.responsibility"),
                "Architecture modules must state their current-phase responsibility.",
            ));
        }
        let scope_refs = module
            .get("scopeRefs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let acceptance_refs = module
            .get("acceptanceRefs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if scope_refs.is_empty() && acceptance_refs.is_empty() {
            issues.push(issue(
                "ARCHITECTURE_MODULE_SOURCE_REQUIRED",
                &format!("{path}.scopeRefs"),
                "Every architecture module must cite current-phase scope or acceptance ownership.",
            ));
        }
    }
    issues
}

fn validate_pattern_decision(content: &Value) -> Vec<delivery_core::RepairIssue> {
    let path = "content.engineeringBoundary.patternDecision";
    let Some(decision) = content
        .pointer("/engineeringBoundary/patternDecision")
        .and_then(Value::as_object)
    else {
        return vec![issue(
            "ARCHITECTURE_PATTERN_DECISION_REQUIRED",
            path,
            "Foundation must include a structured patternDecision for the current phase.",
        )];
    };
    let mut issues = Vec::new();
    let classification = decision
        .get("classification")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(classification, "known" | "hybrid" | "custom") {
        issues.push(issue(
            "ARCHITECTURE_PATTERN_CLASSIFICATION_INVALID",
            &format!("{path}.classification"),
            "patternDecision.classification must be known, hybrid, or custom.",
        ));
    }
    for field in ["patternId", "patternName", "rationale"] {
        if decision
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            issues.push(issue(
                "ARCHITECTURE_PATTERN_FIELD_REQUIRED",
                &format!("{path}.{field}"),
                "Pattern decisions must name and justify the selected current-phase structure.",
            ));
        }
    }
    for field in ["decisionDrivers", "structuralRules"] {
        let Some(values) = decision.get(field).and_then(Value::as_array) else {
            issues.push(issue(
                "ARCHITECTURE_PATTERN_EVIDENCE_REQUIRED",
                &format!("{path}.{field}"),
                "Pattern decisions must include concrete current-phase drivers and structural rules.",
            ));
            continue;
        };
        if values.is_empty() {
            issues.push(issue(
                "ARCHITECTURE_PATTERN_EVIDENCE_REQUIRED",
                &format!("{path}.{field}"),
                "Pattern decisions must include concrete current-phase drivers and structural rules.",
            ));
        }
        validate_array_entries_are_strings(
            values,
            &format!("{path}.{field}"),
            "ARCHITECTURE_PATTERN_EVIDENCE_REQUIRED",
            &mut issues,
        );
    }
    match decision.get("composedOf").and_then(Value::as_array) {
        Some(values) => validate_array_entries_are_strings(
            values,
            &format!("{path}.composedOf"),
            "ARCHITECTURE_PATTERN_COMPOSITION_REQUIRED",
            &mut issues,
        ),
        None => issues.push(issue(
            "ARCHITECTURE_PATTERN_COMPOSITION_REQUIRED",
            &format!("{path}.composedOf"),
            "patternDecision.composedOf must be an array, including an empty array for a non-hybrid structure.",
        )),
    }
    if classification == "hybrid"
        && decision
            .get("composedOf")
            .and_then(Value::as_array)
            .is_none_or(|items| items.len() < 2)
    {
        issues.push(issue(
            "ARCHITECTURE_PATTERN_COMPOSITION_REQUIRED",
            &format!("{path}.composedOf"),
            "Hybrid pattern decisions must identify at least two composed patterns.",
        ));
    }
    issues
}

fn validate_data_architecture(content: &Value) -> Vec<delivery_core::RepairIssue> {
    let path = "content.dataModel.dataArchitecture";
    let Some(architecture) = content
        .pointer("/dataModel/dataArchitecture")
        .and_then(Value::as_object)
    else {
        return vec![issue(
            "DATA_ARCHITECTURE_REQUIRED",
            path,
            "DomainContract must describe how the current phase uses the selected persistence baseline or intentionally uses no persistence.",
        )];
    };
    let mut issues = Vec::new();
    let mode = architecture
        .get("persistenceMode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(mode, "selected_stack" | "no_persistence") {
        issues.push(issue(
            "DATA_ARCHITECTURE_PERSISTENCE_MODE_INVALID",
            &format!("{path}.persistenceMode"),
            "persistenceMode must be selected_stack or no_persistence.",
        ));
    }
    if mode == "selected_stack"
        && architecture
            .get("sourceOfTruth")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
    {
        issues.push(issue(
            "DATA_ARCHITECTURE_SOURCE_OF_TRUTH_REQUIRED",
            &format!("{path}.sourceOfTruth"),
            "Persistent data architecture must identify the current-phase source of truth.",
        ));
    }
    for field in [
        "ownership",
        "invariants",
        "transactionBoundaries",
        "consistencyRules",
        "migrationImpacts",
        "readModels",
        "lifecyclePolicies",
        "derivedData",
    ] {
        if !architecture.get(field).is_some_and(Value::is_array) {
            issues.push(issue(
                "DATA_ARCHITECTURE_COLLECTION_REQUIRED",
                &format!("{path}.{field}"),
                "Data architecture collections must be explicit arrays, including an empty array when the concern does not apply.",
            ));
        }
    }
    for (field, string_fields, array_fields) in [
        (
            "ownership",
            &["dataRef", "ownerModuleRef", "boundary"][..],
            &[][..],
        ),
        (
            "invariants",
            &["invariantId", "ownerModuleRef", "rule", "failureBehavior"][..],
            &["enforcementPoints"][..],
        ),
        (
            "transactionBoundaries",
            &[
                "transactionId",
                "ownerModuleRef",
                "atomicityRule",
                "failureBehavior",
            ][..],
            &["operationRefs"][..],
        ),
        (
            "consistencyRules",
            &["consistencyId", "mode", "rule", "conflictOrStaleBehavior"][..],
            &["dataRefs"][..],
        ),
        (
            "migrationImpacts",
            &[
                "migrationId",
                "change",
                "compatibilityRule",
                "rollbackOrForwardRepair",
                "verification",
            ][..],
            &["dataRefs"][..],
        ),
        (
            "readModels",
            &[
                "readModelId",
                "ownerModuleRef",
                "queryPurpose",
                "boundedReadRule",
                "freshnessRule",
            ][..],
            &["dataRefs"][..],
        ),
        (
            "lifecyclePolicies",
            &["policyId", "lifecycleRule", "cleanupOrArchiveBehavior"][..],
            &["dataRefs"][..],
        ),
        (
            "derivedData",
            &[
                "derivedDataId",
                "ownerModuleRef",
                "refreshTrigger",
                "freshnessRule",
                "rebuildStrategy",
            ][..],
            &["sourceDataRefs"][..],
        ),
    ] {
        validate_structured_entries(
            architecture.get(field),
            &format!("{path}.{field}"),
            string_fields,
            array_fields,
            array_fields,
            "DATA_ARCHITECTURE_ENTRY_INVALID",
            &mut issues,
        );
    }
    let has_entities = content
        .pointer("/dataModel/entities")
        .and_then(Value::as_array)
        .is_some_and(|entities| !entities.is_empty());
    if mode == "selected_stack"
        && has_entities
        && architecture
            .get("ownership")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        issues.push(issue(
            "DATA_ARCHITECTURE_OWNERSHIP_REQUIRED",
            &format!("{path}.ownership"),
            "Persistent current-phase entities must have explicit data and module ownership.",
        ));
    }
    if let Some(rules) = architecture
        .get("consistencyRules")
        .and_then(Value::as_array)
    {
        for (index, rule) in rules.iter().enumerate() {
            if rule
                .get("mode")
                .and_then(Value::as_str)
                .is_some_and(|mode| {
                    !matches!(
                        mode,
                        "strong" | "eventual" | "read_your_writes" | "external_source_owned"
                    )
                })
            {
                issues.push(issue(
                    "DATA_ARCHITECTURE_CONSISTENCY_MODE_INVALID",
                    &format!("{path}.consistencyRules[{index}].mode"),
                    "Consistency mode must use the declared data architecture enum.",
                ));
            }
        }
    }
    issues
}

fn validate_behavior_contract(content: &Value) -> Vec<delivery_core::RepairIssue> {
    let mut issues = Vec::new();
    let Some(flows) = content.get("userFlows").and_then(Value::as_array) else {
        return vec![issue(
            "BEHAVIOR_FLOW_COLLECTION_REQUIRED",
            "content.userFlows",
            "Behavior must declare userFlows as an array.",
        )];
    };
    for (index, flow) in flows.iter().enumerate() {
        let path = format!("content.userFlows[{index}]");
        let Some(flow) = flow.as_object() else {
            issues.push(issue(
                "BEHAVIOR_FLOW_OBJECT_REQUIRED",
                &path,
                "Every user flow must be a structured object.",
            ));
            continue;
        };
        for field in ["flowId", "name", "trigger", "actorRef", "successOutcome"] {
            if flow
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                issues.push(issue(
                    "BEHAVIOR_FLOW_FIELD_REQUIRED",
                    &format!("{path}.{field}"),
                    "User flows must identify their trigger, behavior, and observable success outcome.",
                ));
            }
        }
        if flow
            .get("happyPath")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
        {
            issues.push(issue(
                "BEHAVIOR_HAPPY_PATH_REQUIRED",
                &format!("{path}.happyPath"),
                "User flows must include a non-empty structured happyPath.",
            ));
        }
        validate_structured_entries(
            flow.get("happyPath"),
            &format!("{path}.happyPath"),
            &["stepId", "action", "interactionRef", "observableResult"],
            &["stateMachineRefs", "stateEffects"],
            &[],
            "BEHAVIOR_HAPPY_PATH_INVALID",
            &mut issues,
        );
        validate_structured_entries(
            flow.get("businessBlockingPaths"),
            &format!("{path}.businessBlockingPaths"),
            &["condition", "response", "recovery"],
            &[],
            &[],
            "BEHAVIOR_BLOCKING_PATH_INVALID",
            &mut issues,
        );
        validate_structured_entries(
            flow.get("failurePaths"),
            &format!("{path}.failurePaths"),
            &["failure", "impact", "recovery", "observableResult"],
            &[],
            &[],
            "BEHAVIOR_FAILURE_PATH_INVALID",
            &mut issues,
        );
    }
    let Some(state_machines) = content.get("stateMachines").and_then(Value::as_array) else {
        issues.push(issue(
            "STATE_MACHINE_COLLECTION_REQUIRED",
            "content.stateMachines",
            "Behavior must declare stateMachines as an array, including an empty array when no lifecycle applies.",
        ));
        return issues;
    };
    for (index, machine) in state_machines.iter().enumerate() {
        let path = format!("content.stateMachines[{index}]");
        let Some(machine) = machine.as_object() else {
            issues.push(issue(
                "STATE_MACHINE_OBJECT_REQUIRED",
                &path,
                "Every state machine must be a structured object.",
            ));
            continue;
        };
        for field in ["machineId", "name"] {
            if machine
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                issues.push(issue(
                    "STATE_MACHINE_FIELD_REQUIRED",
                    &format!("{path}.{field}"),
                    "State machines must have a stable id and descriptive name.",
                ));
            }
        }
        if !machine.get("states").is_some_and(Value::is_array) {
            issues.push(issue(
                "STATE_MACHINE_STATES_REQUIRED",
                &format!("{path}.states"),
                "State machines must declare states as an array.",
            ));
        }
        validate_structured_entries(
            machine.get("transitions"),
            &format!("{path}.transitions"),
            &["from", "to", "trigger", "failureBehavior"],
            &["guards", "effects"],
            &[],
            "STATE_TRANSITION_INVALID",
            &mut issues,
        );
    }
    issues
}

fn validate_structured_entries(
    value: Option<&Value>,
    path: &str,
    required_strings: &[&str],
    required_arrays: &[&str],
    non_empty_arrays: &[&str],
    code: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(entries) = value.and_then(Value::as_array) else {
        issues.push(issue(
            code,
            path,
            "The field must be an explicit array of structured objects, including an empty array when it does not apply.",
        ));
        return;
    };
    for (index, entry) in entries.iter().enumerate() {
        let entry_path = format!("{path}[{index}]");
        let Some(entry) = entry.as_object() else {
            issues.push(issue(code, &entry_path, "The entry must be an object."));
            continue;
        };
        for field in required_strings {
            if entry
                .get(*field)
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty())
            {
                issues.push(issue(
                    code,
                    &format!("{entry_path}.{field}"),
                    "The structured entry field must be a non-empty string.",
                ));
            }
        }
        for field in required_arrays {
            let Some(values) = entry.get(*field).and_then(Value::as_array) else {
                issues.push(issue(
                    code,
                    &format!("{entry_path}.{field}"),
                    "The structured entry field must be an array.",
                ));
                continue;
            };
            validate_array_entries_are_strings(
                values,
                &format!("{entry_path}.{field}"),
                code,
                issues,
            );
            if non_empty_arrays.contains(field) && values.is_empty() {
                issues.push(issue(
                    code,
                    &format!("{entry_path}.{field}"),
                    "The structured entry array must identify at least one applicable item.",
                ));
            }
        }
    }
}

fn validate_http_interfaces(
    candidate: &ArchitectureSectionCandidateAgentWritable,
    request_root: &Value,
) -> Vec<delivery_core::RepairIssue> {
    if !matches!(candidate.section, ArchitectureSectionGroup::DomainContract)
        || request_root
            .get("apiQualitySeed")
            .is_none_or(Value::is_null)
    {
        return vec![];
    }
    let Some(interfaces) = candidate
        .content
        .get("interfaces")
        .and_then(Value::as_array)
    else {
        return vec![issue(
            "HTTP_INTERFACE_ARRAY_REQUIRED",
            "content.interfaces",
            "API-enabled DomainContract interfaces must be an array of interface objects.",
        )];
    };
    let allowed_types = BTreeSet::from([
        "http_api",
        "service_method",
        "external_adapter",
        "event",
        "job",
        "cli_command",
    ]);
    let mut issues = Vec::new();
    for (index, interface) in interfaces.iter().enumerate() {
        let path = format!("content.interfaces[{index}]");
        let Some(object) = interface.as_object() else {
            issues.push(issue(
                "HTTP_INTERFACE_OBJECT_REQUIRED",
                &path,
                "HTTP interface entries must be objects; strings cannot represent the API contract.",
            ));
            continue;
        };
        let Some(interface_type) = object.get("type").and_then(Value::as_str) else {
            issues.push(issue(
                "HTTP_INTERFACE_TYPE_REQUIRED",
                &format!("{path}.type"),
                "Every interface entry must declare its structured type.",
            ));
            continue;
        };
        if !allowed_types.contains(interface_type) {
            issues.push(issue(
                "HTTP_INTERFACE_TYPE_INVALID",
                &format!("{path}.type"),
                "Interface type must use the declared structured communication enum.",
            ));
            continue;
        }
        if interface_type == "http_api" {
            for key in [
                "interfaceId",
                "method",
                "path",
                "requestSchema",
                "responseSchema",
            ] {
                if !object.contains_key(key) {
                    issues.push(issue(
                        "HTTP_INTERFACE_FIELD_REQUIRED",
                        &format!("{path}.{key}"),
                        "Every HTTP interface must carry its contract fields in the current DomainContract.",
                    ));
                }
            }
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
    request_root: &Value,
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
        match runtime.get("runtimeDependencies") {
            Some(Value::Array(dependencies)) => {
                for (index, dependency) in dependencies.iter().enumerate() {
                    let path = format!("content.runtimeDelivery.runtimeDependencies[{index}]");
                    let Some(dependency) = dependency.as_object() else {
                        issues.push(issue(
                            "RUNTIME_DEPENDENCY_OBJECT_REQUIRED",
                            &path,
                            "Each runtime dependency must be a structured object.",
                        ));
                        continue;
                    };
                    for field in dependency.keys() {
                        if !matches!(
                            field.as_str(),
                            "dependencyId"
                                | "kind"
                                | "requiredFor"
                                | "startupRequirement"
                                | "failureBehavior"
                                | "recoveryStrategy"
                                | "observability"
                        ) {
                            issues.push(issue(
                                "RUNTIME_DEPENDENCY_FIELD_UNKNOWN",
                                &format!("{path}.{field}"),
                                "Runtime dependency fields must come from the current write contract; remove legacy or invented fields.",
                            ));
                        }
                    }
                    for field in [
                        "dependencyId",
                        "kind",
                        "startupRequirement",
                        "failureBehavior",
                        "recoveryStrategy",
                    ] {
                        if dependency
                            .get(field)
                            .and_then(Value::as_str)
                            .is_none_or(|value| value.trim().is_empty())
                        {
                            issues.push(issue(
                                "RUNTIME_DEPENDENCY_FIELD_REQUIRED",
                                &format!("{path}.{field}"),
                                "Runtime dependencies must declare ownership, startup, failure, and recovery semantics.",
                            ));
                        }
                    }
                    for field in ["requiredFor", "observability"] {
                        let Some(values) = dependency.get(field).and_then(Value::as_array) else {
                            issues.push(issue(
                                "RUNTIME_DEPENDENCY_EVIDENCE_REQUIRED",
                                &format!("{path}.{field}"),
                                "Runtime dependencies must identify affected capabilities and observable signals.",
                            ));
                            continue;
                        };
                        if values.is_empty() {
                            issues.push(issue(
                                "RUNTIME_DEPENDENCY_EVIDENCE_REQUIRED",
                                &format!("{path}.{field}"),
                                "Runtime dependencies must identify affected capabilities and observable signals.",
                            ));
                        }
                        validate_array_entries_are_strings(
                            values,
                            &format!("{path}.{field}"),
                            "RUNTIME_DEPENDENCY_EVIDENCE_REQUIRED",
                            &mut issues,
                        );
                    }
                    if dependency
                        .get("kind")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| {
                            !matches!(
                                kind,
                                "service" | "storage" | "queue" | "filesystem" | "external_runtime"
                            )
                        })
                    {
                        issues.push(issue(
                            "RUNTIME_DEPENDENCY_KIND_INVALID",
                            &format!("{path}.kind"),
                            "Runtime dependency kind must use the declared enum.",
                        ));
                    }
                    if dependency
                        .get("startupRequirement")
                        .and_then(Value::as_str)
                        .is_some_and(|requirement| {
                            !matches!(requirement, "required" | "optional" | "not_applicable")
                        })
                    {
                        issues.push(issue(
                            "RUNTIME_DEPENDENCY_STARTUP_INVALID",
                            &format!("{path}.startupRequirement"),
                            "startupRequirement must be required, optional, or not_applicable.",
                        ));
                    }
                }
            }
            _ => {
                issues.push(issue(
                    "RUNTIME_DEPENDENCIES_INVALID",
                    "content.runtimeDelivery.runtimeDependencies",
                    "runtimeDependencies must be an explicit array, including an empty array when there are no current runtime dependencies.",
                ));
            }
        }
        validate_runtime_dependency_seed(runtime, request_root, &mut issues);
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

fn validate_runtime_dependency_seed(
    runtime: &Value,
    request_root: &Value,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let Some(candidates) = request_root
        .pointer("/runtimeDependencySeed/candidates")
        .and_then(Value::as_array)
    else {
        return;
    };
    if candidates.is_empty() {
        return;
    }
    let dependencies = runtime
        .get("runtimeDependencies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, candidate) in candidates.iter().enumerate() {
        let Some(candidate_id) = candidate.get("dependencyId").and_then(Value::as_str) else {
            continue;
        };
        let matched = dependencies.iter().any(|dependency| {
            dependency.get("dependencyId").and_then(Value::as_str) == Some(candidate_id)
                && dependency.get("kind") == candidate.get("kind")
                && dependency.get("startupRequirement") == candidate.get("startupRequirement")
        });
        if !matched {
            issues.push(issue(
                "RUNTIME_DEPENDENCY_SEED_UNSATISFIED",
                &format!("content.runtimeDelivery.runtimeDependencies[{index}]"),
                &format!(
                    "MCP derived runtime dependency {candidate_id} must be represented with kind and startupRequirement from runtimeDependencySeed; it cannot be removed or replaced with an unrelated dependency."
                ),
            ));
        }
    }
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
        "scalability",
        "availability",
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
    let allowed_module_refs = string_set(allowed_refs.pointer("/moduleRefs"));
    let allowed_interface_refs = string_set(allowed_refs.pointer("/interfaceRefs"));
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
    validate_unique_ids(
        model.decisions.iter().map(|item| item.decision_id.as_str()),
        "content.architectureQuality.decisions",
        "decisionId",
        issues,
    );
    validate_unique_ids(
        model.nfrs.iter().map(|item| item.nfr_id.as_str()),
        "content.architectureQuality.nfrs",
        "nfrId",
        issues,
    );
    validate_unique_ids(
        model.risks.iter().map(|item| item.risk_id.as_str()),
        "content.architectureQuality.risks",
        "riskId",
        issues,
    );
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
        for (alternative_index, alternative) in decision.alternatives_considered.iter().enumerate()
        {
            for (field, value) in [
                ("name", alternative.name.as_str()),
                ("tradeoff", alternative.tradeoff.as_str()),
                ("rejectedBecause", alternative.rejected_because.as_str()),
            ] {
                validate_non_empty(
                    value,
                    &format!(
                        "content.architectureQuality.decisions[{index}].alternativesConsidered[{alternative_index}].{field}"
                    ),
                    issues,
                );
            }
        }
        if decision.consequences.positive.is_empty() || decision.consequences.negative.is_empty() {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INCOMPLETE",
                &format!("content.architectureQuality.decisions[{index}].consequences"),
                "architecture decisions must include at least one positive and one negative consequence.",
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
        if decision.source_refs.scope_refs.is_empty()
            && decision.source_refs.acceptance_refs.is_empty()
            && decision.source_refs.requirement_detail_refs.is_empty()
        {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_SOURCE_REQUIRED",
                &format!("content.architectureQuality.decisions[{index}].sourceRefs"),
                "architecture decisions must cite at least one current-phase scope, acceptance, or requirement detail ref.",
            ));
        }
        validate_ref_members(
            &decision.owner_artifact_refs.modules,
            &allowed_module_refs,
            &format!("content.architectureQuality.decisions[{index}].ownerArtifactRefs.modules"),
            issues,
        );
        validate_ref_members(
            &decision.owner_artifact_refs.interfaces,
            &allowed_interface_refs,
            &format!("content.architectureQuality.decisions[{index}].ownerArtifactRefs.interfaces"),
            issues,
        );
        if decision.owner_artifact_refs.modules.is_empty()
            && decision.owner_artifact_refs.interfaces.is_empty()
        {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_OWNER_REQUIRED",
                &format!("content.architectureQuality.decisions[{index}].ownerArtifactRefs"),
                "architecture decisions must identify at least one owning module or interface.",
            ));
        }
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
        if !matches!(
            nfr.source.as_str(),
            "confirmed_requirement" | "derived_minimum"
        ) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.nfrs[{index}].source"),
                "nfr source must be confirmed_requirement or derived_minimum.",
            ));
        }
        for (field, value) in [
            ("indicator", nfr.measurement.indicator.as_str()),
            (
                "workloadOrCondition",
                nfr.measurement.workload_or_condition.as_str(),
            ),
            (
                "evaluationBoundary",
                nfr.measurement.evaluation_boundary.as_str(),
            ),
        ] {
            validate_non_empty(
                value,
                &format!("content.architectureQuality.nfrs[{index}].measurement.{field}"),
                issues,
            );
        }
        if !allowed_nfr_categories.contains(nfr.category.as_str()) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_INVALID",
                &format!("content.architectureQuality.nfrs[{index}].category"),
                "nfr category must come from enumRefs.architectureQuality.nfrCategory.",
            ));
        }
        validate_ref_members(
            &nfr.source_refs.scope_refs,
            &allowed_scope_refs,
            &format!("content.architectureQuality.nfrs[{index}].sourceRefs.scopeRefs"),
            issues,
        );
        validate_ref_members(
            &nfr.source_refs.acceptance_refs,
            &allowed_acceptance_refs,
            &format!("content.architectureQuality.nfrs[{index}].sourceRefs.acceptanceRefs"),
            issues,
        );
        validate_ref_members(
            &nfr.source_refs.requirement_detail_refs,
            &allowed_detail_refs,
            &format!("content.architectureQuality.nfrs[{index}].sourceRefs.requirementDetailRefs"),
            issues,
        );
        if nfr.source_refs.scope_refs.is_empty()
            && nfr.source_refs.acceptance_refs.is_empty()
            && nfr.source_refs.requirement_detail_refs.is_empty()
        {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_SOURCE_REQUIRED",
                &format!("content.architectureQuality.nfrs[{index}].sourceRefs"),
                "architecture NFRs must cite at least one current-phase scope, acceptance, or requirement detail ref.",
            ));
        }
        if nfr.source == "confirmed_requirement"
            && nfr.source_refs.acceptance_refs.is_empty()
            && nfr.source_refs.requirement_detail_refs.is_empty()
        {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_CONFIRMED_SOURCE_INVALID",
                &format!("content.architectureQuality.nfrs[{index}].sourceRefs"),
                "confirmed_requirement NFRs must cite an accepted criterion or requirement detail; a scope ref alone does not establish a confirmed quality target.",
            ));
        }
        validate_ref_members(
            &nfr.architecture_refs.decisions,
            &decision_ids,
            &format!("content.architectureQuality.nfrs[{index}].architectureRefs.decisions"),
            issues,
        );
        validate_ref_members(
            &nfr.owner_artifact_refs.modules,
            &allowed_module_refs,
            &format!("content.architectureQuality.nfrs[{index}].ownerArtifactRefs.modules"),
            issues,
        );
        validate_ref_members(
            &nfr.owner_artifact_refs.interfaces,
            &allowed_interface_refs,
            &format!("content.architectureQuality.nfrs[{index}].ownerArtifactRefs.interfaces"),
            issues,
        );
        if nfr.owner_artifact_refs.modules.is_empty()
            && nfr.owner_artifact_refs.interfaces.is_empty()
        {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_OWNER_REQUIRED",
                &format!("content.architectureQuality.nfrs[{index}].ownerArtifactRefs"),
                "architecture NFRs must identify at least one owning module or interface.",
            ));
        }
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
            &risk.owner_artifact_refs.modules,
            &allowed_module_refs,
            &format!("content.architectureQuality.risks[{index}].ownerArtifactRefs.modules"),
            issues,
        );
        validate_ref_members(
            &risk.owner_artifact_refs.interfaces,
            &allowed_interface_refs,
            &format!("content.architectureQuality.risks[{index}].ownerArtifactRefs.interfaces"),
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
        if risk.owner_artifact_refs.modules.is_empty()
            && risk.owner_artifact_refs.interfaces.is_empty()
            && risk.owner_artifact_refs.decisions.is_empty()
            && risk.owner_artifact_refs.nfrs.is_empty()
        {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_OWNER_REQUIRED",
                &format!("content.architectureQuality.risks[{index}].ownerArtifactRefs"),
                "architecture risks must identify at least one owning artifact.",
            ));
        }
    }
}

fn validate_architecture_quality_candidate_plan(
    content: &Value,
    candidate_plan: Option<&Value>,
) -> Vec<delivery_core::RepairIssue> {
    let Some(plan) = candidate_plan else {
        return vec![issue(
            "ARCHITECTURE_QUALITY_CANDIDATE_PLAN_REQUIRED",
            "architectureQualitySeed.candidatePlan",
            "Coverage requires the MCP-derived architecture quality candidate plan.",
        )];
    };
    let quality = content.get("architectureQuality").unwrap_or(&Value::Null);
    let mut issues = Vec::new();
    for (plan_key, quality_key, id_key) in [
        ("decisionCandidates", "decisions", "decisionId"),
        ("nfrCandidates", "nfrs", "nfrId"),
        ("riskCandidates", "risks", "riskId"),
    ] {
        let expected = candidate_map(plan.get(plan_key), id_key);
        let actual = candidate_map(quality.get(quality_key), id_key);
        let expected_ids = expected.keys().cloned().collect::<BTreeSet<_>>();
        let actual_ids = actual.keys().cloned().collect::<BTreeSet<_>>();
        if !expected_ids.is_subset(&actual_ids) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_CANDIDATE_COVERAGE_INVALID",
                &format!("content.architectureQuality.{quality_key}"),
                "Architecture quality output must complete every MCP-derived candidate exactly once; additional items require independently valid accepted source facts and ownership.",
            ));
            continue;
        }
        for (id, expected_item) in expected {
            let Some(actual_item) = actual.get(&id) else {
                continue;
            };
            for field in ["category", "source"] {
                if let Some(expected_value) = expected_item.get(field) {
                    if actual_item.get(field) != Some(expected_value) {
                        issues.push(issue(
                            "ARCHITECTURE_QUALITY_CANDIDATE_IDENTITY_INVALID",
                            &format!("content.architectureQuality.{quality_key}.{id}.{field}"),
                            "Candidate identity fields are MCP-derived and must be preserved exactly.",
                        ));
                    }
                }
            }
            if let Some(expected_owners) = expected_item.get("ownerArtifactRefs") {
                for owner_field in ["modules", "interfaces"] {
                    let expected_refs = string_set(expected_owners.get(owner_field));
                    let actual_refs = string_set(
                        actual_item
                            .get("ownerArtifactRefs")
                            .and_then(|owners| owners.get(owner_field)),
                    );
                    if expected_refs != actual_refs {
                        issues.push(issue(
                            "ARCHITECTURE_QUALITY_CANDIDATE_OWNER_INVALID",
                            &format!(
                                "content.architectureQuality.{quality_key}.{id}.ownerArtifactRefs.{owner_field}"
                            ),
                            "Candidate module and interface ownership is MCP-derived and must be preserved exactly.",
                        ));
                    }
                }
            }
            if let Some(expected_sources) = expected_item.get("sourceRefs") {
                for source_field in ["scopeRefs", "acceptanceRefs", "requirementDetailRefs"] {
                    let expected_refs = string_set(expected_sources.get(source_field));
                    let actual_refs = string_set(
                        actual_item
                            .get("sourceRefs")
                            .and_then(|sources| sources.get(source_field)),
                    );
                    if expected_refs != actual_refs {
                        issues.push(issue(
                            "ARCHITECTURE_QUALITY_CANDIDATE_SOURCE_INVALID",
                            &format!(
                                "content.architectureQuality.{quality_key}.{id}.sourceRefs.{source_field}"
                            ),
                            "Candidate source refs are MCP-derived from accepted architecture facts and must be preserved exactly.",
                        ));
                    }
                }
            }
            match quality_key {
                "nfrs" => {
                    validate_candidate_ref_identity(
                        expected_item.get("decisionRefs"),
                        actual_item.pointer("/architectureRefs/decisions"),
                        &format!(
                            "content.architectureQuality.{quality_key}.{id}.architectureRefs.decisions"
                        ),
                        &mut issues,
                    );
                    let expected_risk_refs = candidate_refs_targeting_id(
                        plan.get("riskCandidates"),
                        "nfrRefs",
                        "riskId",
                        &id,
                    );
                    validate_candidate_ref_identity(
                        Some(&expected_risk_refs),
                        actual_item.pointer("/architectureRefs/risks"),
                        &format!(
                            "content.architectureQuality.{quality_key}.{id}.architectureRefs.risks"
                        ),
                        &mut issues,
                    );
                }
                "risks" => {
                    validate_candidate_ref_identity(
                        expected_item.get("decisionRefs"),
                        actual_item.pointer("/ownerArtifactRefs/decisions"),
                        &format!(
                            "content.architectureQuality.{quality_key}.{id}.ownerArtifactRefs.decisions"
                        ),
                        &mut issues,
                    );
                    validate_candidate_ref_identity(
                        expected_item.get("nfrRefs"),
                        actual_item.pointer("/ownerArtifactRefs/nfrs"),
                        &format!(
                            "content.architectureQuality.{quality_key}.{id}.ownerArtifactRefs.nfrs"
                        ),
                        &mut issues,
                    );
                }
                _ => {}
            }
        }
    }
    issues
}

fn candidate_map(value: Option<&Value>, id_key: &str) -> BTreeMap<String, Value> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.get(id_key)
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), item.clone()))
        })
        .collect()
}

fn candidate_refs_targeting_id(
    candidates: Option<&Value>,
    target_refs_field: &str,
    id_field: &str,
    target_id: &str,
) -> Value {
    json!(candidates
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|candidate| {
            candidate
                .get(target_refs_field)
                .and_then(Value::as_array)
                .is_some_and(|refs| refs.iter().any(|item| item.as_str() == Some(target_id)))
        })
        .filter_map(|candidate| candidate.get(id_field).and_then(Value::as_str))
        .collect::<Vec<_>>())
}

fn validate_candidate_ref_identity(
    expected: Option<&Value>,
    actual: Option<&Value>,
    field_path: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let expected_refs = string_set(expected);
    let actual_refs = string_set(actual);
    if expected_refs != actual_refs {
        issues.push(issue(
            "ARCHITECTURE_QUALITY_CANDIDATE_LINK_INVALID",
            field_path,
            "Architecture quality links are MCP-derived from the candidate plan and must be preserved exactly.",
        ));
    }
}

fn validate_unique_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    field_path: &str,
    id_field: &str,
    issues: &mut Vec<delivery_core::RepairIssue>,
) {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !id.trim().is_empty() && !seen.insert(id) {
            issues.push(issue(
                "ARCHITECTURE_QUALITY_ID_DUPLICATE",
                field_path,
                &format!("architecture quality {id_field} values must be unique."),
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

fn write_private_section_outputs(
    project_root: &str,
    request_id: &str,
    section_outputs: &[SectionStateOutput],
) -> Result<(), state::store::StateError> {
    let paths = state::paths::project_paths(project_root)?;
    let relative =
        state::request_manifest::request_storage_ref(&paths.root, request_id, "sectionOutputs")?
            .ok_or_else(|| {
                state::store::StateError::StateCorrupted(format!(
                    "request {request_id} is missing private sectionOutputs storage"
                ))
            })?;
    let file = state::paths::from_project_relative(&paths.root, &relative)?;
    state::store::write_json_atomic(&file, &section_outputs)
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
    project_root: &str,
    request_id: &str,
    mut root: Value,
    next_section: ArchitectureSectionGroup,
    next_output: &SectionStateOutput,
    include_repair_context: bool,
    include_repair_source_ref: bool,
    source_refs: &Value,
    section_outputs: &mut [SectionStateOutput],
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
    let next_output_value =
        serde_json::to_value(next_output).map_err(state::store::StateError::Json)?;
    let section_output = section_outputs
        .iter_mut()
        .find(|output| output.section == next_section)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "architecture request sectionOutputs is missing {}",
                section_name(next_section)
            ))
        })?;
    *section_output = next_output.clone();
    if let Some(inline_section_outputs) =
        root.get_mut("sectionOutputs").and_then(Value::as_array_mut)
    {
        let next_section_value =
            serde_json::to_value(next_section).map_err(state::store::StateError::Json)?;
        let inline_output = inline_section_outputs
            .iter_mut()
            .find(|output| output.get("section") == Some(&next_section_value))
            .ok_or_else(|| {
                state::store::StateError::StateCorrupted(format!(
                    "architecture request sectionOutputs is missing {}",
                    section_name(next_section)
                ))
            })?;
        *inline_output = next_output_value.clone();
    } else {
        write_private_section_outputs(project_root, request_id, section_outputs)?;
    }
    root["currentSectionContract"] = next_output_value;
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

fn apply_api_quality_seed(root: &mut Value, seed: &Value) {
    let Some(object) = root.as_object_mut() else {
        return;
    };
    if seed.is_null() {
        object.remove("apiQualitySeed");
        if let Some(enum_refs) = object.get_mut("enumRefs").and_then(Value::as_object_mut) {
            enum_refs.remove("apiQuality");
        }
        return;
    }
    object.insert("apiQualitySeed".to_string(), seed.clone());
    if let Some(enum_refs) = object.get_mut("enumRefs").and_then(Value::as_object_mut) {
        enum_refs.insert("apiQuality".to_string(), contracts::api_quality_enum_refs());
    }
}

fn apply_architecture_quality_seed(root: &mut Value, seed: &Value) {
    if let Some(object) = root.as_object_mut() {
        object.insert("architectureQualitySeed".to_string(), seed.clone());
    }
}

fn build_architecture_quality_candidate_plan(
    project_root: &Path,
    section_outputs: &[SectionStateOutput],
) -> Result<Value, state::store::StateError> {
    let foundation = read_section_candidate_content(
        project_root,
        section_outputs,
        ArchitectureSectionGroup::Foundation,
    )?;
    let domain = read_section_candidate_content(
        project_root,
        section_outputs,
        ArchitectureSectionGroup::DomainContract,
    )?;
    let behavior = read_section_candidate_content(
        project_root,
        section_outputs,
        ArchitectureSectionGroup::Behavior,
    )?;
    let runtime = read_section_candidate_content(
        project_root,
        section_outputs,
        ArchitectureSectionGroup::RuntimeDelivery,
    )?;
    Ok(architecture_quality_candidate_plan_from_sections(
        &foundation,
        &domain,
        &behavior,
        &runtime,
    ))
}

fn architecture_quality_candidate_plan_from_sections(
    foundation: &Value,
    domain: &Value,
    behavior: &Value,
    runtime: &Value,
) -> Value {
    let module_refs = ids_at(&foundation, "/modules", "moduleId");
    let interface_refs = ids_at(&domain, "/interfaces", "interfaceId");
    let entity_refs = ids_at(&domain, "/dataModel/entities", "entityId");
    let state_machine_refs = ids_at(&behavior, "/stateMachines", "machineId");
    let interactions = foundation
        .pointer("/engineeringBoundary/applicationInteractions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let runtime_surfaces = runtime
        .pointer("/runtimeDelivery/runtimeSurfaces")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let runtime_dependencies = runtime
        .pointer("/runtimeDelivery/runtimeDependencies")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let pattern_id = foundation
        .pointer("/engineeringBoundary/patternDecision/patternId")
        .and_then(Value::as_str)
        .unwrap_or("current_phase_structure");
    let has_auth_boundary = interactions.iter().any(|interaction| {
        matches!(
            interaction
                .pointer("/qualityTraits/authRequirement")
                .and_then(Value::as_str),
            Some("required" | "optional" | "deferred_with_risk")
        )
    });
    let has_async_boundary = interactions.iter().any(|interaction| {
        matches!(
            interaction.get("interactionType").and_then(Value::as_str),
            Some("event" | "job")
        )
    }) || matches!(
        pattern_id,
        "event_driven" | "background_worker" | "serverless_function"
    );
    let has_pagination = domain
        .get("interfaces")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|interface| {
            interface
                .pointer("/paginationPolicy/strategy")
                .and_then(Value::as_str)
                .is_some_and(|strategy| !matches!(strategy, "" | "not_applicable"))
        });
    let source_refs = architecture_candidate_source_refs([foundation, domain, behavior, runtime]);
    let declared_module_refs = module_refs.iter().cloned().collect::<BTreeSet<_>>();
    let data_owner_module_refs = named_string_values(
        domain
            .pointer("/dataModel/dataArchitecture")
            .unwrap_or(&Value::Null),
        "ownerModuleRef",
    )
    .into_iter()
    .filter(|module_ref| declared_module_refs.contains(module_ref))
    .collect::<Vec<_>>();
    let interaction_owner_module_refs = named_string_values(
        foundation
            .pointer("/engineeringBoundary/applicationInteractions")
            .unwrap_or(&Value::Null),
        "providerModuleRef",
    )
    .into_iter()
    .filter(|module_ref| declared_module_refs.contains(module_ref))
    .collect::<Vec<_>>();
    let default_owners = json!({
        "modules": module_refs,
        "interfaces": []
    });
    let data_owners = json!({
        "modules": if data_owner_module_refs.is_empty() {
            default_owners["modules"].clone()
        } else {
            json!(data_owner_module_refs)
        },
        "interfaces": []
    });
    let interface_owners = json!({
        "modules": interaction_owner_module_refs,
        "interfaces": interface_refs
    });

    let mut decisions = vec![architecture_decision_candidate(
        "adr-architecture-style",
        "architecture_style",
        format!("Foundation selected the current-phase structural pattern {pattern_id}."),
        default_owners.clone(),
        source_refs.clone(),
    )];
    if default_owners["modules"]
        .as_array()
        .is_some_and(|items| items.len() > 1)
    {
        decisions.push(architecture_decision_candidate(
            "adr-module-boundary",
            "module_boundary",
            "Multiple current-phase modules require explicit responsibility and dependency boundaries.",
            default_owners.clone(),
            source_refs.clone(),
        ));
    }
    if !entity_refs.is_empty() {
        decisions.push(architecture_decision_candidate(
            "adr-data-boundary",
            "data_boundary",
            "The current phase owns persistent or stateful entities and must preserve source-of-truth, invariant, and transaction boundaries.",
            data_owners.clone(),
            source_refs.clone(),
        ));
    }
    if !interactions.is_empty() {
        decisions.push(architecture_decision_candidate(
            "adr-integration-boundary",
            "integration_boundary",
            "Structured application interactions require an explicit provider, consumer, protocol, and failure boundary.",
            interface_owners.clone(),
            source_refs.clone(),
        ));
    }
    if !runtime_surfaces.is_empty() {
        decisions.push(architecture_decision_candidate(
            "adr-runtime-boundary",
            "runtime_boundary",
            "Accepted runtime surfaces require a concrete build, start, probe, and environment boundary.",
            default_owners.clone(),
            source_refs.clone(),
        ));
    }
    if has_auth_boundary {
        decisions.push(architecture_decision_candidate(
            "adr-security-boundary",
            "security_boundary",
            "At least one accepted interaction declares authentication or authorization behavior.",
            interface_owners.clone(),
            source_refs.clone(),
        ));
    }
    if has_async_boundary || !runtime_dependencies.is_empty() {
        decisions.push(architecture_decision_candidate(
            "adr-operability",
            "operability",
            "Asynchronous work or runtime dependencies require observable failure and recovery behavior.",
            default_owners.clone(),
            source_refs.clone(),
        ));
    }
    let mut nfrs = vec![architecture_nfr_candidate(
        "nfr-maintainability",
        "maintainability",
        "derived_minimum",
        "Current-phase module and interface ownership must remain traceable in implementation and review.",
        source_refs.clone(),
        default_owners.clone(),
        candidate_ids_for_categories(
            &decisions,
            "decisionId",
            &["architecture_style", "module_boundary"],
        ),
    )];
    if !entity_refs.is_empty() || !state_machine_refs.is_empty() {
        nfrs.push(architecture_nfr_candidate(
            "nfr-reliability",
            "reliability",
            "derived_minimum",
            "Stateful behavior requires verifiable invariant, transaction, and recovery boundaries.",
            source_refs.clone(),
            data_owners.clone(),
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["data_boundary", "architecture_style"],
            ),
        ));
    }
    if has_pagination {
        nfrs.push(architecture_nfr_candidate(
            "nfr-scalability",
            "scalability",
            "derived_minimum",
            "A growing collection contract requires bounded reads under the accepted pagination policy.",
            source_refs.clone(),
            interface_owners.clone(),
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["data_boundary", "integration_boundary"],
            ),
        ));
    }
    if !runtime_dependencies.is_empty() {
        nfrs.push(architecture_nfr_candidate(
            "nfr-availability",
            "availability",
            "derived_minimum",
            "Runtime dependencies require explicit degraded or unavailable behavior.",
            source_refs.clone(),
            default_owners.clone(),
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["runtime_boundary", "operability"],
            ),
        ));
    }
    if has_auth_boundary {
        nfrs.push(architecture_nfr_candidate(
            "nfr-security",
            "security",
            "derived_minimum",
            "Protected interactions require server-enforced access and safe failure disclosure.",
            source_refs.clone(),
            interface_owners.clone(),
            candidate_ids_for_categories(&decisions, "decisionId", &["security_boundary"]),
        ));
    }
    if has_async_boundary || !runtime_dependencies.is_empty() {
        nfrs.push(architecture_nfr_candidate(
            "nfr-observability",
            "observability",
            "derived_minimum",
            "Asynchronous or dependency failure paths require an observable completion or failure signal.",
            source_refs.clone(),
            default_owners.clone(),
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["integration_boundary", "runtime_boundary", "operability"],
            ),
        ));
    }

    let mut risks = Vec::new();
    if !entity_refs.is_empty() || !state_machine_refs.is_empty() {
        risks.push(architecture_risk_candidate(
            "risk-data-integrity",
            "data_integrity",
            "Stateful writes can violate invariants, become partial, or expose stale state without the declared ownership and transaction rules.",
            data_owners.clone(),
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["data_boundary", "architecture_style"],
            ),
            candidate_ids_for_categories(&nfrs, "nfrId", &["reliability"]),
        ));
    }
    if !interactions.is_empty() {
        risks.push(architecture_risk_candidate(
            "risk-integration",
            "integration",
            "Provider, consumer, or protocol failure can leave the current operation incomplete or inconsistent.",
            interface_owners.clone(),
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["integration_boundary"],
            ),
            candidate_ids_for_categories(
                &nfrs,
                "nfrId",
                &["reliability", "observability"],
            ),
        ));
    }
    if !runtime_surfaces.is_empty() {
        risks.push(architecture_risk_candidate(
            "risk-runtime",
            "runtime",
            "Build or start success can still leave an accepted runtime surface or dependency unreachable.",
            default_owners.clone(),
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["runtime_boundary", "operability"],
            ),
            candidate_ids_for_categories(
                &nfrs,
                "nfrId",
                &["availability", "observability"],
            ),
        ));
    }
    if has_auth_boundary {
        risks.push(architecture_risk_candidate(
            "risk-security",
            "security",
            "A protected operation can be exposed or disclose sensitive state when access checks and failure behavior drift.",
            interface_owners.clone(),
            candidate_ids_for_categories(&decisions, "decisionId", &["security_boundary"]),
            candidate_ids_for_categories(&nfrs, "nfrId", &["security"]),
        ));
    }
    if risks.is_empty() {
        risks.push(architecture_risk_candidate(
            "risk-maintainability",
            "maintainability",
            "Implementation can drift from the accepted module and interface boundary when ownership is not evidenced.",
            default_owners,
            candidate_ids_for_categories(
                &decisions,
                "decisionId",
                &["architecture_style", "module_boundary"],
            ),
            candidate_ids_for_categories(&nfrs, "nfrId", &["maintainability"]),
        ));
    }
    json!({
        "decisionCandidates": decisions,
        "nfrCandidates": nfrs,
        "riskCandidates": risks
    })
}

fn enrich_architecture_artifact_refs(
    project_root: &Path,
    section_outputs: &[SectionStateOutput],
    allowed_refs: &mut Value,
) -> Result<(), state::store::StateError> {
    let foundation = read_section_candidate_content(
        project_root,
        section_outputs,
        ArchitectureSectionGroup::Foundation,
    )?;
    let domain = read_section_candidate_content(
        project_root,
        section_outputs,
        ArchitectureSectionGroup::DomainContract,
    )?;
    allowed_refs["moduleRefs"] = json!(ids_at(&foundation, "/modules", "moduleId"));
    allowed_refs["interfaceRefs"] = json!(ids_at(&domain, "/interfaces", "interfaceId"));
    Ok(())
}

fn read_section_candidate_content(
    project_root: &Path,
    section_outputs: &[SectionStateOutput],
    section: ArchitectureSectionGroup,
) -> Result<Value, state::store::StateError> {
    let output = section_outputs
        .iter()
        .find(|output| output.section == section)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(format!(
                "architecture request is missing {} output",
                section_name(section)
            ))
        })?;
    let path = from_project_relative(project_root, &output.candidate_file)?;
    let candidate: ArchitectureSectionCandidateAgentWritable = state::store::read_json(&path)?;
    Ok(candidate.content)
}

fn ids_at(value: &Value, pointer: &str, id_field: &str) -> Vec<String> {
    let mut ids = value
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(id_field).and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn named_string_values(value: &Value, key: &str) -> Vec<String> {
    let mut values = BTreeSet::new();
    collect_named_string_values(value, key, &mut values);
    values.into_iter().collect()
}

fn collect_named_string_values(value: &Value, key: &str, values: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (field, child) in object {
                if field == key {
                    if let Some(value) = child.as_str().filter(|value| !value.trim().is_empty()) {
                        values.insert(value.to_string());
                    }
                }
                collect_named_string_values(child, key, values);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_named_string_values(item, key, values);
            }
        }
        _ => {}
    }
}

fn candidate_ids_for_categories(
    candidates: &[Value],
    id_field: &str,
    categories: &[&str],
) -> Vec<Value> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate
                .get("category")
                .and_then(Value::as_str)
                .is_some_and(|category| categories.contains(&category))
        })
        .filter_map(|candidate| candidate.get(id_field).cloned())
        .collect()
}

fn architecture_candidate_source_refs<'a>(values: impl IntoIterator<Item = &'a Value>) -> Value {
    let mut scope_refs = BTreeSet::new();
    let mut acceptance_refs = BTreeSet::new();
    let mut detail_refs = BTreeSet::new();
    for value in values {
        collect_named_string_refs(
            value,
            &mut scope_refs,
            &mut acceptance_refs,
            &mut detail_refs,
        );
    }
    json!({
        "scopeRefs": scope_refs.into_iter().collect::<Vec<_>>(),
        "acceptanceRefs": acceptance_refs.into_iter().collect::<Vec<_>>(),
        "requirementDetailRefs": detail_refs.into_iter().collect::<Vec<_>>()
    })
}

fn collect_named_string_refs(
    value: &Value,
    scope_refs: &mut BTreeSet<String>,
    acceptance_refs: &mut BTreeSet<String>,
    detail_refs: &mut BTreeSet<String>,
) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let target = match key.as_str() {
                    "scopeRefs" => Some(&mut *scope_refs),
                    "acceptanceRefs" => Some(&mut *acceptance_refs),
                    "requirementDetailRefs" => Some(&mut *detail_refs),
                    _ => None,
                };
                if let Some(target) = target {
                    if let Some(items) = child.as_array() {
                        target.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
                    }
                }
                collect_named_string_refs(child, scope_refs, acceptance_refs, detail_refs);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_named_string_refs(item, scope_refs, acceptance_refs, detail_refs);
            }
        }
        _ => {}
    }
}

fn architecture_decision_candidate(
    decision_id: &str,
    category: &str,
    reason: impl Into<String>,
    owner_artifact_refs: Value,
    source_refs: Value,
) -> Value {
    json!({
        "decisionId": decision_id,
        "category": category,
        "reason": reason.into(),
        "ownerArtifactRefs": owner_artifact_refs,
        "sourceRefs": source_refs
    })
}

fn architecture_nfr_candidate(
    nfr_id: &str,
    category: &str,
    source: &str,
    reason: &str,
    source_refs: Value,
    owner_artifact_refs: Value,
    decision_refs: Vec<Value>,
) -> Value {
    json!({
        "nfrId": nfr_id,
        "category": category,
        "source": source,
        "reason": reason,
        "sourceRefs": source_refs,
        "ownerArtifactRefs": owner_artifact_refs,
        "decisionRefs": decision_refs
    })
}

fn architecture_risk_candidate(
    risk_id: &str,
    category: &str,
    reason: &str,
    owner_artifact_refs: Value,
    decision_refs: Vec<Value>,
    nfr_refs: Vec<Value>,
) -> Value {
    json!({
        "riskId": risk_id,
        "category": category,
        "reason": reason,
        "ownerArtifactRefs": owner_artifact_refs,
        "decisionRefs": decision_refs,
        "nfrRefs": nfr_refs
    })
}

fn rebuild_domain_contract_output(
    project_root: &Path,
    source_refs: &Value,
    request_root: &Value,
    api_quality_seed: &Value,
    output: &mut SectionStateOutput,
) -> Result<(), state::store::StateError> {
    let planning_ref = source_refs
        .get("planningContractRef")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "architecture request is missing sourceRefs.planningContractRef".to_string(),
            )
        })?;
    let planning_contract: contracts::PlanningGenerationContract =
        read_project_json(project_root, planning_ref)?;
    let has_previous_runtime_delivery = source_refs
        .get("previousRuntimeDeliveryRef")
        .is_some_and(|value| !value.is_null());
    let frontend_experience_source = request_root
        .get("frontendExperienceSource")
        .cloned()
        .unwrap_or_else(|| json!({}));
    output.schema_shape = section_schema_shape(
        ArchitectureSectionGroup::DomainContract,
        has_previous_runtime_delivery,
        api_quality_seed,
    );
    output.result_template = section_result_template(
        ArchitectureSectionGroup::DomainContract,
        has_previous_runtime_delivery,
        &frontend_experience_source,
        &planning_contract,
        api_quality_seed,
    );
    output.enum_refs = section_enum_refs(
        ArchitectureSectionGroup::DomainContract,
        has_previous_runtime_delivery,
        api_quality_seed,
    );
    output.generation_rules = section_generation_rules(
        ArchitectureSectionGroup::DomainContract,
        has_previous_runtime_delivery,
        api_quality_seed,
    );
    Ok(())
}

fn load_project_api_contract(
    project_root: &Path,
    delivery_id: &str,
) -> Result<Option<Value>, state::store::StateError> {
    let path = project_api_contract_file(project_root, delivery_id);
    if !path.exists() {
        return Ok(None);
    }
    state::store::read_json_value(&path).map(Some)
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

    fn foundation_candidate(interaction: Value) -> ArchitectureSectionCandidateAgentWritable {
        ArchitectureSectionCandidateAgentWritable {
            schema_version: String::new(),
            request_id: String::new(),
            delivery_id: String::new(),
            phase_id: String::new(),
            section: ArchitectureSectionGroup::Foundation,
            status: ArchitectureSectionStatus::Ready,
            content: json!({
                "engineeringBoundary": {
                    "applicationInteractions": [interaction]
                }
            }),
            blocked_reasons: vec![],
            created_at: String::new(),
        }
    }

    fn http_interaction(quality_traits: Value) -> Value {
        json!({
            "interactionId": "interaction-orders",
            "providerApplicationRef": "app-api",
            "consumerApplicationRefs": ["app-web"],
            "providerModuleRef": "module-orders",
            "interactionType": "http_api",
            "qualityTraits": quality_traits
        })
    }

    #[test]
    fn complete_http_quality_traits_pass_foundation_validation() {
        let candidate = foundation_candidate(http_interaction(json!({
            "authRequirement": "required",
            "paginationRequired": true,
            "contractArtifactRequired": true,
            "compatibilityRequired": false,
            "operationalPolicies": ["idempotency", "request_id"]
        })));

        assert!(
            validate_structured_communication_boundaries(&candidate).is_empty(),
            "complete structured HTTP quality traits must pass"
        );
    }

    #[test]
    fn incomplete_http_quality_traits_report_exact_field_paths() {
        let candidate = foundation_candidate(http_interaction(json!({
            "contractArtifactRequired": true,
            "compatibilityRequired": false
        })));
        let issues = validate_structured_communication_boundaries(&candidate);
        let issue_keys = issues
            .iter()
            .map(|issue| (issue.code.clone(), issue.field_path.clone()))
            .collect::<BTreeSet<_>>();
        let base = "content.engineeringBoundary.applicationInteractions[0].qualityTraits";

        assert!(issue_keys.contains(&(
            "HTTP_INTERACTION_AUTH_REQUIREMENT_INVALID".to_string(),
            Some(format!("{base}.authRequirement"))
        )));
        assert!(issue_keys.contains(&(
            "HTTP_INTERACTION_QUALITY_TRAIT_BOOLEAN_REQUIRED".to_string(),
            Some(format!("{base}.paginationRequired"))
        )));
        assert!(issue_keys.contains(&(
            "HTTP_INTERACTION_OPERATIONAL_POLICIES_REQUIRED".to_string(),
            Some(format!("{base}.operationalPolicies"))
        )));
    }

    #[test]
    fn invalid_http_quality_trait_values_report_exact_field_paths() {
        let candidate = foundation_candidate(http_interaction(json!({
            "authRequirement": "sometimes",
            "paginationRequired": "yes",
            "contractArtifactRequired": true,
            "compatibilityRequired": false,
            "operationalPolicies": ["circuit_breaker"]
        })));
        let issues = validate_structured_communication_boundaries(&candidate);
        let issue_keys = issues
            .iter()
            .map(|issue| (issue.code.clone(), issue.field_path.clone()))
            .collect::<BTreeSet<_>>();
        let base = "content.engineeringBoundary.applicationInteractions[0].qualityTraits";

        assert!(issue_keys.contains(&(
            "HTTP_INTERACTION_AUTH_REQUIREMENT_INVALID".to_string(),
            Some(format!("{base}.authRequirement"))
        )));
        assert!(issue_keys.contains(&(
            "HTTP_INTERACTION_QUALITY_TRAIT_BOOLEAN_REQUIRED".to_string(),
            Some(format!("{base}.paginationRequired"))
        )));
        assert!(issue_keys.contains(&(
            "HTTP_INTERACTION_OPERATIONAL_POLICY_INVALID".to_string(),
            Some(format!("{base}.operationalPolicies[0]"))
        )));
    }

    #[test]
    fn custom_pattern_decision_keeps_full_structural_obligations() {
        let content = json!({
            "engineeringBoundary": {
                "patternDecision": {
                    "classification": "custom",
                    "patternId": "local-first-sync-engine",
                    "patternName": "Local-first synchronization engine",
                    "composedOf": [],
                    "decisionDrivers": ["Offline mutation and deterministic reconciliation are current-phase behavior."],
                    "structuralRules": ["The local log owns pending writes and the sync adapter owns remote reconciliation."],
                    "rationale": "Known request/response patterns do not describe the accepted offline boundary."
                }
            }
        });
        assert!(validate_pattern_decision(&content).is_empty());
    }

    #[test]
    fn foundation_modules_require_current_phase_ownership() {
        let mut content = json!({
            "modules": [{
                "moduleId": "module-orders",
                "name": "Orders",
                "responsibility": "Own order behavior.",
                "scopeRefs": [],
                "acceptanceRefs": []
            }]
        });
        let issues = validate_foundation_modules(&content);
        assert!(issues
            .iter()
            .any(|issue| issue.code == "ARCHITECTURE_MODULE_SOURCE_REQUIRED"));

        let duplicate = content["modules"][0].clone();
        content["modules"].as_array_mut().unwrap().push(duplicate);
        assert!(validate_foundation_modules(&content)
            .iter()
            .any(|issue| issue.code == "ARCHITECTURE_MODULE_ID_DUPLICATE"));
    }

    #[test]
    fn architecture_quality_candidates_keep_data_and_integration_ownership_specific() {
        let foundation = json!({
            "engineeringBoundary": {
                "patternDecision": {"patternId": "modular_monolith"},
                "applicationInteractions": [{
                    "interactionType": "http_api",
                    "providerModuleRef": "module-notifications",
                    "qualityTraits": {"authRequirement": "required"}
                }]
            },
            "modules": [
                {"moduleId": "module-orders", "scopeRefs": ["scope-orders"]},
                {"moduleId": "module-notifications", "scopeRefs": ["scope-notifications"]}
            ]
        });
        let domain = json!({
            "dataModel": {
                "entities": [{"entityId": "entity-order"}],
                "dataArchitecture": {
                    "ownership": [{"dataRef": "entity-order", "ownerModuleRef": "module-orders"}]
                }
            },
            "interfaces": [{"interfaceId": "api-notifications"}]
        });
        let behavior = json!({"stateMachines": [{"machineId": "machine-order"}]});
        let runtime = json!({
            "runtimeDelivery": {
                "runtimeSurfaces": [{"surfaceId": "surface-api"}],
                "runtimeDependencies": []
            }
        });

        let plan = architecture_quality_candidate_plan_from_sections(
            &foundation,
            &domain,
            &behavior,
            &runtime,
        );
        let candidate = |group: &str, id_field: &str, id: &str| {
            plan[group]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item[id_field] == id)
                .unwrap()
        };

        assert_eq!(
            candidate("decisionCandidates", "decisionId", "adr-data-boundary")["ownerArtifactRefs"]
                ["modules"],
            json!(["module-orders"])
        );
        assert_eq!(
            candidate(
                "decisionCandidates",
                "decisionId",
                "adr-integration-boundary"
            )["ownerArtifactRefs"],
            json!({
                "modules": ["module-notifications"],
                "interfaces": ["api-notifications"]
            })
        );
        assert_eq!(
            candidate("nfrCandidates", "nfrId", "nfr-reliability")["decisionRefs"],
            json!(["adr-architecture-style", "adr-data-boundary"])
        );
        assert_eq!(
            candidate("riskCandidates", "riskId", "risk-data-integrity")["ownerArtifactRefs"]
                ["modules"],
            json!(["module-orders"])
        );
    }

    #[test]
    fn domain_contract_requires_structured_data_architecture() {
        let missing = validate_data_architecture(&json!({"dataModel": {}}));
        assert!(missing
            .iter()
            .any(|issue| issue.code == "DATA_ARCHITECTURE_REQUIRED"));

        let complete = json!({
            "dataModel": {
                "dataArchitecture": {
                    "persistenceMode": "selected_stack",
                    "sourceOfTruth": "primary-order-store",
                    "ownership": [],
                    "invariants": [],
                    "transactionBoundaries": [],
                    "consistencyRules": [],
                    "migrationImpacts": [],
                    "readModels": [],
                    "lifecyclePolicies": [],
                    "derivedData": []
                }
            }
        });
        assert!(validate_data_architecture(&complete).is_empty());
    }

    #[test]
    fn data_architecture_rejects_prose_only_transaction_entries() {
        let content = json!({
            "dataModel": {
                "dataArchitecture": {
                    "persistenceMode": "selected_stack",
                    "sourceOfTruth": "primary-order-store",
                    "ownership": [],
                    "invariants": [],
                    "transactionBoundaries": ["save the order atomically"],
                    "consistencyRules": [],
                    "migrationImpacts": [],
                    "readModels": [],
                    "lifecyclePolicies": [],
                    "derivedData": []
                }
            }
        });
        assert!(validate_data_architecture(&content)
            .iter()
            .any(|issue| issue.code == "DATA_ARCHITECTURE_ENTRY_INVALID"));
    }

    #[test]
    fn runtime_dependency_seed_cannot_be_removed_from_a_modified_delivery() {
        let request_root = json!({
            "runtimeDependencySeed": {
                "candidates": [{
                    "dependencyId": "runtime_persistence",
                    "kind": "storage",
                    "startupRequirement": "required"
                }]
            }
        });
        let mut issues = Vec::new();
        validate_runtime_dependency_seed(
            &json!({"runtimeDependencies": []}),
            &request_root,
            &mut issues,
        );
        assert_eq!(issues[0].code, "RUNTIME_DEPENDENCY_SEED_UNSATISFIED");

        issues.clear();
        validate_runtime_dependency_seed(
            &json!({
                "runtimeDependencies": [{
                    "dependencyId": "runtime_persistence",
                    "kind": "storage",
                    "startupRequirement": "required"
                }]
            }),
            &request_root,
            &mut issues,
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn behavior_contract_rejects_incomplete_paths_and_transitions() {
        let content = json!({
            "userFlows": [{
                "flowId": "flow-orders",
                "name": "Submit order",
                "trigger": "Staff submits an order.",
                "actorRef": "staff",
                "happyPath": [{"stepId": "step-submit", "action": "Submit", "stateEffects": []}],
                "businessBlockingPaths": [],
                "failurePaths": [{"failure": "storage unavailable"}],
                "successOutcome": "The order is visible."
            }],
            "stateMachines": [{
                "machineId": "machine-order",
                "name": "Order lifecycle",
                "states": [],
                "transitions": [{"from": "draft", "to": "submitted", "trigger": "submit"}]
            }]
        });
        let issues = validate_behavior_contract(&content);
        assert!(issues
            .iter()
            .any(|issue| issue.code == "BEHAVIOR_HAPPY_PATH_INVALID"));
        assert!(issues
            .iter()
            .any(|issue| issue.code == "BEHAVIOR_FAILURE_PATH_INVALID"));
        assert!(issues
            .iter()
            .any(|issue| issue.code == "STATE_TRANSITION_INVALID"));
    }

    fn complete_architecture_quality() -> Value {
        json!({
            "decisions": [{
                "decisionId": "adr-runtime",
                "category": "runtime_boundary",
                "title": "Keep the runtime boundary explicit",
                "status": "accepted",
                "context": "The current phase owns one runtime surface and one dependency.",
                "decision": "Keep startup and dependency recovery in the owning module.",
                "alternativesConsidered": [{
                    "name": "Implicit framework startup",
                    "tradeoff": "Less configuration but no explicit recovery boundary.",
                    "rejectedBecause": "The dependency failure must be observable."
                }],
                "consequences": {
                    "positive": ["Startup ownership is verifiable."],
                    "negative": ["The module owns additional recovery code."],
                    "neutral": []
                },
                "sourceRefs": {
                    "scopeRefs": ["scope-1"],
                    "acceptanceRefs": [],
                    "requirementDetailRefs": []
                },
                "ownerArtifactRefs": {
                    "modules": ["module-1"],
                    "interfaces": []
                },
                "verificationHints": ["Exercise dependency-unavailable startup behavior."]
            }],
            "nfrs": [{
                "nfrId": "nfr-availability",
                "category": "availability",
                "source": "derived_minimum",
                "target": "Dependency failure produces a deterministic unavailable state.",
                "rationale": "Silent startup would expose a broken runtime surface.",
                "measurement": {
                    "indicator": "Unavailable state is observable.",
                    "workloadOrCondition": "Required dependency cannot be reached.",
                    "evaluationBoundary": "Runtime dependency test."
                },
                "sourceRefs": {
                    "scopeRefs": ["scope-1"],
                    "acceptanceRefs": [],
                    "requirementDetailRefs": []
                },
                "architectureRefs": {"decisions": ["adr-runtime"], "risks": ["risk-runtime"]},
                "ownerArtifactRefs": {"modules": ["module-1"], "interfaces": []},
                "verificationStrategy": "Run the dependency-unavailable runtime test."
            }],
            "risks": [{
                "riskId": "risk-runtime",
                "category": "runtime",
                "severity": "high",
                "likelihood": "medium",
                "impact": "The runtime surface accepts traffic while unusable.",
                "mitigation": "Fail startup or expose a deterministic unavailable state.",
                "ownerArtifactRefs": {
                    "modules": ["module-1"],
                    "interfaces": [],
                    "decisions": ["adr-runtime"],
                    "nfrs": ["nfr-availability"]
                },
                "verificationHints": ["Verify the declared unavailable behavior."]
            }]
        })
    }

    #[test]
    fn architecture_quality_accepts_availability_with_complete_ownership() {
        let mut issues = Vec::new();
        validate_architecture_quality(
            Some(&complete_architecture_quality()),
            &json!({
                "scopeRefs": ["scope-1"],
                "acceptanceRefs": [],
                "requirementDetailIds": [],
                "moduleRefs": ["module-1"],
                "interfaceRefs": []
            }),
            &mut issues,
        );
        assert!(
            issues.is_empty(),
            "complete architecture quality: {issues:?}"
        );
    }

    #[test]
    fn architecture_quality_reports_duplicate_ids_and_empty_tradeoffs_together() {
        let mut quality = complete_architecture_quality();
        let duplicate = quality["decisions"][0].clone();
        quality["decisions"].as_array_mut().unwrap().push(duplicate);
        quality["decisions"][0]["alternativesConsidered"][0]["tradeoff"] = json!("");
        quality["decisions"][0]["consequences"]["negative"] = json!([]);
        let mut issues = Vec::new();
        validate_architecture_quality(
            Some(&quality),
            &json!({
                "scopeRefs": ["scope-1"],
                "acceptanceRefs": [],
                "requirementDetailIds": [],
                "moduleRefs": ["module-1"],
                "interfaceRefs": []
            }),
            &mut issues,
        );
        let codes = issues
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<BTreeSet<_>>();
        assert!(codes.contains("ARCHITECTURE_QUALITY_ID_DUPLICATE"));
        assert!(codes.contains("ARCHITECTURE_QUALITY_INCOMPLETE"));
    }

    #[test]
    fn confirmed_nfr_requires_acceptance_or_requirement_detail_source() {
        let mut quality = complete_architecture_quality();
        quality["nfrs"][0]["source"] = json!("confirmed_requirement");
        let mut issues = Vec::new();
        validate_architecture_quality(
            Some(&quality),
            &json!({
                "scopeRefs": ["scope-1"],
                "acceptanceRefs": [],
                "requirementDetailIds": [],
                "moduleRefs": ["module-1"],
                "interfaceRefs": []
            }),
            &mut issues,
        );
        assert!(issues
            .iter()
            .any(|issue| issue.code == "ARCHITECTURE_QUALITY_CONFIRMED_SOURCE_INVALID"));
    }

    #[test]
    fn coverage_quality_must_complete_the_mcp_candidate_plan_exactly() {
        let content = json!({"architectureQuality": complete_architecture_quality()});
        let plan = json!({
            "decisionCandidates": [{
                "decisionId": "adr-runtime",
                "category": "runtime_boundary",
                "ownerArtifactRefs": {"modules": ["module-1"], "interfaces": []}
            }],
            "nfrCandidates": [{
                "nfrId": "nfr-availability",
                "category": "availability",
                "source": "derived_minimum",
                "sourceRefs": {"scopeRefs": ["scope-1"], "acceptanceRefs": [], "requirementDetailRefs": []},
                "decisionRefs": ["adr-runtime"],
                "ownerArtifactRefs": {"modules": ["module-1"], "interfaces": []}
            }],
            "riskCandidates": [{
                "riskId": "risk-runtime",
                "category": "runtime",
                "decisionRefs": ["adr-runtime"],
                "nfrRefs": ["nfr-availability"],
                "ownerArtifactRefs": {"modules": ["module-1"], "interfaces": []}
            }]
        });
        assert!(validate_architecture_quality_candidate_plan(&content, Some(&plan)).is_empty());

        let incomplete = json!({
            "architectureQuality": {
                "decisions": content["architectureQuality"]["decisions"].clone(),
                "nfrs": content["architectureQuality"]["nfrs"].clone(),
                "risks": []
            }
        });
        assert!(
            validate_architecture_quality_candidate_plan(&incomplete, Some(&plan))
                .iter()
                .any(|issue| issue.code == "ARCHITECTURE_QUALITY_CANDIDATE_COVERAGE_INVALID")
        );

        let mut changed_source = content;
        changed_source["architectureQuality"]["nfrs"][0]["sourceRefs"]["scopeRefs"] = json!([]);
        assert!(
            validate_architecture_quality_candidate_plan(&changed_source, Some(&plan))
                .iter()
                .any(|issue| issue.code == "ARCHITECTURE_QUALITY_CANDIDATE_SOURCE_INVALID")
        );

        let mut changed_link = json!({"architectureQuality": complete_architecture_quality()});
        changed_link["architectureQuality"]["nfrs"][0]["architectureRefs"]["risks"] = json!([]);
        assert!(
            validate_architecture_quality_candidate_plan(&changed_link, Some(&plan))
                .iter()
                .any(|issue| issue.code == "ARCHITECTURE_QUALITY_CANDIDATE_LINK_INVALID")
        );
    }

    #[test]
    fn coverage_normalization_restores_mcp_owned_candidate_fields() {
        let mut content = json!({"architectureQuality": complete_architecture_quality()});
        content["architectureQuality"]["decisions"][0]["decisionId"] = json!("agent-id");
        content["architectureQuality"]["decisions"][0]["category"] = json!("agent-category");
        content["architectureQuality"]["nfrs"][0]["sourceRefs"] = json!({});
        content["architectureQuality"]["nfrs"][0]["architectureRefs"] = json!({});
        content["architectureQuality"]["risks"][0]["ownerArtifactRefs"] = json!({});
        let semantic_title = content["architectureQuality"]["decisions"][0]["title"].clone();
        let plan = json!({
            "decisionCandidates": [{
                "decisionId": "adr-runtime",
                "category": "runtime_boundary",
                "sourceRefs": {"scopeRefs": ["scope-1"], "acceptanceRefs": [], "requirementDetailRefs": []},
                "ownerArtifactRefs": {"modules": ["module-1"], "interfaces": []}
            }],
            "nfrCandidates": [{
                "nfrId": "nfr-availability",
                "category": "availability",
                "source": "derived_minimum",
                "sourceRefs": {"scopeRefs": ["scope-1"], "acceptanceRefs": [], "requirementDetailRefs": []},
                "decisionRefs": ["adr-runtime"],
                "ownerArtifactRefs": {"modules": ["module-1"], "interfaces": []}
            }],
            "riskCandidates": [{
                "riskId": "risk-runtime",
                "category": "runtime",
                "decisionRefs": ["adr-runtime"],
                "nfrRefs": ["nfr-availability"],
                "ownerArtifactRefs": {"modules": ["module-1"], "interfaces": []}
            }]
        });

        normalize_architecture_quality_candidate_fields(Some(&mut content), Some(&plan));

        assert_eq!(
            content["architectureQuality"]["decisions"][0]["decisionId"],
            json!("adr-runtime")
        );
        assert_eq!(
            content["architectureQuality"]["decisions"][0]["title"],
            semantic_title
        );
        assert_eq!(
            content["architectureQuality"]["nfrs"][0]["architectureRefs"],
            json!({"decisions": ["adr-runtime"], "risks": ["risk-runtime"]})
        );
        assert_eq!(
            content["architectureQuality"]["risks"][0]["ownerArtifactRefs"],
            json!({
                "modules": ["module-1"],
                "interfaces": [],
                "decisions": ["adr-runtime"],
                "nfrs": ["nfr-availability"]
            })
        );
        assert!(validate_architecture_quality_candidate_plan(&content, Some(&plan)).is_empty());
    }

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

    #[test]
    fn advancing_architecture_sections_updates_the_private_section_output() {
        let next_output = SectionStateOutput {
            section: ArchitectureSectionGroup::DomainContract,
            candidate_file: ".loom/agent-writable/domain.json".to_string(),
            schema_ref: "domain-schema".to_string(),
            schema_shape: json!({"apiEnabled": true}),
            result_template: json!({"content": {"interfaces": []}}),
            enum_refs: json!({"apiQuality": {"httpMethod": ["GET"]}}),
            generation_rules: vec!["Use the current API contract.".to_string()],
        };
        let root = json!({
            "sectionState": {
                "currentSection": "foundation",
                "completedSections": []
            },
            "sectionOutputs": [
                {"section": "foundation", "schemaShape": {"apiEnabled": false}},
                {"section": "domain_contract", "schemaShape": {"apiEnabled": false}}
            ],
            "currentSectionContract": {},
            "outputContract": {
                "schemaProjection": {"requiredContentKeys": []}
            },
            "frontendExperienceSource": {},
            "apiQualitySeed": null,
            "requestReadPlan": {"groups": []}
        });
        let mut section_outputs = vec![
            SectionStateOutput {
                section: ArchitectureSectionGroup::Foundation,
                candidate_file: ".loom/agent-writable/foundation.json".to_string(),
                schema_ref: "foundation-schema".to_string(),
                schema_shape: json!({"apiEnabled": false}),
                result_template: json!({}),
                enum_refs: json!({}),
                generation_rules: vec![],
            },
            next_output.clone(),
        ];
        let updated = update_request_for_next_section(
            "/tmp/project",
            "arch_test",
            root,
            ArchitectureSectionGroup::DomainContract,
            &next_output,
            false,
            false,
            &json!({}),
            &mut section_outputs,
        )
        .expect("advance architecture section");

        assert_eq!(
            updated["sectionOutputs"][1]["schemaShape"],
            json!({"apiEnabled": true})
        );
        assert_eq!(
            updated["currentSectionContract"]["schemaShape"],
            json!({"apiEnabled": true})
        );
    }
}

fn update_output_contract_ref(
    project_root: &str,
    request_id: &str,
    next_section: ArchitectureSectionGroup,
    next_output: &SectionStateOutput,
    field_policies: &std::collections::BTreeMap<String, delivery_core::AgentFieldPolicy>,
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
    delivery_core::finalize_output_contract(&mut output_contract, field_policies);
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
    let runtime_delivery = runtime.content.get("runtimeDelivery").cloned();
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
        api_contract_ref: None,
        current_phase_interface_refs: vec![],
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

fn materialize_project_api_contract(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
    phase_interfaces: &[Value],
    outputs: &[SectionStateOutput],
) -> Result<Option<(String, Value)>, state::store::StateError> {
    let current_http_interfaces = phase_interfaces
        .iter()
        .filter(|interface| interface.get("type").and_then(Value::as_str) == Some("http_api"))
        .cloned()
        .collect::<Vec<_>>();
    let existing = load_project_api_contract(project_root, delivery_id)?;
    if current_http_interfaces.is_empty() && existing.is_none() {
        return Ok(None);
    }

    let domain_output = outputs
        .iter()
        .find(|output| output.section == ArchitectureSectionGroup::DomainContract)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "architecture request is missing domain_contract output".to_string(),
            )
        })?;
    let domain_file = from_project_relative(project_root, &domain_output.candidate_file)?;
    let domain: ArchitectureSectionCandidateAgentWritable = state::store::read_json(&domain_file)?;
    let runtime_output = outputs
        .iter()
        .find(|output| output.section == ArchitectureSectionGroup::RuntimeDelivery)
        .ok_or_else(|| {
            state::store::StateError::StateCorrupted(
                "architecture request is missing runtime_delivery output".to_string(),
            )
        })?;
    let runtime_file = from_project_relative(project_root, &runtime_output.candidate_file)?;
    let runtime: ArchitectureSectionCandidateAgentWritable =
        state::store::read_json(&runtime_file)?;
    let current_exposure = normalize_api_contract(
        domain.content.get("apiContract"),
        &current_http_interfaces,
        runtime.content.get("runtimeDelivery"),
    );

    let mut interfaces_by_id = BTreeMap::<String, Value>::new();
    if let Some(existing_interfaces) = existing
        .as_ref()
        .and_then(|value| value.get("interfaces"))
        .and_then(Value::as_array)
    {
        for interface in existing_interfaces {
            if let Some(interface_id) = interface.get("interfaceId").and_then(Value::as_str) {
                interfaces_by_id.insert(interface_id.to_string(), interface.clone());
            }
        }
    }
    for interface in current_http_interfaces {
        let Some(interface_id) = interface.get("interfaceId").and_then(Value::as_str) else {
            continue;
        };
        interfaces_by_id.insert(interface_id.to_string(), interface);
    }
    let public_exposure = current_exposure
        .as_ref()
        .and_then(|value| value.get("publicExposure"))
        .cloned()
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|value| value.get("publicExposure"))
                .cloned()
        })
        .unwrap_or_else(|| json!({}));
    let browser_binding = current_exposure
        .as_ref()
        .and_then(|value| value.get("browserBinding"))
        .cloned()
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|value| value.get("browserBinding"))
                .cloned()
        })
        .unwrap_or_else(|| json!({}));
    let path = project_api_contract_file(project_root, delivery_id);
    let contract_ref = to_project_relative(project_root, &path)?;
    let contract = json!({
        "schemaVersion": "1.0",
        "apiContractId": format!("api_{}_{}", delivery_id, state::store::now_millis()),
        "deliveryId": delivery_id,
        "currentPhaseId": phase_id,
        "interfaces": interfaces_by_id.into_values().collect::<Vec<_>>(),
        "publicExposure": public_exposure,
        "browserBinding": browser_binding,
        "updatedAt": state::store::now_string()
    });
    Ok(Some((contract_ref, contract)))
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
        stop_allowed: false,
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
        agent_instruction: delivery_core::repairable_error_agent_instruction(mode.resubmit_tool()),
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
