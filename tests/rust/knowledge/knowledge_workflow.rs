use std::sync::{Mutex, MutexGuard};

use delivery_core::{LoomMcpActionResult, LoomMcpNextAction};
use knowledge::{
    add_source, brainstorm_context, build_source, disable_source, discard_pending, inspect_chunk,
    list_sources,
    mcp_models::{
        KnowledgeAddInput, KnowledgeBrainstormContextInput, KnowledgeInspectChunkInput,
        KnowledgeNameInput, KnowledgePendingInput, KnowledgeProjectInput, KnowledgeSearchInput,
    },
    pending_sources, remove_source, search_knowledge, source_status, submit_semantic_pack,
};
use serde_json::json;

#[test]
fn knowledge_build_submit_publish_search_and_disable_are_mcp_native() {
    let fixture = Fixture::new("publish-search");
    let document = fixture.write_file(
        "docs/stock.md",
        r#"# 证券账户业务

证券账户是交易身份和股票持仓归属账户。工作人员可以办理证券账户开户、挂失补办和销户。

开户需要校验证券从业人员、未获监护许可的未成年人、未授权代理法人开户者、市场禁入期未满者不能开户。

销户前必须清空该账户持仓；账户仍有持仓时禁止销户。挂失后需要冻结原账户证券，补办后恢复交易前需要重新关联资金账户。
"#,
    );
    fixture.write_file("docs/ignored.bin", "unsupported");

    let added = add_source(KnowledgeAddInput {
        project_root: fixture.root_str().to_string(),
        name: "stock-rules".to_string(),
        paths: vec![fixture.root.join("docs").to_string_lossy().into_owned()],
    })
    .expect("knowledge add");
    assert_eq!(added.source.name, "stock-rules");
    assert!(added
        .pending
        .as_ref()
        .is_some_and(|queue| queue.operations.len() == 1));
    assert!(added
        .warnings
        .iter()
        .any(|warning| warning.path.ends_with("ignored.bin")));
    assert!(added.created_at_local.contains(char::is_whitespace));

    let listed = list_sources(KnowledgeProjectInput {
        project_root: fixture.root_str().to_string(),
    })
    .expect("knowledge list");
    assert_eq!(listed.sources.len(), 1);

    let next = build_source(fixture.root_str(), "stock-rules").expect("knowledge build");
    let next = match next {
        LoomMcpActionResult::AutoRunnable(result) => result.next,
        other => panic!("expected auto_runnable build result, got {other:?}"),
    };
    let semantic = match next {
        LoomMcpNextAction::GenerateKnowledgeSemantics(semantic) => semantic,
        other => panic!("expected semantic next action, got {other:?}"),
    };

    let semantic_json = serde_json::to_value(&semantic).expect("semantic next json");
    assert!(!semantic_json.to_string().contains("submitCommand"));
    assert!(!semantic_json.to_string().contains("readCommand"));
    assert!(!semantic_json.to_string().contains("argv"));
    assert!(!semantic_json.to_string().contains("builder.rs"));
    assert_eq!(semantic.submit_tool, "loom.knowledgeSemanticSubmitFile");
    assert!(!semantic.chunk_read_plan.is_empty());
    assert!(fixture
        .root
        .join(&semantic.result_file)
        .parent()
        .unwrap()
        .exists());

    let fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: semantic.request_ref.clone(),
        fields: vec![
            "outputContract.resultTemplate".to_string(),
            "generationRules".to_string(),
            "chunkReadPlan".to_string(),
        ],
    })
    .expect("semantic request fields");
    assert_eq!(
        fields.fields["outputContract.resultTemplate"].value["buildId"],
        semantic.build_id
    );
    assert_eq!(
        fields.fields["generationRules"].value["noScriptRuleExtraction"],
        true
    );
    assert!(fields.fields["generationRules"].value["semanticAliasRules"]
        .as_array()
        .expect("semanticAliasRules")
        .iter()
        .any(|rule| rule
            .as_str()
            .unwrap_or_default()
            .contains("object+operation aliases")));

    let first_chunk = semantic.chunk_read_plan.first().expect("first chunk");
    let inspected = inspect_chunk(KnowledgeInspectChunkInput {
        project_root: fixture.root_str().to_string(),
        source_name: "stock-rules".to_string(),
        source_id: Some(first_chunk.source_id.clone()),
        build_id: semantic.build_id.clone(),
        chunk_id: first_chunk.chunk_id.clone(),
    })
    .expect("inspect chunk");
    let inspected_json = serde_json::to_value(&inspected).expect("inspect json");
    assert!(inspected_json.get("sourceName").is_none());
    assert!(inspected_json.get("sourceId").is_none());
    assert!(inspected_json.get("buildId").is_none());
    assert!(inspected_json.get("chunkId").is_none());
    assert!(inspected.text.contains("证券账户"));
    assert!(inspected
        .text
        .contains(&document.to_string_lossy().to_string()));

    let invalid_result = json!({
        "schemaVersion": 1,
        "buildId": semantic.build_id,
        "packId": semantic.pack_id,
        "chunkResults": semantic.chunk_read_plan.iter().map(|chunk| {
            json!({
                "chunkId": chunk.chunk_id,
                "status": "completed",
                "summary": "",
                "semanticLabels": [],
                "semanticAliases": ["旧重复字段"],
                "blockAffinity": {
                    "phaseScope": 0.2,
                    "conceptGrounding": 0.9,
                    "frontendExperience": 0.2,
                    "businessRules": 0.8
                }
            })
        }).collect::<Vec<_>>()
    });
    std::fs::write(
        fixture.root.join(&semantic.result_file),
        serde_json::to_string_pretty(&invalid_result).expect("invalid result json"),
    )
    .expect("write invalid result");
    let repair = submit_semantic_pack(fixture.root_str(), &semantic.request_ref)
        .expect("submit invalid semantic");
    match repair {
        LoomMcpActionResult::RepairableError(error) => {
            assert_eq!(error.fix_scope.as_deref(), Some("current_pack_result_only"));
            assert!(error
                .issues
                .iter()
                .any(|issue| issue.code == "SUMMARY_REQUIRED"));
            assert!(error
                .issues
                .iter()
                .any(|issue| issue.code == "SEMANTIC_ALIASES_NOT_ALLOWED"));
            assert!(error
                .issues
                .iter()
                .any(|issue| issue.code == "BLOCK_AFFINITY_FIELD_NOT_ALLOWED"));
        }
        other => panic!("expected repairable semantic result, got {other:?}"),
    }

    let valid_result = json!({
        "schemaVersion": 1,
        "buildId": semantic.build_id,
        "packId": semantic.pack_id,
        "chunkResults": semantic.chunk_read_plan.iter().map(|chunk| {
            json!({
                "chunkId": chunk.chunk_id,
                "status": "completed",
                "summary": "证券账户开户、挂失补办、销户和持仓清空限制。",
                "semanticLabels": [
                    {"kind": "object", "text": "证券账户", "normalizedText": "证券账户", "aliases": [], "confidence": "high"},
                    {"kind": "operation", "text": "证券账户开户", "normalizedText": " 证券账户开户 ", "aliases": ["开户", " 开户 "], "confidence": "high"},
                    {"kind": "operation", "text": "销户", "normalizedText": "销户", "aliases": ["证券账户销户", " 证券账户销户 "], "confidence": "high"},
                    {"kind": "rule", "text": "持仓清空后方可销户", "normalizedText": "持仓清空后方可销户", "aliases": ["恢复交易能力", "RESTORE TRADE"], "confidence": "high"}
                ],
                "blockAffinity": {
                    "phaseScope": 0.7,
                    "conceptGrounding": 1.0,
                    "frontendExperience": 0.4
                }
            })
        }).collect::<Vec<_>>()
    });
    std::fs::write(
        fixture.root.join(&semantic.result_file),
        serde_json::to_string_pretty(&valid_result).expect("valid result json"),
    )
    .expect("write valid result");
    let published = submit_semantic_pack(fixture.root_str(), &semantic.request_ref)
        .expect("submit valid semantic");
    assert!(matches!(published, LoomMcpActionResult::Done(_)));

    let registry = knowledge::store::load_registry().expect("registry");
    let source = registry
        .sources
        .iter()
        .find(|source| source.name == "stock-rules")
        .expect("registered source");
    let build_id = source.current_build_id.as_ref().expect("current build");
    assert_eq!(build_id, &semantic.build_id);
    assert!(source.last_built_at.is_some());
    let chunks_file: knowledge::models::ChunksFile = knowledge::store::read_json(
        &knowledge::paths::chunks_file(&source.source_id, build_id).expect("chunks path"),
    )
    .expect("chunks file");
    let stored_aliases = chunks_file
        .chunks
        .iter()
        .flat_map(|chunk| chunk.semantic_aliases.iter().cloned())
        .collect::<Vec<_>>();
    assert!(stored_aliases.contains(&"证券账户开户".to_string()));
    assert!(stored_aliases.contains(&"开户".to_string()));
    assert!(stored_aliases.contains(&"restore trade".to_string()));
    assert_eq!(
        stored_aliases
            .iter()
            .filter(|alias| alias.as_str() == "开户")
            .count(),
        1
    );
    let semantic_state: knowledge::models::SemanticState = knowledge::store::read_json(
        &knowledge::paths::semantic_state_file(&source.source_id, build_id).expect("state path"),
    )
    .expect("semantic state");
    assert!(semantic_state.published_at.is_some());
    assert!(
        knowledge::paths::semantic_index_file(&source.source_id, build_id)
            .expect("semantic index path")
            .exists()
    );
    assert!(
        knowledge::paths::lexical_index_file(&source.source_id, build_id)
            .expect("lexical index path")
            .exists()
    );
    let lexical_index: knowledge::models::LexicalIndex = knowledge::store::read_json(
        &knowledge::paths::lexical_index_file(&source.source_id, build_id)
            .expect("lexical index path"),
    )
    .expect("lexical index");
    let lexical_text = lexical_index
        .documents
        .iter()
        .map(|document| document.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(lexical_text.contains("恢复交易能力"));
    assert!(lexical_text.contains("证券账户销户"));

    let search = search_knowledge(KnowledgeSearchInput {
        project_root: fixture.root_str().to_string(),
        natural_language_query: "证券账户开户和销户规则".to_string(),
        semantic_focus: vec![
            "证券账户".to_string(),
            "开户".to_string(),
            "销户".to_string(),
        ],
        source_names: vec![],
        block: Some("concept_grounding".to_string()),
        limit: Some(5),
    })
    .expect("knowledge search");
    assert_eq!(search.status, "available");
    assert!(!search.cards.is_empty());
    let search_json = serde_json::to_value(&search).expect("search json");
    assert!(search_json.get("matchedSources").is_none());
    let card_json = serde_json::to_value(&search.cards[0]).expect("card json");
    let card_object = card_json.as_object().expect("card object");
    assert!(!card_object.contains_key("text"));
    assert!(!card_object.contains_key("sourceId"));
    assert!(!card_object.contains_key("semanticLabels"));
    assert!(!serde_json::to_value(&search.cards[0])
        .expect("card json")
        .as_object()
        .expect("card object")
        .contains_key("inspect"));

    let suffix_search = search_knowledge(KnowledgeSearchInput {
        project_root: fixture.root_str().to_string(),
        natural_language_query: "补办后的恢复交易条件".to_string(),
        semantic_focus: vec!["恢复交易条件".to_string()],
        source_names: vec![],
        block: Some("concept_grounding".to_string()),
        limit: Some(5),
    })
    .expect("suffix-compatible knowledge search");
    assert_eq!(suffix_search.status, "available");
    assert!(suffix_search.cards.iter().any(|card| card
        .matched_labels
        .iter()
        .any(|label| label.text == "持仓清空后方可销户")));

    disable_source(KnowledgeNameInput {
        project_root: fixture.root_str().to_string(),
        name: "stock-rules".to_string(),
    })
    .expect("disable source");
    let disabled_search = search_knowledge(KnowledgeSearchInput {
        project_root: fixture.root_str().to_string(),
        natural_language_query: "证券账户开户".to_string(),
        semantic_focus: vec!["证券账户开户".to_string()],
        source_names: vec![],
        block: Some("phase_scope".to_string()),
        limit: Some(5),
    })
    .expect("disabled search");
    assert_eq!(disabled_search.status, "empty");
}

