use std::collections::BTreeMap;

use crate::hexagon::ports::ServerConfigStore;
use crate::hexagon::usecases::registry_error::RegistryError;

use super::add_allowed_tools::AddAllowedTools;
use super::add_denied_tools::AddDeniedTools;
use super::add_server::AddServer;
use super::get_allowed_tools::GetAllowedTools;
use super::get_denied_tools::GetDeniedTools;
use super::list_servers::ListServers;
use super::remove_allowed_tools::RemoveAllowedTools;
use super::remove_denied_tools::RemoveDeniedTools;
use super::remove_server::RemoveServer;

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
        ListServers::execute(&self.store)
    }

    pub fn add_server(&self, name: String, entry: S::Entry) -> Result<(), RegistryError> {
        AddServer::execute(&self.store, name, entry)
    }

    pub fn remove_server(&self, name: &str) -> Result<(), RegistryError> {
        RemoveServer::execute(&self.store, name)
    }

    pub fn get_allowed_tools(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        GetAllowedTools::execute(&self.store, name)
    }

    pub fn add_allowed_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        AddAllowedTools::execute(&self.store, name, tools)
    }

    pub fn remove_allowed_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        RemoveAllowedTools::execute(&self.store, name, tools)
    }

    pub fn get_denied_tools(&self, name: &str) -> Result<Vec<String>, RegistryError> {
        GetDeniedTools::execute(&self.store, name)
    }

    pub fn add_denied_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        AddDeniedTools::execute(&self.store, name, tools)
    }

    pub fn remove_denied_tools(&self, name: &str, tools: &[String]) -> Result<(), RegistryError> {
        RemoveDeniedTools::execute(&self.store, name, tools)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_helpers {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use crate::adapters::driven::configuration::model::{HttpConfig, McpServerEntry, StdioConfig};
    use crate::hexagon::ports::ServerConfigStore;

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

    pub(crate) struct FailingStore {
        pub(crate) fail_load: bool,
        pub(crate) entries: BTreeMap<String, McpServerEntry>,
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

    pub(crate) fn stdio_entry() -> McpServerEntry {
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_tools: vec![],
            denied_tools: vec![],
        })
    }

    pub(crate) fn http_entry() -> McpServerEntry {
        McpServerEntry::Http(HttpConfig {
            url: "https://example.com".to_string(),
            headers: BTreeMap::new(),
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: None,
        })
    }
}
