#[allow(clippy::unwrap_used, clippy::expect_used)]
mod proxy_stress {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use mcp_gateway::adapters::driven::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};
    use mcp_gateway::adapters::driven::{NullCliRunner, ProcessCliRunner, RmcpUpstreamClient};
    use mcp_gateway::adapters::driving::McpAdapter;
    use mcp_gateway::config::model::CliToolDef;
    use mcp_gateway::hexagon::usecases::gateway::{Gateway, UpstreamEntry};
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
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
                .with_server_info(Implementation::new("echo-stress", "0.1.0"))
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
            Ok(ListToolsResult::with_all_items(vec![Tool::new(
                "echo",
                "echoes input",
                schema,
            )]))
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
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
                .with_server_info(Implementation::new("slow-stress", "0.1.0"))
        }

        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: RequestContext<RoleServer>,
        ) -> Result<ListToolsResult, ErrorData> {
            tokio::time::sleep(self.delay).await;
            let schema: serde_json::Map<String, serde_json::Value> =
                serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                }))
                .expect("static schema");
            Ok(ListToolsResult::with_all_items(vec![Tool::new(
                "echo",
                "echoes input",
                schema,
            )]))
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
        setup_proxy_with_timeout(server, None).await
    }

    async fn setup_proxy_with_timeout<S: ServerHandler + 'static>(
        server: S,
        timeout: Option<Duration>,
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

        let mut client = RmcpUpstreamClient::new(upstream_client);
        if let Some(t) = timeout {
            client = client.with_operation_timeout(t);
        }

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "srv".to_string(),
            UpstreamEntry {
                client,
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let adapter = McpAdapter::new(gateway);

        // downstream: proxy server <-> test client
        let (proxy_server_transport, proxy_client_transport) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let s = adapter.serve(proxy_server_transport).await.unwrap();
            let _ = s.waiting().await;
        });

        let client = ().serve(proxy_client_transport).await.unwrap();
        (client, upstream_handle, proxy_handle)
    }

    fn echo_params(msg: &str) -> CallToolRequestParams {
        CallToolRequestParams::new("srv__echo")
            .with_arguments(serde_json::from_value(serde_json::json!({"message": msg})).unwrap())
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
    async fn upstream_crash_then_list_tools_returns_empty_gracefully() {
        let (client, upstream_h, _proxy_h) = setup_proxy().await;

        upstream_h.abort();
        let _ = upstream_h.await;

        let result = client.list_tools(None).await.unwrap();
        assert!(
            result.tools.is_empty(),
            "crashed upstream should be skipped"
        );
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
    async fn slow_upstream_list_tools_skipped_on_timeout() {
        let (client, upstream_h, proxy_h) = setup_proxy_with_timeout(
            SlowServer {
                delay: Duration::from_secs(5),
            },
            Some(Duration::from_millis(50)),
        )
        .await;

        let result = client.list_tools(None).await.unwrap();
        assert!(
            result.tools.is_empty(),
            "timed-out upstream should be skipped"
        );

        drop(client);
        let _ = proxy_h.await;
        upstream_h.abort();
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn slow_upstream_call_tool_returns_timeout_error() {
        let (client, upstream_h, proxy_h) = setup_proxy_with_timeout(
            SlowServer {
                delay: Duration::from_secs(5),
            },
            Some(Duration::from_millis(50)),
        )
        .await;

        let result = client.call_tool(echo_params("timeout")).await;
        assert!(result.is_err(), "call_tool should fail on timeout");

        drop(client);
        let _ = proxy_h.await;
        upstream_h.abort();
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

        let params = CallToolRequestParams::new("");
        let result = client.call_tool(params).await;
        assert!(result.is_err());

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }

    #[tokio::test]
    async fn call_tool_with_null_arguments_returns_default() {
        let (client, upstream_h, proxy_h) = setup_proxy().await;

        let params = CallToolRequestParams::new("srv__echo");
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
                client: RmcpUpstreamClient::new(upstream_client),
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let adapter = McpAdapter::new(gateway);

        let (proxy_server_t, mut proxy_client_t) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let _ = adapter.serve(proxy_server_t).await;
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
                client: RmcpUpstreamClient::new(upstream_client),
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let adapter = McpAdapter::new(gateway);

        let (proxy_server_t, mut proxy_client_t) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let _ = adapter.serve(proxy_server_t).await;
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

    fn make_cli_runner(name: &str, command: &str) -> ProcessCliRunner {
        let mut tools = BTreeMap::new();
        tools.insert(
            name.to_string(),
            CliToolDef {
                command: command.to_string(),
                description: Some(format!("Stress test tool: {command}")),
            },
        );
        ProcessCliRunner::new(tools)
    }

    async fn setup_proxy_with_cli(
        cli: ProcessCliRunner,
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
                client: RmcpUpstreamClient::new(upstream_client),
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, cli);
        let adapter = McpAdapter::new(gateway);

        let (proxy_server_t, proxy_client_t) = tokio::io::duplex(4096);

        let proxy_handle = tokio::spawn(async move {
            let s = adapter.serve(proxy_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        let client = ().serve(proxy_client_t).await.unwrap();
        (client, upstream_handle, proxy_handle)
    }

    #[tokio::test]
    async fn concurrent_cli_tool_calls_succeed() {
        let cli = make_cli_runner("cat-tool", "cat");
        let (client, upstream_h, proxy_h) = setup_proxy_with_cli(cli).await;
        let client = std::sync::Arc::new(client);

        let mut set = tokio::task::JoinSet::new();
        for i in 0..20 {
            let c = client.clone();
            set.spawn(async move {
                let params = CallToolRequestParams::new("cat-tool")
                    .with_arguments(serde_json::from_value(serde_json::json!({"idx": i})).unwrap());
                let result = c.call_tool(params).await.unwrap();
                let text = extract_text(&result);
                assert!(text.contains("idx"), "response should contain arguments");
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
        let cli = make_cli_runner("false-tool", "false");
        let (client, upstream_h, proxy_h) = setup_proxy_with_cli(cli).await;
        let client = std::sync::Arc::new(client);

        let mut set = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let c = client.clone();
            set.spawn(async move {
                let params = CallToolRequestParams::new("false-tool");
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

    #[tokio::test]
    async fn cli_proxy_also_routes_upstream_and_error_paths() {
        let cli = make_cli_runner("cat-tool", "cat");
        let (client, upstream_h, proxy_h) = setup_proxy_with_cli(cli).await;

        // List tools through the proxy (exercises list_tools with CLI runner)
        let tools = client.list_tools(None).await.unwrap();
        assert!(tools.tools.len() >= 2);

        // Call upstream tool through the proxy (exercises non-CLI path)
        let result = client.call_tool(echo_params("via-upstream")).await.unwrap();
        assert_eq!(extract_text(&result), "via-upstream");

        // Exercise no-prefix error
        let result = client
            .call_tool(CallToolRequestParams::new("no_prefix"))
            .await;
        assert!(result.is_err());

        // Exercise unknown-server error
        let result = client
            .call_tool(CallToolRequestParams::new("unknown__tool"))
            .await;
        assert!(result.is_err());

        drop(client);
        let _ = proxy_h.await;
        let _ = upstream_h.await;
    }
}
