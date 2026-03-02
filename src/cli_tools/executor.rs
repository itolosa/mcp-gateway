use std::collections::BTreeMap;
use std::sync::Arc;

use rmcp::model::{CallToolRequestParams, CallToolResult, Content, Tool};
use rmcp::ErrorData;

use crate::cli_tools::template::{extract_placeholders, render_args};
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

        let empty_params = serde_json::Map::new();
        let params = request.arguments.as_ref().unwrap_or(&empty_params);
        let rendered =
            render_args(&def.args, params).map_err(|e| ErrorData::invalid_params(e, None))?;

        let output = tokio::process::Command::new(&def.command)
            .args(&rendered)
            .output()
            .await
            .map_err(|e| {
                ErrorData::internal_error(format!("failed to execute '{}': {e}", def.command), None)
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

fn build_input_schema(def: &CliToolDef) -> serde_json::Map<String, serde_json::Value> {
    let placeholders = extract_placeholders(&def.args);
    let mut schema = serde_json::Map::new();
    schema.insert(
        "type".to_string(),
        serde_json::Value::String("object".to_string()),
    );

    if placeholders.is_empty() {
        schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    } else {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for name in &placeholders {
            let mut prop = serde_json::Map::new();
            prop.insert(
                "type".to_string(),
                serde_json::Value::String("string".to_string()),
            );
            properties.insert(name.clone(), serde_json::Value::Object(prop));
            required.push(serde_json::Value::String(name.clone()));
        }
        schema.insert(
            "properties".to_string(),
            serde_json::Value::Object(properties),
        );
        schema.insert("required".to_string(), serde_json::Value::Array(required));
    }

    schema
}

fn build_tool_descriptor(name: &str, def: &CliToolDef) -> Tool {
    let description = def
        .description
        .clone()
        .unwrap_or_else(|| format!("Execute: {} {}", def.command, def.args.join(" ")));
    let input_schema = build_input_schema(def);
    Tool {
        name: name.to_string().into(),
        title: None,
        description: Some(description.into()),
        input_schema: Arc::new(input_schema),
        output_schema: None,
        annotations: None,
        execution: None,
        icons: None,
        meta: None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool_def(command: &str, args: &[&str], description: Option<&str>) -> CliToolDef {
        CliToolDef {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            description: description.map(|s| s.to_string()),
        }
    }

    fn call_params(
        name: &str,
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> CallToolRequestParams {
        CallToolRequestParams {
            name: name.to_string().into(),
            arguments: args,
            meta: None,
            task: None,
        }
    }

    // --- CliToolExecutor tests ---

    #[test]
    fn new_empty_executor() {
        let executor = CliToolExecutor::new(BTreeMap::new());
        assert!(executor.list_tools().is_empty());
    }

    #[test]
    fn has_tool_returns_true_for_existing() {
        let mut tools = BTreeMap::new();
        tools.insert("my-tool".to_string(), tool_def("echo", &["hello"], None));
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
        tools.insert(
            "echo-tool".to_string(),
            tool_def("echo", &["hello"], Some("Echo hello")),
        );
        tools.insert("ls-tool".to_string(), tool_def("ls", &[], None));
        let executor = CliToolExecutor::new(tools);
        let listed = executor.list_tools();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].name.as_ref(), "echo-tool");
        assert_eq!(listed[1].name.as_ref(), "ls-tool");
    }

    #[test]
    fn list_tools_auto_description() {
        let mut tools = BTreeMap::new();
        tools.insert("t".to_string(), tool_def("git", &["status"], None));
        let executor = CliToolExecutor::new(tools);
        let listed = executor.list_tools();
        let desc = listed[0].description.as_deref().unwrap();
        assert!(desc.contains("git"));
        assert!(desc.contains("status"));
    }

    #[test]
    fn list_tools_custom_description() {
        let mut tools = BTreeMap::new();
        tools.insert(
            "t".to_string(),
            tool_def("git", &["status"], Some("Show git status")),
        );
        let executor = CliToolExecutor::new(tools);
        let listed = executor.list_tools();
        assert_eq!(listed[0].description.as_deref(), Some("Show git status"));
    }

    #[tokio::test]
    async fn call_tool_success_captures_stdout() {
        let mut tools = BTreeMap::new();
        tools.insert(
            "echo-test".to_string(),
            tool_def("echo", &["hello world"], None),
        );
        let executor = CliToolExecutor::new(tools);
        let params = call_params("echo-test", None);
        let result = executor.call_tool(&params).await.unwrap();
        let text = result
            .content
            .first()
            .unwrap()
            .as_text()
            .unwrap()
            .text
            .as_str();
        assert!(text.contains("hello world"));
        assert!(!result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn call_tool_failure_captures_stderr() {
        let mut tools = BTreeMap::new();
        // `ls` on a nonexistent path writes to stderr and exits nonzero
        tools.insert(
            "fail-test".to_string(),
            tool_def("ls", &["/nonexistent_path_xyz"], None),
        );
        let executor = CliToolExecutor::new(tools);
        let params = call_params("fail-test", None);
        let result = executor.call_tool(&params).await.unwrap();
        assert!(result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn call_tool_nonzero_exit_is_error() {
        let mut tools = BTreeMap::new();
        tools.insert("false-test".to_string(), tool_def("false", &[], None));
        let executor = CliToolExecutor::new(tools);
        let params = call_params("false-test", None);
        let result = executor.call_tool(&params).await.unwrap();
        assert!(result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn call_tool_with_template_rendering() {
        let mut tools = BTreeMap::new();
        tools.insert(
            "echo-arg".to_string(),
            tool_def("echo", &["{{message}}"], None),
        );
        let executor = CliToolExecutor::new(tools);
        let mut args = serde_json::Map::new();
        args.insert("message".to_string(), json!("templated"));
        let params = call_params("echo-arg", Some(args));
        let result = executor.call_tool(&params).await.unwrap();
        let text = result
            .content
            .first()
            .unwrap()
            .as_text()
            .unwrap()
            .text
            .as_str();
        assert!(text.contains("templated"));
    }

    #[tokio::test]
    async fn call_tool_missing_arg_returns_error() {
        let mut tools = BTreeMap::new();
        tools.insert(
            "need-arg".to_string(),
            tool_def("echo", &["{{required}}"], None),
        );
        let executor = CliToolExecutor::new(tools);
        let params = call_params("need-arg", None);
        let result = executor.call_tool(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("required"));
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
            tool_def("/nonexistent_binary_xyz", &[], None),
        );
        let executor = CliToolExecutor::new(tools);
        let params = call_params("bad-cmd", None);
        let result = executor.call_tool(&params).await;
        assert!(result.is_err());
    }

    // --- build_input_schema tests ---

    #[test]
    fn build_input_schema_empty_args() {
        let def = tool_def("echo", &[], None);
        let schema = build_input_schema(&def);
        assert_eq!(schema.get("type").unwrap(), "object");
        assert!(schema.get("required").is_none());
    }

    #[test]
    fn build_input_schema_with_placeholders() {
        let def = tool_def("gh", &["pr", "list", "--repo", "{{repo}}"], None);
        let schema = build_input_schema(&def);
        let props = schema.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("repo"));
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert_eq!(required, &[json!("repo")]);
    }

    // --- build_tool_descriptor tests ---

    #[test]
    fn build_tool_descriptor_with_description() {
        let def = tool_def("gh", &["pr", "list"], Some("List PRs"));
        let tool = build_tool_descriptor("gh-pr", &def);
        assert_eq!(tool.name.as_ref(), "gh-pr");
        assert_eq!(tool.description.as_deref(), Some("List PRs"));
    }

    #[test]
    fn build_tool_descriptor_without_description() {
        let def = tool_def("docker", &["ps"], None);
        let tool = build_tool_descriptor("docker-ps", &def);
        assert_eq!(tool.name.as_ref(), "docker-ps");
        let desc = tool.description.as_deref().unwrap();
        assert!(desc.contains("docker"));
        assert!(desc.contains("ps"));
    }
}
