use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapters::driven::configuration::error::ConfigError;
use crate::adapters::driven::configuration::model::{GatewayConfig, McpServerEntry};
use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;

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

impl ProviderConfigStore for FileConfigStore {
    type Entry = McpServerEntry;

    fn load_entries(&self) -> Result<BTreeMap<String, McpServerEntry>, String> {
        let config = ConfigStore::load(self).map_err(|e| e.to_string())?;
        Ok(config.mcp_servers)
    }

    fn save_entries(&self, entries: BTreeMap<String, McpServerEntry>) -> Result<(), String> {
        let config = ConfigStore::load(self).map_err(|e| e.to_string())?;
        let config = GatewayConfig {
            mcp_servers: entries,
            ..config
        };
        ConfigStore::save(self, &config).map_err(|e| e.to_string())
    }
}
