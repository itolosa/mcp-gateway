#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("server '{name}' already exists")]
    AlreadyExists { name: String },

    #[error("server '{name}' not found")]
    NotFound { name: String },

    #[error("storage error: {0}")]
    Storage(String),
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
    fn storage_error_display() {
        let err = RegistryError::Storage("disk full".to_string());
        assert!(err.to_string().contains("disk full"));
        assert!(err.to_string().contains("storage error"));
    }
}
