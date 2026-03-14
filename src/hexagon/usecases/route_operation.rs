use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationCallRequest, OperationCallResult, OperationPolicy,
    ProviderClient, ProviderError,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct RouteOperation;

fn validate_mapping(operation_name: &str) -> Result<(&str, &str), GatewayError> {
    decode(operation_name).ok_or_else(|| GatewayError::InvalidMapping {
        operation: operation_name.to_string(),
    })
}

fn unknown_provider_error(provider_name: &str, operation_name: &str) -> GatewayError {
    GatewayError::UnknownProvider {
        provider: provider_name.to_string(),
        operation: operation_name.to_string(),
    }
}

fn provider_error(e: ProviderError) -> GatewayError {
    GatewayError::Provider(e.to_string())
}

impl RouteOperation {
    pub(crate) async fn execute<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        cli_runner: &C,
        request: OperationCallRequest,
    ) -> Result<OperationCallResult, GatewayError> {
        if cli_runner.has_operation(&request.name) {
            return cli_runner.call_operation(&request).await;
        }
        let (provider_name, raw_operation) = validate_mapping(&request.name)?;
        let entry = match providers.get(provider_name) {
            Some(e) => e,
            None => return Err(unknown_provider_error(provider_name, &request.name)),
        };
        if !entry.filter.is_allowed(raw_operation) {
            return Err(GatewayError::OperationNotAllowed {
                operation: request.name.clone(),
            });
        }
        let provider_request = OperationCallRequest {
            name: raw_operation.to_string(),
            arguments: request.arguments,
        };
        entry
            .client
            .call_operation(provider_request)
            .await
            .map_err(provider_error)
    }
}
