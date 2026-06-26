use std::path::Path;

use contracts::{ClarificationBlockName, UserFacingLanguageConstraint};
use delivery_core::{
    ArtifactKind, LoomMcpActionResult, LoomMcpAutoRunnableResult, LoomMcpFailure,
    LoomMcpFailureResult, LoomMcpNextAction, LoomMcpUserGateResult, RouteAction, RouteActionKind,
    TransitionStore, WriteArtifactNext, WriteMode, WriteTarget,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state::{
    lifecycle_store::FileTransitionStore,
    paths::{from_project_relative, to_project_relative, DeliveryPhaseLocator},
    store::{ensure_dir, read_json, write_json_atomic, StateError, StateResult},
};

use crate::{
    gate::{
        block_message, required_knowledge_step_ids, to_value, BrainstormGate,
        BrainstormResponseRule,
    },
    paths::{brainstorm_agent_candidate_file, brainstorm_clarification_state_file},
    request::{
        build_brainstorm_candidate_write_request_root, build_brainstorm_clarification_request_root,
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormConfirmBlockInput {
    pub project_root: String,
    pub request_ref: String,
    pub block: ClarificationBlockName,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_data: Option<Value>,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationState {
    pub schema_version: String,
    pub delivery_id: String,
    pub phase_id: String,
    pub brainstorm_run_id: String,
    pub current_block: ClarificationBlockName,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<ConfirmedClarificationBlock>,
    pub final_summary_confirmed: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmedClarificationBlock {
    pub block: ClarificationBlockName,
    pub summary: String,
    pub confirmed_by_user: bool,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_data: Option<Value>,
    pub confirmed_at: String,
}

pub fn initial_state(
    delivery_id: &str,
    phase_id: &str,
    brainstorm_run_id: &str,
) -> ClarificationState {
    ClarificationState {
        schema_version: "1.0".to_string(),
        delivery_id: delivery_id.to_string(),
        phase_id: phase_id.to_string(),
        brainstorm_run_id: brainstorm_run_id.to_string(),
        current_block: ClarificationBlockName::PhaseScope,
        blocks: vec![],
        final_summary_confirmed: false,
        updated_at: state::store::now_string(),
    }
}

pub fn confirm_block(input: BrainstormConfirmBlockInput) -> LoomMcpActionResult {
    match confirm_block_inner(input) {
        Ok(result) => result,
        Err(error) => {
            let recovery_tool = if error.code == "BRAINSTORM_KNOWLEDGE_CONTEXT_REQUIRED" {
                Some("loom.knowledgeBrainstormContext".to_string())
            } else {
                Some("loom.continue".to_string())
            };
            LoomMcpActionResult::Failed(LoomMcpFailureResult {
                project_root: error.project_root,
                error: LoomMcpFailure {
                    code: error.code,
                    message: error.message,
                    target_batch: Some(7),
                    domain: Some("brainstorm".to_string()),
                    route_action: Some("brainstorm_confirm_block".to_string()),
                    recovery_tool,
                },
            })
        }
    }
}

pub fn materialize_confirmation_request(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> LoomMcpActionResult {
    match materialize_confirmation_request_inner(project_root, delivery_id, phase_id) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: project_root.to_string(),
            error: LoomMcpFailure {
                code: "BRAINSTORM_CONFIRMATION_REQUEST_FAILED".to_string(),
                message: error.to_string(),
                target_batch: Some(7),
                domain: Some("brainstorm".to_string()),
                route_action: Some("brainstorm_confirmation".to_string()),
                recovery_tool: Some("loom.continue".to_string()),
            },
        }),
    }
}

fn confirm_block_inner(
    input: BrainstormConfirmBlockInput,
) -> Result<LoomMcpActionResult, ConfirmError> {
    let project_root = input.project_root.clone();
    let request_id = parse_request_id(&input.request_ref).map_err(|message| ConfirmError {
        project_root: project_root.clone(),
        code: "INVALID_REQUEST_REF".to_string(),
        message,
    })?;
    let request_index = state::request_index::get_request_index_entry(&project_root, &request_id)
        .map_err(|error| ConfirmError {
        project_root: project_root.clone(),
        code: "REQUEST_NOT_FOUND".to_string(),
        message: error.to_string(),
    })?;
    if request_index.request_kind != "brainstorm_clarification_block" {
        return Err(ConfirmError {
            project_root,
            code: "BRAINSTORM_CONFIRM_REQUEST_KIND_INVALID".to_string(),
            message: "loom.brainstormConfirmBlock only accepts a Brainstorm clarification block requestRef.".to_string(),
        });
    }
    let delivery_id = request_index
        .delivery_id
        .clone()
        .ok_or_else(|| ConfirmError {
            project_root: project_root.clone(),
            code: "REQUEST_DELIVERY_MISSING".to_string(),
            message: "Brainstorm request is missing deliveryId.".to_string(),
        })?;
    let phase_id = request_index.phase_id.clone().ok_or_else(|| ConfirmError {
        project_root: project_root.clone(),
        code: "REQUEST_PHASE_MISSING".to_string(),
        message: "Brainstorm request is missing phaseId.".to_string(),
    })?;

    let store = FileTransitionStore;
    let mut delivery = store
        .load_delivery_index(&project_root, &delivery_id)
        .map_err(|error| ConfirmError {
            project_root: project_root.clone(),
            code: "DELIVERY_NOT_FOUND".to_string(),
            message: error.to_string(),
        })?;
    let phase = delivery
        .phases
        .iter_mut()
        .find(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| ConfirmError {
            project_root: project_root.clone(),
            code: "PHASE_NOT_FOUND".to_string(),
            message: format!("phase {phase_id} does not exist in delivery {delivery_id}."),
        })?;
    if phase
        .latest_refs
        .get("brainstormRequestRef")
        .map(String::as_str)
        != Some(input.request_ref.as_str())
    {
        return Err(ConfirmError {
            project_root,
            code: "STALE_BRAINSTORM_REQUEST".to_string(),
            message: "Brainstorm confirmation must use the active latest Brainstorm requestRef."
                .to_string(),
        });
    }

    let root = Path::new(&project_root);
    let state_ref = phase
        .latest_refs
        .get("brainstormClarificationState")
        .cloned()
        .ok_or_else(|| ConfirmError {
            project_root: project_root.clone(),
            code: "CLARIFICATION_STATE_MISSING".to_string(),
            message: "Brainstorm clarification state is missing.".to_string(),
        })?;
    let state_file = from_project_relative(root, &state_ref).map_err(|error| ConfirmError {
        project_root: project_root.clone(),
        code: "CLARIFICATION_STATE_PATH_INVALID".to_string(),
        message: error.to_string(),
    })?;
    let mut state: ClarificationState = read_json(&state_file).map_err(|error| ConfirmError {
        project_root: project_root.clone(),
        code: "CLARIFICATION_STATE_INVALID".to_string(),
        message: error.to_string(),
    })?;
    if state.current_block != input.block {
        return Err(ConfirmError {
            project_root,
            code: "BRAINSTORM_BLOCK_OUT_OF_ORDER".to_string(),
            message: format!(
                "Current Brainstorm block is {:?}, but confirm input targeted {:?}.",
                state.current_block, input.block
            ),
        });
    }
    if input.summary.trim().is_empty() {
        return Err(ConfirmError {
            project_root,
            code: "BRAINSTORM_CONFIRM_SUMMARY_REQUIRED".to_string(),
            message: "Brainstorm block confirmation summary is required.".to_string(),
        });
    }
    if input.skipped && input.block != ClarificationBlockName::FrontendExperience {
        return Err(ConfirmError {
            project_root,
            code: "BRAINSTORM_BLOCK_SKIP_NOT_ALLOWED".to_string(),
            message: "Only the page operation path block can be skipped.".to_string(),
        });
    }
    ensure_block_knowledge_context(
        &project_root,
        &delivery_id,
        &phase_id,
        &request_id,
        &input.request_ref,
        &input.block,
    )?;

    let now = state::store::now_string();
    upsert_confirmed_block(
        &mut state,
        ConfirmedClarificationBlock {
            block: input.block.clone(),
            summary: input.summary.trim().to_string(),
            confirmed_by_user: !input.skipped,
            skipped: input.skipped,
            skip_reason: input.skip_reason.filter(|value| !value.trim().is_empty()),
            confirmed_data: input.confirmed_data,
            confirmed_at: now.clone(),
        },
    );
    state.final_summary_confirmed = input.block == ClarificationBlockName::FinalSummary;
    state.current_block = next_block(&input.block).unwrap_or(ClarificationBlockName::FinalSummary);
    state.updated_at = now;
    write_json_atomic(&state_file, &state).map_err(|error| ConfirmError {
        project_root: project_root.clone(),
        code: "CLARIFICATION_STATE_WRITE_FAILED".to_string(),
        message: error.to_string(),
    })?;

    let user_facing_language =
        read_user_facing_language(root, &delivery_id).map_err(|error| ConfirmError {
            project_root: project_root.clone(),
            code: "BRAINSTORM_CONTRACT_INVALID".to_string(),
            message: error.to_string(),
        })?;
    let requirement_context_ref = phase
        .latest_refs
        .get("requirementContext")
        .cloned()
        .ok_or_else(|| ConfirmError {
            project_root: project_root.clone(),
            code: "REQUIREMENT_CONTEXT_MISSING".to_string(),
            message: "requirementContext ref is missing.".to_string(),
        })?;
    let keyword_hints_ref = phase.latest_refs.get("keywordHints").cloned();
    let normalized_text_ref = phase.latest_refs.get("normalizedRequirementText").cloned();
    let brainstorm_run_id = phase
        .latest_refs
        .get("brainstormRunId")
        .cloned()
        .unwrap_or_else(|| state.brainstorm_run_id.clone());

    let result = if state.final_summary_confirmed {
        let request_id = format!("brainstorm-candidate-write-{}", state::store::now_millis());
        let context_refs = context_refs(
            requirement_context_ref,
            normalized_text_ref,
            keyword_hints_ref,
            Some(state_ref.clone()),
        );
        let request_root = build_brainstorm_candidate_write_request_root(
            root,
            &request_id,
            &delivery_id,
            &phase_id,
            &brainstorm_run_id,
            &user_facing_language,
            context_refs,
        );
        let stored = state::write_native_request(
            &project_root,
            state::NativeRequestInput {
                request_id: request_id.clone(),
                request_kind: "brainstorm_candidate_write".to_string(),
                request_file: None,
                delivery_id: Some(delivery_id.clone()),
                phase_id: Some(phase_id.clone()),
                root: request_root,
            },
        )
        .map_err(|error| ConfirmError {
            project_root: project_root.clone(),
            code: "BRAINSTORM_WRITE_REQUEST_FAILED".to_string(),
            message: error.to_string(),
        })?;
        ensure_agent_writable_dir(root, &request_id).map_err(|error| ConfirmError {
            project_root: project_root.clone(),
            code: "BRAINSTORM_TARGET_DIR_FAILED".to_string(),
            message: error.to_string(),
        })?;
        phase
            .latest_refs
            .insert("brainstormRequestId".to_string(), request_id);
        phase.latest_refs.insert(
            "brainstormRequestRef".to_string(),
            stored.request_ref.clone(),
        );
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::BrainstormConfirmation,
            source: "brainstorm_confirm_block".to_string(),
            reason: "final_summary_confirmed".to_string(),
            prompt: None,
            accepted_responses: vec![],
            request_ref: Some(stored.request_ref.clone()),
            details: None,
            target_phase_id: None,
        });
        write_candidate_result(&project_root, &stored.request_ref).map_err(|error| {
            ConfirmError {
                project_root: project_root.clone(),
                code: "BRAINSTORM_WRITE_ACTION_FAILED".to_string(),
                message: error.to_string(),
            }
        })?
    } else {
        let next = next_block(&input.block).expect("non-final block has next");
        let request_id = format!(
            "brainstorm-{}-{}",
            block_id(&next),
            state::store::now_millis()
        );
        let context_refs = context_refs(
            requirement_context_ref,
            normalized_text_ref,
            keyword_hints_ref,
            Some(state_ref),
        );
        let request_root = build_brainstorm_clarification_request_root(
            &request_id,
            &delivery_id,
            &phase_id,
            &brainstorm_run_id,
            &user_facing_language,
            context_refs,
            next.clone(),
        );
        let stored = state::write_native_request(
            &project_root,
            state::NativeRequestInput {
                request_id: request_id.clone(),
                request_kind: "brainstorm_clarification_block".to_string(),
                request_file: None,
                delivery_id: Some(delivery_id.clone()),
                phase_id: Some(phase_id.clone()),
                root: request_root,
            },
        )
        .map_err(|error| ConfirmError {
            project_root: project_root.clone(),
            code: "BRAINSTORM_NEXT_REQUEST_FAILED".to_string(),
            message: error.to_string(),
        })?;
        phase
            .latest_refs
            .insert("brainstormRequestId".to_string(), request_id);
        phase.latest_refs.insert(
            "brainstormRequestRef".to_string(),
            stored.request_ref.clone(),
        );
        let gate = gate_for_block(next);
        phase.next_action = Some(RouteAction {
            kind: RouteActionKind::BrainstormClarification,
            source: "brainstorm_confirm_block".to_string(),
            reason: "await_next_brainstorm_block_confirmation".to_string(),
            prompt: Some(gate.user_message.clone()),
            accepted_responses: vec!["reply_in_chat".to_string()],
            request_ref: Some(stored.request_ref.clone()),
            details: Some(to_value(&gate)),
            target_phase_id: None,
        });
        let gate_value = to_value(&gate);
        LoomMcpActionResult::UserGate(LoomMcpUserGateResult {
            project_root: project_root.clone(),
            prompt: gate.user_message.clone(),
            accepted_responses: vec!["reply_in_chat".to_string()],
            request_ref: Some(stored.request_ref),
            delivery_id: Some(delivery_id.clone()),
            phase_id: Some(phase_id.clone()),
            gate: Some(gate_value),
        })
    };

    delivery.updated_at = state::store::now_string();
    store
        .save_delivery_index(&project_root, &delivery)
        .map_err(|error| ConfirmError {
            project_root,
            code: "DELIVERY_SAVE_FAILED".to_string(),
            message: error.to_string(),
        })?;
    Ok(result)
}

fn ensure_block_knowledge_context(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
    request_id: &str,
    request_ref: &str,
    block: &ClarificationBlockName,
) -> Result<(), ConfirmError> {
    let required_steps = required_knowledge_step_ids(block);
    if required_steps.is_empty() {
        return Ok(());
    }
    let project_paths =
        state::paths::project_paths(project_root).map_err(|error| ConfirmError {
            project_root: project_root.to_string(),
            code: "BRAINSTORM_KNOWLEDGE_CONTEXT_CHECK_FAILED".to_string(),
            message: error.to_string(),
        })?;
    let block_name = block_id(block);
    let base_dir = state::paths::workspace_dir(
        &project_paths.root,
        &DeliveryPhaseLocator {
            delivery_id: delivery_id.to_string(),
            phase_id: phase_id.to_string(),
        },
    )
    .join("brainstorm-knowledge")
    .join(request_id)
    .join(block_name);
    let missing_steps = required_steps
        .iter()
        .filter(|step_id| !base_dir.join(step_id).join("result.json").is_file())
        .copied()
        .collect::<Vec<_>>();
    if missing_steps.is_empty() {
        return Ok(());
    }
    Err(ConfirmError {
        project_root: project_root.to_string(),
        code: "BRAINSTORM_KNOWLEDGE_CONTEXT_REQUIRED".to_string(),
        message: format!(
            "Before confirming {block_name}, the agent must read requestReadPlan group knowledge_context_plan for requestRef {request_ref} and call loom.knowledgeBrainstormContext for the missing stepIds: {}. Empty knowledge results are acceptable; missing request-scoped knowledge result files are not. Do not ask the user to reconfirm this block; run the missing Loom knowledge calls, then retry loom.brainstormConfirmBlock.",
            missing_steps.join(", ")
        ),
    })
}

fn materialize_confirmation_request_inner(
    project_root: &str,
    delivery_id: &str,
    phase_id: &str,
) -> StateResult<LoomMcpActionResult> {
    let store = FileTransitionStore;
    let delivery = store
        .load_delivery_index(project_root, delivery_id)
        .map_err(to_state_error)?;
    let phase = delivery
        .phases
        .iter()
        .find(|phase| phase.phase_id == phase_id)
        .ok_or_else(|| StateError::InvalidArgument(format!("phase {phase_id} not found")))?;
    let request_ref = phase
        .latest_refs
        .get("brainstormRequestRef")
        .ok_or_else(|| StateError::InvalidArgument("Brainstorm requestRef missing".to_string()))?;
    write_candidate_result(project_root, request_ref)
}

fn write_candidate_result(
    project_root: &str,
    request_ref: &str,
) -> StateResult<LoomMcpActionResult> {
    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })?;
    let submit_tool = inspected.submit_tool.ok_or_else(|| {
        StateError::InvalidArgument(format!(
            "request {} is missing outputContract.submitTool",
            inspected.request_id
        ))
    })?;
    let write_targets = inspected
        .write_targets
        .iter()
        .map(value_to_write_target)
        .collect::<StateResult<Vec<_>>>()?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(
            project_root.to_string(),
            LoomMcpNextAction::WriteArtifact(WriteArtifactNext {
                artifact_kind: ArtifactKind::BrainstormCandidate,
                request_ref: request_ref.to_string(),
                write_mode: WriteMode::SingleJson,
                write_targets,
                read_groups: inspected.read_groups,
                submit_tool,
            }),
        ),
    ))
}

