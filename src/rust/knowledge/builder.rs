use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use algorithm_client::AlgorithmClient;
use delivery_core::{
    read_selectors_value_from_paths, KnowledgeChunkReadRef, LoomMcpActionResult,
    LoomMcpAutoRunnableResult, LoomMcpBlockedResult, LoomMcpDoneResult,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    models::{
        ChunksFile, KnowledgeBuildSnapshot, KnowledgeChunk, KnowledgeDocument, LexicalDocument,
        LexicalIndex, LexicalKeyword, PendingOperationKind, SemanticBuildStatus, SemanticPackState,
        SemanticPackStatus, SemanticState, SkippedFile,
    },
    operations::{registry_source, summary},
    paths,
    semantic::{next_pending_pack, semantic_generation_rules, semantic_result_template},
    store::{
        ensure_dir, load_pending, load_registry, now_millis, now_string, read_json, save_registry,
        write_json, write_text, KnowledgeError, KnowledgeResult,
    },
};

const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
const TARGET_TOKENS: u32 = 700;
const SOFT_MAX_TOKENS: u32 = 1200;
const HARD_MAX_TOKENS: u32 = 1800;
const MIN_TOKENS: u32 = 120;
const CONTEXT_PREFIX_MAX_CHARS: usize = 80;
const PACK_BUDGET: u32 = 7000;
const PACK_OVERHEAD: u32 = 1000;
const PACK_CHUNK_METADATA: u32 = 150;