#[test]
fn knowledge_cleanup_tools_are_idempotent_and_include_pending_only_state() {
    let fixture = Fixture::new("pending-only-cleanup");
    let pending_dir = fixture.root.join(".loom-home/knowledge/pending");
    std::fs::create_dir_all(&pending_dir).expect("pending dir");
    std::fs::write(
        pending_dir.join("ksrc_orphan.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1,
            "sourceId": "ksrc_orphan",
            "sourceName": "orphan-rules",
            "operations": [{
                "operationId": "kop_orphan",
                "kind": "add_paths",
                "paths": [fixture.root.join("docs").to_string_lossy().to_string()],
                "createdAt": "2026-06-30T00:00:00Z"
            }]
        }))
        .expect("pending json"),
    )
    .expect("write pending");

    let listed = list_sources(KnowledgeProjectInput {
        project_root: fixture.root_str().to_string(),
    })
    .expect("list pending-only");
    assert_eq!(listed.sources.len(), 1);
    assert_eq!(listed.sources[0].source.name, "orphan-rules");
    assert!(listed.sources[0].pending.is_some());

    let pending = pending_sources(KnowledgePendingInput {
        project_root: fixture.root_str().to_string(),
        name: Some("orphan-rules".to_string()),
    })
    .expect("pending by name");
    assert_eq!(pending.sources.len(), 1);

    let status = source_status(KnowledgeNameInput {
        project_root: fixture.root_str().to_string(),
        name: "orphan-rules".to_string(),
    })
    .expect("status pending-only");
    assert!(status.source.is_none());
    assert!(status.pending.is_some());

    let discarded = discard_pending(KnowledgeNameInput {
        project_root: fixture.root_str().to_string(),
        name: "orphan-rules".to_string(),
    })
    .expect("discard pending-only");
    assert!(discarded.discarded);

    let removed_missing = remove_source(KnowledgeNameInput {
        project_root: fixture.root_str().to_string(),
        name: "orphan-rules".to_string(),
    })
    .expect("remove after discard");
    assert!(!removed_missing.removed_source);
    assert!(!removed_missing.removed_pending);
}

