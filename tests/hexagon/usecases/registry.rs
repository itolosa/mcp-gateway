use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::configuration::model::{McpServerEntry, StdioConfig};
use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

use crate::common::registry_helpers::*;

// ---------------------------------------------------------------------------
// RegistryError display tests
// ---------------------------------------------------------------------------

#[test]
fn already_exists_display() {
    let err = RegistryError::AlreadyExists {
        name: "test".to_string(),
    };
    assert!(err.to_string().contains("test"));
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn not_found_display() {
    let err = RegistryError::NotFound {
        name: "test".to_string(),
    };
    assert!(err.to_string().contains("test"));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn storage_error_display() {
    let err = RegistryError::Storage("disk full".to_string());
    assert!(err.to_string().contains("disk full"));
    assert!(err.to_string().contains("storage error"));
}

// ---------------------------------------------------------------------------
// add_provider tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// remove_provider tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// list_providers tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// get_allowed_operations tests
// ---------------------------------------------------------------------------

#[test]
fn get_allowed_tools_returns_list() {
    let entries = BTreeMap::from([(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec!["read".to_string(), "write".to_string()],
            denied_operations: vec![],
        }),
    )]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    let tools = registry.get_allowed_operations("s1").unwrap();
    assert_eq!(tools, vec!["read", "write"]);
}

#[test]
fn get_allowed_tools_empty_returns_empty() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    let tools = registry.get_allowed_operations("s1").unwrap();
    assert!(tools.is_empty());
}

#[test]
fn get_allowed_tools_not_found() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));

    let result = registry.get_allowed_operations("nope");
    assert!(matches!(
        result,
        Err(RegistryError::NotFound { name }) if name == "nope"
    ));
}

#[test]
fn get_allowed_tools_store_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.get_allowed_operations("s1");
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

// ---------------------------------------------------------------------------
// add_allowed_operations tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// remove_allowed_operations tests
// ---------------------------------------------------------------------------

#[test]
fn remove_allowed_tools_removes_specified() {
    let entries = BTreeMap::from([(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec![
                "read".to_string(),
                "write".to_string(),
                "delete".to_string(),
            ],
            denied_operations: vec![],
        }),
    )]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    registry
        .remove_allowed_operations("s1", &["write".to_string()])
        .unwrap();

    let tools = registry.get_allowed_operations("s1").unwrap();
    assert_eq!(tools, vec!["read", "delete"]);
}

#[test]
fn remove_allowed_tools_ignores_missing() {
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
        .remove_allowed_operations("s1", &["nonexistent".to_string()])
        .unwrap();

    let tools = registry.get_allowed_operations("s1").unwrap();
    assert_eq!(tools, vec!["read"]);
}

#[test]
fn remove_allowed_tools_not_found() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));

    let result = registry.remove_allowed_operations("nope", &["read".to_string()]);
    assert!(matches!(
        result,
        Err(RegistryError::NotFound { name }) if name == "nope"
    ));
}

#[test]
fn remove_allowed_tools_store_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.remove_allowed_operations("s1", &["read".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

#[test]
fn remove_allowed_tools_save_error_propagates() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FailingStore {
        fail_load: false,
        entries,
    });
    let result = registry.remove_allowed_operations("s1", &["read".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

// ---------------------------------------------------------------------------
// get_denied_operations tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// add_denied_operations tests
// ---------------------------------------------------------------------------

#[test]
fn add_denied_tools_appends_new_tools() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FakeConfigStore::new(entries));

    registry
        .add_denied_operations("s1", &["delete".to_string(), "exec".to_string()])
        .unwrap();

    let tools = registry.get_denied_operations("s1").unwrap();
    assert_eq!(tools, vec!["delete", "exec"]);
}

#[test]
fn add_denied_tools_skips_duplicates() {
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
        .add_denied_operations("s1", &["delete".to_string(), "exec".to_string()])
        .unwrap();

    let tools = registry.get_denied_operations("s1").unwrap();
    assert_eq!(tools, vec!["delete", "exec"]);
}

#[test]
fn add_denied_tools_not_found() {
    let registry = RegistryService::new(FakeConfigStore::new(BTreeMap::new()));

    let result = registry.add_denied_operations("nope", &["delete".to_string()]);
    assert!(matches!(
        result,
        Err(RegistryError::NotFound { name }) if name == "nope"
    ));
}

#[test]
fn add_denied_tools_store_error_propagates() {
    let registry = RegistryService::new(FailingStore {
        fail_load: true,
        entries: BTreeMap::new(),
    });
    let result = registry.add_denied_operations("s1", &["delete".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

#[test]
fn add_denied_tools_save_error_propagates() {
    let entries = BTreeMap::from([("s1".to_string(), stdio_entry())]);
    let registry = RegistryService::new(FailingStore {
        fail_load: false,
        entries,
    });
    let result = registry.add_denied_operations("s1", &["delete".to_string()]);
    assert!(matches!(result, Err(RegistryError::Storage(_))));
}

// ---------------------------------------------------------------------------
// remove_denied_operations tests
// ---------------------------------------------------------------------------

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
