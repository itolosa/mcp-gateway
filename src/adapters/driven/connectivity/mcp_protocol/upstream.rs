use std::time::Duration;

use rmcp::model::CallToolRequestParams;
use rmcp::service::{RoleClient, RunningService};

use crate::hexagon::ports::{
    OperationCallRequest, OperationCallResult, OperationDescriptor, ProviderClient, ProviderError,
};

pub struct RmcpProviderClient {
    service: RunningService<RoleClient, ()>,
    operation_timeout: Option<Duration>,
}

impl RmcpProviderClient {
    pub fn new(service: RunningService<RoleClient, ()>) -> Self {
        Self {
            service,
            operation_timeout: None,
        }
    }

    pub fn with_operation_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = Some(timeout);
        self
    }
}

impl ProviderClient for RmcpProviderClient {
    async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
        let fut = self.service.list_tools(None);
        let rmcp_result = if let Some(timeout) = self.operation_timeout {
            tokio::time::timeout(timeout, fut)
                .await
                .map_err(|_| ProviderError::Service("operation timed out".to_string()))?
        } else {
            fut.await
        };
        let result = rmcp_result.map_err(|e| ProviderError::Service(e.to_string()))?;
        Ok(result
            .tools
            .into_iter()
            .map(|t| OperationDescriptor {
                name: t.name.to_string(),
                description: t.description.map(|d| d.to_string()),
                schema: serde_json::to_string(&*t.input_schema).unwrap_or_default(),
            })
            .collect())
    }

    async fn call_operation(
        &self,
        request: OperationCallRequest,
    ) -> Result<OperationCallResult, ProviderError> {
        let mut params = CallToolRequestParams::new(request.name);
        params.arguments = request
            .arguments
            .and_then(|s| serde_json::from_str(&s).ok());
        let fut = self.service.call_tool(params);
        let rmcp_result = if let Some(timeout) = self.operation_timeout {
            tokio::time::timeout(timeout, fut)
                .await
                .map_err(|_| ProviderError::Service("operation timed out".to_string()))?
        } else {
            fut.await
        };
        let result = rmcp_result.map_err(|e| ProviderError::Service(e.to_string()))?;
        let content = result
            .content
            .into_iter()
            .map(|c| serde_json::to_string(&c).unwrap_or_default())
            .collect();
        Ok(OperationCallResult {
            content,
            is_error: result.is_error.unwrap_or(false),
        })
    }
}
