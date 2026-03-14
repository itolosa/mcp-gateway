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
