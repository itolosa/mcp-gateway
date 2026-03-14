use std::fmt;
use std::future::Future;

#[derive(Debug, Clone)]
pub struct OperationDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub schema: String,
}

#[derive(Debug, Clone)]
pub struct OperationCallRequest {
    pub name: String,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OperationCallResult {
    pub content: Vec<String>,
    pub is_error: bool,
}

#[derive(Debug)]
pub enum CliOperationError {
    Execution(String),
}

impl fmt::Display for CliOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execution(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for CliOperationError {}

/// Driven port: runner for host CLI operations.
pub trait CliOperationRunner: Send + Sync {
    fn list_operations(&self) -> Vec<OperationDescriptor>;
    fn has_operation(&self, name: &str) -> bool;
    fn call_operation(
        &self,
        request: &OperationCallRequest,
    ) -> impl Future<Output = Result<OperationCallResult, CliOperationError>> + Send;
}
