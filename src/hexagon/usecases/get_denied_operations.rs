use crate::hexagon::ports::{ProviderConfigStore, ProviderEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct GetDeniedOperations;

impl GetDeniedOperations {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
        name: &str,
    ) -> Result<Vec<String>, RegistryError> {
        let entries = store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries.get(name).ok_or_else(|| RegistryError::NotFound {
            name: name.to_string(),
        })?;
        Ok(entry.denied_operations().to_vec())
    }
}
