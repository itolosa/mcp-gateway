use std::fmt;

#[derive(Debug, Clone)]
pub struct PromptGetRequest {
    pub name: String,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PromptGetResult {
    pub json: String,
}

#[derive(Debug)]
pub enum GetPromptError {
    InvalidMapping { operation: String },
    UnknownProvider { provider: String, operation: String },
    Provider(String),
}

impl fmt::Display for GetPromptError {
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
            Self::Provider(msg) => write!(f, "provider error: {msg}"),
        }
    }
}

impl std::error::Error for GetPromptError {}
