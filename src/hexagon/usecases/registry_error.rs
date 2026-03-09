use crate::config::error::ConfigError;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("server '{name}' already exists")]
    AlreadyExists { name: String },

    #[error("server '{name}' not found")]
    NotFound { name: String },

    #[error(transparent)]
    Config(#[from] ConfigError),
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn config_error_converts() {
        let config_err = ConfigError::Io {
            path: std::path::PathBuf::from("/tmp/test"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };
        let registry_err = RegistryError::from(config_err);
        assert!(matches!(registry_err, RegistryError::Config(_)));
        assert!(registry_err.to_string().contains("/tmp/test"));
    }
}
