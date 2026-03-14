use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct AddProvider;

impl AddProvider {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
        name: String,
        entry: S::Entry,
    ) -> Result<(), RegistryError> {
        let entries = store.load_entries().map_err(RegistryError::Storage)?;

        if entries.contains_key(&name) {
            return Err(RegistryError::AlreadyExists { name });
        }

        let entries = entries
            .into_iter()
            .chain(std::iter::once((name, entry)))
            .collect();
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}
