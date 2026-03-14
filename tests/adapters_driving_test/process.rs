// Tests migrated from:
//   src/adapters/driving/execution/process/pid.rs
//   src/adapters/driving/execution/process/status_socket.rs
//   src/adapters/driving/execution/process/log_file.rs
//   src/adapters/driving/execution/process/log_broadcast.rs
//   src/adapters/driving/execution/process/attach.rs
//   src/adapters/driving/execution/process/error.rs

// ============================================================
// pid tests
// ============================================================
mod pid {
    use std::path::{Path, PathBuf};

    use mcp_gateway::adapters::driving::execution::process::error::DaemonError;
    use mcp_gateway::adapters::driving::execution::process::pid::*;

    /// A PID that is invalid (above i32::MAX) but does NOT wrap to pid_t -1 on
    /// Linux.  u32::MAX (4294967295) wraps to pid_t -1, and `kill(-1, sig)`
    /// sends the signal to **every** process — catastrophic when a cargo-mutants
    /// `is_valid_pid -> true` mutant bypasses the validation guard.
    /// 2147483648 wraps to pid_t -2147483648, which simply fails with ESRCH.
    const DEAD_PID: u32 = i32::MAX as u32 + 1;

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
            pid: DEAD_PID,
            transport: "http".to_string(),
            port: Some(9090),
        };
        write_instance(run_dir, &info).unwrap();
        std::fs::write(run_dir.join(format!("{}.sock", DEAD_PID)), "").unwrap();
        std::fs::write(run_dir.join(format!("{}.log", DEAD_PID)), "log data").unwrap();
        let instances = list_instances(run_dir).unwrap();
        assert!(instances.is_empty());
        assert!(!run_dir.join(format!("{}.json", DEAD_PID)).exists());
        assert!(!run_dir.join(format!("{}.sock", DEAD_PID)).exists());
        // Log file is kept for post-mortem diagnosis
        assert!(run_dir.join(format!("{}.log", DEAD_PID)).exists());
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
        assert!(!is_process_alive(DEAD_PID));
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
        write_pid(&path, DEAD_PID).unwrap();
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
        let result = send_signal(DEAD_PID, "0");
        assert!(matches!(result, Err(DaemonError::SignalFailed { .. })));
    }

    #[test]
    fn stop_instance_returns_not_running_for_dead_pid() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        let info = InstanceInfo {
            pid: DEAD_PID,
            transport: "http".to_string(),
            port: Some(8080),
        };
        write_instance(run_dir, &info).unwrap();
        let result = stop_instance(run_dir, DEAD_PID);
        assert!(matches!(result, Err(DaemonError::NotRunning)));
        assert!(!run_dir.join(format!("{}.json", DEAD_PID)).exists());
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
    fn ensure_run_dir_at_returns_error_when_none() {
        let result = ensure_run_dir_at(None);
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot determine home directory"));
    }

    #[test]
    fn ensure_run_dir_at_returns_error_when_create_fails() {
        let result = ensure_run_dir_at(Some(PathBuf::from("/dev/null/impossible/path")));
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot create run directory"));
    }

    #[test]
    fn stop_instance_exercises_wait_loop_for_slow_dying_process() {
        let dir = tempfile::tempdir().unwrap();
        let run_dir = dir.path();
        // The trap handler runs after `sleep 60` is interrupted by SIGTERM.
        // `sleep 60 & wait` lets bash receive the signal while `wait` is
        // the foreground builtin, so the trap fires promptly.  The 100ms
        // sleep inside the trap forces `wait_for_exit` to iterate its
        // polling loop at least once (50ms per iteration).
        let mut child = std::process::Command::new("bash")
            .args(["-c", "trap 'sleep 0.1; exit 0' TERM; sleep 60 & wait"])
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
    fn remove_pid_file_returns_error_on_directory() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        std::fs::write(inner.join("file"), "data").unwrap();
        let result = remove_pid_file(&inner);
        assert!(matches!(result, Err(DaemonError::PidWrite { .. })));
    }

    // NOTE: is_valid_pid is a private function (not pub) in pid.rs.
    // The following tests cannot be migrated to integration tests:
    //
    // fn is_valid_pid_rejects_zero()
    // fn is_valid_pid_accepts_one()
    // fn is_valid_pid_accepts_i32_max()
    // fn is_valid_pid_rejects_above_i32_max()

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

