use std::path::{Path, PathBuf};

use crate::store::{KnowledgeError, KnowledgeResult};

pub fn knowledge_root() -> KnowledgeResult<PathBuf> {
    let loom_home = match std::env::var("LOOM_HOME") {
        Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
        _ => {
            let home = std::env::var("HOME")
                .map_err(|_| KnowledgeError::invalid("HOME is required for knowledge store"))?;
            PathBuf::from(home).join(".loom")
        }
    };
    Ok(loom_home.join("knowledge"))
}

pub fn registry_file() -> KnowledgeResult<PathBuf> {
    Ok(knowledge_root()?.join("registry.json"))
}

pub fn pending_dir() -> KnowledgeResult<PathBuf> {
    Ok(knowledge_root()?.join("pending"))
}

pub fn pending_file(source_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(pending_dir()?.join(format!("{source_id}.json")))
}

pub fn sources_dir() -> KnowledgeResult<PathBuf> {
    Ok(knowledge_root()?.join("sources"))
}

pub fn source_dir(source_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(sources_dir()?.join(source_id))
}

pub fn build_runs_dir(source_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(source_dir(source_id)?.join("build-runs"))
}

pub fn build_run_dir(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_runs_dir(source_id)?.join(build_id))
}

pub fn chunks_dir(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_run_dir(source_id, build_id)?.join("chunks"))
}

pub fn chunk_body_file(
    source_id: &str,
    build_id: &str,
    chunk_id: &str,
) -> KnowledgeResult<PathBuf> {
    Ok(chunks_dir(source_id, build_id)?.join(format!("{chunk_id}.txt")))
}

pub fn chunks_file(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_run_dir(source_id, build_id)?.join("chunks.json"))
}

pub fn snapshot_file(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_run_dir(source_id, build_id)?.join("snapshot.json"))
}

pub fn lexical_index_file(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_run_dir(source_id, build_id)?.join("lexical-index.json"))
}

pub fn semantic_index_file(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_run_dir(source_id, build_id)?.join("semantic-index.json"))
}

pub fn semantic_state_file(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_run_dir(source_id, build_id)?.join("semantic-state.json"))
}

pub fn semantic_results_dir(source_id: &str, build_id: &str) -> KnowledgeResult<PathBuf> {
    Ok(build_run_dir(source_id, build_id)?.join("semantic-results"))
}

pub fn semantic_result_file(
    source_id: &str,
    build_id: &str,
    pack_id: &str,
) -> KnowledgeResult<PathBuf> {
    Ok(semantic_results_dir(source_id, build_id)?.join(format!("{pack_id}.json")))
}

pub fn to_slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
