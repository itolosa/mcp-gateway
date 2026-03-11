use crate::hexagon::ports::{ProviderConfigStore, ProviderEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct AddAllowedOperations;

impl AddAllowedOperations {
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
        let allowed = entry.allowed_operations_mut();
        for tool in tools {
            if !allowed.contains(tool) {
                allowed.push(tool.clone());
            }
        }
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::adapters::driven::configuration::model::{McpServerEntry, StdioConfig};
    use crate::hexagon::usecases::get_allowed_operations::GetAllowedOperations;
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::AddAllowedOperations;

    #[test]
    fn add_allowed_tools_appends_new_tools() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);

        AddAllowedOperations::execute(&store, "s1", &["read".to_string(), "write".to_string()])
            .unwrap();

        let tools = GetAllowedOperations::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn add_allowed_tools_skips_duplicates() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec!["read".to_string()],
                denied_operations: vec![],
            }),
        );
        let store = FakeConfigStore::new(entries);

        AddAllowedOperations::execute(&store, "s1", &["read".to_string(), "write".to_string()])
            .unwrap();

        let tools = GetAllowedOperations::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn add_allowed_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());

        let result = AddAllowedOperations::execute(&store, "nope", &["read".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn add_allowed_tools_store_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = AddAllowedOperations::execute(&store, "s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_allowed_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FailingStore {
            fail_load: false,
            entries,
        };
        let result = AddAllowedOperations::execute(&store, "s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
