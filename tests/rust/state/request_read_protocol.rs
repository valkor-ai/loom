use serde_json::json;
use state::{
    legacy_ts_reader::register_legacy_ts_request,
    request_resolver::{read_field_by_resource_uri, read_field_group_by_resource_uri},
    store::{read_json_value, write_json_atomic, write_text_atomic},
    write_native_request, NativeRequestInput, WriteTargetAuthorizationError,
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
                "contextRefs": {
                    "requestTextRef": normalized_ref,
                    "normalizedRequirementTextRef": normalized_ref,
                    "requirementContextRef": ".loom/contexts/unused-context.json",
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
                                "outputContract.schemaShape.summary",
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
    assert!(compact_root.get("requestManifest").is_none());
    assert!(compact_root.get("outputContract").is_none());
    assert!(compact_root.get("requestReadPlan").is_some());
    let context_refs = compact_root["contextRefs"]
        .as_object()
        .expect("context refs");
    assert_eq!(context_refs.len(), 2);
    assert!(context_refs.contains_key("keywordHintsRef"));
    assert!(context_refs.contains_key("normalizedRequirementTextRef"));
    let root_text = serde_json::to_string(&compact_root).expect("serialize compact root");
    assert!(!root_text.contains(".refs"));
    assert!(!root_text.contains("readCommand"));
    assert!(!root_text.contains("fallbackRule"));

    let storage_manifest = read_json_value(
        &fixture
            .root
            .join(".loom/requests/req_native_1.manifest.json"),
    )
    .expect("read private storage manifest");
    assert_eq!(
        storage_manifest["protocolAuthority"],
        json!("rust_private_request_storage_manifest")
    );
    let refs = storage_manifest["refs"].as_object().expect("manifest refs");
    assert_eq!(refs.len(), 3);
    assert!(refs.contains_key("task"));
    assert!(refs.contains_key("rules"));
    assert!(refs.contains_key("outputContract"));
    assert!(!refs.contains_key("agentAction"));
    assert_eq!(
        storage_manifest["refs"]["task"]["ref"],
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
    let group_json = serde_json::to_value(&group).expect("serialize read group");
    assert_eq!(
        group_json["fields"]["task.title"],
        json!("实现证券账户开户")
    );
    let group_text = group_json.to_string();
    assert!(!group_text.contains("sourceRef"));
    assert!(!group_text.contains("sourceKind"));
    assert!(!group_text.contains("selector"));
    assert!(!group_text.contains("\"status\":\"resolved\""));
    assert_eq!(group.fields["task.title"].value, "实现证券账户开户");
    assert_eq!(group.fields["task.items.0.name"].value, "开户");
    assert_eq!(
        group.fields["outputContract.schemaShape.summary"].value,
        "string"
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
        group.fields["keywordHints.compact"].value["topKeywords"][0],
        "证券账户"
    );
    assert_eq!(
        group.fields["keywordHints.compact"].value["sectionKeywords"][0]["keywords"],
        json!(["开户", "销户"])
    );
    assert!(
        !group.fields["keywordHints.compact"]
            .value
            .to_string()
            .contains("\"keyword\""),
        "compact keyword hints must expose keyword arrays as strings"
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
fn native_request_rejects_legacy_agent_action_authority() {
    let fixture = Fixture::new("native-forbidden-agent-action");
    let error = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_forbidden_agent_action".to_string(),
            request_kind: "task_execution".to_string(),
            request_file: None,
            delivery_id: None,
            phase_id: None,
            root: json!({
                "agentAction": { "read": { "fieldGroups": [] } },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "core",
                        "fields": ["task.title"]
                    }]
                },
                "task": { "title": "x" }
            }),
        },
    )
    .expect_err("native MCP writer rejects agentAction");
    assert!(error.to_string().contains("agentAction"));
}

