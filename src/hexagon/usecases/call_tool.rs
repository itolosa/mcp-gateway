use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliToolRunner, GatewayError, ToolCallRequest, ToolCallResult, ToolFilter, UpstreamClient,
    UpstreamError,
};
use crate::hexagon::usecases::prefix::split_prefixed_name;

use super::gateway::UpstreamEntry;

pub(crate) struct CallTool;

fn validate_tool_prefix(tool_name: &str) -> Result<(&str, &str), GatewayError> {
    split_prefixed_name(tool_name).ok_or_else(|| GatewayError::NoPrefix {
        tool: tool_name.to_string(),
    })
}

fn unknown_server_error(server_name: &str, tool_name: &str) -> GatewayError {
    GatewayError::UnknownServer {
        server: server_name.to_string(),
        tool: tool_name.to_string(),
    }
}

fn upstream_error(e: UpstreamError) -> GatewayError {
    GatewayError::Upstream(e.to_string())
}

impl CallTool {
    pub(crate) async fn execute<U: UpstreamClient, C: CliToolRunner, F: ToolFilter>(
        upstreams: &BTreeMap<String, UpstreamEntry<U, F>>,
        cli_runner: &C,
        request: ToolCallRequest,
    ) -> Result<ToolCallResult, GatewayError> {
        if cli_runner.has_tool(&request.name) {
            return cli_runner.call_tool(&request).await;
        }
        let (server_name, raw_tool) = validate_tool_prefix(&request.name)?;
        let entry = match upstreams.get(server_name) {
            Some(e) => e,
            None => return Err(unknown_server_error(server_name, &request.name)),
        };
        if !entry.filter.is_tool_allowed(raw_tool) {
            return Err(GatewayError::ToolNotAllowed {
                tool: request.name.clone(),
            });
        }
        let upstream_request = ToolCallRequest {
            name: raw_tool.to_string(),
            arguments: request.arguments,
        };
        entry
            .client
            .call_tool(upstream_request)
            .await
            .map_err(upstream_error)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeMap;

    use crate::adapters::driven::connectivity::cli_execution::NullCliRunner;
    use crate::hexagon::entities::policy::allowlist::AllowlistFilter;
    use crate::hexagon::entities::policy::compound::CompoundFilter;
    use crate::hexagon::entities::policy::denylist::DenylistFilter;
    use crate::hexagon::ports::ToolCallRequest;
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::UpstreamEntry;

    use super::CallTool;

    #[tokio::test]
    async fn call_tool_routes_to_correct_upstream() {
        let upstreams = two_server_setup();

        let request = ToolCallRequest {
            name: "alpha__echo".to_string(),
            arguments: Some(r#"{"message":"hello"}"#.to_string()),
        };
        let result = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("hello"));

        let request = ToolCallRequest {
            name: "beta__read_file".to_string(),
            arguments: Some(r#"{"path":"/etc/hosts"}"#.to_string()),
        };
        let result = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("/etc/hosts"));
    }

    #[tokio::test]
    async fn call_tool_without_prefix_returns_error() {
        let upstreams = two_server_setup();
        let request = ToolCallRequest {
            name: "echo".to_string(),
            arguments: None,
        };
        let err = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no server prefix"));
    }

    #[tokio::test]
    async fn call_tool_unknown_server_returns_error() {
        let upstreams = two_server_setup();
        let request = ToolCallRequest {
            name: "unknown__echo".to_string(),
            arguments: None,
        };
        let err = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown server"));
    }

    #[tokio::test]
    async fn call_tool_blocked_by_filter_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: CompoundFilter::new(
                    AllowlistFilter::new(vec![]),
                    DenylistFilter::new(vec!["echo".to_string()]),
                ),
            },
        );
        let request = ToolCallRequest {
            name: "alpha__echo".to_string(),
            arguments: None,
        };
        let err = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not allowed"));
    }

    #[tokio::test]
    async fn call_unknown_tool_on_upstream_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: passthrough_filter(),
            },
        );
        let request = ToolCallRequest {
            name: "alpha__nonexistent".to_string(),
            arguments: None,
        };
        let err = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn call_unknown_tool_on_beta_upstream_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "beta".to_string(),
            UpstreamEntry {
                client: DualMockServer {
                    server_name: "beta",
                },
                filter: passthrough_filter(),
            },
        );
        let request = ToolCallRequest {
            name: "beta__nonexistent".to_string(),
            arguments: None,
        };
        let err = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn call_cli_tool_routes_to_runner() {
        let upstreams = two_server_setup();
        let request = ToolCallRequest {
            name: "cli-cat".to_string(),
            arguments: None,
        };
        let result = CallTool::execute(&upstreams, &MockCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("cli-cat"));
    }

    #[tokio::test]
    async fn call_upstream_tool_when_cli_present() {
        let upstreams = two_server_setup();
        let request = ToolCallRequest {
            name: "alpha__echo".to_string(),
            arguments: Some(r#"{"message":"upstream"}"#.to_string()),
        };
        let result = CallTool::execute(&upstreams, &MockCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("upstream"));
    }

    #[tokio::test]
    async fn call_tool_on_failing_upstream_returns_error() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "bad".to_string(),
            UpstreamEntry {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let request = ToolCallRequest {
            name: "bad__anything".to_string(),
            arguments: None,
        };
        let err = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("connection closed"));
    }

    #[tokio::test]
    async fn call_tool_routes_to_good_server_in_mixed_upstream() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "good".to_string(),
            UpstreamEntry {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        );
        upstreams.insert(
            "bad".to_string(),
            UpstreamEntry {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let request = ToolCallRequest {
            name: "good__echo".to_string(),
            arguments: Some(r#"{"message":"hello"}"#.to_string()),
        };
        let result = CallTool::execute(&upstreams, &NullCliRunner, request)
            .await
            .unwrap();
        assert!(result.content[0].contains("hello"));
    }

    #[tokio::test]
    async fn call_unknown_cli_tool_returns_error() {
        let upstreams: BTreeMap<String, UpstreamEntry<DualMockServer, TestFilter>> =
            BTreeMap::new();
        let request = ToolCallRequest {
            name: "nonexistent-cli".to_string(),
            arguments: None,
        };
        let err = CallTool::execute(&upstreams, &MockCliRunner, request)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no server prefix"));
    }
}
