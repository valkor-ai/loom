use std::fs;

use crate::{
    mcp_models::{KnowledgeInspectChunkInput, KnowledgeInspectChunkResult},
    models::{ChunksFile, KnowledgeSource},
    paths,
    store::{load_registry, read_json, KnowledgeError, KnowledgeResult},
};

pub fn inspect_chunk(
    input: KnowledgeInspectChunkInput,
) -> KnowledgeResult<KnowledgeInspectChunkResult> {
    let source = resolve_source(&input)?;
    let chunks_file: ChunksFile =
        read_json(&paths::chunks_file(&source.source_id, &input.build_id)?)?;
    let chunk = chunks_file
        .chunks
        .iter()
        .find(|chunk| chunk.chunk_id == input.chunk_id)
        .ok_or_else(|| {
            KnowledgeError::invalid(format!("knowledge chunk not found: {}", input.chunk_id))
        })?;
    let text = read_chunk_body(&source.source_id, &input.build_id, &input.chunk_id)?;
    Ok(KnowledgeInspectChunkResult {
        source_name: source.name,
        source_id: source.source_id,
        build_id: input.build_id,
        chunk_id: input.chunk_id,
        document_title: chunk.document_title.clone(),
        heading_path: chunk.heading_path.clone(),
        text,
    })
}

pub fn read_chunk_body(source_id: &str, build_id: &str, chunk_id: &str) -> KnowledgeResult<String> {
    Ok(fs::read_to_string(paths::chunk_body_file(
        source_id, build_id, chunk_id,
    )?)?)
}

fn resolve_source(input: &KnowledgeInspectChunkInput) -> KnowledgeResult<KnowledgeSource> {
    let registry = load_registry()?;
    if let Some(source_id) = &input.source_id {
        let source = registry
            .sources
            .iter()
            .find(|source| source.source_id == *source_id)
            .cloned()
            .ok_or_else(|| {
                KnowledgeError::invalid(format!("knowledge source not found: {source_id}"))
            })?;
        if !input.source_name.trim().is_empty() && source.name != input.source_name {
            return Err(KnowledgeError::invalid(format!(
                "knowledge source name does not match sourceId: {} != {}",
                input.source_name, source.name
            )));
        }
        return Ok(source);
    }
    registry
        .sources
        .iter()
        .find(|source| source.name == input.source_name)
        .cloned()
        .ok_or_else(|| {
            KnowledgeError::invalid(format!("knowledge source not found: {}", input.source_name))
        })
}