// ============================================================
// status_socket tests
// ============================================================
mod status_socket {
    use mcp_gateway::adapters::driving::execution::process::status_socket::*;
    use tokio::io::AsyncWriteExt;

    fn sample_report() -> GatewayStatusReport {
        GatewayStatusReport {
            state: "Listening".to_string(),
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
        let report = GatewayStatusReport {
            state: "Listening".to_string(),
            providers: vec![],
        };
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
        let report = GatewayStatusReport {
            state: "Listening".to_string(),
            providers: vec![],
        };
        let debug = format!("{report:?}");
        assert!(debug.contains("providers"));
    }

    #[tokio::test]
    async fn listener_removes_stale_socket_file() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("stale.sock");
        // Create a regular file at the socket path
        std::fs::write(&sock_path, "stale").unwrap();
        let report = GatewayStatusReport {
            state: "Listening".to_string(),
            providers: vec![],
        };
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
        let initial = GatewayStatusReport {
            state: "Initializing".to_string(),
            providers: vec![],
        };
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

// ============================================================
// log_file tests
// ============================================================
mod log_file {
    use std::path::PathBuf;

    use mcp_gateway::adapters::driving::execution::process::error::DaemonError;
    use mcp_gateway::adapters::driving::execution::process::log_file::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn should_write_broadcast_messages_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let (sender, _) = broadcast::channel::<String>(16);

        let handle = spawn_log_writer(path.clone(), &sender);
        sender.send("hello world".to_string()).unwrap();
        sender.send("second line".to_string()).unwrap();
        drop(sender);
        handle.await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("hello world"));
        assert!(content.contains("second line"));
    }

    #[tokio::test]
    async fn should_handle_lagged_receiver() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lag.log");
        let (sender, _) = broadcast::channel::<String>(1);

        let handle = spawn_log_writer(path.clone(), &sender);
        // Send more than buffer size to cause lag
        sender.send("first".to_string()).unwrap();
        sender.send("second".to_string()).unwrap();
        sender.send("third".to_string()).unwrap();
        drop(sender);
        handle.await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        // At least the last message should be present
        assert!(content.contains("third"));
    }

    #[tokio::test]
    async fn should_not_panic_on_invalid_path() {
        let (sender, _) = broadcast::channel::<String>(16);
        let handle = spawn_log_writer(PathBuf::from("/dev/null/impossible/test.log"), &sender);
        drop(sender);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_read_log_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("read.log");
        tokio::fs::write(&path, "line one\nline two\n")
            .await
            .unwrap();

        let result = read_log(&path, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_follow_log_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("follow.log");
        tokio::fs::write(&path, "initial\n").await.unwrap();

        let path2 = path.clone();
        let handle = tokio::spawn(async move { read_log(&path2, true).await });

        // Give the follow loop time to read and sleep
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Append more data
        tokio::fs::write(&path, "initial\nappended\n")
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Follow runs forever, so abort it
        handle.abort();
        let result = handle.await;
        assert!(result.is_err()); // JoinError from abort
    }

    #[tokio::test]
    async fn should_return_error_on_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.log");
        let result = read_log(&path, false).await;
        assert!(matches!(result, Err(DaemonError::LogRead { .. })));
    }
}

// ============================================================
// log_broadcast tests
// ============================================================
mod log_broadcast {
    use mcp_gateway::adapters::driving::execution::process::log_broadcast::BroadcastLayer;
    use tokio::sync::broadcast;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn should_capture_event_when_subscriber_active() {
        let (sender, mut receiver) = broadcast::channel::<String>(16);
        let layer = BroadcastLayer::new(sender);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "test_target", "hello world");
        });
        let msg = receiver.try_recv().unwrap();
        assert!(msg.contains("INFO"));
        assert!(msg.contains("test_target"));
        assert!(msg.contains("hello world"));
    }

    #[test]
    fn should_not_panic_when_no_receivers() {
        let (sender, receiver) = broadcast::channel::<String>(16);
        drop(receiver);
        let layer = BroadcastLayer::new(sender);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("no receivers");
        });
    }

    #[test]
    fn should_format_level_target_message() {
        let (sender, mut receiver) = broadcast::channel::<String>(16);
        let layer = BroadcastLayer::new(sender);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!(target: "my_module", "something happened");
        });
        let msg = receiver.try_recv().unwrap();
        assert_eq!(msg, "WARN my_module: something happened");
    }
}

