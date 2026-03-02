use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, ServerInfo,
};
use rmcp::service::{RequestContext, RoleClient, RoleServer, RunningService, ServiceError};
use rmcp::{ErrorData, ServerHandler};

use crate::cli_tools::CliToolExecutor;
use crate::filter::ToolFilter;
use crate::proxy::error::ProxyError;

pub struct ProxyHandler<F: ToolFilter> {
    upstream: RunningService<RoleClient, ()>,
    filter: F,
    cli_tools: Option<CliToolExecutor>,
}

impl<F: ToolFilter> ProxyHandler<F> {
    pub fn new(
        upstream: RunningService<RoleClient, ()>,
        filter: F,
        cli_tools: Option<CliToolExecutor>,
    ) -> Result<Self, ProxyError> {
        Ok(Self {
            upstream,
            filter,
            cli_tools,
        })
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

impl<F: ToolFilter + 'static> ServerHandler for ProxyHandler<F> {
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
        let mut result = self
            .upstream
            .list_tools(request)
            .await
            .map_err(service_error_to_mcp)?;
        if let Some(cli) = &self.cli_tools {
            result.tools.extend(cli.list_tools());
        }
        result
            .tools
            .retain(|tool| self.filter.is_tool_allowed(tool.name.as_ref()));
        Ok(result)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.filter.is_tool_allowed(request.name.as_ref()) {
            return Err(ErrorData::invalid_params(
                format!("tool '{}' is not allowed", request.name),
                None,
            ));
        }
        if let Some(cli) = &self.cli_tools {
            if cli.has_tool(request.name.as_ref()) {
                return cli.call_tool(&request).await;
            }
        }
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
    use crate::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};
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
    async fn create_proxy_client(
        filter: AllowlistFilter,
    ) -> (
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
        let proxy = ProxyHandler::new(upstream_client, filter, None).unwrap();

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
        let (client, upstream_h, proxy_h) = create_proxy_client(AllowlistFilter::new(vec![])).await;
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
        let (client, upstream_h, proxy_h) = create_proxy_client(AllowlistFilter::new(vec![])).await;
        let result = client.list_tools(None).await.unwrap();
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools.first().map(|t| t.name.as_ref()), Some("echo"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_call_tool_forwards_and_returns_result() {
        let (client, upstream_h, proxy_h) = create_proxy_client(AllowlistFilter::new(vec![])).await;
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
        let (client, upstream_h, proxy_h) = create_proxy_client(AllowlistFilter::new(vec![])).await;
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

    #[tokio::test]
    async fn proxy_list_tools_filters_by_allowlist() {
        let filter = AllowlistFilter::new(vec!["not_echo".to_string()]);
        let (client, upstream_h, proxy_h) = create_proxy_client(filter).await;
        let result = client.list_tools(None).await.unwrap();
        assert!(result.tools.is_empty());
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_list_tools_allows_matching_tools() {
        let filter = AllowlistFilter::new(vec!["echo".to_string()]);
        let (client, upstream_h, proxy_h) = create_proxy_client(filter).await;
        let result = client.list_tools(None).await.unwrap();
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools.first().map(|t| t.name.as_ref()), Some("echo"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_call_blocked_tool_returns_error() {
        let filter = AllowlistFilter::new(vec!["not_echo".to_string()]);
        let (client, upstream_h, proxy_h) = create_proxy_client(filter).await;
        let params = CallToolRequestParams {
            name: "echo".into(),
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

    #[tokio::test]
    async fn proxy_call_allowed_tool_succeeds() {
        let filter = AllowlistFilter::new(vec!["echo".to_string()]);
        let (client, upstream_h, proxy_h) = create_proxy_client(filter).await;
        let params = CallToolRequestParams {
            name: "echo".into(),
            arguments: Some(serde_json::from_str(r#"{"message":"filtered"}"#).unwrap()),
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await.unwrap();
        let text = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str());
        assert_eq!(text, Some("filtered"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    // --- CLI tools integration tests ---

    use crate::config::model::CliToolDef;
    use std::collections::BTreeMap;

    async fn create_proxy_client_with_filter<F: ToolFilter + 'static>(
        filter: F,
        cli_tools: Option<CliToolExecutor>,
    ) -> (
        RunningService<RoleClient, ()>,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let (upstream_server_transport, upstream_client_transport) = tokio::io::duplex(4096);

        let upstream_handle = tokio::spawn(async move {
            let s = MockUpstreamServer
                .serve(upstream_server_transport)
                .await
                .unwrap();
            let _ = s.waiting().await;
        });

        let upstream_client = ().serve(upstream_client_transport).await.unwrap();
        let proxy = ProxyHandler::new(upstream_client, filter, cli_tools).unwrap();

        let (proxy_server_transport, proxy_client_transport) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let s = proxy.serve(proxy_server_transport).await.unwrap();
            let _ = s.waiting().await;
        });

        let client = ().serve(proxy_client_transport).await.unwrap();
        (client, upstream_handle, proxy_handle)
    }

    async fn create_proxy_client_with_cli_tools(
        filter: AllowlistFilter,
        cli_tools: Option<CliToolExecutor>,
    ) -> (
        RunningService<RoleClient, ()>,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
    ) {
        create_proxy_client_with_filter(filter, cli_tools).await
    }

    fn make_cli_executor() -> CliToolExecutor {
        let mut tools = BTreeMap::new();
        tools.insert(
            "cli-cat".to_string(),
            CliToolDef {
                command: "cat".to_string(),
                description: Some("Cat stdin to stdout".to_string()),
            },
        );
        CliToolExecutor::new(tools)
    }

    #[tokio::test]
    async fn proxy_list_tools_includes_cli_tools() {
        let (client, upstream_h, proxy_h) = create_proxy_client_with_cli_tools(
            AllowlistFilter::new(vec![]),
            Some(make_cli_executor()),
        )
        .await;
        let result = client.list_tools(None).await.unwrap();
        // upstream "echo" + CLI "cli-cat"
        assert_eq!(result.tools.len(), 2);
        let names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"cli-cat"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_call_tool_routes_to_cli_executor() {
        let (client, upstream_h, proxy_h) = create_proxy_client_with_cli_tools(
            AllowlistFilter::new(vec![]),
            Some(make_cli_executor()),
        )
        .await;
        let params = CallToolRequestParams {
            name: "cli-cat".into(),
            arguments: None,
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await.unwrap();
        let text = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("");
        // cat echoes the JSON request from stdin
        assert!(text.contains("cli-cat"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_list_tools_filters_cli_tools_by_allowlist() {
        let filter = AllowlistFilter::new(vec!["echo".to_string()]);
        let (client, upstream_h, proxy_h) =
            create_proxy_client_with_cli_tools(filter, Some(make_cli_executor())).await;
        let result = client.list_tools(None).await.unwrap();
        // Only upstream "echo" should pass; CLI "cli-cat" is not in allowlist
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools.first().map(|t| t.name.as_ref()), Some("echo"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_list_tools_filters_cli_tools_by_denylist() {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(vec![]),
            DenylistFilter::new(vec!["cli-cat".to_string()]),
        );
        let (client, upstream_h, proxy_h) =
            create_proxy_client_with_filter(filter, Some(make_cli_executor())).await;
        let result = client.list_tools(None).await.unwrap();
        // Upstream "echo" allowed, CLI "cli-cat" denied
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools.first().map(|t| t.name.as_ref()), Some("echo"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn proxy_call_cli_tool_blocked_by_denylist() {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(vec![]),
            DenylistFilter::new(vec!["cli-cat".to_string()]),
        );
        let (client, upstream_h, proxy_h) =
            create_proxy_client_with_filter(filter, Some(make_cli_executor())).await;
        let params = CallToolRequestParams {
            name: "cli-cat".into(),
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

    #[tokio::test]
    async fn proxy_call_upstream_when_cli_present() {
        let (client, upstream_h, proxy_h) = create_proxy_client_with_cli_tools(
            AllowlistFilter::new(vec![]),
            Some(make_cli_executor()),
        )
        .await;
        // Call the upstream "echo" tool even though CLI tools are present
        let params = CallToolRequestParams {
            name: "echo".into(),
            arguments: Some(serde_json::from_str(r#"{"message":"upstream"}"#).unwrap()),
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await.unwrap();
        let text = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str());
        assert_eq!(text, Some("upstream"));
        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }
}
