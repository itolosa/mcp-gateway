use std::collections::BTreeMap;

use tokio::io::AsyncWriteExt;

use crate::config::model::CliToolDef;
use crate::hexagon::ports::{
    CliToolRunner, GatewayError, ToolCallRequest, ToolCallResult, ToolDescriptor,
};

pub struct ProcessCliRunner {
    tools: BTreeMap<String, CliToolDef>,
}

impl ProcessCliRunner {
    pub fn new(tools: BTreeMap<String, CliToolDef>) -> Self {
        Self { tools }
    }
}

impl CliToolRunner for ProcessCliRunner {
    fn list_tools(&self) -> Vec<ToolDescriptor> {
        self.tools
            .iter()
            .map(|(name, def)| build_tool_descriptor(name, def))
            .collect()
    }

    fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    async fn call_tool(&self, request: &ToolCallRequest) -> Result<ToolCallResult, GatewayError> {
        let name = &request.name;
        let def = self
            .tools
            .get(name.as_str())
            .ok_or_else(|| GatewayError::CliTool(format!("unknown CLI tool: {name}")))?;

        let input_json = request.arguments.as_deref().unwrap_or("{}");

        let output = run_command(&def.command, input_json)
            .await
            .map_err(|e| GatewayError::CliTool(format!("failed to run '{}': {e}", def.command)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(ToolCallResult {
                content: vec![serde_json::json!({"type": "text", "text": stdout}).to_string()],
                is_error: false,
            })
        } else {
            Ok(ToolCallResult {
                content: vec![serde_json::json!({"type": "text", "text": stderr}).to_string()],
                is_error: true,
            })
        }
    }
}

async fn run_command(command: &str, input: &str) -> std::io::Result<std::process::Output> {
    let mut child = tokio::process::Command::new(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    #[allow(clippy::expect_used)]
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let _ = stdin.write_all(input.as_bytes()).await;
    drop(stdin);

    child.wait_with_output().await
}

fn build_tool_descriptor(name: &str, def: &CliToolDef) -> ToolDescriptor {
    let description = def
        .description
        .clone()
        .unwrap_or_else(|| format!("Execute: {}", def.command));
    ToolDescriptor {
        name: name.to_string(),
        description: Some(description),
        schema: r#"{"type":"object"}"#.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn tool_def(command: &str, description: Option<&str>) -> CliToolDef {
        CliToolDef {
            command: command.to_string(),
            description: description.map(|s| s.to_string()),
        }
    }

    fn call_request(name: &str, args: Option<&str>) -> ToolCallRequest {
        ToolCallRequest {
            name: name.to_string(),
            arguments: args.map(|s| s.to_string()),
        }
    }

    #[test]
    fn new_empty_runner() {
        let runner = ProcessCliRunner::new(BTreeMap::new());
        assert!(runner.list_tools().is_empty());
    }

    #[test]
    fn has_tool_returns_true_for_existing() {
        let mut tools = BTreeMap::new();
        tools.insert("my-tool".to_string(), tool_def("echo", None));
        let runner = ProcessCliRunner::new(tools);
        assert!(runner.has_tool("my-tool"));
    }

    #[test]
    fn has_tool_returns_false_for_missing() {
        let runner = ProcessCliRunner::new(BTreeMap::new());
        assert!(!runner.has_tool("nope"));
    }

    #[test]
    fn list_tools_returns_descriptors() {
        let mut tools = BTreeMap::new();
        tools.insert("alpha".to_string(), tool_def("echo", Some("Echo tool")));
        tools.insert("beta".to_string(), tool_def("cat", None));
        let runner = ProcessCliRunner::new(tools);
        let listed = runner.list_tools();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name, "alpha");
        assert_eq!(listed[1].name, "beta");
    }

    #[test]
    fn list_tools_auto_description() {
        let mut tools = BTreeMap::new();
        tools.insert("t".to_string(), tool_def("git", None));
        let runner = ProcessCliRunner::new(tools);
        let listed = runner.list_tools();
        let desc = listed[0].description.as_deref().unwrap();
        assert!(desc.contains("git"));
    }

    #[test]
    fn list_tools_custom_description() {
        let mut tools = BTreeMap::new();
        tools.insert("t".to_string(), tool_def("git", Some("Show git status")));
        let runner = ProcessCliRunner::new(tools);
        let listed = runner.list_tools();
        assert_eq!(listed[0].description.as_deref(), Some("Show git status"));
    }

    #[test]
    fn build_tool_descriptor_schema_is_open_object() {
        let def = tool_def("echo", None);
        let tool = build_tool_descriptor("test", &def);
        let schema: serde_json::Value = serde_json::from_str(&tool.schema).unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema.get("properties").is_none());
        assert!(schema.get("required").is_none());
    }

    #[tokio::test]
    async fn call_tool_pipes_json_to_stdin() {
        let mut tools = BTreeMap::new();
        tools.insert("cat-tool".to_string(), tool_def("cat", None));
        let runner = ProcessCliRunner::new(tools);
        let request = call_request("cat-tool", Some(r#"{"key":"value"}"#));
        let result = runner.call_tool(&request).await.unwrap();
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
        let result = runner.call_tool(&request).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn call_tool_captures_stderr_on_failure() {
        let mut tools = BTreeMap::new();
        tools.insert("sh-fail".to_string(), tool_def("sh", None));
        let runner = ProcessCliRunner::new(tools);
        let request = call_request("sh-fail", None);
        let result = runner.call_tool(&request).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn call_tool_unknown_returns_error() {
        let runner = ProcessCliRunner::new(BTreeMap::new());
        let request = call_request("nope", None);
        let result = runner.call_tool(&request).await;
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
        let result = runner.call_tool(&request).await;
        assert!(result.is_err());
    }

    #[test]
    fn build_tool_descriptor_with_custom_description() {
        let def = tool_def("gh", Some("List PRs"));
        let tool = build_tool_descriptor("gh-pr", &def);
        assert_eq!(tool.name, "gh-pr");
        assert_eq!(tool.description.as_deref(), Some("List PRs"));
    }

    #[test]
    fn build_tool_descriptor_auto_description() {
        let def = tool_def("docker", None);
        let tool = build_tool_descriptor("docker-ps", &def);
        assert_eq!(tool.name, "docker-ps");
        let desc = tool.description.as_deref().unwrap();
        assert!(desc.contains("docker"));
    }
}
