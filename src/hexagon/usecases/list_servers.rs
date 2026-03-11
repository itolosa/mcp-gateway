use std::collections::BTreeMap;

use crate::hexagon::ports::ServerConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct ListServers;

impl ListServers {
    pub(crate) fn execute<S: ServerConfigStore>(
        store: &S,
    ) -> Result<BTreeMap<String, S::Entry>, RegistryError> {
        store.load_entries().map_err(RegistryError::Storage)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::ListServers;

    #[test]
    fn list_empty_config_returns_empty() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let result = ListServers::execute(&store).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn list_populated_config_returns_all_servers() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        entries.insert("h1".to_string(), http_entry());
        let store = FakeConfigStore::new(entries);

        let result = ListServers::execute(&store).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("s1"));
        assert!(result.contains_key("h1"));
    }

    #[test]
    fn list_with_store_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = ListServers::execute(&store);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
