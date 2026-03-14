use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;

use rmcp::transport::auth::{AuthClient, AuthError, AuthorizationManager, OAuthClientConfig};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;

use super::callback::{run_callback_on_listener, CallbackParams};
use super::credentials::FileCredentialStore;
use super::error::OAuthError;
use crate::adapters::driven::configuration::model::OAuthConfig;

pub async fn create_oauth_transport(
    server_url: &str,
    oauth_config: &OAuthConfig,
    server_name: &str,
    custom_headers: HashMap<http::HeaderName, http::HeaderValue>,
) -> Result<StreamableHttpClientTransport<AuthClient<reqwest::Client>>, OAuthError> {
    create_oauth_transport_with(
        server_url,
        oauth_config,
        server_name,
        custom_headers,
        browser_auth,
    )
    .await
}

pub(crate) async fn create_oauth_transport_with<F, Fut>(
    server_url: &str,
    oauth_config: &OAuthConfig,
    server_name: &str,
    custom_headers: HashMap<http::HeaderName, http::HeaderValue>,
    interactive: F,
) -> Result<StreamableHttpClientTransport<AuthClient<reqwest::Client>>, OAuthError>
where
    F: FnOnce(String, tokio::net::TcpListener) -> Fut,
    Fut: Future<Output = Result<CallbackParams, OAuthError>>,
{
    #[rustfmt::skip]
    let mut auth_manager = AuthorizationManager::new(server_url).await.map_err(|e: AuthError| OAuthError::MetadataDiscovery { message: e.to_string() })?;

    let default = FileCredentialStore::default_path(server_name);
    #[rustfmt::skip]
    let cred_path = oauth_config.credentials_file.as_deref().map(PathBuf::from).or(default).ok_or_else(|| OAuthError::CredentialStore { message: "cannot determine credentials path".to_string() })?;

    auth_manager.set_credential_store(FileCredentialStore::new(cred_path));

    #[rustfmt::skip]
    let metadata = auth_manager.discover_metadata().await.map_err(|e: AuthError| OAuthError::MetadataDiscovery { message: e.to_string() })?;
    auth_manager.set_metadata(metadata);

    #[rustfmt::skip]
    let has_stored_token = auth_manager.initialize_from_store().await.map_err(|e: AuthError| OAuthError::CredentialStore { message: e.to_string() })?;

    if !has_stored_token {
        run_authorization_flow(&mut auth_manager, oauth_config, interactive).await?;
    }

    let auth_client = AuthClient::new(reqwest::Client::new(), auth_manager);
    let transport_config =
        StreamableHttpClientTransportConfig::with_uri(server_url).custom_headers(custom_headers);
    Ok(StreamableHttpClientTransport::with_client(
        auth_client,
        transport_config,
    ))
}

async fn run_authorization_flow<F, Fut>(
    auth_manager: &mut AuthorizationManager,
    oauth_config: &OAuthConfig,
    interactive: F,
) -> Result<(), OAuthError>
where
    F: FnOnce(String, tokio::net::TcpListener) -> Fut,
    Fut: Future<Output = Result<CallbackParams, OAuthError>>,
{
    let redirect_port = oauth_config.redirect_port;
    let redirect_uri = format!("http://127.0.0.1:{redirect_port}");

    if let Some(client_id) = &oauth_config.client_id {
        let config = OAuthClientConfig {
            client_id: client_id.clone(),
            client_secret: oauth_config.client_secret.clone(),
            scopes: oauth_config.scopes.clone(),
            redirect_uri: redirect_uri.clone(),
        };
        #[rustfmt::skip]
        auth_manager.configure_client(config).map_err(|e: AuthError| OAuthError::Authorization { message: e.to_string() })?;
    } else {
        let scope_refs: Vec<&str> = oauth_config.scopes.iter().map(String::as_str).collect();
        #[rustfmt::skip]
        auth_manager.register_client("mcp-gateway", &redirect_uri, &scope_refs).await.map_err(|e: AuthError| OAuthError::Authorization { message: e.to_string() })?;
    }

    let scope_refs: Vec<&str> = oauth_config.scopes.iter().map(|s| s.as_str()).collect();
    #[rustfmt::skip]
    let auth_url = auth_manager.get_authorization_url(&scope_refs).await.map_err(|e: AuthError| OAuthError::Authorization { message: e.to_string() })?;

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{redirect_port}"))
        .await
        .map_err(|e| OAuthError::CallbackServer {
            message: format!("bind to port {redirect_port}: {e}"),
        })?;

    let callback = interactive(auth_url.to_string(), listener).await?;

    #[rustfmt::skip]
    auth_manager.exchange_code_for_token(&callback.code, &callback.state).await.map_err(|e: AuthError| OAuthError::TokenExchange { message: e.to_string() })?;

    Ok(())
}

async fn browser_auth(
    auth_url: String,
    listener: tokio::net::TcpListener,
) -> Result<CallbackParams, OAuthError> {
    let browser_override = std::env::var("BROWSER");
    let program = browser_override.as_deref().unwrap_or(default_browser());
    let _ = std::process::Command::new(program)
        .arg(&auth_url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    eprintln!("Visit this URL to authorize: {auth_url}");
    run_callback_on_listener(listener).await
}

fn default_browser() -> &'static str {
    #[cfg(target_os = "macos")]
    return "open";
    #[cfg(target_os = "windows")]
    return "cmd";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return "xdg-open";
}
