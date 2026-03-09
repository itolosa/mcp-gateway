use std::future::Future;

use crate::hexagon::entities::{
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
