use std::future::{ready, Future};

use brainstorm::accept_brainstorm_file;
use delivery_core::{
    is_submit_tool, normalize_project_root, status_details, submit_tool_spec, validate_plan_input,
    DomainDispatcher, FileSubmitInput, InspectRequestInput, LoomMcpActionResult, LoomMcpDoneResult,
    LoomMcpFailure, LoomMcpFailureResult, LoomMcpRepairableErrorResult, LoomMcpRuntimeContext,
    OperationContext, PlanToolInput, ProjectToolInput, ReadFieldGroupInput, ReadRequestFieldsInput,
    SubmitAcceptedEvent, TransitionEngine, TransitionStore,
};
use deploy::{DeployBootstrapInput, DeployToolInput};
use knowledge::mcp_models::{
    KnowledgeAddInput, KnowledgeBrainstormContextInput, KnowledgeInspectChunkInput,
    KnowledgeNameInput, KnowledgeProjectInput, KnowledgeSearchInput, KnowledgeSemanticSubmitInput,
    KnowledgeUpdateInput,
};
use planning::{accept_repository_context_file, accept_technical_baseline_file};
use rmcp::{
    model::{
        CallToolRequestMethod, CallToolRequestParams, CallToolResult, Implementation,
        ListResourceTemplatesResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ServerCapabilities,
        ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer},
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde_json::json;
use state::lifecycle_store::{init_project_state, FileTransitionStore};
use workflow::WorkflowDomainDispatcher;

use crate::{resource_registry::ResourceRegistry, tool_registry::ToolRegistry};

#[derive(Debug, Clone)]
pub struct LoomMcpServer {
    runtime: LoomMcpRuntimeContext,
    tools: ToolRegistry,
    resources: ResourceRegistry,
}

impl LoomMcpServer {
    pub fn new(runtime: LoomMcpRuntimeContext) -> Self {
        Self {
            runtime,
            tools: ToolRegistry::batch_2(),
            resources: ResourceRegistry::batch_2(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(LoomMcpRuntimeContext::from_env())
    }

    pub fn runtime(&self) -> &LoomMcpRuntimeContext {
        &self.runtime
    }

    pub fn tool_registry(&self) -> &ToolRegistry {
        &self.tools
    }

    pub fn resource_registry(&self) -> &ResourceRegistry {
        &self.resources
    }

    pub fn invoke_tool(
        &self,
        name: &str,
        arguments: Option<rmcp::model::JsonObject>,
    ) -> Result<CallToolResult, McpError> {
        let request = match arguments {
            Some(arguments) => {
                CallToolRequestParams::new(name.to_string()).with_arguments(arguments)
            }
            None => CallToolRequestParams::new(name.to_string()),
        };
        call_tool(self, request)
    }
}

impl Default for LoomMcpServer {
    fn default() -> Self {
        Self::from_env()
    }
}

impl ServerHandler for LoomMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(Implementation::new("loom-mcp-server", env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Loom MCP server. Use registered Loom tools and resources; do not call legacy CLI commands.",
        )
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        ready(Ok(ListToolsResult::with_all_items(self.tools.list_tools())))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools
            .list_tools()
            .into_iter()
            .find(|tool| tool.name == name)
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        ready(call_tool(self, request))
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        ready(Ok(ListResourcesResult::default()))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_ {
        ready(Ok(self.resources.list_resource_templates()))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, McpError>> + Send + '_ {
        ready(read_resource(self, &request.uri))
    }
}

fn call_tool(
    server: &LoomMcpServer,
    request: CallToolRequestParams,
) -> Result<CallToolResult, McpError> {
    match request.name.as_ref() {
        "loom.initProject" => action_result(init_project_tool(parse_args::<ProjectToolInput>(
            request.arguments,
        )?)),
        "loom.status" => action_result(status_tool(parse_args::<ProjectToolInput>(
            request.arguments,
        )?)),
        "loom.plan" => action_result(plan_tool(parse_args::<PlanToolInput>(request.arguments)?)),
        "loom.continue" => action_result(continue_tool(parse_args::<ProjectToolInput>(
            request.arguments,
        )?)),
        "loom.inspectRequest" => structured(state::inspect_request(parse_args::<
            InspectRequestInput,
        >(request.arguments)?)),
        "loom.readFieldGroup" => structured(state::read_field_group(parse_args::<
            ReadFieldGroupInput,
        >(request.arguments)?)),
        "loom.readRequestFields" => {
            structured(state::read_request_fields(parse_args::<
                ReadRequestFieldsInput,
            >(request.arguments)?))
        }
        "loom.knowledgeAdd" => {
            let input = parse_args::<KnowledgeAddInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source registered.",
                knowledge::add_source(input),
            ))
        }
        "loom.knowledgeUpdate" => {
            let input = parse_args::<KnowledgeUpdateInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source updated.",
                knowledge::update_source(input),
            ))
        }
        "loom.knowledgePending" => {
            let input = parse_args::<KnowledgeProjectInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge pending queue loaded.",
                knowledge::pending_sources(input),
            ))
        }
        "loom.knowledgeDiscard" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge pending operations discarded.",
                knowledge::discard_pending(input),
            ))
        }
        "loom.knowledgeBuild" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_result(
                &project_root,
                knowledge::build_source_from_input(input),
            ))
        }
        "loom.knowledgeResume" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_result(
                &project_root,
                knowledge::resume_source_from_input(input),
            ))
        }
        "loom.knowledgeList" => {
            let input = parse_args::<KnowledgeProjectInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge sources listed.",
                knowledge::list_sources(input),
            ))
        }
        "loom.knowledgeStatus" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source status loaded.",
                knowledge::source_status(input),
            ))
        }
        "loom.knowledgeRemove" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source removed.",
                knowledge::remove_source(input),
            ))
        }
        "loom.knowledgeEnable" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source enabled.",
                knowledge::enable_source(input),
            ))
        }
        "loom.knowledgeDisable" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source disabled.",
                knowledge::disable_source(input),
            ))
        }
        "loom.knowledgeSearch" => {
            let input = parse_args::<KnowledgeSearchInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge search completed.",
                knowledge::search_knowledge(input),
            ))
        }
        "loom.knowledgeBrainstormContext" => {
            let input = parse_args::<KnowledgeBrainstormContextInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge Brainstorm context prepared.",
                knowledge::brainstorm_context(input),
            ))
        }
        "loom.knowledgeInspectChunk" => {
            let input = parse_args::<KnowledgeInspectChunkInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge chunk inspected.",
                knowledge::inspect_chunk(input),
            ))
        }
        "loom.knowledgeSemanticSubmitFile" => {
            let input = parse_args::<KnowledgeSemanticSubmitInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_result(
                &project_root,
                knowledge::submit_semantic_pack_from_input(input),
            ))
        }
        "loom.deployPrepare" => action_result(deploy::deploy_prepare(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "loom.deployRun" => {
            action_result(deploy::deploy_run(normalize_deploy_input(parse_args::<
                DeployToolInput,
            >(
                request.arguments,
            )?)?))
        }
        "loom.deployUp" => action_result(deploy::deploy_up(normalize_deploy_input(parse_args::<
            DeployToolInput,
        >(
            request.arguments,
        )?)?)),
        "loom.deployStatus" => action_result(deploy::deploy_status(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "loom.deployInspect" => action_result(deploy::deploy_inspect(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "loom.deployValidate" => action_result(deploy::deploy_validate(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "loom.deployLogs" => {
            action_result(deploy::deploy_logs(normalize_deploy_input(parse_args::<
                DeployToolInput,
            >(
                request.arguments,
            )?)?))
        }
        "loom.deployBootstrap" => {
            action_result(deploy::deploy_bootstrap(normalize_deploy_bootstrap_input(
                parse_args::<DeployBootstrapInput>(request.arguments)?,
            )?))
        }
        "loom.deployDown" => {
            action_result(deploy::deploy_down(normalize_deploy_input(parse_args::<
                DeployToolInput,
            >(
                request.arguments,
            )?)?))
        }
        "loom.deployRepair" => action_result(deploy::deploy_repair(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        name if is_submit_tool(name) => action_result(submit_file_tool(
            name,
            parse_args::<FileSubmitInput>(request.arguments)?,
        )),
        _ => server
            .tools
            .call_registered_placeholder(&request.name, request.arguments)
            .map_err(|_| McpError::method_not_found::<CallToolRequestMethod>()),
    }
}

fn init_project_tool(input: ProjectToolInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    match init_project_state(&normalized.display) {
        Ok(result) => LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: normalized.display,
            summary: "Loom project initialized.".to_string(),
            details: Some(json!({
                "initialized": true,
                "created": result.created,
                "alreadyExisted": result.already_existed,
            })),
            warnings: vec![],
        }),
        Err(error) => state_failure(normalized.display, error.to_string()),
    }
}

fn status_tool(input: ProjectToolInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    let store = FileTransitionStore;
    let status = match store.load_status(&normalized.display) {
        Ok(status) => status,
        Err(error) if error.code() == "STATE_NOT_INITIALIZED" => {
            return LoomMcpActionResult::Failed(LoomMcpFailureResult {
                project_root: normalized.display,
                error: LoomMcpFailure {
                    code: "STATE_NOT_INITIALIZED".to_string(),
                    message: error.message().to_string(),
                    target_batch: None,
                    domain: Some("project_lifecycle".to_string()),
                    route_action: None,
                    recovery_tool: Some("loom.initProject".to_string()),
                },
            });
        }
        Err(error) => {
            return LoomMcpActionResult::Failed(LoomMcpFailureResult {
                project_root: normalized.display,
                error: LoomMcpFailure {
                    code: error.code().to_string(),
                    message: error.message().to_string(),
                    target_batch: None,
                    domain: Some("project_lifecycle".to_string()),
                    route_action: None,
                    recovery_tool: None,
                },
            });
        }
    };
    let active_delivery = status
        .active_delivery_id
        .as_ref()
        .map(|delivery_id| store.load_delivery_index(&normalized.display, delivery_id))
        .transpose()
        .ok()
        .flatten();
    let active_operation = status
        .active_delivery_id
        .as_ref()
        .map(|delivery_id| store.read_operation_lease(&normalized.display, delivery_id))
        .transpose()
        .ok()
        .flatten()
        .flatten();

    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: normalized.display,
        summary: "Loom project status read.".to_string(),
        details: Some(status_details(
            &status,
            active_delivery.as_ref(),
            active_operation.as_ref(),
            &[],
        )),
        warnings: vec![],
    })
}

fn plan_tool(input: PlanToolInput) -> LoomMcpActionResult {
    let validated = match validate_plan_input(input) {
        Ok(validated) => validated,
        Err(message) => {
            return LoomMcpActionResult::Failed(LoomMcpFailureResult {
                project_root: String::new(),
                error: LoomMcpFailure {
                    code: "INVALID_ARGUMENT".to_string(),
                    message,
                    target_batch: None,
                    domain: Some("project_lifecycle".to_string()),
                    route_action: None,
                    recovery_tool: None,
                },
            });
        }
    };
    if let Err(error) = init_project_state(&validated.project_root) {
        return state_failure(validated.project_root, error.to_string());
    }
    WorkflowDomainDispatcher.start_brainstorm(&validated)
}

fn continue_tool(input: ProjectToolInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    let engine = TransitionEngine {
        store: FileTransitionStore,
        dispatcher: WorkflowDomainDispatcher,
    };
    match engine.continue_current(OperationContext {
        project_root: normalized.display.clone(),
    }) {
        Ok(result) => result,
        Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: normalized.display,
            error: LoomMcpFailure {
                code: error.code().to_string(),
                message: error.message().to_string(),
                target_batch: None,
                domain: Some("transition".to_string()),
                route_action: None,
                recovery_tool: None,
            },
        }),
    }
}

fn submit_file_tool(tool_name: &str, input: FileSubmitInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    let normalized_input = FileSubmitInput {
        project_root: normalized.display.clone(),
        request_ref: input.request_ref,
        written_target_ids: input.written_target_ids,
    };
    let authorized = match state::authorize_write_targets(&normalized_input, tool_name) {
        Ok(authorized) => authorized,
        Err(state::WriteTargetAuthorizationError::Repairable {
            target_file,
            target_ids,
            issues,
            read_groups,
            resubmit_tool,
        }) => {
            return LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
                project_root: normalized.display,
                target_file,
                target_ids,
                issues,
                resubmit_tool,
                fix_scope: Some(
                    "Edit only the authorized artifact JSON target, then resubmit with the same Loom MCP submit tool."
                        .to_string(),
                ),
                read_groups,
            });
        }
        Err(state::WriteTargetAuthorizationError::Fatal { code, message }) => {
            return LoomMcpActionResult::Failed(LoomMcpFailureResult {
                project_root: normalized.display,
                error: LoomMcpFailure {
                    code: code.to_string(),
                    message,
                    target_batch: None,
                    domain: Some("submit".to_string()),
                    route_action: None,
                    recovery_tool: None,
                },
            });
        }
    };

    match tool_name {
        "loom.brainstormAcceptFile" => {
            return accept_brainstorm_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.technicalBaselineAcceptFile" => {
            return accept_technical_baseline_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.repositoryContextAcceptFile" => {
            return accept_repository_context_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.architectureSectionSubmitFile" => {
            return architecture::accept_architecture_section_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.taskPlanAcceptFile" => {
            return execution::accept_task_plan_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.recordTaskResultFile" => {
            return execution::accept_task_result_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.reviewAcceptFile" => {
            return execution::accept_review_result_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.reviewResolveFile" => {
            return execution::accept_manual_review_resolution_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        "loom.repairSubmitFile" => {
            if authorized.artifact_kind == delivery_core::ArtifactKind::ArchitectureArtifactRepair {
                return architecture::accept_architecture_repair_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                );
            }
            if authorized.artifact_kind == delivery_core::ArtifactKind::DeployExecutionRepairResult
            {
                return deploy::accept_deploy_execution_repair_file(&normalized_input, &authorized);
            }
            return execution::accept_repair_file(
                &normalized_input,
                &authorized,
                WorkflowDomainDispatcher,
            );
        }
        _ => {}
    }

    if let (Some(delivery_id), Some(phase_id), Some(next_action)) = (
        authorized.delivery_id.clone(),
        authorized.phase_id.clone(),
        authorized.next_action.clone(),
    ) {
        let engine = TransitionEngine {
            store: FileTransitionStore,
            dispatcher: WorkflowDomainDispatcher,
        };
        return match engine.advance_after_submit(
            OperationContext {
                project_root: normalized.display.clone(),
            },
            SubmitAcceptedEvent {
                delivery_id,
                phase_id,
                source_tool: tool_name.to_string(),
                accepted_artifact_ref: format!(
                    "{}/targets/{}",
                    authorized.request_ref,
                    authorized
                        .targets
                        .first()
                        .map(|target| target.target_id.as_str())
                        .unwrap_or("artifact")
                ),
                next_action: Some(next_action),
            },
        ) {
            Ok(result) => result,
            Err(error) => LoomMcpActionResult::Failed(LoomMcpFailureResult {
                project_root: normalized.display,
                error: LoomMcpFailure {
                    code: error.code().to_string(),
                    message: error.message().to_string(),
                    target_batch: None,
                    domain: Some("transition".to_string()),
                    route_action: None,
                    recovery_tool: None,
                },
            }),
        };
    }

    let target_batch = submit_tool_spec(tool_name)
        .map(|spec| spec.target_batch)
        .unwrap_or(5);
    let summary = authorized.summary();
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root: normalized.display,
        error: LoomMcpFailure {
            code: "not_implemented_for_batch".to_string(),
            message: format!(
                "{tool_name} passed MCP native submit preflight for {:?} targets {:?}, but its domain accept handler is assigned to batch {target_batch}.",
                summary.artifact_kind, summary.target_ids
            ),
            target_batch: Some(target_batch),
            domain: Some("submit".to_string()),
            route_action: None,
            recovery_tool: None,
        },
    })
}

