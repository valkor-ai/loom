use mcp_server::LoomMcpServer;
use serde_json::{json, Value};
use std::sync::{Mutex, MutexGuard};

#[test]
fn knowledge_add_returns_done_summary_with_pending_details() {
    let fixture = Fixture::new("knowledge-add");
    fixture.write_file(
        "docs/rules.md",
        "# 证券账户\n\n证券账户开户、销户、挂失补办都需要工作人员办理。",
    );
    let server = LoomMcpServer::default();

    let result = server
        .invoke_tool(
            "loom.knowledgeAdd",
            Some(args(json!({
                "projectRoot": fixture.root_str(),
                "name": "stock-rules",
                "paths": [fixture.root.join("docs").to_string_lossy().to_string()]
            }))),
        )
        .expect("knowledgeAdd call");
    let value = structured(result);

    assert_eq!(value["state"], "done");
    assert_eq!(value["summary"], "Knowledge source registered.");
    assert_eq!(value["details"]["source"]["name"], "stock-rules");
    assert_eq!(
        value["details"]["pending"]["operations"][0]["kind"],
        "add_paths"
    );
}

#[test]
fn knowledge_pending_accepts_optional_source_name_filter() {
    let fixture = Fixture::new("knowledge-pending-filter");
    fixture.write_file("docs/a.md", "# A\n\n证券账户开户。");
    fixture.write_file("docs/b.md", "# B\n\n资金账户开户。");
    let server = LoomMcpServer::default();

    for (name, file) in [("stock-rules", "docs/a.md"), ("fund-rules", "docs/b.md")] {
        server
            .invoke_tool(
                "loom.knowledgeAdd",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "name": name,
                    "paths": [fixture.root.join(file).to_string_lossy().to_string()]
                }))),
            )
            .expect("knowledgeAdd call");
    }

    let filtered = structured(
        server
            .invoke_tool(
                "loom.knowledgePending",
                Some(args(json!({
                    "projectRoot": fixture.root_str(),
                    "name": "stock-rules"
                }))),
            )
            .expect("knowledgePending filtered call"),
    );

    assert_eq!(filtered["state"], "done");
    let sources = filtered["details"]["sources"]
        .as_array()
        .expect("pending sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["source"]["name"], "stock-rules");
}

#[test]
fn knowledge_build_returns_auto_runnable_semantic_next_action() {
    let fixture = Fixture::new("knowledge-build");
    fixture.write_file(
        "rules.md",
        "# 页面办理路径\n\n证券账户管理页面支持开户、销户和挂失补办。",
    );
    let server = LoomMcpServer::default();

    server
        .invoke_tool(
            "loom.knowledgeAdd",
            Some(args(json!({
                "projectRoot": fixture.root_str(),
                "name": "page-paths",
                "paths": [fixture.root.join("rules.md").to_string_lossy().to_string()]
            }))),
        )
        .expect("knowledgeAdd call");
    let result = server
        .invoke_tool(
            "loom.knowledgeBuild",
            Some(args(json!({
                "projectRoot": fixture.root_str(),
                "name": "page-paths"
            }))),
        )
        .expect("knowledgeBuild call");
    let value = structured(result);

    assert_eq!(value["state"], "auto_runnable");
    assert_eq!(value["next"]["kind"], "generate_knowledge_semantics");
    assert_eq!(
        value["next"]["submitTool"],
        "loom.knowledgeSemanticSubmitFile"
    );
    assert!(!value.to_string().contains("submitCommand"));
    assert!(!value.to_string().contains("readCommand"));
    assert!(!value.to_string().contains("argv"));
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
        let guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "loom-mcp-server-knowledge-{name}-{}-{}",
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

    fn write_file(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, text).expect("write fixture file");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
