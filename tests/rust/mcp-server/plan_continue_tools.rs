use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard};

#[test]
fn plan_returns_batch_seven_failure_without_creating_delivery() {
    let fixture = Fixture::new("plan-tool");
    let server = LoomMcpServer::default();

    let result = server
        .invoke_tool(
            "loom.plan",
            Some(args(json!({
                "projectRoot": fixture.root_str(),
                "requestText": "实现证券账户开户流程"
            }))),
        )
        .expect("plan call");
    let value = structured(result);

    assert_eq!(value["state"], "failed");
    assert_eq!(value["error"]["code"], "not_implemented_for_batch");
    assert_eq!(value["error"]["targetBatch"], 7);
    assert_eq!(value["error"]["domain"], "brainstorm");
    assert_eq!(count_entries(&fixture.root.join(".loom/deliveries")), 0);
}

#[test]
fn continue_blocks_after_init_when_no_active_delivery_exists() {
    let fixture = Fixture::new("continue-tool");
    let server = LoomMcpServer::default();

    server
        .invoke_tool(
            "loom.initProject",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("initProject");
    let result = server
        .invoke_tool(
            "loom.continue",
            Some(args(json!({ "projectRoot": fixture.root_str() }))),
        )
        .expect("continue");
    let value = structured(result);

    assert_eq!(value["state"], "blocked");
    assert_eq!(value["recommendedTool"], "loom.plan");
}

fn count_entries(path: &std::path::Path) -> usize {
    std::fs::read_dir(path)
        .expect("read dir")
        .filter_map(Result::ok)
        .count()
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
            "loom-mcp-plan-{name}-{}-{}",
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
