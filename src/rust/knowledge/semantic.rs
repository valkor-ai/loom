use std::fs;

use delivery_core::{
    FileSubmitInput, KnowledgeChunkReadRef, KnowledgeReadMode, LoomMcpActionResult,
    LoomMcpAutoRunnableResult, LoomMcpDoneResult, LoomMcpNextAction, LoomMcpRepairableErrorResult,
    RepairIssue,
};
use serde_json::{json, Value};

use crate::{
    builder::{cleanup_pending_build_runs, rebuild_lexical_index},
    models::{
        BlockAffinity, ChunksFile, SemanticBuildStatus, SemanticChunkFeature, SemanticIndex,
        SemanticLabel, SemanticPackStatus, SemanticState,
    },
    paths,
    store::{
        load_registry, now_string, read_json, remove_file_if_exists, save_registry, write_json,
        KnowledgeError, KnowledgeResult,
    },
};

pub fn semantic_result_template(read_plan: &[KnowledgeChunkReadRef]) -> Value {
    json!({
        "chunkResults": read_plan.iter().map(|_chunk| {
            json!({
                "status": "completed",
                "summary": "",
                "semanticLabels": [{
                    "kind": "object|operation|rule|state|field|page|flow|other",
                    "text": "",
                    "normalizedText": "",
                    "aliases": [],
                    "confidence": "high|medium|low"
                }],
                "blockAffinity": {
                    "phaseScope": 0.0,
                    "conceptGrounding": 0.0,
                    "frontendExperience": 0.0
                }
            })
        }).collect::<Vec<_>>()
    })
}

pub fn semantic_generation_rules() -> Value {
    json!({
        "summaryLanguage": "Use the source chunk language. If the chunk is Chinese, summary must be Chinese.",
        "statusEnum": ["completed", "low_signal", "unreadable"],
        "semanticLabelKinds": ["object", "operation", "rule", "state", "field", "page", "flow", "other"],
        "confidenceEnum": ["high", "medium", "low"],
        "semanticAnchorRule": "Prefer self-contained anchors. For operation labels, use object+operation when the object is explicit in the chunk; keep split object and operation wording in semanticLabels[].aliases.",
        "semanticLabelFieldRules": [
            "semanticLabels[].kind must be one of generationRules.semanticLabelKinds.",
            "semanticLabels[].text is the source-supported label text.",
            "semanticLabels[].normalizedText is the normalized label text used for retrieval.",
            "semanticLabels[].aliases is an array; use [] when there are no aliases.",
            "semanticLabels[].confidence must be one of generationRules.confidenceEnum."
        ],
        "semanticAliasRules": [
            "Put retrieval aliases on semanticLabels[].aliases, not a separate top-level semanticAliases field.",
            "Include object+operation aliases when both are present, for example '<object><operation>'.",
            "Include atomic operation aliases for short user wording, for example the operation without its object.",
            "For each rule label, include one short rule or blocker alias of 4-12 source-language characters/words. Prefer the condition, blocker, or required outcome over copying the whole rule sentence.",
            "For each state or flow label, include one short state/flow goal alias that a user might query.",
            "Do not invent business facts that are not in the chunk. Do not include source ids, chunk ids, or file paths."
        ],
        "chunkResultOrderRule": "Write exactly one chunkResults item for each chunk in chunkReadPlan, in the same order. Loom assigns schemaVersion, buildId, packId, and each chunkId from the request; do not write those machine fields.",
        "blockAffinityFields": ["phaseScope", "conceptGrounding", "frontendExperience"],
        "blockAffinityGuidance": {
            "phaseScope": "Score high when the chunk helps decide phase boundaries, included work, excluded work, deferred work, dependency order, or next-phase handoff.",
            "conceptGrounding": "Score high when the chunk explains objects, operations, fields, states, rules, invariants, preconditions, validation, blocking reasons, outcomes, or misunderstanding boundaries.",
            "frontendExperience": "Score high when the chunk explains a page or workspace surface, target discovery, query and selection, list or detail view, action entry point, form input, success feedback, error or business-blocking feedback, loading or empty state, or refresh/readback behavior for a user-facing or staff-facing workflow."
        },
        "noSourceCodeSchemaLookup": true,
        "noScriptRuleExtraction": true
    })
}

