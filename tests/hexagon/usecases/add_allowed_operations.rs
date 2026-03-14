use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::configuration::model::{McpServerEntry, StdioConfig};
use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

use crate::common::registry_helpers::*;

#[test]
fn add_allowed_tools_appends_new_tools() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    registry
        .add_allowed_operations("s1", &["read".to_string(), "write".to_string()])
        .unwrap();

    let tools = registry.get_allowed_operations("s1").unwrap();
    assert_eq!(tools, vec!["read", "write"]);
}

#[test]
fn add_allowed_tools_skips_duplicates() {
    let entries = BTreeMap::from([(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec!["read".to_string()],
            denied_operations: vec![],
        }),
    )]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    registry
        .add_allowed_operations("s1", &["read".to_string(), "write".to_string()])
        .unwrap();

    let tools = registry.get_allowed_operations("s1").unwrap();
    assert_eq!(tools, vec!["read", "write"]);
}

#[test]
fn add_allowed_tools_not_found() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));

    let result = registry.add_allowed_operations("nope", &["read".to_string()]);
    assert!(matches!(
        result,
        Err(RegistryError::NotFound { name }) if name == "nope"
    ));
}

#[test]
fn add_allowed_tools_store_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.add_allowed_operations("s1", &["read".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

#[test]
fn add_allowed_tools_save_error_propagates() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FailingStore {
        fail_load: false,
        entries,
    });
    let result = registry.add_allowed_operations("s1", &["read".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}
