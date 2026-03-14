use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::cli_operation_runner::NullCliRunner;
use mcp_gateway::hexagon::ports::driving::read_resource::ResourceReadRequest;
use mcp_gateway::hexagon::usecases::gateway::{DefaultPolicy, Gateway, ProviderHandle};

use crate::common::gateway_helpers::*;

#[tokio::test]
async fn should_route_read_resource_to_correct_provider() {
    let providers = BTreeMap::from([(
        "server".to_string(),
        ProviderHandle {
            client: TestProvider::empty(),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = ResourceReadRequest {
        uri: "server__file:///test.txt".to_string(),
    };
    let result = gateway.read_resource(request).await.unwrap();
    assert!(result.json.contains("file:///test.txt"));
}

#[tokio::test]
async fn should_return_error_for_unprefixed_resource_uri() {
    let providers: BTreeMap<String, ProviderHandle<TestProvider, DefaultPolicy>> = BTreeMap::new();
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = ResourceReadRequest {
        uri: "file:///test.txt".to_string(),
    };
    let err = gateway.read_resource(request).await.unwrap_err();
    assert!(err.to_string().contains("no provider prefix"));
}

#[tokio::test]
async fn should_return_error_for_unknown_resource_provider() {
    let providers: BTreeMap<String, ProviderHandle<TestProvider, DefaultPolicy>> = BTreeMap::new();
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = ResourceReadRequest {
        uri: "unknown__file:///test.txt".to_string(),
    };
    let err = gateway.read_resource(request).await.unwrap_err();
    assert!(err.to_string().contains("unknown provider"));
}

#[tokio::test]
async fn should_return_error_when_resource_provider_fails() {
    let providers = BTreeMap::from([(
        "bad".to_string(),
        ProviderHandle {
            client: TestUpstream::Failing(FailingUpstream),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = ResourceReadRequest {
        uri: "bad__file:///test.txt".to_string(),
    };
    let err = gateway.read_resource(request).await.unwrap_err();
    assert!(err.to_string().contains("connection closed"));
}

#[tokio::test]
async fn should_forward_read_resource_through_gateway() {
    let gateway = two_server_gateway();
    let request = ResourceReadRequest {
        uri: "alpha__file:///test.txt".to_string(),
    };
    let result = gateway.read_resource(request).await.unwrap();
    assert!(result.json.contains("file:///test.txt"));
}

#[tokio::test]
async fn should_forward_read_resource_to_beta_through_gateway() {
    let gateway = two_server_gateway();
    let request = ResourceReadRequest {
        uri: "beta__file:///beta.txt".to_string(),
    };
    let result = gateway.read_resource(request).await.unwrap();
    assert!(result.json.contains("file:///beta.txt"));
}

#[tokio::test]
async fn should_read_resource_from_fast_upstream() {
    let providers = BTreeMap::from([(
        "good".to_string(),
        ProviderHandle {
            client: TestUpstream::Fast(DualMockServer {
                server_name: "alpha",
            }),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = ResourceReadRequest {
        uri: "good__file:///test.txt".to_string(),
    };
    let result = gateway.read_resource(request).await.unwrap();
    assert!(result.json.contains("file:///test.txt"));
}
