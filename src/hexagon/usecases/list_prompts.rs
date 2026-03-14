use std::collections::BTreeMap;

use crate::hexagon::ports::{GatewayError, OperationPolicy, PromptDescriptor, ProviderClient};
use crate::hexagon::usecases::mapping::{encode, update_json_field};

use super::gateway::ProviderHandle;

pub(crate) struct ListPrompts;

impl ListPrompts {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
    ) -> Result<Vec<PromptDescriptor>, GatewayError> {
        let mut all = Vec::new();
        for (name, entry) in providers {
            let prompts = match entry.client.list_prompts().await {
                Ok(p) => p,
                Err(_) => continue,
            };
            let encoded: Vec<_> = prompts
                .into_iter()
                .filter(|p| entry.filter.is_allowed(&p.name))
                .map(|p| {
                    let encoded_name = encode(name, &p.name);
                    let json = update_json_field(&p.json, "name", &encoded_name);
                    PromptDescriptor {
                        name: encoded_name,
                        json,
                    }
                })
                .collect();
            all.extend(encoded);
        }
        Ok(all)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use std::collections::BTreeMap;

    use crate::adapters::driven::connectivity::cli_execution::NullCliRunner;
    use crate::hexagon::ports::PromptDescriptor;
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::{Gateway, ProviderHandle};

    use super::ListPrompts;

    #[tokio::test]
    async fn should_return_prefixed_prompts_from_all_providers() {
        let mut providers = BTreeMap::new();
        providers.insert(
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
        );
        providers.insert(
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
        );

        let result = ListPrompts::execute(&providers).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "server-a__summarize");
        assert_eq!(result[1].name, "server-b__translate");
    }

    #[tokio::test]
    async fn should_return_empty_when_no_providers() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let result = ListPrompts::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_skip_failing_providers() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "bad".to_string(),
            ProviderHandle {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let result = ListPrompts::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_list_prompts_from_fast_upstream() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "good".to_string(),
            ProviderHandle {
                client: TestUpstream::Fast(DualMockServer {
                    server_name: "alpha",
                }),
                filter: passthrough_filter(),
            },
        );
        let result = ListPrompts::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_forward_list_prompts_through_gateway() {
        let providers: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            two_server_setup();
        let gateway = Gateway::new(providers, NullCliRunner);
        let result = gateway.list_prompts().await.unwrap();
        assert!(result.is_empty());
    }
}
