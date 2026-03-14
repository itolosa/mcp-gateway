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
