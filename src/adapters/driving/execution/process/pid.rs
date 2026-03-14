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

pub fn ensure_run_dir_at(run_dir: Option<PathBuf>) -> Result<PathBuf, DaemonError> {
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
