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

    pub fn add_server(&self, name: String, entry: McpServerEntry) -> Result<(), RegistryError> {
        let mut config = self.store.load()?;

        if config.mcp_servers.contains_key(&name) {
            return Err(RegistryError::AlreadyExists { name });
        }

        config.mcp_servers.insert(name, entry);
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
    }

    impl ConfigStore for FailingStore {
        fn load(&self) -> Result<GatewayConfig, ConfigError> {
            if self.fail_load {
                Err(io_error())
            } else {
                Ok(GatewayConfig::default())
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
        })
    }

    fn http_entry() -> McpServerEntry {
        McpServerEntry::Http(HttpConfig {
            url: "https://example.com".to_string(),
            headers: BTreeMap::new(),
        })
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
        let service = RegistryService::new(FailingStore { fail_load: true });

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn add_with_store_save_error_propagates() {
        let service = RegistryService::new(FailingStore { fail_load: false });

        let result = service.add_server("test".to_string(), stdio_entry());
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }
}
