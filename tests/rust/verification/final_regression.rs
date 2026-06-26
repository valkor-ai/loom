use std::{
    fs,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use state::{store::read_json_value, write_native_request, NativeRequestInput};
use verification::{
    canonical_project_projection, generate_final_verification, require_final_verification_passed,
    BenchmarkInput, FinalVerificationInput, NamedCheck, TranscriptInput,
};

#[test]
fn final_verification_reports_cover_protocol_metrics_and_delivery_isolation() {
    let fixture = Fixture::persistent("final-regression");
    let server = LoomMcpServer::default();

    let first = structured(
        server
            .invoke_tool(
                "loom.plan",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestText": "实现证券账户开户、挂失补办、销户闭环"
                }))),
            )
            .expect("first plan"),
    );
    let first_delivery = first["deliveryId"].as_str().expect("first delivery");
    let first_request = first["requestRef"].as_str().expect("first request");
    let first_write_request = confirm_all_brainstorm_blocks(&server, &fixture, first_request);
    state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: first_write_request,
        group_id: "candidate_write_contract".to_string(),
    })
    .expect("read first request group");

    let phase_two = write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: "phase-2-synthetic-request".to_string(),
            request_kind: "technical_baseline_request".to_string(),
            request_file: None,
            delivery_id: Some(first_delivery.to_string()),
            phase_id: Some("phase-2".to_string()),
            root: json!({
                "protocolPurpose": "phase two isolation request",
                "outputContract": {
                    "artifactKind": "technical_baseline_candidate",
                    "submitTool": "loom.technicalBaselineAcceptFile",
                    "writeTargets": [{
                        "targetId": "candidate",
                        "path": ".loom/agent-writable/phase-2-technical-baseline.json",
                        "required": true,
                        "description": "Synthetic phase two candidate for isolation verification."
                    }]
                },
                "requestReadPlan": {
                    "groups": [{
                        "groupId": "write_contract",
                        "required": true,
                        "purpose": "Read phase two write contract.",
                        "whenToRead": "Before writing the phase two candidate.",
                        "fields": [
                            "protocolPurpose",
                            "outputContract.submitTool",
                            "outputContract.writeTargets"
                        ]
                    }]
                }
            }),
        },
    )
    .expect("write phase two request");
    state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: phase_two.request_ref,
        group_id: "write_contract".to_string(),
    })
    .expect("read phase two group");

    std::thread::sleep(Duration::from_millis(2));
    let second = structured(
        server
            .invoke_tool(
                "loom.plan",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestText": "实现资金账户开户与证券账户关联"
                }))),
            )
            .expect("second plan"),
    );
    let second_delivery = second["deliveryId"].as_str().expect("second delivery");
    assert_ne!(first_delivery, second_delivery);
    let second_request = second["requestRef"].as_str().expect("second request");
    let second_write_request = confirm_all_brainstorm_blocks(&server, &fixture, second_request);
    state::read_field_group(delivery_core::ReadFieldGroupInput {
        project_root: fixture.root_str().to_string(),
        request_ref: second_write_request,
        group_id: "candidate_write_contract".to_string(),
    })
    .expect("read second request group");

    let reference_projection =
        canonical_project_projection(fixture.root_str()).expect("canonical projection");
    let summary = generate_final_verification(
        fixture.root_str(),
        FinalVerificationInput {
            reference_projection,
            transcripts: vec![
                transcript("codex"),
                transcript("claude-code"),
                transcript("opencode"),
            ],
            setup_checks: setup_matrix(),
            package_entries: release_entries(),
            benchmark: BenchmarkInput {
                reference_payload_bytes: 1_000_000,
                reference_completion_rate: 1.0,
                mcp_completion_rate: 1.0,
                notes: vec!["fixture benchmark gate".to_string()],
            },
        },
    )
    .expect("generate final verification");
    require_final_verification_passed(&summary).expect("final verification passed");

    let final_dir = fixture.root.join(".loom/verification/final");
    let token_dir = fixture.root.join(".loom/verification/token-read-plan");
    assert!(final_dir.join("mcp-only-regression-report.md").exists());
    assert!(final_dir.join("canonical-projection-diff.json").exists());
    assert!(final_dir.join("protocol-snapshot-report.json").exists());
    assert!(final_dir.join("agent-transcript-audit.json").exists());
    assert!(final_dir.join("setup-packaging-report.json").exists());
    assert!(final_dir.join("token-read-plan-report.json").exists());
    assert!(final_dir.join("benchmark-summary.md").exists());
    assert!(token_dir.join("summary.json").exists());
    assert!(token_dir.join("by-stage.json").exists());
    assert!(token_dir.join("benchmark.md").exists());

    let protocol =
        read_json_value(&final_dir.join("protocol-snapshot-report.json")).expect("protocol report");
    assert_eq!(protocol["status"], "passed");
    assert_eq!(
        protocol["issues"]
            .as_array()
            .expect("protocol issues")
            .len(),
        0
    );
    assert_eq!(
        protocol["requestCount"].as_u64().expect("request count"),
        11
    );
    let deliveries = protocol["deliveries"].as_array().expect("deliveries");
    assert_eq!(deliveries.len(), 2);
    let first_summary = deliveries
        .iter()
        .find(|delivery| delivery["deliveryId"] == first_delivery)
        .expect("first delivery summary");
    assert!(first_summary["phaseIds"]
        .as_array()
        .expect("phase ids")
        .iter()
        .any(|phase| phase == "phase-2"));
    assert!(first_summary["requestIds"]
        .as_array()
        .expect("request ids")
        .iter()
        .any(|request| request == "phase-2-synthetic-request"));

    let token_summary = read_json_value(&token_dir.join("summary.json")).expect("token summary");
    assert_eq!(token_summary["status"], "passed");
    assert_eq!(token_summary["fullArtifactReadCount"], 0);
    assert_eq!(token_summary["jqCount"], 0);
    assert!(token_summary["readFieldGroupCount"].as_u64().unwrap_or(0) >= 3);

    let agent_audit =
        read_json_value(&final_dir.join("agent-transcript-audit.json")).expect("agent audit");
    assert_eq!(agent_audit["status"], "passed");
    assert_eq!(agent_audit["audits"].as_array().expect("audits").len(), 3);

    let report = fs::read_to_string(final_dir.join("mcp-only-regression-report.md"))
        .expect("regression report");
    assert!(report.contains("Overall: passed"));
    assert!(report.contains("Release blockers: 0"));
}

