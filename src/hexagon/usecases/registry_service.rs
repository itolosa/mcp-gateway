use std::collections::BTreeMap;

use super::registry_error::RegistryError;
use crate::hexagon::ports::{ServerConfigStore, ServerEntry};

pub struct RegistryService<S: ServerConfigStore> {
    store: S,
}

impl<S: ServerConfigStore> RegistryService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn list_servers(&self) -> Result<BTreeMap<String, S::Entry>, RegistryError> {
        self.store.load_entries().map_err(RegistryError::Storage)
    }

    pub fn add_server(&self, name: String, entry: S::Entry) -> Result<(), RegistryError> {
        let mut entries = self.store.load_entries().map_err(RegistryError::Storage)?;

        if entries.contains_key(&name) {
            return Err(RegistryError::AlreadyExists { name });
        }

        entries.insert(name, entry);
        self.store
            .save_entries(entries)
            .map_err(RegistryError::Storage)
    }

    pub fn remove_server(&self, name: &str) -> Result<(), RegistryError> {
        let mut entries = self.store.load_entries().map_err(RegistryError::Storage)?;

        if entries.remove(name).is_none() {
            return Err(RegistryError::NotFound {
                name: name.to_string(),
            });
        }

        self.store
            .save_entries(entries)
            .map_err(RegistryError::Storage)
    }

    pub fn get_allowed_tools(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        let entries = self.store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries.get(name).ok_or_else(|| RegistryError::NotFound {
            name: name.to_string(),
        })?;
        Ok(entry.allowed_tools().to_vec())
    }

    pub fn add_allowed_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        let mut entries = self.store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::NotFound {
                name: name.to_string(),
            })?;
        let allowed = entry.allowed_tools_mut();
        for tool in tools {
            if !allowed.contains(tool) {
                allowed.push(tool.clone());
            }
        }
        self.store
            .save_entries(entries)
            .map_err(RegistryError::Storage)
    }

    pub fn remove_allowed_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        let mut entries = self.store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::NotFound {
                name: name.to_string(),
            })?;
        entry.allowed_tools_mut().retain(|t| !tools.contains(t));
        self.store
            .save_entries(entries)
            .map_err(RegistryError::Storage)
    }

    pub fn get_denied_tools(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        let entries = self.store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries.get(name).ok_or_else(|| RegistryError::NotFound {
            name: name.to_string(),
        })?;
        Ok(entry.denied_tools().to_vec())
    }

    pub fn add_denied_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        let mut entries = self.store.load_entries().map_err(RegistryError::Storage)?;
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
        self.store
            .save_entries(entries)
            .map_err(RegistryError::Storage)
    }

    pub fn remove_denied_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        let mut entries = self.store.load_entries().map_err(RegistryError::Storage)?;
        let entry = entries
            .get_mut(name)
            .ok_or_else(|| RegistryError::NotFound {
                name: name.to_string(),
            })?;
        entry.denied_tools_mut().retain(|t| !tools.contains(t));
        self.store
            .save_entries(entries)
            .map_err(RegistryError::Storage)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::model::{HttpConfig, McpServerEntry, StdioConfig};
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    struct FakeConfigStore {
        entries: Mutex<BTreeMap<String, McpServerEntry>>,
    }

    impl FakeConfigStore {
        fn new(entries: BTreeMap<String, McpServerEntry>) -> Self {
            Self {
                entries: Mutex::new(entries),
            }
        }
    }

    impl ServerConfigStore for FakeConfigStore {
        type Entry = McpServerEntry;

        fn load_entries(&self) -> Result<BTreeMap<String, McpServerEntry>, String> {
            Ok(self.entries.lock().unwrap().clone())
        }

        fn save_entries(&self, entries: BTreeMap<String, McpServerEntry>) -> Result<(), String> {
            *self.entries.lock().unwrap() = entries;
            Ok(())
        }
    }

    struct FailingStore {
        fail_load: bool,
        entries: BTreeMap<String, McpServerEntry>,
    }

    impl ServerConfigStore for FailingStore {
        type Entry = McpServerEntry;

        fn load_entries(&self) -> Result<BTreeMap<String, McpServerEntry>, String> {
            if self.fail_load {
                Err("denied".to_string())
            } else {
                Ok(self.entries.clone())
            }
        }

        fn save_entries(&self, _entries: BTreeMap<String, McpServerEntry>) -> Result<(), String> {
            Err("denied".to_string())
        }
    }

    fn stdio_entry() -> McpServerEntry {
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_tools: vec![],
            denied_tools: vec![],
        })
    }

    fn http_entry() -> McpServerEntry {
        McpServerEntry::Http(HttpConfig {
            url: "https://example.com".to_string(),
            headers: BTreeMap::new(),
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: None,
        })
    }

    #[test]
    fn list_empty_config_returns_empty() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.list_servers().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn list_populated_config_returns_all_servers() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        entries.insert("h1".to_string(), http_entry());
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        let result = service.list_servers().unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("s1"));
        assert!(result.contains_key("h1"));
    }

    #[test]
    fn list_with_store_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.list_servers();
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_to_empty_config_succeeds() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(result.is_ok());
    }

    #[test]
    fn add_persists_to_store() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        service
            .add_server("my-server".to_string(), http_entry())
            .unwrap();

        let entries = service.store.load_entries().unwrap();
        assert!(entries.contains_key("my-server"));
    }

    #[test]
    fn add_duplicate_name_returns_already_exists() {
        let mut entries = BTreeMap::new();
        entries.insert("existing".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        let result = service.add_server("existing".to_string(), http_entry());
        assert!(matches!(
            result,
            Err(RegistryError::AlreadyExists { name }) if name == "existing"
        ));
    }

    #[test]
    fn add_with_store_load_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_with_store_save_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            entries: BTreeMap::new(),
        });

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_existing_server_succeeds() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        service.remove_server("s1").unwrap();

        let entries = service.store().load_entries().unwrap();
        assert!(!entries.contains_key("s1"));
    }

    #[test]
    fn remove_nonexistent_server_returns_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.remove_server("nope");
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn remove_with_store_load_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.remove_server("test");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_with_store_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("test".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            entries,
        });

        let result = service.remove_server("test");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

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
        let service = RegistryService::new(store);

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn get_allowed_tools_empty_returns_empty() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        let tools = service.get_allowed_tools("s1").unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn get_allowed_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.get_allowed_tools("nope");
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn get_allowed_tools_store_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.get_allowed_tools("s1");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_allowed_tools_appends_new_tools() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        service
            .add_allowed_tools("s1", &["read".to_string(), "write".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
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
                allowed_tools: vec!["read".to_string()],
                denied_tools: vec![],
            }),
        );
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        service
            .add_allowed_tools("s1", &["read".to_string(), "write".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn add_allowed_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.add_allowed_tools("nope", &["read".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn add_allowed_tools_store_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.add_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_allowed_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            entries,
        });

        let result = service.add_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

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
        let service = RegistryService::new(store);

        service
            .remove_allowed_tools("s1", &["write".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
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
        let service = RegistryService::new(store);

        service
            .remove_allowed_tools("s1", &["nonexistent".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read"]);
    }

    #[test]
    fn remove_allowed_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.remove_allowed_tools("nope", &["read".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn remove_allowed_tools_store_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.remove_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_allowed_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            entries,
        });

        let result = service.remove_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn get_denied_tools_returns_list() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec![],
                denied_tools: vec!["delete".to_string(), "exec".to_string()],
            }),
        );
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        let tools = service.get_denied_tools("s1").unwrap();
        assert_eq!(tools, vec!["delete", "exec"]);
    }

    #[test]
    fn get_denied_tools_empty_returns_empty() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        let tools = service.get_denied_tools("s1").unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn get_denied_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.get_denied_tools("nope");
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn get_denied_tools_store_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.get_denied_tools("s1");
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_denied_tools_appends_new_tools() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        service
            .add_denied_tools("s1", &["delete".to_string(), "exec".to_string()])
            .unwrap();

        let tools = service.get_denied_tools("s1").unwrap();
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
        let service = RegistryService::new(store);

        service
            .add_denied_tools("s1", &["delete".to_string(), "exec".to_string()])
            .unwrap();

        let tools = service.get_denied_tools("s1").unwrap();
        assert_eq!(tools, vec!["delete", "exec"]);
    }

    #[test]
    fn add_denied_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.add_denied_tools("nope", &["delete".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn add_denied_tools_store_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.add_denied_tools("s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn add_denied_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            entries,
        });

        let result = service.add_denied_tools("s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_denied_tools_removes_specified() {
        let mut entries = BTreeMap::new();
        entries.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec![],
                denied_tools: vec![
                    "delete".to_string(),
                    "exec".to_string(),
                    "admin".to_string(),
                ],
            }),
        );
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        service
            .remove_denied_tools("s1", &["exec".to_string()])
            .unwrap();

        let tools = service.get_denied_tools("s1").unwrap();
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
                allowed_tools: vec![],
                denied_tools: vec!["delete".to_string()],
            }),
        );
        let store = FakeConfigStore::new(entries);
        let service = RegistryService::new(store);

        service
            .remove_denied_tools("s1", &["nonexistent".to_string()])
            .unwrap();

        let tools = service.get_denied_tools("s1").unwrap();
        assert_eq!(tools, vec!["delete"]);
    }

    #[test]
    fn remove_denied_tools_not_found() {
        let store = FakeConfigStore::new(BTreeMap::new());
        let service = RegistryService::new(store);

        let result = service.remove_denied_tools("nope", &["delete".to_string()]);
        assert!(matches!(
            result,
            Err(RegistryError::NotFound { name }) if name == "nope"
        ));
    }

    #[test]
    fn remove_denied_tools_store_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: true,
            entries: BTreeMap::new(),
        });

        let result = service.remove_denied_tools("s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn remove_denied_tools_save_error_propagates() {
        let mut entries = BTreeMap::new();
        entries.insert("s1".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            entries,
        });

        let result = service.remove_denied_tools("s1", &["delete".to_string()]);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }
}
