use std::fmt;

#[derive(Debug)]
pub enum RegistryError {
    AlreadyExists { name: String },
    NotFound { name: String },
    Storage(String),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyExists { name } => write!(f, "provider '{name}' already exists"),
            Self::NotFound { name } => write!(f, "provider '{name}' not found"),
            Self::Storage(msg) => write!(f, "storage error: {msg}"),
        }
    }
}

impl std::error::Error for RegistryError {}

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
