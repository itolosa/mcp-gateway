#![allow(clippy::unwrap_used, clippy::expect_used)]

use rmcp::model::CallToolRequestParams;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::ServiceExt;

const GATEWAY_BIN: &str = env!("CARGO_BIN_EXE_mcp-gateway");
const ECHO_SERVER_BIN: &str = env!("CARGO_BIN_EXE_echo-mcp-server");
const MULTI_ECHO_SERVER_BIN: &str = env!("CARGO_BIN_EXE_multi-echo-server");

fn register_stdio_server(config_str: &str, name: &str, bin: &str) {
    let status = std::process::Command::new(GATEWAY_BIN)
        .args([
            "-c",
            config_str,
            "add",
            name,
            "-t",
            "stdio",
            "--command",
            bin,
        ])
        .status()
        .unwrap();
    assert!(status.success());
}

fn run_cli(config_str: &str, args: &[&str]) {
    let status = std::process::Command::new(GATEWAY_BIN)
        .args(["-c", config_str])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}

async fn spawn_gateway(config_str: &str) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut cmd = tokio::process::Command::new(GATEWAY_BIN);
    cmd.args(["-c", config_str, "run"]);
    let transport = TokioChildProcess::new(cmd).unwrap();
    ().serve(transport).await.unwrap()
}

fn extract_text(result: &rmcp::model::CallToolResult) -> &str {
    result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.as_str())
        .unwrap()
}

#[tokio::test]
#[ignore]
async fn should_aggregate_tools_from_multiple_servers() {
    // Given two servers registered with different names
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "alpha", ECHO_SERVER_BIN);
    register_stdio_server(config_str, "beta", MULTI_ECHO_SERVER_BIN);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then all tools from both servers are listed with prefixes
    let result = client.list_tools(None).await.unwrap();
    let mut tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    tool_names.sort();
    assert_eq!(
        tool_names,
        ["alpha__echo", "beta__echo", "beta__reverse", "beta__upper"]
    );

    // And tools from each server work correctly
    let params = CallToolRequestParams::new("alpha__echo").with_arguments(
        serde_json::json!({"message": "hello"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "hello"
    );

    let params = CallToolRequestParams::new("beta__reverse").with_arguments(
        serde_json::json!({"message": "hello"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "olleh"
    );

    let params = CallToolRequestParams::new("beta__upper").with_arguments(
        serde_json::json!({"message": "hello"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "HELLO"
    );
}

#[tokio::test]
#[ignore]
async fn should_only_expose_allowed_tools() {
    // Given a server with an allowlist of echo and upper
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", MULTI_ECHO_SERVER_BIN);
    run_cli(config_str, &["allowlist", "add", "srv", "echo", "upper"]);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then only allowed tools are listed
    let result = client.list_tools(None).await.unwrap();
    let mut tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    tool_names.sort();
    assert_eq!(tool_names, ["srv__echo", "srv__upper"]);

    // And allowed tools work
    let params = CallToolRequestParams::new("srv__echo").with_arguments(
        serde_json::json!({"message": "hi"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(extract_text(&client.call_tool(params).await.unwrap()), "hi");

    // And blocked tools return an error
    let params = CallToolRequestParams::new("srv__reverse");
    assert!(client.call_tool(params).await.is_err());
}

#[tokio::test]
#[ignore]
async fn should_hide_denied_tools() {
    // Given a server with reverse on the denylist
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", MULTI_ECHO_SERVER_BIN);
    run_cli(config_str, &["denylist", "add", "srv", "reverse"]);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then denied tools are hidden from listing
    let result = client.list_tools(None).await.unwrap();
    let mut tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    tool_names.sort();
    assert_eq!(tool_names, ["srv__echo", "srv__upper"]);

    // And non-denied tools work
    let params = CallToolRequestParams::new("srv__upper").with_arguments(
        serde_json::json!({"message": "hi"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(extract_text(&client.call_tool(params).await.unwrap()), "HI");

    // And denied tools return an error
    let params = CallToolRequestParams::new("srv__reverse");
    assert!(client.call_tool(params).await.is_err());
}

#[tokio::test]
#[ignore]
async fn should_return_error_for_unknown_tool() {
    // Given a single server registered
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then calling a tool with unknown server prefix returns an error
    let params = CallToolRequestParams::new("nonexistent__tool");
    assert!(client.call_tool(params).await.is_err());

    // And calling a tool with valid server but unknown tool name returns an error
    let params = CallToolRequestParams::new("srv__nonexistent");
    assert!(client.call_tool(params).await.is_err());
}
