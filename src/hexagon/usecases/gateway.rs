use std::collections::BTreeMap;
use std::time::Duration;

use crate::adapters::driven::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};
use crate::hexagon::entities::{GatewayError, ToolCallRequest, ToolCallResult, ToolDescriptor};
use crate::hexagon::ports::ToolFilter;
use crate::hexagon::ports::{CliToolRunner, UpstreamClient};
use crate::hexagon::usecases::prefix::{prefix_tool_name, split_prefixed_name};

pub const DEFAULT_UPSTREAM_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

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

fn upstream_error(e: crate::hexagon::entities::UpstreamError) -> GatewayError {
    GatewayError::Upstream(e.to_string())
}

pub struct UpstreamEntry<U> {
    pub client: U,
    pub filter: CompoundFilter<AllowlistFilter, DenylistFilter>,
}

pub struct Gateway<U, C> {
    upstreams: BTreeMap<String, UpstreamEntry<U>>,
    cli_runner: C,
    operation_timeout: Duration,
}

impl<U: UpstreamClient, C: CliToolRunner> Gateway<U, C> {
    pub fn new(upstreams: BTreeMap<String, UpstreamEntry<U>>, cli_runner: C) -> Self {
        Self {
            upstreams,
            cli_runner,
            operation_timeout: DEFAULT_UPSTREAM_OPERATION_TIMEOUT,
        }
    }

