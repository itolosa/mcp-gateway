/// A tool descriptor exposed by the gateway.
#[derive(Debug, Clone)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub schema: serde_json::Map<String, serde_json::Value>,
}

/// A request to call a tool.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Result of a tool call.
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    pub content: Vec<serde_json::Value>,
    pub is_error: bool,
}

impl ToolCallResult {
    pub fn text_success(text: String) -> Self {
        Self {
            content: vec![serde_json::json!({"type": "text", "text": text})],
            is_error: false,
        }
    }

    pub fn text_error(text: String) -> Self {
        Self {
            content: vec![serde_json::json!({"type": "text", "text": text})],
            is_error: true,
        }
    }
}

/// Error from upstream operations.
#[derive(Debug, thiserror::Error)]
pub enum UpstreamError {
    #[error("{0}")]
    Service(String),
}

/// Error from gateway operations.
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("tool '{tool}' has no server prefix")]
    NoPrefix { tool: String },

    #[error("unknown server '{server}' in tool '{tool}'")]
    UnknownServer { server: String, tool: String },

    #[error("tool '{tool}' is not allowed")]
    ToolNotAllowed { tool: String },

    #[error("upstream server '{server}' timed out after {timeout_secs}s")]
    UpstreamTimeout { server: String, timeout_secs: u64 },

    #[error("upstream error: {0}")]
    Upstream(String),

    #[error("CLI tool error: {0}")]
    CliTool(String),
}
