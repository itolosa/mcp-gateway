#[allow(clippy::unwrap_used, clippy::expect_used)]
mod proxy_http_e2e {
    use std::sync::Arc;

    use rmcp::model::{
        CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    };
    use rmcp::service::RequestContext;
    use rmcp::transport::child_process::TokioChildProcess;
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };
    use rmcp::{RoleServer, ServerHandler, ServiceExt};
    use tokio_util::sync::CancellationToken;

    const GATEWAY_BIN: &str = env!("CARGO_BIN_EXE_mcp-gateway");

    struct EchoServer;

    impl ServerHandler for EchoServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
                .with_server_info(Implementation::new("echo-test", "0.1.0"))
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

    async fn spawn_http_echo_server() -> (String, CancellationToken) {
        let ct = CancellationToken::new();
        let config = StreamableHttpServerConfig {
            cancellation_token: ct.clone(),
            ..Default::default()
        };
        let service: StreamableHttpService<EchoServer, LocalSessionManager> =
            StreamableHttpService::new(
                || Ok(EchoServer),
                Arc::new(LocalSessionManager::default()),
                config,
            );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ct_clone = ct.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { ct_clone.cancelled_owned().await })
                .await;
        });
        (format!("http://127.0.0.1:{}/mcp", addr.port()), ct)
    }

    async fn spawn_gateway_http_proxy() -> (
        rmcp::service::RunningService<rmcp::RoleClient, ()>,
        tempfile::TempDir,
        CancellationToken,
    ) {
        let (url, ct) = spawn_http_echo_server().await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_str = config_path.to_str().unwrap();

        // Register the HTTP echo server via CLI
        let status = std::process::Command::new(GATEWAY_BIN)
            .args([
                "-c",
                config_str,
                "add",
                "echo-http",
                "-t",
                "http",
                "--url",
                &url,
            ])
            .status()
            .unwrap();
        assert!(status.success());

        // Spawn the gateway proxy as a child process (no server name — runs all)
        let mut cmd = tokio::process::Command::new(GATEWAY_BIN);
        cmd.args(["-c", config_str, "run"]);

        let transport = TokioChildProcess::new(cmd).unwrap();
        let client = ().serve(transport).await.unwrap();

        (client, dir, ct)
    }

    #[tokio::test]
    async fn http_proxy_e2e_forwards_list_tools() {
        let (client, _dir, ct) = spawn_gateway_http_proxy().await;

        let result = client.list_tools(None).await.unwrap();
        assert_eq!(result.tools.len(), 1);
        // Tool is prefixed with server name
        assert_eq!(
            result.tools.first().map(|t| t.name.as_ref()),
            Some("echo-http__echo")
        );

        drop(client);
        ct.cancel();
    }

    #[tokio::test]
    async fn http_proxy_e2e_forwards_call_tool() {
        let (client, _dir, ct) = spawn_gateway_http_proxy().await;

        let params = CallToolRequestParams::new("echo-http__echo")
            .with_arguments(serde_json::from_str(r#"{"message":"hello-http"}"#).unwrap());
        let result = client.call_tool(params).await.unwrap();
        let text = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str());
        assert_eq!(text, Some("hello-http"));

        drop(client);
        ct.cancel();
    }

    #[tokio::test]
    async fn http_proxy_e2e_returns_gateway_identity() {
        let (client, _dir, ct) = spawn_gateway_http_proxy().await;

        let info = client.peer_info().unwrap();
        assert_eq!(info.server_info.name, "mcp-gateway");
        assert!(info.capabilities.tools.is_some());

        drop(client);
        ct.cancel();
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used)]
mod proxy_http_negative {
    use rmcp::transport::child_process::TokioChildProcess;
    use rmcp::ServiceExt;
    use tokio_util::sync::CancellationToken;

    const GATEWAY_BIN: &str = env!("CARGO_BIN_EXE_mcp-gateway");
    const ECHO_SERVER_BIN: &str = env!("CARGO_BIN_EXE_echo-mcp-server");

    fn register_server(config_str: &str, name: &str, transport: &str, target_args: &[&str]) {
        let mut args = vec!["-c", config_str, "add", name, "-t", transport];
        args.extend_from_slice(target_args);
        let status = std::process::Command::new(GATEWAY_BIN)
            .args(&args)
            .status()
            .unwrap();
        assert!(status.success());
    }

