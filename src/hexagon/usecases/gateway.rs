use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliToolRunner, GatewayError, ToolCallRequest, ToolCallResult, ToolDescriptor, ToolFilter,
    UpstreamClient,
};

use super::call_tool::CallTool;
use super::list_tools::ListTools;

pub struct UpstreamEntry<U, F> {
    pub client: U,
    pub filter: F,
}

pub struct Gateway<U, C, F> {
    upstreams: BTreeMap<String, UpstreamEntry<U, F>>,
    cli_runner: C,
}

impl<U: UpstreamClient, C: CliToolRunner, F: ToolFilter> Gateway<U, C, F> {
    pub fn new(upstreams: BTreeMap<String, UpstreamEntry<U, F>>, cli_runner: C) -> Self {
        Self {
            upstreams,
            cli_runner,
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, GatewayError> {
        ListTools::execute(&self.upstreams, &self.cli_runner).await
    }

    pub async fn call_tool(
        &self,
        request: ToolCallRequest,
    ) -> Result<ToolCallResult, GatewayError> {
        CallTool::execute(&self.upstreams, &self.cli_runner, request).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
pub(crate) mod test_helpers {
    use std::collections::BTreeMap;

    use crate::hexagon::entities::policy::allowlist::AllowlistFilter;
    use crate::hexagon::entities::policy::compound::CompoundFilter;
    use crate::hexagon::entities::policy::denylist::DenylistFilter;
    use crate::hexagon::ports::{
        CliToolRunner, GatewayError, ToolCallRequest, ToolCallResult, ToolDescriptor,
        UpstreamClient, UpstreamError,
    };

    use super::UpstreamEntry;

    pub(crate) struct MockServerA;

    impl UpstreamClient for MockServerA {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            Ok(vec![ToolDescriptor {
                name: "echo".to_string(),
                description: Some("echoes input".to_string()),
                schema: r#"{"type":"object","properties":{"message":{"type":"string"}}}"#
                    .to_string(),
            }])
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            if request.name == "echo" {
                let input = request.arguments.unwrap_or_default();
                Ok(ToolCallResult {
                    content: vec![input],
                    is_error: false,
                })
            } else {
                Err(UpstreamError::Service(format!(
                    "unknown tool: {}",
                    request.name
                )))
            }
        }
    }

    pub(crate) struct MockServerB;

    impl UpstreamClient for MockServerB {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            Ok(vec![ToolDescriptor {
                name: "read_file".to_string(),
                description: Some("reads a file".to_string()),
                schema: r#"{"type":"object","properties":{"path":{"type":"string"}}}"#.to_string(),
            }])
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            if request.name == "read_file" {
                let args = request.arguments.unwrap_or_default();
                Ok(ToolCallResult {
                    content: vec![format!("content from {args}")],
                    is_error: false,
                })
            } else {
                Err(UpstreamError::Service(format!(
                    "unknown tool: {}",
                    request.name
                )))
            }
        }
    }

    pub(crate) struct DualMockServer {
        pub(crate) server_name: &'static str,
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

    pub(crate) fn passthrough_filter() -> CompoundFilter<AllowlistFilter, DenylistFilter> {
        CompoundFilter::new(AllowlistFilter::new(vec![]), DenylistFilter::new(vec![]))
    }

    pub(crate) type TestFilter = CompoundFilter<AllowlistFilter, DenylistFilter>;

    pub(crate) fn two_server_setup() -> BTreeMap<String, UpstreamEntry<DualMockServer, TestFilter>>
    {
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

    pub(crate) struct MockCliRunner;

    impl CliToolRunner for MockCliRunner {
        fn list_tools(&self) -> Vec<ToolDescriptor> {
            vec![ToolDescriptor {
                name: "cli-cat".to_string(),
                description: Some("Cat stdin to stdout".to_string()),
                schema: r#"{"type":"object"}"#.to_string(),
            }]
        }

        fn has_tool(&self, name: &str) -> bool {
            name == "cli-cat"
        }

        async fn call_tool(
            &self,
            _request: &ToolCallRequest,
        ) -> Result<ToolCallResult, GatewayError> {
            Ok(ToolCallResult {
                content: vec!["cli-cat output".to_string()],
                is_error: false,
            })
        }
    }

    pub(crate) struct FailingUpstream;

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

    pub(crate) enum TestUpstream {
        Fast(DualMockServer),
        Failing(FailingUpstream),
    }

    impl UpstreamClient for TestUpstream {
        async fn list_tools(&self) -> Result<Vec<ToolDescriptor>, UpstreamError> {
            match self {
                TestUpstream::Fast(s) => s.list_tools().await,
                TestUpstream::Failing(s) => s.list_tools().await,
            }
        }

        async fn call_tool(
            &self,
            request: ToolCallRequest,
        ) -> Result<ToolCallResult, UpstreamError> {
            match self {
                TestUpstream::Fast(s) => s.call_tool(request).await,
                TestUpstream::Failing(s) => s.call_tool(request).await,
            }
        }
    }
}
