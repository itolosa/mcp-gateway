use crate::hexagon::ports::{ServerConfigStore, ServerEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct RemoveAllowedTools;

impl RemoveAllowedTools {
    pub(crate) fn execute<S: ServerConfigStore>(
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
        entry.allowed_tools_mut().retain(|t| !tools.contains(t));
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::config::model::{McpServerEntry, StdioConfig};
    use crate::hexagon::usecases::get_allowed_tools::GetAllowedTools;
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::RemoveAllowedTools;

    #[test]
    fn remove_allowed_tools_removes_specified() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec![
                    "read".to_string(),
                    "write".to_string(),
                    "delete".to_string(),
                ],
                denied_tools: vec![],
            }),
        );
        let store = FakeConfigStore::new(entries);

        RemoveAllowedTools::execute(&store, "s1", &["write".to_string()]).unwrap();

        let tools = GetAllowedTools::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["read", "delete"]);
    }

    #[test]
    fn remove_allowed_tools_ignores_missing() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec!["read".to_string()],
                denied_tools: vec![],
            }),
        );
        let store = FakeConfigStore::new(entries);

        RemoveAllowedTools::execute(&store, "s1", &["nonexistent".to_string()]).unwrap();

        let tools = GetAllowedTools::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["read"]);
    }

    #[test]
    fn remove_allowed_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());

        let result = RemoveAllowedTools::execute(&store, "nope", &["read".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn remove_allowed_tools_store_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = RemoveAllowedTools::execute(&store, "s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_allowed_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FailingStore {
            fail_load: false,
            entries,
        };
        let result = RemoveAllowedTools::execute(&store, "s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