#[test]
fn legacy_cli_pending_knowledge_can_be_listed_and_discarded() {
    let fixture = Fixture::new("legacy-cli-pending");
    let pending_dir = fixture.root.join(".loom-home/knowledge/pending");
    std::fs::create_dir_all(&pending_dir).expect("pending dir");
    std::fs::write(
        pending_dir.join("legacy-cli-pending.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": "1.0",
            "name": "legacy-cli-rules",
            "sourceId": null,
            "createNew": true,
            "operations": [{
                "type": "add_paths",
                "paths": [fixture.root.join("docs/legacy.md").to_string_lossy().to_string()]
            }],
            "validation": {
                "acceptedPaths": [],
                "acceptedFiles": 0,
                "acceptedDirectories": 0,
                "supportedFiles": 0,
                "skippedFiles": [],
                "maxFileBytes": 20971520
            },
            "createdAt": "2026-06-30T00:00:00Z",
            "updatedAt": "2026-06-30T00:00:00Z"
        }))
        .expect("legacy pending json"),
    )
    .expect("write legacy pending");

    let listed = list_sources(KnowledgeProjectInput {
        project_root: fixture.root_str().to_string(),
    })
    .expect("list legacy pending");
    assert_eq!(listed.sources.len(), 1);
    assert_eq!(listed.sources[0].source.name, "legacy-cli-rules");
    assert_eq!(
        listed.sources[0].pending.as_ref().unwrap().operations[0].kind,
        knowledge::models::PendingOperationKind::AddPaths
    );

    let status = source_status(KnowledgeNameInput {
        project_root: fixture.root_str().to_string(),
        name: "legacy-cli-rules".to_string(),
    })
    .expect("legacy pending status");
    assert!(status.source.is_none());
    assert!(status.pending.is_some());

    let discarded = discard_pending(KnowledgeNameInput {
        project_root: fixture.root_str().to_string(),
        name: "legacy-cli-rules".to_string(),
    })
    .expect("discard legacy pending");
    assert!(discarded.discarded);
}

#[test]
fn knowledge_update_remove_path_does_not_require_existing_file() {
    let fixture = Fixture::new("remove-missing-path");
    let document = fixture.write_file("docs/stock.md", "# 证券账户\n\n证券账户开户规则。");
    add_source(KnowledgeAddInput {
        project_root: fixture.root_str().to_string(),
        name: "remove-rules".to_string(),
        paths: vec![document.to_string_lossy().into_owned()],
    })
    .expect("add source");

    let removed_path = fixture
        .root
        .join("docs/deleted-before-remove.md")
        .to_string_lossy()
        .into_owned();
    let updated = knowledge::update_source(knowledge::mcp_models::KnowledgeUpdateInput {
        project_root: fixture.root_str().to_string(),
        name: "remove-rules".to_string(),
        add_paths: vec![],
        remove_paths: vec![removed_path.clone()],
        replace_paths: vec![],
    })
    .expect("remove missing path should be queued");

    let pending = updated.pending.expect("pending remove queue");
    assert!(pending.operations.iter().any(|operation| {
        matches!(
            operation.kind,
            knowledge::models::PendingOperationKind::RemovePaths
        ) && operation
            .paths
            .iter()
            .any(|path| path.ends_with("deleted-before-remove.md"))
    }));
    assert!(updated.warnings.is_empty());
}

