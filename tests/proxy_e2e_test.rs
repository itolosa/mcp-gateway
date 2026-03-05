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
        cmd.args(["-c", config_str, "run", "--stdio"]);

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

#[allow(clippy::unwrap_used)]
mod proxy_e2e {
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
        cmd.args(["-c", config_str, "run", "--stdio"]);

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
}
