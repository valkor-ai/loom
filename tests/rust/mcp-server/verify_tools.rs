use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard};

#[test]
fn verify_onboarding_records_deferred_choice_without_creating_appkey() {
    let fixture = Fixture::new("deferred");
    let server = LoomMcpServer::default();
    let root = fixture.root.to_string_lossy().into_owned();

    let initialized = server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": root }))),
        )
        .expect("initProject call");
    assert_eq!(structured(initialized)["state"], "done");

    let gate = server
        .invoke_tool(
            "loom.verify",
            Some(args(json!({ "projectRoot": fixture.root }))),
        )
        .expect("verify onboarding call");
    let gate = structured(gate);
    assert_eq!(gate["state"], "user_gate", "{gate:#}");
    assert_eq!(gate["gate"]["kind"], "vsefm_onboarding");
    assert_eq!(gate["acceptedResponses"], json!(["required", "deferred"]));
    assert!(fixture
        .root
        .join(".loom/verification/v-sefm.json")
        .is_file());

    let resolved = server
        .invoke_tool(
            "loom.verify",
            Some(args(json!({
                "projectRoot": fixture.root,
                "decision": "deferred"
            }))),
        )
        .expect("verify deferred call");
    let resolved = structured(resolved);
    assert_eq!(resolved["state"], "done", "{resolved:#}");
    assert_eq!(resolved["details"]["decision"], "deferred");
    assert!(!fixture.root.join(".loom-home/v-sefm/appkey").exists());

    let record: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.root.join(".loom/verification/v-sefm.json"))
            .expect("V-SEFM record"),
    )
    .expect("valid V-SEFM record");
    assert_eq!(record["decision"], "deferred");
    assert_eq!(record["status"], "completed");
    assert!(record.get("appKey").is_none());
    assert!(record.get("resumeAction").is_none());
}

fn args(value: Value) -> rmcp::model::JsonObject {
    value.as_object().cloned().expect("json object args")
}

fn structured(result: rmcp::model::CallToolResult) -> Value {
    serde_json::to_value(result).expect("call result")["structuredContent"].clone()
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
            "loom-mcp-verify-{name}-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ));
        std::fs::create_dir_all(&root).expect("fixture root");
        std::env::set_var("LOOM_HOME", root.join(".loom-home"));
        Self {
            root,
            _guard: guard,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
