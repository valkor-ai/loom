use mcp_server::{resource_registry::ResourceRegistry, tool_registry::ToolRegistry};
use serde_json::{json, Value};

const FORBIDDEN_KEYS: &[&str] = &[
    "commandInvocation",
    "argv",
    "argvTemplate",
    "submitCommand",
    "retryCommand",
    "command",
    "launcher",
    "env",
    "actionRequired",
    "readCommand",
    "fallbackRule",
];

#[test]
fn batch_2_tool_surface_is_registered_without_cli_fields() {
    let tools = ToolRegistry::batch_2().list_tools();
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(
        names,
        vec![
            "loom.continue",
            "loom.initProject",
            "loom.inspectRequest",
            "loom.knowledgeSemanticSubmitFile",
            "loom.plan",
            "loom.readFieldGroup",
            "loom.readRequestFields",
            "loom.recordTaskResultFile",
            "loom.repairSubmitFile",
            "loom.status",
        ]
    );

    for tool in tools {
        let value = serde_json::to_value(&tool).expect("tool json");
        assert_no_forbidden_keys(&value);
        assert!(value.get("inputSchema").is_some());
        assert!(value.get("outputSchema").is_some());
        assert!(value.to_string().contains("projectRoot"));
        assert!(!value.to_string().contains("host"));
    }
}

#[test]
fn batch_2_resource_templates_are_registered() {
    let templates = ResourceRegistry::batch_2().list_resource_templates();
    let value = serde_json::to_value(&templates).expect("templates json");
    assert_no_forbidden_keys(&value);
    let uris: Vec<&str> = templates
        .resource_templates
        .iter()
        .map(|template| template.uri_template.as_str())
        .collect();
    assert_eq!(
        uris,
        vec![
            "loom://knowledge/{sourceId}/builds/{buildId}/chunks/{chunkId}",
            "loom://projects/{projectId}/requests/{requestId}/field-groups/{groupId}",
            "loom://projects/{projectId}/requests/{requestId}/fields/{encodedFieldPath}",
        ]
    );
}

#[test]
fn registered_placeholder_returns_target_batch() {
    let project_root = std::env::current_dir()
        .expect("current dir")
        .canonicalize()
        .expect("canonical current dir")
        .to_string_lossy()
        .into_owned();
    let arguments = json!({ "projectRoot": project_root })
        .as_object()
        .expect("arguments object")
        .clone();

    let result = ToolRegistry::batch_2()
        .call_registered_placeholder("loom.readFieldGroup", Some(arguments))
        .expect("placeholder result");
    let structured = result
        .structured_content
        .expect("structured placeholder result");
    assert_eq!(structured["state"], "failed");
    assert_eq!(structured["error"]["code"], "not_implemented_for_batch");
    assert_eq!(structured["error"]["targetBatch"], 3);
}

fn assert_no_forbidden_keys(value: &Value) {
    match value {
        Value::Object(object) => {
            for key in object.keys() {
                assert!(
                    !FORBIDDEN_KEYS.contains(&key.as_str()),
                    "forbidden key {key} appears in {value}"
                );
            }
            for child in object.values() {
                assert_no_forbidden_keys(child);
            }
        }
        Value::Array(items) => {
            for item in items {
                assert_no_forbidden_keys(item);
            }
        }
        _ => {}
    }
}
