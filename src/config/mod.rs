pub mod error;
pub mod model;
pub mod store;

use std::path::PathBuf;

pub fn default_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".mcp-gateway.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_path_ends_with_expected_filename() {
        let path = default_config_path();
        assert!(path.is_some());
        let path = path.unwrap_or_default();
        assert!(path.ends_with(".mcp-gateway.json"));
    }
}
