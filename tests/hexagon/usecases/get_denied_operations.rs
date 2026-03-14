use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::configuration::model::{McpServerEntry, StdioConfig};
use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

use crate::common::registry_helpers::*;

#[test]
fn get_denied_tools_returns_list() {
    let entries = BTreeMap::from([(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec!["delete".to_string(), "exec".to_string()],
        }),
    )]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    let tools = registry.get_denied_operations("s1").unwrap();
    assert_eq!(tools, vec!["delete", "exec"]);
}

#[test]
fn get_denied_tools_empty_returns_empty() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    let tools = registry.get_denied_operations("s1").unwrap();
    assert!(tools.is_empty());
}

#[test]
fn get_denied_tools_not_found() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));

    let result = registry.get_denied_operations("nope");
    assert!(matches!(
        result,
        Err(RegistryError::NotFound { name }) if name == "nope"
    ));
}

#[test]
fn get_denied_tools_store_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.get_denied_operations("s1");
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}
