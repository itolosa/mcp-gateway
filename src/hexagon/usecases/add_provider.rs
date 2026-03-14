use crate::hexagon::ports::ProviderConfigStore;
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::hexagon::ports::ProviderConfigStore;
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::AddProvider;

    #[test]
    fn add_to_empty_config_succeeds() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let result = AddProvider::execute(&store, "test".to_string(), stdio_entry());
        assert!(result.is_ok());
    }

    #[test]
    fn add_persists_to_store() {
        let store = FakeConfigStore::new(BTreeMap::new());
        AddProvider::execute(&store, "my-server".to_string(), http_entry()).unwrap();

        let entries = store.load_entries().unwrap();
        assert!(entries.contains_key("my-server"));
    }

    #[test]
    fn add_duplicate_name_returns_already_exists() {
        let mut entries = BTreeMap::new();
        entries.insert("existing".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);

        let result = AddProvider::execute(&store, "existing".to_string(), http_entry());
        assert!(matches!(
            result,
            Err(RegistryError::AlreadyExists { name }) if name == "existing"
        ));
    }

    #[test]
    fn add_with_store_load_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = AddProvider::execute(&store, "test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_with_store_save_error_propagates() {
        let store = FailingStore {
            fail_load: false,
            entries: BTreeMap::new(),
        };
        let result = AddProvider::execute(&store, "test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
