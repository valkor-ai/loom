use serde_json::json;
use state::{
    paths::{
        delivery_dir, delivery_index_file, operation_lease_file, phase_tmp_dir, project_paths,
        task_run_file, workspace_dir, DeliveryPhaseLocator, DeliveryPhaseRunLocator,
    },
    request_resolver::read_field_group_by_resource_uri,
    store::{read_json_value, write_json_atomic, write_text_atomic},
    write_native_request, NativeRequestInput,
};
use std::fs::read_to_string;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

#[test]
fn project_paths_include_delivery_and_status_roots() {
    let root = std::env::current_dir().expect("current dir");
    let paths = project_paths(root.to_str().expect("current dir utf8")).expect("project paths");
    assert!(paths.status_file.ends_with(".loom/status.json"));
    assert!(paths.gitignore_file.ends_with(".loom/.gitignore"));
    assert!(paths.deliveries_dir.ends_with(".loom/deliveries"));
    assert!(paths.tmp_dir.ends_with(".loom/tmp"));
}

#[test]
fn delivery_phase_and_run_locators_match_mcp_layout() {
    let root = PathBuf::from("/tmp/loom-state-paths");
    let phase = DeliveryPhaseLocator {
        delivery_id: "delivery_1".to_string(),
        phase_id: "phase_scope".to_string(),
    };
    let run = DeliveryPhaseRunLocator {
        delivery_id: "delivery_1".to_string(),
        phase_id: "phase_scope".to_string(),
        run_id: "run_1".to_string(),
    };

    assert_eq!(
        delivery_dir(&root, &phase.delivery_id),
        PathBuf::from("/tmp/loom-state-paths/.loom/deliveries/delivery_1")
    );
    assert_eq!(
        delivery_index_file(&root, &phase.delivery_id),
        PathBuf::from("/tmp/loom-state-paths/.loom/deliveries/delivery_1/index.json")
    );
    assert_eq!(
        phase_tmp_dir(&root, &phase),
        PathBuf::from("/tmp/loom-state-paths/.loom/deliveries/delivery_1/tmp/phase_scope")
    );
    assert_eq!(
        workspace_dir(&root, &phase),
        PathBuf::from("/tmp/loom-state-paths/.loom/deliveries/delivery_1/workspace/phase_scope")
    );
    assert_eq!(
        task_run_file(&root, &run),
        PathBuf::from(
            "/tmp/loom-state-paths/.loom/deliveries/delivery_1/tasks/phase_scope/runs/run_1.json"
        )
    );
    assert_eq!(
        operation_lease_file(&root, &phase.delivery_id),
        PathBuf::from(
            "/tmp/loom-state-paths/.loom/deliveries/delivery_1/operations/active-lease.json"
        )
    );
}

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
                            "selectors": selectors([
                                "task.title",
                                "task.items.0.name",
                                "outputContract.schemaShape.summary",
                                "requirementContext.normalizedText",
                                "keywordHints.compact",
                                "rules.requirementSemanticGrounding.compactRules"
                            ])
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
        group_json
            .as_object()
            .expect("read group object")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["fields".to_string()]
    );
    assert_eq!(
        field(&group.fields, "task.title"),
        &json!("实现证券账户开户")
    );
    let group_text = group_json.to_string();
    assert!(!group_text.contains("sourceRef"));
    assert!(!group_text.contains("sourceKind"));
    assert!(!group_text.contains("selector"));
    assert!(!group_text.contains("\"status\":\"resolved\""));
    assert_eq!(
        field(&group.fields, "task.title"),
        &json!("实现证券账户开户")
    );
    assert_eq!(field(&group.fields, "task.items.0.name"), &json!("开户"));
    assert_eq!(
        field(&group.fields, "outputContract.schemaShape.summary"),
        &json!("string")
    );
    assert_eq!(
        field(&group.fields, "requirementContext.normalizedText"),
        &json!("证券账户开户需求")
    );
    assert_eq!(
        field(&group.fields, "keywordHints.compact.status"),
        &json!("completed")
    );
    assert_eq!(
        field(&group.fields, "keywordHints.compact.topKeywords.0"),
        &json!("证券账户")
    );
    assert_eq!(
        field(
            &group.fields,
            "keywordHints.compact.sectionKeywords.0.keywords"
        ),
        &json!(["开户", "销户"])
    );
    assert!(
        !field(&group.fields, "keywordHints.compact")
            .to_string()
            .contains("\"keyword\""),
        "compact keyword hints must expose keyword arrays as strings"
    );
    assert_eq!(
        field(
            &group.fields,
            "rules.requirementSemanticGrounding.compactRules"
        ),
        &json!(["rule_1", "rule_2", "rule_3", "rule_4", "rule_5", "rule_6", "rule_7"])
    );

    let selected = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        fields: vec!["task.title".to_string(), "task.title".to_string()],
    })
    .expect("read selected fields");
    let selected_json = serde_json::to_value(&selected).expect("serialize selected fields");
    assert_eq!(
        selected_json
            .as_object()
            .expect("selected fields object")
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["fields".to_string()]
    );
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
    assert_eq!(
        field(&by_resource.fields, "task.title"),
        &json!("实现证券账户开户")
    );

    let paths = state::paths::project_paths(fixture.root_str()).expect("project paths");
    let size_audit = read_to_string(paths.request_size_audit_file).expect("request size audit");
    assert!(size_audit.contains("\"requestRef\""));
    assert!(size_audit.contains("req_native_1"));

    let field_audit = read_to_string(paths.field_read_audit_file).expect("field read audit");
    assert!(field_audit.contains("\"source\":\"readFieldGroup\""));
    assert!(field_audit.contains("\"source\":\"internalReadRequestFields\""));
}

