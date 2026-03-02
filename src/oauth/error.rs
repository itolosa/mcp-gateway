#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("metadata discovery failed: {message}")]
    MetadataDiscovery { message: String },

    #[error("authorization failed: {message}")]
    Authorization { message: String },

    #[error("token exchange failed: {message}")]
    TokenExchange { message: String },

    #[error("callback server failed: {message}")]
    CallbackServer { message: String },

    #[error("credential store error: {message}")]
    CredentialStore { message: String },

    #[error("transport error: {message}")]
    Transport { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_discovery_display() {
        let err = OAuthError::MetadataDiscovery {
            message: "no endpoint".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("metadata discovery"));
        assert!(msg.contains("no endpoint"));
    }

    #[test]
    fn authorization_display() {
        let err = OAuthError::Authorization {
            message: "denied".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("authorization"));
        assert!(msg.contains("denied"));
    }

    #[test]
    fn token_exchange_display() {
        let err = OAuthError::TokenExchange {
            message: "invalid code".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("token exchange"));
        assert!(msg.contains("invalid code"));
    }

    #[test]
    fn callback_server_display() {
        let err = OAuthError::CallbackServer {
            message: "bind failed".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("callback server"));
        assert!(msg.contains("bind failed"));
    }

    #[test]
    fn credential_store_display() {
        let err = OAuthError::CredentialStore {
            message: "io error".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("credential store"));
        assert!(msg.contains("io error"));
    }

    #[test]
    fn transport_display() {
        let err = OAuthError::Transport {
            message: "connection refused".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("transport"));
        assert!(msg.contains("connection refused"));
    }
}
