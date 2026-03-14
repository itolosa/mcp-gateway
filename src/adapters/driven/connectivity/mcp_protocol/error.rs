use crate::adapters::driven::configuration::error::ConfigError;
use crate::adapters::driven::connectivity::oauth::OAuthError;
use crate::hexagon::usecases::registry_error::RegistryError;

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("failed to spawn upstream server")]
    UpstreamSpawn { source: std::io::Error },

    #[error("upstream server initialization failed: {message}")]
    UpstreamInit { message: String },

    #[error("downstream server initialization failed: {message}")]
    DownstreamInit { message: String },

    #[error("invalid HTTP header: {message}")]
    HttpTransport { message: String },

    #[error("port {port} is already in use: {message}")]
    PortInUse { port: u16, message: String },

    #[error("OAuth authentication failed: {message}")]
    OAuthAuth { message: String },

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Registry(#[from] RegistryError),
}

impl From<OAuthError> for ProxyError {
    fn from(err: OAuthError) -> Self {
        ProxyError::OAuthAuth {
            message: err.to_string(),
        }
    }
}
