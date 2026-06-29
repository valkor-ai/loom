pub mod builder;
pub mod inspect;
pub mod mcp_models;
pub mod models;
pub mod operations;
pub mod paths;
pub mod search;
pub mod semantic;
pub mod store;

use delivery_core::LoomMcpActionResult;

pub use builder::{build_source, rebuild_lexical_index, resume_source, validate_candidate_paths};
pub use inspect::{inspect_chunk, read_chunk_body};
use mcp_models::{KnowledgeNameInput, KnowledgeSemanticSubmitInput};
pub use operations::{
    add_source, disable_source, discard_pending, enable_source, list_sources, pending_sources,
    remove_source, source_status, update_source,
};
pub use search::{brainstorm_context, search_knowledge};
pub use semantic::submit_semantic_pack;
pub use store::{KnowledgeError, KnowledgeResult};

pub fn build_source_from_input(input: KnowledgeNameInput) -> KnowledgeResult<LoomMcpActionResult> {
    build_source(&input.project_root, &input.name)
}

pub fn resume_source_from_input(input: KnowledgeNameInput) -> KnowledgeResult<LoomMcpActionResult> {
    resume_source(&input.project_root, &input.name)
}

pub fn submit_semantic_pack_from_input(
    input: KnowledgeSemanticSubmitInput,
) -> KnowledgeResult<LoomMcpActionResult> {
    submit_semantic_pack(&input.project_root, &input.request_ref)
}
