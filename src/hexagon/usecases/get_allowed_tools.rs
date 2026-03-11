use crate::hexagon::ports::{ServerConfigStore, ServerEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct GetAllowedTools;

impl GetAllowedTools {
    pub(crate) fn execute<S: ServerConfigStore>(
        store: &S,
        name: &str,
    ) -> Result<Vec<String>, RegistryError> {
        let entries = store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries.get(name).ok_or_else(|| RegistryError::NotFound {
            name: name.to_string(),
        })?;
        Ok(entry.allowed_tools().to_vec())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::config::model::{McpServerEntry, StdioConfig};
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::GetAllowedTools;

    #[test]
    fn get_allowed_tools_returns_list() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec!["read".to_string(), "write".to_string()],
                denied_tools: vec![],
            }),
        );
        let store = FakeConfigStore::new(entries);

        let tools = GetAllowedTools::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn get_allowed_tools_empty_returns_empty() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);

        let tools = GetAllowedTools::execute(&store, "s1").unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn get_allowed_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());

        let result = GetAllowedTools::execute(&store, "nope");
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn get_allowed_tools_store_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = GetAllowedTools::execute(&store, "s1");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
