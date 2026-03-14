use crate::adapters::driven::configuration::model::McpServerEntry;
use crate::hexagon::ports::driven::provider_entry::ProviderEntry;

impl ProviderEntry for McpServerEntry {
    fn allowed_operations(&self) -> &[String] {
        McpServerEntry::allowed_operations(self)
    }

    fn allowed_operations_mut(&mut self) -> &mut Vec<String> {
        McpServerEntry::allowed_operations_mut(self)
    }

    fn denied_operations(&self) -> &[String] {
        McpServerEntry::denied_operations(self)
    }

    fn denied_operations_mut(&mut self) -> &mut Vec<String> {
        McpServerEntry::denied_operations_mut(self)
    }
}
