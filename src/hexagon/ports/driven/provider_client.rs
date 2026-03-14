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

#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    pub uri: String,
    pub name: String,
    pub json: String,
}

#[derive(Debug, Clone)]
pub struct ResourceTemplateDescriptor {
    pub uri_template: String,
    pub name: String,
    pub json: String,
}

#[derive(Debug, Clone)]
pub struct ResourceReadRequest {
    pub uri: String,
}

#[derive(Debug, Clone)]
pub struct ResourceReadResult {
    pub json: String,
}

#[derive(Debug, Clone)]
pub struct PromptDescriptor {
    pub name: String,
    pub json: String,
}

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
pub enum ProviderError {
    Service(String),
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ProviderError {}

/// Driven port: client to a provider (upstream MCP server).
pub trait ProviderClient: Send + Sync {
    fn list_operations(
        &self,
    ) -> impl Future<Output = Result<Vec<OperationDescriptor>, ProviderError>> + Send;
    fn call_operation(
        &self,
        request: OperationCallRequest,
    ) -> impl Future<Output = Result<OperationCallResult, ProviderError>> + Send;
    fn list_resources(
        &self,
    ) -> impl Future<Output = Result<Vec<ResourceDescriptor>, ProviderError>> + Send;
    fn list_resource_templates(
        &self,
    ) -> impl Future<Output = Result<Vec<ResourceTemplateDescriptor>, ProviderError>> + Send;
    fn read_resource(
        &self,
        request: ResourceReadRequest,
    ) -> impl Future<Output = Result<ResourceReadResult, ProviderError>> + Send;
    fn list_prompts(
        &self,
    ) -> impl Future<Output = Result<Vec<PromptDescriptor>, ProviderError>> + Send;
    fn get_prompt(
        &self,
        request: PromptGetRequest,
    ) -> impl Future<Output = Result<PromptGetResult, ProviderError>> + Send;
}
