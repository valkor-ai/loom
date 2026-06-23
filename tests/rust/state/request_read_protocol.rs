use serde_json::json;
use state::{
    legacy_ts_reader::register_legacy_ts_request,
    request_resolver::{read_field_by_resource_uri, read_field_group_by_resource_uri},
    store::{read_json_value, write_json_atomic, write_text_atomic},
    write_native_request, NativeRequestInput,
};
use std::fs::read_to_string;
use std::sync::{Mutex, MutexGuard};

#[test]
fn native_request_read_protocol_resolves_declared_fields() {
    let fixture = Fixture::new("native");
    let normalized_ref = ".loom/contexts/normalized.txt";
    let keyword_hints_ref = ".loom/contexts/keyword-hints.json";
    write_text_atomic(&fixture.root.join(normalized_ref), "证券账户开户需求")
        .expect("write normalized text");
    write_json_atomic(
        &fixture.root.join(keyword_hints_ref),
        &json!({
            "status": "completed",
            "languageHints": ["zh-CN", "zh-Hans", "finance"],
            "globalKeywords": [
                {
                    "keyword": "证券账户",
                    "occurrences": 5,
                    "sourceItemIds": ["input_1", "input_2", "input_3", "input_4"]
                }
            ],
            "sectionKeywords": [
                {
                    "sectionId": "account_rules",
                    "sourceItemId": "input_1",
                    "title": "账户规则",
                    "keywords": [
                        { "keyword": "开户" },
                        { "keyword": "销户" }
                    ]
                }
            ]
        }),
    )
    .expect("write keyword hints");

    let stored = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_native_1".to_string(),
            request_kind: "task_execution".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase_1".to_string()),
            root: json!({
                "agentAction": {
                    "actionKind": "execute_task",
                    "read": { "fieldGroups": [{ "groupId": "old", "fields": ["task"] }] }
                },
                "contextRefs": {
                    "normalizedRequirementTextRef": normalized_ref,
                    "keywordHintsRef": keyword_hints_ref
                },
                "task": {
                    "title": "实现证券账户开户",
                    "items": [{ "name": "开户" }]
                },
                "rules": {
                    "requirementSemanticGrounding": {
                        "rules": [
                            "rule_1",
                            "rule_2",
                            "rule_3",
                            "rule_4",
                            "rule_5",
                            "rule_6",
                            "rule_7",
                            "rule_8"
                        ]
                    }
                },
                "outputContract": {
                    "schemaShape": { "summary": "string" }
                },
                "writeTargets": [{ "targetId": "result", "path": ".loom/result.json" }],
                "submitTool": "loom.recordTaskResultFile",
                "requestReadPlan": {
                    "groups": [
                        {
                            "groupId": "core",
                            "required": true,
                            "purpose": "Read core fields.",
                            "whenToRead": "Before execution.",
                            "fields": [
                                "task.title",
                                "task.items.0.name",
                                "outputContract.schemaShape",
                                "requirementContext.normalizedText",
                                "keywordHints.compact",
                                "rules.requirementSemanticGrounding.compactRules"
                            ]
                        }
                    ]
                }
            }),
        },
    )
    .expect("write request");
    assert!(stored.request_ref.starts_with("loom://projects/"));
    assert!(!stored.request_ref.contains(".loom/"));

    let request_file = fixture.root.join(&stored.request_file);
    let compact_root = read_json_value(&request_file).expect("read compact request");
    assert!(compact_root.get("agentAction").is_none());
    assert!(compact_root.get("agentActionRef").is_none());
    assert!(compact_root.get("taskRef").is_none());
    assert!(compact_root.get("requestReadPlan").is_some());
    assert_eq!(
        compact_root["requestManifest"]["refs"]["agentAction"]["ref"],
        json!(".loom/requests/req_native_1.refs/agent-action.json")
    );
    assert_eq!(
        compact_root["requestManifest"]["refs"]["task"]["ref"],
        json!(".loom/requests/req_native_1.refs/task.json")
    );

    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
    })
    .expect("inspect request");
    assert_eq!(inspected.request_id, "req_native_1");
    assert_eq!(inspected.read_groups[0].group_id, "core");
    assert_eq!(
        inspected.submit_tool.as_deref(),
        Some("loom.recordTaskResultFile")
    );
    assert_eq!(inspected.write_targets.len(), 1);

    let group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        group_id: "core".to_string(),
    })
    .expect("read group");
    assert_eq!(group.fields["task.title"].value, "实现证券账户开户");
    assert_eq!(group.fields["task.items.0.name"].value, "开户");
    assert_eq!(
        group.fields["outputContract.schemaShape"].value,
        json!({ "summary": "string" })
    );
    assert_eq!(
        group.fields["requirementContext.normalizedText"].value,
        "证券账户开户需求"
    );
    assert_eq!(
        group.fields["keywordHints.compact"].value["status"],
        "completed"
    );
    assert_eq!(
        group.fields["keywordHints.compact"].value["topKeywords"][0]["keyword"],
        "证券账户"
    );
    assert_eq!(
        group.fields["rules.requirementSemanticGrounding.compactRules"].value,
        json!(["rule_1", "rule_2", "rule_3", "rule_4", "rule_5", "rule_6", "rule_7"])
    );

    let selected = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        fields: vec!["task.title".to_string(), "task.title".to_string()],
    })
    .expect("read selected fields");
    assert_eq!(selected.fields.len(), 1);

    let denied = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        fields: vec!["requestManifest.refs".to_string()],
    })
    .expect_err("forbidden field should fail");
    assert!(denied.to_string().contains("FIELD_NOT_ALLOWED"));

    let by_resource = read_field_group_by_resource_uri(&stored.read_groups[0].resource_uri)
        .expect("resource field group read");
    assert_eq!(by_resource.fields["task.title"].value, "实现证券账户开户");

    let field_uri = format!(
        "loom://projects/{}/requests/{}/fields/task.title",
        stored.project_id, stored.request_id
    );
    let by_field_resource = read_field_by_resource_uri(&field_uri).expect("resource field read");
    assert_eq!(
        by_field_resource.fields["task.title"].value,
        "实现证券账户开户"
    );

    let paths = state::paths::project_paths(fixture.root_str()).expect("project paths");
    let size_audit = read_to_string(paths.request_size_audit_file).expect("request size audit");
    assert!(size_audit.contains("\"requestRef\""));
    assert!(size_audit.contains("req_native_1"));

    let field_audit = read_to_string(paths.field_read_audit_file).expect("field read audit");
    assert!(field_audit.contains("\"source\":\"readFieldGroup\""));
    assert!(field_audit.contains("\"source\":\"readRequestFields\""));
}

