use std::path::PathBuf;

use async_trait::async_trait;
use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};

pub struct FileCredentialStore {
    path: PathBuf,
}

impl FileCredentialStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path(server_name: &str) -> Option<PathBuf> {
        dirs::home_dir().map(|home| {
            home.join(".mcp-gateway")
                .join("credentials")
                .join(format!("{server_name}.json"))
        })
    }
}

#[async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(contents) => {
                let creds: StoredCredentials = serde_json::from_str(&contents)
                    .map_err(|e| AuthError::InternalError(format!("parse credentials: {e}")))?;
                Ok(Some(creds))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(AuthError::InternalError(format!("read credentials: {e}"))),
        }
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| AuthError::InternalError(format!("create credentials dir: {e}")))?;
        }
        let json = serde_json::to_string_pretty(&credentials).unwrap_or_default();
        tokio::fs::write(&self.path, json)
            .await
            .map_err(|e| AuthError::InternalError(format!("write credentials: {e}")))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AuthError::InternalError(format!("remove credentials: {e}"))),
        }
    }
}