#[test]
fn native_request_rejects_unresolvable_or_broad_read_plan_fields() {
    let fixture = Fixture::new("native-read-plan-validation");
    let missing = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_missing_field".to_string(),
            request_kind: "task_execution".to_string(),
            request_file: None,
            delivery_id: None,
            phase_id: None,
            root: json!({
                "task": { "title": "x" },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "core",
                        "fields": ["task.missing"]
                    }]
                }
            }),
        },
    )
    .expect_err("missing read field is rejected before request is returned");
    assert!(missing.to_string().contains("task.missing"));

    let broad = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_broad_field".to_string(),
            request_kind: "task_execution".to_string(),
            request_file: None,
            delivery_id: None,
            phase_id: None,
            root: json!({
                "rules": { "long": true },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "core",
                        "fields": ["rules"]
                    }]
                }
            }),
        },
    )
    .expect_err("broad read field is rejected");
    assert!(broad.to_string().contains("too broad"));
}

#[test]
fn native_request_protocol_snapshot_covers_delivery_request_kinds() {
    let fixture = Fixture::new("native-kind-snapshots");
    let cases = [
        (
            "brainstorm",
            Some("brainstorm_candidate"),
            "loom.brainstormAcceptFile",
        ),
        (
            "knowledge_semantic",
            None,
            "loom.knowledgeSemanticSubmitFile",
        ),
        (
            "technical_baseline",
            Some("technical_baseline_candidate"),
            "loom.technicalBaselineAcceptFile",
        ),
        (
            "repository_context",
            Some("repository_context_candidate"),
            "loom.repositoryContextAcceptFile",
        ),
        (
            "aac_section",
            Some("architecture_section_candidate"),
            "loom.architectureSectionSubmitFile",
        ),
        (
            "task_plan",
            Some("task_plan_candidate"),
            "loom.taskPlanAcceptFile",
        ),
        (
            "task_execution",
            Some("task_result"),
            "loom.recordTaskResultFile",
        ),
        ("review", Some("review_result"), "loom.reviewAcceptFile"),
        (
            "manual_review",
            Some("manual_review_resolution"),
            "loom.reviewResolveFile",
        ),
        (
            "task_repair",
            Some("task_result_repair"),
            "loom.repairSubmitFile",
        ),
        (
            "deploy_repair",
            Some("deploy_execution_repair_result"),
            "loom.repairSubmitFile",
        ),
    ];

    for (index, (request_kind, artifact_kind, submit_tool)) in cases.iter().enumerate() {
        let request_id = format!("req_snapshot_{index}");
        let target_path = format!(".loom/agent-writable/{request_kind}.json");
        let mut root = json!({
            "protocolPurpose": request_kind,
            "submitTool": submit_tool,
            "writeTargets": [{
                "targetId": "result",
                "path": target_path,
                "required": true,
                "description": "Agent-writable result JSON."
            }],
            "requestReadPlan": {
                "groups": [{
                    "groupId": "write_contract",
                    "required": true,
                    "purpose": "Read the native MCP write contract.",
                    "whenToRead": "Before writing the result file.",
                    "fields": ["protocolPurpose", "submitTool", "writeTargets"]
                }]
            }
        });
        if let Some(artifact_kind) = artifact_kind {
            root.as_object_mut()
                .expect("root object")
                .insert("artifactKind".to_string(), json!(artifact_kind));
        }

        let stored = write_native_request(
            fixture.root_str(),
            NativeRequestInput {
                request_id: request_id.clone(),
                request_kind: request_kind.to_string(),
                request_file: None,
                delivery_id: Some("delivery_1".to_string()),
                phase_id: Some("phase_1".to_string()),
                root,
            },
        )
        .unwrap_or_else(|error| panic!("{request_kind} request should be valid: {error}"));

        let request_file = fixture.root.join(&stored.request_file);
        let compact_root = read_json_value(&request_file).expect("read compact request");
        let root_text = serde_json::to_string(&compact_root).expect("serialize compact root");
        assert!(
            compact_root.get("requestManifest").is_none(),
            "{request_kind}"
        );
        assert!(compact_root.get("agentAction").is_none(), "{request_kind}");
        assert!(
            compact_root.get("submitCommand").is_none(),
            "{request_kind}"
        );
        assert!(compact_root.get("contextRefs").is_none(), "{request_kind}");
        assert!(!root_text.contains(".refs"), "{request_kind}");
        assert!(!root_text.contains("readCommand"), "{request_kind}");
        assert!(!root_text.contains("fallbackRule"), "{request_kind}");

        let storage_manifest = read_json_value(
            &fixture
                .root
                .join(format!(".loom/requests/{request_id}.manifest.json")),
        )
        .expect("read private storage manifest");
        assert_eq!(
            storage_manifest["refs"]
                .as_object()
                .expect("manifest refs")
                .len(),
            0,
            "{request_kind} should not write unused backing refs"
        );

        let group = state::read_field_group(delivery_core::ReadFieldGroupInput {
            project_root: fixture.root_str().to_string(),
            request_ref: stored.request_ref,
            group_id: "write_contract".to_string(),
        })
        .expect("read snapshot write contract");
        assert_eq!(group.fields["protocolPurpose"].value, json!(request_kind));
        assert_eq!(group.fields["submitTool"].value, json!(submit_tool));
        assert!(group.fields["writeTargets"].value.is_array());
    }
}

