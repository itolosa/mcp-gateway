use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationCallRequest, OperationCallResult,
    OperationDescriptor,
};

/// No-op CLI runner for when no CLI tools are configured.
pub struct NullCliRunner;

impl CliOperationRunner for NullCliRunner {
    fn list_operations(&self) -> Vec<OperationDescriptor> {
        vec![]
    }

    fn has_operation(&self, _name: &str) -> bool {
        false
    }

    async fn call_operation(
        &self,
        request: &OperationCallRequest,
    ) -> Result<OperationCallResult, GatewayError> {
        Err(GatewayError::CliOperation(format!(
            "unknown operation: {}",
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
        assert!(NullCliRunner.list_operations().is_empty());
    }

    #[test]
    fn has_tool_returns_false() {
        assert!(!NullCliRunner.has_operation("anything"));
    }

    #[tokio::test]
    async fn call_tool_returns_error() {
        let request = OperationCallRequest {
            name: "test".to_string(),
            arguments: None,
        };
        let result = NullCliRunner.call_operation(&request).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("test"));
    }
}
