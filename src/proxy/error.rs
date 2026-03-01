use crate::config::error::ConfigError;

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("server '{name}' not found in config")]
    ServerNotFound { name: String },

    #[error(transparent)]
    Config(#[from] ConfigError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_not_found_display() {
        let err = ProxyError::ServerNotFound {
            name: "test".to_string(),
        };
        assert!(err.to_string().contains("test"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn config_error_converts() {
        let config_err = ConfigError::Io {
            path: std::path::PathBuf::from("/tmp/test"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };
        let proxy_err = ProxyError::from(config_err);
        assert!(matches!(proxy_err, ProxyError::Config(_)));
        assert!(proxy_err.to_string().contains("/tmp/test"));
    }
}
