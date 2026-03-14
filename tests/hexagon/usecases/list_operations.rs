use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::cli_operation_runner::NullCliRunner;
use mcp_gateway::hexagon::usecases::gateway::{
    create_policy, DefaultPolicy, Gateway, ProviderHandle,
};

use crate::common::gateway_helpers::*;

#[tokio::test]
async fn list_tools_returns_prefixed_tools_from_all_upstreams() {
    let gateway = two_server_gateway();
    let result = gateway.list_operations().await.unwrap();
    let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alpha__echo"));
    assert!(names.contains(&"beta__read_file"));
}

#[tokio::test]
async fn list_tools_with_no_upstreams_returns_empty() {
    let gateway = empty_gateway();
    let result = gateway.list_operations().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_tools_applies_per_server_allowlist_filter() {
    let providers = BTreeMap::from([
        (
            "alpha".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: create_policy(vec!["nonexistent".to_string()], vec![]),
            },
        ),
        (
            "beta".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "beta",
                },
                filter: passthrough_filter(),
            },
        ),
    ]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_operations().await.unwrap();
    let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["beta__read_file"]);
}

#[tokio::test]
async fn list_tools_applies_denylist_filter() {
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
    let result = gateway.list_operations().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_tools_includes_cli_tools_unprefixed() {
    let gateway = two_server_gateway_with_cli();
    let result = gateway.list_operations().await.unwrap();
    let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"alpha__echo"));
    assert!(names.contains(&"beta__read_file"));
    assert!(names.contains(&"cli-cat"));
}

#[tokio::test]
async fn cli_tools_only_no_upstreams() {
    let providers: BTreeMap<String, ProviderHandle<DualMockServer, DefaultPolicy>> =
        BTreeMap::new();
    let gateway = Gateway::new(providers, MockCliRunner);
    let result = gateway.list_operations().await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result.first().map(|t| t.name.as_str()), Some("cli-cat"));
}

#[tokio::test]
async fn should_return_empty_from_test_provider_with_no_operations() {
    let providers = BTreeMap::from([(
        "server".to_string(),
        ProviderHandle {
            client: TestProvider::empty(),
            filter: passthrough_filter(),
        },
    )]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let result = gateway.list_operations().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_tools_skips_erroring_upstream_gracefully() {
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
    let result = gateway.list_operations().await.unwrap();
    let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["good__echo"]);
}
