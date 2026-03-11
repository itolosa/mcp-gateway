use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;

/// A tool descriptor exposed by the gateway.
#[derive(Debug, Clone)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: Option<String>,
    /// Opaque JSON schema string — the hexagon passes it through untouched.
    pub schema: String,
}

/// A request to call a tool.
#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    pub name: String,
    /// Opaque JSON arguments string — the hexagon passes it through untouched.
    pub arguments: Option<String>,
}

/// Result of a tool call.
#[derive(Debug, Clone)]
pub struct ToolCallResult {
    /// Opaque content items — the hexagon passes them through untouched.
    pub content: Vec<String>,
    pub is_error: bool,
}

/// Error from upstream operations.
#[derive(Debug)]
pub enum UpstreamError {
    Service(String),
}

impl fmt::Display for UpstreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for UpstreamError {}

/// Error from gateway operations.
#[derive(Debug)]
pub enum GatewayError {
    NoPrefix { tool: String },
    UnknownServer { server: String, tool: String },
    ToolNotAllowed { tool: String },
    Upstream(String),
    CliTool(String),
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPrefix { tool } => write!(f, "tool '{tool}' has no server prefix"),
            Self::UnknownServer { server, tool } => {
                write!(f, "unknown server '{server}' in tool '{tool}'")
            }
            Self::ToolNotAllowed { tool } => write!(f, "tool '{tool}' is not allowed"),
            Self::Upstream(msg) => write!(f, "upstream error: {msg}"),
            Self::CliTool(msg) => write!(f, "CLI tool error: {msg}"),
        }
    }
}

impl std::error::Error for GatewayError {}

/// Driven port: client to an upstream MCP server.
pub trait UpstreamClient: Send + Sync {
    fn list_tools(&self)
        -> impl Future<Output = Result<Vec<ToolDescriptor>, UpstreamError>> + Send;
    fn call_tool(
        &self,
        request: ToolCallRequest,
    ) -> impl Future<Output = Result<ToolCallResult, UpstreamError>> + Send;
}

/// Driven port: tool filter for upstream tools.
pub trait ToolFilter: Send + Sync {
    fn is_tool_allowed(&self, tool_name: &str) -> bool;
}

/// Driven port: runner for host CLI tools.
pub trait CliToolRunner: Send + Sync {
    fn list_tools(&self) -> Vec<ToolDescriptor>;
    fn has_tool(&self, name: &str) -> bool;
    fn call_tool(
        &self,
        request: &ToolCallRequest,
    ) -> impl Future<Output = Result<ToolCallResult, GatewayError>> + Send;
}

/// Driven port: a server entry with tool filter lists.
pub trait ServerEntry: Send + Sync {
    fn allowed_tools(&self) -> &[String];
    fn allowed_tools_mut(&mut self) -> &mut Vec<String>;
    fn denied_tools(&self) -> &[String];
    fn denied_tools_mut(&mut self) -> &mut Vec<String>;
}

/// Driven port: persistent storage for server entries.
pub trait ServerConfigStore: Send + Sync {
    type Entry: ServerEntry;
    fn load_entries(&self) -> Result<BTreeMap<String, Self::Entry>, String>;
    fn save_entries(&self, entries: BTreeMap<String, Self::Entry>) -> Result<(), String>;
}
