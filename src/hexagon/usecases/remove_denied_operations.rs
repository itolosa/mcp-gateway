use crate::hexagon::ports::{ProviderConfigStore, ProviderEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct RemoveDeniedOperations;

impl RemoveDeniedOperations {
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
        entry.denied_operations_mut().retain(|t| !tools.contains(t));
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::adapters::driven::configuration::model::{McpServerEntry, StdioConfig};
    use crate::hexagon::usecases::get_denied_operations::GetDeniedOperations;
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::RemoveDeniedOperations;

    #[test]
    fn remove_denied_tools_removes_specified() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec![
                    "delete".to_string(),
                    "exec".to_string(),
                    "admin".to_string(),
                ],
            }),
        );
        let store = FakeConfigStore::new(entries);

        RemoveDeniedOperations::execute(&store, "s1", &["exec".to_string()]).unwrap();

        let tools = GetDeniedOperations::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["delete", "admin"]);
    }

    #[test]
    fn remove_denied_tools_ignores_missing() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec!["delete".to_string()],
            }),
        );
        let store = FakeConfigStore::new(entries);

        RemoveDeniedOperations::execute(&store, "s1", &["nonexistent".to_string()]).unwrap();

        let tools = GetDeniedOperations::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["delete"]);
    }

    #[test]
    fn remove_denied_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());

        let result = RemoveDeniedOperations::execute(&store, "nope", &["delete".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn remove_denied_tools_store_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = RemoveDeniedOperations::execute(&store, "s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_denied_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FailingStore {
            fail_load: false,
            entries,
        };
        let result = RemoveDeniedOperations::execute(&store, "s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
