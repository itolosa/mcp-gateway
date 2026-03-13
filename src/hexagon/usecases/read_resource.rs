use std::collections::BTreeMap;

use crate::hexagon::ports::{
    GatewayError, OperationPolicy, ProviderClient, ProviderError, ResourceReadRequest,
    ResourceReadResult,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct ReadResource;

impl ReadResource {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        request: ResourceReadRequest,
    ) -> Result<ResourceReadResult, GatewayError> {
        let (provider_name, raw_uri) =
            decode(&request.uri).ok_or_else(|| GatewayError::InvalidMapping {
                operation: request.uri.clone(),
            })?;
        let entry = providers
            .get(provider_name)
            .ok_or_else(|| GatewayError::UnknownProvider {
                provider: provider_name.to_string(),
                operation: request.uri.clone(),
            })?;
        let provider_request = ResourceReadRequest {
            uri: raw_uri.to_string(),
        };
        entry
            .client
            .read_resource(provider_request)
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

    use crate::hexagon::ports::ResourceReadRequest;
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::ProviderHandle;

    use super::ReadResource;

    #[tokio::test]
    async fn should_route_read_resource_to_correct_provider() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "server".to_string(),
            ProviderHandle {
                client: TestProvider::empty(),
                filter: passthrough_filter(),
            },
        );

        let request = ResourceReadRequest {
            uri: "server__file:///test.txt".to_string(),
        };
        let result = ReadResource::execute(&providers, request).await.unwrap();
        assert!(result.json.contains("file:///test.txt"));
    }

    #[tokio::test]
    async fn should_return_error_for_unprefixed_uri() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let request = ResourceReadRequest {
            uri: "file:///test.txt".to_string(),
        };
        let err = ReadResource::execute(&providers, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no provider prefix"));
    }

    #[tokio::test]
    async fn should_return_error_for_unknown_provider() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let request = ResourceReadRequest {
            uri: "unknown__file:///test.txt".to_string(),
        };
        let err = ReadResource::execute(&providers, request)
            .await
            .unwrap_err();
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
        let request = ResourceReadRequest {
            uri: "bad__file:///test.txt".to_string(),
        };
        let err = ReadResource::execute(&providers, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("connection closed"));
    }

    #[tokio::test]
    async fn should_forward_read_resource_through_gateway() {
        let providers: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            two_server_setup();
        let gateway = crate::hexagon::usecases::gateway::Gateway::new(
            providers,
            crate::adapters::driven::connectivity::cli_execution::NullCliRunner,
        );
        let request = ResourceReadRequest {
            uri: "alpha__file:///test.txt".to_string(),
        };
        let result = gateway.read_resource(request).await.unwrap();
        assert!(result.json.contains("file:///test.txt"));
    }

    #[tokio::test]
    async fn should_forward_read_resource_to_beta_through_gateway() {
        let providers: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            two_server_setup();
        let gateway = crate::hexagon::usecases::gateway::Gateway::new(
            providers,
            crate::adapters::driven::connectivity::cli_execution::NullCliRunner,
        );
        let request = ResourceReadRequest {
            uri: "beta__file:///beta.txt".to_string(),
        };
        let result = gateway.read_resource(request).await.unwrap();
        assert!(result.json.contains("file:///beta.txt"));
    }

    #[tokio::test]
    async fn should_read_resource_from_fast_upstream() {
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
        let request = ResourceReadRequest {
            uri: "good__file:///test.txt".to_string(),
        };
        let result = ReadResource::execute(&providers, request).await.unwrap();
        assert!(result.json.contains("file:///test.txt"));
    }
}
