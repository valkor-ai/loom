use std::sync::{Mutex, MutexGuard};

use delivery_core::{LoomMcpActionResult, LoomMcpNextAction};
use knowledge::{
    add_source, brainstorm_context, build_source, disable_source, inspect_chunk, list_sources,
    mcp_models::{
        KnowledgeAddInput, KnowledgeBrainstormContextInput, KnowledgeInspectChunkInput,
        KnowledgeNameInput, KnowledgeProjectInput, KnowledgeSearchInput,
    },
    search_knowledge, submit_semantic_pack,
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

    let first_chunk = semantic.chunk_read_plan.first().expect("first chunk");
    let inspected = inspect_chunk(KnowledgeInspectChunkInput {
        project_root: fixture.root_str().to_string(),
        source_name: "stock-rules".to_string(),
        source_id: Some(first_chunk.source_id.clone()),
        build_id: semantic.build_id.clone(),
        chunk_id: first_chunk.chunk_id.clone(),
    })
    .expect("inspect chunk");
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
                "status": "ok",
                "summary": "",
                "semanticLabels": [],
                "semanticAliases": [],
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
                "status": "ok",
                "summary": "证券账户开户、挂失补办、销户和持仓清空限制。",
                "semanticLabels": [
                    {"kind": "object", "text": "证券账户", "confidence": "high"},
                    {"kind": "operation", "text": "证券账户开户", "confidence": "high"},
                    {"kind": "operation", "text": "销户", "confidence": "high"},
                    {"kind": "rule", "text": "持仓清空后方可销户", "confidence": "high"}
                ],
                "semanticAliases": ["开户", "证券账户销户", "恢复交易条件"],
                "blockAffinity": {
                    "phaseScope": 0.7,
                    "conceptGrounding": 1.0,
                    "frontendExperience": 0.4,
                    "businessRules": 0.9
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
    assert!(lexical_text.contains("恢复交易条件"));
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
    assert!(!serde_json::to_value(&search.cards[0])
        .expect("card json")
        .as_object()
        .expect("card object")
        .contains_key("text"));
    assert_eq!(search.cards[0].inspect.project_root, fixture.root_str());

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

    let inspected = inspect_chunk(search.cards[0].inspect.clone()).expect("legacy inspect");
    assert!(inspected.text.contains("销户前必须清空"));
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
                                "queryKind": "capability_closure"
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
    assert_eq!(context.block, "frontend_experience");
    assert_eq!(context.read_plan.mode, "inspect_all_listed_chunks");
    assert!(!context.read_plan.chunks.is_empty());
    assert!(context
        .read_plan
        .chunks
        .iter()
        .all(|chunk| chunk.inspect.source_name == "page-paths"));
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
    assert_eq!(persisted_result["requestRef"], context.request_ref);
    assert_eq!(persisted_result["status"], "available");

    let wrong_step = brainstorm_context(KnowledgeBrainstormContextInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref,
        block: "frontend_experience".to_string(),
        step_id: "phase_scope_closure".to_string(),
        query_subject: "证券账户管理页面".to_string(),
        natural_language_query: "页面办理路径".to_string(),
        semantic_focus: vec![],
    })
    .expect_err("wrong step must fail");
    assert!(wrong_step.to_string().contains("does not belong"));
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
                "status": "ok",
                "summary": "证券账户管理页面的列表查询、开户、挂失补办和销户办理路径。",
                "semanticLabels": [
                    {"kind": "object", "text": "证券账户管理页面", "confidence": "high"},
                    {"kind": "page_operation", "text": "证券账户销户办理路径", "confidence": "high"},
                    {"kind": "operation", "text": "销户", "confidence": "high"}
                ],
                "semanticAliases": ["页面办理路径", "销户办理", "证券账户"],
                "blockAffinity": {
                    "phaseScope": 0.4,
                    "conceptGrounding": 0.6,
                    "frontendExperience": 1.0,
                    "businessRules": 0.7
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