pub fn validate_candidate_paths(
    paths: &[String],
    explicit_files_must_be_supported: bool,
) -> KnowledgeResult<Vec<SkippedFile>> {
    let mut warnings = Vec::new();
    for raw in paths {
        let path = expand_tilde(raw);
        if !path.exists() {
            return Err(KnowledgeError::invalid(format!(
                "knowledge path does not exist: {raw}"
            )));
        }
        if path.is_file() {
            validate_explicit_file(&path, explicit_files_must_be_supported)?;
        } else if path.is_dir() {
            let scanned = scan_directory(&path)?;
            warnings.extend(scanned.skipped);
            if explicit_files_must_be_supported && scanned.files.is_empty() {
                return Err(KnowledgeError::invalid(format!(
                    "knowledge directory contains no supported files: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(warnings)
}

pub fn build_source(project_root: &str, name: &str) -> KnowledgeResult<LoomMcpActionResult> {
    let mut registry = load_registry()?;
    let source = registry_source(&registry, name)?.clone();
    cleanup_pending_build_runs(&source.source_id, None)?;
    let pending = load_pending(&source.source_id, &source.name)?;
    let document_paths = apply_pending_paths(&source.document_paths, &pending.operations)?;
    if document_paths.is_empty() {
        return Err(KnowledgeError::invalid(
            "knowledge source has no document paths to build",
        ));
    }

    let discovered = discover_documents(&document_paths)?;
    if discovered.files.is_empty() {
        return Err(KnowledgeError::invalid(
            "knowledge build found no supported readable documents",
        ));
    }

    let build_id = format!("kbld_{}_{}", now_millis(), short_hash(&source.source_id));
    let build_dir = paths::build_run_dir(&source.source_id, &build_id)?;
    ensure_dir(&build_dir)?;
    ensure_dir(&paths::chunks_dir(&source.source_id, &build_id)?)?;

    let mut documents = Vec::new();
    let mut chunks = Vec::new();
    for (document_index, path) in discovered.files.iter().enumerate() {
        let parsed = parse_document(path)?;
        let document_id = format!("kdoc_{:06}", document_index + 1);
        documents.push(KnowledgeDocument {
            document_id: document_id.clone(),
            path: path.to_string_lossy().to_string(),
            title: parsed.title.clone(),
            content_type: parsed.content_type,
            sha256: file_sha256(path)?,
            size_bytes: fs::metadata(path)?.len(),
        });
        let document_chunks = chunk_document(
            &source.source_id,
            &build_id,
            &document_id,
            &parsed.title,
            &path.to_string_lossy(),
            &parsed.text,
            chunks.len(),
        )?;
        chunks.extend(document_chunks);
    }
    fill_neighbors(&mut chunks);

    let chunks_file = ChunksFile {
        schema_version: 1,
        source_id: source.source_id.clone(),
        source_name: source.name.clone(),
        build_id: build_id.clone(),
        chunks: chunks.clone(),
    };
    write_json(
        &paths::chunks_file(&source.source_id, &build_id)?,
        &chunks_file,
    )?;
    write_json(
        &paths::snapshot_file(&source.source_id, &build_id)?,
        &KnowledgeBuildSnapshot {
            schema_version: 1,
            source_id: source.source_id.clone(),
            source_name: source.name.clone(),
            build_id: build_id.clone(),
            documents,
            skipped_files: discovered.skipped,
            created_at: now_string(),
        },
    )?;
    rebuild_lexical_index(&source.source_id, &build_id, &chunks)?;

    let semantic_state = create_semantic_state(
        project_root,
        &source.name,
        &source.source_id,
        &build_id,
        &chunks,
    )?;
    write_json(
        &paths::semantic_state_file(&source.source_id, &build_id)?,
        &semantic_state,
    )?;

    let registry_source = registry
        .sources
        .iter_mut()
        .find(|candidate| candidate.source_id == source.source_id)
        .ok_or_else(|| {
            KnowledgeError::invalid(format!(
                "knowledge source disappeared during build: {}",
                source.source_id
            ))
        })?;
    registry_source.document_paths = document_paths;
    registry_source.updated_at = now_string();
    save_registry(&registry)?;

    let next = next_pending_pack(project_root, &source.name, &source.source_id, &build_id)?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(project_root, next),
    ))
}

pub fn resume_source(project_root: &str, name: &str) -> KnowledgeResult<LoomMcpActionResult> {
    let registry = load_registry()?;
    let source = registry_source(&registry, name)?.clone();
    cleanup_pending_build_runs(&source.source_id, source.current_build_id.as_deref())?;
    let Some(build_id) = latest_pending_build(&source.source_id)? else {
        if source.current_build_id.is_some() {
            return Ok(LoomMcpActionResult::Done(LoomMcpDoneResult {
                project_root: project_root.to_string(),
                summary: "Knowledge source is already published.".to_string(),
                details: Some(json!(summary(source, None, vec![]))),
                warnings: vec![],
            }));
        }
        return Ok(LoomMcpActionResult::Blocked(LoomMcpBlockedResult {
            project_root: project_root.to_string(),
            blockers: vec![
                "Knowledge source has no pending semantic build.".to_string(),
                "Run loom.knowledgeBuild before loom.knowledgeResume.".to_string(),
            ],
            recommended_tool: Some("loom.knowledgeBuild".to_string()),
            details: Some(json!({
                "sourceName": source.name,
                "sourceId": source.source_id,
            })),
        }));
    };
    let next = next_pending_pack(project_root, &source.name, &source.source_id, &build_id)?;
    Ok(LoomMcpActionResult::AutoRunnable(
        LoomMcpAutoRunnableResult::new(project_root, next),
    ))
}

pub(crate) fn create_semantic_state(
    project_root: &str,
    source_name: &str,
    source_id: &str,
    build_id: &str,
    chunks: &[KnowledgeChunk],
) -> KnowledgeResult<SemanticState> {
    let packs = pack_chunks(chunks);
    let pack_count = packs.len() as u32;
    let mut states = Vec::new();
    for (index, chunk_ids) in packs.into_iter().enumerate() {
        let pack_id = format!("kpack_{:04}", index + 1);
        let result_file =
            format!(".loom/agent-writable/knowledge/{source_id}/{build_id}/{pack_id}.json");
        let result_abs = PathBuf::from(project_root).join(&result_file);
        if let Some(parent) = result_abs.parent() {
            fs::create_dir_all(parent)?;
        }
        let read_plan = chunk_ids
            .iter()
            .filter_map(|chunk_id| chunks.iter().find(|chunk| &chunk.chunk_id == chunk_id))
            .map(|chunk| KnowledgeChunkReadRef {
                source_name: source_name.to_string(),
                source_id: source_id.to_string(),
                build_id: build_id.to_string(),
                chunk_id: chunk.chunk_id.clone(),
                document_title: chunk.document_title.clone(),
                heading_path: chunk.heading_path.clone(),
                token_estimate: chunk.token_estimate,
                summary_language: summary_language_for_chunk(source_id, build_id, &chunk.chunk_id),
                read_tool: "loom.knowledgeInspectChunk".to_string(),
                resource_uri: format!(
                    "loom://knowledge/{source_id}/builds/{build_id}/chunks/{}",
                    chunk.chunk_id
                ),
            })
            .collect::<Vec<_>>();
        let request_ref = write_semantic_request(
            project_root,
            source_name,
            source_id,
            build_id,
            &pack_id,
            index as u32 + 1,
            pack_count,
            &result_file,
            &read_plan,
        )?;
        states.push(SemanticPackState {
            pack_id,
            pack_index: index as u32 + 1,
            chunk_ids,
            status: SemanticPackStatus::Pending,
            request_ref,
            result_file,
            accepted_at: None,
        });
    }
    Ok(SemanticState {
        schema_version: 1,
        source_id: source_id.to_string(),
        source_name: source_name.to_string(),
        build_id: build_id.to_string(),
        status: SemanticBuildStatus::SemanticPending,
        pack_count,
        packs: states,
        created_at: now_string(),
        published_at: None,
    })
}

pub(crate) fn cleanup_pending_build_runs(
    source_id: &str,
    keep_build_id: Option<&str>,
) -> KnowledgeResult<()> {
    let build_runs_dir = paths::build_runs_dir(source_id)?;
    if !build_runs_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&build_runs_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let build_id = entry.file_name().to_string_lossy().to_string();
        if keep_build_id == Some(build_id.as_str()) {
            continue;
        }
        let semantic_state_file = paths::semantic_state_file(source_id, &build_id)?;
        if !semantic_state_file.exists() {
            continue;
        }
        let semantic_state: SemanticState = read_json(&semantic_state_file)?;
        if matches!(semantic_state.status, SemanticBuildStatus::SemanticPending) {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

fn write_semantic_request(
    project_root: &str,
    source_name: &str,
    source_id: &str,
    build_id: &str,
    pack_id: &str,
    pack_index: u32,
    pack_count: u32,
    result_file: &str,
    read_plan: &[KnowledgeChunkReadRef],
) -> KnowledgeResult<String> {
    let request_id = format!("ksem_{build_id}_{pack_id}");
    let result_template = semantic_result_template(read_plan);
    let generation_rules = semantic_generation_rules();
    let stored = state::write_native_request(
        project_root,
        state::NativeRequestInput {
            request_id,
            request_kind: "knowledge_semantic_pack".to_string(),
            request_file: None,
            delivery_id: None,
            phase_id: None,
            root: json!({
                "sourceName": source_name,
                "sourceId": source_id,
                "buildId": build_id,
                "packId": pack_id,
                "packIndex": pack_index,
                "packCount": pack_count,
                "chunkReadPlan": read_plan,
                "outputContract": {
                    "artifactKind": "knowledge_semantic_pack_result",
                    "submitTool": "loom.knowledgeSemanticSubmitFile",
                    "resultTemplate": result_template,
                    "writeMode": "single_json",
                    "writeTargets": [{
                        "targetId": "semantic_result",
                        "path": result_file,
                        "required": true,
                        "description": "Knowledge semantic pack result JSON."
                    }]
                },
                "generationRules": generation_rules,
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "semantic_pack_contract",
                        "required": true,
                        "purpose": "Read the semantic pack contract and chunk inspect plan.",
                        "whenToRead": "Before reading chunks and writing the semantic result.",
                        "selectors": read_selectors_value_from_paths([
                            "chunkReadPlan",
                            "outputContract.resultTemplate",
                            "generationRules",
                            "outputContract.writeTargets",
                            "outputContract.submitTool"
                        ])
                    }]
                }
            }),
        },
    )
    .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    Ok(stored.request_ref)
}

fn latest_pending_build(source_id: &str) -> KnowledgeResult<Option<String>> {
    let dir = paths::build_runs_dir(source_id)?;
    if !dir.exists() {
        return Ok(None);
    }
    let mut builds = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let build_id = entry.file_name().to_string_lossy().to_string();
        let state_file = paths::semantic_state_file(source_id, &build_id)?;
        if !state_file.exists() {
            continue;
        }
        let state: SemanticState = read_json(&state_file)?;
        if matches!(state.status, SemanticBuildStatus::SemanticPending) {
            builds.push(build_id);
        }
    }
    builds.sort();
    Ok(builds.pop())
}

fn validate_explicit_file(
    path: &Path,
    explicit_files_must_be_supported: bool,
) -> KnowledgeResult<()> {
    if !explicit_files_must_be_supported {
        return Ok(());
    }
    let meta = fs::metadata(path)?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(KnowledgeError::invalid(format!(
            "knowledge file exceeds 20MB: {}",
            path.display()
        )));
    }
    if !is_supported_file(path) {
        return Err(KnowledgeError::invalid(format!(
            "unsupported knowledge file type: {}",
            path.display()
        )));
    }
    Ok(())
}

struct DiscoveredDocuments {
    files: Vec<PathBuf>,
    skipped: Vec<SkippedFile>,
}

fn discover_documents(paths: &[String]) -> KnowledgeResult<DiscoveredDocuments> {
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    for raw in paths {
        let path = expand_tilde(raw);
        if path.is_file() {
            validate_explicit_file(&path, true)?;
            files.push(path.canonicalize()?);
        } else if path.is_dir() {
            let scanned = scan_directory(&path)?;
            files.extend(scanned.files);
            skipped.extend(scanned.skipped);
        }
    }
    files.sort();
    files.dedup();
    Ok(DiscoveredDocuments { files, skipped })
}

fn scan_directory(path: &Path) -> KnowledgeResult<DiscoveredDocuments> {
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    scan_directory_inner(path, &mut files, &mut skipped)?;
    files.sort();
    files.dedup();
    Ok(DiscoveredDocuments { files, skipped })
}

fn scan_directory_inner(
    path: &Path,
    files: &mut Vec<PathBuf>,
    skipped: &mut Vec<SkippedFile>,
) -> KnowledgeResult<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if ignored_name(&name) {
            continue;
        }
        if entry.file_type()?.is_dir() {
            scan_directory_inner(&entry_path, files, skipped)?;
            continue;
        }
        if !entry.file_type()?.is_file() {
            continue;
        }
        if fs::metadata(&entry_path)?.len() > MAX_FILE_BYTES {
            skipped.push(SkippedFile {
                path: entry_path.to_string_lossy().to_string(),
                reason: "file exceeds 20MB".to_string(),
            });
            continue;
        }
        if is_supported_file(&entry_path) {
            files.push(entry_path.canonicalize()?);
        } else {
            skipped.push(SkippedFile {
                path: entry_path.to_string_lossy().to_string(),
                reason: "unsupported file type".to_string(),
            });
        }
    }
    Ok(())
}

