#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("gateway already running (PID {pid})")]
    AlreadyRunning { pid: u32 },

    #[error("port {port} is already in use")]
    PortInUse { port: u16 },

    #[error("failed to write PID file: {message}")]
    PidWrite { message: String },

    #[error("failed to read PID file: {message}")]
    PidRead { message: String },

    #[error("gateway is not running")]
    NotRunning,

    #[error("failed to send signal: {message}")]
    SignalFailed { message: String },

    #[error("attach failed: {message}")]
    AttachFailed { message: String },

    #[error("{0}")]
    UserInput(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_running_display() {
        let err = DaemonError::AlreadyRunning { pid: 1234 };
        assert!(err.to_string().contains("1234"));
        assert!(err.to_string().contains("already running"));
    }

    #[test]
    fn port_in_use_display() {
        let err = DaemonError::PortInUse { port: 8080 };
        assert!(err.to_string().contains("8080"));
        assert!(err.to_string().contains("already in use"));
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
        assert!(err.to_string().contains("not running"));
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
        assert!(err.to_string().contains("connection refused"));
        assert!(err.to_string().contains("attach"));
    }

    #[test]
    fn user_input_display() {
        let err = DaemonError::UserInput("invalid selection".to_string());
        assert!(err.to_string().contains("invalid selection"));
    }
}
