use std::collections::BTreeMap;

use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

use crate::common::registry_helpers::*;

#[test]
fn list_empty_config_returns_empty() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));
    let result = registry.list_providers().unwrap();
    assert!(result.is_empty());
}

#[test]
fn list_populated_config_returns_all_servers() {
    let entries = BTreeMap::from([
        ("s1".to_string(), stdio_entry()),
        ("h1".to_string(), http_entry()),
    ]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    let result = registry.list_providers().unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("s1"));
    assert!(result.contains_key("h1"));
}

#[test]
fn list_with_store_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.list_providers();
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}
