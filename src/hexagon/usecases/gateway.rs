use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationCallRequest, OperationCallResult,
    OperationDescriptor, OperationPolicy, PromptDescriptor, PromptGetRequest, PromptGetResult,
    ProviderClient, ResourceDescriptor, ResourceReadRequest, ResourceReadResult,
    ResourceTemplateDescriptor,
};

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

    pub async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, GatewayError> {
        ListOperations::execute(&self.providers, &self.cli_runner).await
    }

    pub async fn route_operation(
        &self,
        request: OperationCallRequest,
    ) -> Result<OperationCallResult, GatewayError> {
        RouteOperation::execute(&self.providers, &self.cli_runner, request).await
    }

    pub async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, GatewayError> {
        ListResources::execute(&self.providers).await
    }

    pub async fn list_resource_templates(
        &self,
    ) -> Result<Vec<ResourceTemplateDescriptor>, GatewayError> {
        ListResourceTemplates::execute(&self.providers).await
    }

    pub async fn read_resource(
        &self,
        request: ResourceReadRequest,
    ) -> Result<ResourceReadResult, GatewayError> {
        ReadResource::execute(&self.providers, request).await
    }

    pub async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, GatewayError> {
        ListPrompts::execute(&self.providers).await
    }

    pub async fn get_prompt(
        &self,
        request: PromptGetRequest,
    ) -> Result<PromptGetResult, GatewayError> {
        GetPrompt::execute(&self.providers, request).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
pub(crate) mod test_helpers {
    use std::collections::BTreeMap;

    use crate::hexagon::entities::policy::allowlist::AllowlistPolicy;
    use crate::hexagon::entities::policy::compound::CompoundPolicy;
    use crate::hexagon::entities::policy::denylist::DenylistPolicy;
    use crate::hexagon::ports::{
        CliOperationRunner, GatewayError, OperationCallRequest, OperationCallResult,
        OperationDescriptor, PromptDescriptor, PromptGetRequest, PromptGetResult, ProviderClient,
        ProviderError, ResourceDescriptor, ResourceReadRequest, ResourceReadResult,
        ResourceTemplateDescriptor,
    };

    use super::ProviderHandle;

    pub(crate) struct MockServerA;

    impl ProviderClient for MockServerA {
        async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
            Ok(vec![OperationDescriptor {
                name: "echo".to_string(),
                description: Some("echoes input".to_string()),
                schema: r#"{"type":"object","properties":{"message":{"type":"string"}}}"#
                    .to_string(),
            }])
        }

        async fn call_operation(
            &self,
            request: OperationCallRequest,
        ) -> Result<OperationCallResult, ProviderError> {
            if request.name == "echo" {
                let input = request.arguments.unwrap_or_default();
                Ok(OperationCallResult {
                    content: vec![input],
                    is_error: false,
                })
            } else {
                Err(ProviderError::Service(format!(
                    "unknown tool: {}",
                    request.name
                )))
            }
        }

        async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, ProviderError> {
            Ok(vec![])
        }
        async fn list_resource_templates(
            &self,
        ) -> Result<Vec<ResourceTemplateDescriptor>, ProviderError> {
            Ok(vec![])
        }
        async fn read_resource(
            &self,
            request: ResourceReadRequest,
        ) -> Result<ResourceReadResult, ProviderError> {
            Ok(ResourceReadResult {
                json: format!(r#"{{"uri":"{}"}}"#, request.uri),
            })
        }
        async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, ProviderError> {
            Ok(vec![])
        }
        async fn get_prompt(
            &self,
            request: PromptGetRequest,
        ) -> Result<PromptGetResult, ProviderError> {
            Ok(PromptGetResult {
                json: format!(r#"{{"name":"{}"}}"#, request.name),
            })
        }
    }

    pub(crate) struct MockServerB;

    impl ProviderClient for MockServerB {
        async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
            Ok(vec![OperationDescriptor {
                name: "read_file".to_string(),
                description: Some("reads a file".to_string()),
                schema: r#"{"type":"object","properties":{"path":{"type":"string"}}}"#.to_string(),
            }])
        }

        async fn call_operation(
            &self,
            request: OperationCallRequest,
        ) -> Result<OperationCallResult, ProviderError> {
            if request.name == "read_file" {
                let args = request.arguments.unwrap_or_default();
                Ok(OperationCallResult {
                    content: vec![format!("content from {args}")],
                    is_error: false,
                })
            } else {
                Err(ProviderError::Service(format!(
                    "unknown tool: {}",
                    request.name
                )))
            }
        }

        async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, ProviderError> {
            Ok(vec![])
        }
        async fn list_resource_templates(
            &self,
        ) -> Result<Vec<ResourceTemplateDescriptor>, ProviderError> {
            Ok(vec![])
        }
        async fn read_resource(
            &self,
            request: ResourceReadRequest,
        ) -> Result<ResourceReadResult, ProviderError> {
            Ok(ResourceReadResult {
                json: format!(r#"{{"uri":"{}"}}"#, request.uri),
            })
        }
        async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, ProviderError> {
            Ok(vec![])
        }
        async fn get_prompt(
            &self,
            request: PromptGetRequest,
        ) -> Result<PromptGetResult, ProviderError> {
            Ok(PromptGetResult {
                json: format!(r#"{{"name":"{}"}}"#, request.name),
            })
        }
    }

    pub(crate) struct DualMockServer {
        pub(crate) server_name: &'static str,
    }

    impl ProviderClient for DualMockServer {
        async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
            if self.server_name == "alpha" {
                MockServerA.list_operations().await
            } else {
                MockServerB.list_operations().await
            }
        }

        async fn call_operation(
            &self,
            request: OperationCallRequest,
        ) -> Result<OperationCallResult, ProviderError> {
            if self.server_name == "alpha" {
                MockServerA.call_operation(request).await
            } else {
                MockServerB.call_operation(request).await
            }
        }

        async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, ProviderError> {
            if self.server_name == "alpha" {
                MockServerA.list_resources().await
            } else {
                MockServerB.list_resources().await
            }
        }
        async fn list_resource_templates(
            &self,
        ) -> Result<Vec<ResourceTemplateDescriptor>, ProviderError> {
            if self.server_name == "alpha" {
                MockServerA.list_resource_templates().await
            } else {
                MockServerB.list_resource_templates().await
            }
        }
        async fn read_resource(
            &self,
            request: ResourceReadRequest,
        ) -> Result<ResourceReadResult, ProviderError> {
            if self.server_name == "alpha" {
                MockServerA.read_resource(request).await
            } else {
                MockServerB.read_resource(request).await
            }
        }
        async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, ProviderError> {
            if self.server_name == "alpha" {
                MockServerA.list_prompts().await
            } else {
                MockServerB.list_prompts().await
            }
        }
        async fn get_prompt(
            &self,
            request: PromptGetRequest,
        ) -> Result<PromptGetResult, ProviderError> {
            if self.server_name == "alpha" {
                MockServerA.get_prompt(request).await
            } else {
                MockServerB.get_prompt(request).await
            }
        }
    }

    pub(crate) fn passthrough_filter() -> CompoundPolicy<AllowlistPolicy, DenylistPolicy> {
        CompoundPolicy::new(AllowlistPolicy::new(vec![]), DenylistPolicy::new(vec![]))
    }

    pub(crate) type TestFilter = CompoundPolicy<AllowlistPolicy, DenylistPolicy>;

    pub(crate) fn two_server_setup() -> BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>>
    {
        let mut providers = BTreeMap::new();
        providers.insert(
            "alpha".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: passthrough_filter(),
            },
        );
        providers.insert(
            "beta".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "beta",
                },
                filter: passthrough_filter(),
            },
        );
        providers
    }

    pub(crate) struct MockCliRunner;

    impl CliOperationRunner for MockCliRunner {
        fn list_operations(&self) -> Vec<OperationDescriptor> {
            vec![OperationDescriptor {
                name: "cli-cat".to_string(),
                description: Some("Cat stdin to stdout".to_string()),
                schema: r#"{"type":"object"}"#.to_string(),
            }]
        }

        fn has_operation(&self, name: &str) -> bool {
            name == "cli-cat"
        }

        async fn call_operation(
            &self,
            _request: &OperationCallRequest,
        ) -> Result<OperationCallResult, GatewayError> {
            Ok(OperationCallResult {
                content: vec!["cli-cat output".to_string()],
                is_error: false,
            })
        }
    }

    pub(crate) struct TestProvider {
        pub operations: Vec<OperationDescriptor>,
        pub resources: Vec<ResourceDescriptor>,
        pub templates: Vec<ResourceTemplateDescriptor>,
        pub prompts: Vec<PromptDescriptor>,
    }

    impl TestProvider {
        pub(crate) fn empty() -> Self {
            Self {
                operations: vec![],
                resources: vec![],
                templates: vec![],
                prompts: vec![],
            }
        }
    }

    impl ProviderClient for TestProvider {
        async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
            Ok(self.operations.clone())
        }
        async fn call_operation(
            &self,
            _request: OperationCallRequest,
        ) -> Result<OperationCallResult, ProviderError> {
            Err(ProviderError::Service("not supported".to_string()))
        }
        async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, ProviderError> {
            Ok(self.resources.clone())
        }
        async fn list_resource_templates(
            &self,
        ) -> Result<Vec<ResourceTemplateDescriptor>, ProviderError> {
            Ok(self.templates.clone())
        }
        async fn read_resource(
            &self,
            request: ResourceReadRequest,
        ) -> Result<ResourceReadResult, ProviderError> {
            Ok(ResourceReadResult {
                json: format!(r#"{{"uri":"{}"}}"#, request.uri),
            })
        }
        async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, ProviderError> {
            Ok(self.prompts.clone())
        }
        async fn get_prompt(
            &self,
            request: PromptGetRequest,
        ) -> Result<PromptGetResult, ProviderError> {
            Ok(PromptGetResult {
                json: format!(r#"{{"name":"{}"}}"#, request.name),
            })
        }
    }

    pub(crate) struct FailingUpstream;

    impl ProviderClient for FailingUpstream {
        async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
            Err(ProviderError::Service("connection closed".to_string()))
        }

        async fn call_operation(
            &self,
            _request: OperationCallRequest,
        ) -> Result<OperationCallResult, ProviderError> {
            Err(ProviderError::Service("connection closed".to_string()))
        }

        async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, ProviderError> {
            Err(ProviderError::Service("connection closed".to_string()))
        }
        async fn list_resource_templates(
            &self,
        ) -> Result<Vec<ResourceTemplateDescriptor>, ProviderError> {
            Err(ProviderError::Service("connection closed".to_string()))
        }
        async fn read_resource(
            &self,
            _request: ResourceReadRequest,
        ) -> Result<ResourceReadResult, ProviderError> {
            Err(ProviderError::Service("connection closed".to_string()))
        }
        async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, ProviderError> {
            Err(ProviderError::Service("connection closed".to_string()))
        }
        async fn get_prompt(
            &self,
            _request: PromptGetRequest,
        ) -> Result<PromptGetResult, ProviderError> {
            Err(ProviderError::Service("connection closed".to_string()))
        }
    }

    pub(crate) enum TestUpstream {
        Fast(DualMockServer),
        Failing(FailingUpstream),
    }

    impl ProviderClient for TestUpstream {
        async fn list_operations(&self) -> Result<Vec<OperationDescriptor>, ProviderError> {
            match self {
                TestUpstream::Fast(s) => s.list_operations().await,
                TestUpstream::Failing(s) => s.list_operations().await,
            }
        }

        async fn call_operation(
            &self,
            request: OperationCallRequest,
        ) -> Result<OperationCallResult, ProviderError> {
            match self {
                TestUpstream::Fast(s) => s.call_operation(request).await,
                TestUpstream::Failing(s) => s.call_operation(request).await,
            }
        }

        async fn list_resources(&self) -> Result<Vec<ResourceDescriptor>, ProviderError> {
            match self {
                TestUpstream::Fast(s) => s.list_resources().await,
                TestUpstream::Failing(s) => s.list_resources().await,
            }
        }
        async fn list_resource_templates(
            &self,
        ) -> Result<Vec<ResourceTemplateDescriptor>, ProviderError> {
            match self {
                TestUpstream::Fast(s) => s.list_resource_templates().await,
                TestUpstream::Failing(s) => s.list_resource_templates().await,
            }
        }
        async fn read_resource(
            &self,
            request: ResourceReadRequest,
        ) -> Result<ResourceReadResult, ProviderError> {
            match self {
                TestUpstream::Fast(s) => s.read_resource(request).await,
                TestUpstream::Failing(s) => s.read_resource(request).await,
            }
        }
        async fn list_prompts(&self) -> Result<Vec<PromptDescriptor>, ProviderError> {
            match self {
                TestUpstream::Fast(s) => s.list_prompts().await,
                TestUpstream::Failing(s) => s.list_prompts().await,
            }
        }
        async fn get_prompt(
            &self,
            request: PromptGetRequest,
        ) -> Result<PromptGetResult, ProviderError> {
            match self {
                TestUpstream::Fast(s) => s.get_prompt(request).await,
                TestUpstream::Failing(s) => s.get_prompt(request).await,
            }
        }
    }
}