    pub fn with_operation_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = timeout;
        self
    }

    pub async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, GatewayError> {
        let mut all_tools = Vec::new();
        let timeout_secs = self.operation_timeout.as_secs();
        for (name, entry) in &self.upstreams {
            let tools =
                match tokio::time::timeout(self.operation_timeout, entry.client.list_tools()).await
                {
                    Ok(Ok(tools)) => tools,
                    Ok(Err(err)) => {
                        tracing::warn!(
                            upstream = %name,
                            error = %err,
                            "upstream failed during list_tools, skipping"
                        );
                        continue;
                    }
                    Err(_) => {
                        tracing::warn!(
                            upstream = %name,
                            timeout_secs,
                            "upstream timed out during list_tools, skipping"
                        );
                        continue;
                    }
                };
            for mut tool in tools {
                if entry.filter.is_tool_allowed(&tool.name) {
                    tool.name = prefix_tool_name(name, &tool.name);
                    all_tools.push(tool);
                }
            }
        }
        all_tools.extend(self.cli_runner.list_tools());
        Ok(all_tools)
    }

    pub async fn call_tool(
        &self,
        request: ToolCallRequest,
    ) -> Result<ToolCallResult, GatewayError> {
        if self.cli_runner.has_tool(&request.name) {
            return self.cli_runner.call_tool(&request).await;
        }
        let (server_name, raw_tool) = validate_tool_prefix(&request.name)?;
        let entry = match self.upstreams.get(server_name) {
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
        match tokio::time::timeout(
            self.operation_timeout,
            entry.client.call_tool(upstream_request),
        )
        .await
        {
            Ok(result) => result.map_err(upstream_error),
            Err(_) => Err(GatewayError::UpstreamTimeout {
                server: server_name.to_string(),
                timeout_secs: self.operation_timeout.as_secs(),
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hexagon::entities::UpstreamError;

    struct MockServerA;

    impl UpstreamClient for MockServerA {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            Ok(vec![ToolDescriptor {
                name: "echo".to_string(),
                description: Some("echoes input".to_string()),
                schema: serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                }))
                .unwrap(),
            }])
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            if request.name == "echo" {
                let input = request
                    .arguments
                    .and_then(|a| a.get("message").cloned())
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                Ok(ToolCallResult::text_success(input))
            } else {
                Err(UpstreamError::Service(format!(
                    "unknown tool: {}",
                    request.name
                )))
            }
        }
    }

    struct MockServerB;

    impl UpstreamClient for MockServerB {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            Ok(vec![ToolDescriptor {
                name: "read_file".to_string(),
                description: Some("reads a file".to_string()),
                schema: serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    }
                }))
                .unwrap(),
            }])
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            if request.name == "read_file" {
                let path = request
                    .arguments
                    .and_then(|a| a.get("path").cloned())
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                Ok(ToolCallResult::text_success(format!("content of {path}")))
            } else {
                Err(UpstreamError::Service(format!(
                    "unknown tool: {}",
                    request.name
                )))
            }
        }
    }

    // Both mock servers implement the same trait, but Gateway<U, C> requires
    // all upstreams to be the same type U. For tests with mixed servers,
    // we use a single mock that can act as either server.
    struct DualMockServer {
        server_name: &'static str,
    }

    impl UpstreamClient for DualMockServer {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            if self.server_name == "alpha" {
                MockServerA.list_tools().await
            } else {
                MockServerB.list_tools().await
            }
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            if self.server_name == "alpha" {
                MockServerA.call_tool(request).await
            } else {
                MockServerB.call_tool(request).await
            }
        }
    }

    fn passthrough_filter() -> CompoundFilter<AllowlistFilter, DenylistFilter> {
        CompoundFilter::new(AllowlistFilter::new(vec![]), DenylistFilter::new(vec![]))
    }

    fn two_server_setup() -> BTreeMap<String, UpstreamEntry<DualMockServer>> {
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
        upstreams.insert(
            "beta".to_string(),
            UpstreamEntry {
                client: DualMockServer {
                    server_name: "beta",
                },
                filter: passthrough_filter(),
            },
        );
        upstreams
    }

    fn text_from_result(result: &ToolCallResult) -> Option<&str> {
        result
            .content
            .first()
            .and_then(|v| v.get("text"))
            .and_then(|v| v.as_str())
    }

    use crate::adapters::driven::NullCliRunner;

    // --- list_tools ---

    #[tokio::test]
    async fn list_tools_returns_prefixed_tools_from_all_upstreams() {
        let gateway = Gateway::new(two_server_setup(), NullCliRunner);
        let result = gateway.list_tools().await.unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha__echo"));
        assert!(names.contains(&"beta__read_file"));
    }

    #[tokio::test]
    async fn list_tools_with_no_upstreams_returns_empty() {
        let gateway: Gateway<DualMockServer, NullCliRunner> =
            Gateway::new(BTreeMap::new(), NullCliRunner);
        let result = gateway.list_tools().await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_tools_applies_per_server_allowlist_filter() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: CompoundFilter::new(
                    AllowlistFilter::new(vec!["nonexistent".to_string()]),
                    DenylistFilter::new(vec![]),
                ),
            },
        );
        upstreams.insert(
            "beta".to_string(),
            UpstreamEntry {
                client: DualMockServer {
                    server_name: "beta",
                },
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let result = gateway.list_tools().await.unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["beta__read_file"]);
    }

    #[tokio::test]
    async fn list_tools_applies_denylist_filter() {
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
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let result = gateway.list_tools().await.unwrap();
        assert!(result.is_empty());
    }

    // --- call_tool ---

    #[tokio::test]
    async fn call_tool_routes_to_correct_upstream() {
        let gateway = Gateway::new(two_server_setup(), NullCliRunner);

        let request = ToolCallRequest {
            name: "alpha__echo".to_string(),
            arguments: Some(
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    r#"{"message":"hello"}"#,
                )
                .unwrap(),
            ),
        };
        let result = gateway.call_tool(request).await.unwrap();
        assert_eq!(text_from_result(&result), Some("hello"));

        let request = ToolCallRequest {
            name: "beta__read_file".to_string(),
            arguments: Some(
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    r#"{"path":"/etc/hosts"}"#,
                )
                .unwrap(),
            ),
        };
        let result = gateway.call_tool(request).await.unwrap();
        assert_eq!(text_from_result(&result), Some("content of /etc/hosts"));
    }

    #[tokio::test]
    async fn call_tool_without_prefix_returns_error() {
        let gateway = Gateway::new(two_server_setup(), NullCliRunner);
        let request = ToolCallRequest {
            name: "echo".to_string(),
            arguments: None,
        };
        let err = gateway.call_tool(request).await.unwrap_err();
        assert!(err.to_string().contains("no server prefix"));
    }

    #[tokio::test]
    async fn call_tool_unknown_server_returns_error() {
        let gateway = Gateway::new(two_server_setup(), NullCliRunner);
        let request = ToolCallRequest {
            name: "unknown__echo".to_string(),
            arguments: None,
        };
        let err = gateway.call_tool(request).await.unwrap_err();
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
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let request = ToolCallRequest {
            name: "alpha__echo".to_string(),
            arguments: None,
        };
        let err = gateway.call_tool(request).await.unwrap_err();
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
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let request = ToolCallRequest {
            name: "alpha__nonexistent".to_string(),
            arguments: None,
        };
        let err = gateway.call_tool(request).await.unwrap_err();
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
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let request = ToolCallRequest {
            name: "beta__nonexistent".to_string(),
            arguments: None,
        };
        let err = gateway.call_tool(request).await.unwrap_err();
        assert!(err.to_string().contains("unknown tool"));
    }

    // --- CLI tools ---

    struct MockCliRunner;

    impl CliToolRunner for MockCliRunner {
        fn list_tools(&self) -> Vec<ToolDescriptor> {
            vec![ToolDescriptor {
                name: "cli-cat".to_string(),
                description: Some("Cat stdin to stdout".to_string()),
                schema: {
                    let mut m = serde_json::Map::new();
                    m.insert(
                        "type".to_string(),
                        serde_json::Value::String("object".to_string()),
                    );
                    m
                },
            }]
        }

        fn has_tool(&self, name: &str) -> bool {
            name == "cli-cat"
        }

        async fn call_tool(
            &self,
            _request: &ToolCallRequest,
        ) -> Result<ToolCallResult, GatewayError> {
            Ok(ToolCallResult::text_success("cli-cat output".to_string()))
        }
    }

    #[tokio::test]
    async fn list_tools_includes_cli_tools_unprefixed() {
        let gateway = Gateway::new(two_server_setup(), MockCliRunner);
        let result = gateway.list_tools().await.unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"alpha__echo"));
        assert!(names.contains(&"beta__read_file"));
        assert!(names.contains(&"cli-cat"));
    }

    #[tokio::test]
    async fn call_cli_tool_routes_to_runner() {
        let gateway = Gateway::new(two_server_setup(), MockCliRunner);
        let request = ToolCallRequest {
            name: "cli-cat".to_string(),
            arguments: None,
        };
        let result = gateway.call_tool(request).await.unwrap();
        let text = text_from_result(&result).unwrap_or("");
        assert!(text.contains("cli-cat"));
    }

    #[tokio::test]
    async fn call_upstream_tool_when_cli_present() {
        let gateway = Gateway::new(two_server_setup(), MockCliRunner);
        let request = ToolCallRequest {
            name: "alpha__echo".to_string(),
            arguments: Some(
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    r#"{"message":"upstream"}"#,
                )
                .unwrap(),
            ),
        };
        let result = gateway.call_tool(request).await.unwrap();
        assert_eq!(text_from_result(&result), Some("upstream"));
    }

    #[tokio::test]
    async fn cli_tools_only_no_upstreams() {
        let gateway: Gateway<DualMockServer, MockCliRunner> =
            Gateway::new(BTreeMap::new(), MockCliRunner);
        let result = gateway.list_tools().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.first().map(|t| t.name.as_str()), Some("cli-cat"));
    }

    // --- Resilience: timeout handling ---

    struct SlowServer {
        list_delay: Duration,
        call_delay: Duration,
    }

    impl UpstreamClient for SlowServer {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            tokio::time::sleep(self.list_delay).await;
            Ok(vec![ToolDescriptor {
                name: "slow_echo".to_string(),
                description: Some("slow echo".to_string()),
                schema: serde_json::from_value(serde_json::json!({
                    "type": "object",
                    "properties": { "message": { "type": "string" } }
                }))
                .unwrap(),
            }])
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            tokio::time::sleep(self.call_delay).await;
            let input = request
                .arguments
                .and_then(|a| a.get("message").cloned())
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            Ok(ToolCallResult::text_success(input))
        }
    }

    // A mock upstream that always fails
    struct FailingUpstream;

    impl UpstreamClient for FailingUpstream {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            Err(UpstreamError::Service("connection closed".to_string()))
        }

        async fn call_tool(
            &self,
            _request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            Err(UpstreamError::Service("connection closed".to_string()))
        }
    }

    // For mixed-type tests (fast + slow), use an enum wrapper
    enum TestUpstream {
        Fast(DualMockServer),
        Slow(SlowServer),
        Failing(FailingUpstream),
    }

    impl UpstreamClient for TestUpstream {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            match self {
                TestUpstream::Fast(s) => s.list_tools().await,
                TestUpstream::Slow(s) => s.list_tools().await,
                TestUpstream::Failing(s) => s.list_tools().await,
            }
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            match self {
                TestUpstream::Fast(s) => s.call_tool(request).await,
                TestUpstream::Slow(s) => s.call_tool(request).await,
                TestUpstream::Failing(s) => s.call_tool(request).await,
            }
        }
    }

    #[tokio::test]
    async fn list_tools_skips_timed_out_upstream() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "fast".to_string(),
            UpstreamEntry {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        );
        upstreams.insert(
            "slow".to_string(),
            UpstreamEntry {
                client: TestUpstream::Slow(SlowServer {
                    list_delay: Duration::from_secs(5),
                    call_delay: Duration::ZERO,
                }),
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner)
            .with_operation_timeout(Duration::from_millis(100));
        let result = gateway.list_tools().await.unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["fast__echo"]);
    }

    #[tokio::test]
    async fn call_tool_returns_timeout_error_with_clear_message() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "srv".to_string(),
            UpstreamEntry {
                client: SlowServer {
                    list_delay: Duration::ZERO,
                    call_delay: Duration::from_secs(5),
                },
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner)
            .with_operation_timeout(Duration::from_millis(100));
        let request = ToolCallRequest {
            name: "srv__slow_echo".to_string(),
            arguments: Some(
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    r#"{"message":"test"}"#,
                )
                .unwrap(),
            ),
        };
        let err = gateway.call_tool(request).await.unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("timed out"),
            "error should mention timeout: {err_msg}"
        );
        assert!(
            err_msg.contains("srv"),
            "error should mention server name: {err_msg}"
        );
    }

    #[tokio::test]
    async fn list_tools_returns_all_when_within_timeout() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "fast".to_string(),
            UpstreamEntry {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        );
        upstreams.insert(
            "slow".to_string(),
            UpstreamEntry {
                client: TestUpstream::Slow(SlowServer {
                    list_delay: Duration::from_millis(10),
                    call_delay: Duration::ZERO,
                }),
                filter: passthrough_filter(),
            },
        );
        let gateway =
            Gateway::new(upstreams, NullCliRunner).with_operation_timeout(Duration::from_secs(2));
        let result = gateway.list_tools().await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn call_tool_succeeds_when_slow_upstream_responds_within_timeout() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "srv".to_string(),
            UpstreamEntry {
                client: SlowServer {
                    list_delay: Duration::ZERO,
                    call_delay: Duration::from_millis(10),
                },
                filter: passthrough_filter(),
            },
        );
        let gateway =
            Gateway::new(upstreams, NullCliRunner).with_operation_timeout(Duration::from_secs(2));
        let request = ToolCallRequest {
            name: "srv__slow_echo".to_string(),
            arguments: Some(
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    r#"{"message":"hello"}"#,
                )
                .unwrap(),
            ),
        };
        let result = gateway.call_tool(request).await.unwrap();
        assert_eq!(text_from_result(&result), Some("hello"));
    }

    #[tokio::test]
    async fn list_tools_skips_erroring_upstream_gracefully() {
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
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let result = gateway.list_tools().await.unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["good__echo"]);
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
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let request = ToolCallRequest {
            name: "bad__anything".to_string(),
            arguments: None,
        };
        let err = gateway.call_tool(request).await.unwrap_err();
        assert!(err.to_string().contains("connection closed"));
    }

    #[tokio::test]
    async fn call_tool_routes_through_fast_test_upstream() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            UpstreamEntry {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let request = ToolCallRequest {
            name: "alpha__echo".to_string(),
            arguments: Some(
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    r#"{"message":"via-enum"}"#,
                )
                .unwrap(),
            ),
        };
        let result = gateway.call_tool(request).await.unwrap();
        assert_eq!(text_from_result(&result), Some("via-enum"));
    }

    #[tokio::test]
    async fn call_tool_on_slow_test_upstream_within_timeout() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "slow".to_string(),
            UpstreamEntry {
                client: TestUpstream::Slow(SlowServer {
                    list_delay: Duration::ZERO,
                    call_delay: Duration::from_millis(10),
                }),
                filter: passthrough_filter(),
            },
        );
        let gateway =
            Gateway::new(upstreams, NullCliRunner).with_operation_timeout(Duration::from_secs(2));
        let request = ToolCallRequest {
            name: "slow__slow_echo".to_string(),
            arguments: Some(
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                    r#"{"message":"via-slow-enum"}"#,
                )
                .unwrap(),
            ),
        };
        let result = gateway.call_tool(request).await.unwrap();
        assert_eq!(text_from_result(&result), Some("via-slow-enum"));
    }

    #[tokio::test]
    async fn call_unknown_cli_tool_returns_error() {
        let gateway: Gateway<DualMockServer, MockCliRunner> =
            Gateway::new(BTreeMap::new(), MockCliRunner);
        let request = ToolCallRequest {
            name: "nonexistent-cli".to_string(),
            arguments: None,
        };
        let err = gateway.call_tool(request).await.unwrap_err();
        assert!(err.to_string().contains("no server prefix"));
    }
}
