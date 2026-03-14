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
            let encoded: Vec<_> = resources
                .into_iter()
                .map(|r| {
                    let uri = encode(name, &r.uri);
                    let encoded_name = encode(name, &r.name);
                    let json = update_json_field(&r.json, "uri", &uri);
                    let json = update_json_field(&json, "name", &encoded_name);
                    ResourceDescriptor {
                        uri,
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
            let encoded: Vec<_> = templates
                .into_iter()
                .map(|t| {
                    let uri_template = encode(name, &t.uri_template);
                    let encoded_name = encode(name, &t.name);
                    let json = update_json_field(&t.json, "uriTemplate", &uri_template);
                    let json = update_json_field(&json, "name", &encoded_name);
                    ResourceTemplateDescriptor {
                        uri_template,
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
