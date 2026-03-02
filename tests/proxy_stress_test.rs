#[allow(clippy::unwrap_used, clippy::expect_used)]
mod proxy_stress {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use mcp_gateway::cli_tools::CliToolExecutor;
    use mcp_gateway::config::model::CliToolDef;
    use mcp_gateway::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};
    use mcp_gateway::proxy::handler::{ProxyHandler, UpstreamEntry};
    use rmcp::model::{
        CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    };
    use rmcp::service::{RequestContext, RoleClient, RunningService};
    use rmcp::{RoleServer, ServerHandler, ServiceExt};

    // ── Mock Servers ──────────────────────────────────────────────────

    struct EchoServer;

    impl ServerHandler for EchoServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                server_info: Implementation {
                    name: "echo-stress".to_string(),
                    version: "0.1.0".to_string(),
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
                serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                }))
                .expect("static schema");
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

    struct SlowServer {
        delay: Duration,
    }

    impl ServerHandler for SlowServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                server_info: Implementation {
                    name: "slow-stress".to_string(),
                    version: "0.1.0".to_string(),
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
                serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                }))
                .expect("static schema");
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
            tokio::time::sleep(self.delay).await;
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

    // ── Helpers ────────────────────────────────────────────────────────

    fn passthrough_filter() -> CompoundFilter<AllowlistFilter, DenylistFilter> {
        CompoundFilter::new(AllowlistFilter::new(vec![]), DenylistFilter::new(vec![]))
    }

    async fn setup_proxy() -> (
        RunningService<RoleClient, ()>,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
    ) {
        setup_proxy_with_server(EchoServer).await
    }

    async fn setup_proxy_with_server<S: ServerHandler + 'static>(
        server: S,
    ) -> (
        RunningService<RoleClient, ()>,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
    ) {
        // upstream: mock server <-> proxy's client
        let (upstream_server_transport, upstream_client_transport) = tokio::io::duplex(4096);

        let upstream_handle = tokio::spawn(async move {
            let s = server.serve(upstream_server_transport).await.unwrap();
            let _ = s.waiting().await;
        });

        let upstream_client = ().serve(upstream_client_transport).await.unwrap();

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "srv".to_string(),
            UpstreamEntry {
                service: upstream_client,
                filter: passthrough_filter(),
            },
        );
        let proxy = ProxyHandler::new(upstreams, None);

        // downstream: proxy server <-> test client
        let (proxy_server_transport, proxy_client_transport) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let s = proxy.serve(proxy_server_transport).await.unwrap();
            let _ = s.waiting().await;
        });

        let client = ().serve(proxy_client_transport).await.unwrap();
        (client, upstream_handle, proxy_handle)
    }

    fn echo_params(msg: &str) -> CallToolRequestParams {
        CallToolRequestParams {
            name: "srv__echo".into(),
            arguments: Some(serde_json::from_value(serde_json::json!({"message": msg})).unwrap()),
            meta: None,
            task: None,
        }
    }

    fn extract_text(result: &CallToolResult) -> &str {
        result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str())
            .unwrap_or("")
    }

    // ── Group 1: Concurrent Load ──────────────────────────────────────

    #[tokio::test]
    async fn concurrent_tool_calls_all_succeed() {
        let (client, upstream_h, proxy_h) = setup_proxy().await;
        let client = std::sync::Arc::new(client);

        let mut set = tokio::task::JoinSet::new();
        for i in 0..50 {
            let c = client.clone();
            set.spawn(async move {
                let msg = format!("msg-{i}");
                let result = c.call_tool(echo_params(&msg)).await.unwrap();
                assert_eq!(extract_text(&result), msg);
            });
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn concurrent_list_and_call_interleaved() {
        let (client, upstream_h, proxy_h) = setup_proxy().await;
        let client = std::sync::Arc::new(client);

        let mut set = tokio::task::JoinSet::new();
        for i in 0..25 {
            let c = client.clone();
            set.spawn(async move {
                let result = c.list_tools(None).await.unwrap();
                assert_eq!(result.tools.len(), 1);
                assert_eq!(
                    result.tools.first().map(|t| t.name.as_ref()),
                    Some("srv__echo")
                );
                // suppress unused variable warning
                let _ = i;
            });
        }
        for i in 0..25 {
            let c = client.clone();
            set.spawn(async move {
                let msg = format!("call-{i}");
                let result = c.call_tool(echo_params(&msg)).await.unwrap();
                assert_eq!(extract_text(&result), msg);
            });
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    // ── Group 2: Concurrent Connections ───────────────────────────────

    #[tokio::test]
    async fn multiple_independent_proxy_pipelines() {
        let mut set = tokio::task::JoinSet::new();
        for pipeline_idx in 0..5 {
            set.spawn(async move {
                let (client, upstream_h, proxy_h) = setup_proxy().await;
                let msg = format!("pipeline-{pipeline_idx}");
                let result = client.call_tool(echo_params(&msg)).await.unwrap();
                assert_eq!(extract_text(&result), msg);
                drop(client);
                let _ = proxy_h.await;
                let _ = upstream_h.await;
            });
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }
    }

    // ── Group 3: Upstream Crash ───────────────────────────────────────

    #[tokio::test]
    async fn upstream_crash_then_call_tool_returns_error() {
        let (client, upstream_h, _proxy_h) = setup_proxy().await;

        // Abort the upstream server to simulate a crash
        upstream_h.abort();
        let _ = upstream_h.await;

        let result = client.call_tool(echo_params("after-crash")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn upstream_crash_then_list_tools_returns_error() {
        let (client, upstream_h, _proxy_h) = setup_proxy().await;

        upstream_h.abort();
        let _ = upstream_h.await;

        let result = client.list_tools(None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn upstream_close_then_proxy_exits_cleanly() {
        let (client, upstream_h, proxy_h) = setup_proxy().await;

        // Abort upstream, then drop client — proxy should exit without panic
        upstream_h.abort();
        let _ = upstream_h.await;
        drop(client);

        // proxy_handle.await.unwrap() — if proxy panicked, JoinError::Panic propagates
        let result = proxy_h.await;
        assert!(result.is_ok(), "proxy task should not panic");
    }

    // ── Group 4: Slow Upstream ────────────────────────────────────────

    #[tokio::test]
    async fn slow_upstream_responds_eventually() {
        let (client, upstream_h, proxy_h) = setup_proxy_with_server(SlowServer {
            delay: Duration::from_millis(100),
        })
        .await;

        let result = tokio::time::timeout(Duration::from_secs(2), async {
            client.call_tool(echo_params("slow")).await
        })
        .await
        .expect("should not time out")
        .unwrap();

        assert_eq!(extract_text(&result), "slow");

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn client_disconnect_during_slow_upstream() {
        let (client, upstream_h, proxy_h) = setup_proxy_with_server(SlowServer {
            delay: Duration::from_secs(10),
        })
        .await;

        // Drop client immediately — proxy should not hang
        drop(client);

        let result = tokio::time::timeout(Duration::from_secs(2), proxy_h).await;
        assert!(
            result.is_ok(),
            "proxy should exit within 2s after client disconnect"
        );

        upstream_h.abort();
        let _ = upstream_h.await;
    }

    // ── Group 5: Edge-Case Input ──────────────────────────────────────

    #[tokio::test]
    async fn call_tool_with_empty_name_returns_error() {
        let (client, upstream_h, proxy_h) = setup_proxy().await;

        let params = CallToolRequestParams {
            name: "".into(),
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
    async fn call_tool_with_null_arguments_returns_default() {
        let (client, upstream_h, proxy_h) = setup_proxy().await;

        let params = CallToolRequestParams {
            name: "srv__echo".into(),
            arguments: None,
            meta: None,
            task: None,
        };
        let result = client.call_tool(params).await.unwrap();
        // EchoServer with None arguments returns empty string
        assert_eq!(extract_text(&result), "");

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn raw_invalid_bytes_do_not_crash_proxy() {
        use tokio::io::AsyncWriteExt;

        // Wire up upstream + proxy manually to get raw transport access
        let (upstream_server_t, upstream_client_t) = tokio::io::duplex(4096);

        let upstream_handle = tokio::spawn(async move {
            let s = EchoServer.serve(upstream_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        let upstream_client = ().serve(upstream_client_t).await.unwrap();

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "srv".to_string(),
            UpstreamEntry {
                service: upstream_client,
                filter: passthrough_filter(),
            },
        );
        let proxy = ProxyHandler::new(upstreams, None);

        let (proxy_server_t, mut proxy_client_t) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let _ = proxy.serve(proxy_server_t).await;
        });

        // Write garbage bytes
        let _ = proxy_client_t.write_all(b"\xff\xfe\xfd garbage").await;
        drop(proxy_client_t);

        // Proxy should not panic
        let result = tokio::time::timeout(Duration::from_secs(2), proxy_handle).await;
        assert!(result.is_ok(), "proxy should exit within timeout");
        let join_result = result.unwrap();
        assert!(join_result.is_ok(), "proxy task should not panic");

        upstream_handle.abort();
        let _ = upstream_handle.await;
    }

    #[tokio::test]
    async fn raw_incomplete_json_does_not_crash_proxy() {
        use tokio::io::AsyncWriteExt;

        let (upstream_server_t, upstream_client_t) = tokio::io::duplex(4096);

        let upstream_handle = tokio::spawn(async move {
            let s = EchoServer.serve(upstream_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        let upstream_client = ().serve(upstream_client_t).await.unwrap();

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "srv".to_string(),
            UpstreamEntry {
                service: upstream_client,
                filter: passthrough_filter(),
            },
        );
        let proxy = ProxyHandler::new(upstreams, None);

        let (proxy_server_t, mut proxy_client_t) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let _ = proxy.serve(proxy_server_t).await;
        });

        // Write incomplete JSON-RPC
        let _ = proxy_client_t
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"tools/list\"")
            .await;
        drop(proxy_client_t);

        let result = tokio::time::timeout(Duration::from_secs(2), proxy_handle).await;
        assert!(result.is_ok(), "proxy should exit within timeout");
        let join_result = result.unwrap();
        assert!(join_result.is_ok(), "proxy task should not panic");

        upstream_handle.abort();
        let _ = upstream_handle.await;
    }

    // ── Group 6: CLI Tool Stress ──────────────────────────────────────

    fn make_cli_executor(name: &str, command: &str) -> CliToolExecutor {
        let mut tools = BTreeMap::new();
        tools.insert(
            name.to_string(),
            CliToolDef {
                command: command.to_string(),
                description: Some(format!("Stress test tool: {command}")),
            },
        );
        CliToolExecutor::new(tools)
    }

    async fn setup_proxy_with_cli(
        cli: CliToolExecutor,
    ) -> (
        RunningService<RoleClient, ()>,
        tokio::task::JoinHandle<()>,
        tokio::task::JoinHandle<()>,
    ) {
        let (upstream_server_t, upstream_client_t) = tokio::io::duplex(4096);

        let upstream_handle = tokio::spawn(async move {
            let s = EchoServer.serve(upstream_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        let upstream_client = ().serve(upstream_client_t).await.unwrap();

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "srv".to_string(),
            UpstreamEntry {
                service: upstream_client,
                filter: passthrough_filter(),
            },
        );
        let proxy = ProxyHandler::new(upstreams, Some(cli));

        let (proxy_server_t, proxy_client_t) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let s = proxy.serve(proxy_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        let client = ().serve(proxy_client_t).await.unwrap();
        (client, upstream_handle, proxy_handle)
    }

    #[tokio::test]
    async fn concurrent_cli_tool_calls_succeed() {
        let cli = make_cli_executor("cat-tool", "cat");
        let (client, upstream_h, proxy_h) = setup_proxy_with_cli(cli).await;
        let client = std::sync::Arc::new(client);

        let mut set = tokio::task::JoinSet::new();
        for i in 0..20 {
            let c = client.clone();
            set.spawn(async move {
                let params = CallToolRequestParams {
                    name: "cat-tool".into(),
                    arguments: Some(serde_json::from_value(serde_json::json!({"idx": i})).unwrap()),
                    meta: None,
                    task: None,
                };
                let result = c.call_tool(params).await.unwrap();
                let text = extract_text(&result);
                assert!(
                    text.contains("cat-tool"),
                    "response should contain tool name"
                );
                assert!(!result.is_error.unwrap_or(false));
            });
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn concurrent_failing_cli_tool_calls_return_errors() {
        let cli = make_cli_executor("false-tool", "false");
        let (client, upstream_h, proxy_h) = setup_proxy_with_cli(cli).await;
        let client = std::sync::Arc::new(client);

        let mut set = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let c = client.clone();
            set.spawn(async move {
                let params = CallToolRequestParams {
                    name: "false-tool".into(),
                    arguments: None,
                    meta: None,
                    task: None,
                };
                let result = c.call_tool(params).await.unwrap();
                assert!(
                    result.is_error.unwrap_or(false),
                    "false should report error"
                );
            });
        }
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }
}
