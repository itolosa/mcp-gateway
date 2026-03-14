use std::collections::BTreeMap;

use crate::hexagon::ports::{
    GatewayError, OperationPolicy, PromptGetRequest, PromptGetResult, ProviderClient, ProviderError,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct GetPrompt;

impl GetPrompt {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        request: PromptGetRequest,
    ) -> Result<PromptGetResult, GatewayError> {
        let (provider_name, raw_name) =
            decode(&request.name).ok_or_else(|| GatewayError::InvalidMapping {
                operation: request.name.clone(),
            })?;
        let entry = providers
            .get(provider_name)
            .ok_or_else(|| GatewayError::UnknownProvider {
                provider: provider_name.to_string(),
                operation: request.name.clone(),
            })?;
        let provider_request = PromptGetRequest {
            name: raw_name.to_string(),
            arguments: request.arguments,
        };
        entry
            .client
            .get_prompt(provider_request)
            .await
            .map_err(|e: ProviderError| GatewayError::Provider(e.to_string()))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use std::collections::BTreeMap;

    use crate::hexagon::ports::PromptGetRequest;
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::ProviderHandle;

    use super::GetPrompt;

    #[tokio::test]
    async fn should_route_get_prompt_to_correct_provider() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "server".to_string(),
            ProviderHandle {
                client: TestProvider::empty(),
                filter: passthrough_filter(),
            },
        );

        let request = PromptGetRequest {
            name: "server__summarize".to_string(),
            arguments: None,
        };
        let result = GetPrompt::execute(&providers, request).await.unwrap();
        assert!(result.json.contains("summarize"));
    }

    #[tokio::test]
    async fn should_forward_arguments_to_provider() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "server".to_string(),
            ProviderHandle {
                client: TestProvider::empty(),
                filter: passthrough_filter(),
            },
        );

        let request = PromptGetRequest {
            name: "server__translate".to_string(),
            arguments: Some(r#"{"lang":"es"}"#.to_string()),
        };
        let result = GetPrompt::execute(&providers, request).await.unwrap();
        assert!(result.json.contains("translate"));
    }

    #[tokio::test]
    async fn should_return_error_for_unprefixed_name() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let request = PromptGetRequest {
            name: "summarize".to_string(),
            arguments: None,
        };
        let err = GetPrompt::execute(&providers, request).await.unwrap_err();
        assert!(err.to_string().contains("no provider prefix"));
    }

    #[tokio::test]
    async fn should_return_error_for_unknown_provider() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let request = PromptGetRequest {
            name: "unknown__summarize".to_string(),
            arguments: None,
        };
        let err = GetPrompt::execute(&providers, request).await.unwrap_err();
        assert!(err.to_string().contains("unknown provider"));
    }

    #[tokio::test]
    async fn should_return_error_when_provider_fails() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "bad".to_string(),
            ProviderHandle {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let request = PromptGetRequest {
            name: "bad__summarize".to_string(),
            arguments: None,
        };
        let err = GetPrompt::execute(&providers, request).await.unwrap_err();
        assert!(err.to_string().contains("connection closed"));
    }

    #[tokio::test]
    async fn should_forward_get_prompt_through_gateway() {
        let providers: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            two_server_setup();
        let gateway = crate::hexagon::usecases::gateway::Gateway::new(
            providers,
            crate::adapters::driven::connectivity::cli_execution::NullCliRunner,
        );
        let request = PromptGetRequest {
            name: "alpha__greet".to_string(),
            arguments: None,
        };
        let result = gateway.get_prompt(request).await.unwrap();
        assert!(result.json.contains("greet"));
    }

    #[tokio::test]
    async fn should_forward_get_prompt_to_beta_through_gateway() {
        let providers: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            two_server_setup();
        let gateway = crate::hexagon::usecases::gateway::Gateway::new(
            providers,
            crate::adapters::driven::connectivity::cli_execution::NullCliRunner,
        );
        let request = PromptGetRequest {
            name: "beta__greet".to_string(),
            arguments: None,
        };
        let result = gateway.get_prompt(request).await.unwrap();
        assert!(result.json.contains("greet"));
    }

    #[tokio::test]
    async fn should_get_prompt_from_fast_upstream() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "good".to_string(),
            ProviderHandle {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        );
        let request = PromptGetRequest {
            name: "good__greet".to_string(),
            arguments: None,
        };
        let result = GetPrompt::execute(&providers, request).await.unwrap();
        assert!(result.json.contains("greet"));
    }
}