#[test]
fn write_groups_receive_shared_contract_metadata_without_reading_private_schema() {
    let fixture = Fixture::new("shared-write-contract");
    let stored = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_shared_contract_1".to_string(),
            request_kind: "technical_baseline_request".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase_1".to_string()),
            root: json!({
                "outputContract": {
                    "artifactKind": "technical_baseline_candidate",
                    "writeMode": "single_json",
                    "submitTool": "loom.technicalBaselineAcceptFile",
                    "writeTargets": [{
                        "targetId": "candidate",
                        "path": ".loom/agent-writable/candidate.json",
                        "required": true
                    }],
                    "schemaShape": {
                        "type": "object",
                        "properties": {
                            "reasoningSummary": {
                                "type": "array",
                                "items": {"type": "string"}
                            }
                        },
                        "required": ["reasoningSummary"]
                    },
                    "schemaProjection": {
                        "requiredTopLevelFields": ["reasoningSummary"]
                    }
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "write_contract",
                        "required": true,
                        "purpose": "Read the write contract.",
                        "whenToRead": "Before writing.",
                        "selectors": selectors(["outputContract.writeTargets"])
                    }]
                }
            }),
        },
    )
    .expect("write request");

    let fields = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref,
        group_id: "write_contract".to_string(),
    })
    .expect("read write group")
    .fields;
    assert_eq!(
        field(&fields, "outputContract.contractVersion"),
        &json!("1.0")
    );
    assert!(field(&fields, "outputContract.contractFingerprint")
        .as_str()
        .is_some_and(|value| value.starts_with("sha256:")));
    assert_eq!(
        field(
            &fields,
            "outputContract.schemaProjection.fieldContract.properties.reasoningSummary.type"
        ),
        &json!("array")
    );
    assert_eq!(
        field(
            &fields,
            "outputContract.schemaProjection.fieldContract.properties.reasoningSummary.items.type"
        ),
        &json!("string")
    );
}

