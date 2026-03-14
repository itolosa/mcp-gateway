use std::collections::BTreeMap;
use std::sync::Mutex;

use mcp_gateway::adapters::driven::configuration::model::{
    HttpConfig, McpServerEntry, StdioConfig,
};
use mcp_gateway::hexagon::ports::ProviderConfigStore;

pub struct FakeConfigStore {
    entries: Mutex<BTreeMap<String, McpServerEntry>>,
}

impl FakeConfigStore {
    pub fn new(entries: BTreeMap<String, McpServerEntry>) -> Self {
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

pub struct FailingStore {
    pub fail_load: bool,
    pub entries: BTreeMap<String, McpServerEntry>,
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

pub fn stdio_entry() -> McpServerEntry {
    McpServerEntry::Stdio(StdioConfig {
        command: "echo".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
    })
}

pub fn http_entry() -> McpServerEntry {
    McpServerEntry::Http(HttpConfig {
        url: "https://example.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    })
}
