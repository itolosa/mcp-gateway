use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::cli_operation_runner::NullCliRunner;
use mcp_gateway::hexagon::ports::driven::provider_client::{
    ResourceDescriptor, ResourceTemplateDescriptor,
};
use mcp_gateway::hexagon::usecases::gateway::{DefaultPolicy, Gateway, ProviderHandle};

use crate::common::gateway_helpers::*;

#[tokio::test]
async fn should_return_prefixed_resources_from_all_providers() {
    let providers = BTreeMap::from([
        (
            "server-a".to_string(),
            ProviderHandle {
                client: TestProvider {
                    resources: vec![ResourceDescriptor {
                        uri: "file:///a.txt".to_string(),
                        name: "a.txt".to_string(),
                        json: "{}".to_string(),
                    }],
                    ..TestProvider::empty()
                },
                filter: passthrough_filter(),
            },
        ),
        (
            "server-b".to_string(),
            ProviderHandle {
                client: TestProvider {
                    resources: vec![ResourceDescriptor {
                        uri: "file:///b.txt".to_string(),
                        name: "b.txt".to_string(),
                        json: "{}".to_string(),
                    }],
                    ..TestProvider::empty()
                },
                filter: passthrough_filter(),
            },
        ),
    ]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_resources().await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].uri, "server-a__file:///a.txt");
    assert_eq!(result[0].name, "server-a__a.txt");
    assert_eq!(result[1].uri, "server-b__file:///b.txt");
    assert_eq!(result[1].name, "server-b__b.txt");
}

#[tokio::test]
async fn should_return_empty_resources_when_no_providers() {
    let providers: BTreeMap<String, ProviderHandle<TestProvider, DefaultPolicy>> = BTreeMap::new();
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_resources().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_skip_failing_providers_for_resources() {
    let providers = BTreeMap::from([
        (
            "good".to_string(),
            ProviderHandle {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        ),
        (
            "bad".to_string(),
            ProviderHandle {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        ),
    ]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_resources().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_return_prefixed_resource_templates() {
    let providers = BTreeMap::from([(
        "server".to_string(),
        ProviderHandle {
            client: TestProvider {
                templates: vec![ResourceTemplateDescriptor {
                    uri_template: "file:///{path}".to_string(),
                    name: "file-template".to_string(),
                    json: "{}".to_string(),
                }],
                ..TestProvider::empty()
            },
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_resource_templates().await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].uri_template, "server__file:///{path}");
    assert_eq!(result[0].name, "server__file-template");
}

#[tokio::test]
async fn should_return_empty_templates_when_no_providers() {
    let providers: BTreeMap<String, ProviderHandle<TestProvider, DefaultPolicy>> = BTreeMap::new();
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_resource_templates().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_skip_failing_providers_for_templates() {
    let providers = BTreeMap::from([(
        "bad".to_string(),
        ProviderHandle {
            client: TestUpstream::Failing(FailingUpstream),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_resource_templates().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_list_resource_templates_from_fast_upstream() {
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
    let result = gateway.list_resource_templates().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_forward_list_resources_through_gateway() {
    let gateway = two_server_gateway();
    let result = gateway.list_resources().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_forward_list_resource_templates_through_gateway() {
    let gateway = two_server_gateway();
    let result = gateway.list_resource_templates().await.unwrap();
    assert!(result.is_empty());
}