pub fn next_pending_pack(
    _project_root: &str,
    source_name: &str,
    source_id: &str,
    build_id: &str,
) -> KnowledgeResult<LoomMcpNextAction> {
    let state: SemanticState = read_json(&paths::semantic_state_file(source_id, build_id)?)?;
    let pack = state
        .packs
        .iter()
        .find(|pack| matches!(pack.status, SemanticPackStatus::Pending))
        .ok_or_else(|| KnowledgeError::invalid("semantic build has no pending packs"))?;
    let chunks_file: ChunksFile = read_json(&paths::chunks_file(source_id, build_id)?)?;
    let read_plan = pack
        .chunk_ids
        .iter()
        .filter_map(|chunk_id| {
            chunks_file
                .chunks
                .iter()
                .find(|chunk| &chunk.chunk_id == chunk_id)
        })
        .map(|chunk| KnowledgeChunkReadRef {
            source_name: source_name.to_string(),
            source_id: source_id.to_string(),
            build_id: build_id.to_string(),
            chunk_id: chunk.chunk_id.clone(),
            document_title: chunk.document_title.clone(),
            heading_path: chunk.heading_path.clone(),
            token_estimate: chunk.token_estimate,
            summary_language: summary_language(source_id, build_id, &chunk.chunk_id),
            read_tool: "loom.knowledgeInspectChunk".to_string(),
            resource_uri: format!(
                "loom://knowledge/{source_id}/builds/{build_id}/chunks/{}",
                chunk.chunk_id
            ),
        })
        .collect::<Vec<_>>();
    Ok(LoomMcpNextAction::GenerateKnowledgeSemantics(
        delivery_core::GenerateKnowledgeSemanticsNext {
            source_name: source_name.to_string(),
            source_id: source_id.to_string(),
            build_id: build_id.to_string(),
            pack_id: pack.pack_id.clone(),
            pack_index: pack.pack_index,
            pack_count: state.pack_count,
            request_ref: pack.request_ref.clone(),
            result_file: pack.result_file.clone(),
            output_contract: json!({
                "resultTemplate": semantic_result_template(&read_plan)
            }),
            generation_rules: semantic_generation_rules(),
            read_mode: KnowledgeReadMode::ChunkInspect,
            chunk_read_plan: read_plan,
            submit_tool: "loom.knowledgeSemanticSubmitFile".to_string(),
        },
    ))
}

