use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
    ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};

use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationCallRequest, OperationPolicy, ProviderClient,
};
use crate::hexagon::usecases::gateway::Gateway;

pub struct McpAdapter<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy> {
    gateway: Gateway<U, C, F>,
}

impl<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy> McpAdapter<U, C, F> {
    pub fn new(gateway: Gateway<U, C, F>) -> Self {
        Self { gateway }
    }
}

fn gateway_error_to_mcp(err: GatewayError) -> ErrorData {
    match err {
        GatewayError::InvalidMapping { .. }
        | GatewayError::UnknownProvider { .. }
        | GatewayError::OperationNotAllowed { .. } => {
            ErrorData::invalid_params(err.to_string(), None)
        }
        _ => ErrorData::internal_error(err.to_string(), None),
    }
}

fn domain_content_to_mcp(content: Vec<String>) -> Vec<Content> {
    content
        .into_iter()
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect()
}

impl<
        U: ProviderClient + 'static,
        C: CliOperationRunner + 'static,
        F: OperationPolicy + 'static,
    > ServerHandler for McpAdapter<U, C, F>
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
            .list_operations()
            .await
            .map_err(gateway_error_to_mcp)?;
        let mcp_tools = tools
            .into_iter()
            .map(|t| {
                let schema: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_str(&t.schema).unwrap_or_default();
                Tool::new(t.name, t.description.unwrap_or_default(), schema)
            })
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
        let domain_request = OperationCallRequest {
            name: request.name.to_string(),
            arguments: request
                .arguments
                .map(|m| serde_json::to_string(&m).unwrap_or_default()),
        };
        let result = self
            .gateway
            .route_operation(domain_request)
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
    fn invalid_mapping_maps_to_invalid_params() {
        let err = gateway_error_to_mcp(GatewayError::InvalidMapping {
            operation: "t".to_string(),
        });
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("prefix"));
    }

    #[test]
    fn unknown_provider_maps_to_invalid_params() {
        let err = gateway_error_to_mcp(GatewayError::UnknownProvider {
            provider: "s".to_string(),
            operation: "t".to_string(),
        });
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("unknown provider"));
    }

    #[test]
    fn operation_not_allowed_maps_to_invalid_params() {
        let err = gateway_error_to_mcp(GatewayError::OperationNotAllowed {
            operation: "t".to_string(),
        });
        assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("not allowed"));
    }

    #[test]
    fn provider_error_maps_to_internal_error() {
        let err = gateway_error_to_mcp(GatewayError::Provider("fail".to_string()));
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn cli_operation_error_maps_to_internal_error() {
        let err = gateway_error_to_mcp(GatewayError::CliOperation("fail".to_string()));
        assert_eq!(err.code, rmcp::model::ErrorCode::INTERNAL_ERROR);
    }

    #[test]
    fn domain_content_to_mcp_converts_valid_content() {
        let content = vec![r#"{"type":"text","text":"hello"}"#.to_string()];
        let result = domain_content_to_mcp(content);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn domain_content_to_mcp_skips_invalid_content() {
        let content = vec![r#"{"invalid":true}"#.to_string()];
        let result = domain_content_to_mcp(content);
        assert!(result.is_empty());
    }
}
