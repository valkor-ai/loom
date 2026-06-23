use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
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
    assert!(tool_names.contains(&"loom.status"));
    assert!(tool_names.contains(&"loom.readFieldGroup"));

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

struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpProcess {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_loom-mcp-server"))
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