#[test]
fn knowledge_build_keeps_markdown_heading_sections_as_chunk_boundaries() {
    let fixture = Fixture::new("heading-boundaries");
    fixture.write_file(
        "docs/stock.md",
        r#"# 股票交易业务领域知识库

## 4. 账户生命周期

### 4.1 证券账户开户

证券账户开户是投资者进入股市的第一步。开户完成后，投资者获得证券交易身份。

### 4.2 证券账户挂失与补办

证券账户丢失时，需要办理挂失和重新开户手续。挂失不是删除原账户，补办会产生新的证券账户凭证。

### 4.3 证券账户销户

证券账户销户表示投资者不再使用某个证券账户。持仓未清空时不能销户。

### 4.4 资金账户开户

资金账户开户是在证券经纪商处建立交易结算资金账户，并与已有证券账户关联。
"#,
    );
    add_source(KnowledgeAddInput {
        project_root: fixture.root_str().to_string(),
        name: "section-rules".to_string(),
        paths: vec![fixture.root.join("docs").to_string_lossy().into_owned()],
    })
    .expect("add section source");
    let next = build_source(fixture.root_str(), "section-rules").expect("build section source");
    let semantic = match next {
        LoomMcpActionResult::AutoRunnable(result) => match result.next {
            LoomMcpNextAction::GenerateKnowledgeSemantics(semantic) => semantic,
            other => panic!("expected semantic next action, got {other:?}"),
        },
        other => panic!("expected auto_runnable build result, got {other:?}"),
    };
    let chunks_file: knowledge::models::ChunksFile = knowledge::store::read_json(
        &knowledge::paths::chunks_file(&semantic.source_id, &semantic.build_id)
            .expect("chunks path"),
    )
    .expect("chunks file");
    let headings = chunks_file
        .chunks
        .iter()
        .map(|chunk| chunk.heading_path.join(" / "))
        .collect::<Vec<_>>();
    assert_eq!(chunks_file.chunks.len(), 4);
    assert!(headings
        .iter()
        .any(|heading| heading.ends_with("4.1 证券账户开户")));
    assert!(headings
        .iter()
        .any(|heading| heading.ends_with("4.2 证券账户挂失与补办")));
    assert!(headings
        .iter()
        .any(|heading| heading.ends_with("4.3 证券账户销户")));
    assert!(headings
        .iter()
        .any(|heading| heading.ends_with("4.4 资金账户开户")));
    let fund_chunk = chunks_file
        .chunks
        .iter()
        .find(|chunk| chunk.heading_path.last().map(String::as_str) == Some("4.4 资金账户开户"))
        .expect("fund account chunk");
    let fund_body = std::fs::read_to_string(
        knowledge::paths::chunk_body_file(
            &semantic.source_id,
            &semantic.build_id,
            &fund_chunk.chunk_id,
        )
        .expect("fund body path"),
    )
    .expect("fund body");
    assert!(fund_body.contains("资金账户开户"));
    assert!(!fund_body.contains("证券账户销户表示"));
}

#[test]
fn legacy_cli_knowledge_store_can_be_listed_searched_and_inspected() {
    let fixture = Fixture::new("legacy-cli-store");
    let source_id = "ksrc_legacy_stock_rules";
    let build_id = "kbld_legacy_stock_rules";
    let loom_home = fixture.root.join(".loom-home");
    let build_dir = loom_home
        .join("knowledge/sources")
        .join(source_id)
        .join("build-runs")
        .join(build_id);
    std::fs::create_dir_all(build_dir.join("chunks")).expect("legacy chunks dir");
    std::fs::write(
        loom_home.join("knowledge/registry.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": "1.0",
            "sources": [{
                "sourceId": source_id,
                "name": "stock-trade-rules",
                "status": "enabled",
                "roots": [{
                    "type": "file",
                    "path": "/legacy/StockTradingSystem.md"
                }],
                "index": {
                    "version": 1,
                    "lastBuiltAt": "2026-06-19T03:32:52.815Z",
                    "currentBuildId": build_id,
                    "documentCount": 1,
                    "chunkCount": 1
                },
                "createdAt": "2026-06-19T03:32:52.815Z",
                "updatedAt": "2026-06-19T03:32:52.815Z"
            }]
        }))
        .expect("legacy registry json"),
    )
    .expect("legacy registry");
    std::fs::write(
        build_dir.join("chunks.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": "1.0",
            "sourceId": source_id,
            "sourceName": null,
            "buildId": build_id,
            "chunks": [{
                "chunkId": "kchunk_000001",
                "documentId": "kdoc_000001",
                "sourceId": source_id,
                "title": "股票交易业务领域知识库",
                "headingPath": ["证券账户业务"],
                "textRef": "chunks/kchunk_000001.txt",
                "tokenEstimate": 120,
                "neighborChunkIds": [],
                "contextPrefix": "证券账户是交易身份和持仓归属账户。",
                "splitReason": "section",
                "retrievalFields": {
                    "summary": "证券账户开户、挂失补办、销户和持仓清空规则。",
                    "semanticLabelTexts": ["证券账户", "证券账户开户", "持仓清空后方可销户"],
                    "semanticAliases": ["开户", "销户", "恢复交易条件"],
                    "bodyTextRef": "chunks/kchunk_000001.txt"
                },
                "semanticLabels": [
                    {"kind": "object", "text": "证券账户", "normalizedText": "证券账户", "confidence": "high"},
                    {"kind": "operation", "text": "证券账户开户", "normalizedText": "证券账户开户", "confidence": "high"},
                    {"kind": "rule", "text": "持仓清空后方可销户", "normalizedText": "持仓清空后方可销户", "confidence": "high"}
                ],
                "blockAffinity": {
                    "phaseScope": 0.8,
                    "conceptGrounding": 1.0,
                    "frontendExperience": 0.2,
                    "finalSummary": 0.7
                }
            }]
        }))
        .expect("legacy chunks json"),
    )
    .expect("legacy chunks");
    std::fs::write(
        build_dir.join("chunks/kchunk_000001.txt"),
        "证券账户开户是交易身份的上游能力。销户前必须清空该账户持仓。",
    )
    .expect("legacy chunk body");
    std::fs::write(
        build_dir.join("lexical-index.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": "1.0",
            "sourceId": source_id,
            "buildId": build_id,
            "terms": {},
            "documentLengths": {},
            "averageDocumentLength": 0,
            "fieldWeights": {},
            "chunkCount": 1
        }))
        .expect("legacy lexical json"),
    )
    .expect("legacy lexical");

    let listed = list_sources(KnowledgeProjectInput {
        project_root: fixture.root_str().to_string(),
    })
    .expect("legacy list");
    assert_eq!(
        listed.sources[0].source.current_build_id.as_deref(),
        Some(build_id)
    );
    assert!(listed.sources[0].source.enabled);

    let search = search_knowledge(KnowledgeSearchInput {
        project_root: fixture.root_str().to_string(),
        natural_language_query: "证券账户开户和销户规则".to_string(),
        semantic_focus: vec!["证券账户".to_string(), "销户".to_string()],
        source_names: vec![],
        block: Some("phase_scope".to_string()),
        limit: Some(5),
    })
    .expect("legacy search");
    assert_eq!(search.status, "available");
    assert_eq!(search.cards[0].source_name, "stock-trade-rules");
    assert_eq!(
        search.cards[0].summary.as_deref(),
        Some("证券账户开户、挂失补办、销户和持仓清空规则。")
    );

    let inspected = inspect_chunk(KnowledgeInspectChunkInput {
        project_root: fixture.root_str().to_string(),
        source_name: search.cards[0].source_name.clone(),
        source_id: Some(search.cards[0].source_id.clone()),
        build_id: search.cards[0].build_id.clone(),
        chunk_id: search.cards[0].chunk_id.clone(),
    })
    .expect("legacy inspect");
    assert!(inspected.text.contains("销户前必须清空"));
}

