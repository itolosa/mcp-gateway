use std::collections::BTreeMap;

use rmcp::model::{CallToolRequestParams, CallToolResult, Content, Tool};
use rmcp::ErrorData;
use tokio::io::AsyncWriteExt;

use crate::config::model::CliToolDef;

pub struct CliToolExecutor {
    tools: BTreeMap<String, CliToolDef>,
}

impl CliToolExecutor {
    pub fn new(tools: BTreeMap<String, CliToolDef>) -> Self {
        Self { tools }
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools
            .iter()
            .map(|(name, def)| build_tool_descriptor(name, def))
            .collect()
    }

    pub async fn call_tool(
        &self,
        request: &CallToolRequestParams,
    ) -> Result<CallToolResult, ErrorData> {
        let name = request.name.as_ref();
        let def = self
            .tools
            .get(name)
            .ok_or_else(|| ErrorData::invalid_params(format!("unknown CLI tool: {name}"), None))?;

        let input_json = serde_json::to_string(request).unwrap_or_default();

        let output = run_command(&def.command, &input_json).await.map_err(|e| {
            ErrorData::internal_error(format!("failed to run '{}': {e}", def.command), None)
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            Ok(CallToolResult::success(vec![Content::text(stdout)]))
        } else {
            Ok(CallToolResult::error(vec![Content::text(stderr)]))
        }
    }
}

async fn run_command(command: &str, input: &str) -> std::io::Result<std::process::Output> {
    let mut child = tokio::process::Command::new(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // stdin is always present because we configured Stdio::piped()
    #[allow(clippy::expect_used)]
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let _ = stdin.write_all(input.as_bytes()).await;
    drop(stdin);

    child.wait_with_output().await
}

fn build_tool_descriptor(name: &str, def: &CliToolDef) -> Tool {
    let description = def
        .description
        .clone()
        .unwrap_or_else(|| format!("Execute: {}", def.command));
    let mut schema = serde_json::Map::new();
    schema.insert(
        "type".to_string(),
        serde_json::Value::String("object".to_string()),
    );
    Tool::new(name.to_string(), description, schema)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool_def(command: &str, description: Option<&str>) -> CliToolDef {
        CliToolDef {
            command: command.to_string(),
            description: description.map(|s| s.to_string()),
        }
    }

    fn call_params(
        name: &str,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> CallToolRequestParams {
        let mut params = CallToolRequestParams::new(name.to_string());
        params.arguments = args;
        params
    }

    #[test]
    fn new_empty_executor() {
        let executor = CliToolExecutor::new(BTreeMap::new());
        assert!(executor.list_tools().is_empty());
    }

    #[test]
    fn has_tool_returns_true_for_existing() {
        let mut tools = BTreeMap::new();
        tools.insert("my-tool".to_string(), tool_def("echo", None));
        let executor = CliToolExecutor::new(tools);
        assert!(executor.has_tool("my-tool"));
    }

    #[test]
    fn has_tool_returns_false_for_missing() {
        let executor = CliToolExecutor::new(BTreeMap::new());
        assert!(!executor.has_tool("nope"));
    }

    #[test]
    fn list_tools_returns_descriptors() {
        let mut tools = BTreeMap::new();
        tools.insert("alpha".to_string(), tool_def("echo", Some("Echo tool")));
        tools.insert("beta".to_string(), tool_def("cat", None));
        let executor = CliToolExecutor::new(tools);
        let listed = executor.list_tools();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name.as_ref(), "alpha");
        assert_eq!(listed[1].name.as_ref(), "beta");
    }

    #[test]
    fn list_tools_auto_description() {
        let mut tools = BTreeMap::new();
        tools.insert("t".to_string(), tool_def("git", None));
        let executor = CliToolExecutor::new(tools);
        let listed = executor.list_tools();
        let desc = listed[0].description.as_deref().unwrap();
        assert!(desc.contains("git"));
    }

    #[test]
    fn list_tools_custom_description() {
        let mut tools = BTreeMap::new();
        tools.insert("t".to_string(), tool_def("git", Some("Show git status")));
        let executor = CliToolExecutor::new(tools);
        let listed = executor.list_tools();
        assert_eq!(listed[0].description.as_deref(), Some("Show git status"));
    }

    #[test]
    fn build_tool_descriptor_schema_is_open_object() {
        let def = tool_def("echo", None);
        let tool = build_tool_descriptor("test", &def);
        assert_eq!(tool.input_schema.get("type").unwrap(), "object");
        // No properties or required — scripts parse stdin themselves
        assert!(tool.input_schema.get("properties").is_none());
        assert!(tool.input_schema.get("required").is_none());
    }

    #[tokio::test]
    async fn call_tool_pipes_json_to_stdin() {
        // `cat` reads stdin and echoes it to stdout — verifies JSON piping
        let mut tools = BTreeMap::new();
        tools.insert("cat-tool".to_string(), tool_def("cat", None));
        let executor = CliToolExecutor::new(tools);
        let mut args = serde_json::Map::new();
        args.insert("key".to_string(), json!("value"));
        let params = call_params("cat-tool", Some(args));
        let result = executor.call_tool(&params).await.unwrap();
        let text = result
            .content
            .first()
            .unwrap()
            .as_text()
            .unwrap()
            .text
            .as_str();
        // stdout should contain the serialized CallToolRequestParams
        assert!(text.contains("cat-tool"));
        assert!(text.contains("key"));
        assert!(text.contains("value"));
        assert!(!result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn call_tool_nonzero_exit_is_error() {
        let mut tools = BTreeMap::new();
        tools.insert("false-test".to_string(), tool_def("false", None));
        let executor = CliToolExecutor::new(tools);
        let params = call_params("false-test", None);
        let result = executor.call_tool(&params).await.unwrap();
        assert!(result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn call_tool_captures_stderr_on_failure() {
        // sh reads JSON from stdin as a script, fails to parse it, writes to stderr
        let mut tools = BTreeMap::new();
        tools.insert("sh-fail".to_string(), tool_def("sh", None));
        let executor = CliToolExecutor::new(tools);
        let params = call_params("sh-fail", None);
        let result = executor.call_tool(&params).await.unwrap();
        assert!(result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn call_tool_unknown_returns_error() {
        let executor = CliToolExecutor::new(BTreeMap::new());
        let params = call_params("nope", None);
        let result = executor.call_tool(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("nope"));
    }

    #[tokio::test]
    async fn call_tool_command_not_found_returns_error() {
        let mut tools = BTreeMap::new();
        tools.insert(
            "bad-cmd".to_string(),
            tool_def("/nonexistent_binary_xyz", None),
        );
        let executor = CliToolExecutor::new(tools);
        let params = call_params("bad-cmd", None);
        let result = executor.call_tool(&params).await;
        assert!(result.is_err());
    }

    #[test]
    fn build_tool_descriptor_with_custom_description() {
        let def = tool_def("gh", Some("List PRs"));
        let tool = build_tool_descriptor("gh-pr", &def);
        assert_eq!(tool.name.as_ref(), "gh-pr");
        assert_eq!(tool.description.as_deref(), Some("List PRs"));
    }

    #[test]
    fn build_tool_descriptor_auto_description() {
        let def = tool_def("docker", None);
        let tool = build_tool_descriptor("docker-ps", &def);
        assert_eq!(tool.name.as_ref(), "docker-ps");
        let desc = tool.description.as_deref().unwrap();
        assert!(desc.contains("docker"));
    }
}