fn value_to_write_target(value: &Value) -> StateResult<WriteTarget> {
    Ok(WriteTarget {
        target_id: value
            .get("targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                StateError::InvalidArgument("write target missing targetId".to_string())
            })?
            .to_string(),
        path: value
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| StateError::InvalidArgument("write target missing path".to_string()))?
            .to_string(),
        required: value
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("Write the requested artifact.")
            .to_string(),
    })
}

fn upsert_confirmed_block(state: &mut ClarificationState, block: ConfirmedClarificationBlock) {
    if let Some(existing) = state
        .blocks
        .iter_mut()
        .find(|existing| existing.block == block.block)
    {
        *existing = block;
    } else {
        state.blocks.push(block);
    }
    state.blocks.sort_by_key(|block| block_order(&block.block));
}

fn next_block(block: &ClarificationBlockName) -> Option<ClarificationBlockName> {
    match block {
        ClarificationBlockName::PhaseScope => Some(ClarificationBlockName::ConceptGrounding),
        ClarificationBlockName::ConceptGrounding => {
            Some(ClarificationBlockName::FrontendExperience)
        }
        ClarificationBlockName::FrontendExperience => Some(ClarificationBlockName::FinalSummary),
        ClarificationBlockName::FinalSummary => None,
    }
}

