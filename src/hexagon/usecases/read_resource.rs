use std::collections::BTreeMap;

use crate::hexagon::ports::driven::operation_policy::OperationPolicy;
use crate::hexagon::ports::driven::provider_client::{self, ProviderClient, ProviderError};
use crate::hexagon::ports::driving::read_resource::{
    ReadResourceError, ResourceReadRequest, ResourceReadResult,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct ReadResource;

impl ReadResource {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        request: ResourceReadRequest,
    ) -> Result<ResourceReadResult, ReadResourceError> {
        let (provider_name, raw_uri) =
            decode(&request.uri).ok_or_else(|| ReadResourceError::InvalidMapping {
                operation: request.uri.clone(),
            })?;
        let entry =
            providers
                .get(provider_name)
                .ok_or_else(|| ReadResourceError::UnknownProvider {
                    provider: provider_name.to_string(),
                    operation: request.uri.clone(),
                })?;
        let provider_request = provider_client::ResourceReadRequest {
            uri: raw_uri.to_string(),
        };
        entry
            .client
            .read_resource(provider_request)
            .await
            .map(|r| ResourceReadResult { json: r.json })
            .map_err(|e: ProviderError| ReadResourceError::Provider(e.to_string()))
    }
}
