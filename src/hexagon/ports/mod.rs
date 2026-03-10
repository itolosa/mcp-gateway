use std::collections::BTreeMap;
use std::future::Future;

pub use crate::hexagon::entities::{
    GatewayError, ToolCallRequest, ToolCallResult, ToolDescriptor, UpstreamError,
};

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
