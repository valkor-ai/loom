use std::future::{ready, Future};

use delivery_core::{
    InspectRequestInput, LoomMcpRuntimeContext, ReadFieldGroupInput, ReadRequestFieldsInput,
};
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
        _ => server
            .tools
            .call_registered_placeholder(&request.name, request.arguments)
            .map_err(|_| McpError::method_not_found::<CallToolRequestMethod>()),
    }
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

pub async fn run_stdio_server() -> anyhow::Result<()> {
    let service = LoomMcpServer::from_env().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
