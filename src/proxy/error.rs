use crate::config::error::ConfigError;
use crate::oauth::OAuthError;
use crate::registry::error::RegistryError;

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn http_transport_display() {
        let err = ProxyError::HttpTransport {
            message: "bad header".to_string(),
        };
        assert!(err.to_string().contains("bad header"));
        assert!(err.to_string().contains("HTTP header"));
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

    #[test]
    fn oauth_auth_display() {
        let err = ProxyError::OAuthAuth {
            message: "token expired".to_string(),
        };
        assert!(err.to_string().contains("OAuth"));
        assert!(err.to_string().contains("token expired"));
    }

    #[test]
    fn oauth_error_converts() {
        let oauth_err = OAuthError::MetadataDiscovery {
            message: "no endpoint".to_string(),
        };
        let proxy_err = ProxyError::from(oauth_err);
        assert!(matches!(proxy_err, ProxyError::OAuthAuth { .. }));
        assert!(proxy_err.to_string().contains("no endpoint"));
    }
}
