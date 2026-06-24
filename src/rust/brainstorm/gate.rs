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

pub fn block_message(block: &ClarificationBlockName) -> String {
    match block {
        ClarificationBlockName::PhaseScope => {
            "Start at phase_scope. Present the current stage scope options in the user's language, wait for the user's visible confirmation, then continue to the next Brainstorm block.".to_string()
        }
        ClarificationBlockName::ConceptGrounding => {
            "Return to concept_grounding. Confirm the business objects, operations, rules, fields, blockers, outcomes, and misunderstanding boundaries for the user-confirmed current scope.".to_string()
        }
        ClarificationBlockName::FrontendExperience => {
            "Return to frontend_experience. Confirm the page or workspace operation path, target discovery, action entry, feedback, and readback, or explicitly record why UI is not applicable.".to_string()
        }
        ClarificationBlockName::FinalSummary => {
            "Return to final_summary. Present the pre-submit coverage checklist, apply any user corrections back to structured fields, then confirm before writing the Brainstorm candidate.".to_string()
        }
    }
}
