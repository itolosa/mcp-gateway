use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationCallRequest, OperationCallResult, OperationPolicy,
    ProviderClient, ProviderError,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct RouteOperation;

fn validate_mapping(operation_name: &str) -> Result<(&str, &str), GatewayError> {
    decode(operation_name).ok_or_else(|| GatewayError::InvalidMapping {
        operation: operation_name.to_string(),
    })
}

fn unknown_provider_error(provider_name: &str, operation_name: &str) -> GatewayError {
    GatewayError::UnknownProvider {
        provider: provider_name.to_string(),
        operation: operation_name.to_string(),
    }
}

fn provider_error(e: ProviderError) -> GatewayError {
    GatewayError::Provider(e.to_string())
}

impl RouteOperation {
    pub(crate) async fn execute<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        cli_runner: &C,
        request: OperationCallRequest,
    ) -> Result<OperationCallResult, GatewayError> {
        if cli_runner.has_operation(&request.name) {
            return cli_runner.call_operation(&request).await;
        }
        let (provider_name, raw_operation) = validate_mapping(&request.name)?;
        let entry = match providers.get(provider_name) {
            Some(e) => e,
            None => return Err(unknown_provider_error(provider_name, &request.name)),
        };
        if !entry.filter.is_allowed(raw_operation) {
            return Err(GatewayError::OperationNotAllowed {
                operation: request.name.clone(),
            });
        }
        let provider_request = OperationCallRequest {
            name: raw_operation.to_string(),
            arguments: request.arguments,
        };
        entry
            .client
            .call_operation(provider_request)
            .await
            .map_err(provider_error)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeMap;

    use crate::adapters::driven::connectivity::cli_execution::NullCliRunner;
    use crate::hexagon::entities::policy::allowlist::AllowlistPolicy;
    use crate::hexagon::entities::policy::compound::CompoundPolicy;
    use crate::hexagon::entities::policy::denylist::DenylistPolicy;
    use crate::hexagon::ports::OperationCallRequest;
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::ProviderHandle;

    use super::RouteOperation;

    #[tokio::test]
    async fn call_tool_routes_to_correct_upstream() {
        let upstreams = two_server_setup();

        let request = OperationCallRequest {
            name: "alpha__echo".to_string(),
            arguments: Some(r#"{"message":"hello"}"#.to_string()),
        };
        let result = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("hello"));

        let request = OperationCallRequest {
            name: "beta__read_file".to_string(),
            arguments: Some(r#"{"path":"/etc/hosts"}"#.to_string()),
        };
        let result = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("/etc/hosts"));
    }

    #[tokio::test]
    async fn call_tool_without_prefix_returns_error() {
        let upstreams = two_server_setup();
        let request = OperationCallRequest {
            name: "echo".to_string(),
            arguments: None,
        };
        let err = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no provider prefix"));
    }

    #[tokio::test]
    async fn call_tool_unknown_server_returns_error() {
        let upstreams = two_server_setup();
        let request = OperationCallRequest {
            name: "unknown__echo".to_string(),
            arguments: None,
        };
        let err = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown provider"));
    }

    #[tokio::test]
    async fn call_tool_blocked_by_filter_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: CompoundPolicy::new(
                    AllowlistPolicy::new(vec![]),
                    DenylistPolicy::new(vec!["echo".to_string()]),
                ),
            },
        );
        let request = OperationCallRequest {
            name: "alpha__echo".to_string(),
            arguments: None,
        };
        let err = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not allowed"));
    }

    #[tokio::test]
    async fn call_unknown_tool_on_upstream_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: passthrough_filter(),
            },
        );
        let request = OperationCallRequest {
            name: "alpha__nonexistent".to_string(),
            arguments: None,
        };
        let err = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn call_unknown_tool_on_beta_upstream_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "beta".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "beta",
                },
                filter: passthrough_filter(),
            },
        );
        let request = OperationCallRequest {
            name: "beta__nonexistent".to_string(),
            arguments: None,
        };
        let err = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn call_cli_tool_routes_to_runner() {
        let upstreams = two_server_setup();
        let request = OperationCallRequest {
            name: "cli-cat".to_string(),
            arguments: None,
        };
        let result = RouteOperation::execute(&upstreams, &MockCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("cli-cat"));
    }

    #[tokio::test]
    async fn call_upstream_tool_when_cli_present() {
        let upstreams = two_server_setup();
        let request = OperationCallRequest {
            name: "alpha__echo".to_string(),
            arguments: Some(r#"{"message":"upstream"}"#.to_string()),
        };
        let result = RouteOperation::execute(&upstreams, &MockCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("upstream"));
    }

    #[tokio::test]
    async fn call_tool_on_failing_upstream_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "bad".to_string(),
            ProviderHandle {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let request = OperationCallRequest {
            name: "bad__anything".to_string(),
            arguments: None,
        };
        let err = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("connection closed"));
    }

    #[tokio::test]
    async fn call_tool_routes_to_good_server_in_mixed_upstream() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "good".to_string(),
            ProviderHandle {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        );
        upstreams.insert(
            "bad".to_string(),
            ProviderHandle {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let request = OperationCallRequest {
            name: "good__echo".to_string(),
            arguments: Some(r#"{"message":"hello"}"#.to_string()),
        };
        let result = RouteOperation::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("hello"));
    }

    #[tokio::test]
    async fn call_unknown_cli_tool_returns_error() {
        let upstreams: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            BTreeMap::new();
        let request = OperationCallRequest {
            name: "nonexistent-cli".to_string(),
            arguments: None,
        };
        let err = RouteOperation::execute(&upstreams, &MockCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no provider prefix"));
    }
}
