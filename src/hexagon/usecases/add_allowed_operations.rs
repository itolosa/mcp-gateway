use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use crate::hexagon::ports::driven::provider_entry::ProviderEntry;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct AddAllowedOperations;

impl AddAllowedOperations {
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
        let allowed = entry.allowed_operations_mut();
        for tool in tools {
            if !allowed.contains(tool) {
                allowed.push(tool.clone());
            }
        }
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}
