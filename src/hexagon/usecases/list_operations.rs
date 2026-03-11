use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationDescriptor, OperationPolicy, ProviderClient,
};
use crate::hexagon::usecases::mapping::encode;

use super::gateway::ProviderHandle;

pub(crate) struct ListOperations;

impl ListOperations {
    pub(crate) async fn execute<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        cli_runner: &C,
    ) -> Result<Vec<OperationDescriptor>, GatewayError> {
        let mut all_operations = Vec::new();
        for (name, entry) in providers {
            let operations = match entry.client.list_operations().await {
                Ok(ops) => ops,
                Err(_) => continue,
            };
            for mut op in operations {
                if entry.filter.is_allowed(&op.name) {
                    op.name = encode(name, &op.name);
                    all_operations.push(op);
                }
            }
        }
        all_operations.extend(cli_runner.list_operations());
        Ok(all_operations)
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
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::ProviderHandle;

    use super::ListOperations;

    #[tokio::test]
    async fn list_tools_returns_prefixed_tools_from_all_upstreams() {
        let upstreams = two_server_setup();
        let result = ListOperations::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha__echo"));
        assert!(names.contains(&"beta__read_file"));
    }

    #[tokio::test]
    async fn list_tools_with_no_upstreams_returns_empty() {
        let upstreams: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            BTreeMap::new();
        let result = ListOperations::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_tools_applies_per_server_allowlist_filter() {
        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "alpha".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "alpha",
                },
                filter: CompoundPolicy::new(
                    AllowlistPolicy::new(vec!["nonexistent".to_string()]),
                    DenylistPolicy::new(vec![]),
                ),
            },
        );
        upstreams.insert(
            "beta".to_string(),
            ProviderHandle {
                client: DualMockServer {
                    server_name: "beta",
                },
                filter: passthrough_filter(),
            },
        );
        let result = ListOperations::execute(&upstreams, &NullCliRunner)
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
        let result = ListOperations::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_tools_includes_cli_tools_unprefixed() {
        let upstreams = two_server_setup();
        let result = ListOperations::execute(&upstreams, &MockCliRunner)
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
        let upstreams: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            BTreeMap::new();
        let result = ListOperations::execute(&upstreams, &MockCliRunner)
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
        let result = ListOperations::execute(&upstreams, &NullCliRunner)
            .await
            .unwrap();
        let names: Vec<&str> = result.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["good__echo"]);
    }
}