fn transcript(agent: &str) -> TranscriptInput {
    TranscriptInput {
        agent: agent.to_string(),
        source: format!("{agent}-smoke.jsonl"),
        content: format!(
            "{agent} called loom.plan, loom.readFieldGroup, loom.brainstormAcceptFile, loom.continue and followed auto_runnable next actions."
        ),
    }
}

fn setup_matrix() -> Vec<NamedCheck> {
    [
        "macos-arm64 install doctor upgrade legacy-cleanup uninstall purge",
        "linux-x64 install doctor upgrade legacy-cleanup uninstall purge",
        "windows-x64 install doctor upgrade legacy-cleanup uninstall purge",
        "macos-x64 package smoke manifest checksum doctor smoke",
        "linux-arm64 package smoke manifest checksum doctor smoke",
    ]
    .into_iter()
    .map(|name| NamedCheck {
        name: name.to_string(),
        passed: true,
        details: json!({ "source": "batch13-fixture" }),
    })
    .collect()
}

fn release_entries() -> Vec<String> {
    vec![
        "bin/loom-mcp-server".to_string(),
        "bin/loom-setup".to_string(),
        "python/runtime/README".to_string(),
        "python/algorithms/worker.py".to_string(),
        "plugins/codex/plugin.json".to_string(),
        "plugins/claude-code/plugin.json".to_string(),
        "plugins/opencode/plugin.json".to_string(),
    ]
}

