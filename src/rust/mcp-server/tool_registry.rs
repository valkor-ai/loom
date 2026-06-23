use std::{borrow::Cow, sync::Arc};

use delivery_core::{normalize_project_root, LoomMcpActionResult, ProjectToolInput};
use rmcp::{
    handler::server::common::schema_for_type,
    model::{CallToolResult, JsonObject, Tool},
};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolRegistration {
    pub name: &'static str,
    pub description: &'static str,
    pub target_batch: u32,
}

pub const BATCH_2_TOOLS: &[ToolRegistration] = &[
    ToolRegistration {
        name: "loom.initProject",
        description: "Initialize Loom project state for the current project.",
        target_batch: 4,
    },
    ToolRegistration {
        name: "loom.status",
        description: "Read Loom project status for the current project.",
        target_batch: 4,
    },
    ToolRegistration {
        name: "loom.plan",
        description: "Start or route a Loom delivery plan for a requirement.",
        target_batch: 4,
    },
    ToolRegistration {
        name: "loom.continue",
        description: "Continue the active Loom workflow for the current project.",
        target_batch: 4,
    },
    ToolRegistration {
        name: "loom.readFieldGroup",
        description: "Read a declared request field group.",
        target_batch: 3,
    },
    ToolRegistration {
        name: "loom.readRequestFields",
        description: "Read declared request fields by path.",
        target_batch: 3,
    },
    ToolRegistration {
        name: "loom.recordTaskResultFile",
        description: "Submit a task execution result file.",
        target_batch: 8,
    },
    ToolRegistration {
        name: "loom.repairSubmitFile",
        description: "Submit a repair artifact file.",
        target_batch: 5,
    },
    ToolRegistration {
        name: "loom.knowledgeSemanticSubmitFile",
        description: "Submit a generated knowledge semantic pack result file.",
        target_batch: 6,
    },
];

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: Vec<ToolRegistration>,
}

impl ToolRegistry {
    pub fn batch_2() -> Self {
        Self {
            tools: BATCH_2_TOOLS.to_vec(),
        }
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        let mut tools: Vec<Tool> = self.tools.iter().map(tool_to_mcp).collect();
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        tools
    }

    pub fn get(&self, name: &str) -> Option<ToolRegistration> {
        self.tools.iter().copied().find(|tool| tool.name == name)
    }

    pub fn call_registered_placeholder(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let tool = self
            .get(name)
            .ok_or_else(|| rmcp::ErrorData::invalid_params("tool not found", None))?;
        let project_root = match project_input_from_arguments(arguments) {
            Ok(input) => match normalize_project_root(&input.project_root) {
                Ok(project_root) => project_root.display,
                Err(message) => {
                    let result = LoomMcpActionResult::invalid_project_root(message);
                    return Ok(CallToolResult::structured(to_value(result)?));
                }
            },
            Err(message) => {
                let result = LoomMcpActionResult::invalid_project_root(message);
                return Ok(CallToolResult::structured(to_value(result)?));
            }
        };

        Ok(CallToolResult::structured(to_value(
            LoomMcpActionResult::not_implemented_for_batch(
                project_root,
                tool.name,
                tool.target_batch,
            ),
        )?))
    }
}

fn tool_to_mcp(registration: &ToolRegistration) -> Tool {
    let mut tool = Tool::new(
        Cow::Borrowed(registration.name),
        Cow::Borrowed(registration.description),
        schema_for_type::<ProjectToolInput>(),
    );
    tool.output_schema = Some(Arc::new(schema_json_object::<LoomMcpActionResult>()));
    tool
}

fn schema_json_object<T>() -> JsonObject
where
    T: schemars::JsonSchema,
{
    let schema = schemars::schema_for!(T);
    match serde_json::to_value(schema).expect("schema serializes to JSON") {
        Value::Object(object) => object,
        _ => JsonObject::new(),
    }
}

fn project_input_from_arguments(arguments: Option<JsonObject>) -> Result<ProjectToolInput, String> {
    let Some(arguments) = arguments else {
        return Err("projectRoot is required.".to_string());
    };
    let value = Value::Object(arguments);
    serde_json::from_value(value).map_err(|error| format!("invalid tool input: {error}"))
}

fn to_value(result: LoomMcpActionResult) -> Result<Value, rmcp::ErrorData> {
    serde_json::to_value(result).map_err(|error| {
        rmcp::ErrorData::internal_error(format!("failed to serialize result: {error}"), None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_2_tools_are_registered_in_sorted_order() {
        let tools = ToolRegistry::batch_2().list_tools();
        let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        assert_eq!(
            names,
            vec![
                "loom.continue",
                "loom.initProject",
                "loom.knowledgeSemanticSubmitFile",
                "loom.plan",
                "loom.readFieldGroup",
                "loom.readRequestFields",
                "loom.recordTaskResultFile",
                "loom.repairSubmitFile",
                "loom.status",
            ]
        );
    }

    #[test]
    fn project_tool_schema_does_not_expose_host() {
        let tool = ToolRegistry::batch_2()
            .list_tools()
            .into_iter()
            .find(|tool| tool.name == "loom.status")
            .expect("loom.status tool");
        let schema = serde_json::to_value(tool.input_schema).expect("schema json");
        assert!(schema.to_string().contains("projectRoot"));
        assert!(!schema.to_string().contains("host"));
    }
}