fn apply_pending_paths(
    existing: &[String],
    operations: &[crate::models::PendingOperation],
) -> KnowledgeResult<Vec<String>> {
    let mut paths = existing.iter().cloned().collect::<BTreeSet<_>>();
    for operation in operations {
        match operation.kind {
            PendingOperationKind::AddPaths => {
                paths.extend(operation.paths.iter().cloned());
            }
            PendingOperationKind::RemovePaths => {
                for path in &operation.paths {
                    paths.remove(path);
                }
            }
            PendingOperationKind::ReplacePaths => {
                paths = operation.paths.iter().cloned().collect();
            }
        }
    }
    Ok(paths.into_iter().collect())
}

struct ParsedDocument {
    title: String,
    content_type: String,
    text: String,
}

fn parse_document(path: &Path) -> KnowledgeResult<ParsedDocument> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("document")
        .to_string();
    let text = match extension.as_str() {
        "md" | "txt" => fs::read_to_string(path)?,
        "json" => {
            let value: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
            serde_json::to_string_pretty(&value)?
        }
        "yaml" | "yml" => {
            let value: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(path)?)?;
            serde_yaml::to_string(&value)?
        }
        "pdf" => pdf_extract::extract_text(path).map_err(|error| {
            KnowledgeError::invalid(format!("failed to parse PDF {}: {error}", path.display()))
        })?,
        "docx" => extract_docx_text(path)?,
        _ => {
            return Err(KnowledgeError::invalid(format!(
                "unsupported knowledge file type: {}",
                path.display()
            )))
        }
    };
    Ok(ParsedDocument {
        title,
        content_type: extension,
        text,
    })
}

