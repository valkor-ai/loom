use std::sync::{Mutex, MutexGuard};

use mcp_server::server::LoomMcpServer;
use serde_json::json;
use state::{store::write_json_atomic, write_native_request, NativeRequestInput};

#[test]
fn submit_tool_returns_repairable_error_for_missing_target_file() {
    let fixture = Fixture::new("submit-missing-target");
    let stored = write_brainstorm_request(&fixture, "req_submit_missing", false);
    let result = call_submit(&stored.request_ref, fixture.root_str());

    assert_eq!(result["state"], "repairable_error");
    assert_eq!(result["targetFile"], ".loom/agent-writable/candidate.json");
    assert_eq!(result["targetIds"], json!(["candidate"]));
    assert_eq!(result["issues"][0]["code"], "TARGET_MISSING");
    assert_eq!(result["resubmitTool"], "loom.brainstormAcceptFile");
    assert!(result.get("submitCommand").is_none());
}

#[test]
fn submit_tool_runs_native_preflight_before_domain_handler() {
    let fixture = Fixture::new("submit-preflight");
    let stored = write_brainstorm_request(&fixture, "req_submit_preflight", true);
    let result = call_submit(&stored.request_ref, fixture.root_str());

    assert_eq!(result["state"], "failed");
    assert_eq!(result["error"]["code"], "not_implemented_for_batch");
    assert_eq!(result["error"]["targetBatch"], 7);
    assert!(result["error"]["message"]
        .as_str()
        .expect("message")
        .contains("passed MCP native submit preflight"));
}

fn write_brainstorm_request(
    fixture: &Fixture,
    request_id: &str,
    write_target: bool,
) -> state::StoredRequest {
    if write_target {
        write_json_atomic(
            &fixture.root.join(".loom/agent-writable/candidate.json"),
            &json!({ "summary": "ok" }),
        )
        .expect("write target");
    }
    write_native_request(
        fixture.root_str(),
        NativeRequestInput {
            request_id: request_id.to_string(),
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
    .expect("write request")
}

fn call_submit(request_ref: &str, project_root: &str) -> serde_json::Value {
    let server = LoomMcpServer::default();
    let arguments = json!({
        "projectRoot": project_root,
        "requestRef": request_ref,
        "writtenTargetIds": ["candidate"]
    })
    .as_object()
    .expect("arguments object")
    .clone();
    server
        .invoke_tool("loom.brainstormAcceptFile", Some(arguments))
        .expect("call tool")
        .structured_content
        .expect("structured content")
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
            "loom-mcp-submit-{name}-{}-{}",
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
