use std::fmt;

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
pub enum RouteOperationError {
    InvalidMapping { operation: String },
    UnknownProvider { provider: String, operation: String },
    OperationNotAllowed { operation: String },
    Provider(String),
    CliOperation(String),
}

impl fmt::Display for RouteOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMapping { operation } => {
                write!(f, "operation '{operation}' has no provider prefix")
            }
            Self::UnknownProvider {
                provider,
                operation,
            } => {
                write!(
                    f,
                    "unknown provider '{provider}' in operation '{operation}'"
                )
            }
            Self::OperationNotAllowed { operation } => {
                write!(f, "operation '{operation}' is not allowed")
            }
            Self::Provider(msg) => write!(f, "provider error: {msg}"),
            Self::CliOperation(msg) => write!(f, "CLI operation error: {msg}"),
        }
    }
}

impl std::error::Error for RouteOperationError {}