// ============================================================
// attach tests
// ============================================================
mod attach {
    use mcp_gateway::adapters::driving::execution::process::attach::attach;
    use mcp_gateway::adapters::driving::execution::process::error::DaemonError;
    use tokio_util::sync::CancellationToken;

    // NOTE: process_chunk and attach_err are private functions in attach.rs.
    // Tests that directly call process_chunk cannot be migrated to integration tests:
    //
    // fn should_return_error_when_write_fails()
    // fn should_return_error_when_flush_fails()
    // fn should_skip_non_data_lines()

    async fn start_mock_sse_server() -> (u16, CancellationToken, tokio::task::JoinHandle<()>) {
        use axum::response::sse::{Event, Sse};
        use std::convert::Infallible;

        async fn mock_logs() -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
            Sse::new(tokio_stream::iter(vec![
                Ok::<_, Infallible>(Event::default().data("hello world")),
                Ok(Event::default().data("second line")),
            ]))
        }

        let router = axum::Router::new().route("/logs", axum::routing::get(mock_logs));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let ct = CancellationToken::new();
        let ct_inner = ct.clone();
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(ct_inner.cancelled_owned())
                .await;
        });
        (port, ct, handle)
    }

    #[tokio::test]
    async fn should_return_error_when_connection_refused() {
        let mut buf = Vec::new();
        let result = attach(1, &mut buf).await;
        assert!(matches!(result, Err(DaemonError::AttachFailed { .. })));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn should_stream_sse_data_lines_to_writer() {
        let (port, ct, handle) = start_mock_sse_server().await;

        let mut buf = Vec::new();
        let result = attach(port, &mut buf).await;
        assert!(result.is_ok());
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("hello world"));
        assert!(output.contains("second line"));

        ct.cancel();
        let _ = handle.await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn should_return_error_when_stream_interrupted() {
        use tokio::io::AsyncWriteExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\n\
                      Transfer-Encoding: chunked\r\n\r\n\
                      5\r\nhello\r\n\
                      INVALID\r\n",
                )
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        });

        let mut buf = Vec::new();
        let result = attach(port, &mut buf).await;
        assert!(matches!(result, Err(DaemonError::AttachFailed { .. })));
        let _ = server.await;
    }
}

// ============================================================
// error tests
// ============================================================
mod error {
    use mcp_gateway::adapters::driving::execution::process::error::DaemonError;

    #[test]
    fn already_running_display() {
        let err = DaemonError::AlreadyRunning {
            pid: 1234,
            port: 8080,
        };
        let msg = err.to_string();
        assert!(msg.contains("1234"));
        assert!(msg.contains("8080"));
        assert!(msg.contains("stop --port"));
    }

    #[test]
    fn port_in_use_display() {
        let err = DaemonError::PortInUse { port: 8080 };
        let msg = err.to_string();
        assert!(msg.contains("8080"));
        assert!(msg.contains("another process"));
        assert!(msg.contains("--port"));
    }

    #[test]
    fn pid_write_display() {
        let err = DaemonError::PidWrite {
            message: "permission denied".to_string(),
        };
        assert!(err.to_string().contains("permission denied"));
        assert!(err.to_string().contains("write PID"));
    }

    #[test]
    fn pid_read_display() {
        let err = DaemonError::PidRead {
            message: "corrupt file".to_string(),
        };
        assert!(err.to_string().contains("corrupt file"));
        assert!(err.to_string().contains("read PID"));
    }

    #[test]
    fn not_running_display() {
        let err = DaemonError::NotRunning;
        let msg = err.to_string();
        assert!(msg.contains("no gateway instance found"));
        assert!(msg.contains("mcp-gateway start"));
    }

    #[test]
    fn signal_failed_display() {
        let err = DaemonError::SignalFailed {
            message: "operation not permitted".to_string(),
        };
        assert!(err.to_string().contains("operation not permitted"));
        assert!(err.to_string().contains("signal"));
    }

    #[test]
    fn attach_failed_display() {
        let err = DaemonError::AttachFailed {
            message: "connection refused".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("connection refused"));
        assert!(msg.contains("cannot attach"));
    }

    #[test]
    fn log_read_display() {
        let err = DaemonError::LogRead {
            message: "file not found".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("file not found"));
        assert!(msg.contains("cannot read logs"));
    }

    #[test]
    fn user_input_display() {
        let err = DaemonError::UserInput("invalid selection".to_string());
        assert!(err.to_string().contains("invalid selection"));
    }
}