fn extract_docx_text(path: &Path) -> KnowledgeResult<String> {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut document = archive.by_name("word/document.xml")?;
    let mut xml = String::new();
    document.read_to_string(&mut xml)?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Text(event)) => {
                text.push_str(&String::from_utf8_lossy(event.as_ref()));
                text.push('\n');
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(KnowledgeError::invalid(format!(
                    "failed to parse DOCX XML: {error}"
                )))
            }
        }
    }
    Ok(text)
}

fn chunk_document(
    source_id: &str,
    build_id: &str,
    document_id: &str,
    title: &str,
    source_path: &str,
    text: &str,
    offset: usize,
) -> KnowledgeResult<Vec<KnowledgeChunk>> {
    let blocks = split_blocks(text);
    let mut chunks = Vec::new();
    for (heading, block) in blocks {
        let block_tokens = estimate_tokens(&block);
        if block_tokens > SOFT_MAX_TOKENS {
            let parts = split_large_section(&block);
            let part_count = parts.len();
            for (part_index, part) in parts.into_iter().enumerate() {
                let heading_path = heading_path_for_part(&heading, part_index, part_count);
                let mut text = part;
                flush_chunk(
                    source_id,
                    build_id,
                    document_id,
                    title,
                    source_path,
                    &heading_path,
                    &mut text,
                    "section_split",
                    offset,
                    &mut chunks,
                )?;
            }
            continue;
        }
        let mut text = block;
        flush_chunk(
            source_id,
            build_id,
            document_id,
            title,
            source_path,
            &heading,
            &mut text,
            "section",
            offset,
            &mut chunks,
        )?;
    }
    merge_small_chunks(source_id, build_id, &mut chunks)?;
    Ok(chunks)
}

