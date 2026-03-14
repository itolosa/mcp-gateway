use std::collections::BTreeMap;

use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

use crate::common::registry_helpers::*;

#[test]
fn add_to_empty_config_succeeds() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));
    let result = registry.add_provider("test".to_string(), stdio_entry());
    assert!(result.is_ok());
}

#[test]
fn add_persists_to_store() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));
    registry
        .add_provider("my-server".to_string(), http_entry())
        .unwrap();

    let entries = registry.list_providers().unwrap();
    assert!(entries.contains_key("my-server"));
}

#[test]
fn add_duplicate_name_returns_already_exists() {
    let entries = BTreeMap::from([("existing".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    let result = registry.add_provider("existing".to_string(), http_entry());
    assert!(matches!(
        result,
        Err(RegistryError::AlreadyExists { name }) if name == "existing"
    ));
}

#[test]
fn add_with_store_load_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.add_provider("test".to_string(), stdio_entry());
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

#[test]
fn add_with_store_save_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: false,
        entries: BTreeMap::new(),
    });
    let result = registry.add_provider("test".to_string(), stdio_entry());
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}