fn normalize_deploy_input(mut input: DeployToolInput) -> Result<DeployToolInput, McpError> {
    let normalized = normalize_project_root(&input.project_root)
        .map_err(|message| McpError::invalid_params(message, None))?;
    input.project_root = normalized.display;
    Ok(input)
}

fn normalize_deploy_bootstrap_input(
    mut input: DeployBootstrapInput,
) -> Result<DeployBootstrapInput, McpError> {
    let normalized = normalize_project_root(&input.project_root)
        .map_err(|message| McpError::invalid_params(message, None))?;
    input.project_root = normalized.display;
    Ok(input)
}

fn read_resource(server: &LoomMcpServer, uri: &str) -> Result<ReadResourceResult, McpError> {
    if uri.contains("/field-groups/") {
        let result =
            state::request_resolver::read_field_group_by_resource_uri(uri).map_err(state_error)?;
        return json_resource(uri, &result);
    }
    if uri.contains("/fields/") {
        let result =
            state::request_resolver::read_field_by_resource_uri(uri).map_err(state_error)?;
        return json_resource(uri, &result);
    }
    Ok(server.resources.read_placeholder(uri))
}

fn parse_args<T>(arguments: Option<rmcp::model::JsonObject>) -> Result<T, McpError>
where
    T: serde::de::DeserializeOwned,
{
    let Some(arguments) = arguments else {
        return Err(McpError::invalid_params(
            "tool arguments are required",
            None,
        ));
    };
    serde_json::from_value(serde_json::Value::Object(arguments))
        .map_err(|error| McpError::invalid_params(format!("invalid tool arguments: {error}"), None))
}

