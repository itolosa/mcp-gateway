use std::collections::BTreeMap;
use std::fmt;
use std::future::Future;

/// An operation descriptor exposed by the gateway.
#[derive(Debug, Clone)]
pub struct OperationDescriptor {
    pub name: String,
    pub description: Option<String>,
    /// Opaque JSON schema string — the hexagon passes it through untouched.
    pub schema: String,
}

/// A request to call an operation.
#[derive(Debug, Clone)]
pub struct OperationCallRequest {
    pub name: String,
    /// Opaque JSON arguments string — the hexagon passes it through untouched.
    pub arguments: Option<String>,
}

/// Result of an operation call.
#[derive(Debug, Clone)]
pub struct OperationCallResult {
    /// Opaque content items — the hexagon passes them through untouched.
    pub content: Vec<String>,
    pub is_error: bool,
}

/// A resource descriptor exposed by the gateway.
#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    pub uri: String,
    pub name: String,
    /// Opaque JSON string — the hexagon passes it through untouched.
    pub json: String,
}

/// A resource template descriptor exposed by the gateway.
#[derive(Debug, Clone)]
pub struct ResourceTemplateDescriptor {
    pub uri_template: String,
    pub name: String,
    /// Opaque JSON string — the hexagon passes it through untouched.
    pub json: String,
}

/// A request to read a resource.
#[derive(Debug, Clone)]
pub struct ResourceReadRequest {
    pub uri: String,
}

/// Result of reading a resource.
#[derive(Debug, Clone)]
pub struct ResourceReadResult {
    /// Opaque JSON string — the hexagon passes it through untouched.
    pub json: String,
}

/// A prompt descriptor exposed by the gateway.
#[derive(Debug, Clone)]
pub struct PromptDescriptor {
    pub name: String,
    /// Opaque JSON string — the hexagon passes it through untouched.
    pub json: String,
}

/// A request to get a prompt.
#[derive(Debug, Clone)]
pub struct PromptGetRequest {
    pub name: String,
    /// Opaque JSON arguments string — the hexagon passes it through untouched.
    pub arguments: Option<String>,
}

/// Result of getting a prompt.
#[derive(Debug, Clone)]
pub struct PromptGetResult {
    /// Opaque JSON string — the hexagon passes it through untouched.
    pub json: String,
}

/// Error from provider operations.
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

/// Error from gateway operations.
#[derive(Debug)]
pub enum GatewayError {
    InvalidMapping { operation: String },
    UnknownProvider { provider: String, operation: String },
    OperationNotAllowed { operation: String },
    Provider(String),
    CliOperation(String),
}

impl fmt::Display for GatewayError {
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

impl std::error::Error for GatewayError {}

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

/// Driven port: operation policy for provider operations.
pub trait OperationPolicy: Send + Sync {
    fn is_allowed(&self, operation_name: &str) -> bool;
}

/// Driven port: runner for host CLI operations.
pub trait CliOperationRunner: Send + Sync {
    fn list_operations(&self) -> Vec<OperationDescriptor>;
    fn has_operation(&self, name: &str) -> bool;
    fn call_operation(
        &self,
        request: &OperationCallRequest,
    ) -> impl Future<Output = Result<OperationCallResult, GatewayError>> + Send;
}

/// Driven port: a provider entry with operation policy lists.
pub trait ProviderEntry: Send + Sync {
    fn allowed_operations(&self) -> &[String];
    fn allowed_operations_mut(&mut self) -> &mut Vec<String>;
    fn denied_operations(&self) -> &[String];
    fn denied_operations_mut(&mut self) -> &mut Vec<String>;
}

/// Driven port: persistent storage for provider entries.
pub trait ProviderConfigStore: Send + Sync {
    type Entry: ProviderEntry;
    fn load_entries(&self) -> Result<BTreeMap<String, Self::Entry>, String>;
    fn save_entries(&self, entries: BTreeMap<String, Self::Entry>) -> Result<(), String>;
}
