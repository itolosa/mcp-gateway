use rmcp::model::CallToolRequestParams;
use rmcp::service::{RoleClient, RunningService};

use crate::hexagon::entities::{ToolCallRequest, ToolCallResult, ToolDescriptor, UpstreamError};
use crate::hexagon::ports::UpstreamClient;

pub struct RmcpUpstreamClient {
    service: RunningService<RoleClient, ()>,
}

impl RmcpUpstreamClient {
    pub fn new(service: RunningService<RoleClient, ()>) -> Self {
        Self { service }
    }
}

impl UpstreamClient for RmcpUpstreamClient {
    async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
        let result = self
            .service
            .list_tools(None)
            .await
            .map_err(|e| UpstreamError::Service(e.to_string()))?;
        Ok(result
            .tools
            .into_iter()
            .map(|t| ToolDescriptor {
                name: t.name.to_string(),
                description: t.description.map(|d| d.to_string()),
                schema: (*t.input_schema).clone(),
            })
            .collect())
    }

    async fn call_tool(&self, request: ToolCallRequest) -> Result<ToolCallResult, UpstreamError> {
        let mut params = CallToolRequestParams::new(request.name);
        params.arguments = request.arguments;
        let result = self
            .service
            .call_tool(params)
            .await
            .map_err(|e| UpstreamError::Service(e.to_string()))?;
        let content = result
            .content
            .into_iter()
            .map(|c| serde_json::to_value(c).unwrap_or_default())
            .collect();
        Ok(ToolCallResult {
            content,
            is_error: result.is_error.unwrap_or(false),
        })
    }
}
