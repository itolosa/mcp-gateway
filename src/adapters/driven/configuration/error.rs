use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config from {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse config from {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}
