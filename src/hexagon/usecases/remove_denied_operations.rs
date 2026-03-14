use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use crate::hexagon::ports::driven::provider_entry::ProviderEntry;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct RemoveDeniedOperations;

impl RemoveDeniedOperations {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError>
    where
        S::Entry: ProviderEntry,
    {
        let mut entries = store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::NotFound {
                name: name.to_string(),
            })?;
        entry.denied_operations_mut().retain(|t| !tools.contains(t));
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}