#[test]
fn native_request_size_thresholds_are_audit_warnings_not_flow_blockers() {
    let fixture = Fixture::new("native-size-warning");
    let large_text = "证券账户开户规则。".repeat(5000);

    let stored = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_size_warning_1".to_string(),
            request_kind: "technical_baseline".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase_1".to_string()),
            root: json!({
                "context": {
                    "largeField": large_text
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "large_context",
                        "required": true,
                        "purpose": "Read a large but valid field.",
                        "whenToRead": "Before writing.",
                        "selectors": selectors(["context.largeField"])
                    }]
                }
            }),
        },
    )
    .expect("large fields should warn without blocking native request creation");

    let group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref,
        group_id: "large_context".to_string(),
    })
    .expect("read large field group");
    assert!(field(&group.fields, "context.largeField")
        .as_str()
        .expect("large field text")
        .contains("证券账户开户规则"));

    let audit = read_to_string(fixture.root.join(".loom/metrics/request-size-audit.jsonl"))
        .expect("read request size audit");
    let last_line = audit.lines().last().expect("audit line");
    let audit_value: serde_json::Value = serde_json::from_str(last_line).expect("audit json");
    let warnings = audit_value["readPlanWarnings"]
        .as_array()
        .expect("read plan warnings");
    assert!(warnings.iter().any(|warning| {
        warning["level"] == "warn"
            && warning["groupId"] == "large_context"
            && warning["field"] == "context.largeField"
    }));
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
                        "selectors": selectors(["task.title"])
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
fn native_request_omits_missing_read_fields_and_rejects_broad_fields() {
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
                        "selectors": selectors(["task.missing"])
                    }]
                }
            }),
        },
    )
    .expect("missing read field does not block request generation");
    let missing_group = state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: missing.request_ref.clone(),
        group_id: "core".to_string(),
    })
    .expect("read missing group");
    assert!(missing_group
        .fields
        .as_object()
        .is_some_and(serde_json::Map::is_empty));
    let missing_fields = state::read_request_fields(delivery_core::ReadRequestFieldsInput {
        project_root: fixture.root_str().to_string(),
        request_ref: missing.request_ref,
        fields: vec!["task.missing".to_string()],
    })
    .expect("read missing allowed field");
    assert!(missing_fields.fields.is_empty());

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
                        "selectors": selectors(["rules"])
                    }]
                }
            }),
        },
    )
    .expect_err("broad read field is rejected");
    assert!(broad.to_string().contains("too broad"));

    let private_workflow_state = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_private_workflow_state".to_string(),
            request_kind: "architecture_sections_generation".to_string(),
            request_file: None,
            delivery_id: None,
            phase_id: None,
            root: json!({
                "sectionOutputs": [{ "section": "foundation" }],
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "core",
                        "selectors": selectors(["sectionOutputs.0.section"])
                    }]
                }
            }),
        },
    )
    .expect_err("private workflow state is rejected");
    assert!(private_workflow_state
        .to_string()
        .contains("sectionOutputs is private workflow state"));
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
            "outputContract": {
                "submitTool": submit_tool,
                "writeTargets": [{
                    "targetId": "result",
                    "path": target_path,
                    "required": true,
                    "description": "Agent-writable result JSON."
                }]
            },
            "requestReadPlan": {
                "groups": [{
                    "groupId": "write_contract",
                    "required": true,
                    "purpose": "Read the native MCP write contract.",
                    "whenToRead": "Before writing the result file.",
                    "selectors": selectors([
                        "protocolPurpose",
                        "outputContract.submitTool",
                        "outputContract.writeTargets"
                    ])
                }]
            }
        });
        if let Some(artifact_kind) = artifact_kind {
            root["outputContract"]["artifactKind"] = json!(artifact_kind);
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
            1,
            "{request_kind} should keep the write contract in one private ref"
        );
        assert!(storage_manifest["refs"]["outputContract"].is_object());

        let group = state::read_field_group(delivery_core::ReadFieldGroupInput {
            project_root: fixture.root_str().to_string(),
            request_ref: stored.request_ref,
            group_id: "write_contract".to_string(),
        })
        .expect("read snapshot write contract");
        assert_eq!(
            field(&group.fields, "protocolPurpose"),
            &json!(request_kind)
        );
        assert_eq!(
            field(&group.fields, "outputContract.submitTool"),
            &json!(submit_tool)
        );
        assert!(field(&group.fields, "outputContract.writeTargets").is_array());
    }
}

