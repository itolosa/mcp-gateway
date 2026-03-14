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
