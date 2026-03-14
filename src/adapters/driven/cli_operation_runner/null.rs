use crate::hexagon::ports::driven::cli_operation_runner::{
    CliOperationError, CliOperationRunner, OperationCallRequest, OperationCallResult,
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
    ) -> Result<OperationCallResult, CliOperationError> {
        Err(CliOperationError::Execution(format!(
            "unknown operation: {}",
            request.name
        )))
    }
}
