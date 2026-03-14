use std::collections::BTreeMap;

use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct ListProviders;

impl ListProviders {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
    ) -> Result<BTreeMap<String, S::Entry>, RegistryError> {
        store.load_entries().map_err(RegistryError::Storage)
    }
}
