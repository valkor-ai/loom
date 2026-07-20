use std::path::{Path, PathBuf};

use state::paths::delivery_dir;

pub fn requirements_dir(project_root: &Path, delivery_id: &str) -> PathBuf {
    delivery_dir(project_root, delivery_id).join("requirements")
}

pub fn requirement_input_file(project_root: &Path, delivery_id: &str, item_id: &str) -> PathBuf {
    requirements_dir(project_root, delivery_id)
        .join("inputs")
        .join(format!("{item_id}.txt"))
}

pub fn requirement_context_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    requirements_dir(project_root, delivery_id).join("context.json")
}

pub fn requirement_normalized_text_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    requirements_dir(project_root, delivery_id).join("normalized.txt")
}

pub fn requirement_keyword_hints_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    requirements_dir(project_root, delivery_id).join("keyword-hints.json")
}

pub fn brainstorm_dir(project_root: &Path, delivery_id: &str) -> PathBuf {
    delivery_dir(project_root, delivery_id).join("brainstorm")
}

pub fn brainstorm_contract_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    brainstorm_dir(project_root, delivery_id).join("contract.json")
}

pub fn brainstorm_latest_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    brainstorm_dir(project_root, delivery_id).join("latest.json")
}

pub fn brainstorm_clarification_state_file(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
) -> PathBuf {
    brainstorm_phase_dir(project_root, delivery_id, phase_id).join("clarification-state.json")
}

pub fn brainstorm_phases_dir(project_root: &Path, delivery_id: &str) -> PathBuf {
    brainstorm_dir(project_root, delivery_id).join("phases")
}

pub fn brainstorm_phase_dir(project_root: &Path, delivery_id: &str, phase_id: &str) -> PathBuf {
    brainstorm_phases_dir(project_root, delivery_id).join(phase_id)
}

pub fn brainstorm_decision_snapshot_file(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
) -> PathBuf {
    brainstorm_phase_dir(project_root, delivery_id, phase_id).join("decision-snapshot.json")
}

pub fn brainstorm_phase_concept_file(
    project_root: &Path,
    delivery_id: &str,
    phase_id: &str,
) -> PathBuf {
    brainstorm_phase_dir(project_root, delivery_id, phase_id).join("phase-concept-grounding.json")
}

pub fn brainstorm_delivery_glossary_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    brainstorm_dir(project_root, delivery_id).join("delivery-concept-glossary.json")
}

pub fn brainstorm_decisions_index_file(project_root: &Path, delivery_id: &str) -> PathBuf {
    brainstorm_dir(project_root, delivery_id).join("decisions-index.json")
}

pub fn brainstorm_agent_candidate_file(project_root: &Path, request_id: &str) -> PathBuf {
    project_root
        .join(".loom")
        .join("agent-writable")
        .join(request_id)
        .join("brainstorm-candidate.json")
}