#[test]
fn knowledge_search_prioritizes_semantic_focus_coverage_before_lexical_fallback() {
    let fixture = Fixture::new("semantic-focus-rerank");
    let source_id = "ksrc_focus_rerank";
    let build_id = "kbld_focus_rerank";
    let loom_home = fixture.root.join(".loom-home");
    let build_dir = loom_home
        .join("knowledge/sources")
        .join(source_id)
        .join("build-runs")
        .join(build_id);
    std::fs::create_dir_all(build_dir.join("chunks")).expect("rerank chunks dir");
    std::fs::write(
        loom_home.join("knowledge/registry.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1,
            "sources": [{
                "sourceId": source_id,
                "name": "focus-rules",
                "enabled": true,
                "documentPaths": ["/fixture/focus.md"],
                "currentBuildId": build_id,
                "createdAt": "2026-06-26T00:00:00Z",
                "updatedAt": "2026-06-26T00:00:00Z",
                "lastBuiltAt": "2026-06-26T00:00:00Z"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
    std::fs::write(
        build_dir.join("chunks.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1,
            "sourceId": source_id,
            "sourceName": "focus-rules",
            "buildId": build_id,
            "chunks": [
                focus_chunk("kchunk_000001", "证券账户开户", "证券账户开户是生命周期起点。", 0.2),
                focus_chunk("kchunk_000002", "证券账户挂失与补办", "证券账户挂失与补办是风险保护流程。", 0.2),
                focus_chunk("kchunk_000003", "证券账户销户", "证券账户销户是生命周期结束。", 0.2),
                semantic_chunk(
                    "kchunk_000004",
                    "账户状态背景",
                    "证券账户生命周期 开户 挂失 补办 销户 账户状态。证券账户生命周期 开户 挂失 补办 销户 账户状态。证券账户生命周期 开户 挂失 补办 销户 账户状态。",
                    json!([
                        {"kind": "object", "text": "证券账户", "confidence": "high"},
                        {"kind": "field", "text": "账户状态", "confidence": "high"},
                        {"kind": "state", "text": "挂失", "confidence": "high"}
                    ]),
                    json!(["证券账户", "账户状态", "挂失"]),
                    1.0,
                ),
                semantic_chunk(
                    "kchunk_000005",
                    "资金账户挂失与补办",
                    "证券账户生命周期 开户 挂失 补办 销户。证券账户生命周期 开户 挂失 补办 销户。证券账户生命周期 开户 挂失 补办 销户。",
                    json!([
                        {"kind": "page_operation", "text": "资金账户挂失与补办", "confidence": "high"},
                        {"kind": "operation", "text": "冻结关联证券账户下所有证券", "confidence": "medium"}
                    ]),
                    json!(["资金账户挂失", "资金账户补办"]),
                    1.0,
                )
            ]
        }))
        .expect("chunks json"),
    )
    .expect("write chunks");
    for (chunk_id, text) in [
        ("kchunk_000001", "证券账户开户是生命周期起点。"),
        ("kchunk_000002", "证券账户挂失与补办是风险保护流程。"),
        ("kchunk_000003", "证券账户销户是生命周期结束。"),
        (
            "kchunk_000004",
            "证券账户生命周期 开户 挂失 补办 销户 账户状态。证券账户生命周期 开户 挂失 补办 销户 账户状态。证券账户生命周期 开户 挂失 补办 销户 账户状态。",
        ),
        (
            "kchunk_000005",
            "证券账户生命周期 开户 挂失 补办 销户。证券账户生命周期 开户 挂失 补办 销户。证券账户生命周期 开户 挂失 补办 销户。",
        ),
    ] {
        std::fs::write(build_dir.join("chunks").join(format!("{chunk_id}.txt")), text)
            .expect("write chunk body");
    }

    let search = search_knowledge(KnowledgeSearchInput {
        project_root: fixture.root_str().to_string(),
        natural_language_query: "证券账户生命周期 开户 挂失 销户".to_string(),
        semantic_focus: vec![
            "证券账户开户".to_string(),
            "证券账户挂失与补办".to_string(),
            "证券账户销户".to_string(),
        ],
        source_names: vec![],
        block: Some("phase_scope".to_string()),
        limit: Some(3),
    })
    .expect("reranked search");
    let chunk_ids = search
        .cards
        .iter()
        .map(|card| card.chunk_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        chunk_ids,
        vec!["kchunk_000001", "kchunk_000002", "kchunk_000003"]
    );
}

#[test]
fn knowledge_search_respects_typed_semantic_focus_kinds() {
    let fixture = Fixture::new("typed-semantic-focus");
    let source_id = "ksrc_typed_focus";
    let build_id = "kbld_typed_focus";
    let loom_home = fixture.root.join(".loom-home");
    let build_dir = loom_home
        .join("knowledge/sources")
        .join(source_id)
        .join("build-runs")
        .join(build_id);
    std::fs::create_dir_all(build_dir.join("chunks")).expect("typed focus chunks dir");
    std::fs::write(
        loom_home.join("knowledge/registry.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1,
            "sources": [{
                "sourceId": source_id,
                "name": "typed-focus-rules",
                "enabled": true,
                "documentPaths": ["/fixture/typed-focus.md"],
                "currentBuildId": build_id,
                "createdAt": "2026-06-30T00:00:00Z",
                "updatedAt": "2026-06-30T00:00:00Z",
                "lastBuiltAt": "2026-06-30T00:00:00Z"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
    std::fs::write(
        build_dir.join("chunks.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1,
            "sourceId": source_id,
            "sourceName": "typed-focus-rules",
            "buildId": build_id,
            "chunks": [
                semantic_chunk(
                    "kchunk_operation_only",
                    "operation wording",
                    "证券账户持仓清空后方可销户只是一个操作短语。",
                    json!([
                        {"kind": "operation", "text": "证券账户持仓清空后方可销户", "normalizedText": "证券账户持仓清空后方可销户", "aliases": [], "confidence": "high"}
                    ]),
                    json!([]),
                    1.0,
                ),
                semantic_chunk(
                    "kchunk_object_rule",
                    "object rule",
                    "证券账户对象及持仓清空后方可销户规则。",
                    json!([
                        {"kind": "object", "text": "证券账户", "normalizedText": "证券账户", "aliases": [], "confidence": "high"},
                        {"kind": "rule", "text": "持仓清空后方可销户", "normalizedText": "持仓清空后方可销户", "aliases": [], "confidence": "high"}
                    ]),
                    json!([]),
                    0.0,
                )
            ]
        }))
        .expect("chunks json"),
    )
    .expect("write chunks");
    std::fs::write(
        build_dir.join("chunks/kchunk_operation_only.txt"),
        "证券账户持仓清空后方可销户只是一个操作短语。",
    )
    .expect("write operation-only chunk");
    std::fs::write(
        build_dir.join("chunks/kchunk_object_rule.txt"),
        "证券账户对象及持仓清空后方可销户规则。",
    )
    .expect("write object-rule chunk");

    let search = search_knowledge(KnowledgeSearchInput {
        project_root: fixture.root_str().to_string(),
        natural_language_query: "".to_string(),
        semantic_focus: vec![
            "object:证券账户".to_string(),
            "rule:持仓清空后方可销户".to_string(),
        ],
        source_names: vec![],
        block: Some("concept_grounding".to_string()),
        limit: Some(2),
    })
    .expect("typed focus search");

    assert_eq!(search.status, "available");
    assert_eq!(search.cards[0].chunk_id, "kchunk_object_rule");
    let matched_kinds = search.cards[0]
        .matched_labels
        .iter()
        .map(|label| label.kind.as_str())
        .collect::<Vec<_>>();
    assert!(matched_kinds.contains(&"object"));
    assert!(matched_kinds.contains(&"rule"));
}

#[test]
fn brainstorm_context_is_request_scoped_and_uses_inspect_read_plan() {
    let fixture = Fixture::new("brainstorm-context");
    fixture.write_file(
        "rules.md",
        "# 页面办理路径\n\n证券账户管理页面支持列表查询、新建开户、挂失补办和销户办理，销户时展示持仓未清空阻断原因。",
    );
    publish_simple_source(&fixture, "page-paths");

    let stored = state::write_native_request(
        fixture.root_str(),
        state::NativeRequestInput {
            request_id: "brainstorm_session_req_1".to_string(),
            request_kind: "brainstorm_session".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase-1".to_string()),
            root: json!({
                "knowledgeQueryPlan": {
                    "blocks": {
                        "phase_scope": {
                            "executionOrder": [{
                                "stepId": "phase_scope_closure",
                                "queryKind": "capability_closure",
                                "repeatMode": "per_candidate_phase_cut"
                            }]
                        },
                        "concept_grounding": {
                            "executionOrder": [{
                                "stepId": "concept_scope_items",
                                "queryKind": "scope_item_grounding"
                            }]
                        },
                        "frontend_experience": {
                            "executionOrder": [{
                                "stepId": "frontend_paths",
                                "queryKind": "page_operation_path"
                            }]
                        }
                    }
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "knowledge_context_protocol",
                        "fields": ["knowledgeQueryPlan"]
                    }]
                }
            }),
        },
    )
    .expect("write brainstorm request");

    let context = brainstorm_context(KnowledgeBrainstormContextInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        block: "frontend_experience".to_string(),
        step_id: "frontend_paths".to_string(),
        query_id: None,
        atomic_scope_reason: None,
        query_subject: "证券账户管理页面".to_string(),
        natural_language_query: "开户 挂失补办 销户 页面办理路径".to_string(),
        semantic_focus: vec![
            "证券账户".to_string(),
            "销户".to_string(),
            "页面办理路径".to_string(),
        ],
    })
    .expect("brainstorm context");
    assert_eq!(context.status, "available");
    assert_eq!(context.read_plan.mode, "inspect_all_listed_chunks");
    assert!(!context.read_plan.chunks.is_empty());
    assert!(context
        .read_plan
        .chunks
        .iter()
        .all(|chunk| chunk.source_name == "page-paths"));
    assert!(context
        .read_plan
        .chunks
        .iter()
        .all(|chunk| !chunk.build_id.is_empty() && !chunk.chunk_id.is_empty()));
    let context_json = serde_json::to_value(&context).expect("context json");
    assert!(context_json["matchedSources"].is_array());
    assert!(!context_json.to_string().contains("topChunks"));
    assert!(!context_json.to_string().contains("sourceId"));
    let query_file = fixture.root.join(
        ".loom/deliveries/delivery_1/workspace/phase-1/brainstorm-knowledge/brainstorm_session_req_1/frontend_experience/frontend_paths/query.json",
    );
    let result_file = fixture.root.join(
        ".loom/deliveries/delivery_1/workspace/phase-1/brainstorm-knowledge/brainstorm_session_req_1/frontend_experience/frontend_paths/result.json",
    );
    assert!(query_file.exists());
    assert!(result_file.exists());
    let persisted_query: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&query_file).expect("read query"))
            .expect("parse query");
    assert_eq!(persisted_query["block"], "frontend_experience");
    assert_eq!(persisted_query["stepId"], "frontend_paths");
    let persisted_result: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&result_file).expect("read result"))
            .expect("parse result");
    assert_eq!(persisted_result["status"], "available");
    assert!(persisted_result.get("requestRef").is_none());
    assert!(persisted_result.get("stepId").is_none());
    assert!(persisted_result.get("querySubject").is_none());
    assert!(persisted_result.get("naturalLanguageQuery").is_none());
    assert!(persisted_result.get("semanticFocus").is_none());

    let missing_query_id = brainstorm_context(KnowledgeBrainstormContextInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        block: "phase_scope".to_string(),
        step_id: "phase_scope_closure".to_string(),
        query_id: None,
        atomic_scope_reason: None,
        query_subject: "方案A：证券账户模块闭环".to_string(),
        natural_language_query: "证券账户 开户 挂失补办 销户".to_string(),
        semantic_focus: vec!["证券账户".to_string(), "开户".to_string()],
    })
    .expect_err("repeat knowledge step requires queryId");
    assert!(missing_query_id.to_string().contains("queryId is required"));

    for query_id in ["capability_closure_A", "capability_closure_B"] {
        brainstorm_context(KnowledgeBrainstormContextInput {
            project_root: fixture.root_str().to_string(),
            request_ref: stored.request_ref.clone(),
            block: "phase_scope".to_string(),
            step_id: "phase_scope_closure".to_string(),
            query_id: Some(query_id.to_string()),
            atomic_scope_reason: None,
            query_subject: format!("{query_id}：证券账户模块边界"),
            natural_language_query: "证券账户 开户 挂失补办 销户".to_string(),
            semantic_focus: vec!["证券账户".to_string(), "开户".to_string()],
        })
        .expect("candidate closure context");
        let query_file = fixture.root.join(format!(
            ".loom/deliveries/delivery_1/workspace/phase-1/brainstorm-knowledge/brainstorm_session_req_1/phase_scope/phase_scope_closure/{query_id}/query.json"
        ));
        let result_file = fixture.root.join(format!(
            ".loom/deliveries/delivery_1/workspace/phase-1/brainstorm-knowledge/brainstorm_session_req_1/phase_scope/phase_scope_closure/{query_id}/result.json"
        ));
        assert!(query_file.exists(), "missing query file for {query_id}");
        assert!(result_file.exists(), "missing result file for {query_id}");
        let persisted_query: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&query_file).expect("read query"))
                .expect("parse query");
        assert_eq!(persisted_query["queryId"], query_id);
    }

    let wrong_step = brainstorm_context(KnowledgeBrainstormContextInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref,
        block: "frontend_experience".to_string(),
        step_id: "phase_scope_closure".to_string(),
        query_id: None,
        atomic_scope_reason: None,
        query_subject: "证券账户管理页面".to_string(),
        natural_language_query: "页面办理路径".to_string(),
        semantic_focus: vec![],
    })
    .expect_err("wrong step must fail");
    assert!(wrong_step.to_string().contains("does not belong"));
}

