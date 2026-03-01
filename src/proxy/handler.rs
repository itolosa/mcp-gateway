use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, ServerInfo,
};
use rmcp::service::{RequestContext, RoleClient, RoleServer, RunningService, ServiceError};
use rmcp::{ErrorData, ServerHandler};

use crate::proxy::error::ProxyError;

pub struct ProxyHandler {
    upstream: RunningService<RoleClient, ()>,
}

impl ProxyHandler {
    pub fn new(upstream: RunningService<RoleClient, ()>) -> Result<Self, ProxyError> {
        Ok(Self { upstream })
    }

    fn upstream_server_info(&self) -> Option<&ServerInfo> {
        self.upstream.peer_info()
    }
}

fn service_error_to_mcp(err: ServiceError) -> ErrorData {
    match err {
        ServiceError::McpError(e) => e,
        other => ErrorData::internal_error(other.to_string(), None),
    }
}

impl ServerHandler for ProxyHandler {
    fn get_info(&self) -> ServerInfo {
        let upstream_info = self.upstream_server_info();
        ServerInfo {
            protocol_version: upstream_info
                .map(|i| i.protocol_version.clone())
                .unwrap_or_default(),
            capabilities: upstream_info
                .map(|i| i.capabilities.clone())
                .unwrap_or_default(),
            server_info: rmcp::model::Implementation {
                name: "mcp-gateway".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: upstream_info.and_then(|i| i.instructions.clone()),
        }
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.upstream
            .list_tools(request)
            .await
            .map_err(service_error_to_mcp)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.upstream
            .call_tool(request)
            .await
            .map_err(service_error_to_mcp)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use rmcp::model::{
        CallToolResult, Content, Implementation, ListToolsResult, ServerCapabilities, ServerInfo,
        Tool,
    };
    use rmcp::ServiceExt;
    use serde_json::json;

    struct MockUpstreamServer;

    impl ServerHandler for MockUpstreamServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                server_info: Implementation {
                    name: "mock-upstream".to_string(),
                    version: "1.0.0".to_string(),
                    title: None,
                    description: None,
                    icons: None,
                    website_url: None,
                },
                ..Default::default()
            }
        }

        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListToolsResult, ErrorData> {
            let schema: serde_json::Map<String, serde_json::Value> =
                serde_json::from_value(json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                }))
                .unwrap();
            Ok(ListToolsResult {
                tools: vec![Tool::new("echo", "echoes input", schema)],
                next_cursor: None,
                meta: None,
            })
        }

        async fn call_tool(
            &self,
            request: CallToolRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> Result<CallToolResult, ErrorData> {
            if request.name.as_ref() == "echo" {
                let input = request
                    .arguments
                    .and_then(|a| a.get("message").cloned())
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(input)]))
            } else {
                Err(ErrorData::invalid_params(
                    format!("unknown tool: {}", request.name),
                    None,
                ))
            }
        }
    }

    /// Creates a proxy wired to a mock upstream via in-memory transport,
    /// and a client connected to the proxy (also in-memory).
    /// Returns the client and task handles for clean shutdown.
    async fn create_proxy_client() -> (
        RunningService<RoleClient, ()>,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
    ) {
        // upstream: mock server <-> proxy's client
        let (upstream_server_transport, upstream_client_transport) = tokio::io::duplex(4096);

        let upstream_handle = tokio::spawn(async move {
            let s = MockUpstreamServer
                .serve(upstream_server_transport)
                .await
                .unwrap();
            let _ = s.waiting().await;
        });

        let upstream_client = ().serve(upstream_client_transport).await.unwrap();
        let proxy = ProxyHandler::new(upstream_client).unwrap();

        // downstream: proxy server <-> test client
        let (proxy_server_transport, proxy_client_transport) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let s = proxy.serve(proxy_server_transport).await.unwrap();
            let _ = s.waiting().await;
        });

        let client = ().serve(proxy_client_transport).await.unwrap();
        (client, upstream_handle, proxy_handle)
    }

    #[test]
    fn service_error_to_mcp_preserves_mcp_error() {
        let mcp_err = ErrorData::invalid_params("bad param".to_string(), None);
        let converted = service_error_to_mcp(ServiceError::McpError(mcp_err.clone()));
        assert_eq!(converted.message, mcp_err.message);
        assert_eq!(converted.code, mcp_err.code);
    }

    #[test]
    fn service_error_to_mcp_wraps_other_errors() {
        let err = ServiceError::TransportClosed;
        let converted = service_error_to_mcp(err);
        assert!(converted.message.contains("closed"));
    }

    #[tokio::test]
    async fn proxy_get_info_returns_gateway_identity() {
        let (client, upstream_h, proxy_h) = create_proxy_client().await;
        let info = client.peer_info().unwrap();
        assert_eq!(info.server_info.name, "mcp-gateway");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some());
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_list_tools_forwards_upstream_tools() {
        let (client, upstream_h, proxy_h) = create_proxy_client().await;
        let result = client.list_tools(None).await.unwrap();
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools.first().map(|t| t.name.as_ref()), Some("echo"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_call_tool_forwards_and_returns_result() {
        let (client, upstream_h, proxy_h) = create_proxy_client().await;
        let params = CallToolRequestParams {
            name: "echo".into(),
            arguments: Some(serde_json::from_str(r#"{"message":"hello"}"#).unwrap()),
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await.unwrap();
        let text = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str());
        assert_eq!(text, Some("hello"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_call_unknown_tool_returns_error() {
        let (client, upstream_h, proxy_h) = create_proxy_client().await;
        let params = CallToolRequestParams {
            name: "nonexistent".into(),
            arguments: None,
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await;
        assert!(result.is_err());
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }
}
