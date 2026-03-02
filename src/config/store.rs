use std::path::{Path, PathBuf};

use crate::config::error::ConfigError;
use crate::config::model::GatewayConfig;

pub trait ConfigStore {
    fn load(&self) -> Result<GatewayConfig, ConfigError>;
    fn save(&self, config: &GatewayConfig) -> Result<(), ConfigError>;
}

pub struct FileConfigStore {
    path: PathBuf,
}

impl FileConfigStore {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }
}

fn ensure_parent_exists(path: &Path) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
    }
    Ok(())
}

impl ConfigStore for FileConfigStore {
    fn load(&self) -> Result<GatewayConfig, ConfigError> {
        let contents = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(GatewayConfig::default());
            }
            Err(e) => {
                return Err(ConfigError::Io {
                    path: self.path.clone(),
                    source: e,
                });
            }
        };

        serde_json::from_str(&contents).map_err(|e| ConfigError::Parse {
            path: self.path.clone(),
            source: e,
        })
    }

    fn save(&self, config: &GatewayConfig) -> Result<(), ConfigError> {
        // GatewayConfig serialization is infallible (only String/Vec/BTreeMap fields)
        let json = serde_json::to_string_pretty(config).unwrap_or_default();

        ensure_parent_exists(&self.path)?;

        std::fs::write(&self.path, json).map_err(|e| ConfigError::Io {
            path: self.path.clone(),
            source: e,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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

    #[test]
    fn ensure_parent_exists_with_bare_filename() {
        assert!(ensure_parent_exists(Path::new("file.json")).is_ok());
    }

    #[test]
    fn ensure_parent_exists_with_empty_path() {
        assert!(ensure_parent_exists(Path::new("")).is_ok());
    }

    #[test]
    fn load_then_save_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roundtrip.json");
        let store = FileConfigStore::new(&path);

        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            crate::config::model::McpServerEntry::Stdio(crate::config::model::StdioConfig {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                env: std::collections::BTreeMap::new(),
                allowed_tools: vec![],
            }),
        );

        store.save(&config).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded, config);
    }
}