#[test]
fn brainstorm_context_adds_block_retrieval_intent_without_agent_facing_bloat() {
    let fixture = Fixture::new("block-retrieval-intent");
    let source_id = "ksrc_block_intent";
    let build_id = "kbld_block_intent";
    let loom_home = fixture.root.join(".loom-home");
    let build_dir = loom_home
        .join("knowledge/sources")
        .join(source_id)
        .join("build-runs")
        .join(build_id);
    std::fs::create_dir_all(build_dir.join("chunks")).expect("intent chunks dir");
    std::fs::write(
        loom_home.join("knowledge/registry.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1,
            "sources": [{
                "sourceId": source_id,
                "name": "block-intent-rules",
                "enabled": true,
                "documentPaths": ["/fixture/block-intent.md"],
                "currentBuildId": build_id,
                "createdAt": "2026-06-26T00:00:00Z",
                "updatedAt": "2026-06-26T00:00:00Z",
                "lastBuiltAt": "2026-06-26T00:00:00Z"
            }]
        }))
        .expect("registry json"),
    )
    .expect("write registry");
    std::fs::write(
        build_dir.join("chunks.json"),
        serde_json::to_string_pretty(&json!({
            "schemaVersion": 1,
            "sourceId": source_id,
            "sourceName": "block-intent-rules",
            "buildId": build_id,
            "chunks": [
                {
                    "chunkId": "kchunk_000001",
                    "documentId": "kdoc_000001",
                    "documentTitle": "irrelevant frontend-affinity note",
                    "sourcePath": "/fixture/block-intent.md",
                    "headingPath": ["fixture", "runtime"],
                    "tokenEstimate": 80,
                    "contextPrefix": "运行容器端口健康检查。",
                    "neighborChunkIds": [],
                    "splitReason": "section",
                    "bodyRef": "chunks/kchunk_000001.txt",
                    "summary": "运行容器端口健康检查。",
                    "semanticLabels": [],
                    "semanticAliases": [],
                    "blockAffinity": {
                        "phaseScope": 0.0,
                        "conceptGrounding": 0.0,
                        "frontendExperience": 1.0
                    }
                },
                {
                    "chunkId": "kchunk_000002",
                    "documentId": "kdoc_000001",
                    "documentTitle": "frontend operation path",
                    "sourcePath": "/fixture/block-intent.md",
                    "headingPath": ["fixture", "frontend"],
                    "tokenEstimate": 80,
                    "contextPrefix": "页面操作路径包含查询、筛选、分页、列表、详情、操作入口、表单输入、成功反馈、业务阻断、刷新回读。",
                    "neighborChunkIds": [],
                    "splitReason": "section",
                    "bodyRef": "chunks/kchunk_000002.txt",
                    "summary": "页面操作路径包含查询、筛选、分页、列表、详情、操作入口、表单输入、成功反馈、业务阻断、刷新回读。",
                    "semanticLabels": [],
                    "semanticAliases": [],
                    "blockAffinity": {
                        "phaseScope": 0.0,
                        "conceptGrounding": 0.0,
                        "frontendExperience": 1.0
                    }
                }
            ]
        }))
        .expect("chunks json"),
    )
    .expect("write chunks");
    std::fs::write(
        build_dir.join("chunks/kchunk_000001.txt"),
        "运行 容器 端口 健康检查。",
    )
    .expect("write irrelevant chunk");
    std::fs::write(
        build_dir.join("chunks/kchunk_000002.txt"),
        "页面 操作 路径 查询 筛选 分页 列表 详情 操作 入口 表单 输入 成功 反馈 失败 提示 业务 阻断 加载 空状态 刷新 回读。",
    )
    .expect("write frontend chunk");

    let stored = state::write_native_request(
        fixture.root_str(),
        state::NativeRequestInput {
            request_id: "brainstorm_session_req_intent".to_string(),
            request_kind: "brainstorm_session".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase-1".to_string()),
            root: json!({
                "knowledgeQueryPlan": {
                    "blocks": {
                        "frontend_experience": {
                            "executionOrder": [{
                                "stepId": "frontend_paths",
                                "queryKind": "page_operation_path"
                            }]
                        }
                    }
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "knowledge_context_protocol",
                        "fields": ["knowledgeQueryPlan.blocks.frontend_experience.executionOrder"]
                    }]
                }
            }),
        },
    )
    .expect("write brainstorm request");

    let context = brainstorm_context(KnowledgeBrainstormContextInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref,
        block: "frontend_experience".to_string(),
        step_id: "frontend_paths".to_string(),
        query_id: None,
        atomic_scope_reason: None,
        query_subject: "当前办理体验".to_string(),
        natural_language_query: "".to_string(),
        semantic_focus: vec![],
    })
    .expect("frontend context");

    assert_eq!(context.status, "available");
    assert_eq!(context.read_plan.chunks[0].chunk_id, "kchunk_000002");
    let context_json = serde_json::to_value(&context).expect("context json");
    assert!(context_json.get("requestRef").is_none());
    assert!(context_json.get("querySubject").is_none());
    assert!(context_json.get("naturalLanguageQuery").is_none());
    assert!(context_json.get("semanticFocus").is_none());
}

