use std::{borrow::Cow, sync::Arc};

use delivery_core::{
    normalize_project_root, InspectRequestInput, InspectRequestResult, LoomMcpActionResult,
    ProjectToolInput, ReadFieldGroupInput, ReadFieldGroupResult, ReadRequestFieldsInput,
    ReadRequestFieldsResult,
};
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
    pub input_kind: ToolInputKind,
    pub output_kind: ToolOutputKind,
    pub implemented: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolInputKind {
    Project,
    InspectRequest,
    ReadFieldGroup,
    ReadRequestFields,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOutputKind {
    ActionResult,
    InspectRequest,
    ReadFieldGroup,
    ReadRequestFields,
}

pub const BATCH_2_TOOLS: &[ToolRegistration] = &[
    ToolRegistration {
        name: "loom.initProject",
        description: "Initialize Loom project state for the current project.",
        target_batch: 4,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: false,
    },
    ToolRegistration {
        name: "loom.status",
        description: "Read Loom project status for the current project.",
        target_batch: 4,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: false,
    },
    ToolRegistration {
        name: "loom.plan",
        description: "Start or route a Loom delivery plan for a requirement.",
        target_batch: 4,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: false,
    },
    ToolRegistration {
        name: "loom.continue",
        description: "Continue the active Loom workflow for the current project.",
        target_batch: 4,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: false,
    },
    ToolRegistration {
        name: "loom.inspectRequest",
        description:
            "Inspect request metadata and declared read groups without returning the full request.",
        target_batch: 3,
        input_kind: ToolInputKind::InspectRequest,
        output_kind: ToolOutputKind::InspectRequest,
        implemented: true,
    },
    ToolRegistration {
        name: "loom.readFieldGroup",
        description: "Read a declared request field group.",
        target_batch: 3,
        input_kind: ToolInputKind::ReadFieldGroup,
        output_kind: ToolOutputKind::ReadFieldGroup,
        implemented: true,
    },
    ToolRegistration {
        name: "loom.readRequestFields",
        description: "Read declared request fields by path.",
        target_batch: 3,
        input_kind: ToolInputKind::ReadRequestFields,
        output_kind: ToolOutputKind::ReadRequestFields,
        implemented: true,
    },
    ToolRegistration {
        name: "loom.recordTaskResultFile",
        description: "Submit a task execution result file.",
        target_batch: 8,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: false,
    },
    ToolRegistration {
        name: "loom.repairSubmitFile",
        description: "Submit a repair artifact file.",
        target_batch: 5,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: false,
    },
    ToolRegistration {
        name: "loom.knowledgeSemanticSubmitFile",
        description: "Submit a generated knowledge semantic pack result file.",
        target_batch: 6,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: false,
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
        input_schema(registration.input_kind),
    );
    tool.output_schema = Some(Arc::new(output_schema(registration.output_kind)));
    tool
}

fn input_schema(kind: ToolInputKind) -> Arc<JsonObject> {
    match kind {
        ToolInputKind::Project => schema_for_type::<ProjectToolInput>(),
        ToolInputKind::InspectRequest => schema_for_type::<InspectRequestInput>(),
        ToolInputKind::ReadFieldGroup => schema_for_type::<ReadFieldGroupInput>(),
        ToolInputKind::ReadRequestFields => schema_for_type::<ReadRequestFieldsInput>(),
    }
}

fn output_schema(kind: ToolOutputKind) -> JsonObject {
    match kind {
        ToolOutputKind::ActionResult => schema_json_object::<LoomMcpActionResult>(),
        ToolOutputKind::InspectRequest => schema_json_object::<InspectRequestResult>(),
        ToolOutputKind::ReadFieldGroup => schema_json_object::<ReadFieldGroupResult>(),
        ToolOutputKind::ReadRequestFields => schema_json_object::<ReadRequestFieldsResult>(),
    }
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
