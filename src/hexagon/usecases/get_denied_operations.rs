use crate::hexagon::ports::{ProviderConfigStore, ProviderEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct GetDeniedOperations;

impl GetDeniedOperations {
    pub(crate) fn execute<S: ProviderConfigStore>(
        store: &S,
        name: &str,
    ) -> Result<Vec<String>, RegistryError> {
        let entries = store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries.get(name).ok_or_else(|| RegistryError::NotFound {
            name: name.to_string(),
        })?;
        Ok(entry.denied_operations().to_vec())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::adapters::driven::configuration::model::{McpServerEntry, StdioConfig};
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::GetDeniedOperations;

    #[test]
    fn get_denied_tools_returns_list() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec!["delete".to_string(), "exec".to_string()],
            }),
        );
        let store = FakeConfigStore::new(entries);

        let tools = GetDeniedOperations::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["delete", "exec"]);
    }

    #[test]
    fn get_denied_tools_empty_returns_empty() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);

        let tools = GetDeniedOperations::execute(&store, "s1").unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn get_denied_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());

        let result = GetDeniedOperations::execute(&store, "nope");
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn get_denied_tools_store_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = GetDeniedOperations::execute(&store, "s1");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