#[allow(clippy::too_many_arguments)]
fn flush_chunk(
    source_id: &str,
    build_id: &str,
    document_id: &str,
    title: &str,
    source_path: &str,
    heading_path: &[String],
    text: &mut String,
    split_reason: &str,
    offset: usize,
    chunks: &mut Vec<KnowledgeChunk>,
) -> KnowledgeResult<()> {
    let body = text.trim();
    if body.is_empty() {
        text.clear();
        return Ok(());
    }
    let chunk_id = format!("kchunk_{:06}", offset + chunks.len() + 1);
    let body_text = render_chunk_body(title, source_path, heading_path, body);
    write_text(
        &paths::chunk_body_file(source_id, build_id, &chunk_id)?,
        &body_text,
    )?;
    chunks.push(KnowledgeChunk {
        chunk_id: chunk_id.clone(),
        document_id: document_id.to_string(),
        document_title: title.to_string(),
        source_path: source_path.to_string(),
        heading_path: heading_path.to_vec(),
        token_estimate: estimate_tokens(&body_text),
        context_prefix: body_text.chars().take(CONTEXT_PREFIX_MAX_CHARS).collect(),
        neighbor_chunk_ids: vec![],
        split_reason: split_reason.to_string(),
        body_ref: format!("chunks/{chunk_id}.txt"),
        summary: None,
        semantic_labels: vec![],
        semantic_aliases: vec![],
        block_affinity: None,
    });
    text.clear();
    Ok(())
}