    async fn spawn_gateway(
        config_str: &str,
    ) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
        let mut cmd = tokio::process::Command::new(GATEWAY_BIN);
        cmd.args(["-c", config_str, "run"]);
        let transport = TokioChildProcess::new(cmd).unwrap();
        ().serve(transport).await.unwrap()
    }

    /// Spawn an HTTP server that always returns 401 Unauthorized.
    async fn spawn_auth_rejecting_server() -> (String, CancellationToken) {
        let ct = CancellationToken::new();
        let router = axum::Router::new().fallback(|| async {
            (
                axum::http::StatusCode::UNAUTHORIZED,
                "Unauthorized: invalid credentials",
            )
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ct_clone = ct.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { ct_clone.cancelled_owned().await })
                .await;
        });
        (format!("http://127.0.0.1:{}/mcp", addr.port()), ct)
    }

    /// Spawn an HTTP server that returns HTML (not MCP protocol).
    async fn spawn_non_mcp_server() -> (String, CancellationToken) {
        let ct = CancellationToken::new();
        let router = axum::Router::new().fallback(|| async {
            axum::response::Html("<html><body>Not an MCP server</body></html>")
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ct_clone = ct.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { ct_clone.cancelled_owned().await })
                .await;
        });
        (format!("http://127.0.0.1:{}/mcp", addr.port()), ct)
    }

    /// Spawn an HTTP server that always returns 500 Internal Server Error.
    async fn spawn_error_server() -> (String, CancellationToken) {
        let ct = CancellationToken::new();
        let router = axum::Router::new().fallback(|| async {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
            )
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ct_clone = ct.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { ct_clone.cancelled_owned().await })
                .await;
        });
        (format!("http://127.0.0.1:{}/mcp", addr.port()), ct)
    }

    // Scenario: HTTP upstream rejects authentication (401).
    // Gateway should skip it and serve remaining upstreams.
    #[tokio::test]
    async fn http_upstream_auth_rejection_is_skipped() {
        let (url, ct) = spawn_auth_rejecting_server().await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_str = config_path.to_str().unwrap();

        // Register a valid stdio server and the auth-rejecting HTTP server
        register_server(config_str, "good", "stdio", &["--command", ECHO_SERVER_BIN]);
        register_server(config_str, "auth-reject", "http", &["--url", &url]);

        // When the gateway runs
        let client = spawn_gateway(config_str).await;

        // Then only the valid server's tools are available
        let result = client.list_tools(None).await.unwrap();
        let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(tool_names, ["good__echo"]);

        drop(client);
        ct.cancel();
    }

    // Scenario: HTTP upstream returns non-MCP content (HTML).
    // Gateway should skip it and serve remaining upstreams.
    #[tokio::test]
    async fn http_upstream_non_mcp_protocol_is_skipped() {
        let (url, ct) = spawn_non_mcp_server().await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_str = config_path.to_str().unwrap();

        register_server(config_str, "good", "stdio", &["--command", ECHO_SERVER_BIN]);
        register_server(config_str, "not-mcp", "http", &["--url", &url]);

        // When the gateway runs
        let client = spawn_gateway(config_str).await;

        // Then only the valid server's tools are available
        let result = client.list_tools(None).await.unwrap();
        let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(tool_names, ["good__echo"]);

        drop(client);
        ct.cancel();
    }

    // Scenario: HTTP upstream returns 500 errors.
    // Gateway should skip it and serve remaining upstreams.
    #[tokio::test]
    async fn http_upstream_server_error_is_skipped() {
        let (url, ct) = spawn_error_server().await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_str = config_path.to_str().unwrap();

        register_server(config_str, "good", "stdio", &["--command", ECHO_SERVER_BIN]);
        register_server(config_str, "erroring", "http", &["--url", &url]);

        // When the gateway runs
        let client = spawn_gateway(config_str).await;

        // Then only the valid server's tools are available
        let result = client.list_tools(None).await.unwrap();
        let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(tool_names, ["good__echo"]);

        drop(client);
        ct.cancel();
    }

    // Scenario: All HTTP upstreams fail. Gateway starts with no tools.
    #[tokio::test]
    async fn all_http_upstreams_failing_results_in_empty_tools() {
        let (url1, ct1) = spawn_auth_rejecting_server().await;
        let (url2, ct2) = spawn_error_server().await;
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_str = config_path.to_str().unwrap();

        register_server(config_str, "reject", "http", &["--url", &url1]);
        register_server(config_str, "error", "http", &["--url", &url2]);

        // When the gateway runs
        let client = spawn_gateway(config_str).await;

        // Then no tools are available
        let result = client.list_tools(None).await.unwrap();
        assert!(result.tools.is_empty());

        drop(client);
        ct1.cancel();
        ct2.cancel();
    }

    // Scenario: HTTP upstream at a valid server but wrong path (404).
    #[tokio::test]
    async fn http_upstream_wrong_path_is_skipped() {
        // Use the auth-rejecting server but point to /wrong-path
        let ct = CancellationToken::new();
        let router = axum::Router::new()
            .fallback(|| async { (axum::http::StatusCode::NOT_FOUND, "Not Found") });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ct_clone = ct.clone();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { ct_clone.cancelled_owned().await })
                .await;
        });
        let url = format!("http://127.0.0.1:{}/wrong-path", addr.port());

        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_str = config_path.to_str().unwrap();

        register_server(config_str, "good", "stdio", &["--command", ECHO_SERVER_BIN]);
        register_server(config_str, "wrong-path", "http", &["--url", &url]);

        // When the gateway runs
        let client = spawn_gateway(config_str).await;

        // Then only the valid server's tools are available
        let result = client.list_tools(None).await.unwrap();
        let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert_eq!(tool_names, ["good__echo"]);

        drop(client);
        ct.cancel();
    }
}

