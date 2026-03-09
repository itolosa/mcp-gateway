use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};

use crate::hexagon::entities::{GatewayError, ToolCallRequest};
use crate::hexagon::ports::{CliToolRunner, UpstreamClient};
use crate::hexagon::usecases::Gateway;

pub struct McpAdapter<U: UpstreamClient, C: CliToolRunner> {
    gateway: Gateway<U, C>,
}

impl<U: UpstreamClient, C: CliToolRunner> McpAdapter<U, C> {
    pub fn new(gateway: Gateway<U, C>) -> Self {
        Self { gateway }
    }
}

fn gateway_error_to_mcp(err: GatewayError) -> ErrorData {
    match err {
        GatewayError::NoPrefix { .. }
        | GatewayError::UnknownServer { .. }
        | GatewayError::ToolNotAllowed { .. } => ErrorData::invalid_params(err.to_string(), None),
        _ => ErrorData::internal_error(err.to_string(), None),
    }
}

fn domain_content_to_mcp(content: Vec<serde_json::Value>) -> Vec<Content> {
    content
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect()
}

impl<U: UpstreamClient + 'static, C: CliToolRunner + 'static> ServerHandler for McpAdapter<U, C> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new(
            "mcp-gateway",
            env!("CARGO_PKG_VERSION"),
        ))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self
            .gateway
            .list_tools()
            .await
            .map_err(gateway_error_to_mcp)?;
        let mcp_tools = tools
            .into_iter()
            .map(|t| Tool::new(t.name, t.description.unwrap_or_default(), t.schema))
            .collect();
        Ok(ListToolsResult {
            tools: mcp_tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let domain_request = ToolCallRequest {
            name: request.name.to_string(),
            arguments: request.arguments,
        };
        let result = self
            .gateway
            .call_tool(domain_request)
            .await
            .map_err(gateway_error_to_mcp)?;
        let content = domain_content_to_mcp(result.content);
        if result.is_error {
            Ok(CallToolResult::error(content))
        } else {
            Ok(CallToolResult::success(content))
        }
    }
}
