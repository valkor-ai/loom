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
            "loom.architectureSectionSubmitFile",
            "loom.brainstormAcceptFile",
            "loom.continue",
            "loom.deployBootstrap",
            "loom.deployDown",
            "loom.deployInspect",
            "loom.deployLogs",
            "loom.deployPrepare",
            "loom.deployRepair",
            "loom.deployRun",
            "loom.deployStatus",
            "loom.deployUp",
            "loom.deployValidate",
            "loom.initProject",
            "loom.inspectRequest",
            "loom.knowledgeAdd",
            "loom.knowledgeBrainstormContext",
            "loom.knowledgeBuild",
            "loom.knowledgeDisable",
            "loom.knowledgeDiscard",
            "loom.knowledgeEnable",
            "loom.knowledgeInspectChunk",
            "loom.knowledgeList",
            "loom.knowledgePending",
            "loom.knowledgeRemove",
            "loom.knowledgeResume",
            "loom.knowledgeSearch",
            "loom.knowledgeSemanticSubmitFile",
            "loom.knowledgeStatus",
            "loom.knowledgeUpdate",
            "loom.plan",
            "loom.readFieldGroup",
            "loom.readRequestFields",
            "loom.recordTaskResultFile",
            "loom.repairSubmitFile",
            "loom.repositoryContextAcceptFile",
            "loom.reviewAcceptFile",
            "loom.reviewResolveFile",
            "loom.status",
            "loom.taskPlanAcceptFile",
            "loom.technicalBaselineAcceptFile",
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
fn submit_tools_use_file_submit_input_without_legacy_cli_paths() {
    let tools = ToolRegistry::batch_2().list_tools();
    for name in [
        "loom.brainstormAcceptFile",
        "loom.technicalBaselineAcceptFile",
        "loom.repositoryContextAcceptFile",
        "loom.architectureSectionSubmitFile",
        "loom.taskPlanAcceptFile",
        "loom.recordTaskResultFile",
        "loom.reviewAcceptFile",
        "loom.reviewResolveFile",
        "loom.repairSubmitFile",
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
        .find(|tool| tool.name.as_ref() == "loom.knowledgeSemanticSubmitFile")
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
