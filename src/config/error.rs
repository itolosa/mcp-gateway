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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn io_error_display_contains_path() {
        let err = ConfigError::Io {
            path: PathBuf::from("/tmp/test.json"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        let msg = err.to_string();
        assert!(msg.contains("/tmp/test.json"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn parse_error_display_contains_path() {
        let serde_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
        let err = ConfigError::Parse {
            path: PathBuf::from("/tmp/bad.json"),
            source: serde_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("/tmp/bad.json"));
    }
}
