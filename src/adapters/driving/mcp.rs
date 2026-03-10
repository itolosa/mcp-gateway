use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};

use crate::hexagon::entities::{GatewayError, ToolCallRequest};
use crate::hexagon::ports::{CliToolRunner, ToolFilter, UpstreamClient};
use crate::hexagon::usecases::Gateway;

pub struct McpAdapter<U: UpstreamClient, C: CliToolRunner, F: ToolFilter> {
    gateway: Gateway<U, C, F>,
}

impl<U: UpstreamClient, C: CliToolRunner, F: ToolFilter> McpAdapter<U, C, F> {
    pub fn new(gateway: Gateway<U, C, F>) -> Self {
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

impl<U: UpstreamClient + 'static, C: CliToolRunner + 'static, F: ToolFilter + 'static> ServerHandler
    for McpAdapter<U, C, F>
{
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn no_prefix_maps_to_invalid_params() {
        let err = gateway_error_to_mcp(GatewayError::NoPrefix {
            tool: "t".to_string(),
        });
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("prefix"));
    }

    #[test]
    fn unknown_server_maps_to_invalid_params() {
        let err = gateway_error_to_mcp(GatewayError::UnknownServer {
            server: "s".to_string(),
            tool: "t".to_string(),
        });
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("unknown server"));
    }

    #[test]
    fn tool_not_allowed_maps_to_invalid_params() {
        let err = gateway_error_to_mcp(GatewayError::ToolNotAllowed {
            tool: "t".to_string(),
        });
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("not allowed"));
    }

    #[test]
    fn upstream_error_maps_to_internal_error() {
        let err = gateway_error_to_mcp(GatewayError::Upstream("fail".to_string()));
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn upstream_timeout_maps_to_internal_error() {
        let err = gateway_error_to_mcp(GatewayError::UpstreamTimeout {
            server: "s".to_string(),
            timeout_secs: 5,
        });
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn cli_tool_error_maps_to_internal_error() {
        let err = gateway_error_to_mcp(GatewayError::CliTool("fail".to_string()));
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn domain_content_to_mcp_converts_valid_content() {
        let content = vec![serde_json::json!({"type": "text", "text": "hello"})];
        let result = domain_content_to_mcp(content);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn domain_content_to_mcp_skips_invalid_content() {
        let content = vec![serde_json::json!({"invalid": true})];
        let result = domain_content_to_mcp(content);
        assert!(result.is_empty());
    }
}
