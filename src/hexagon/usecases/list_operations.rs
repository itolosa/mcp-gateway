use std::collections::BTreeMap;

use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationDescriptor, OperationPolicy, ProviderClient,
};
use crate::hexagon::usecases::mapping::encode;

use super::gateway::ProviderHandle;

pub(crate) struct ListOperations;

impl ListOperations {
    pub(crate) async fn execute<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        cli_runner: &C,
    ) -> Result<Vec<OperationDescriptor>, GatewayError> {
        let mut all_operations = Vec::new();
        for (name, entry) in providers {
            let operations = match entry.client.list_operations().await {
                Ok(ops) => ops,
                Err(_) => continue,
            };
            let encoded: Vec<_> = operations
                .into_iter()
                .filter(|op| entry.filter.is_allowed(&op.name))
                .map(|op| OperationDescriptor {
                    name: encode(name, &op.name),
                    ..op
                })
                .collect();
            all_operations.extend(encoded);
        }
        all_operations.extend(cli_runner.list_operations());
        Ok(all_operations)
    }
}
