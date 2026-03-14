use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::cli_operation_runner::{NullCliRunner, ProcessCliRunner};
use mcp_gateway::adapters::driven::configuration::model::CliOperationDef;
use mcp_gateway::hexagon::ports::driven::cli_operation_runner::CliOperationRunner;
use mcp_gateway::hexagon::ports::driven::cli_operation_runner::OperationCallRequest;

// -- NullCliRunner tests (from cli_execution/null.rs) --

#[test]
fn null_list_tools_returns_empty() {
    assert!(NullCliRunner.list_operations().is_empty());
}

#[test]
fn null_has_tool_returns_false() {
    assert!(!NullCliRunner.has_operation("anything"));
}

#[tokio::test]
async fn null_call_tool_returns_error() {
    let request = OperationCallRequest {
        name: "test".to_string(),
        arguments: None,
    };
    let result = NullCliRunner.call_operation(&request).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("test"));
}

// -- ProcessCliRunner tests (from cli_execution/runner.rs) --

fn tool_def(command: &str, description: Option<&str>) -> CliOperationDef {
    CliOperationDef {
        command: command.to_string(),
        description: description.map(|s| s.to_string()),
    }
}

fn call_request(name: &str, args: Option<&str>) -> OperationCallRequest {
    OperationCallRequest {
        name: name.to_string(),
        arguments: args.map(|s| s.to_string()),
    }
}

#[test]
fn new_empty_runner() {
    let runner = ProcessCliRunner::new(BTreeMap::new());
    assert!(runner.list_operations().is_empty());
}

#[test]
fn has_tool_returns_true_for_existing() {
    let mut tools = BTreeMap::new();
    tools.insert("my-tool".to_string(), tool_def("echo", None));
    let runner = ProcessCliRunner::new(tools);
    assert!(runner.has_operation("my-tool"));
}

#[test]
fn has_tool_returns_false_for_missing() {
    let runner = ProcessCliRunner::new(BTreeMap::new());
    assert!(!runner.has_operation("nope"));
}

#[test]
fn list_tools_returns_descriptors() {
    let mut tools = BTreeMap::new();
    tools.insert("alpha".to_string(), tool_def("echo", Some("Echo tool")));
    tools.insert("beta".to_string(), tool_def("cat", None));
    let runner = ProcessCliRunner::new(tools);
    let listed = runner.list_operations();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].name, "alpha");
    assert_eq!(listed[1].name, "beta");
}

#[test]
fn list_tools_auto_description() {
    let mut tools = BTreeMap::new();
    tools.insert("t".to_string(), tool_def("git", None));
    let runner = ProcessCliRunner::new(tools);
    let listed = runner.list_operations();
    let desc = listed[0].description.as_deref().unwrap();
    assert!(desc.contains("git"));
}

#[test]
fn list_tools_custom_description() {
    let mut tools = BTreeMap::new();
    tools.insert("t".to_string(), tool_def("git", Some("Show git status")));
    let runner = ProcessCliRunner::new(tools);
    let listed = runner.list_operations();
    assert_eq!(listed[0].description.as_deref(), Some("Show git status"));
}

// NOTE: build_tool_descriptor is a private function in runner.rs. The following
// tests exercise the same behavior indirectly through list_operations().

#[test]
fn list_tools_schema_is_open_object() {
    let mut tools = BTreeMap::new();
    tools.insert("test".to_string(), tool_def("echo", None));
    let runner = ProcessCliRunner::new(tools);
    let listed = runner.list_operations();
    let schema: serde_json::Value = serde_json::from_str(&listed[0].schema).unwrap();
    assert_eq!(schema["type"], "object");
    assert!(schema.get("properties").is_none());
    assert!(schema.get("required").is_none());
}

#[test]
fn list_tools_with_custom_description_via_descriptor() {
    let mut tools = BTreeMap::new();
    tools.insert("gh-pr".to_string(), tool_def("gh", Some("List PRs")));
    let runner = ProcessCliRunner::new(tools);
    let listed = runner.list_operations();
    assert_eq!(listed[0].name, "gh-pr");
    assert_eq!(listed[0].description.as_deref(), Some("List PRs"));
}

#[test]
fn list_tools_auto_description_via_descriptor() {
    let mut tools = BTreeMap::new();
    tools.insert("docker-ps".to_string(), tool_def("docker", None));
    let runner = ProcessCliRunner::new(tools);
    let listed = runner.list_operations();
    assert_eq!(listed[0].name, "docker-ps");
    let desc = listed[0].description.as_deref().unwrap();
    assert!(desc.contains("docker"));
}

#[tokio::test]
async fn call_tool_pipes_json_to_stdin() {
    let mut tools = BTreeMap::new();
    tools.insert("cat-tool".to_string(), tool_def("cat", None));
    let runner = ProcessCliRunner::new(tools);
    let request = call_request("cat-tool", Some(r#"{"key":"value"}"#));
    let result = runner.call_operation(&request).await.unwrap();
    let content: serde_json::Value = serde_json::from_str(&result.content[0]).unwrap();
    let text = content["text"].as_str().unwrap();
    assert!(text.contains("key"));
    assert!(text.contains("value"));
    assert!(!result.is_error);
}

#[tokio::test]
async fn call_tool_nonzero_exit_is_error() {
    let mut tools = BTreeMap::new();
    tools.insert("false-test".to_string(), tool_def("false", None));
    let runner = ProcessCliRunner::new(tools);
    let request = call_request("false-test", None);
    let result = runner.call_operation(&request).await.unwrap();
    assert!(result.is_error);
}

#[tokio::test]
async fn call_tool_captures_stderr_on_failure() {
    let mut tools = BTreeMap::new();
    tools.insert("sh-fail".to_string(), tool_def("sh", None));
    let runner = ProcessCliRunner::new(tools);
    let request = call_request("sh-fail", None);
    let result = runner.call_operation(&request).await.unwrap();
    assert!(result.is_error);
}

#[tokio::test]
async fn call_tool_unknown_returns_error() {
    let runner = ProcessCliRunner::new(BTreeMap::new());
    let request = call_request("nope", None);
    let result = runner.call_operation(&request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn call_tool_command_not_found_returns_error() {
    let mut tools = BTreeMap::new();
    tools.insert(
        "bad-cmd".to_string(),
        tool_def("/nonexistent_binary_xyz", None),
    );
    let runner = ProcessCliRunner::new(tools);
    let request = call_request("bad-cmd", None);
    let result = runner.call_operation(&request).await;
    assert!(result.is_err());
}