#[test]
fn legacy_ts_problem_fixture_reports_repeated_authorities() {
    let legacy = json!({
        "contextRefs": {
            "requestTextRef": ".loom/requirements/normalized.txt",
            "normalizedRequirementTextRef": ".loom/requirements/normalized.txt"
        },
        "agentAction": {
            "read": {
                "fieldGroups": [{
                    "groupId": "legacy",
                    "fields": ["rules", "outputContract.schemaShape"],
                    "readCommand": { "argv": ["loom-cli", "inspect"] },
                    "fallbackRule": "Read requestManifest refs."
                }]
            },
            "submit": { "command": { "argv": ["loom-cli", "accept"] } }
        },
        "requestReadPlan": {
            "groups": [{
                "groupId": "root",
                "fields": ["rules", "outputContract.schemaShape"]
            }]
        },
        "requestManifest": {
            "refs": {
                "agentAction": { "ref": ".loom/requests/old.refs/agent-action.json" },
                "rules": { "ref": ".loom/requests/old.refs/rules.json" }
            }
        },
        "submitCommand": { "argv": ["loom-cli", "accept"] },
        "rules": { "long": true },
        "outputContract": { "schemaShape": { "candidateRules": [] } }
    });

    let findings = audit_legacy_request_authorities(&legacy);
    assert_eq!(
        findings,
        vec![
            "root requestManifest refs",
            "agentAction read contract",
            "agentAction submit argv",
            "root submitCommand",
            "read group sidecar instructions",
            "broad read fields",
            "duplicate context ref paths",
        ]
    );
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

#[test]
fn native_submit_authorizes_declared_write_targets() {
    let fixture = Fixture::new("submit-native");
    write_json_atomic(
        &fixture.root.join(".loom/agent-writable/candidate.json"),
        &json!({ "summary": "ok" }),
    )
    .expect("write target");
    let stored = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_submit_1".to_string(),
            request_kind: "brainstorm_candidate".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase_1".to_string()),
            root: json!({
                "artifactKind": "brainstorm_candidate",
                "submitTool": "loom.brainstormAcceptFile",
                "outputContract": {
                    "writeMode": "single_json",
                    "writeTargets": [{
                        "targetId": "candidate",
                        "path": ".loom/agent-writable/candidate.json",
                        "required": true,
                        "description": "Brainstorm candidate JSON."
                    }]
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "core",
                        "required": true,
                        "purpose": "Read core fields.",
                        "whenToRead": "Before writing.",
                        "fields": ["writeTargets"]
                    }]
                }
            }),
        },
    )
    .expect("write request");
    let compact_root = read_json_value(&fixture.root.join(&stored.request_file))
        .expect("read compact submit request");
    assert!(compact_root.get("outputContract").is_none());
    let storage_manifest = read_json_value(
        &fixture
            .root
            .join(".loom/requests/req_submit_1.manifest.json"),
    )
    .expect("read submit private manifest");
    assert!(storage_manifest["refs"]["outputContract"].is_object());

    let authorized = state::authorize_write_targets(
        &delivery_core::FileSubmitInput {
            project_root: fixture.root_str().to_string(),
            request_ref: stored.request_ref,
            written_target_ids: Some(vec!["candidate".to_string()]),
        },
        "loom.brainstormAcceptFile",
    )
    .expect("authorize submit");

    assert_eq!(
        authorized.artifact_kind,
        delivery_core::ArtifactKind::BrainstormCandidate
    );
    assert_eq!(authorized.targets[0].target_id, "candidate");
    assert_eq!(authorized.submit_tool, "loom.brainstormAcceptFile");
}

