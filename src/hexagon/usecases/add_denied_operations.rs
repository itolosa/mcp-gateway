use crate::hexagon::ports::{ProviderConfigStore, ProviderEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct AddDeniedOperations;

impl AddDeniedOperations {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError> {
        let mut entries = store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::NotFound {
                name: name.to_string(),
            })?;
        let denied = entry.denied_operations_mut();
        for tool in tools {
            if !denied.contains(tool) {
                denied.push(tool.clone());
            }
        }
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}
