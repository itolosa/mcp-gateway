use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::DaemonError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub pid: u32,
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

pub fn default_run_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".mcp-gateway").join("run"))
}

pub fn ensure_run_dir() -> Result<PathBuf, DaemonError> {
    ensure_run_dir_at(default_run_dir())
}

fn ensure_run_dir_at(run_dir: Option<PathBuf>) -> Result<PathBuf, DaemonError> {
    let dir = run_dir.ok_or_else(|| DaemonError::PidWrite {
        message: "cannot determine home directory".to_string(),
    })?;
    std::fs::create_dir_all(&dir).map_err(|e| DaemonError::PidWrite {
        message: format!("cannot create run directory: {e}"),
    })?;
    Ok(dir)
}

pub fn instance_path(run_dir: &Path, pid: u32) -> PathBuf {
    run_dir.join(format!("{pid}.json"))
}

pub fn sock_path(run_dir: &Path, pid: u32) -> PathBuf {
    run_dir.join(format!("{pid}.sock"))
}

pub fn log_path(run_dir: &Path, pid: u32) -> PathBuf {
    run_dir.join(format!("{pid}.log"))
}

pub fn write_instance(run_dir: &Path, info: &InstanceInfo) -> Result<(), DaemonError> {
    let path = instance_path(run_dir, info.pid);
    let json = match info.port {
        Some(port) => format!(
            r#"{{"pid":{},"transport":"{}","port":{}}}"#,
            info.pid, info.transport, port
        ),
        None => format!(r#"{{"pid":{},"transport":"{}"}}"#, info.pid, info.transport),
    };
    std::fs::write(&path, json).map_err(|e| DaemonError::PidWrite {
        message: e.to_string(),
    })
}

pub fn list_instances(run_dir: &Path) -> Result<Vec<InstanceInfo>, DaemonError> {
    if !run_dir.exists() {
        return Ok(vec![]);
    }
    let entries = std::fs::read_dir(run_dir).map_err(|e| DaemonError::PidRead {
        message: format!("cannot read run directory: {e}"),
    })?;
    let mut instances = Vec::new();
    for entry in entries {
        #[rustfmt::skip]
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        #[rustfmt::skip]
        let Some(ext) = path.extension() else { continue };
        if ext != "json" {
            continue;
        }
        #[rustfmt::skip]
        let Some(stem) = path.file_stem() else { continue };
        #[rustfmt::skip]
        let Ok(pid) = stem.to_string_lossy().parse::<u32>() else { continue };
        if is_process_alive(pid) {
            #[rustfmt::skip]
            let Ok(contents) = std::fs::read_to_string(&path) else { continue };
            #[rustfmt::skip]
            let Ok(info) = serde_json::from_str::<InstanceInfo>(&contents) else { continue };
            instances.push(info);
        } else {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(run_dir.join(format!("{pid}.sock")));
            // Log file is intentionally kept for post-mortem diagnosis via `logs`
        }
    }
    instances.sort_by_key(|i| i.pid);
    Ok(instances)
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

fn is_valid_pid(pid: u32) -> bool {
    pid > 0 && pid <= i32::MAX as u32
}

pub fn is_process_alive(pid: u32) -> bool {
    if !is_valid_pid(pid) {
        return false;
    }
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

pub fn remove_instance(run_dir: &Path, pid: u32) {
    let _ = std::fs::remove_file(instance_path(run_dir, pid));
    let _ = std::fs::remove_file(sock_path(run_dir, pid));
    // Log file is intentionally kept for post-mortem diagnosis via `logs`
}

pub fn send_signal(pid: u32, signal: &str) -> Result<(), DaemonError> {
    if !is_valid_pid(pid) {
        return Err(DaemonError::SignalFailed {
            message: format!("invalid PID {pid}"),
        });
    }
    let success = std::process::Command::new("kill")
        .args([&format!("-{signal}"), &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if success {
        Ok(())
    } else {
        Err(DaemonError::SignalFailed {
            message: format!("kill -{signal} {pid} failed"),
        })
    }
}

pub fn stop_instance(run_dir: &Path, pid: u32) -> Result<(), DaemonError> {
    if !is_process_alive(pid) {
        remove_instance(run_dir, pid);
        return Err(DaemonError::NotRunning);
    }
    send_signal(pid, "TERM")?;
    wait_for_exit(pid);
    remove_instance(run_dir, pid);
    Ok(())
}

fn wait_for_exit(pid: u32) {
    for _ in 0..100 {
        if !is_process_alive(pid) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

pub async fn check_port_available(port: u16) -> Result<(), DaemonError> {
    tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map(drop)
        .map_err(|_| DaemonError::PortInUse { port })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn default_run_dir_returns_some() {
        let path = default_run_dir();
        assert!(path.is_some());
        let p = path.unwrap();
        assert!(p.to_string_lossy().contains(".mcp-gateway"));
        assert!(p.to_string_lossy().contains("run"));
    }

    #[test]
    fn ensure_run_dir_creates_directory() {
        let dir = ensure_run_dir().unwrap();
        assert!(dir.exists());
        assert!(dir.is_dir());
    }

    #[test]
    fn instance_path_contains_pid() {
        let dir = Path::new("/tmp/run");
        let path = instance_path(dir, 1234);
        assert_eq!(path, PathBuf::from("/tmp/run/1234.json"));
    }

    #[test]
    fn sock_path_contains_pid() {
        let dir = Path::new("/tmp/run");
        let path = sock_path(dir, 1234);
        assert_eq!(path, PathBuf::from("/tmp/run/1234.sock"));
    }

    #[test]
    fn log_path_contains_pid() {
        let dir = Path::new("/tmp/run");
        let path = log_path(dir, 1234);
        assert_eq!(path, PathBuf::from("/tmp/run/1234.log"));
    }

    #[test]
    fn write_instance_creates_json_file() {
        let dir = tempfile::tempdir().unwrap();
        let info = InstanceInfo {
            pid: 42,
            transport: "http".to_string(),
            port: Some(8080),
        };
        write_instance(dir.path(), &info).unwrap();
        let content = std::fs::read_to_string(dir.path().join("42.json")).unwrap();
        let parsed: InstanceInfo = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, info);
    }

    #[test]
    fn write_instance_stdio_omits_port() {
        let dir = tempfile::tempdir().unwrap();
        let info = InstanceInfo {
            pid: 42,
            transport: "stdio".to_string(),
            port: None,
        };
        write_instance(dir.path(), &info).unwrap();
        let content = std::fs::read_to_string(dir.path().join("42.json")).unwrap();
        assert!(!content.contains(r#""port""#));
        let parsed: InstanceInfo = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed, info);
    }

    #[test]
    fn list_instances_returns_empty_when_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path().join("nonexistent");
        let instances = list_instances(&run_dir).unwrap();
        assert!(instances.is_empty());
    }

    #[test]
    fn list_instances_returns_alive_instance() {
        let dir = tempfile::tempdir().unwrap();
        let info = InstanceInfo {
            pid: std::process::id(),
            transport: "http".to_string(),
            port: Some(8080),
        };
        write_instance(dir.path(), &info).unwrap();
        let instances = list_instances(dir.path()).unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0], info);
    }

    #[test]
    fn list_instances_cleans_stale_instance() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let info = InstanceInfo {
            pid: u32::MAX,
            transport: "http".to_string(),
            port: Some(9090),
        };
        write_instance(run_dir, &info).unwrap();
        std::fs::write(run_dir.join(format!("{}.sock", u32::MAX)), "").unwrap();
        std::fs::write(run_dir.join(format!("{}.log", u32::MAX)), "log data").unwrap();
        let instances = list_instances(run_dir).unwrap();
        assert!(instances.is_empty());
        assert!(!run_dir.join(format!("{}.json", u32::MAX)).exists());
        assert!(!run_dir.join(format!("{}.sock", u32::MAX)).exists());
        // Log file is kept for post-mortem diagnosis
        assert!(run_dir.join(format!("{}.log", u32::MAX)).exists());
    }

    #[test]
    fn list_instances_skips_non_json_files() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        std::fs::write(run_dir.join("1234.sock"), "").unwrap();
        std::fs::write(run_dir.join("readme.txt"), "").unwrap();
        let instances = list_instances(run_dir).unwrap();
        assert!(instances.is_empty());
    }

    #[test]
    fn list_instances_skips_non_numeric_filenames() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        std::fs::write(run_dir.join("abc.json"), "{}").unwrap();
        let instances = list_instances(run_dir).unwrap();
        assert!(instances.is_empty());
    }

    #[test]
    fn list_instances_skips_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let pid = std::process::id();
        std::fs::write(run_dir.join(format!("{pid}.json")), "not json").unwrap();
        let instances = list_instances(run_dir).unwrap();
        assert!(instances.is_empty());
    }

    #[test]
    fn list_instances_sorts_by_pid() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let own_pid = std::process::id();
        // Use own PID for "alive" and own PID + 1 would be dead, so use own PID only.
        // Instead, write two instances both using our PID (contrived but tests sorting).
        let info1 = InstanceInfo {
            pid: own_pid,
            transport: "http".to_string(),
            port: Some(8080),
        };
        write_instance(run_dir, &info1).unwrap();
        let instances = list_instances(run_dir).unwrap();
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].pid, own_pid);
    }

    #[test]
    fn instance_info_debug() {
        let info = InstanceInfo {
            pid: 1234,
            transport: "http".to_string(),
            port: Some(8080),
        };
        let debug = format!("{info:?}");
        assert!(debug.contains("1234"));
        assert!(debug.contains("http"));
        assert!(debug.contains("8080"));
    }

    #[test]
    fn instance_info_clone() {
        let info = InstanceInfo {
            pid: 1234,
            transport: "stdio".to_string(),
            port: None,
        };
        let cloned = info.clone();
        assert_eq!(info, cloned);
    }

    #[test]
    fn instance_info_serde_roundtrip_http() {
        let info = InstanceInfo {
            pid: 1234,
            transport: "http".to_string(),
            port: Some(8080),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: InstanceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, info);
    }

    #[test]
    fn instance_info_serde_roundtrip_stdio() {
        let info = InstanceInfo {
            pid: 5678,
            transport: "stdio".to_string(),
            port: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains(r#""port""#));
        let parsed: InstanceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, info);
    }

    #[test]
    fn remove_instance_cleans_json_and_sock_but_keeps_log() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let info = InstanceInfo {
            pid: 42,
            transport: "http".to_string(),
            port: Some(8080),
        };
        write_instance(run_dir, &info).unwrap();
        std::fs::write(run_dir.join("42.sock"), "").unwrap();
        std::fs::write(run_dir.join("42.log"), "log data").unwrap();
        remove_instance(run_dir, 42);
        assert!(!run_dir.join("42.json").exists());
        assert!(!run_dir.join("42.sock").exists());
        assert!(run_dir.join("42.log").exists());
    }

    #[test]
    fn remove_instance_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        remove_instance(dir.path(), 99999);
        // No panic, no error
    }

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
        remove_pid_file(&path).unwrap();
        write_pid(&path, 1).unwrap();
        remove_pid_file(&path).unwrap();
        assert!(!path.exists());
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
    fn write_pid_to_nonexistent_dir_returns_error() {
        let path = Path::new("/nonexistent/dir/test.pid");
        let result = write_pid(path, 42);
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }

    #[test]
    fn read_pid_returns_error_on_io_failure() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_pid(dir.path());
        assert!(matches!(result, Err(DaemonError::PidRead { .. })));
    }

    #[test]
    fn send_signal_success_for_self() {
        send_signal(std::process::id(), "0").unwrap();
    }

    #[test]
    fn send_signal_rejects_pid_zero() {
        let result = send_signal(0, "0");
        assert!(matches!(result, Err(DaemonError::SignalFailed { .. })));
    }

    #[test]
    fn send_signal_fails_for_invalid_pid() {
        let result = send_signal(u32::MAX, "0");
        assert!(matches!(result, Err(DaemonError::SignalFailed { .. })));
    }

    #[test]
    fn stop_instance_returns_not_running_for_dead_pid() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let info = InstanceInfo {
            pid: u32::MAX,
            transport: "http".to_string(),
            port: Some(8080),
        };
        write_instance(run_dir, &info).unwrap();
        let result = stop_instance(run_dir, u32::MAX);
        assert!(matches!(result, Err(DaemonError::NotRunning)));
        assert!(!run_dir.join(format!("{}.json", u32::MAX)).exists());
    }

    #[test]
    fn stop_instance_stops_running_process() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let pid = child.id();
        let info = InstanceInfo {
            pid,
            transport: "http".to_string(),
            port: Some(8080),
        };
        write_instance(run_dir, &info).unwrap();
        assert!(is_process_alive(pid));
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        stop_instance(run_dir, pid).unwrap();
        assert!(!run_dir.join(format!("{pid}.json")).exists());
    }

    #[test]
    fn wait_for_exit_returns_immediately_for_dead_process() {
        wait_for_exit(u32::MAX);
    }

    #[test]
    fn wait_for_exit_polls_until_process_dies() {
        let mut child = std::process::Command::new("sleep")
            .arg("0.05")
            .spawn()
            .unwrap();
        let pid = child.id();
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        wait_for_exit(pid);
        assert!(!is_process_alive(pid));
    }

    #[test]
    fn remove_pid_file_returns_error_on_directory() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        std::fs::write(inner.join("file"), "data").unwrap();
        let result = remove_pid_file(&inner);
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }

    #[test]
    fn is_valid_pid_rejects_zero() {
        assert!(!is_valid_pid(0));
    }

    #[test]
    fn is_valid_pid_accepts_one() {
        assert!(is_valid_pid(1));
    }

    #[test]
    fn is_valid_pid_accepts_i32_max() {
        assert!(is_valid_pid(i32::MAX as u32));
    }

    #[test]
    fn is_valid_pid_rejects_above_i32_max() {
        assert!(!is_valid_pid(i32::MAX as u32 + 1));
    }

    #[test]
    fn ensure_run_dir_at_returns_error_when_none() {
        let result = ensure_run_dir_at(None);
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }

    #[test]
    fn ensure_run_dir_at_returns_error_when_create_fails() {
        let result = ensure_run_dir_at(Some(PathBuf::from("/dev/null/impossible")));
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }

    #[test]
    fn list_instances_returns_error_on_non_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("not-a-dir");
        std::fs::write(&file_path, "data").unwrap();
        let result = list_instances(&file_path);
        assert!(matches!(result, Err(DaemonError::PidRead { .. })));
    }

    #[test]
    fn send_signal_fails_for_nonexistent_process() {
        let result = send_signal(i32::MAX as u32, "0");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed"));
        assert!(!err.contains("invalid PID"));
    }

    #[test]
    fn write_instance_to_nonexistent_dir_returns_error() {
        let result = write_instance(
            Path::new("/nonexistent/dir"),
            &InstanceInfo {
                pid: 1,
                transport: "stdio".to_string(),
                port: None,
            },
        );
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }
}
