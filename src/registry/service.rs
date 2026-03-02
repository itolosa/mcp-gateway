use std::collections::BTreeMap;

use crate::config::model::McpServerEntry;
use crate::config::store::ConfigStore;
use crate::registry::error::RegistryError;

pub struct RegistryService<S: ConfigStore> {
    store: S,
}

impl<S: ConfigStore> RegistryService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn list_servers(&self) -> Result<BTreeMap<String, McpServerEntry>, RegistryError> {
        let config = self.store.load()?;
        Ok(config.mcp_servers)
    }

    pub fn add_server(&self, name: String, entry: McpServerEntry) -> Result<(), RegistryError> {
        let mut config = self.store.load()?;

        if config.mcp_servers.contains_key(&name) {
            return Err(RegistryError::AlreadyExists { name });
        }

        config.mcp_servers.insert(name, entry);
        self.store.save(&config)?;
        Ok(())
    }

    pub fn remove_server(&self, name: &str) -> Result<(), RegistryError> {
        let mut config = self.store.load()?;

        if config.mcp_servers.remove(name).is_none() {
            return Err(RegistryError::NotFound {
                name: name.to_string(),
            });
        }

        self.store.save(&config)?;
        Ok(())
    }

    pub fn get_allowed_tools(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        let config = self.store.load()?;
        let entry = config
            .mcp_servers
            .get(name)
            .ok_or_else(|| RegistryError::NotFound {
                name: name.to_string(),
            })?;
        Ok(entry.allowed_tools().to_vec())
    }

    pub fn add_allowed_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        let mut config = self.store.load()?;
        let entry = config
            .mcp_servers
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
        self.store.save(&config)?;
        Ok(())
    }

    pub fn remove_allowed_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        let mut config = self.store.load()?;
        let entry = config
            .mcp_servers
            .get_mut(name)
            .ok_or_else(|| RegistryError::NotFound {
                name: name.to_string(),
            })?;
        entry.allowed_tools_mut().retain(|t| !tools.contains(t));
        self.store.save(&config)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::error::ConfigError;
    use crate::config::model::{GatewayConfig, HttpConfig, StdioConfig};
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    struct FakeConfigStore {
        config: RefCell<GatewayConfig>,
    }

    impl FakeConfigStore {
        fn new(config: GatewayConfig) -> Self {
            Self {
                config: RefCell::new(config),
            }
        }
    }

    impl ConfigStore for FakeConfigStore {
        fn load(&self) -> Result<GatewayConfig, ConfigError> {
            Ok(self.config.borrow().clone())
        }

        fn save(&self, config: &GatewayConfig) -> Result<(), ConfigError> {
            *self.config.borrow_mut() = config.clone();
            Ok(())
        }
    }

    fn io_error() -> ConfigError {
        ConfigError::Io {
            path: std::path::PathBuf::from("/fail"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        }
    }

    struct FailingStore {
        fail_load: bool,
        config: GatewayConfig,
    }

    impl ConfigStore for FailingStore {
        fn load(&self) -> Result<GatewayConfig, ConfigError> {
            if self.fail_load {
                Err(io_error())
            } else {
                Ok(self.config.clone())
            }
        }

        fn save(&self, _config: &GatewayConfig) -> Result<(), ConfigError> {
            Err(io_error())
        }
    }

    fn stdio_entry() -> McpServerEntry {
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_tools: vec![],
        })
    }

    fn http_entry() -> McpServerEntry {
        McpServerEntry::Http(HttpConfig {
            url: "https://example.com".to_string(),
            headers: BTreeMap::new(),
            allowed_tools: vec![],
        })
    }

    #[test]
    fn list_empty_config_returns_empty() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let result = service.list_servers().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn list_populated_config_returns_all_servers() {
        let mut initial = GatewayConfig::default();
        initial.mcp_servers.insert("s1".to_string(), stdio_entry());
        initial.mcp_servers.insert("h1".to_string(), http_entry());
        let store = FakeConfigStore::new(initial);
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
            config: GatewayConfig::default(),
        });

        let result = service.list_servers();
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn add_to_empty_config_succeeds() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(result.is_ok());
    }

    #[test]
    fn add_persists_to_store() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        service
            .add_server("my-server".to_string(), http_entry())
            .unwrap();

        let config = service.store.load().unwrap();
        assert!(config.mcp_servers.contains_key("my-server"));
    }

    #[test]
    fn add_duplicate_name_returns_already_exists() {
        let mut initial = GatewayConfig::default();
        initial
            .mcp_servers
            .insert("existing".to_string(), stdio_entry());
        let store = FakeConfigStore::new(initial);
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
            config: GatewayConfig::default(),
        });

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn add_with_store_save_error_propagates() {
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            config: GatewayConfig::default(),
        });

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn remove_existing_server_succeeds() {
        let mut initial = GatewayConfig::default();
        initial.mcp_servers.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(initial);
        let service = RegistryService::new(store);

        service.remove_server("s1").unwrap();

        let config = service.store().load().unwrap();
        assert!(!config.mcp_servers.contains_key("s1"));
    }

    #[test]
    fn remove_nonexistent_server_returns_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
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
            config: GatewayConfig::default(),
        });

        let result = service.remove_server("test");
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn remove_with_store_save_error_propagates() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert("test".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            config,
        });

        let result = service.remove_server("test");
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn get_allowed_tools_returns_list() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec!["read".to_string(), "write".to_string()],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn get_allowed_tools_empty_returns_empty() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let tools = service.get_allowed_tools("s1").unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn get_allowed_tools_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
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
            config: GatewayConfig::default(),
        });

        let result = service.get_allowed_tools("s1");
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn add_allowed_tools_appends_new_tools() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert("s1".to_string(), stdio_entry());
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        service
            .add_allowed_tools("s1", &["read".to_string(), "write".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn add_allowed_tools_skips_duplicates() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec!["read".to_string()],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        service
            .add_allowed_tools("s1", &["read".to_string(), "write".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read", "write"]);
    }

    #[test]
    fn add_allowed_tools_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
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
            config: GatewayConfig::default(),
        });

        let result = service.add_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn add_allowed_tools_save_error_propagates() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert("s1".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            config,
        });

        let result = service.add_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn remove_allowed_tools_removes_specified() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
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
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        service
            .remove_allowed_tools("s1", &["write".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read", "delete"]);
    }

    #[test]
    fn remove_allowed_tools_ignores_missing() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "s1".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_tools: vec!["read".to_string()],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        service
            .remove_allowed_tools("s1", &["nonexistent".to_string()])
            .unwrap();

        let tools = service.get_allowed_tools("s1").unwrap();
        assert_eq!(tools, vec!["read"]);
    }

    #[test]
    fn remove_allowed_tools_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
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
            config: GatewayConfig::default(),
        });

        let result = service.remove_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn remove_allowed_tools_save_error_propagates() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert("s1".to_string(), stdio_entry());
        let service = RegistryService::new(FailingStore {
            fail_load: false,
            config,
        });

        let result = service.remove_allowed_tools("s1", &["read".to_string()]);
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }
}