fn structured<T>(result: Result<T, state::store::StateError>) -> Result<CallToolResult, McpError>
where
    T: serde::Serialize,
{
    let value = result.map_err(state_error)?;
    let value = serde_json::to_value(value).map_err(|error| {
        McpError::internal_error(format!("failed to serialize result: {error}"), None)
    })?;
    Ok(CallToolResult::structured(value))
}

fn action_result(result: LoomMcpActionResult) -> Result<CallToolResult, McpError> {
    let value = serde_json::to_value(result).map_err(|error| {
        McpError::internal_error(format!("failed to serialize action result: {error}"), None)
    })?;
    Ok(CallToolResult::structured(value))
}

fn json_resource(uri: &str, value: &impl serde::Serialize) -> Result<ReadResourceResult, McpError> {
    let text = serde_json::to_string(value).map_err(|error| {
        McpError::internal_error(format!("failed to serialize resource: {error}"), None)
    })?;
    Ok(ReadResourceResult::new(vec![ResourceContents::text(
        text, uri,
    )
    .with_mime_type("application/json")]))
}

fn state_error(error: state::store::StateError) -> McpError {
    McpError::invalid_params(error.to_string(), None)
}

fn knowledge_action<T>(
    project_root: &str,
    summary: &str,
    result: knowledge::KnowledgeResult<T>,
) -> LoomMcpActionResult
where
    T: serde::Serialize,
{
    let normalized = match normalize_project_root(project_root) {
        Ok(root) => root.display,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    match result {
        Ok(details) => LoomMcpActionResult::Done(LoomMcpDoneResult {
            project_root: normalized,
            summary: summary.to_string(),
            details: Some(serde_json::to_value(details).unwrap_or_else(|_| json!({}))),
            warnings: vec![],
        }),
        Err(error) => knowledge_failure(normalized, error),
    }
}

fn knowledge_result(
    project_root: &str,
    result: knowledge::KnowledgeResult<LoomMcpActionResult>,
) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(project_root) {
        Ok(root) => root.display,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    match result {
        Ok(result) => result,
        Err(error) => knowledge_failure(normalized, error),
    }
}

fn state_failure(project_root: String, message: String) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root,
        error: LoomMcpFailure {
            code: "STATE_ERROR".to_string(),
            message,
            target_batch: None,
            domain: Some("project_lifecycle".to_string()),
            route_action: None,
            recovery_tool: None,
        },
    })
}

fn knowledge_failure(
    project_root: String,
    error: knowledge::KnowledgeError,
) -> LoomMcpActionResult {
    LoomMcpActionResult::Failed(LoomMcpFailureResult {
        project_root,
        error: LoomMcpFailure {
            code: "KNOWLEDGE_ERROR".to_string(),
            message: error.to_string(),
            target_batch: None,
            domain: Some("knowledge".to_string()),
            route_action: None,
            recovery_tool: None,
        },
    })
}

pub async fn run_stdio_server() -> anyhow::Result<()> {
    let service = LoomMcpServer::from_env().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
