use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliToolRunner, GatewayError, ToolDescriptor, ToolFilter, UpstreamClient,
};
use crate::hexagon::usecases::prefix::prefix_tool_name;

use super::gateway::UpstreamEntry;

pub(crate) struct ListTools;

impl ListTools {
    pub(crate) async fn execute<U: UpstreamClient, C: CliToolRunner, F: ToolFilter>(
        upstreams: &BTreeMap<String, UpstreamEntry<U, F>>,
        cli_runner: &C,
    ) -> Result<Vec<ToolDescriptor>, GatewayError> {
        let mut all_tools = Vec::new();
        for (name, entry) in upstreams {
            let tools = match entry.client.list_tools().await {
                Ok(tools) => tools,
                Err(_) => continue,
            };
            for mut tool in tools {
                if entry.filter.is_tool_allowed(&tool.name) {
                    tool.name = prefix_tool_name(name, &tool.name);
                    all_tools.push(tool);
                }
            }
        }
        all_tools.extend(cli_runner.list_tools());
        Ok(all_tools)
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
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::UpstreamEntry;

    use super::ListTools;

    #[tokio::test]
    async fn list_tools_returns_prefixed_tools_from_all_upstreams() {
        let upstreams = two_server_setup();
        let result = ListTools::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha__echo"));
        assert!(names.contains(&"beta__read_file"));
    }

    #[tokio::test]
    async fn list_tools_with_no_upstreams_returns_empty() {
        let upstreams: BTreeMap<String, UpstreamEntry<DualMockServer, TestFilter>> =
            BTreeMap::new();
        let result = ListTools::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
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
        let result = ListTools::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
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
        let result = ListTools::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_tools_includes_cli_tools_unprefixed() {
        let upstreams = two_server_setup();
        let result = ListTools::execute(&upstreams, &MockCliRunner)
            .await
            .unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"alpha__echo"));
        assert!(names.contains(&"beta__read_file"));
        assert!(names.contains(&"cli-cat"));
    }

    #[tokio::test]
    async fn cli_tools_only_no_upstreams() {
        let upstreams: BTreeMap<String, UpstreamEntry<DualMockServer, TestFilter>> =
            BTreeMap::new();
        let result = ListTools::execute(&upstreams, &MockCliRunner)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.first().map(|t| t.name.as_str()), Some("cli-cat"));
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
        let result = ListTools::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["good__echo"]);
    }
}
