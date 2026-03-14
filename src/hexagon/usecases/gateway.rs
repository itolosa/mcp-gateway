use std::collections::BTreeMap;

use crate::hexagon::entities::policy::allowlist::AllowlistPolicy;
use crate::hexagon::entities::policy::compound::CompoundPolicy;
use crate::hexagon::entities::policy::denylist::DenylistPolicy;
use crate::hexagon::ports::driven::cli_operation_runner::CliOperationRunner;
use crate::hexagon::ports::driven::operation_policy::OperationPolicy;
use crate::hexagon::ports::driven::provider_client::ProviderClient;
use crate::hexagon::ports::driving::get_prompt::{
    GetPromptError, PromptGetRequest, PromptGetResult,
};
use crate::hexagon::ports::driving::list_operations::OperationDescriptor;
use crate::hexagon::ports::driving::list_prompts::PromptDescriptor;
use crate::hexagon::ports::driving::list_resources::{
    ResourceDescriptor, ResourceTemplateDescriptor,
};
use crate::hexagon::ports::driving::read_resource::{
    ReadResourceError, ResourceReadRequest, ResourceReadResult,
};
use crate::hexagon::ports::driving::route_operation::{
    OperationCallRequest, OperationCallResult, RouteOperationError,
};

pub type DefaultPolicy = CompoundPolicy<AllowlistPolicy, DenylistPolicy>;

pub fn create_policy(allowed: Vec<String>, denied: Vec<String>) -> DefaultPolicy {
    CompoundPolicy::new(AllowlistPolicy::new(allowed), DenylistPolicy::new(denied))
}

use super::get_prompt::GetPrompt;
use super::list_operations::ListOperations;
use super::list_prompts::ListPrompts;
use super::list_resources::{ListResourceTemplates, ListResources};
use super::read_resource::ReadResource;
use super::route_operation::RouteOperation;

pub struct ProviderHandle<U, F> {
    pub client: U,
    pub filter: F,
}

pub struct Gateway<U, C, F> {
    providers: BTreeMap<String, ProviderHandle<U, F>>,
    cli_runner: C,
}

impl<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy> Gateway<U, C, F> {
    pub fn new(providers: BTreeMap<String, ProviderHandle<U, F>>, cli_runner: C) -> Self {
        Self {
            providers,
            cli_runner,
        }
    }

    pub async fn list_operations(
        &self,
    ) -> Result<Vec<OperationDescriptor>, std::convert::Infallible> {
        ListOperations::execute(&self.providers, &self.cli_runner).await
    }

    pub async fn route_operation(
        &self,
        request: OperationCallRequest,
    ) -> Result<OperationCallResult, RouteOperationError> {
        RouteOperation::execute(&self.providers, &self.cli_runner, request).await
    }

    pub async fn list_resources(
        &self,
    ) -> Result<Vec<ResourceDescriptor>, std::convert::Infallible> {
        ListResources::execute(&self.providers).await
    }

    pub async fn list_resource_templates(
        &self,
    ) -> Result<Vec<ResourceTemplateDescriptor>, std::convert::Infallible> {
        ListResourceTemplates::execute(&self.providers).await
    }

    pub async fn read_resource(
        &self,
        request: ResourceReadRequest,
    ) -> Result<ResourceReadResult, ReadResourceError> {
        ReadResource::execute(&self.providers, request).await
    }

    pub async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, std::convert::Infallible> {
        ListPrompts::execute(&self.providers).await
    }

    pub async fn get_prompt(
        &self,
        request: PromptGetRequest,
    ) -> Result<PromptGetResult, GetPromptError> {
        GetPrompt::execute(&self.providers, request).await
    }
}
