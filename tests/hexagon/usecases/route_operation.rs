use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::cli_operation_runner::NullCliRunner;
use mcp_gateway::hexagon::ports::driving::route_operation::OperationCallRequest;
use mcp_gateway::hexagon::usecases::gateway::{
    create_policy, DefaultPolicy, Gateway, ProviderHandle,
};

use crate::common::gateway_helpers::*;

#[tokio::test]
async fn call_tool_routes_to_correct_upstream() {
    let gateway = two_server_gateway();

    let request = OperationCallRequest {
        name: "alpha__echo".to_string(),
        arguments: Some(r#"{"message":"hello"}"#.to_string()),
    };
    let result = gateway.route_operation(request).await.unwrap();
    assert!(result.content[0].contains("hello"));

    let gateway = two_server_gateway();
    let request = OperationCallRequest {
        name: "beta__read_file".to_string(),
        arguments: Some(r#"{"path":"/etc/hosts"}"#.to_string()),
    };
    let result = gateway.route_operation(request).await.unwrap();
    assert!(result.content[0].contains("/etc/hosts"));
}

#[tokio::test]
async fn call_tool_without_prefix_returns_error() {
    let gateway = two_server_gateway();
    let request = OperationCallRequest {
        name: "echo".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("no provider prefix"));
}

#[tokio::test]
async fn call_tool_unknown_server_returns_error() {
    let gateway = two_server_gateway();
    let request = OperationCallRequest {
        name: "unknown__echo".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("unknown provider"));
}

#[tokio::test]
async fn call_tool_blocked_by_filter_returns_error() {
    let providers = BTreeMap::from([(
        "alpha".to_string(),
        ProviderHandle {
            client: DualMockServer {
                server_name: "alpha",
            },
            filter: create_policy(vec![], vec!["echo".to_string()]),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = OperationCallRequest {
        name: "alpha__echo".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("not allowed"));
}

#[tokio::test]
async fn call_unknown_tool_on_upstream_returns_error() {
    let providers = BTreeMap::from([(
        "alpha".to_string(),
        ProviderHandle {
            client: DualMockServer {
                server_name: "alpha",
            },
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = OperationCallRequest {
        name: "alpha__nonexistent".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("unknown tool"));
}

#[tokio::test]
async fn call_unknown_tool_on_beta_upstream_returns_error() {
    let providers = BTreeMap::from([(
        "beta".to_string(),
        ProviderHandle {
            client: DualMockServer {
                server_name: "beta",
            },
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = OperationCallRequest {
        name: "beta__nonexistent".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("unknown tool"));
}

#[tokio::test]
async fn call_cli_tool_routes_to_runner() {
    let gateway = two_server_gateway_with_cli();
    let request = OperationCallRequest {
        name: "cli-cat".to_string(),
        arguments: None,
    };
    let result = gateway.route_operation(request).await.unwrap();
    assert!(result.content[0].contains("cli-cat"));
}

#[tokio::test]
async fn call_upstream_tool_when_cli_present() {
    let gateway = two_server_gateway_with_cli();
    let request = OperationCallRequest {
        name: "alpha__echo".to_string(),
        arguments: Some(r#"{"message":"upstream"}"#.to_string()),
    };
    let result = gateway.route_operation(request).await.unwrap();
    assert!(result.content[0].contains("upstream"));
}

#[tokio::test]
async fn call_tool_on_failing_upstream_returns_error() {
    let providers = BTreeMap::from([(
        "bad".to_string(),
        ProviderHandle {
            client: TestUpstream::Failing(FailingUpstream),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = OperationCallRequest {
        name: "bad__anything".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("connection closed"));
}

#[tokio::test]
async fn call_tool_routes_to_good_server_in_mixed_upstream() {
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
    let request = OperationCallRequest {
        name: "good__echo".to_string(),
        arguments: Some(r#"{"message":"hello"}"#.to_string()),
    };
    let result = gateway.route_operation(request).await.unwrap();
    assert!(result.content[0].contains("hello"));
}

#[tokio::test]
async fn call_unknown_cli_tool_returns_error() {
    let providers: BTreeMap<String, ProviderHandle<DualMockServer, DefaultPolicy>> =
        BTreeMap::new();
    let gateway = Gateway::new(providers, MockCliRunner);
    let request = OperationCallRequest {
        name: "nonexistent-cli".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("no provider prefix"));
}

#[tokio::test]
async fn call_operation_on_test_provider_returns_error() {
    let providers = BTreeMap::from([(
        "server".to_string(),
        ProviderHandle {
            client: TestProvider::empty(),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let request = OperationCallRequest {
        name: "server__anything".to_string(),
        arguments: None,
    };
    let err = gateway.route_operation(request).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}
