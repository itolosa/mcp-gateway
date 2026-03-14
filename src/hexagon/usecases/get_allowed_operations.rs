use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use crate::hexagon::ports::driven::provider_entry::ProviderEntry;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct GetAllowedOperations;

impl GetAllowedOperations {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
        name: &str,
    ) -> Result<Vec<String>, RegistryError>
    where
        S::Entry: ProviderEntry,
    {
        let entries = store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries.get(name).ok_or_else(|| RegistryError::NotFound {
            name: name.to_string(),
        })?;
        Ok(entry.allowed_operations().to_vec())
    }
}
