use std::collections::BTreeMap;

use crate::hexagon::ports::driven::cli_operation_runner::CliOperationRunner;
use crate::hexagon::ports::driven::operation_policy::OperationPolicy;
use crate::hexagon::ports::driven::provider_client::ProviderClient;
use crate::hexagon::ports::driving::list_operations::OperationDescriptor;
use crate::hexagon::usecases::mapping::encode;

use super::gateway::ProviderHandle;

pub(crate) struct ListOperations;

impl ListOperations {
    pub(crate) async fn execute<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy>(
        providers: &BTreeMap<String, ProviderHandle<U, F>>,
        cli_runner: &C,
    ) -> Result<Vec<OperationDescriptor>, std::convert::Infallible> {
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
                    description: op.description,
                    schema: op.schema,
                })
                .collect();
            all_operations.extend(encoded);
        }
        all_operations.extend(cli_runner.list_operations().into_iter().map(|op| {
            OperationDescriptor {
                name: op.name,
                description: op.description,
                schema: op.schema,
            }
        }));
        Ok(all_operations)
    }
}