fn focus_chunk(
    chunk_id: &str,
    semantic_label: &str,
    summary: &str,
    phase_scope: f64,
) -> serde_json::Value {
    semantic_chunk(
        chunk_id,
        semantic_label,
        summary,
        json!([
            {"kind": "operation", "text": semantic_label, "normalizedText": semantic_label, "aliases": [], "confidence": "high"}
        ]),
        json!([]),
        phase_scope,
    )
}

fn semantic_chunk(
    chunk_id: &str,
    heading: &str,
    summary: &str,
    semantic_labels: serde_json::Value,
    semantic_aliases: serde_json::Value,
    phase_scope: f64,
) -> serde_json::Value {
    json!({
        "chunkId": chunk_id,
        "documentId": "kdoc_000001",
        "documentTitle": "focus fixture",
        "sourcePath": "/fixture/focus.md",
        "headingPath": ["fixture", heading],
        "tokenEstimate": 80,
        "contextPrefix": summary,
        "neighborChunkIds": [],
        "splitReason": "section",
        "bodyRef": format!("chunks/{chunk_id}.txt"),
        "summary": summary,
        "semanticLabels": semantic_labels,
        "semanticAliases": semantic_aliases,
        "blockAffinity": {
            "phaseScope": phase_scope,
            "conceptGrounding": phase_scope,
            "frontendExperience": 0.0
        }
    })
}

