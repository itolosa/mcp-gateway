#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("port {port} already in use by gateway (PID {pid}), use 'stop --port {port}' first or choose a different port")]
    AlreadyRunning { pid: u32, port: u16 },

    #[error("port {port} is in use by another process, choose a different port with --port")]
    PortInUse { port: u16 },

    #[error("failed to write PID file: {message}")]
    PidWrite { message: String },

    #[error("failed to read PID file: {message}")]
    PidRead { message: String },

    #[error("no gateway instance found, start one with 'mcp-gateway start'")]
    NotRunning,

    #[error("failed to send signal: {message}")]
    SignalFailed { message: String },

    #[error("cannot attach: {message}")]
    AttachFailed { message: String },

    #[error("cannot read logs: {message}")]
    LogRead { message: String },

    #[error("{0}")]
    UserInput(String),
}

#[cfg(test)]
mod tests {
    use super::*;

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