fn split_blocks(text: &str) -> Vec<(Vec<String>, String)> {
    let mut heading = Vec::new();
    let mut blocks = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((level, title)) = markdown_heading(trimmed) {
            if !current.trim().is_empty() {
                blocks.push((heading.clone(), current.trim().to_string()));
                current.clear();
            }
            heading.truncate(level.saturating_sub(1));
            if !title.is_empty() {
                heading.push(title);
            }
            continue;
        }
        if !current.is_empty() || !trimmed.is_empty() {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        blocks.push((heading, current.trim().to_string()));
    }
    blocks
}

fn markdown_heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let title = line[level..].trim();
    if title.is_empty() {
        return None;
    }
    Some((level, title.to_string()))
}

fn split_large_section(text: &str) -> Vec<String> {
    let paragraphs = text
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .collect::<Vec<_>>();
    if paragraphs.is_empty() {
        return vec![];
    }
    let mut parts = Vec::new();
    let mut current = String::new();
    for paragraph in paragraphs {
        let paragraph_tokens = estimate_tokens(paragraph);
        if paragraph_tokens > HARD_MAX_TOKENS {
            flush_text_part(&mut current, &mut parts);
            parts.extend(hard_split(paragraph));
            continue;
        }
        let next_tokens = if current.is_empty() {
            paragraph_tokens
        } else {
            estimate_tokens(&current) + paragraph_tokens
        };
        if !current.is_empty() && next_tokens > TARGET_TOKENS {
            flush_text_part(&mut current, &mut parts);
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(paragraph);
    }
    flush_text_part(&mut current, &mut parts);
    parts
}

fn flush_text_part(current: &mut String, parts: &mut Vec<String>) {
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
        current.clear();
    }
}

fn heading_path_for_part(heading: &[String], part_index: usize, part_count: usize) -> Vec<String> {
    let mut heading_path = heading.to_vec();
    if part_count > 1 {
        heading_path.push(format!("part {} of {}", part_index + 1, part_count));
    }
    heading_path
}

