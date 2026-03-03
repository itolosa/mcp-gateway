use std::path::{Path, PathBuf};

use crate::daemon::error::DaemonError;

pub fn default_pid_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".mcp-gateway.pid"))
}

pub fn write_pid(path: &Path, pid: u32) -> Result<(), DaemonError> {
    std::fs::write(path, pid.to_string()).map_err(|e| DaemonError::PidWrite {
        message: e.to_string(),
    })
}

pub fn read_pid(path: &Path) -> Result<Option<u32>, DaemonError> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            let pid = contents
                .trim()
                .parse::<u32>()
                .map_err(|e| DaemonError::PidRead {
                    message: e.to_string(),
                })?;
            Ok(Some(pid))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(DaemonError::PidRead {
            message: e.to_string(),
        }),
    }
}

pub fn is_process_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn check_already_running(path: &Path) -> Result<Option<u32>, DaemonError> {
    match read_pid(path)? {
        Some(pid) if is_process_alive(pid) => Ok(Some(pid)),
        Some(_) => {
            let _ = std::fs::remove_file(path);
            Ok(None)
        }
        None => Ok(None),
    }
}

pub fn remove_pid_file(path: &Path) -> Result<(), DaemonError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(DaemonError::PidWrite {
            message: e.to_string(),
        }),
    }
}

pub async fn check_port_available(port: u16) -> Result<(), DaemonError> {
    tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map(drop)
        .map_err(|_| DaemonError::PortInUse { port })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn write_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.pid");
        write_pid(&path, 42).unwrap();
        let pid = read_pid(&path).unwrap();
        assert_eq!(pid, Some(42));
    }

    #[test]
    fn read_returns_none_on_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.pid");
        let pid = read_pid(&path).unwrap();
        assert_eq!(pid, None);
    }

    #[test]
    fn read_returns_error_on_malformed_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.pid");
        std::fs::write(&path, "not-a-number").unwrap();
        let result = read_pid(&path);
        assert!(matches!(result, Err(DaemonError::PidRead { .. })));
    }

    #[test]
    fn is_process_alive_true_for_self() {
        assert!(is_process_alive(std::process::id()));
    }

    #[test]
    fn is_process_alive_false_for_max_pid() {
        assert!(!is_process_alive(u32::MAX));
    }

    #[test]
    fn check_already_running_none_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no.pid");
        let result = check_already_running(&path).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn check_already_running_none_when_stale_pid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stale.pid");
        write_pid(&path, u32::MAX).unwrap();
        let result = check_already_running(&path).unwrap();
        assert_eq!(result, None);
        // Stale PID file should be cleaned up
        assert!(!path.exists());
    }

    #[test]
    fn check_already_running_some_when_alive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("alive.pid");
        let own_pid = std::process::id();
        write_pid(&path, own_pid).unwrap();
        let result = check_already_running(&path).unwrap();
        assert_eq!(result, Some(own_pid));
    }

    #[test]
    fn remove_pid_file_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rm.pid");
        // Remove when file does not exist — should be fine
        remove_pid_file(&path).unwrap();
        // Create and remove
        write_pid(&path, 1).unwrap();
        remove_pid_file(&path).unwrap();
        assert!(!path.exists());
        // Remove again — idempotent
        remove_pid_file(&path).unwrap();
    }

    #[tokio::test]
    async fn check_port_available_ok_for_free_port() {
        let result = check_port_available(0).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn check_port_available_err_for_bound_port() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let result = check_port_available(port).await;
        assert!(matches!(result, Err(DaemonError::PortInUse { .. })));
    }

    #[test]
    fn default_pid_path_returns_some() {
        // In test environments, home_dir should return Some
        let path = default_pid_path();
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.to_string_lossy().contains(".mcp-gateway.pid"));
    }

    #[test]
    fn write_pid_to_nonexistent_dir_returns_error() {
        let path = Path::new("/nonexistent/dir/test.pid");
        let result = write_pid(path, 42);
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }

    #[test]
    fn read_pid_returns_error_on_io_failure() {
        // Reading a directory as a file triggers a non-NotFound IO error
        let dir = tempfile::tempdir().unwrap();
        let result = read_pid(dir.path());
        assert!(matches!(result, Err(DaemonError::PidRead { .. })));
    }

    #[test]
    fn remove_pid_file_returns_error_on_directory() {
        // Removing a non-empty directory with remove_file triggers a non-NotFound error
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        std::fs::write(inner.join("file"), "data").unwrap();
        let result = remove_pid_file(&inner);
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }
}
