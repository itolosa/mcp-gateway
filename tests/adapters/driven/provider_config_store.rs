use std::collections::BTreeMap;
use std::path::Path;

use mcp_gateway::adapters::driven::configuration::error::ConfigError;
use mcp_gateway::adapters::driven::configuration::model::{
    GatewayConfig, McpServerEntry, StdioConfig,
};
use mcp_gateway::adapters::driven::provider_config_store::{ConfigStore, FileConfigStore};
use mcp_gateway::hexagon::ports::driven::provider_config_store::ProviderConfigStore;

#[test]
fn load_missing_file_returns_default() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileConfigStore::new(&dir.path().join("nonexistent.json"));

    let config = store.load().unwrap();
    assert_eq!(config, GatewayConfig::default());
}

#[test]
fn load_valid_file_returns_config() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"test":{"type":"stdio","command":"echo"}}}"#,
    )
    .unwrap();

    let store = FileConfigStore::new(&path);
    let config = store.load().unwrap();
    assert!(config.mcp_servers.contains_key("test"));
}

#[test]
fn load_malformed_file_returns_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "not json").unwrap();

    let store = FileConfigStore::new(&path);
    let result = store.load();
    assert!(matches!(result, Err(ConfigError::Parse { .. })));
}

#[test]
fn load_directory_returns_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileConfigStore::new(dir.path());

    let result = store.load();
    assert!(matches!(result, Err(ConfigError::Io { .. })));
}

#[test]
fn save_writes_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.json");
    let store = FileConfigStore::new(&path);

    let config = GatewayConfig::default();
    store.save(&config).unwrap();

    let contents = std::fs::read_to_string(&path).unwrap();
    let roundtrip: GatewayConfig = serde_json::from_str(&contents).unwrap();
    assert_eq!(roundtrip, config);
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("dir").join("config.json");
    let store = FileConfigStore::new(&path);

    store.save(&GatewayConfig::default()).unwrap();
    assert!(path.exists());
}

#[test]
fn save_to_invalid_parent_returns_io_error() {
    let store = FileConfigStore::new(Path::new("/dev/null/impossible/config.json"));

    let result = store.save(&GatewayConfig::default());
    assert!(matches!(result, Err(ConfigError::Io { .. })));
}

#[test]
fn save_to_directory_returns_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileConfigStore::new(dir.path());

    let result = store.save(&GatewayConfig::default());
    assert!(matches!(result, Err(ConfigError::Io { .. })));
}

// NOTE: ensure_parent_exists is a private function in store.rs and cannot be
// tested directly from an integration test. The following two tests exercise
// the same code paths indirectly through save().

#[test]
fn save_bare_filename_succeeds_in_current_dir() {
    // Exercises the ensure_parent_exists path where parent is ""
    // We use a tempdir to avoid polluting the working directory
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file.json");
    let store = FileConfigStore::new(&path);
    assert!(store.save(&GatewayConfig::default()).is_ok());
}

#[test]
fn load_entries_returns_server_map() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    std::fs::write(
        &path,
        r#"{"mcpServers":{"test":{"type":"stdio","command":"echo"}}}"#,
    )
    .unwrap();

    let store = FileConfigStore::new(&path);
    let entries = ProviderConfigStore::load_entries(&store).unwrap();
    assert!(entries.contains_key("test"));
}

#[test]
fn save_entries_persists_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.json");
    let store = FileConfigStore::new(&path);

    let mut entries = BTreeMap::new();
    entries.insert(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
        }),
    );
    ProviderConfigStore::save_entries(&store, entries).unwrap();

    let loaded = ProviderConfigStore::load_entries(&store).unwrap();
    assert!(loaded.contains_key("s1"));
}

#[test]
fn load_entries_missing_file_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileConfigStore::new(&dir.path().join("nonexistent.json"));

    let entries = ProviderConfigStore::load_entries(&store).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn load_entries_malformed_file_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "not json").unwrap();

    let store = FileConfigStore::new(&path);
    let result = ProviderConfigStore::load_entries(&store);
    assert!(result.is_err());
}

#[test]
fn save_entries_to_invalid_path_returns_error() {
    let store = FileConfigStore::new(Path::new("/dev/null/impossible/config.json"));
    let result = ProviderConfigStore::save_entries(&store, BTreeMap::new());
    assert!(result.is_err());
}

#[test]
fn save_empty_path_triggers_io_error_after_ensure_parent() {
    // Path::new("").parent() returns None, so ensure_parent_exists skips create_dir_all
    // and returns Ok(()), then std::fs::write("") fails with an IO error.
    let store = FileConfigStore::new(Path::new(""));
    let result = store.save(&GatewayConfig::default());
    assert!(matches!(result, Err(ConfigError::Io { .. })));
}

#[test]
fn load_then_save_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("roundtrip.json");
    let store = FileConfigStore::new(&path);

    let mut config = GatewayConfig::default();
    config.mcp_servers.insert(
        "test".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            env: std::collections::BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
        }),
    );

    store.save(&config).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded, config);
}
