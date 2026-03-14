use std::future::Future;
use std::time::Duration;

use rmcp::model::{CallToolRequestParams, GetPromptRequestParams, ReadResourceRequestParams};
use rmcp::service::{RoleClient, RunningService};

use crate::hexagon::ports::driven::provider_client::{
    OperationCallRequest, OperationCallResult, OperationDescriptor, PromptDescriptor,
    PromptGetRequest, PromptGetResult, ProviderClient, ProviderError, ResourceDescriptor,
    ResourceReadRequest, ResourceReadResult, ResourceTemplateDescriptor,
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

    async fn execute<T, E: std::fmt::Display>(
        &self,
        fut: impl Future<Output = Result<T, E>> + Send,
    ) -> Result<T, ProviderError> {
        let result = match self.operation_timeout {
            Some(timeout) => tokio::time::timeout(timeout, fut)
                .await
                .map_err(|_| ProviderError::Service("operation timed out".to_string()))?,
            None => fut.await,
        };
        result.map_err(|e| ProviderError::Service(e.to_string()))
    }
}

impl ProviderClient for RmcpProviderClient {
    async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
        let result = self.execute(self.service.list_tools(None)).await?;
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
        let result = self.execute(self.service.call_tool(params)).await?;
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

    async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, ProviderError> {
        let result = self.execute(self.service.list_resources(None)).await?;
        Ok(result
            .resources
            .into_iter()
            .map(|r| ResourceDescriptor {
                uri: r.uri.clone(),
                name: r.name.clone(),
                json: serde_json::to_string(&r).unwrap_or_default(),
            })
            .collect())
    }

    async fn list_resource_templates(
        &self,
    ) -> Result<Vec<ResourceTemplateDescriptor>, ProviderError> {
        let result = self
            .execute(self.service.list_resource_templates(None))
            .await?;
        Ok(result
            .resource_templates
            .into_iter()
            .map(|t| ResourceTemplateDescriptor {
                uri_template: t.uri_template.clone(),
                name: t.name.clone(),
                json: serde_json::to_string(&t).unwrap_or_default(),
            })
            .collect())
    }

    async fn read_resource(
        &self,
        request: ResourceReadRequest,
    ) -> Result<ResourceReadResult, ProviderError> {
        let params = ReadResourceRequestParams::new(request.uri);
        let result = self.execute(self.service.read_resource(params)).await?;
        Ok(ResourceReadResult {
            json: serde_json::to_string(&result).unwrap_or_default(),
        })
    }

    async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, ProviderError> {
        let result = self.execute(self.service.list_prompts(None)).await?;
        Ok(result
            .prompts
            .into_iter()
            .map(|p| PromptDescriptor {
                name: p.name.clone(),
                json: serde_json::to_string(&p).unwrap_or_default(),
            })
            .collect())
    }

    async fn get_prompt(
        &self,
        request: PromptGetRequest,
    ) -> Result<PromptGetResult, ProviderError> {
        let mut params = GetPromptRequestParams::new(request.name);
        params.arguments = request
            .arguments
            .and_then(|s| serde_json::from_str(&s).ok());
        let result = self.execute(self.service.get_prompt(params)).await?;
        Ok(PromptGetResult {
            json: serde_json::to_string(&result).unwrap_or_default(),
        })
    }
}
