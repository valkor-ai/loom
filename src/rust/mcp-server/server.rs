use std::future::{ready, Future};

use delivery_core::LoomMcpRuntimeContext;
use rmcp::{
    model::{
        CallToolRequestMethod, CallToolRequestParams, CallToolResult, Implementation,
        ListResourceTemplatesResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
        ReadResourceRequestParams, ReadResourceResult, ServerCapabilities, ServerInfo, Tool,
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
        ready(
            self.tools
                .call_registered_placeholder(&request.name, request.arguments)
                .map_err(|_| McpError::method_not_found::<CallToolRequestMethod>()),
        )
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
        ready(Ok(self.resources.read_placeholder(&request.uri)))
    }
}

pub async fn run_stdio_server() -> anyhow::Result<()> {
    let service = LoomMcpServer::from_env().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
