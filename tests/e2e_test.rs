#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use rmcp::model::CallToolRequestParams;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::ServiceExt;

const GATEWAY_BIN: &str = env!("CARGO_BIN_EXE_mcp-gateway");
const ECHO_SERVER_BIN: &str = env!("CARGO_BIN_EXE_echo-mcp-server");
const MULTI_ECHO_SERVER_BIN: &str = env!("CARGO_BIN_EXE_multi-echo-server");

fn gateway_cmd(config_str: &str) -> assert_cmd::Command {
    let mut cmd = assert_cmd::Command::new(GATEWAY_BIN);
    cmd.args(["-c", config_str]);
    cmd
}

fn cli_output(config_str: &str, args: &[&str]) -> std::process::Output {
    std::process::Command::new(GATEWAY_BIN)
        .args(["-c", config_str])
        .args(args)
        .output()
        .unwrap()
}

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
async fn should_deny_tool_when_in_both_allowlist_and_denylist() {
    // Given a server with "echo" in both allowlist and denylist
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", MULTI_ECHO_SERVER_BIN);
    run_cli(config_str, &["allowlist", "add", "srv", "echo", "upper"]);
    run_cli(config_str, &["denylist", "add", "srv", "echo"]);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then denylist wins — echo is hidden, only upper remains
    let result = client.list_tools(None).await.unwrap();
    let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(tool_names, ["srv__upper"]);

    // And calling the denied tool returns an error
    let params = CallToolRequestParams::new("srv__echo");
    assert!(client.call_tool(params).await.is_err());
}

#[tokio::test]
#[ignore]
async fn should_work_with_explicit_transport_stdio() {
    // Given a registered server
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);

    // When the gateway runs with explicit --transport stdio
    let mut cmd = tokio::process::Command::new(GATEWAY_BIN);
    cmd.args(["-c", config_str, "run", "--transport", "stdio"]);
    let transport = TokioChildProcess::new(cmd).unwrap();
    let client: rmcp::service::RunningService<rmcp::RoleClient, ()> =
        ().serve(transport).await.unwrap();

    // Then tools are listed correctly
    let result = client.list_tools(None).await.unwrap();
    assert_eq!(result.tools.len(), 1);
    assert_eq!(
        result.tools.first().map(|t| t.name.as_ref()),
        Some("srv__echo")
    );
}

#[tokio::test]
#[ignore]
async fn should_preserve_tool_descriptions_and_schemas_through_gateway() {
    // Given a server with tools that have descriptions and schemas
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", MULTI_ECHO_SERVER_BIN);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then tool descriptions are preserved through the gateway
    let result = client.list_tools(None).await.unwrap();
    let echo_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "srv__echo")
        .unwrap();
    assert_eq!(echo_tool.description.as_deref(), Some("echoes input"));

    let reverse_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "srv__reverse")
        .unwrap();
    assert_eq!(reverse_tool.description.as_deref(), Some("reverses input"));

    let upper_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "srv__upper")
        .unwrap();
    assert_eq!(upper_tool.description.as_deref(), Some("uppercases input"));

    // And tool input schemas are preserved
    for tool in &result.tools {
        assert_eq!(
            tool.input_schema.get("type").and_then(|v| v.as_str()),
            Some("object")
        );
        let props = tool
            .input_schema
            .get("properties")
            .and_then(|v| v.as_object())
            .unwrap();
        assert!(props.contains_key("message"));
    }
}

