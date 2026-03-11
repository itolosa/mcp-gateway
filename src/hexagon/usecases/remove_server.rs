use crate::hexagon::ports::ServerConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct RemoveServer;

impl RemoveServer {
    pub(crate) fn execute<S: ServerConfigStore>(
        store: &S,
        name: &str,
    ) -> Result<(), RegistryError> {
        let mut entries = store.load_entries().map_err(RegistryError::Storage)?;

        if entries.remove(name).is_none() {
            return Err(RegistryError::NotFound {
                name: name.to_string(),
            });
        }

        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::hexagon::ports::ServerConfigStore;
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::RemoveServer;

    #[test]
    fn remove_existing_server_succeeds() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);

        RemoveServer::execute(&store, "s1").unwrap();

        let entries = store.load_entries().unwrap();
        assert!(!entries.contains_key("s1"));
    }

    #[test]
    fn remove_nonexistent_server_returns_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());

        let result = RemoveServer::execute(&store, "nope");
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn remove_with_store_load_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = RemoveServer::execute(&store, "test");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_with_store_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("test".to_string(), stdio_entry());
        let store = FailingStore {
            fail_load: false,
            entries,
        };
        let result = RemoveServer::execute(&store, "test");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
