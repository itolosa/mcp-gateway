use std::collections::BTreeMap;

use crate::hexagon::ports::{
    GatewayError, OperationPolicy, PromptGetRequest, PromptGetResult, ProviderClient, ProviderError,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct GetPrompt;

impl GetPrompt {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        request: PromptGetRequest,
    ) -> Result<PromptGetResult, GatewayError> {
        let (provider_name, raw_name) =
            decode(&request.name).ok_or_else(|| GatewayError::InvalidMapping {
                operation: request.name.clone(),
            })?;
        let entry = providers
            .get(provider_name)
            .ok_or_else(|| GatewayError::UnknownProvider {
                provider: provider_name.to_string(),
                operation: request.name.clone(),
            })?;
        let provider_request = PromptGetRequest {
            name: raw_name.to_string(),
            arguments: request.arguments,
        };
        entry
            .client
            .get_prompt(provider_request)
            .await
            .map_err(|e: ProviderError| GatewayError::Provider(e.to_string()))
    }
}