fn block_order(block: &ClarificationBlockName) -> u8 {
    match block {
        ClarificationBlockName::PhaseScope => 1,
        ClarificationBlockName::ConceptGrounding => 2,
        ClarificationBlockName::FrontendExperience => 3,
        ClarificationBlockName::FinalSummary => 4,
    }
}

fn block_id(block: &ClarificationBlockName) -> &'static str {
    match block {
        ClarificationBlockName::PhaseScope => "phase_scope",
        ClarificationBlockName::ConceptGrounding => "concept_grounding",
        ClarificationBlockName::FrontendExperience => "frontend_experience",
        ClarificationBlockName::FinalSummary => "final_summary",
    }
}

fn gate_for_block(block: ClarificationBlockName) -> BrainstormGate {
    BrainstormGate {
        gate_id: format!("gate_{}", block_id(&block)),
        current_block: block.clone(),
        required_blocks: crate::gate::required_blocks(),
        already_confirmed_blocks: vec![],
        skipped_blocks: vec![],
        user_message: block_message(&block),
        response_rule: BrainstormResponseRule {
            mode: "progressive_brainstorm".to_string(),
            final_summary_required_before_write: true,
            user_visible_confirmation_required: true,
        },
        issues: vec![],
    }
}

fn context_refs(
    requirement_context_ref: String,
    normalized_text_ref: Option<String>,
    keyword_hints_ref: Option<String>,
    clarification_state_ref: Option<String>,
) -> Value {
    let mut value = json!({
        "requirementContextRef": requirement_context_ref
    });
    if let Some(normalized) = normalized_text_ref {
        value["normalizedRequirementTextRef"] = json!(normalized);
    }
    if let Some(keyword_hints) = keyword_hints_ref {
        value["keywordHintsRef"] = json!(keyword_hints);
    }
    if let Some(state_ref) = clarification_state_ref {
        value["clarificationStateRef"] = json!(state_ref);
    }
    value
}