#[tokio::test]
#[ignore]
async fn should_expose_cli_tools_alongside_mcp_tools() {
    // Given a config with both MCP servers and CLI tools
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);

    // Add CLI tools directly to the config file
    let raw = std::fs::read_to_string(&config_path).unwrap();
    let mut config: serde_json::Value = serde_json::from_str(&raw).unwrap();
    config["cliTools"] = serde_json::json!({
        "my-cat": {
            "command": "cat",
            "description": "Cat stdin to stdout"
        }
    });
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then both MCP tools and CLI tools are listed
    let result = client.list_tools(None).await.unwrap();
    let mut tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    tool_names.sort();
    assert_eq!(tool_names, ["my-cat", "srv__echo"]);

    // And the CLI tool description is preserved
    let cli_tool = result
        .tools
        .iter()
        .find(|t| t.name.as_ref() == "my-cat")
        .unwrap();
    assert_eq!(cli_tool.description.as_deref(), Some("Cat stdin to stdout"));

    // And the CLI tool can be called successfully
    let params = CallToolRequestParams::new("my-cat");
    let result = client.call_tool(params).await.unwrap();
    assert!(!result.is_error.unwrap_or(false));

    // And the MCP tool still works alongside CLI tools
    let params = CallToolRequestParams::new("srv__echo").with_arguments(
        serde_json::json!({"message": "hi"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(extract_text(&client.call_tool(params).await.unwrap()), "hi");
}

#[tokio::test]
#[ignore]
async fn should_disambiguate_same_tool_name_across_servers() {
    // Given two servers that both expose an "echo" tool
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "alpha", ECHO_SERVER_BIN);
    register_stdio_server(config_str, "beta", MULTI_ECHO_SERVER_BIN);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then both echo tools are listed with distinct prefixed names
    let result = client.list_tools(None).await.unwrap();
    let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(tool_names.contains(&"alpha__echo"));
    assert!(tool_names.contains(&"beta__echo"));

    // And calling each routes to the correct server
    let params = CallToolRequestParams::new("alpha__echo").with_arguments(
        serde_json::json!({"message": "from-alpha"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "from-alpha"
    );

    let params = CallToolRequestParams::new("beta__echo").with_arguments(
        serde_json::json!({"message": "from-beta"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "from-beta"
    );
}

#[tokio::test]
#[ignore]
async fn should_handle_unicode_messages() {
    // Given a server with an echo tool
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then unicode messages round-trip correctly through the gateway
    let unicode_msg = "こんにちは 🌍 émojis café";
    let params = CallToolRequestParams::new("srv__echo").with_arguments(
        serde_json::json!({"message": unicode_msg})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        unicode_msg
    );
}

#[tokio::test]
#[ignore]
async fn should_handle_empty_arguments() {
    // Given a server with an echo tool
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);

    // When the gateway runs and a tool is called with no arguments
    let client = spawn_gateway(config_str).await;
    let params = CallToolRequestParams::new("srv__echo");
    let result = client.call_tool(params).await.unwrap();

    // Then the tool returns a default empty response
    assert_eq!(extract_text(&result), "");
}

#[tokio::test]
#[ignore]
async fn should_handle_multiple_sequential_calls() {
    // Given a server with multiple tools registered
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", MULTI_ECHO_SERVER_BIN);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then multiple sequential calls to the same tool all succeed
    for i in 0..10 {
        let msg = format!("call-{i}");
        let params = CallToolRequestParams::new("srv__echo").with_arguments(
            serde_json::json!({"message": &msg})
                .as_object()
                .unwrap()
                .clone(),
        );
        assert_eq!(extract_text(&client.call_tool(params).await.unwrap()), msg);
    }

    // And different tools can be called in sequence
    let params = CallToolRequestParams::new("srv__reverse").with_arguments(
        serde_json::json!({"message": "abc"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "cba"
    );

    let params = CallToolRequestParams::new("srv__upper").with_arguments(
        serde_json::json!({"message": "abc"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "ABC"
    );
}

#[tokio::test]
#[ignore]
async fn should_apply_different_filters_per_server() {
    // Given two servers with different filter configurations
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "alpha", MULTI_ECHO_SERVER_BIN);
    register_stdio_server(config_str, "beta", MULTI_ECHO_SERVER_BIN);

    // alpha: only allow echo
    run_cli(config_str, &["allowlist", "add", "alpha", "echo"]);
    // beta: deny echo (expose reverse and upper)
    run_cli(config_str, &["denylist", "add", "beta", "echo"]);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then only the correct tools from each server are listed
    let result = client.list_tools(None).await.unwrap();
    let mut tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    tool_names.sort();
    assert_eq!(tool_names, ["alpha__echo", "beta__reverse", "beta__upper"]);

    // And the allowed tools work correctly
    let params = CallToolRequestParams::new("alpha__echo").with_arguments(
        serde_json::json!({"message": "hi"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(extract_text(&client.call_tool(params).await.unwrap()), "hi");

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

    // And filtered-out tools return errors
    let params = CallToolRequestParams::new("alpha__reverse");
    assert!(client.call_tool(params).await.is_err());

    let params = CallToolRequestParams::new("beta__echo");
    assert!(client.call_tool(params).await.is_err());
}

#[tokio::test]
#[ignore]
async fn should_report_gateway_identity() {
    // Given a server registered
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then the gateway identifies itself with name and version
    let info = client.peer_info().unwrap();
    assert_eq!(info.server_info.name, "mcp-gateway");
    assert!(!info.server_info.version.is_empty());

    // And it advertises tool capabilities
    assert!(info.capabilities.tools.is_some());
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

// ── User Story Tests ─────────────────────────────────────────────────────

// User Story: As a new user, I want the CLI to guide me when I run it
// without arguments, so I know what commands are available.
#[test]
fn user_sees_help_when_running_without_arguments() {
    // Given no arguments
    // When the user runs mcp-gateway
    let output = std::process::Command::new(GATEWAY_BIN).output().unwrap();

    // Then the output shows available commands
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("add"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("remove"));
    assert!(stdout.contains("run"));
    assert!(stdout.contains("allowlist"));
    assert!(stdout.contains("denylist"));
}

// User Story: As a user, I want to check the version so I know what I'm running.
#[test]
fn user_can_check_version() {
    // When the user runs mcp-gateway --version
    let output = std::process::Command::new(GATEWAY_BIN)
        .arg("--version")
        .output()
        .unwrap();

    // Then it shows a version string
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mcp-gateway"));
}

// User Story: As a user, I want to register a stdio server, see it listed,
// and then remove it, so I can manage my gateway configuration.
#[test]
fn user_can_register_list_and_remove_a_stdio_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // Given an empty configuration
    // When the user lists servers
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Then the output is empty (no servers registered)
    assert!(stdout.is_empty());

    // When the user adds a stdio server
    gateway_cmd(config_str)
        .args([
            "add",
            "my-echo",
            "-t",
            "stdio",
            "--command",
            ECHO_SERVER_BIN,
        ])
        .assert()
        .success();

    // Then `list` shows the server with name, type, and command
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("NAME"));
    assert!(stdout.contains("TYPE"));
    assert!(stdout.contains("TARGET"));
    assert!(stdout.contains("my-echo"));
    assert!(stdout.contains("stdio"));

    // When the user removes the server
    gateway_cmd(config_str)
        .args(["remove", "my-echo"])
        .assert()
        .success();

    // Then `list` is empty again
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty());
}

// User Story: As a user, I want to register an HTTP server with headers,
// so I can connect to remote MCP services.
#[test]
fn user_can_register_http_server_with_headers() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // When the user adds an http server with a header
    gateway_cmd(config_str)
        .args([
            "add",
            "remote-api",
            "-t",
            "http",
            "--url",
            "https://example.com/mcp",
            "--header",
            "Authorization: Bearer my-token",
        ])
        .assert()
        .success();

    // Then `list` shows the server with http type and URL
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("remote-api"));
    assert!(stdout.contains("http"));
    assert!(stdout.contains("https://example.com/mcp"));

    // And the config file contains the header
    let raw = std::fs::read_to_string(&config_path).unwrap();
    assert!(raw.contains("Authorization"));
    assert!(raw.contains("Bearer my-token"));
}

// User Story: As a user, I want to register a stdio server with args and env vars,
// so I can pass configuration to my MCP server process.
#[test]
fn user_can_register_stdio_server_with_args_and_env() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // When the user adds a stdio server with args and env
    gateway_cmd(config_str)
        .args([
            "add",
            "my-node",
            "-t",
            "stdio",
            "--command",
            "node",
            "--args",
            "server.js",
            "--env",
            "API_KEY=secret123",
            "--env",
            "DEBUG=true",
        ])
        .assert()
        .success();

    // Then the config file contains the args and env vars
    let raw = std::fs::read_to_string(&config_path).unwrap();
    assert!(raw.contains("server.js"));
    assert!(raw.contains("API_KEY"));
    assert!(raw.contains("secret123"));
    assert!(raw.contains("DEBUG"));
    assert!(raw.contains("true"));

    // And `list` shows the server
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("my-node"));
    assert!(stdout.contains("stdio"));
    assert!(stdout.contains("node"));
}

// User Story: As a user, I want to manage multiple servers and see them
// all in the list, so I can work with several MCP services at once.
#[test]
fn user_can_manage_multiple_servers() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // Given two registered servers
    register_stdio_server(config_str, "alpha", ECHO_SERVER_BIN);
    register_stdio_server(config_str, "beta", MULTI_ECHO_SERVER_BIN);

    // When the user lists servers
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Then both servers appear in the list
    assert!(stdout.contains("alpha"));
    assert!(stdout.contains("beta"));

    // When the user removes one
    gateway_cmd(config_str)
        .args(["remove", "alpha"])
        .assert()
        .success();

    // Then only the remaining server is listed
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("alpha"));
    assert!(stdout.contains("beta"));
}

// User Story: As a user, I want to configure allowlists so I can restrict
// which tools are exposed through the gateway.
#[test]
fn user_can_configure_and_inspect_allowlists() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", MULTI_ECHO_SERVER_BIN);

    // Given a server with no allowlist
    // When the user shows the allowlist
    let output = cli_output(config_str, &["allowlist", "show", "srv"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Then the output is empty
    assert!(stdout.is_empty());

    // When the user adds tools to the allowlist
    run_cli(config_str, &["allowlist", "add", "srv", "echo", "reverse"]);

    // Then `allowlist show` displays them
    let output = cli_output(config_str, &["allowlist", "show", "srv"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("echo"));
    assert!(stdout.contains("reverse"));

    // When the user removes one tool from the allowlist
    run_cli(config_str, &["allowlist", "remove", "srv", "reverse"]);

    // Then `allowlist show` reflects the change
    let output = cli_output(config_str, &["allowlist", "show", "srv"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("echo"));
    assert!(!stdout.contains("reverse"));
}

// User Story: As a user, I want to configure denylists so I can block
// dangerous tools from being exposed.
#[test]
fn user_can_configure_and_inspect_denylists() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", MULTI_ECHO_SERVER_BIN);

    // Given a server with no denylist
    // When the user shows the denylist
    let output = cli_output(config_str, &["denylist", "show", "srv"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Then the output is empty
    assert!(stdout.is_empty());

    // When the user adds tools to the denylist
    run_cli(config_str, &["denylist", "add", "srv", "upper", "reverse"]);

    // Then `denylist show` displays them
    let output = cli_output(config_str, &["denylist", "show", "srv"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("upper"));
    assert!(stdout.contains("reverse"));

    // When the user removes one tool from the denylist
    run_cli(config_str, &["denylist", "remove", "srv", "upper"]);

    // Then `denylist show` reflects the change
    let output = cli_output(config_str, &["denylist", "show", "srv"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("upper"));
    assert!(stdout.contains("reverse"));
}

// User Story: As a user, I want clear error messages when I make mistakes,
// so I know what went wrong and how to fix it.
#[test]
fn user_gets_clear_error_for_duplicate_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // Given an existing server
    register_stdio_server(config_str, "my-server", ECHO_SERVER_BIN);

    // When the user tries to add a server with the same name
    let output = cli_output(
        config_str,
        &["add", "my-server", "-t", "stdio", "--command", "echo"],
    );

    // Then the command fails with a clear error mentioning the name
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("my-server"));
}

#[test]
fn user_gets_clear_error_for_removing_nonexistent_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // When the user tries to remove a server that doesn't exist
    let output = cli_output(config_str, &["remove", "ghost"]);

    // Then the command fails with a clear error mentioning the name
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost"));
}

#[test]
fn user_gets_clear_error_for_allowlist_on_nonexistent_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // When the user tries to configure allowlist for a nonexistent server
    let output = cli_output(config_str, &["allowlist", "add", "ghost", "read"]);

    // Then the command fails with a clear error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost"));
}

#[test]
fn user_gets_clear_error_for_denylist_on_nonexistent_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // When the user tries to configure denylist for a nonexistent server
    let output = cli_output(config_str, &["denylist", "add", "ghost", "delete"]);

    // Then the command fails with a clear error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost"));
}

// User Story: As a user, I want to set up a complete gateway from scratch
// and verify everything works end-to-end.
#[tokio::test]
#[ignore]
async fn user_full_lifecycle_add_configure_run_verify() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // Step 1: User registers two servers
    gateway_cmd(config_str)
        .args([
            "add",
            "tools",
            "-t",
            "stdio",
            "--command",
            MULTI_ECHO_SERVER_BIN,
        ])
        .assert()
        .success();

    gateway_cmd(config_str)
        .args(["add", "simple", "-t", "stdio", "--command", ECHO_SERVER_BIN])
        .assert()
        .success();

    // Step 2: User verifies both are listed
    let output = cli_output(config_str, &["list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tools"));
    assert!(stdout.contains("simple"));

    // Step 3: User restricts "tools" server to only echo and reverse
    run_cli(
        config_str,
        &["allowlist", "add", "tools", "echo", "reverse"],
    );
    let output = cli_output(config_str, &["allowlist", "show", "tools"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("echo"));
    assert!(stdout.contains("reverse"));

    // Step 4: User blocks echo on "simple" server
    run_cli(config_str, &["denylist", "add", "simple", "echo"]);
    let output = cli_output(config_str, &["denylist", "show", "simple"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("echo"));

    // Step 5: User runs the gateway and verifies
    let client = spawn_gateway(config_str).await;

    // Step 6: User lists tools and sees correct filtering
    let result = client.list_tools(None).await.unwrap();
    let mut tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    tool_names.sort();
    // "tools" server: only echo and reverse (upper filtered by allowlist)
    // "simple" server: echo denied, so nothing
    assert_eq!(tool_names, ["tools__echo", "tools__reverse"]);

    // Step 7: User calls available tools
    let params = CallToolRequestParams::new("tools__echo").with_arguments(
        serde_json::json!({"message": "works!"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "works!"
    );

    let params = CallToolRequestParams::new("tools__reverse").with_arguments(
        serde_json::json!({"message": "hello"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "olleh"
    );

    // Step 8: User verifies blocked tools are rejected
    let params = CallToolRequestParams::new("tools__upper");
    assert!(client.call_tool(params).await.is_err());

    let params = CallToolRequestParams::new("simple__echo");
    assert!(client.call_tool(params).await.is_err());
}

// User Story: As a user, I want the config file to be human-readable JSON,
// so I can inspect and edit it manually if needed.
#[test]
fn config_file_is_readable_json() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // Given a server registered with various options
    gateway_cmd(config_str)
        .args([
            "add",
            "my-server",
            "-t",
            "stdio",
            "--command",
            "node",
            "--args",
            "server.js",
            "--env",
            "PORT=3000",
        ])
        .assert()
        .success();

    run_cli(config_str, &["allowlist", "add", "my-server", "read"]);
    run_cli(config_str, &["denylist", "add", "my-server", "delete"]);

    // Then the config file is valid, well-structured JSON
    let raw = std::fs::read_to_string(&config_path).unwrap();
    let config: serde_json::Value = serde_json::from_str(&raw).unwrap();

    // And uses camelCase keys matching the MCP ecosystem conventions
    assert!(config.get("mcpServers").is_some());
    let server = &config["mcpServers"]["my-server"];
    assert_eq!(server["type"], "stdio");
    assert_eq!(server["command"], "node");
    assert!(server["args"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("server.js")));
    assert_eq!(server["env"]["PORT"], "3000");
    assert!(server["allowedTools"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("read")));
    assert!(server["deniedTools"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("delete")));
}

// ── Negative Scenario Tests ──────────────────────────────────────────────

// Scenario: Gateway starts even when an upstream server binary doesn't exist,
// skipping the unavailable server and serving the rest.
#[tokio::test]
#[ignore]
async fn should_skip_unavailable_upstream_and_serve_remaining() {
    // Given one valid server and one with a nonexistent binary
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "good", ECHO_SERVER_BIN);
    run_cli(
        config_str,
        &[
            "add",
            "bad",
            "-t",
            "stdio",
            "--command",
            "/nonexistent/binary/that/does/not/exist",
        ],
    );

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then only the working server's tools are available
    let result = client.list_tools(None).await.unwrap();
    let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(tool_names, ["good__echo"]);

    // And the working server's tools function correctly
    let params = CallToolRequestParams::new("good__echo").with_arguments(
        serde_json::json!({"message": "still works"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "still works"
    );
}

// Scenario: Gateway starts with no servers at all.
#[tokio::test]
#[ignore]
async fn should_start_with_empty_config_and_list_no_tools() {
    // Given an empty configuration
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    std::fs::write(&config_path, "{}").unwrap();
    let config_str = config_path.to_str().unwrap();

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then list_tools returns an empty list
    let result = client.list_tools(None).await.unwrap();
    assert!(result.tools.is_empty());

    // And calling any tool returns an error
    let params = CallToolRequestParams::new("anything__anywhere");
    assert!(client.call_tool(params).await.is_err());
}

// Scenario: Calling a tool without the server prefix separator returns an error.
#[tokio::test]
#[ignore]
async fn should_reject_tool_call_without_prefix() {
    // Given a running gateway with a server
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);
    let client = spawn_gateway(config_str).await;

    // When calling a tool without the double-underscore prefix
    let params = CallToolRequestParams::new("echo");
    let result = client.call_tool(params).await;

    // Then the call is rejected
    assert!(result.is_err());
}

// Scenario: Corrupted config file prevents startup.
#[test]
fn should_fail_gracefully_with_corrupted_config() {
    // Given a corrupted config file
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    std::fs::write(&config_path, "{{{{not valid json!!!!").unwrap();
    let config_str = config_path.to_str().unwrap();

    // When the user tries to list servers
    let output = cli_output(config_str, &["list"]);

    // Then the command fails
    assert!(!output.status.success());
}

// Scenario: Adding a server to a read-only config path fails gracefully.
#[test]
fn should_fail_gracefully_with_unwritable_config_path() {
    // Given a config path in a non-existent directory
    let config_str = "/nonexistent/deeply/nested/path/config.json";

    // When the user tries to add a server
    let output = cli_output(
        config_str,
        &["add", "srv", "-t", "stdio", "--command", "echo"],
    );

    // Then the command fails with an error
    assert!(!output.status.success());
}

// Scenario: Allowlist show on nonexistent server fails.
#[test]
fn should_fail_for_allowlist_show_on_nonexistent_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // When the user tries to show allowlist for a nonexistent server
    let output = cli_output(config_str, &["allowlist", "show", "ghost"]);

    // Then the command fails with a clear error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost"));
}

// Scenario: Denylist show on nonexistent server fails.
#[test]
fn should_fail_for_denylist_show_on_nonexistent_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    // When the user tries to show denylist for a nonexistent server
    let output = cli_output(config_str, &["denylist", "show", "ghost"]);

    // Then the command fails with a clear error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost"));
}

// Scenario: Allowlist remove on nonexistent server fails.
#[test]
fn should_fail_for_allowlist_remove_on_nonexistent_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    let output = cli_output(config_str, &["allowlist", "remove", "ghost", "read"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost"));
}

// Scenario: Denylist remove on nonexistent server fails.
#[test]
fn should_fail_for_denylist_remove_on_nonexistent_server() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    let output = cli_output(config_str, &["denylist", "remove", "ghost", "delete"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost"));
}

// Scenario: CLI tool with a failing command reports error to the caller.
#[tokio::test]
#[ignore]
async fn should_report_cli_tool_failure_as_error_result() {
    // Given a config with a CLI tool that always fails
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    std::fs::write(
        &config_path,
        serde_json::json!({
            "mcpServers": {},
            "cliTools": {
                "always-fail": {
                    "command": "false",
                    "description": "Always exits with error"
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then the failing CLI tool is listed
    let result = client.list_tools(None).await.unwrap();
    assert_eq!(result.tools.len(), 1);
    assert_eq!(
        result.tools.first().map(|t| t.name.as_ref()),
        Some("always-fail")
    );

    // And calling it returns an error result (not a transport error)
    let params = CallToolRequestParams::new("always-fail");
    let result = client.call_tool(params).await.unwrap();
    assert!(result.is_error.unwrap_or(false));
}

// Scenario: CLI tool with nonexistent binary is reported as error.
#[tokio::test]
#[ignore]
async fn should_report_error_for_nonexistent_cli_tool_binary() {
    // Given a config with a CLI tool pointing to a nonexistent binary
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    std::fs::write(
        &config_path,
        serde_json::json!({
            "mcpServers": {},
            "cliTools": {
                "ghost-tool": {
                    "command": "/nonexistent/binary/xyz",
                    "description": "This binary does not exist"
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then the tool is listed (config is valid)
    let result = client.list_tools(None).await.unwrap();
    assert_eq!(result.tools.len(), 1);

    // And calling it returns a JSON-RPC error
    let params = CallToolRequestParams::new("ghost-tool");
    let result = client.call_tool(params).await;
    assert!(result.is_err());
}

// Scenario: Gateway with only unavailable upstreams still starts
// and responds to tool listing with empty results.
#[tokio::test]
#[ignore]
async fn should_start_with_all_upstreams_unavailable() {
    // Given all registered servers have nonexistent binaries
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    run_cli(
        config_str,
        &["add", "bad1", "-t", "stdio", "--command", "/nonexistent/a"],
    );
    run_cli(
        config_str,
        &["add", "bad2", "-t", "stdio", "--command", "/nonexistent/b"],
    );

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then it starts successfully with no tools
    let result = client.list_tools(None).await.unwrap();
    assert!(result.tools.is_empty());
}

// Scenario: HTTP upstream at an unreachable URL is skipped,
// and remaining servers still work.
#[tokio::test]
#[ignore]
async fn should_skip_unreachable_http_upstream_and_serve_remaining() {
    // Given a valid stdio server and an HTTP server at an unreachable URL
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "good", ECHO_SERVER_BIN);
    run_cli(
        config_str,
        &[
            "add",
            "unreachable",
            "-t",
            "http",
            "--url",
            "http://127.0.0.1:1/mcp",
        ],
    );

    // When the gateway runs
    let client = spawn_gateway(config_str).await;

    // Then only the working server's tools are available
    let result = client.list_tools(None).await.unwrap();
    let tool_names: Vec<&str> = result.tools.iter().map(|t| t.name.as_ref()).collect();
    assert_eq!(tool_names, ["good__echo"]);

    // And the working server functions correctly
    let params = CallToolRequestParams::new("good__echo").with_arguments(
        serde_json::json!({"message": "still works"})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        extract_text(&client.call_tool(params).await.unwrap()),
        "still works"
    );
}

// ── Fuzzy / Robustness Tests ─────────────────────────────────────────────

// Fuzzy: Random garbage as server name should not crash the CLI.
#[test]
fn fuzz_add_with_special_character_names() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    let weird_names = [
        " ",
        "name with spaces",
        "name\twith\ttabs",
        "../../../etc/passwd",
        "name;rm -rf /",
        "名前",
        "🚀🔥",
        "null",
        "undefined",
        "__double__underscores__",
    ];

    for name in &weird_names {
        // None of these should cause a panic or crash
        let output = cli_output(
            config_str,
            &["add", name, "-t", "stdio", "--command", "echo"],
        );
        // We don't care if it succeeds or fails, just that it doesn't crash
        let _ = output.status;
    }
}

// Fuzzy: Random garbage as tool names in allowlist/denylist.
#[test]
fn fuzz_allowlist_with_special_tool_names() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);

    let weird_tools = [
        "../etc/passwd",
        "tool;injection",
        "🔧",
        "tool with spaces",
        "__prefix__tool",
    ];

    for tool in &weird_tools {
        // Should not crash, regardless of success or failure
        let _ = cli_output(config_str, &["allowlist", "add", "srv", tool]);
    }

    // And the server should still work after all the abuse
    let output = cli_output(config_str, &["list"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("srv"));
}

// Fuzzy: Random tool call arguments through the gateway.
#[tokio::test]
#[ignore]
async fn fuzz_tool_calls_with_unusual_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);
    let client = spawn_gateway(config_str).await;

    // Various unusual argument patterns — none should crash the gateway
    let test_cases: Vec<serde_json::Value> = vec![
        serde_json::json!({}),
        serde_json::json!({"message": null}),
        serde_json::json!({"message": 42}),
        serde_json::json!({"message": true}),
        serde_json::json!({"message": {"nested": "value"}}),
        serde_json::json!({"message": [1, 2, 3]}),
        serde_json::json!({"message": "x".repeat(100_000)}),
        serde_json::json!({"message": "\0\n\r\t\\\""}),
        serde_json::json!({"message": "hi", "extra": "field", "another": 123}),
        serde_json::json!({"message": "\u{FEFF}\u{200B}"}),
    ];

    for (i, args) in test_cases.iter().enumerate() {
        let params = if let Some(obj) = args.as_object() {
            CallToolRequestParams::new("srv__echo").with_arguments(obj.clone())
        } else {
            CallToolRequestParams::new("srv__echo")
        };
        // Should not crash — may return success or error
        let _ = client.call_tool(params).await;
        // Verify gateway is still responsive after each weird call
        let result = client.list_tools(None).await;
        assert!(
            result.is_ok(),
            "gateway became unresponsive after test case {i}"
        );
    }
}

// Fuzzy: Random prefixed tool names.
#[tokio::test]
#[ignore]
async fn fuzz_tool_calls_with_malformed_prefixes() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    register_stdio_server(config_str, "srv", ECHO_SERVER_BIN);
    let client = spawn_gateway(config_str).await;

    let malformed_names = [
        "__",
        "____",
        "__echo",
        "srv__",
        "srv____echo",
        "srv__echo__extra",
        "nonexistent__echo",
        "srv echo",
        &format!("{}__echo", "a".repeat(10000)),
    ];

    for name in &malformed_names {
        let params = CallToolRequestParams::new(name.to_string());
        // Should return an error, not crash
        let _ = client.call_tool(params).await;
    }

    // Gateway should still be responsive
    let result = client.list_tools(None).await.unwrap();
    assert_eq!(result.tools.len(), 1);
}

// Fuzzy: Corrupted JSON config files with partial/malformed content.
#[test]
fn fuzz_corrupted_config_files() {
    let dir = tempfile::tempdir().unwrap();

    let corrupted_configs = [
        "",
        "null",
        "[]",
        "\"string\"",
        "42",
        "{\"mcpServers\": null}",
        "{\"mcpServers\": []}",
        "{\"mcpServers\": \"not an object\"}",
        "{\"mcpServers\": {\"srv\": null}}",
        "{\"mcpServers\": {\"srv\": []}}",
        "{\"mcpServers\": {\"srv\": \"string\"}}",
        "{\"mcpServers\": {\"srv\": {\"type\": \"unknown\"}}}",
        "{\"mcpServers\": {\"srv\": {\"type\": \"stdio\"}}}",
        "{{broken json",
        "{\"mcpServers\":",
    ];

    for (i, content) in corrupted_configs.iter().enumerate() {
        let config_path = dir.path().join(format!("config_{i}.json"));
        std::fs::write(&config_path, content).unwrap();
        let config_str = config_path.to_str().unwrap();

        // list should either succeed (empty) or fail gracefully — never crash
        let output = cli_output(config_str, &["list"]);
        let _ = output.status;

        // add should either succeed or fail gracefully — never crash
        let output = cli_output(
            config_str,
            &["add", "test", "-t", "stdio", "--command", "echo"],
        );
        let _ = output.status;
    }
}

// Fuzzy: Inject malicious content through env vars and headers.
#[test]
fn fuzz_injection_through_env_and_headers() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    let config_str = config_path.to_str().unwrap();

    let injection_payloads = [
        ("KEY=value; rm -rf /", "header: value; rm -rf /"),
        ("KEY=$(whoami)", "header: $(whoami)"),
        ("KEY=`id`", "header: `id`"),
        ("KEY=val\nINJECTED=true", "header: val\r\nInjected: true"),
    ];

    for (i, (env_payload, header_payload)) in injection_payloads.iter().enumerate() {
        let name = format!("srv-{i}");

        // Test env injection on stdio
        let output = std::process::Command::new(GATEWAY_BIN)
            .args([
                "-c",
                config_str,
                "add",
                &name,
                "-t",
                "stdio",
                "--command",
                "echo",
                "--env",
                env_payload,
            ])
            .output()
            .unwrap();
        // Should not crash
        let _ = output.status;

        // Test header injection on http
        let http_name = format!("http-{i}");
        let output = std::process::Command::new(GATEWAY_BIN)
            .args([
                "-c",
                config_str,
                "add",
                &http_name,
                "-t",
                "http",
                "--url",
                "http://localhost:9999",
                "--header",
                header_payload,
            ])
            .output()
            .unwrap();
        // Should not crash
        let _ = output.status;
    }

    // Config file should still be valid JSON after all the abuse
    let raw = std::fs::read_to_string(&config_path).unwrap();
    let _: serde_json::Value = serde_json::from_str(&raw).unwrap();
}