#[test]
fn duplicated_protocol_authority_fixture_reports_repeated_authorities() {
    let duplicated = json!({
        "contextRefs": {
            "requestTextRef": ".loom/requirements/normalized.txt",
            "normalizedRequirementTextRef": ".loom/requirements/normalized.txt"
        },
        "agentAction": {
            "read": {
                "fieldGroups": [{
                    "groupId": "duplicated",
                    "fields": ["rules", "outputContract.schemaShape"],
                    "readCommand": { "argv": ["old-runner", "inspect"] },
                    "fallbackRule": "Read requestManifest refs."
                }]
            },
            "submit": { "command": { "argv": ["old-runner", "accept"] } }
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
        "submitCommand": { "argv": ["old-runner", "accept"] },
        "rules": { "long": true },
        "outputContract": { "schemaShape": { "candidateRules": [] } }
    });

    let findings = audit_legacy_request_authorities(&duplicated);
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
                "outputContract": {
                    "artifactKind": "brainstorm_candidate",
                    "submitTool": "loom.brainstormAcceptFile",
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
                        "selectors": selectors(["outputContract.writeTargets"])
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

    state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        group_id: "core".to_string(),
    })
    .expect("read submit write contract");

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
fn native_submit_rejects_a_read_from_an_older_contract_fingerprint() {
    let fixture = Fixture::new("submit-stale-contract");
    write_json_atomic(
        &fixture.root.join(".loom/agent-writable/candidate.json"),
        &json!({ "summary": "ok" }),
    )
    .expect("write target");
    let stored = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "req_submit_stale_contract".to_string(),
            request_kind: "brainstorm_candidate".to_string(),
            request_file: None,
            delivery_id: Some("delivery_1".to_string()),
            phase_id: Some("phase_1".to_string()),
            root: json!({
                "outputContract": {
                    "artifactKind": "brainstorm_candidate",
                    "submitTool": "loom.brainstormAcceptFile",
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
                        "selectors": selectors(["outputContract.writeTargets"])
                    }]
                }
            }),
        },
    )
    .expect("write request");

    state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: stored.request_ref.clone(),
        group_id: "core".to_string(),
    })
    .expect("read initial write contract");

    let manifest = read_json_value(
        &fixture
            .root
            .join(".loom/requests/req_submit_stale_contract.manifest.json"),
    )
    .expect("read request manifest");
    let output_contract_ref = manifest["refs"]["outputContract"]["ref"]
        .as_str()
        .expect("output contract ref");
    let output_contract_file = fixture.root.join(output_contract_ref);
    let mut output_contract = read_json_value(&output_contract_file).expect("read output contract");
    output_contract["schemaProjection"]["requiredTopLevelFields"] = json!(["summary"]);
    delivery_core::finalize_output_contract(
        &mut output_contract,
        &std::collections::BTreeMap::new(),
    );
    write_json_atomic(&output_contract_file, &output_contract).expect("rewrite output contract");

    let error = state::authorize_write_targets(
        &delivery_core::FileSubmitInput {
            project_root: fixture.root_str().to_string(),
            request_ref: stored.request_ref,
            written_target_ids: Some(vec!["candidate".to_string()]),
        },
        "loom.brainstormAcceptFile",
    )
    .expect_err("stale contract read must not authorize submit");
    match error {
        state::WriteTargetAuthorizationError::Repairable { issues, .. } => assert!(issues
            .iter()
            .any(|issue| issue.code == "WRITE_CONTRACT_NOT_READ")),
        other => panic!("expected repairable stale-contract error, got {other:?}"),
    }
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

fn selectors<const N: usize>(fields: [&str; N]) -> serde_json::Value {
    delivery_core::read_selectors_value_from_paths(fields)
}

fn field<'a>(value: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    let mut current = value;
    for part in path.split('.') {
        current = if let Ok(index) = part.parse::<usize>() {
            current
                .as_array()
                .and_then(|items| items.get(index))
                .unwrap_or_else(|| panic!("missing array path segment {part} in {path}"))
        } else {
            current
                .get(part)
                .unwrap_or_else(|| panic!("missing object path segment {part} in {path}"))
        };
    }
    current
}
