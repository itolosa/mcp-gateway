use crate::hexagon::ports::{ServerConfigStore, ServerEntry};
use crate::hexagon::usecases::registry_error::RegistryError;

pub(crate) struct AddDeniedTools;

impl AddDeniedTools {
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
        let denied = entry.denied_tools_mut();
        for tool in tools {
            if !denied.contains(tool) {
                denied.push(tool.clone());
            }
        }
        store.save_entries(entries).map_err(RegistryError::Storage)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeMap;

    use crate::config::model::{McpServerEntry, StdioConfig};
    use crate::hexagon::usecases::get_denied_tools::GetDeniedTools;
    use crate::hexagon::usecases::registry_error::RegistryError;
    use crate::hexagon::usecases::registry_service::test_helpers::*;

    use super::AddDeniedTools;

    #[test]
    fn add_denied_tools_appends_new_tools() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);

        AddDeniedTools::execute(&store, "s1", &["delete".to_string(), "exec".to_string()]).unwrap();

        let tools = GetDeniedTools::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["delete", "exec"]);
    }

    #[test]
    fn add_denied_tools_skips_duplicates() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec![],
                denied_tools: vec!["delete".to_string()],
            }),
        );
        let store = FakeConfigStore::new(entries);

        AddDeniedTools::execute(&store, "s1", &["delete".to_string(), "exec".to_string()]).unwrap();

        let tools = GetDeniedTools::execute(&store, "s1").unwrap();
        assert_eq!(tools, vec!["delete", "exec"]);
    }

    #[test]
    fn add_denied_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());

        let result = AddDeniedTools::execute(&store, "nope", &["delete".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn add_denied_tools_store_error_propagates() {
        let store = FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        };
        let result = AddDeniedTools::execute(&store, "s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_denied_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FailingStore {
            fail_load: false,
            entries,
        };
        let result = AddDeniedTools::execute(&store, "s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
