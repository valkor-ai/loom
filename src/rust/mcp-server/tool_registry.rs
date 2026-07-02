use std::{borrow::Cow, sync::Arc};

use brainstorm::BrainstormConfirmBlockInput;
use delivery_core::{
    normalize_project_root, FileSubmitInput, InspectRequestInput, InspectRequestResult,
    LoomMcpActionResult, PlanToolInput, ProjectToolInput, ReadFieldGroupInput,
    ReadFieldGroupResult,
};
use deploy::{DeployBootstrapInput, DeployToolInput};
use knowledge::mcp_models::{
    KnowledgeAddInput, KnowledgeBrainstormContextInput, KnowledgeInspectChunkInput,
    KnowledgeNameInput, KnowledgePendingInput, KnowledgeProjectInput, KnowledgeSearchInput,
    KnowledgeSemanticSubmitInput, KnowledgeUpdateInput,
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
    Plan,
    FileSubmit,
    BrainstormConfirmBlock,
    KnowledgeAdd,
    KnowledgeUpdate,
    KnowledgeName,
    KnowledgePending,
    KnowledgeProject,
    KnowledgeSearch,
    KnowledgeBrainstormContext,
    KnowledgeInspectChunk,
    KnowledgeSemanticSubmit,
    Deploy,
    DeployBootstrap,
    InspectRequest,
    ReadFieldGroup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOutputKind {
    ActionResult,
    InspectRequest,
    ReadFieldGroup,
}

pub const BATCH_2_TOOLS: &[ToolRegistration] = &[
    ToolRegistration {
        name: "initProject",
        description: "Initialize Loom project state for the current project.",
        target_batch: 4,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "status",
        description: "Read Loom project status for the current project.",
        target_batch: 4,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "plan",
        description: "Start or route a Loom delivery plan for a requirement.",
        target_batch: 4,
        input_kind: ToolInputKind::Plan,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "continue",
        description: "Continue the active Loom workflow for the current project.",
        target_batch: 4,
        input_kind: ToolInputKind::Project,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "inspectRequest",
        description:
            "Inspect request metadata and declared read groups without returning the full request.",
        target_batch: 3,
        input_kind: ToolInputKind::InspectRequest,
        output_kind: ToolOutputKind::InspectRequest,
        implemented: true,
    },
    ToolRegistration {
        name: "readFieldGroup",
        description: "Read a declared request field group.",
        target_batch: 3,
        input_kind: ToolInputKind::ReadFieldGroup,
        output_kind: ToolOutputKind::ReadFieldGroup,
        implemented: true,
    },
    ToolRegistration {
        name: "brainstormConfirmBlock",
        description:
            "Record a user-confirmed Brainstorm block and return the next Brainstorm action.",
        target_batch: 7,
        input_kind: ToolInputKind::BrainstormConfirmBlock,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "brainstormAcceptFile",
        description: "Submit a Brainstorm candidate file.",
        target_batch: 7,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "technicalBaselineAcceptFile",
        description: "Submit a technical baseline candidate file.",
        target_batch: 8,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "repositoryContextAcceptFile",
        description: "Submit a repository context candidate file.",
        target_batch: 8,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "architectureSectionSubmitFile",
        description: "Submit an architecture artifact section file.",
        target_batch: 8,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "taskPlanAcceptFile",
        description: "Submit a task plan candidate file.",
        target_batch: 8,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "recordTaskResultFile",
        description: "Submit a task execution result file.",
        target_batch: 8,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "reviewAcceptFile",
        description: "Submit a review result file.",
        target_batch: 9,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "reviewResolveFile",
        description: "Submit a manual review resolution file.",
        target_batch: 9,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "repairSubmitFile",
        description: "Submit a repair artifact file.",
        target_batch: 5,
        input_kind: ToolInputKind::FileSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeAdd",
        description: "Register a knowledge source and write pending source paths.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeAdd,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeUpdate",
        description: "Add, remove, or replace pending paths for a knowledge source.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeUpdate,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgePending",
        description: "List knowledge sources with pending operations.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgePending,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeDiscard",
        description: "Discard pending operations for a knowledge source.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeName,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeBuild",
        description: "Build a knowledge source and start semantic pack generation.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeName,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeResume",
        description: "Resume a pending semantic build for a knowledge source.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeName,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeList",
        description: "List registered knowledge sources.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeProject,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeStatus",
        description: "Read status for a knowledge source.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeName,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeRemove",
        description: "Remove a knowledge source without deleting original documents.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeName,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeEnable",
        description: "Enable a knowledge source for search and Brainstorm context.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeName,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeDisable",
        description: "Disable a knowledge source for search and Brainstorm context.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeName,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeSearch",
        description: "Search enabled published knowledge sources and return chunk cards.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeSearch,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeBrainstormContext",
        description: "Build request-scoped Brainstorm knowledge context.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeBrainstormContext,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeInspectChunk",
        description: "Read a knowledge chunk body.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeInspectChunk,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "knowledgeSemanticSubmitFile",
        description: "Submit a generated knowledge semantic pack result file.",
        target_batch: 6,
        input_kind: ToolInputKind::KnowledgeSemanticSubmit,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployPrepare",
        description: "Prepare generated deployment assets from RuntimeDeliveryContract.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployRun",
        description: "Prepare, start, validate, and report local deployment state.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployUp",
        description: "Start an already prepared deployment and validate preview/API routes.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployStatus",
        description: "Read local deployment status or active deploy operation.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployInspect",
        description:
            "Inspect deployment spec, source model, topology, generated files, and repair state.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployValidate",
        description: "Validate generated deployment assets plus preview and public API paths.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployLogs",
        description: "Read deployment log tail and full log ref without inlining full logs.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployBootstrap",
        description: "Confirm and run deployment bootstrap tasks when required.",
        target_batch: 10,
        input_kind: ToolInputKind::DeployBootstrap,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployDown",
        description: "Stop the local deployment.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
    },
    ToolRegistration {
        name: "deployRepair",
        description: "Return the current deployment repair next action.",
        target_batch: 10,
        input_kind: ToolInputKind::Deploy,
        output_kind: ToolOutputKind::ActionResult,
        implemented: true,
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
        let project_root = match project_root_from_arguments(arguments, tool.input_kind) {
            Ok(project_root) => match normalize_project_root(&project_root) {
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
    normalize_schema_arc(match kind {
        ToolInputKind::Project => schema_for_type::<ProjectToolInput>(),
        ToolInputKind::Plan => schema_for_type::<PlanToolInput>(),
        ToolInputKind::FileSubmit => schema_for_type::<FileSubmitInput>(),
        ToolInputKind::BrainstormConfirmBlock => schema_for_type::<BrainstormConfirmBlockInput>(),
        ToolInputKind::KnowledgeAdd => schema_for_type::<KnowledgeAddInput>(),
        ToolInputKind::KnowledgeUpdate => schema_for_type::<KnowledgeUpdateInput>(),
        ToolInputKind::KnowledgeName => schema_for_type::<KnowledgeNameInput>(),
        ToolInputKind::KnowledgePending => schema_for_type::<KnowledgePendingInput>(),
        ToolInputKind::KnowledgeProject => schema_for_type::<KnowledgeProjectInput>(),
        ToolInputKind::KnowledgeSearch => schema_for_type::<KnowledgeSearchInput>(),
        ToolInputKind::KnowledgeBrainstormContext => {
            schema_for_type::<KnowledgeBrainstormContextInput>()
        }
        ToolInputKind::KnowledgeInspectChunk => schema_for_type::<KnowledgeInspectChunkInput>(),
        ToolInputKind::KnowledgeSemanticSubmit => schema_for_type::<KnowledgeSemanticSubmitInput>(),
        ToolInputKind::Deploy => schema_for_type::<DeployToolInput>(),
        ToolInputKind::DeployBootstrap => schema_for_type::<DeployBootstrapInput>(),
        ToolInputKind::InspectRequest => schema_for_type::<InspectRequestInput>(),
        ToolInputKind::ReadFieldGroup => schema_for_type::<ReadFieldGroupInput>(),
    })
}

fn output_schema(kind: ToolOutputKind) -> JsonObject {
    match kind {
        ToolOutputKind::ActionResult => schema_json_object::<LoomMcpActionResult>(),
        ToolOutputKind::InspectRequest => schema_json_object::<InspectRequestResult>(),
        ToolOutputKind::ReadFieldGroup => schema_json_object::<ReadFieldGroupResult>(),
    }
}

fn schema_json_object<T>() -> JsonObject
where
    T: schemars::JsonSchema,
{
    let schema = schemars::schema_for!(T);
    match serde_json::to_value(schema).expect("schema serializes to JSON") {
        Value::Object(mut object) => {
            normalize_schema_object(&mut object);
            object
                .entry("type".to_string())
                .or_insert_with(|| Value::String("object".to_string()));
            object
        }
        _ => JsonObject::new(),
    }
}

fn normalize_schema_arc(schema: Arc<JsonObject>) -> Arc<JsonObject> {
    let mut object = (*schema).clone();
    normalize_schema_object(&mut object);
    Arc::new(object)
}

fn normalize_schema_object(object: &mut JsonObject) {
    let mut value = Value::Object(std::mem::take(object));
    normalize_schema_keywords(&mut value);
    if let Value::Object(normalized) = value {
        *object = normalized;
    }
}

fn normalize_schema_node(value: &mut Value) {
    match value {
        Value::Bool(true) => {
            *value = Value::Object(JsonObject::new());
        }
        Value::Bool(false) => {
            let mut object = JsonObject::new();
            object.insert("not".to_string(), Value::Object(JsonObject::new()));
            *value = Value::Object(object);
        }
        _ => normalize_schema_keywords(value),
    }
}

fn normalize_schema_keywords(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object
                .get("format")
                .and_then(Value::as_str)
                .is_some_and(is_rust_integer_format)
            {
                object.remove("format");
            }
            for (key, child) in object.iter_mut() {
                match key.as_str() {
                    "$defs" | "definitions" | "properties" | "patternProperties"
                    | "dependentSchemas" => {
                        if let Value::Object(children) = child {
                            for nested in children.values_mut() {
                                normalize_schema_node(nested);
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
                        normalize_schema_node(child);
                    }
                    "oneOf" | "anyOf" | "allOf" | "prefixItems" => {
                        if let Value::Array(items) = child {
                            for item in items {
                                normalize_schema_node(item);
                            }
                        }
                    }
                    _ => {
                        if child.is_object() || child.is_array() {
                            normalize_schema_keywords(child);
                        }
                    }
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_schema_keywords(item);
            }
        }
        _ => {}
    }
}

fn is_rust_integer_format(format: &str) -> bool {
    matches!(
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
    )
}

fn project_root_from_arguments(
    arguments: Option<JsonObject>,
    _kind: ToolInputKind,
) -> Result<String, String> {
    let Some(arguments) = arguments else {
        return Err("projectRoot is required.".to_string());
    };
    arguments
        .get("projectRoot")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "projectRoot is required.".to_string())
}

fn to_value(result: LoomMcpActionResult) -> Result<Value, rmcp::ErrorData> {
    serde_json::to_value(result).map_err(|error| {
        rmcp::ErrorData::internal_error(format!("failed to serialize result: {error}"), None)
    })
}
