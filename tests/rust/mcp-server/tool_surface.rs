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

const FORBIDDEN_SUBMIT_INPUT_KEYS: &[&str] = &[
    "candidateFile",
    "resultFile",
    "requestId",
    "repairId",
    "section",
    "groupId",
    "path",
    "submitCommand",
    "argv",
    "commandInvocation",
    "run_cli",
    "next-task",
];

#[test]
fn batch_2_tool_surface_is_registered_without_cli_fields() {
    let tools = ToolRegistry::batch_2().list_tools();
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(
        names,
        vec![
            "architectureSectionSubmitFile",
            "brainstormAcceptFile",
            "brainstormConfirmBlock",
            "continue",
            "deployBootstrap",
            "deployDown",
            "deployInspect",
            "deployLogs",
            "deployPrepare",
            "deployRepair",
            "deployRun",
            "deployStatus",
            "deployUp",
            "deployValidate",
            "initProject",
            "inspectRequest",
            "knowledgeAdd",
            "knowledgeBrainstormContext",
            "knowledgeBuild",
            "knowledgeDisable",
            "knowledgeDiscard",
            "knowledgeEnable",
            "knowledgeInspectChunk",
            "knowledgeList",
            "knowledgePending",
            "knowledgeRemove",
            "knowledgeResume",
            "knowledgeSearch",
            "knowledgeSemanticSubmitFile",
            "knowledgeStatus",
            "knowledgeUpdate",
            "plan",
            "readFieldGroup",
            "readRequestFields",
            "recordTaskResultFile",
            "repairSubmitFile",
            "repositoryContextAcceptFile",
            "reviewAcceptFile",
            "reviewResolveFile",
            "status",
            "taskPlanAcceptFile",
            "technicalBaselineAcceptFile",
        ]
    );

    for tool in tools {
        let value = serde_json::to_value(&tool).expect("tool json");
        assert_no_forbidden_keys(&value);
        assert!(value.get("inputSchema").is_some());
        assert!(value.get("outputSchema").is_some());
        assert_eq!(
            value["outputSchema"]["type"].as_str(),
            Some("object"),
            "{} outputSchema must be a top-level object schema: {}",
            tool.name,
            value["outputSchema"]
        );
        assert_no_boolean_schema_nodes(
            &value["inputSchema"],
            &format!("{}.inputSchema", tool.name),
        );
        assert_no_boolean_schema_nodes(
            &value["outputSchema"],
            &format!("{}.outputSchema", tool.name),
        );
        assert_no_unsupported_integer_formats(
            &value["inputSchema"],
            &format!("{}.inputSchema", tool.name),
        );
        assert_no_unsupported_integer_formats(
            &value["outputSchema"],
            &format!("{}.outputSchema", tool.name),
        );
        assert!(value.to_string().contains("projectRoot"));
        assert!(!value.to_string().contains("host"));
    }
}

#[test]
fn submit_tools_use_file_submit_input_without_legacy_cli_paths() {
    let tools = ToolRegistry::batch_2().list_tools();
    for name in [
        "brainstormAcceptFile",
        "technicalBaselineAcceptFile",
        "repositoryContextAcceptFile",
        "architectureSectionSubmitFile",
        "taskPlanAcceptFile",
        "recordTaskResultFile",
        "reviewAcceptFile",
        "reviewResolveFile",
        "repairSubmitFile",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool.name.as_ref() == name)
            .unwrap_or_else(|| panic!("missing submit tool {name}"));
        let value = serde_json::to_value(tool).expect("tool json");
        let schema_text = value["inputSchema"].to_string();
        assert!(schema_text.contains("projectRoot"));
        assert!(schema_text.contains("requestRef"));
        assert!(schema_text.contains("writtenTargetIds"));
        for forbidden in FORBIDDEN_SUBMIT_INPUT_KEYS {
            assert!(
                !schema_text.contains(forbidden),
                "{name} input schema must not expose {forbidden}: {schema_text}"
            );
        }
    }

    let knowledge_submit = tools
        .iter()
        .find(|tool| tool.name.as_ref() == "knowledgeSemanticSubmitFile")
        .expect("knowledge semantic submit tool");
    let knowledge_schema =
        serde_json::to_value(knowledge_submit).expect("tool json")["inputSchema"].to_string();
    assert!(knowledge_schema.contains("projectRoot"));
    assert!(knowledge_schema.contains("requestRef"));
    assert!(!knowledge_schema.contains("writtenTargetIds"));
    for forbidden in FORBIDDEN_SUBMIT_INPUT_KEYS {
        assert!(
            !knowledge_schema.contains(forbidden),
            "knowledge semantic submit input schema must not expose {forbidden}: {knowledge_schema}"
        );
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
        .call_registered_placeholder("readFieldGroup", Some(arguments))
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

fn assert_no_boolean_schema_nodes(value: &Value, path: &str) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                match key.as_str() {
                    "$defs" | "definitions" | "properties" | "patternProperties"
                    | "dependentSchemas" => {
                        if let Value::Object(children) = child {
                            for (child_key, nested) in children {
                                assert_schema_node_is_not_boolean(
                                    nested,
                                    &format!("{path}.{key}.{child_key}"),
                                );
                            }
                        }
                    }
                    "items"
                    | "additionalProperties"
                    | "unevaluatedProperties"
                    | "contains"
                    | "propertyNames"
                    | "not"
                    | "if"
                    | "then"
                    | "else" => {
                        assert_schema_node_is_not_boolean(child, &format!("{path}.{key}"));
                    }
                    "oneOf" | "anyOf" | "allOf" | "prefixItems" => {
                        if let Value::Array(items) = child {
                            for (index, item) in items.iter().enumerate() {
                                assert_schema_node_is_not_boolean(
                                    item,
                                    &format!("{path}.{key}[{index}]"),
                                );
                            }
                        }
                    }
                    _ => {
                        if child.is_object() || child.is_array() {
                            assert_no_boolean_schema_nodes(child, &format!("{path}.{key}"));
                        }
                    }
                }
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                assert_no_boolean_schema_nodes(item, &format!("{path}[{index}]"));
            }
        }
        _ => {}
    }
}

fn assert_schema_node_is_not_boolean(value: &Value, path: &str) {
    assert!(
        !value.is_boolean(),
        "{path} must be an object schema, got {value}"
    );
    assert_no_boolean_schema_nodes(value, path);
}

fn assert_no_unsupported_integer_formats(value: &Value, path: &str) {
    match value {
        Value::Object(object) => {
            if let Some(format) = object.get("format").and_then(Value::as_str) {
                assert!(
                    !matches!(
                        format,
                        "int8"
                            | "int16"
                            | "int32"
                            | "int64"
                            | "int128"
                            | "uint8"
                            | "uint16"
                            | "uint32"
                            | "uint64"
                            | "uint128"
                            | "usize"
                            | "isize"
                    ),
                    "{path} must not expose Rust integer format {format}"
                );
            }
            for (key, child) in object {
                assert_no_unsupported_integer_formats(child, &format!("{path}.{key}"));
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                assert_no_unsupported_integer_formats(item, &format!("{path}[{index}]"));
            }
        }
        _ => {}
    }
}
