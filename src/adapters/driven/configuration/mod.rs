pub mod error;
pub mod model;

use std::path::PathBuf;

pub fn default_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".mcp-gateway.json"))
}
