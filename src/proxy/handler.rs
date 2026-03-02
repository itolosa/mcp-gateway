use std::borrow::Cow;
use std::collections::BTreeMap;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams, ServerInfo,
};
use rmcp::service::{RequestContext, RoleClient, RoleServer, RunningService, ServiceError};
use rmcp::{ErrorData, ServerHandler};

use crate::cli_tools::CliToolExecutor;
use crate::filter::{AllowlistFilter, CompoundFilter, DenylistFilter, ToolFilter};
use crate::proxy::prefix::{prefix_tool_name, split_prefixed_name};

pub struct UpstreamEntry {
    pub service: RunningService<RoleClient, ()>,
    pub filter: CompoundFilter<AllowlistFilter, DenylistFilter>,
}

pub struct ProxyHandler {
    upstreams: BTreeMap<String, UpstreamEntry>,
    cli_tools: Option<CliToolExecutor>,
}

impl ProxyHandler {
    pub fn new(
        upstreams: BTreeMap<String, UpstreamEntry>,
        cli_tools: Option<CliToolExecutor>,
    ) -> Self {
        Self {
            upstreams,
            cli_tools,
        }
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
        ServerInfo {
            capabilities: rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: rmcp::model::Implementation {
                name: "mcp-gateway".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
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
        let mut all_tools = Vec::new();
        for (name, entry) in &self.upstreams {
            let result = entry
                .service
                .list_tools(None)
                .await
                .map_err(service_error_to_mcp)?;
            for mut tool in result.tools {
                if entry.filter.is_tool_allowed(tool.name.as_ref()) {
                    tool.name = Cow::Owned(prefix_tool_name(name, tool.name.as_ref()));
                    all_tools.push(tool);
                }
            }
        }
        if let Some(cli) = &self.cli_tools {
            all_tools.extend(cli.list_tools());
        }
        Ok(ListToolsResult {
            tools: all_tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Some(cli) = &self.cli_tools {
            if cli.has_tool(request.name.as_ref()) {
                return cli.call_tool(&request).await;
            }
        }
        let (server_name, raw_tool) =
            split_prefixed_name(request.name.as_ref()).ok_or_else(|| {
                ErrorData::invalid_params(
                    format!("tool '{}' has no server prefix", request.name),
                    None,
                )
            })?;
        let entry = self.upstreams.get(server_name).ok_or_else(|| {
            ErrorData::invalid_params(
                format!("unknown server '{server_name}' in tool '{}'", request.name),
                None,
            )
        })?;
        if !entry.filter.is_tool_allowed(raw_tool) {
            return Err(ErrorData::invalid_params(
                format!("tool '{}' is not allowed", request.name),
                None,
            ));
        }
        let upstream_request = CallToolRequestParams {
            name: raw_tool.to_string().into(),
            arguments: request.arguments,
            meta: request.meta,
            task: request.task,
        };
        entry
            .service
            .call_tool(upstream_request)
            .await
            .map_err(service_error_to_mcp)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::model::CliToolDef;
    use rmcp::model::{
        CallToolResult, Content, Implementation, ListToolsResult, ServerCapabilities, ServerInfo,
        Tool,
    };
    use rmcp::ServiceExt;
    use serde_json::json;

    struct MockServerA;

    impl ServerHandler for MockServerA {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                server_info: Implementation {
                    name: "mock-a".to_string(),
                    version: "1.0.0".to_string(),
                    ..Default::default()
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

    struct MockServerB;

    impl ServerHandler for MockServerB {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                server_info: Implementation {
                    name: "mock-b".to_string(),
                    version: "1.0.0".to_string(),
                    ..Default::default()
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
                        "path": { "type": "string" }
                    }
                }))
                .unwrap();
            Ok(ListToolsResult {
                tools: vec![Tool::new("read_file", "reads a file", schema)],
                next_cursor: None,
                meta: None,
            })
        }

        async fn call_tool(
            &self,
            request: CallToolRequestParams,
            _context: RequestContext<RoleServer>,
        ) -> Result<CallToolResult, ErrorData> {
            if request.name.as_ref() == "read_file" {
                let path = request
                    .arguments
                    .and_then(|a| a.get("path").cloned())
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "content of {path}"
                ))]))
            } else {
                Err(ErrorData::invalid_params(
                    format!("unknown tool: {}", request.name),
                    None,
                ))
            }
        }
    }

    fn passthrough_filter() -> CompoundFilter<AllowlistFilter, DenylistFilter> {
        CompoundFilter::new(AllowlistFilter::new(vec![]), DenylistFilter::new(vec![]))
    }

    async fn connect_upstream<S: ServerHandler + 'static>(
        server: S,
    ) -> (RunningService<RoleClient, ()>, tokio::task::JoinHandle<()>) {
        let (server_t, client_t) = tokio::io::duplex(4096);
        let handle = tokio::spawn(async move {
            let s = server.serve(server_t).await.unwrap();
            let _ = s.waiting().await;
        });
        let client = ().serve(client_t).await.unwrap();
        (client, handle)
    }

    async fn create_multi_proxy_client(
        upstreams: BTreeMap<String, UpstreamEntry>,
        cli_tools: Option<CliToolExecutor>,
    ) -> (RunningService<RoleClient, ()>, tokio::task::JoinHandle<()>) {
        let proxy = ProxyHandler::new(upstreams, cli_tools);
        let (proxy_server_t, proxy_client_t) = tokio::io::duplex(4096);
        let proxy_handle = tokio::spawn(async move {
            let s = proxy.serve(proxy_server_t).await.unwrap();
            let _ = s.waiting().await;
        });
        let client = ().serve(proxy_client_t).await.unwrap();
        (client, proxy_handle)
    }

    /// Build a BTreeMap with two mock upstreams (alpha -> MockServerA, beta -> MockServerB)
    /// and return the join handles for cleanup.
    async fn two_server_setup() -> (
        BTreeMap<String, UpstreamEntry>,
        Vec<tokio::task::JoinHandle<()>>,
    ) {
        let (client_a, h_a) = connect_upstream(MockServerA).await;
        let (client_b, h_b) = connect_upstream(MockServerB).await;
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                service: client_a,
                filter: passthrough_filter(),
            },
        );
        upstreams.insert(
            "beta".to_string(),
            UpstreamEntry {
                service: client_b,
                filter: passthrough_filter(),
            },
        );
        (upstreams, vec![h_a, h_b])
    }

    // --- service_error_to_mcp ---

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

    // --- get_info ---

    #[tokio::test]
    async fn get_info_returns_gateway_identity() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let info = client.peer_info().unwrap();
        assert_eq!(info.server_info.name, "mcp-gateway");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some());
        drop(client);
        let _ = proxy_h.await;
        for h in handles {
            let _ = h.await;
        }
    }

    // --- list_tools ---

    #[tokio::test]
    async fn list_tools_returns_prefixed_tools_from_all_upstreams() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let result = client.list_tools(None).await.unwrap();
        let names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha__echo"));
        assert!(names.contains(&"beta__read_file"));
        drop(client);
        let _ = proxy_h.await;
        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn list_tools_with_no_upstreams_returns_empty() {
        let (client, proxy_h) = create_multi_proxy_client(BTreeMap::new(), None).await;
        let result = client.list_tools(None).await.unwrap();
        assert!(result.tools.is_empty());
        drop(client);
        let _ = proxy_h.await;
    }

    #[tokio::test]
    async fn list_tools_applies_per_server_filter() {
        let (client_a, h_a) = connect_upstream(MockServerA).await;
        let (client_b, h_b) = connect_upstream(MockServerB).await;
        let mut upstreams = BTreeMap::new();
        // alpha: block "echo" via allowlist that only allows "nonexistent"
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                service: client_a,
                filter: CompoundFilter::new(
                    AllowlistFilter::new(vec!["nonexistent".to_string()]),
                    DenylistFilter::new(vec![]),
                ),
            },
        );
        // beta: passthrough
        upstreams.insert(
            "beta".to_string(),
            UpstreamEntry {
                service: client_b,
                filter: passthrough_filter(),
            },
        );
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let result = client.list_tools(None).await.unwrap();
        let names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names, vec!["beta__read_file"]);
        drop(client);
        let _ = proxy_h.await;
        let _ = h_a.await;
        let _ = h_b.await;
    }

    #[tokio::test]
    async fn list_tools_applies_denylist_filter() {
        let (client_a, h_a) = connect_upstream(MockServerA).await;
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                service: client_a,
                filter: CompoundFilter::new(
                    AllowlistFilter::new(vec![]),
                    DenylistFilter::new(vec!["echo".to_string()]),
                ),
            },
        );
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let result = client.list_tools(None).await.unwrap();
        assert!(result.tools.is_empty());
        drop(client);
        let _ = proxy_h.await;
        let _ = h_a.await;
    }

    // --- call_tool ---

    #[tokio::test]
    async fn call_tool_routes_to_correct_upstream() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;

        // Call alpha__echo
        let params = CallToolRequestParams {
            name: "alpha__echo".into(),
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

        // Call beta__read_file
        let params = CallToolRequestParams {
            name: "beta__read_file".into(),
            arguments: Some(serde_json::from_str(r#"{"path":"/etc/hosts"}"#).unwrap()),
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await.unwrap();
        let text = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str());
        assert_eq!(text, Some("content of /etc/hosts"));

        drop(client);
        let _ = proxy_h.await;
        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn call_tool_without_prefix_returns_error() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
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
        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn call_tool_unknown_server_returns_error() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let params = CallToolRequestParams {
            name: "unknown__echo".into(),
            arguments: None,
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await;
        assert!(result.is_err());
        drop(client);
        let _ = proxy_h.await;
        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn call_tool_blocked_by_filter_returns_error() {
        let (client_a, h_a) = connect_upstream(MockServerA).await;
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                service: client_a,
                filter: CompoundFilter::new(
                    AllowlistFilter::new(vec![]),
                    DenylistFilter::new(vec!["echo".to_string()]),
                ),
            },
        );
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let params = CallToolRequestParams {
            name: "alpha__echo".into(),
            arguments: None,
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await;
        assert!(result.is_err());
        drop(client);
        let _ = proxy_h.await;
        let _ = h_a.await;
    }

    #[tokio::test]
    async fn call_unknown_tool_on_upstream_returns_error() {
        let (client_a, h_a) = connect_upstream(MockServerA).await;
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                service: client_a,
                filter: passthrough_filter(),
            },
        );
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let params = CallToolRequestParams {
            name: "alpha__nonexistent".into(),
            arguments: None,
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await;
        assert!(result.is_err());
        drop(client);
        let _ = proxy_h.await;
        let _ = h_a.await;
    }

    #[tokio::test]
    async fn call_unknown_tool_on_beta_upstream_returns_error() {
        let (client_b, h_b) = connect_upstream(MockServerB).await;
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "beta".to_string(),
            UpstreamEntry {
                service: client_b,
                filter: passthrough_filter(),
            },
        );
        let (client, proxy_h) = create_multi_proxy_client(upstreams, None).await;
        let params = CallToolRequestParams {
            name: "beta__nonexistent".into(),
            arguments: None,
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await;
        assert!(result.is_err());
        drop(client);
        let _ = proxy_h.await;
        let _ = h_b.await;
    }

    // --- CLI tools ---

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
    async fn list_tools_includes_cli_tools_unprefixed() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) =
            create_multi_proxy_client(upstreams, Some(make_cli_executor())).await;
        let result = client.list_tools(None).await.unwrap();
        let names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"alpha__echo"));
        assert!(names.contains(&"beta__read_file"));
        assert!(names.contains(&"cli-cat"));
        drop(client);
        let _ = proxy_h.await;
        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn call_cli_tool_routes_to_executor() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) =
            create_multi_proxy_client(upstreams, Some(make_cli_executor())).await;
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
        assert!(text.contains("cli-cat"));
        drop(client);
        let _ = proxy_h.await;
        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn call_upstream_tool_when_cli_present() {
        let (upstreams, handles) = two_server_setup().await;
        let (client, proxy_h) =
            create_multi_proxy_client(upstreams, Some(make_cli_executor())).await;
        let params = CallToolRequestParams {
            name: "alpha__echo".into(),
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
        for h in handles {
            let _ = h.await;
        }
    }

    #[tokio::test]
    async fn cli_tools_only_no_upstreams() {
        let (client, proxy_h) =
            create_multi_proxy_client(BTreeMap::new(), Some(make_cli_executor())).await;
        let result = client.list_tools(None).await.unwrap();
        assert_eq!(result.tools.len(), 1);
        assert_eq!(
            result.tools.first().map(|t| t.name.as_ref()),
            Some("cli-cat")
        );
        drop(client);
        let _ = proxy_h.await;
    }
}