#[test]
fn native_submit_rejects_legacy_ts_request_ref() {
    let fixture = Fixture::new("submit-legacy");
    let legacy_file = fixture.root.join(".loom/legacy/request.json");
    write_json_atomic(
        &legacy_file,
        &json!({
            "requestKind": "legacy_request",
            "requestReadPlan": {
                "groups": [{
                    "groupId": "legacy_core",
                    "fields": ["task.title"]
                }]
            },
            "task": { "title": "旧请求标题" }
        }),
    )
    .expect("write legacy request");
    let request_ref = register_legacy_ts_request(fixture.root_str(), ".loom/legacy/request.json")
        .expect("register legacy request");

    let error = state::authorize_write_targets(
        &delivery_core::FileSubmitInput {
            project_root: fixture.root_str().to_string(),
            request_ref,
            written_target_ids: None,
        },
        "loom.brainstormAcceptFile",
    )
    .expect_err("legacy request cannot be submitted");

    let WriteTargetAuthorizationError::Fatal { code, message } = error else {
        panic!("expected fatal legacy submit rejection");
    };
    assert_eq!(code, "LEGACY_REQUEST_NOT_ALLOWED");
    assert!(message.contains("migration inputs"));
}

#[test]
fn legacy_artifact_reader_is_read_only_migration_input() {
    let fixture = Fixture::new("legacy-artifact");
    write_json_atomic(
        &fixture.root.join(".loom/legacy/artifact.json"),
        &json!({ "summary": "old artifact" }),
    )
    .expect("write legacy artifact");

    let artifact = state::read_legacy_ts_artifact(fixture.root_str(), ".loom/legacy/artifact.json")
        .expect("read legacy artifact");

    assert_eq!(artifact.artifact_file, ".loom/legacy/artifact.json");
    assert_eq!(artifact.value["summary"], "old artifact");
    let index =
        state::request_index::load_request_index(fixture.root_str()).expect("request index loads");
    assert!(index.requests.is_empty());
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

fn audit_legacy_request_authorities(value: &serde_json::Value) -> Vec<&'static str> {
    let mut findings = Vec::new();
    if value.pointer("/requestManifest/refs").is_some() {
        findings.push("root requestManifest refs");
    }
    if value.pointer("/agentAction/read").is_some() {
        findings.push("agentAction read contract");
    }
    if value.pointer("/agentAction/submit/command/argv").is_some() {
        findings.push("agentAction submit argv");
    }
    if value.get("submitCommand").is_some() {
        findings.push("root submitCommand");
    }
    let read_groups = value
        .pointer("/agentAction/read/fieldGroups")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            value
                .pointer("/requestReadPlan/groups")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten(),
        )
        .collect::<Vec<_>>();
    if read_groups
        .iter()
        .any(|group| group.get("readCommand").is_some() || group.get("fallbackRule").is_some())
    {
        findings.push("read group sidecar instructions");
    }
    let broad_fields = ["rules", "outputContract.schemaShape"];
    if read_groups.iter().any(|group| {
        group
            .get("fields")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .any(|field| broad_fields.contains(&field))
    }) {
        findings.push("broad read fields");
    }
    if has_duplicate_context_ref_path(value) {
        findings.push("duplicate context ref paths");
    }
    findings
}

fn has_duplicate_context_ref_path(value: &serde_json::Value) -> bool {
    let Some(context_refs) = value
        .get("contextRefs")
        .and_then(serde_json::Value::as_object)
    else {
        return false;
    };
    let mut seen = std::collections::BTreeSet::new();
    for path in context_refs.values().filter_map(serde_json::Value::as_str) {
        if !seen.insert(path) {
            return true;
        }
    }
    false
}