fn read_user_facing_language(
    project_root: &Path,
    delivery_id: &str,
) -> StateResult<UserFacingLanguageConstraint> {
    let contract_file = crate::paths::brainstorm_contract_file(project_root, delivery_id);
    let value = state::store::read_json_value(&contract_file)?;
    serde_json::from_value(
        value
            .pointer("/deliveryContext/userFacingLanguage")
            .cloned()
            .ok_or_else(|| StateError::StateCorrupted("userFacingLanguage missing".to_string()))?,
    )
    .map_err(|error| StateError::StateCorrupted(format!("invalid userFacingLanguage: {error}")))
}

fn ensure_agent_writable_dir(project_root: &Path, request_id: &str) -> StateResult<()> {
    if let Some(parent) = brainstorm_agent_candidate_file(project_root, request_id).parent() {
        ensure_dir(parent)?;
    }
    Ok(())
}

fn parse_request_id(request_ref: &str) -> Result<String, String> {
    let prefix = "loom://projects/";
    let rest = request_ref
        .strip_prefix(prefix)
        .ok_or_else(|| "requestRef must start with loom://projects/.".to_string())?;
    let (_project_id, request_id) = rest
        .split_once("/requests/")
        .ok_or_else(|| "requestRef must include /requests/.".to_string())?;
    if request_id.is_empty() || request_id.contains('/') {
        return Err(format!("invalid requestRef: {request_ref}"));
    }
    Ok(request_id.to_string())
}

fn to_state_error(error: delivery_core::LoomCoreError) -> StateError {
    StateError::StateCorrupted(error.to_string())
}

#[derive(Debug)]
struct ConfirmError {
    project_root: String,
    code: String,
    message: String,
}

pub fn write_initial_state_file(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
    state: &ClarificationState,
) -> StateResult<String> {
    let file = brainstorm_clarification_state_file(project_root, delivery_id, phase_id);
    write_json_atomic(&file, state)?;
    to_project_relative(project_root, &file)
}
