use std::collections::BTreeMap;

use crate::hexagon::ports::{
    GatewayError, OperationPolicy, ProviderClient, ResourceDescriptor, ResourceTemplateDescriptor,
};
use crate::hexagon::usecases::mapping::{encode, update_json_field};

use super::gateway::ProviderHandle;

pub(crate) struct ListResources;

impl ListResources {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
    ) -> Result<Vec<ResourceDescriptor>, GatewayError> {
        let mut all = Vec::new();
        for (name, entry) in providers {
            let resources = match entry.client.list_resources().await {
                Ok(r) => r,
                Err(_) => continue,
            };
            for mut r in resources {
                r.uri = encode(name, &r.uri);
                r.name = encode(name, &r.name);
                r.json = update_json_field(&r.json, "uri", &r.uri);
                r.json = update_json_field(&r.json, "name", &r.name);
                all.push(r);
            }
        }
        Ok(all)
    }
}

pub(crate) struct ListResourceTemplates;

impl ListResourceTemplates {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
    ) -> Result<Vec<ResourceTemplateDescriptor>, GatewayError> {
        let mut all = Vec::new();
        for (name, entry) in providers {
            let templates = match entry.client.list_resource_templates().await {
                Ok(t) => t,
                Err(_) => continue,
            };
            for mut t in templates {
                t.uri_template = encode(name, &t.uri_template);
                t.name = encode(name, &t.name);
                t.json = update_json_field(&t.json, "uriTemplate", &t.uri_template);
                t.json = update_json_field(&t.json, "name", &t.name);
                all.push(t);
            }
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
    use crate::hexagon::ports::{ResourceDescriptor, ResourceTemplateDescriptor};
    use crate::hexagon::usecases::gateway::test_helpers::*;
    use crate::hexagon::usecases::gateway::{Gateway, ProviderHandle};

    use super::{ListResourceTemplates, ListResources};

    #[tokio::test]
    async fn should_return_prefixed_resources_from_all_providers() {
        let mut providers = BTreeMap::new();
        providers.insert(
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
        );
        providers.insert(
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
        );

        let result = ListResources::execute(&providers).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].uri, "server-a__file:///a.txt");
        assert_eq!(result[0].name, "server-a__a.txt");
        assert_eq!(result[1].uri, "server-b__file:///b.txt");
        assert_eq!(result[1].name, "server-b__b.txt");
    }

    #[tokio::test]
    async fn should_return_empty_when_no_providers() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let result = ListResources::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_skip_failing_providers_for_resources() {
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
        providers.insert(
            "bad".to_string(),
            ProviderHandle {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let result = ListResources::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_return_prefixed_resource_templates() {
        let mut providers = BTreeMap::new();
        providers.insert(
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
        );

        let result = ListResourceTemplates::execute(&providers).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uri_template, "server__file:///{path}");
        assert_eq!(result[0].name, "server__file-template");
    }

    #[tokio::test]
    async fn should_return_empty_templates_when_no_providers() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let result = ListResourceTemplates::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_skip_failing_providers_for_templates() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "bad".to_string(),
            ProviderHandle {
                client: TestUpstream::Failing(FailingUpstream),
                filter: passthrough_filter(),
            },
        );
        let result = ListResourceTemplates::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_list_resource_templates_from_fast_upstream() {
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
        let result = ListResourceTemplates::execute(&providers).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_forward_list_resources_through_gateway() {
        let providers: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            two_server_setup();
        let gateway = Gateway::new(providers, NullCliRunner);
        let result = gateway.list_resources().await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn should_forward_list_resource_templates_through_gateway() {
        let providers: BTreeMap<String, ProviderHandle<DualMockServer, TestFilter>> =
            two_server_setup();
        let gateway = Gateway::new(providers, NullCliRunner);
        let result = gateway.list_resource_templates().await.unwrap();
        assert!(result.is_empty());
    }
}