fn publish_simple_source(fixture: &Fixture, name: &str) {
    let path = fixture.root.join("rules.md").to_string_lossy().into_owned();
    add_source(KnowledgeAddInput {
        project_root: fixture.root_str().to_string(),
        name: name.to_string(),
        paths: vec![path],
    })
    .expect("add simple source");
    let next = build_source(fixture.root_str(), name).expect("build simple source");
    let semantic = match next {
        LoomMcpActionResult::AutoRunnable(result) => match result.next {
            LoomMcpNextAction::GenerateKnowledgeSemantics(semantic) => semantic,
            other => panic!("expected semantic next action, got {other:?}"),
        },
        other => panic!("expected auto_runnable build result, got {other:?}"),
    };
    let result = json!({
        "schemaVersion": 1,
        "buildId": semantic.build_id,
        "packId": semantic.pack_id,
        "chunkResults": semantic.chunk_read_plan.iter().map(|chunk| {
            json!({
                "chunkId": chunk.chunk_id,
                "status": "completed",
                "summary": "证券账户管理页面的列表查询、开户、挂失补办和销户办理路径。",
                "semanticLabels": [
                    {"kind": "object", "text": "证券账户管理页面", "normalizedText": "证券账户管理页面", "aliases": ["证券账户页面"], "confidence": "high"},
                    {"kind": "page", "text": "证券账户销户办理路径", "normalizedText": "证券账户销户办理路径", "aliases": ["页面办理路径", "销户办理"], "confidence": "high"},
                    {"kind": "operation", "text": "销户", "normalizedText": "销户", "aliases": ["证券账户销户"], "confidence": "high"}
                ],
                "blockAffinity": {
                    "phaseScope": 0.4,
                    "conceptGrounding": 0.6,
                    "frontendExperience": 1.0
                }
            })
        }).collect::<Vec<_>>()
    });
    std::fs::write(
        fixture.root.join(&semantic.result_file),
        serde_json::to_string_pretty(&result).expect("result json"),
    )
    .expect("write simple semantic result");
    assert!(matches!(
        submit_semantic_pack(fixture.root_str(), &semantic.request_ref).expect("submit semantic"),
        LoomMcpActionResult::Done(_)
    ));
}

struct Fixture {
    root: std::path::PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "loom-mcp-knowledge-{name}-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");
        std::env::set_var("LOOM_HOME", root.join(".loom-home"));
        Self {
            root,
            _guard: guard,
        }
    }

    fn root_str(&self) -> &str {
        self.root.to_str().expect("fixture path utf8")
    }

    fn write_file(&self, relative: &str, text: &str) -> std::path::PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, text).expect("write fixture file");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
