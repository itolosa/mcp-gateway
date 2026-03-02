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

        // Spawn the gateway proxy as a child process
        let mut cmd = tokio::process::Command::new(GATEWAY_BIN);
        cmd.args(["-c", config_str, "run", "echo"]);

        let transport = TokioChildProcess::new(cmd).unwrap();
        let client = ().serve(transport).await.unwrap();

        (client, dir)
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_list_tools_through_gateway() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let result = client.list_tools(None).await.unwrap();
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools.first().map(|t| t.name.as_ref()), Some("echo"));
    }

    #[tokio::test]
    async fn proxy_e2e_forwards_call_tool_through_gateway() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let params = rmcp::model::CallToolRequestParams {
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
    }

    #[tokio::test]
    async fn proxy_e2e_returns_gateway_identity() {
        let (client, _dir) = spawn_gateway_proxy().await;

        let info = client.peer_info().unwrap();
        assert_eq!(info.server_info.name, "mcp-gateway");
        assert!(info.capabilities.tools.is_some());
    }
}
