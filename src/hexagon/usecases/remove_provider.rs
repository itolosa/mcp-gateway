use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct RemoveProvider;

impl RemoveProvider {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
        name: &str,
    ) -> Result<(), RegistryError> {
        let entries = store.load_entries().map_err(RegistryError::Storage)?;

        if !entries.contains_key(name) {
            return Err(RegistryError::NotFound {
                name: name.to_string(),
            });
        }

        let entries = entries.into_iter().filter(|(k, _)| k != name).collect();
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}
