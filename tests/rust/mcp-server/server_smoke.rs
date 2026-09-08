use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};

#[test]
fn stdio_server_initializes_and_lists_batch_2_surface() {
    let mut client = McpProcess::start();

    let initialize = client.request(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "loom-test-client", "version": "0.0.1" }
        }
    }));
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "loom-mcp-server"
    );
    let instructions = initialize["result"]["instructions"]
        .as_str()
        .expect("server instructions");
    assert!(instructions.contains("plain @loom software delivery request"));
    assert!(instructions.contains("call the plan tool first"));
    assert!(initialize["result"]["capabilities"].get("tools").is_some());
    assert!(initialize["result"]["capabilities"]
        .get("resources")
        .is_some());

    client.notify(json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    let tools = client.request(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    }));
    let tool_names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    assert!(tool_names.contains(&"status"));
    assert!(tool_names.contains(&"readFieldGroup"));
    for tool in tools["result"]["tools"].as_array().expect("tools array") {
        assert_eq!(
            tool["outputSchema"]["type"].as_str(),
            Some("object"),
            "{} outputSchema must be a top-level object schema",
            tool["name"]
        );
    }

    let templates = client.request(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "resources/templates/list"
    }));
    let template_uris: Vec<&str> = templates["result"]["resourceTemplates"]
        .as_array()
        .expect("resource templates array")
        .iter()
        .map(|template| template["uriTemplate"].as_str().expect("uri template"))
        .collect();
    assert!(template_uris
        .contains(&"loom://projects/{projectId}/requests/{requestId}/field-groups/{groupId}"));

    let project_root = std::env::current_dir()
        .expect("current dir")
        .canonicalize()
        .expect("canonical current dir")
        .to_string_lossy()
        .into_owned();
    let status = client.request(json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "loom.status",
            "arguments": { "projectRoot": project_root }
        }
    }));
    assert_eq!(status["result"]["structuredContent"]["state"], "failed");
    assert_eq!(
        status["result"]["structuredContent"]["error"]["code"],
        "STATE_NOT_INITIALIZED"
    );
    assert_eq!(
        status["result"]["structuredContent"]["error"]["recoveryTool"],
        "loom.initProject"
    );
}

#[test]
fn stdio_server_persists_and_reads_a_delivery_request() {
    let fixture = TestProject::new("delivery-lifecycle");
    let mut client = McpProcess::start_with_loom_home(&fixture.loom_home);
    client.initialize();

    let initialized = client.call_tool(
        1,
        "loom.initProject",
        json!({ "projectRoot": fixture.project_root }),
    );
    assert_eq!(initialized["result"]["structuredContent"]["state"], "done");
    assert!(fixture.project_root.join(".loom/status.json").exists());

    let planned = client.call_tool(
        2,
        "loom.plan",
        json!({
            "projectRoot": fixture.project_root,
            "requestText": "Add an account search flow with tests and a clear handoff."
        }),
    );
    let plan = &planned["result"]["structuredContent"];
    assert_eq!(plan["state"], "user_gate", "{plan:#}");
    let request_ref = plan["requestRef"].as_str().expect("request ref");

    let inspected = client.call_tool(
        3,
        "loom.inspectRequest",
        json!({
            "projectRoot": fixture.project_root,
            "requestRef": request_ref
        }),
    );
    let inspection = &inspected["result"]["structuredContent"];
    assert_eq!(inspection["requestRef"], request_ref);
    assert!(inspection["readGroups"]
        .as_array()
        .is_some_and(|groups| !groups.is_empty()));
    let first_group = &inspection["readGroups"][0];
    assert!(first_group["fieldCount"].is_u64());
    assert!(first_group.get("selectors").is_none());

    let read = client.call_tool(
        4,
        "loom.readFieldGroup",
        json!({
            "projectRoot": fixture.project_root,
            "requestRef": request_ref,
            "groupId": "conversation_protocol"
        }),
    );
    assert!(
        read["result"]["structuredContent"]["fields"]["clarificationConversationProtocol"]
            .is_object()
    );
    let response_audit = std::fs::read_to_string(
        fixture
            .project_root
            .join(".loom/metrics/mcp-response-audit.jsonl"),
    )
    .expect("MCP response audit");
    assert!(response_audit.contains("\"toolName\":\"plan\""));
    assert!(response_audit.contains("\"toolName\":\"readFieldGroup\""));
    assert!(response_audit.contains("\"serializedBytes\":"));
}

struct TestProject {
    root: PathBuf,
    project_root: PathBuf,
    loom_home: PathBuf,
}

impl TestProject {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "loom-mcp-stdio-{name}-{}-{nonce}",
            std::process::id()
        ));
        let project_root = root.join("project");
        let loom_home = root.join("loom-home");
        std::fs::create_dir_all(&project_root).expect("create project root");
        std::fs::create_dir_all(&loom_home).expect("create Loom home");
        Self {
            root,
            project_root,
            loom_home,
        }
    }
}

impl Drop for TestProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpProcess {
    fn start() -> Self {
        Self::start_with_loom_home(Path::new(""))
    }

    fn start_with_loom_home(loom_home: &Path) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_loom-mcp-server"));
        if !loom_home.as_os_str().is_empty() {
            command.env("LOOM_HOME", loom_home);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn loom-mcp-server");
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn request(&mut self, request: Value) -> Value {
        self.write_message(&request);
        self.read_message()
    }

    fn notify(&mut self, notification: Value) {
        self.write_message(&notification);
    }

    fn initialize(&mut self) {
        let initialized = self.request(json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "loom-test-client", "version": "0.0.1" }
            }
        }));
        assert_eq!(
            initialized["result"]["serverInfo"]["name"],
            "loom-mcp-server"
        );
        self.notify(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
    }

    fn call_tool(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        self.request(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }))
    }

    fn write_message(&mut self, value: &Value) {
        writeln!(self.stdin, "{value}").expect("write message");
        self.stdin.flush().expect("flush message");
    }

    fn read_message(&mut self) -> Value {
        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line).expect("read message");
        assert!(bytes > 0, "server closed stdout before response");
        serde_json::from_str(line.trim()).expect("valid JSON-RPC response")
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
