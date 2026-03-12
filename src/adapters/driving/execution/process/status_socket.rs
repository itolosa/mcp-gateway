use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderStatus {
    pub name: String,
    pub connected: bool,
    pub provider_type: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayStatusReport {
    pub providers: Vec<ProviderStatus>,
}

pub fn start_status_listener(
    sock_path: PathBuf,
    report: tokio::sync::watch::Receiver<GatewayStatusReport>,
) -> Option<tokio::task::JoinHandle<()>> {
    let _ = std::fs::remove_file(&sock_path);
    let listener = match tokio::net::UnixListener::bind(&sock_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "could not bind status socket at {}: {e}",
                sock_path.display()
            );
            return None;
        }
    };
    Some(tokio::spawn(async move {
        loop {
            #[rustfmt::skip]
            let Ok((mut stream, _)) = listener.accept().await else { continue };
            let json = serde_json::to_string(&*report.borrow()).unwrap_or_default();
            let _ = stream.write_all(json.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    }))
}

pub async fn query_status(sock_path: &Path) -> Result<GatewayStatusReport, String> {
    let mut stream = tokio::net::UnixStream::connect(sock_path)
        .await
        .map_err(|e| format!("failed to connect to status socket: {e}"))?;
    let mut buf = String::new();
    stream
        .read_to_string(&mut buf)
        .await
        .map_err(|e| format!("failed to read status: {e}"))?;
    serde_json::from_str(&buf).map_err(|e| format!("failed to parse status: {e}"))
}

pub fn remove_sock_file(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample_report() -> GatewayStatusReport {
        GatewayStatusReport {
            providers: vec![
                ProviderStatus {
                    name: "alpha".to_string(),
                    connected: true,
                    provider_type: "stdio".to_string(),
                    target: "node server.js".to_string(),
                },
                ProviderStatus {
                    name: "beta".to_string(),
                    connected: false,
                    provider_type: "http".to_string(),
                    target: "https://example.com".to_string(),
                },
            ],
        }
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let report = sample_report();
        let json = serde_json::to_string(&report).unwrap();
        let parsed: GatewayStatusReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, parsed);
    }

    #[test]
    fn empty_report_serializes() {
        let report = GatewayStatusReport { providers: vec![] };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: GatewayStatusReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, parsed);
    }

    #[tokio::test]
    async fn listener_responds_to_client() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let report = sample_report();
        let (_tx, rx) = tokio::sync::watch::channel(report.clone());

        let _handle = start_status_listener(sock_path.clone(), rx);

        let result = query_status(&sock_path).await.unwrap();
        assert_eq!(result, report);
    }

    #[tokio::test]
    async fn listener_responds_to_multiple_clients() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let report = sample_report();
        let (_tx, rx) = tokio::sync::watch::channel(report.clone());

        let _handle = start_status_listener(sock_path.clone(), rx);

        let result1 = query_status(&sock_path).await.unwrap();
        let result2 = query_status(&sock_path).await.unwrap();
        assert_eq!(result1, report);
        assert_eq!(result2, report);
    }

    #[tokio::test]
    async fn query_nonexistent_socket_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("missing.sock");
        let result = query_status(&sock_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to connect"));
    }

    #[test]
    fn remove_sock_file_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        // Remove when file doesn't exist — no panic
        remove_sock_file(&sock_path);
        // Create a file and remove it
        std::fs::write(&sock_path, "").unwrap();
        assert!(sock_path.exists());
        remove_sock_file(&sock_path);
        assert!(!sock_path.exists());
        // Remove again — idempotent
        remove_sock_file(&sock_path);
    }

    #[test]
    fn provider_status_debug_format() {
        let status = ProviderStatus {
            name: "test".to_string(),
            connected: true,
            provider_type: "stdio".to_string(),
            target: "cmd".to_string(),
        };
        let debug = format!("{status:?}");
        assert!(debug.contains("test"));
    }

    #[test]
    fn gateway_status_report_debug_format() {
        let report = GatewayStatusReport { providers: vec![] };
        let debug = format!("{report:?}");
        assert!(debug.contains("providers"));
    }

    #[tokio::test]
    async fn listener_removes_stale_socket_file() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("stale.sock");
        // Create a regular file at the socket path
        std::fs::write(&sock_path, "stale").unwrap();
        let report = GatewayStatusReport { providers: vec![] };
        let (_tx, rx) = tokio::sync::watch::channel(report.clone());
        // Should succeed despite stale file
        let _handle = start_status_listener(sock_path.clone(), rx);
        let result = query_status(&sock_path).await.unwrap();
        assert_eq!(result, report);
    }

    #[test]
    fn provider_status_clone() {
        let status = ProviderStatus {
            name: "a".to_string(),
            connected: true,
            provider_type: "stdio".to_string(),
            target: "cmd".to_string(),
        };
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }

    #[test]
    fn gateway_status_report_clone() {
        let report = sample_report();
        let cloned = report.clone();
        assert_eq!(report, cloned);
    }

    #[tokio::test]
    async fn listener_returns_none_on_bind_failure() {
        // Try to bind to a path in a nonexistent directory
        let (_tx, rx) = tokio::sync::watch::channel(sample_report());
        let result = start_status_listener("/nonexistent/dir/test.sock".into(), rx);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn listener_reflects_updated_report() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("watch.sock");
        let initial = GatewayStatusReport { providers: vec![] };
        let (tx, rx) = tokio::sync::watch::channel(initial.clone());

        let _handle = start_status_listener(sock_path.clone(), rx);

        let result = query_status(&sock_path).await.unwrap();
        assert_eq!(result, initial);

        let updated = sample_report();
        tx.send(updated.clone()).unwrap();

        let result = query_status(&sock_path).await.unwrap();
        assert_eq!(result, updated);
    }

    #[tokio::test]
    async fn query_status_returns_error_on_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("bad.sock");
        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = stream.write_all(b"not json").await;
            let _ = stream.shutdown().await;
        });
        let result = query_status(&sock_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    #[tokio::test]
    async fn query_status_returns_error_on_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("bad-utf8.sock");
        let listener = tokio::net::UnixListener::bind(&sock_path).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Send invalid UTF-8 bytes — read_to_string will fail
            let _ = stream.write_all(&[0xFF, 0xFE, 0x80]).await;
            let _ = stream.shutdown().await;
        });
        let result = query_status(&sock_path).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to read"));
    }
}