#[test]
fn legacy_ts_request_is_converted_without_rewriting_file() {
    let fixture = Fixture::new("legacy");
    let legacy_file = fixture.root.join(".loom/legacy/request.json");
    write_json_atomic(
        &legacy_file,
        &json!({
            "requestKind": "legacy_request",
            "agentAction": {
                "read": {
                    "fieldGroups": [{
                        "groupId": "legacy_core",
                        "required": true,
                        "fields": ["task.title"],
                        "readCommand": { "argv": ["inspect"] },
                        "fallbackRule": "old fallback"
                    }]
                }
            },
            "task": { "title": "旧请求标题" }
        }),
    )
    .expect("write legacy request");

    let request_ref = register_legacy_ts_request(fixture.root_str(), ".loom/legacy/request.json")
        .expect("register legacy request");
    let legacy_after = read_json_value(&legacy_file).expect("read legacy request");
    assert!(legacy_after.get("requestReadPlan").is_none());

    let inspected = state::inspect_request(delivery_core::InspectRequestInput {
        project_root: fixture.root_str().to_string(),
        request_ref: request_ref.clone(),
    })
    .expect("inspect legacy");
    assert_eq!(inspected.read_groups[0].group_id, "legacy_core");
    assert!(!serde_json::to_string(&inspected)
        .unwrap()
        .contains("readCommand"));

    let group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref,
        group_id: "legacy_core".to_string(),
    })
    .expect("read legacy group");
    assert_eq!(group.fields["task.title"].value, "旧请求标题");
}

struct Fixture {
    root: std::path::PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let guard = ENV_LOCK.lock().expect("env lock");
        let root = std::env::temp_dir().join(format!(
            "loom-mcp-state-{name}-{}-{}",
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
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
