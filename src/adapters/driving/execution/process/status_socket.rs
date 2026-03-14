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
    pub state: String,
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
