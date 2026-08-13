use std::future::{ready, Future};

use brainstorm::{accept_brainstorm_file, BrainstormConfirmBlockInput};
use delivery_core::{
    is_submit_tool, normalize_project_root, status_details, submit_tool_spec, validate_plan_input,
    DomainDispatcher, FileSubmitInput, InspectRequestInput, LoomMcpActionResult, LoomMcpDoneResult,
    LoomMcpFailure, LoomMcpFailureResult, LoomMcpRepairableErrorResult, LoomMcpRuntimeContext,
    OperationContext, PlanConflictChoice, PlanConflictResolveInput, PlanToolInput,
    ProjectToolInput, ReadFieldGroupInput, SubmitAcceptedEvent, TransitionEngine, TransitionStore,
};
use deploy::{DeployBootstrapInput, DeployToolInput};
use execution::{VsefmToolInput, VsefmVerificationResolveInput};
use knowledge::mcp_models::{
    KnowledgeAddInput, KnowledgeBrainstormContextInput, KnowledgeInspectChunkInput,
    KnowledgeNameInput, KnowledgePendingInput, KnowledgeProjectInput, KnowledgeSearchInput,
    KnowledgeSemanticSubmitInput, KnowledgeUpdateInput,
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
            "Loom MCP server. For a plain @loom software delivery request, call the plan tool first; do not inspect or modify the repository before it returns. Use deploy tools only for an explicit @loom deploy request. Use verify for the V-SEFM onboarding gate. Use registered Loom tools and resources; do not call legacy CLI commands. An auto_runnable result is a required continuation checkpoint: the agent must execute its next action and may not finish with a progress summary. If a local or wrapped tool call fails, recover from the exact failure and continue from the latest Loom state; only user_gate, done, blocked, or failed is terminal.",
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
    let tool_name = canonical_tool_name(request.name.as_ref());
    match tool_name {
        "initProject" => action_result(init_project_tool(parse_args::<ProjectToolInput>(
            request.arguments,
        )?)),
        "status" => action_result(status_tool(parse_args::<ProjectToolInput>(
            request.arguments,
        )?)),
        "plan" => action_result(plan_tool(parse_args::<PlanToolInput>(request.arguments)?)),
        "planConflictResolve" => action_result(resolve_plan_conflict(parse_args::<
            PlanConflictResolveInput,
        >(request.arguments)?)),
        "continue" => action_result(continue_tool(parse_args::<ProjectToolInput>(
            request.arguments,
        )?)),
        "verify" => action_result(verify_tool(parse_args::<VsefmToolInput>(
            request.arguments,
        )?)),
        "vsefmVerificationResolve" => {
            action_result(resolve_vsefm_tool(parse_args::<
                VsefmVerificationResolveInput,
            >(request.arguments)?))
        }
        "browserRuntimePrepare" => action_result(crate::browser_runtime::prepare(parse_args::<
            ProjectToolInput,
        >(
            request.arguments,
        )?)),
        "brainstormConfirmBlock" => action_result(brainstorm_confirm_block_tool(parse_args::<
            BrainstormConfirmBlockInput,
        >(
            request.arguments,
        )?)),
        "inspectRequest" => structured(state::inspect_request(parse_args::<InspectRequestInput>(
            request.arguments,
        )?)),
        "readFieldGroup" => structured(state::read_field_group(parse_args::<ReadFieldGroupInput>(
            request.arguments,
        )?)),
        "knowledgeAdd" => {
            let input = parse_args::<KnowledgeAddInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source registered.",
                knowledge::add_source(input),
            ))
        }
        "knowledgeUpdate" => {
            let input = parse_args::<KnowledgeUpdateInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source updated.",
                knowledge::update_source(input),
            ))
        }
        "knowledgePending" => {
            let input = parse_args::<KnowledgePendingInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge pending queue loaded.",
                knowledge::pending_sources(input),
            ))
        }
        "knowledgeDiscard" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge pending operations discarded.",
                knowledge::discard_pending(input),
            ))
        }
        "knowledgeBuild" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_result(
                &project_root,
                knowledge::build_source_from_input(input),
            ))
        }
        "knowledgeResume" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_result(
                &project_root,
                knowledge::resume_source_from_input(input),
            ))
        }
        "knowledgeList" => {
            let input = parse_args::<KnowledgeProjectInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge sources listed.",
                knowledge::list_sources(input),
            ))
        }
        "knowledgeStatus" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source status loaded.",
                knowledge::source_status(input),
            ))
        }
        "knowledgeRemove" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source removed.",
                knowledge::remove_source(input),
            ))
        }
        "knowledgeEnable" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source enabled.",
                knowledge::enable_source(input),
            ))
        }
        "knowledgeDisable" => {
            let input = parse_args::<KnowledgeNameInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge source disabled.",
                knowledge::disable_source(input),
            ))
        }
        "knowledgeSearch" => {
            let input = parse_args::<KnowledgeSearchInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge search completed.",
                knowledge::search_knowledge(input),
            ))
        }
        "knowledgeBrainstormContext" => {
            let input = parse_args::<KnowledgeBrainstormContextInput>(request.arguments)?;
            action_result(knowledge_brainstorm_context_tool(input))
        }
        "knowledgeInspectChunk" => {
            let input = parse_args::<KnowledgeInspectChunkInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_action(
                &project_root,
                "Knowledge chunk inspected.",
                knowledge::inspect_chunk(input),
            ))
        }
        "knowledgeSemanticSubmitFile" => {
            let input = parse_args::<KnowledgeSemanticSubmitInput>(request.arguments)?;
            let project_root = input.project_root.clone();
            action_result(knowledge_result(
                &project_root,
                knowledge::submit_semantic_pack_from_input(input),
            ))
        }
        "deployPrepare" => action_result(deploy::deploy_prepare(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "deployRun" => action_result(deploy::deploy_run(normalize_deploy_input(parse_args::<
            DeployToolInput,
        >(
            request.arguments,
        )?)?)),
        "deployUp" => action_result(deploy::deploy_up(normalize_deploy_input(parse_args::<
            DeployToolInput,
        >(
            request.arguments,
        )?)?)),
        "deployStatus" => action_result(deploy::deploy_status(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "deployInspect" => action_result(deploy::deploy_inspect(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "deployValidate" => action_result(deploy::deploy_validate(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        "deployLogs" => action_result(deploy::deploy_logs(normalize_deploy_input(parse_args::<
            DeployToolInput,
        >(
            request.arguments,
        )?)?)),
        "deployBootstrap" => {
            action_result(deploy::deploy_bootstrap(normalize_deploy_bootstrap_input(
                parse_args::<DeployBootstrapInput>(request.arguments)?,
            )?))
        }
        "deployDown" => action_result(deploy::deploy_down(normalize_deploy_input(parse_args::<
            DeployToolInput,
        >(
            request.arguments,
        )?)?)),
        "deployRepair" => action_result(deploy::deploy_repair(normalize_deploy_input(
            parse_args::<DeployToolInput>(request.arguments)?,
        )?)),
        name if is_submit_tool(name) => action_result(submit_file_tool(
            name,
            parse_args::<FileSubmitInput>(request.arguments)?,
        )),
        _ => server
            .tools
            .call_registered_placeholder(tool_name, request.arguments)
            .map_err(|_| McpError::method_not_found::<CallToolRequestMethod>()),
    }
}

fn canonical_tool_name(name: &str) -> &str {
    name.strip_prefix("loom.").unwrap_or(name)
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
    if let Err(error) = state::recover_lifecycle_transaction(&normalized.display) {
        return state_failure(normalized.display, error.to_string());
    }
    let store = FileTransitionStore;
    let status = match store.load_status(&normalized.display) {
        Ok(status) => status,
        Err(error) if error.code() == "STATE_NOT_INITIALIZED" => {
            return LoomMcpActionResult::Failed(LoomMcpFailureResult {
                project_root: normalized.display.clone(),
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

    let mut details = status_details(
        &status,
        active_delivery.as_ref(),
        active_operation.as_ref(),
        &[],
    );
    if let Some(conflict_id) = status.pending_plan_conflict_id.as_deref() {
        if let Ok(conflict) = state::load_plan_conflict(&normalized.display, conflict_id) {
            if let Some(object) = details.as_object_mut() {
                object.insert(
                    "pendingPlanConflict".to_string(),
                    json!({
                        "conflictRef": format!(".loom/plan-conflicts/{}.json", conflict.conflict_id),
                        "activeDeliveryId": conflict.active_delivery_id,
                        "status": conflict.status,
                        "choices": [
                            {"number": "1", "choice": "continue_current"},
                            {"number": "2", "choice": "start_new"}
                        ]
                    }),
                );
            }
        }
    }

    LoomMcpActionResult::Done(LoomMcpDoneResult {
        project_root: normalized.display,
        summary: "Loom project status read.".to_string(),
        details: Some(details),
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
    let result = init_project_state(&validated.project_root)
        .map_err(|error| state::store::StateError::StateCorrupted(error.to_string()))
        .and_then(|_| state::persist_plan_request(&validated).map(|_| ()))
        .and_then(|_| route_plan_request(&validated));
    match result {
        Ok(result) => result,
        Err(error) => state_failure(validated.project_root, error.to_string()),
    }
}

fn route_plan_request(
    input: &delivery_core::ValidatedPlanInput,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    let store = FileTransitionStore;
    state::recover_lifecycle_transaction(&input.project_root)?;
    let status = store
        .load_status(&input.project_root)
        .map_err(|error| state::store::StateError::StateCorrupted(error.to_string()))?;

    let Some(active_delivery_id) = status.active_delivery_id.clone() else {
        let mut prepared = input.clone();
        prepared.expected_lifecycle_revision = Some(status.revision);
        prepared.supersede_active_delivery_id = None;
        return Ok(WorkflowDomainDispatcher.start_brainstorm(&prepared));
    };
    let active_delivery = store
        .load_delivery_index(&input.project_root, &active_delivery_id)
        .map_err(|error| state::store::StateError::StateCorrupted(error.to_string()))?;
    if matches!(
        active_delivery.status,
        delivery_core::DeliveryLifecycleStatus::Completed
            | delivery_core::DeliveryLifecycleStatus::CompletedWithOverride
            | delivery_core::DeliveryLifecycleStatus::Superseded
    ) {
        let result = state::mutate_lifecycle(&input.project_root, |current, _| {
            if current.active_delivery_id.as_deref() == Some(active_delivery_id.as_str()) {
                current.active_delivery_id = None;
            }
            Ok(((), Vec::new(), Vec::new(), Vec::new()))
        });
        result?;
        let refreshed = store
            .load_status(&input.project_root)
            .map_err(|error| state::store::StateError::StateCorrupted(error.to_string()))?;
        let mut prepared = input.clone();
        prepared.expected_lifecycle_revision = Some(refreshed.revision);
        return Ok(WorkflowDomainDispatcher.start_brainstorm(&prepared));
    }

    if active_delivery.request_fingerprint.as_deref()
        == Some(input.request_identity.fingerprint.as_str())
    {
        resolve_pending_plan_conflict_as_continue(&input.project_root, &status)?;
        return Ok(continue_tool_inner(ProjectToolInput {
            project_root: input.project_root.clone(),
        }));
    }

    if let Some(conflict_id) = status.pending_plan_conflict_id.clone() {
        let conflict =
            state::load_plan_conflict(&input.project_root, &conflict_id).map_err(|error| {
                state::store::StateError::StateCorrupted(format!(
                    "pending plan conflict {conflict_id} cannot be read: {error}"
                ))
            })?;
        if conflict.status == delivery_core::PlanConflictStatus::Pending
            && conflict.active_delivery_id == active_delivery_id
            && conflict.incoming_request_fingerprint == input.request_identity.fingerprint
        {
            return Ok(plan_conflict_gate(&input.project_root, &conflict));
        }
    }

    let mut conflict = state::create_or_load_plan_conflict(
        &input.project_root,
        &active_delivery_id,
        status.revision.saturating_add(1),
        input,
    )
    .map_err(|error| state::store::StateError::StateCorrupted(error.to_string()))?;
    conflict.active_revision = status.revision.saturating_add(1);
    conflict.status = delivery_core::PlanConflictStatus::Pending;
    conflict.updated_at = state::store::now_string();
    let replacement = conflict.clone();
    state::commit_lifecycle(
        &input.project_root,
        state::LifecycleCommit {
            expected_revision: Some(status.revision),
            expected_active_delivery_id: Some(Some(active_delivery_id)),
            conflicts: pending_conflict_replacement(&input.project_root, &status, replacement)?,
            pending_plan_conflict_id: Some(Some(conflict.conflict_id.clone())),
            ..state::LifecycleCommit::default()
        },
    )?;
    Ok(plan_conflict_gate(&input.project_root, &conflict))
}

fn resolve_pending_plan_conflict_as_continue(
    project_root: &str,
    status: &delivery_core::ProjectStatus,
) -> Result<(), state::store::StateError> {
    let Some(conflict_id) = status.pending_plan_conflict_id.as_deref() else {
        return Ok(());
    };
    let mut conflict = state::load_plan_conflict(project_root, conflict_id)?;
    if conflict.status == delivery_core::PlanConflictStatus::Pending {
        conflict.status = delivery_core::PlanConflictStatus::ResolvedContinue;
        conflict.updated_at = state::store::now_string();
    }
    state::commit_lifecycle(
        project_root,
        state::LifecycleCommit {
            expected_revision: Some(status.revision),
            conflicts: vec![conflict],
            pending_plan_conflict_id: Some(None),
            ..state::LifecycleCommit::default()
        },
    )?;
    Ok(())
}

fn pending_conflict_replacement(
    project_root: &str,
    status: &delivery_core::ProjectStatus,
    replacement: delivery_core::PlanConflictRecord,
) -> Result<Vec<delivery_core::PlanConflictRecord>, state::store::StateError> {
    let mut records = Vec::new();
    if let Some(conflict_id) = status.pending_plan_conflict_id.as_deref() {
        if conflict_id != replacement.conflict_id {
            let mut previous = state::load_plan_conflict(project_root, conflict_id)?;
            if previous.status == delivery_core::PlanConflictStatus::Pending {
                previous.status = delivery_core::PlanConflictStatus::Expired;
                previous.updated_at = state::store::now_string();
            }
            records.push(previous);
        }
    }
    records.push(replacement);
    Ok(records)
}

fn plan_conflict_gate(
    project_root: &str,
    conflict: &delivery_core::PlanConflictRecord,
) -> LoomMcpActionResult {
    let request = state::load_plan_request(project_root, &conflict.incoming_request_ref).ok();
    let request_text = request
        .as_ref()
        .map(|request| request.request_text.as_str())
        .unwrap_or("待处理的新需求");
    let prompt = format!(
        "当前已有一个 Loom 交付正在进行，本次请求与当前交付不同。\n\n当前交付：{}\n本次新请求：{}\n\n请选择：\n\n1. 继续当前交付：继续执行原需求，本次新请求暂不创建交付\n2. 开始新的需求：关闭当前交付并开始本次新需求",
        conflict.active_delivery_id, request_text
    );
    LoomMcpActionResult::UserGate(
        delivery_core::LoomMcpUserGateResult::new(
            project_root.to_string(),
            prompt,
            vec!["1".to_string(), "2".to_string()],
            None,
            Some(conflict.active_delivery_id.clone()),
            None,
            Some(json!({
                "kind": "plan_conflict",
                "conflictRef": state::conflict_ref(conflict),
                "incomingRequestText": request_text,
                "choices": [
                    {"number": "1", "choice": "continue_current", "label": "继续当前交付"},
                    {"number": "2", "choice": "start_new", "label": "开始新的需求"}
                ]
            })),
        )
        .with_agent_instruction(
            "Present this plan conflict in the user's language and wait for 1 or 2. Then call loom.planConflictResolve with the returned conflictRef and the corresponding structured choice. Choice 1 resumes the existing delivery through loom.continue; choice 2 closes the existing delivery as superseded and starts the pending request. Do not call loom.plan again.",
        ),
    )
}

fn resolve_plan_conflict(input: PlanConflictResolveInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    let result = resolve_plan_conflict_inner(&normalized.display, input);
    match result {
        Ok(result) => result,
        Err(error) => state_failure(normalized.display, error.to_string()),
    }
}

fn resolve_plan_conflict_inner(
    project_root: &str,
    input: PlanConflictResolveInput,
) -> Result<LoomMcpActionResult, state::store::StateError> {
    state::recover_lifecycle_transaction(project_root)?;
    let conflict_id = state::conflict_id_from_ref(&input.conflict_ref)?;
    let mut conflict = state::load_plan_conflict(project_root, &conflict_id)?;
    if conflict.status != delivery_core::PlanConflictStatus::Pending {
        return Err(state::store::StateError::InvalidArgument(
            "plan conflict has already been resolved".to_string(),
        ));
    }
    let store = FileTransitionStore;
    let status = store
        .load_status(project_root)
        .map_err(|error| state::store::StateError::StateCorrupted(error.to_string()))?;
    if status.active_delivery_id.as_deref() != Some(conflict.active_delivery_id.as_str())
        || status.revision != conflict.active_revision
    {
        return Err(state::store::StateError::StateCorrupted(
            "the active delivery changed before the plan conflict was resolved".to_string(),
        ));
    }

    match input.choice {
        PlanConflictChoice::ContinueCurrent => {
            conflict.status = delivery_core::PlanConflictStatus::ResolvedContinue;
            conflict.updated_at = state::store::now_string();
            state::commit_lifecycle(
                project_root,
                state::LifecycleCommit {
                    expected_revision: Some(status.revision),
                    expected_active_delivery_id: Some(Some(conflict.active_delivery_id.clone())),
                    conflicts: vec![conflict],
                    pending_plan_conflict_id: Some(None),
                    ..state::LifecycleCommit::default()
                },
            )?;
            Ok(continue_tool_inner(ProjectToolInput {
                project_root: project_root.to_string(),
            }))
        }
        PlanConflictChoice::StartNew => {
            let request = state::load_plan_request(project_root, &conflict.incoming_request_ref)?;
            let mut validated = validate_plan_input(PlanToolInput {
                project_root: project_root.to_string(),
                request_text: request.request_text,
                requirement_files: request.requirement_file_refs,
            })
            .map_err(state::store::StateError::InvalidArgument)?;
            if validated.request_identity.fingerprint != conflict.incoming_request_fingerprint
                || validated.request_identity.request_ref != conflict.incoming_request_ref
            {
                return Err(state::store::StateError::InvalidArgument(
                    "the pending plan request changed; start a new loom.plan request".to_string(),
                ));
            }
            validated.supersede_active_delivery_id = Some(conflict.active_delivery_id.clone());
            validated.expected_lifecycle_revision = Some(status.revision);
            validated.plan_conflict_id = Some(conflict.conflict_id);
            Ok(WorkflowDomainDispatcher.start_brainstorm(&validated))
        }
    }
}

fn continue_tool(input: ProjectToolInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    if let Err(error) = state::recover_lifecycle_transaction(&normalized.display) {
        return state_failure(normalized.display, error.to_string());
    }
    continue_tool_inner(ProjectToolInput {
        project_root: normalized.display,
    })
}

fn continue_tool_inner(input: ProjectToolInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    if let Some(result) = execution::resume_unattached_vsefm(&normalized.display) {
        return result;
    }
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

fn verify_tool(mut input: VsefmToolInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    input.project_root = normalized.display;
    execution::verify(input, WorkflowDomainDispatcher)
}

fn resolve_vsefm_tool(mut input: VsefmVerificationResolveInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    input.project_root = normalized.display;
    execution::resolve_vsefm_verification(input, WorkflowDomainDispatcher)
}

fn brainstorm_confirm_block_tool(input: BrainstormConfirmBlockInput) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    let input = BrainstormConfirmBlockInput {
        project_root: normalized.display,
        request_ref: input.request_ref,
        block: input.block,
        summary: input.summary,
        confirmed_data: input.confirmed_data,
        skipped: input.skipped,
        skip_reason: input.skip_reason,
    };
    brainstorm::confirm_block(input)
}

fn submit_file_tool(tool_name: &str, input: FileSubmitInput) -> LoomMcpActionResult {
    submit_file_tool_locked(tool_name, input)
}

fn submit_file_tool_locked(tool_name: &str, input: FileSubmitInput) -> LoomMcpActionResult {
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
            let result = LoomMcpActionResult::RepairableError(LoomMcpRepairableErrorResult {
                project_root: normalized.display.clone(),
                stop_allowed: false,
                target_file,
                target_ids,
                issues,
                resubmit_tool: resubmit_tool.clone(),
                fix_scope: Some(
                    "Edit only the authorized artifact JSON target, then resubmit with the same Loom MCP submit tool."
                        .to_string(),
                ),
                read_groups,
                agent_instruction: delivery_core::repairable_error_agent_instruction(&resubmit_tool),
            });
            if let LoomMcpActionResult::RepairableError(repair) = &result {
                if let Err(error) = state::record_pending_repair_for_request(
                    &normalized_input.project_root,
                    &normalized_input.request_ref,
                    repair,
                ) {
                    return LoomMcpActionResult::Failed(LoomMcpFailureResult {
                        project_root: normalized.display,
                        error: LoomMcpFailure {
                            code: "REPAIR_STATE_PERSIST_FAILED".to_string(),
                            message: error.to_string(),
                            target_batch: None,
                            domain: Some("workflow".to_string()),
                            route_action: Some("repair_preflight".to_string()),
                            recovery_tool: Some("loom.status".to_string()),
                        },
                    });
                }
            }
            return result;
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

    match delivery_core::canonical_tool_name(tool_name) {
        "brainstormAcceptFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                accept_brainstorm_file(&normalized_input, &authorized, WorkflowDomainDispatcher),
            );
        }
        "technicalBaselineAcceptFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                accept_technical_baseline_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "repositoryContextAcceptFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                accept_repository_context_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "architectureSectionSubmitFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                architecture::accept_architecture_section_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "taskPlanAcceptFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                execution::accept_task_plan_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "recordTaskResultFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                execution::accept_task_result_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "reviewAcceptFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                execution::accept_review_result_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "reviewResolveFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                execution::accept_manual_review_resolution_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "repairSubmitFile" => {
            if authorized.artifact_kind == delivery_core::ArtifactKind::ArchitectureArtifactRepair {
                return persist_repairable_result(
                    &normalized_input,
                    &authorized,
                    architecture::accept_architecture_repair_file(
                        &normalized_input,
                        &authorized,
                        WorkflowDomainDispatcher,
                    ),
                );
            }
            if authorized.artifact_kind == delivery_core::ArtifactKind::DeployExecutionRepairResult
            {
                return persist_repairable_result(
                    &normalized_input,
                    &authorized,
                    deploy::accept_deploy_execution_repair_file(&normalized_input, &authorized),
                );
            }
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                execution::accept_repair_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "vsefmVerificationAcceptFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                execution::accept_vsefm_verification_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
            );
        }
        "vsefmRepairAcceptFile" => {
            return persist_repairable_result(
                &normalized_input,
                &authorized,
                execution::accept_vsefm_repair_file(
                    &normalized_input,
                    &authorized,
                    WorkflowDomainDispatcher,
                ),
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

fn knowledge_brainstorm_context_tool(
    mut input: KnowledgeBrainstormContextInput,
) -> LoomMcpActionResult {
    let normalized = match normalize_project_root(&input.project_root) {
        Ok(root) => root,
        Err(message) => return LoomMcpActionResult::invalid_project_root(message),
    };
    let project_root = normalized.display.clone();
    input.project_root = project_root.clone();
    knowledge_action(
        &project_root,
        "Knowledge Brainstorm context prepared.",
        knowledge::brainstorm_context(input),
    )
}

fn persist_repairable_result(
    input: &FileSubmitInput,
    authorized: &state::AuthorizedWriteSet,
    result: LoomMcpActionResult,
) -> LoomMcpActionResult {
    let LoomMcpActionResult::RepairableError(repair) = &result else {
        return result;
    };
    if let Err(error) = state::record_pending_repair(&input.project_root, authorized, repair) {
        return LoomMcpActionResult::Failed(LoomMcpFailureResult {
            project_root: input.project_root.clone(),
            error: LoomMcpFailure {
                code: "REPAIR_STATE_PERSIST_FAILED".to_string(),
                message: error.to_string(),
                target_batch: None,
                domain: Some("workflow".to_string()),
                route_action: Some("repair_submit".to_string()),
                recovery_tool: Some("loom.status".to_string()),
            },
        });
    }
    result
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
