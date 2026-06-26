use contracts::ClarificationBlockName;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormGate {
    pub gate_id: String,
    pub current_block: ClarificationBlockName,
    pub required_blocks: Vec<ClarificationBlockName>,
    pub already_confirmed_blocks: Vec<ClarificationBlockName>,
    pub skipped_blocks: Vec<SkippedBlockSummary>,
    pub user_message: String,
    pub response_rule: BrainstormResponseRule,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedBlockSummary {
    pub block: ClarificationBlockName,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormResponseRule {
    pub mode: String,
    pub final_summary_required_before_write: bool,
    pub user_visible_confirmation_required: bool,
}

pub fn required_blocks() -> Vec<ClarificationBlockName> {
    vec![
        ClarificationBlockName::PhaseScope,
        ClarificationBlockName::ConceptGrounding,
        ClarificationBlockName::FrontendExperience,
        ClarificationBlockName::FinalSummary,
    ]
}

pub fn to_value(gate: &BrainstormGate) -> Value {
    serde_json::to_value(gate).unwrap_or_else(|_| serde_json::json!({}))
}

pub fn required_knowledge_step_ids(block: &ClarificationBlockName) -> &'static [&'static str] {
    match block {
        ClarificationBlockName::PhaseScope => &[
            "phase_scope_dependency_order",
            "phase_scope_capability_closure",
        ],
        ClarificationBlockName::ConceptGrounding => &["concept_grounding_scope_item"],
        ClarificationBlockName::FrontendExperience => &["frontend_experience_page_operation_path"],
        ClarificationBlockName::FinalSummary => &[],
    }
}

pub fn block_message(block: &ClarificationBlockName) -> String {
    match block {
        ClarificationBlockName::PhaseScope => {
            "Read the current block's knowledge plan, query request-scoped knowledge, then present 2-3 active phase boundary options in the user's language, not a full multi-stage project roadmap. Wait for the user's visible confirmation, then continue to business understanding and rule confirmation. Do not show internal block ids to the user.".to_string()
        }
        ClarificationBlockName::ConceptGrounding => {
            "Read the current block's knowledge plan, query request-scoped knowledge, then confirm the business objects, operations, rules, fields, blockers, outcomes, and misunderstanding boundaries for the user-confirmed current scope. Use a user-facing title such as business understanding and rule confirmation.".to_string()
        }
        ClarificationBlockName::FrontendExperience => {
            "Read the current block's knowledge plan, query request-scoped knowledge, then confirm the page or workspace operation path, target discovery, action entry, feedback, and readback, or explicitly record why UI is not applicable. Use a user-facing title such as page operation path confirmation.".to_string()
        }
        ClarificationBlockName::FinalSummary => {
            "Present the pre-submit coverage checklist, apply any user corrections back to structured fields, then confirm before writing the final structured requirement result. Do not show internal block ids to the user.".to_string()
        }
    }
}
