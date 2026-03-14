use std::collections::BTreeMap;

use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

use crate::common::registry_helpers::*;

#[test]
fn remove_existing_server_succeeds() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    registry.remove_provider("s1").unwrap();

    let entries = registry.list_providers().unwrap();
    assert!(!entries.contains_key("s1"));
}

#[test]
fn remove_nonexistent_server_returns_not_found() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));

    let result = registry.remove_provider("nope");
    assert!(matches!(
        result,
        Err(RegistryError::NotFound { name }) if name == "nope"
    ));
}

#[test]
fn remove_with_store_load_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.remove_provider("test");
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

#[test]
fn remove_with_store_save_error_propagates() {
    let entries = BTreeMap::from([("test".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FailingStore {
        fail_load: false,
        entries,
    });
    let result = registry.remove_provider("test");
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}