fn args(value: Value) -> rmcp::model::JsonObject {
    value.as_object().cloned().expect("json object args")
}

fn structured(result: rmcp::model::CallToolResult) -> Value {
    serde_json::to_value(result).expect("call result to value")["structuredContent"].clone()
}

fn confirm_all_brainstorm_blocks(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
) -> String {
    let mut request_ref = request_ref.to_string();
    request_ref = confirm_block(
        server,
        fixture,
        &request_ref,
        "phase_scope",
        "确认第一阶段为证券账户模块闭环。",
        json!({
            "scope": {
                "included": ["证券账户开户", "证券账户挂失补办", "证券账户销户"],
                "deferred": ["资金账户", "交易客户端"],
                "excluded": []
            },
            "recommendation": {
                "label": "证券账户模块闭环",
                "reason": "证券账户是交易身份基础。"
            }
        }),
    )["requestRef"]
        .as_str()
        .expect("concept requestRef")
        .to_string();
    request_ref = confirm_block(
        server,
        fixture,
        &request_ref,
        "concept_grounding",
        "确认证券账户生命周期规则。",
        json!({
            "objects": ["证券账户"],
            "operations": ["开户", "挂失补办", "销户"],
            "rules": ["销户前必须清空持仓"],
            "boundaries": ["资金账户递延"]
        }),
    )["requestRef"]
        .as_str()
        .expect("frontend requestRef")
        .to_string();
    request_ref = confirm_block(
        server,
        fixture,
        &request_ref,
        "frontend_experience",
        "确认工作人员后台证券账户管理页面路径。",
        json!({
            "required": true,
            "surfaces": ["证券账户管理页面"],
            "targetDiscovery": ["分页查询列表"],
            "operationPaths": ["开户从新建入口进入", "挂失补办和销户先查询并选择目标账户"],
            "mustNot": ["不能只靠内部主键触发办理动作"]
        }),
    )["requestRef"]
        .as_str()
        .expect("final requestRef")
        .to_string();
    let write_action = confirm_block(
        server,
        fixture,
        &request_ref,
        "final_summary",
        "用户已确认阶段范围、业务理解、页面办理路径和提交前核对。",
        json!({
            "coverageChecklist": ["证券账户模块闭环", "生命周期规则", "工作人员页面路径"],
            "readyToWriteCandidate": true
        }),
    );
    assert_eq!(write_action["state"], "auto_runnable", "{write_action:#}");
    write_action["next"]["requestRef"]
        .as_str()
        .expect("candidate write requestRef")
        .to_string()
}

fn confirm_block(
    server: &LoomMcpServer,
    fixture: &Fixture,
    request_ref: &str,
    block: &str,
    summary: &str,
    confirmed_data: Value,
) -> Value {
    structured(
        server
            .invoke_tool(
                "loom.brainstormConfirmBlock",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "requestRef": request_ref,
                    "block": block,
                    "summary": summary,
                    "confirmedData": confirmed_data
                }))),
            )
            .expect("confirm brainstorm block"),
    )
}

struct Fixture {
    root: std::path::PathBuf,
    preserve: bool,
    _guard: MutexGuard<'static, ()>,
}

impl Fixture {
    fn persistent(name: &str) -> Self {
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let guard = ENV_LOCK.lock().expect("env lock");
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../target/mcp-only-final-verification")
            .join(name);
        if root.exists() {
            fs::remove_dir_all(&root).expect("clean persistent fixture root");
        }
        fs::create_dir_all(&root).expect("create fixture root");
        std::env::set_var("LOOM_HOME", root.join(".loom-home"));
        Self {
            root,
            preserve: true,
            _guard: guard,
        }
    }

    fn root_str(&self) -> &str {
        self.root.to_str().expect("fixture path utf8")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.preserve {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
