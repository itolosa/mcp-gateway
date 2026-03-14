use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::configuration::model::{McpServerEntry, StdioConfig};
use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

use crate::common::registry_helpers::*;

#[test]
fn remove_denied_tools_removes_specified() {
    let entries = BTreeMap::from([(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![
                "delete".to_string(),
                "exec".to_string(),
                "admin".to_string(),
            ],
        }),
    )]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    registry
        .remove_denied_operations("s1", &["exec".to_string()])
        .unwrap();

    let tools = registry.get_denied_operations("s1").unwrap();
    assert_eq!(tools, vec!["delete", "admin"]);
}

#[test]
fn remove_denied_tools_ignores_missing() {
    let entries = BTreeMap::from([(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec!["delete".to_string()],
        }),
    )]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    registry
        .remove_denied_operations("s1", &["nonexistent".to_string()])
        .unwrap();

    let tools = registry.get_denied_operations("s1").unwrap();
    assert_eq!(tools, vec!["delete"]);
}

#[test]
fn remove_denied_tools_not_found() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));

    let result = registry.remove_denied_operations("nope", &["delete".to_string()]);
    assert!(matches!(
        result,
        Err(RegistryError::NotFound { name }) if name == "nope"
    ));
}

#[test]
fn remove_denied_tools_store_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.remove_denied_operations("s1", &["delete".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

#[test]
fn remove_denied_tools_save_error_propagates() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FailingStore {
        fail_load: false,
        entries,
    });
    let result = registry.remove_denied_operations("s1", &["delete".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}
