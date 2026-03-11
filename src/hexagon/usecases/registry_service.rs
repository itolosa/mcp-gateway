use std::collections::BTreeMap;

use crate::hexagon::ports::ProviderConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

use super::add_allowed_operations::AddAllowedOperations;
use super::add_denied_operations::AddDeniedOperations;
use super::add_provider::AddProvider;
use super::get_allowed_operations::GetAllowedOperations;
use super::get_denied_operations::GetDeniedOperations;
use super::list_providers::ListProviders;
use super::remove_allowed_operations::RemoveAllowedOperations;
use super::remove_denied_operations::RemoveDeniedOperations;
use super::remove_provider::RemoveProvider;

pub struct RegistryService<S: ProviderConfigStore> {
    store: S,
}

impl<S: ProviderConfigStore> RegistryService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn list_providers(&self) -> Result<BTreeMap<String, S::Entry>, RegistryError> {
        ListProviders::execute(&self.store)
    }

    pub fn add_provider(&self, name: String, entry: S::Entry) -> Result<(), RegistryError> {
        AddProvider::execute(&self.store, name, entry)
    }

    pub fn remove_provider(&self, name: &str) -> Result<(), RegistryError> {
        RemoveProvider::execute(&self.store, name)
    }

    pub fn get_allowed_operations(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        GetAllowedOperations::execute(&self.store, name)
    }

    pub fn add_allowed_operations(
        &self,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError> {
        AddAllowedOperations::execute(&self.store, name, tools)
    }

    pub fn remove_allowed_operations(
        &self,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError> {
        RemoveAllowedOperations::execute(&self.store, name, tools)
    }

    pub fn get_denied_operations(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        GetDeniedOperations::execute(&self.store, name)
    }

    pub fn add_denied_operations(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        AddDeniedOperations::execute(&self.store, name, tools)
    }

    pub fn remove_denied_operations(
        &self,
        name: &str,
        tools: &[String],
    ) -> Result<(), RegistryError> {
        RemoveDeniedOperations::execute(&self.store, name, tools)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_helpers {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use crate::adapters::driven::configuration::model::{HttpConfig, McpServerEntry, StdioConfig};
    use crate::hexagon::ports::ProviderConfigStore;

    pub(crate) struct FakeConfigStore {
        entries: Mutex<BTreeMap<String, McpServerEntry>>,
    }

    impl FakeConfigStore {
        pub(crate) fn new(entries: BTreeMap<String, McpServerEntry>) -> Self {
            Self {
                entries: Mutex::new(entries),
            }
        }
    }

    impl ProviderConfigStore for FakeConfigStore {
        type Entry = McpServerEntry;

        fn load_entries(&self) -> Result<BTreeMap<String, McpServerEntry>, String> {
            Ok(self.entries.lock().unwrap().clone())
        }

        fn save_entries(&self, entries: BTreeMap<String, McpServerEntry>) -> Result<(), String> {
            *self.entries.lock().unwrap() = entries;
            Ok(())
        }
    }

    pub(crate) struct FailingStore {
        pub(crate) fail_load: bool,
        pub(crate) entries: BTreeMap<String, McpServerEntry>,
    }

    impl ProviderConfigStore for FailingStore {
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

    pub(crate) fn stdio_entry() -> McpServerEntry {
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
        })
    }

    pub(crate) fn http_entry() -> McpServerEntry {
        McpServerEntry::Http(HttpConfig {
            url: "https://example.com".to_string(),
            headers: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
            auth: None,
        })
    }
}
