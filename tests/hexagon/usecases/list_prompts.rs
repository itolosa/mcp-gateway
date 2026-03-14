use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::cli_operation_runner::NullCliRunner;
use mcp_gateway::hexagon::ports::driven::provider_client::PromptDescriptor;
use mcp_gateway::hexagon::usecases::gateway::{DefaultPolicy, Gateway, ProviderHandle};

use crate::common::gateway_helpers::*;

#[tokio::test]
async fn should_return_prefixed_prompts_from_all_providers() {
    let providers = BTreeMap::from([
        (
            "server-a".to_string(),
            ProviderHandle {
                client: TestProvider {
                    prompts: vec![PromptDescriptor {
                        name: "summarize".to_string(),
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
                    prompts: vec![PromptDescriptor {
                        name: "translate".to_string(),
                        json: "{}".to_string(),
                    }],
                    ..TestProvider::empty()
                },
                filter: passthrough_filter(),
            },
        ),
    ]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_prompts().await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].name, "server-a__summarize");
    assert_eq!(result[1].name, "server-b__translate");
}

#[tokio::test]
async fn should_return_empty_prompts_when_no_providers() {
    let providers: BTreeMap<String, ProviderHandle<TestProvider, DefaultPolicy>> = BTreeMap::new();
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_prompts().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_skip_failing_providers_for_prompts() {
    let providers = BTreeMap::from([(
        "bad".to_string(),
        ProviderHandle {
            client: TestUpstream::Failing(FailingUpstream),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_prompts().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_list_prompts_from_fast_upstream() {
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
    let result = gateway.list_prompts().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn should_forward_list_prompts_through_gateway() {
    let gateway = two_server_gateway();
    let result = gateway.list_prompts().await.unwrap();
    assert!(result.is_empty());
}
