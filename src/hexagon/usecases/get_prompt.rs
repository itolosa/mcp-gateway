use std::collections::BTreeMap;

use crate::hexagon::ports::driven::operation_policy::OperationPolicy;
use crate::hexagon::ports::driven::provider_client::{self, ProviderClient, ProviderError};
use crate::hexagon::ports::driving::get_prompt::{
    GetPromptError, PromptGetRequest, PromptGetResult,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct GetPrompt;

impl GetPrompt {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        request: PromptGetRequest,
    ) -> Result<PromptGetResult, GetPromptError> {
        let (provider_name, raw_name) =
            decode(&request.name).ok_or_else(|| GetPromptError::InvalidMapping {
                operation: request.name.clone(),
            })?;
        let entry =
            providers
                .get(provider_name)
                .ok_or_else(|| GetPromptError::UnknownProvider {
                    provider: provider_name.to_string(),
                    operation: request.name.clone(),
                })?;
        let provider_request = provider_client::PromptGetRequest {
            name: raw_name.to_string(),
            arguments: request.arguments,
        };
        entry
            .client
            .get_prompt(provider_request)
            .await
            .map(|r| PromptGetResult { json: r.json })
            .map_err(|e: ProviderError| GetPromptError::Provider(e.to_string()))
    }
}
