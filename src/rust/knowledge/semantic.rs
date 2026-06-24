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

pub fn semantic_result_template(
    build_id: &str,
    pack_id: &str,
    read_plan: &[KnowledgeChunkReadRef],
) -> Value {
    json!({
        "schemaVersion": 1,
        "buildId": build_id,
        "packId": pack_id,
        "chunkResults": read_plan.iter().map(|chunk| {
            json!({
                "chunkId": chunk.chunk_id,
                "status": "ok",
                "summary": "",
                "semanticLabels": [{
                    "kind": "object|operation|rule|state|field|flow|page_operation",
                    "text": "",
                    "confidence": "high|medium|low"
                }],
                "semanticAliases": [],
                "blockAffinity": {
                    "phaseScope": 0.0,
                    "conceptGrounding": 0.0,
                    "frontendExperience": 0.0,
                    "businessRules": 0.0
                }
            })
        }).collect::<Vec<_>>()
    })
}

pub fn semantic_generation_rules() -> Value {
    json!({
        "summaryLanguage": "Use the source chunk language. If the chunk is Chinese, summary must be Chinese.",
        "statusEnum": ["ok", "unreadable"],
        "semanticLabelKinds": ["object", "operation", "rule", "state", "field", "flow", "page_operation"],
        "confidenceEnum": ["high", "medium", "low"],
        "semanticAnchorRule": "Prefer self-contained anchors. For operations, include object+operation when the object is known; aliases may include split object and operation forms.",
        "blockAffinityFields": ["phaseScope", "conceptGrounding", "frontendExperience", "businessRules"],
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
                "resultTemplate": semantic_result_template(build_id, &pack.pack_id, &read_plan)
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
    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
        fields: vec![
            "sourceName".to_string(),
            "sourceId".to_string(),
            "buildId".to_string(),
            "packId".to_string(),
            "chunkReadPlan".to_string(),
        ],
    })
    .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let source_name = fields.fields["sourceName"]
        .value
        .as_str()
        .unwrap_or_default()
        .to_string();
    let source_id = fields.fields["sourceId"]
        .value
        .as_str()
        .unwrap_or_default()
        .to_string();
    let build_id = fields.fields["buildId"]
        .value
        .as_str()
        .unwrap_or_default()
        .to_string();
    let pack_id = fields.fields["packId"]
        .value
        .as_str()
        .unwrap_or_default()
        .to_string();
    let expected_chunk_ids = fields.fields["chunkReadPlan"]
        .value
        .as_array()
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
    let result: Value = read_json(&result_file)?;
    let issues = validate_semantic_result(
        &result,
        &build_id,
        &pack_id,
        &expected_chunk_ids,
        &source_id,
    )?;
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

fn validate_semantic_result(
    result: &Value,
    build_id: &str,
    pack_id: &str,
    expected_chunk_ids: &[String],
    source_id: &str,
) -> KnowledgeResult<Vec<RepairIssue>> {
    let mut issues = Vec::new();
    if result.get("buildId").and_then(Value::as_str) != Some(build_id) {
        issues.push(issue(
            "BUILD_ID_MISMATCH",
            "buildId must match request.",
            Some("buildId"),
        ));
    }
    if result.get("packId").and_then(Value::as_str) != Some(pack_id) {
        issues.push(issue(
            "PACK_ID_MISMATCH",
            "packId must match request.",
            Some("packId"),
        ));
    }
    let chunk_results = result
        .get("chunkResults")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
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
    for chunk in &chunk_results {
        let chunk_id = chunk
            .get("chunkId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = chunk
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !matches!(status, "ok" | "unreadable") {
            issues.push(issue(
                "STATUS_INVALID",
                "status must be ok or unreadable.",
                Some("status"),
            ));
        }
        let summary = chunk
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if status == "ok" && summary.trim().is_empty() {
            issues.push(issue(
                "SUMMARY_REQUIRED",
                "summary is required for ok chunks.",
                Some("summary"),
            ));
        }
        if status == "ok"
            && chunk_is_chinese(source_id, build_id, chunk_id)
            && !contains_cjk(summary)
        {
            issues.push(issue(
                "SUMMARY_LANGUAGE_MISMATCH",
                "Chinese chunks require Chinese summary.",
                Some("summary"),
            ));
        }
        if status == "ok"
            && !chunk
                .get("semanticLabels")
                .and_then(Value::as_array)
                .is_some()
        {
            issues.push(issue(
                "SEMANTIC_LABELS_REQUIRED",
                "semanticLabels must be an array.",
                Some("semanticLabels"),
            ));
        }
        let affinity = chunk.get("blockAffinity");
        for field in [
            "phaseScope",
            "conceptGrounding",
            "frontendExperience",
            "businessRules",
        ] {
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
            chunk.semantic_aliases = chunk_result
                .get("semanticAliases")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
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
                        business_rules: 0.0,
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
            Some(SemanticLabel {
                kind: item.get("kind")?.as_str()?.to_string(),
                text: item.get("text")?.as_str()?.to_string(),
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
        business_rules: get("businessRules"),
    }
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
