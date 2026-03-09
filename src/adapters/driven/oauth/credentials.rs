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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileCredentialStore::new(dir.path().join("nonexistent.json"));
        let result = store.load().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds.json");
        let store = FileCredentialStore::new(path);

        let creds = StoredCredentials {
            client_id: "test-app".to_string(),
            token_response: None,
            granted_scopes: vec!["read".to_string()],
            token_received_at: Some(1000),
        };

        store.save(creds.clone()).await.unwrap();
        let loaded = store.load().await.unwrap().unwrap();
        assert_eq!(loaded.client_id, "test-app");
        assert_eq!(loaded.granted_scopes, vec!["read"]);
        assert_eq!(loaded.token_received_at, Some(1000));
    }

    #[tokio::test]
    async fn save_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deep").join("nested").join("creds.json");
        let store = FileCredentialStore::new(path.clone());

        let creds = StoredCredentials {
            client_id: "app".to_string(),
            token_response: None,
            granted_scopes: vec![],
            token_received_at: None,
        };

        store.save(creds).await.unwrap();
        assert!(path.exists());
    }

    #[tokio::test]
    async fn clear_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds.json");
        tokio::fs::write(&path, "{}").await.unwrap();

        let store = FileCredentialStore::new(path.clone());
        store.clear().await.unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn clear_missing_file_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileCredentialStore::new(dir.path().join("nope.json"));
        store.clear().await.unwrap();
    }

    #[tokio::test]
    async fn load_invalid_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        tokio::fs::write(&path, "not json").await.unwrap();

        let store = FileCredentialStore::new(path);
        let result = store.load().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("parse credentials"));
    }

    #[tokio::test]
    async fn default_path_contains_server_name() {
        let path = FileCredentialStore::default_path("my-server").unwrap();
        assert!(path.to_string_lossy().contains("my-server.json"));
        assert!(path.to_string_lossy().contains(".mcp-gateway"));
        assert!(path.to_string_lossy().contains("credentials"));
    }

    #[tokio::test]
    async fn load_permission_error_returns_error() {
        // Use a path that can't be read (directory as file)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir");
        tokio::fs::create_dir(&path).await.unwrap();

        let store = FileCredentialStore::new(path);
        let result = store.load().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("read credentials"));
    }

    #[tokio::test]
    async fn save_to_readonly_dir_returns_error() {
        // Save to an impossible path
        let store = FileCredentialStore::new(PathBuf::from("/dev/null/impossible/creds.json"));
        let creds = StoredCredentials {
            client_id: "app".to_string(),
            token_response: None,
            granted_scopes: vec![],
            token_received_at: None,
        };
        let result = store.save(creds).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn save_write_to_directory_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds_dir");
        tokio::fs::create_dir(&path).await.unwrap();

        // path is a directory, so writing to it will fail
        let store = FileCredentialStore::new(path);
        let creds = StoredCredentials {
            client_id: "app".to_string(),
            token_response: None,
            granted_scopes: vec![],
            token_received_at: None,
        };
        let result = store.save(creds).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("write credentials"));
    }

    #[tokio::test]
    async fn clear_permission_error_returns_error() {
        // Try to clear a path that is a directory, not a file
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir");
        tokio::fs::create_dir(&path).await.unwrap();

        let store = FileCredentialStore::new(path);
        let result = store.clear().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("remove credentials"));
    }
}
