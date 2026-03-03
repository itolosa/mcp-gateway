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
}