#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod proxy_e2e {
    use rmcp::model::{GetPromptRequestParams, ReadResourceRequestParams, ResourceContents};
    use rmcp::transport::child_process::TokioChildProcess;
    use rmcp::ServiceExt;

    const GATEWAY_BIN: &str = env!("CARGO_BIN_EXE_mcp-gateway");
    const ECHO_SERVER_BIN: &str = env!("CARGO_BIN_EXE_echo-mcp-server");

    /// Register the echo server and spawn `mcp-gateway run` as a child process,
    /// returning an rmcp client connected to the proxy.
    async fn spawn_gateway_proxy() -> (
        rmcp::service::RunningService<rmcp::RoleClient, ()>,
        tempfile::TempDir,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");
        let config_str = config_path.to_str().unwrap();

        // Register the echo server via CLI
        let status = std::process::Command::new(GATEWAY_BIN)
            .args([
                "-c",
                config_str,
                "add",
                "echo",
                "-t",
                "stdio",
                "--command",
                ECHO_SERVER_BIN,
            ])
            .status()
            .unwrap();
        assert!(status.success());

        // Spawn the gateway proxy as a child process (no server name — runs all)
        let mut cmd = tokio::process::Command::new(GATEWAY_BIN);
        cmd.args(["-c", config_str, "run"]);

        let transport = TokioChildProcess::new(cmd).unwrap();
        let client = ().serve(transport).await.unwrap();

        (client, dir)
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_list_tools_through_gateway() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let result = client.list_tools(None).await.unwrap();
        assert_eq!(result.tools.len(), 1);
        // Tool is prefixed with server name
        assert_eq!(
            result.tools.first().map(|t| t.name.as_ref()),
            Some("echo__echo")
        );
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_call_tool_through_gateway() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let params = rmcp::model::CallToolRequestParams::new("echo__echo")
            .with_arguments(serde_json::from_str(r#"{"message":"hello"}"#).unwrap());
        let result = client.call_tool(params).await.unwrap();
        let text = result
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|t| t.text.as_str());
        assert_eq!(text, Some("hello"));
    }

    #[tokio::test]
    async fn proxy_e2e_returns_gateway_identity() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let info = client.peer_info().unwrap();
        assert_eq!(info.server_info.name, "mcp-gateway");
        assert!(info.capabilities.tools.is_some());
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_list_resources() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let result = client.list_resources(None).await.unwrap();
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].uri, "echo__file:///hello.txt");
        assert_eq!(result.resources[0].name, "echo__hello.txt");
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_list_resource_templates() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let result = client.list_resource_templates(None).await.unwrap();
        assert_eq!(result.resource_templates.len(), 1);
        assert_eq!(
            result.resource_templates[0].uri_template,
            "echo__file:///{path}"
        );
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_read_resource() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let params = ReadResourceRequestParams::new("echo__file:///hello.txt");
        let result = client.read_resource(params).await.unwrap();
        assert_eq!(result.contents.len(), 1);
        match &result.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => {
                assert!(text.contains("content of"));
            }
            _ => panic!("expected text resource contents"),
        }
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_list_prompts() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let result = client.list_prompts(None).await.unwrap();
        assert_eq!(result.prompts.len(), 1);
        assert_eq!(result.prompts[0].name, "echo__greet");
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_get_prompt() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let params = GetPromptRequestParams::new("echo__greet");
        let result = client.get_prompt(params).await.unwrap();
        assert!(!result.messages.is_empty());
    }
}
