use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::connectivity::cli_execution::NullCliRunner;
use mcp_gateway::hexagon::ports::{
    OperationCallRequest, PromptDescriptor, PromptGetRequest, ResourceDescriptor,
    ResourceReadRequest, ResourceTemplateDescriptor,
};
use mcp_gateway::hexagon::usecases::gateway::{
    create_policy, DefaultPolicy, Gateway, ProviderHandle,
};

use crate::common::gateway_helpers::*;

// ---------------------------------------------------------------------------
// route_operation tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// list_operations tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// list_resources tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// read_resource tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// list_prompts tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// get_prompt tests
// ---------------------------------------------------------------------------

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
