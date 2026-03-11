use crate::hexagon::ports::{
    CliToolRunner, GatewayError, ToolCallRequest, ToolCallResult, ToolDescriptor,
};

/// No-op CLI runner for when no CLI tools are configured.
pub struct NullCliRunner;

impl CliToolRunner for NullCliRunner {
    fn list_tools(&self) -> Vec<ToolDescriptor> {
        vec![]
    }

    fn has_tool(&self, _name: &str) -> bool {
        false
    }

    async fn call_tool(&self, request: &ToolCallRequest) -> Result<ToolCallResult, GatewayError> {
        Err(GatewayError::CliTool(format!(
            "unknown tool: {}",
            request.name
        )))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_tools_returns_empty() {
        assert!(NullCliRunner.list_tools().is_empty());
    }

    #[test]
    fn has_tool_returns_false() {
        assert!(!NullCliRunner.has_tool("anything"));
    }

    #[tokio::test]
    async fn call_tool_returns_error() {
        let request = ToolCallRequest {
            name: "test".to_string(),
            arguments: None,
        };
        let result = NullCliRunner.call_tool(&request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("test"));
    }
}
