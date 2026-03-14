use std::collections::BTreeMap;

/// Driven port: persistent storage for provider entries.
pub trait ProviderConfigStore: Send + Sync {
    type Entry: Send + Sync;
    fn load_entries(&self) -> Result<BTreeMap<String, Self::Entry>, String>;
    fn save_entries(&self, entries: BTreeMap<String, Self::Entry>) -> Result<(), String>;
}
