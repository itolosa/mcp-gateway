use std::collections::BTreeMap;

use crate::hexagon::ports::{
    GatewayError, OperationPolicy, ProviderClient, ProviderError, ResourceReadRequest,
    ResourceReadResult,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct ReadResource;

impl ReadResource {
    pub(crate) async fn execute<U: ProviderClient, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        request: ResourceReadRequest,
    ) -> Result<ResourceReadResult, GatewayError> {
        let (provider_name, raw_uri) =
            decode(&request.uri).ok_or_else(|| GatewayError::InvalidMapping {
                operation: request.uri.clone(),
            })?;
        let entry = providers
            .get(provider_name)
            .ok_or_else(|| GatewayError::UnknownProvider {
                provider: provider_name.to_string(),
                operation: request.uri.clone(),
            })?;
        let provider_request = ResourceReadRequest {
            uri: raw_uri.to_string(),
        };
        entry
            .client
            .read_resource(provider_request)
            .await
            .map_err(|e: ProviderError| GatewayError::Provider(e.to_string()))
    }
}
