use crate::config::error::ConfigError;
use crate::registry::error::RegistryError;

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("server '{name}' not found in config")]
    ServerNotFound { name: String },

    #[error("failed to spawn upstream server")]
    UpstreamSpawn { source: std::io::Error },

    #[error("upstream server initialization failed: {message}")]
    UpstreamInit { message: String },

    #[error("downstream server initialization failed: {message}")]
    DownstreamInit { message: String },

    #[error("server '{name}' uses http transport, which is not yet supported for proxying")]
    UnsupportedTransport { name: String },

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Registry(#[from] RegistryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_not_found_display() {
        let err = ProxyError::ServerNotFound {
            name: "test".to_string(),
        };
        assert!(err.to_string().contains("test"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn upstream_spawn_display() {
        let err = ProxyError::UpstreamSpawn {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        assert!(err.to_string().contains("spawn"));
    }

    #[test]
    fn upstream_init_display() {
        let err = ProxyError::UpstreamInit {
            message: "handshake failed".to_string(),
        };
        assert!(err.to_string().contains("handshake failed"));
    }

    #[test]
    fn downstream_init_display() {
        let err = ProxyError::DownstreamInit {
            message: "bind failed".to_string(),
        };
        assert!(err.to_string().contains("bind failed"));
    }

    #[test]
    fn unsupported_transport_display() {
        let err = ProxyError::UnsupportedTransport {
            name: "remote".to_string(),
        };
        assert!(err.to_string().contains("remote"));
        assert!(err.to_string().contains("http"));
    }

    #[test]
    fn config_error_converts() {
        let config_err = ConfigError::Io {
            path: std::path::PathBuf::from("/tmp/test"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };
        let proxy_err = ProxyError::from(config_err);
        assert!(matches!(proxy_err, ProxyError::Config(_)));
        assert!(proxy_err.to_string().contains("/tmp/test"));
    }

    #[test]
    fn registry_error_converts() {
        let reg_err = RegistryError::NotFound {
            name: "test".to_string(),
        };
        let proxy_err = ProxyError::from(reg_err);
        assert!(matches!(proxy_err, ProxyError::Registry(_)));
        assert!(proxy_err.to_string().contains("test"));
    }
}
