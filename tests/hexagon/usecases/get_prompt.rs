use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::cli_operation_runner::NullCliRunner;
use mcp_gateway::hexagon::ports::driving::get_prompt::PromptGetRequest;
use mcp_gateway::hexagon::usecases::gateway::{DefaultPolicy, Gateway, ProviderHandle};

use crate::common::gateway_helpers::*;

#[tokio::test]
async fn should_route_get_prompt_to_correct_provider() {
    let providers = BTreeMap::from([(
        "server".to_string(),
        ProviderHandle {
            client: TestProvider::empty(),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = PromptGetRequest {
        name: "server__summarize".to_string(),
        arguments: None,
    };
    let result = gateway.get_prompt(request).await.unwrap();
    assert!(result.json.contains("summarize"));
}

#[tokio::test]
async fn should_forward_prompt_arguments_to_provider() {
    let providers = BTreeMap::from([(
        "server".to_string(),
        ProviderHandle {
            client: TestProvider::empty(),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = PromptGetRequest {
        name: "server__translate".to_string(),
        arguments: Some(r#"{"lang":"es"}"#.to_string()),
    };
    let result = gateway.get_prompt(request).await.unwrap();
    assert!(result.json.contains("translate"));
}

#[tokio::test]
async fn should_return_error_for_unprefixed_prompt_name() {
    let providers: BTreeMap<String, ProviderHandle<TestProvider, DefaultPolicy>> = BTreeMap::new();
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = PromptGetRequest {
        name: "summarize".to_string(),
        arguments: None,
    };
    let err = gateway.get_prompt(request).await.unwrap_err();
    assert!(err.to_string().contains("no provider prefix"));
}

#[tokio::test]
async fn should_return_error_for_unknown_prompt_provider() {
    let providers: BTreeMap<String, ProviderHandle<TestProvider, DefaultPolicy>> = BTreeMap::new();
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = PromptGetRequest {
        name: "unknown__summarize".to_string(),
        arguments: None,
    };
    let err = gateway.get_prompt(request).await.unwrap_err();
    assert!(err.to_string().contains("unknown provider"));
}

#[tokio::test]
async fn should_return_error_when_prompt_provider_fails() {
    let providers = BTreeMap::from([(
        "bad".to_string(),
        ProviderHandle {
            client: TestUpstream::Failing(FailingUpstream),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = PromptGetRequest {
        name: "bad__summarize".to_string(),
        arguments: None,
    };
    let err = gateway.get_prompt(request).await.unwrap_err();
    assert!(err.to_string().contains("connection closed"));
}

#[tokio::test]
async fn should_forward_get_prompt_through_gateway() {
    let gateway = two_server_gateway();
    let request = PromptGetRequest {
        name: "alpha__greet".to_string(),
        arguments: None,
    };
    let result = gateway.get_prompt(request).await.unwrap();
    assert!(result.json.contains("greet"));
}

#[tokio::test]
async fn should_forward_get_prompt_to_beta_through_gateway() {
    let gateway = two_server_gateway();
    let request = PromptGetRequest {
        name: "beta__greet".to_string(),
        arguments: None,
    };
    let result = gateway.get_prompt(request).await.unwrap();
    assert!(result.json.contains("greet"));
}

#[tokio::test]
async fn should_get_prompt_from_fast_upstream() {
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
    let request = PromptGetRequest {
        name: "good__greet".to_string(),
        arguments: None,
    };
    let result = gateway.get_prompt(request).await.unwrap();
    assert!(result.json.contains("greet"));
}