fn hard_split(text: &str) -> Vec<String> {
    let chars = text.chars().collect::<Vec<_>>();
    let window = (HARD_MAX_TOKENS as usize).saturating_mul(2).max(1);
    chars
        .chunks(window)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

fn merge_small_chunks(
    source_id: &str,
    build_id: &str,
    chunks: &mut Vec<KnowledgeChunk>,
) -> KnowledgeResult<()> {
    let mut index = 0;
    while index < chunks.len() {
        if chunks[index].token_estimate >= MIN_TOKENS || chunks.len() == 1 {
            index += 1;
            continue;
        }
        let can_merge_next = index + 1 < chunks.len()
            && chunks[index].document_id == chunks[index + 1].document_id
            && chunks[index].heading_path == chunks[index + 1].heading_path;
        if can_merge_next {
            merge_chunk_pair(source_id, build_id, chunks, index, index + 1)?;
            continue;
        }
        let can_merge_prev = index > 0
            && chunks[index].document_id == chunks[index - 1].document_id
            && chunks[index].heading_path == chunks[index - 1].heading_path;
        if can_merge_prev {
            merge_chunk_pair(source_id, build_id, chunks, index - 1, index)?;
            index = index.saturating_sub(1);
            continue;
        }
        index += 1;
    }
    Ok(())
}

fn fill_neighbors(chunks: &mut [KnowledgeChunk]) {
    for index in 0..chunks.len() {
        let mut neighbors = Vec::new();
        if index > 0 && chunks[index - 1].document_id == chunks[index].document_id {
            neighbors.push(chunks[index - 1].chunk_id.clone());
        }
        if index + 1 < chunks.len() && chunks[index + 1].document_id == chunks[index].document_id {
            neighbors.push(chunks[index + 1].chunk_id.clone());
        }
        chunks[index].neighbor_chunk_ids = neighbors;
    }
}

fn merge_chunk_pair(
    source_id: &str,
    build_id: &str,
    chunks: &mut Vec<KnowledgeChunk>,
    keep_index: usize,
    remove_index: usize,
) -> KnowledgeResult<()> {
    let keep_payload = chunk_payload(source_id, build_id, &chunks[keep_index].chunk_id)?;
    let remove_payload = chunk_payload(source_id, build_id, &chunks[remove_index].chunk_id)?;
    let merged_payload = match (keep_payload.trim(), remove_payload.trim()) {
        ("", right) => right.to_string(),
        (left, "") => left.to_string(),
        (left, right) => format!("{left}\n\n{right}"),
    };
    if merged_payload.trim().is_empty() {
        return Ok(());
    }

    let heading_path = if chunks[keep_index].heading_path.is_empty() {
        chunks[remove_index].heading_path.clone()
    } else {
        chunks[keep_index].heading_path.clone()
    };
    let body_text = render_chunk_body(
        &chunks[keep_index].document_title,
        &chunks[keep_index].source_path,
        &heading_path,
        &merged_payload,
    );
    write_text(
        &paths::chunk_body_file(source_id, build_id, &chunks[keep_index].chunk_id)?,
        &body_text,
    )?;
    chunks[keep_index].heading_path = heading_path;
    chunks[keep_index].token_estimate = estimate_tokens(&body_text);
    chunks[keep_index].context_prefix = body_text.chars().take(CONTEXT_PREFIX_MAX_CHARS).collect();
    chunks[keep_index].split_reason = "merged_small_chunk".to_string();

    let removed_chunk = chunks.remove(remove_index);
    let removed_body = paths::chunk_body_file(source_id, build_id, &removed_chunk.chunk_id)?;
    if removed_body.exists() {
        fs::remove_file(removed_body)?;
    }
    Ok(())
}

pub fn rebuild_lexical_index(
    source_id: &str,
    build_id: &str,
    chunks: &[KnowledgeChunk],
) -> KnowledgeResult<()> {
    let client = algorithm_client()?;
    let mut documents = Vec::new();
    for chunk in chunks {
        let body = fs::read_to_string(paths::chunk_body_file(
            source_id,
            build_id,
            &chunk.chunk_id,
        )?)?;
        let text = lexical_text(chunk, &body);
        let token_response = client
            .call(&json!({"operation": "tokenize", "text": text}))
            .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
        let tokens = token_response["tokens"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        documents.push(LexicalDocument {
            id: chunk.chunk_id.clone(),
            text,
            tokens,
        });
    }
    let tfidf_docs = documents
        .iter()
        .map(|document| json!({"id": document.id, "text": document.text}))
        .collect::<Vec<_>>();
    let keyword_response = client
        .call(&json!({"operation": "tfidf", "documents": tfidf_docs, "limit": 80}))
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let keywords = keyword_response["keywords"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(LexicalKeyword {
                        term: item.get("term")?.as_str()?.to_string(),
                        score: item.get("score").and_then(Value::as_f64).unwrap_or(0.0),
                        document_ids: item
                            .get("documentIds")
                            .and_then(Value::as_array)
                            .map(|ids| {
                                ids.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_string)
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    write_json(
        &paths::lexical_index_file(source_id, build_id)?,
        &LexicalIndex {
            schema_version: 1,
            source_id: source_id.to_string(),
            build_id: build_id.to_string(),
            documents,
            keywords,
        },
    )
}

fn lexical_text(chunk: &KnowledgeChunk, body: &str) -> String {
    let labels = chunk
        .semantic_labels
        .iter()
        .map(|label| label.text.clone())
        .collect::<Vec<_>>();
    weighted_lexical_text(
        &chunk.document_title,
        &chunk.heading_path,
        chunk.summary.as_deref(),
        &labels,
        &chunk.semantic_aliases,
        body,
    )
}

fn weighted_lexical_text(
    title: &str,
    heading_path: &[String],
    summary: Option<&str>,
    labels: &[String],
    aliases: &[String],
    body: &str,
) -> String {
    let heading = heading_path.join(" ");
    let label_text = labels.join(" ");
    let alias_text = aliases.join(" ");
    let summary = summary.unwrap_or_default();
    [
        repeat_field(title, 4),
        repeat_field(&heading, 4),
        repeat_field(summary, 3),
        repeat_field(&label_text, 5),
        repeat_field(&alias_text, 5),
        body.to_string(),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn repeat_field(value: &str, times: usize) -> String {
    std::iter::repeat_n(value.trim(), times)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_chunk_body(
    title: &str,
    source_path: &str,
    heading_path: &[String],
    body: &str,
) -> String {
    format!(
        "Document: {title}\nPath: {source_path}\nSection: {}\n\n{}\n",
        heading_path.join(" > "),
        body.trim()
    )
}

fn chunk_payload(source_id: &str, build_id: &str, chunk_id: &str) -> KnowledgeResult<String> {
    let raw = fs::read_to_string(paths::chunk_body_file(source_id, build_id, chunk_id)?)?;
    Ok(raw
        .split_once("\n\n")
        .map(|(_, body)| body.trim().to_string())
        .unwrap_or_else(|| raw.trim().to_string()))
}

fn pack_chunks(chunks: &[KnowledgeChunk]) -> Vec<Vec<String>> {
    let effective = PACK_BUDGET.saturating_sub(PACK_OVERHEAD);
    let mut packs = Vec::<Vec<String>>::new();
    let mut current = Vec::<String>::new();
    let mut budget = 0u32;
    for chunk in chunks {
        let cost = chunk.token_estimate + PACK_CHUNK_METADATA;
        if !current.is_empty() && budget + cost > effective {
            packs.push(std::mem::take(&mut current));
            budget = 0;
        }
        current.push(chunk.chunk_id.clone());
        budget += cost;
    }
    if !current.is_empty() {
        packs.push(current);
    }
    packs
}

fn algorithm_client() -> KnowledgeResult<AlgorithmClient> {
    AlgorithmClient::from_environment().map_err(|error| KnowledgeError::invalid(error.to_string()))
}

fn is_supported_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "md" | "txt" | "json" | "yaml" | "yml" | "pdf" | "docx"
    )
}

fn ignored_name(name: &str) -> bool {
    matches!(
        name,
        ".git" | ".loom" | "node_modules" | "dist" | "build" | ".DS_Store"
    )
}

fn expand_tilde(raw: &str) -> PathBuf {
    if raw == "~" {
        return std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(raw));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(raw)
}

fn estimate_tokens(text: &str) -> u32 {
    let cjk = text
        .chars()
        .filter(|ch| ('\u{3400}'..='\u{9fff}').contains(ch))
        .count();
    let latin = text.split_whitespace().count();
    ((cjk / 2) + latin).max(1) as u32
}

fn file_sha256(path: &Path) -> KnowledgeResult<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn short_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    digest[..8].to_string()
}

fn summary_language_for_chunk(source_id: &str, build_id: &str, chunk_id: &str) -> String {
    let body = paths::chunk_body_file(source_id, build_id, chunk_id)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();
    if body
        .chars()
        .any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
    {
        "zh-CN".to_string()
    } else {
        "source-language".to_string()
    }
}
