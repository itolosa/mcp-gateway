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
