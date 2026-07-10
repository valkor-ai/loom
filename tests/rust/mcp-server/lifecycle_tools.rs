use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard};

#[test]
fn init_project_creates_mcp_state_files() {
    let fixture = Fixture::new("init-project");
    let server = LoomMcpServer::default();

    let result = server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("initProject call");
    let value = structured(result);

    assert_eq!(value["state"], "done");
    assert_eq!(value["details"]["initialized"], true);
    assert!(fixture.root.join(".loom/status.json").exists());
    assert!(fixture.root.join(".loom/.gitignore").exists());
}

#[test]
fn status_requires_init_and_then_returns_structured_summary() {
    let fixture = Fixture::new("status-tool");
    let server = LoomMcpServer::default();

    let before = server
        .invoke_tool(
            "loom.status",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("status before init");
    let before_value = structured(before);
    assert_eq!(before_value["state"], "failed");
    assert_eq!(before_value["error"]["code"], "STATE_NOT_INITIALIZED");

    server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("initProject call");
    let after = server
        .invoke_tool(
            "loom.status",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("status after init");
    let after_value = structured(after);
    assert_eq!(after_value["state"], "done");
    assert_eq!(after_value["details"]["initialized"], true);
    assert_eq!(after_value["details"]["workflowState"], "idle");
    assert_eq!(after_value["details"]["hasActiveWorkflow"], false);
    assert_eq!(after_value["details"]["activeDeliveryId"], Value::Null);
}

fn args(value: Value) -> rmcp::model::JsonObject {
    value.as_object().cloned().expect("json object args")
}

fn structured(result: rmcp::model::CallToolResult) -> Value {
    serde_json::to_value(result).expect("call result to value")["structuredContent"].clone()
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
            "loom-mcp-lifecycle-{name}-{}-{}",
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
