use std::collections::BTreeMap;

use crate::hexagon::ports::driven::cli_operation_runner::{self, CliOperationRunner};
use crate::hexagon::ports::driven::operation_policy::OperationPolicy;
use crate::hexagon::ports::driven::provider_client::{self, ProviderClient, ProviderError};
use crate::hexagon::ports::driving::route_operation::{
    OperationCallRequest, OperationCallResult, RouteOperationError,
};
use crate::hexagon::usecases::mapping::decode;

use super::gateway::ProviderHandle;

pub(crate) struct RouteOperation;

fn validate_mapping(operation_name: &str) -> Result<(&str, &str), RouteOperationError> {
    decode(operation_name).ok_or_else(|| RouteOperationError::InvalidMapping {
        operation: operation_name.to_string(),
    })
}

fn unknown_provider_error(provider_name: &str, operation_name: &str) -> RouteOperationError {
    RouteOperationError::UnknownProvider {
        provider: provider_name.to_string(),
        operation: operation_name.to_string(),
    }
}

fn provider_error(e: ProviderError) -> RouteOperationError {
    RouteOperationError::Provider(e.to_string())
}

impl RouteOperation {
    pub(crate) async fn execute<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        cli_runner: &C,
        request: OperationCallRequest,
    ) -> Result<OperationCallResult, RouteOperationError> {
        if cli_runner.has_operation(&request.name) {
            let cli_request = cli_operation_runner::OperationCallRequest {
                name: request.name,
                arguments: request.arguments,
            };
            return cli_runner
                .call_operation(&cli_request)
                .await
                .map(|r| OperationCallResult {
                    content: r.content,
                    is_error: r.is_error,
                })
                .map_err(|e| RouteOperationError::CliOperation(e.to_string()));
        }
        let (provider_name, raw_operation) = validate_mapping(&request.name)?;
        let entry = match providers.get(provider_name) {
            Some(e) => e,
            None => return Err(unknown_provider_error(provider_name, &request.name)),
        };
        if !entry.filter.is_allowed(raw_operation) {
            return Err(RouteOperationError::OperationNotAllowed {
                operation: request.name.clone(),
            });
        }
        let provider_request = provider_client::OperationCallRequest {
            name: raw_operation.to_string(),
            arguments: request.arguments,
        };
        entry
            .client
            .call_operation(provider_request)
            .await
            .map(|r| OperationCallResult {
                content: r.content,
                is_error: r.is_error,
            })
            .map_err(provider_error)
    }
}