pub fn submit_semantic_pack(
    project_root: &str,
    request_ref: &str,
) -> KnowledgeResult<LoomMcpActionResult> {
    let authorized = match state::authorize_write_targets(
        &FileSubmitInput {
            project_root: project_root.to_string(),
            request_ref: request_ref.to_string(),
            written_target_ids: Some(vec!["semantic_result".to_string()]),
        },
        "loom.knowledgeSemanticSubmitFile",
    ) {
        Ok(value) => value,
        Err(state::WriteTargetAuthorizationError::Repairable {
            target_file,
            target_ids,
            issues,
            read_groups,
            resubmit_tool,
        }) => {
            return Ok(LoomMcpActionResult::RepairableError(
                LoomMcpRepairableErrorResult {
                    project_root: project_root.to_string(),
                    target_file,
                    issues,
                    resubmit_tool,
                    fix_scope: Some("current_pack_result_only".to_string()),
                    target_ids,
                    read_groups,
                },
            ));
        }
        Err(state::WriteTargetAuthorizationError::Fatal { code, message }) => {
            return Err(KnowledgeError::invalid(format!("{code}: {message}")));
        }
    };
    let request_root = read_semantic_request_root(project_root, request_ref)?;
    let source_name = request_root
        .get("sourceName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let source_id = request_root
        .get("sourceId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let build_id = request_root
        .get("buildId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let pack_id = request_root
        .get("packId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let expected_chunk_ids = request_root
        .get("chunkReadPlan")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|chunk| chunk.get("chunkId").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();

    let target = authorized
        .targets
        .first()
        .ok_or_else(|| KnowledgeError::invalid("semantic submit has no authorized target"))?;
    let result_file = std::path::PathBuf::from(project_root).join(&target.path);
    let raw_result: Value = read_json(&result_file)?;
    let result = normalize_semantic_result_machine_fields(
        raw_result,
        &build_id,
        &pack_id,
        &expected_chunk_ids,
    );
    let issues = validate_semantic_result(&result, &build_id, &expected_chunk_ids, &source_id)?;
    if !issues.is_empty() {
        return Ok(LoomMcpActionResult::RepairableError(
            LoomMcpRepairableErrorResult {
                project_root: project_root.to_string(),
                target_file: target.path.clone(),
                target_ids: vec![target.target_id.clone()],
                issues,
                resubmit_tool: "loom.knowledgeSemanticSubmitFile".to_string(),
                fix_scope: Some("current_pack_result_only".to_string()),
                read_groups: authorized.read_groups,
            },
        ));
    }
    write_json(
        &paths::semantic_result_file(&source_id, &build_id, &pack_id)?,
        &result,
    )?;
    let mut state: SemanticState = read_json(&paths::semantic_state_file(&source_id, &build_id)?)?;
    if let Some(pack) = state.packs.iter_mut().find(|pack| pack.pack_id == pack_id) {
        pack.status = SemanticPackStatus::Accepted;
        pack.accepted_at = Some(now_string());
    }
    write_json(&paths::semantic_state_file(&source_id, &build_id)?, &state)?;
    if state
        .packs
        .iter()
        .any(|pack| matches!(pack.status, SemanticPackStatus::Pending))
    {
        let next = next_pending_pack(project_root, &source_name, &source_id, &build_id)?;
        return Ok(LoomMcpActionResult::AutoRunnable(
            LoomMcpAutoRunnableResult::new(project_root, next),
        ));
    }
    publish_build(&source_id, &build_id)?;
    Ok(LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: project_root.to_string(),
        summary: "Knowledge semantic build published.".to_string(),
        details: Some(json!({
            "sourceId": source_id,
            "sourceName": source_name,
            "buildId": build_id,
            "status": "published"
        })),
        warnings: vec![],
    }))
}

fn read_semantic_request_root(project_root: &str, request_ref: &str) -> KnowledgeResult<Value> {
    let request_id = parse_semantic_request_id(request_ref)?;
    let request_index = state::request_index::get_request_index_entry(project_root, &request_id)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let project_paths = state::paths::project_paths(project_root)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let request_file =
        state::paths::from_project_relative(&project_paths.root, &request_index.request_file)
            .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    read_json(&request_file).map_err(|error| KnowledgeError::invalid(error.to_string()))
}

fn parse_semantic_request_id(request_ref: &str) -> KnowledgeResult<String> {
    let rest = request_ref
        .strip_prefix("loom://projects/")
        .ok_or_else(|| KnowledgeError::invalid(format!("invalid requestRef: {request_ref}")))?;
    let request_id = rest
        .split_once("/requests/")
        .and_then(|(_, id)| (!id.is_empty()).then_some(id))
        .ok_or_else(|| KnowledgeError::invalid(format!("invalid requestRef: {request_ref}")))?;
    Ok(request_id.to_string())
}

fn validate_semantic_result(
    result: &Value,
    build_id: &str,
    expected_chunk_ids: &[String],
    source_id: &str,
) -> KnowledgeResult<Vec<RepairIssue>> {
    let mut issues = Vec::new();
    let chunk_results = result
        .get("chunkResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if chunk_results.len() != expected_chunk_ids.len() {
        issues.push(issue(
            "CHUNK_RESULT_COUNT_MISMATCH",
            format!(
                "chunkResults must contain exactly {} item(s) in chunkReadPlan order.",
                expected_chunk_ids.len()
            ),
            Some("chunkResults"),
        ));
    }
    let returned = chunk_results
        .iter()
        .filter_map(|chunk| chunk.get("chunkId").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    for chunk_id in expected_chunk_ids {
        if !returned.contains(chunk_id) {
            issues.push(issue(
                "CHUNK_RESULT_MISSING",
                format!("chunkResults must include {chunk_id}"),
                Some("chunkResults"),
            ));
        }
    }
    for (index, chunk) in chunk_results.iter().enumerate() {
        if index >= expected_chunk_ids.len() {
            continue;
        }
        let chunk_id = chunk
            .get("chunkId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = chunk
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !matches!(status, "completed" | "low_signal" | "unreadable") {
            issues.push(issue(
                "STATUS_INVALID",
                "status must be completed, low_signal, or unreadable.",
                Some("status"),
            ));
        }
        let summary = chunk
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if status == "completed" && summary.trim().is_empty() {
            issues.push(issue(
                "SUMMARY_REQUIRED",
                "summary is required for completed chunks.",
                Some("summary"),
            ));
        }
        if status == "completed"
            && chunk_is_chinese(source_id, build_id, chunk_id)
            && !contains_cjk(summary)
        {
            issues.push(issue(
                "SUMMARY_LANGUAGE_MISMATCH",
                "Chinese chunks require Chinese summary.",
                Some("summary"),
            ));
        }
        let semantic_labels = chunk.get("semanticLabels").and_then(Value::as_array);
        if status == "completed" && semantic_labels.is_none() {
            issues.push(issue(
                "SEMANTIC_LABELS_REQUIRED",
                "semanticLabels must be an array.",
                Some("semanticLabels"),
            ));
        }
        if let Some(labels) = semantic_labels {
            for (index, label) in labels.iter().enumerate() {
                let field_path = format!("semanticLabels[{index}]");
                if label
                    .get("normalizedText")
                    .and_then(Value::as_str)
                    .is_none()
                {
                    issues.push(issue(
                        "SEMANTIC_LABEL_NORMALIZED_TEXT_REQUIRED",
                        "semanticLabels[].normalizedText must be a string.",
                        Some(&field_path),
                    ));
                }
                let aliases = label.get("aliases").and_then(Value::as_array);
                if aliases.map(|items| items.iter().all(|item| item.as_str().is_some()))
                    != Some(true)
                {
                    issues.push(issue(
                        "SEMANTIC_LABEL_ALIASES_REQUIRED",
                        "semanticLabels[].aliases must be an array of strings.",
                        Some(&field_path),
                    ));
                }
            }
        }
        if chunk.get("semanticAliases").is_some() {
            issues.push(issue(
                "SEMANTIC_ALIASES_NOT_ALLOWED",
                "semanticAliases is an old duplicate field. Put retrieval aliases on semanticLabels[].aliases.",
                Some("semanticAliases"),
            ));
        }
        let affinity = chunk.get("blockAffinity");
        for old_field in ["businessRules", "finalSummary"] {
            if affinity.and_then(|value| value.get(old_field)).is_some() {
                issues.push(issue(
                    "BLOCK_AFFINITY_FIELD_NOT_ALLOWED",
                    format!(
                        "blockAffinity.{old_field} is not used by the current MCP knowledge query path."
                    ),
                    Some("blockAffinity"),
                ));
            }
        }
        for field in ["phaseScope", "conceptGrounding", "frontendExperience"] {
            if affinity
                .and_then(|value| value.get(field))
                .and_then(Value::as_f64)
                .is_none()
            {
                issues.push(issue(
                    "BLOCK_AFFINITY_FIELD_REQUIRED",
                    format!("blockAffinity.{field} must be numeric."),
                    Some("blockAffinity"),
                ));
            }
        }
    }
    Ok(issues)
}

fn normalize_semantic_result_machine_fields(
    mut raw: Value,
    build_id: &str,
    pack_id: &str,
    expected_chunk_ids: &[String],
) -> Value {
    let Some(object) = raw.as_object_mut() else {
        return raw;
    };
    object.insert("schemaVersion".to_string(), json!(1));
    object.insert("buildId".to_string(), json!(build_id));
    object.insert("packId".to_string(), json!(pack_id));
    let Some(chunks) = object.get_mut("chunkResults").and_then(Value::as_array_mut) else {
        return raw;
    };
    for (index, chunk) in chunks.iter_mut().enumerate() {
        let Some(chunk) = chunk.as_object_mut() else {
            continue;
        };
        if let Some(chunk_id) = expected_chunk_ids.get(index) {
            chunk.insert("chunkId".to_string(), json!(chunk_id));
        } else {
            chunk.remove("chunkId");
        }
    }
    raw
}

fn publish_build(source_id: &str, build_id: &str) -> KnowledgeResult<()> {
    let mut chunks_file: ChunksFile = read_json(&paths::chunks_file(source_id, build_id)?)?;
    let state: SemanticState = read_json(&paths::semantic_state_file(source_id, build_id)?)?;
    for pack in &state.packs {
        let result: Value = read_json(&paths::semantic_result_file(
            source_id,
            build_id,
            &pack.pack_id,
        )?)?;
        for chunk_result in result
            .get("chunkResults")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let chunk_id = chunk_result
                .get("chunkId")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(chunk) = chunks_file
                .chunks
                .iter_mut()
                .find(|chunk| chunk.chunk_id == chunk_id)
            else {
                continue;
            };
            chunk.summary = chunk_result
                .get("summary")
                .and_then(Value::as_str)
                .map(str::to_string);
            chunk.semantic_labels = parse_labels(chunk_result.get("semanticLabels"));
            chunk.semantic_aliases = semantic_aliases_from_result(chunk_result);
            chunk.block_affinity = Some(parse_affinity(chunk_result.get("blockAffinity")));
        }
    }
    write_json(&paths::chunks_file(source_id, build_id)?, &chunks_file)?;
    rebuild_lexical_index(source_id, build_id, &chunks_file.chunks)?;
    write_json(
        &paths::semantic_index_file(source_id, build_id)?,
        &SemanticIndex {
            schema_version: 1,
            source_id: source_id.to_string(),
            build_id: build_id.to_string(),
            chunk_features: chunks_file
                .chunks
                .iter()
                .map(|chunk| SemanticChunkFeature {
                    chunk_id: chunk.chunk_id.clone(),
                    summary: chunk.summary.clone().unwrap_or_default(),
                    labels: chunk.semantic_labels.clone(),
                    aliases: chunk.semantic_aliases.clone(),
                    block_affinity: chunk.block_affinity.clone().unwrap_or(BlockAffinity {
                        phase_scope: 0.0,
                        concept_grounding: 0.0,
                        frontend_experience: 0.0,
                    }),
                })
                .collect(),
        },
    )?;
    let mut semantic_state: SemanticState =
        read_json(&paths::semantic_state_file(source_id, build_id)?)?;
    semantic_state.status = SemanticBuildStatus::Published;
    semantic_state.published_at = Some(now_string());
    write_json(
        &paths::semantic_state_file(source_id, build_id)?,
        &semantic_state,
    )?;

    let mut registry = load_registry()?;
    if let Some(source) = registry
        .sources
        .iter_mut()
        .find(|source| source.source_id == source_id)
    {
        source.current_build_id = Some(build_id.to_string());
        source.last_built_at = semantic_state.published_at.clone();
        source.updated_at = now_string();
        remove_file_if_exists(&paths::pending_file(source_id)?)?;
    }
    save_registry(&registry)?;
    cleanup_pending_build_runs(source_id, Some(build_id))?;
    Ok(())
}

fn parse_labels(value: Option<&Value>) -> Vec<SemanticLabel> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let mut aliases = item
                .get("aliases")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .filter_map(normalize_semantic_alias)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            aliases.sort();
            aliases.dedup();
            Some(SemanticLabel {
                kind: item.get("kind")?.as_str()?.trim().to_string(),
                text: item.get("text")?.as_str()?.trim().to_string(),
                normalized_text: item
                    .get("normalizedText")
                    .and_then(Value::as_str)
                    .and_then(normalize_semantic_alias),
                aliases,
                confidence: item
                    .get("confidence")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect()
}

fn parse_affinity(value: Option<&Value>) -> BlockAffinity {
    let get = |field: &str| {
        value
            .and_then(|item| item.get(field))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    BlockAffinity {
        phase_scope: get("phaseScope"),
        concept_grounding: get("conceptGrounding"),
        frontend_experience: get("frontendExperience"),
    }
}

fn semantic_aliases_from_result(chunk_result: &Value) -> Vec<String> {
    let mut aliases = Vec::new();
    if let Some(labels) = chunk_result.get("semanticLabels").and_then(Value::as_array) {
        for label in labels {
            if let Some(normalized) = label.get("normalizedText").and_then(Value::as_str) {
                if let Some(alias) = normalize_semantic_alias(normalized) {
                    aliases.push(alias);
                }
            }
            if let Some(items) = label.get("aliases").and_then(Value::as_array) {
                aliases.extend(
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .filter_map(normalize_semantic_alias),
                );
            }
        }
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn normalize_semantic_alias(value: &str) -> Option<String> {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = compact.trim().to_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn issue(
    code: impl Into<String>,
    message: impl Into<String>,
    field_path: Option<&str>,
) -> RepairIssue {
    RepairIssue {
        code: code.into(),
        message: message.into(),
        target_id: Some("semantic_result".to_string()),
        field_path: field_path.map(str::to_string),
    }
}

fn chunk_is_chinese(source_id: &str, build_id: &str, chunk_id: &str) -> bool {
    let body = paths::chunk_body_file(source_id, build_id, chunk_id)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();
    contains_cjk(&body)
}

fn summary_language(source_id: &str, build_id: &str, chunk_id: &str) -> String {
    if chunk_is_chinese(source_id, build_id, chunk_id) {
        "zh-CN".to_string()
    } else {
        "source-language".to_string()
    }
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
}
