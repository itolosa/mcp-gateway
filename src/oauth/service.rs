use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;

use rmcp::transport::auth::{AuthClient, AuthError, AuthorizationManager, OAuthClientConfig};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;

use crate::config::model::OAuthConfig;
use crate::oauth::callback::{run_callback_server, CallbackParams};
use crate::oauth::credentials::FileCredentialStore;
use crate::oauth::error::OAuthError;

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
    F: FnOnce(String, u16) -> Fut,
    Fut: Future<Output = Result<CallbackParams, OAuthError>>,
{
    let mut auth_manager = AuthorizationManager::new(server_url)
        .await
        .map_err(metadata_err)?;

    let default = FileCredentialStore::default_path(server_name);
    let cred_path = resolve_credential_path(oauth_config.credentials_file.as_deref(), default)?;

    auth_manager.set_credential_store(FileCredentialStore::new(cred_path));

    let metadata = auth_manager
        .discover_metadata()
        .await
        .map_err(metadata_err)?;
    auth_manager.set_metadata(metadata);

    let has_stored_token = auth_manager
        .initialize_from_store()
        .await
        .map_err(cred_store_err)?;

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

fn resolve_credential_path(
    explicit_path: Option<&str>,
    default_path: Option<PathBuf>,
) -> Result<PathBuf, OAuthError> {
    explicit_path
        .map(PathBuf::from)
        .or(default_path)
        .ok_or_else(|| OAuthError::CredentialStore {
            message: "cannot determine credentials path".to_string(),
        })
}

fn metadata_err(e: AuthError) -> OAuthError {
    OAuthError::MetadataDiscovery {
        message: e.to_string(),
    }
}

fn cred_store_err(e: AuthError) -> OAuthError {
    OAuthError::CredentialStore {
        message: e.to_string(),
    }
}

fn auth_err(e: AuthError) -> OAuthError {
    OAuthError::Authorization {
        message: e.to_string(),
    }
}

fn token_err(e: AuthError) -> OAuthError {
    OAuthError::TokenExchange {
        message: e.to_string(),
    }
}

async fn run_authorization_flow<F, Fut>(
    auth_manager: &mut AuthorizationManager,
    oauth_config: &OAuthConfig,
    interactive: F,
) -> Result<(), OAuthError>
where
    F: FnOnce(String, u16) -> Fut,
    Fut: Future<Output = Result<CallbackParams, OAuthError>>,
{
    let redirect_uri = format!("http://127.0.0.1:{}", oauth_config.redirect_port);

    if let Some(client_id) = &oauth_config.client_id {
        let config = OAuthClientConfig {
            client_id: client_id.clone(),
            client_secret: oauth_config.client_secret.clone(),
            scopes: oauth_config.scopes.clone(),
            redirect_uri: redirect_uri.clone(),
        };
        auth_manager.configure_client(config).map_err(auth_err)?;
    } else {
        auth_manager
            .register_client("mcp-gateway", &redirect_uri)
            .await
            .map_err(auth_err)?;
    }

    let scope_refs: Vec<&str> = oauth_config.scopes.iter().map(|s| s.as_str()).collect();
    let auth_url = auth_manager
        .get_authorization_url(&scope_refs)
        .await
        .map_err(auth_err)?;

    let callback = interactive(auth_url.to_string(), oauth_config.redirect_port).await?;

    auth_manager
        .exchange_code_for_token(&callback.code, &callback.state)
        .await
        .map_err(token_err)?;

    Ok(())
}

async fn browser_auth(auth_url: String, redirect_port: u16) -> Result<CallbackParams, OAuthError> {
    let mut cmd = browser_command();
    cmd.arg(&auth_url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let _ = cmd.spawn();
    eprintln!("Visit this URL to authorize: {auth_url}");
    run_callback_server(redirect_port).await
}

fn browser_command() -> std::process::Command {
    #[cfg(target_os = "macos")]
    return std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    return std::process::Command::new("cmd");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return std::process::Command::new("xdg-open");
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    use axum::extract::State;
    use axum::routing::{get, post};
    use axum::Json;

    #[derive(Clone)]
    struct MockOAuthState {
        base_url: String,
    }

    async fn metadata_handler(State(state): State<MockOAuthState>) -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "authorization_endpoint": format!("{}/authorize", state.base_url),
            "token_endpoint": format!("{}/token", state.base_url),
            "registration_endpoint": format!("{}/register", state.base_url),
            "issuer": state.base_url,
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"]
        }))
    }

    async fn token_handler() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "access_token": "test_access_token",
            "token_type": "bearer",
            "expires_in": 3600
        }))
    }

    async fn register_handler() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "client_id": "registered_client",
            "redirect_uris": ["http://127.0.0.1:9876"]
        }))
    }

    fn spawn_mock_oauth_server(listener: tokio::net::TcpListener, base_url: String) {
        let app = axum::Router::new()
            .route(
                "/.well-known/oauth-authorization-server",
                get(metadata_handler),
            )
            .route("/token", post(token_handler))
            .route("/register", post(register_handler))
            .with_state(MockOAuthState { base_url });
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    }

    async fn start_mock_oauth_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");
        spawn_mock_oauth_server(listener, base_url.clone());
        base_url
    }

    async fn test_interactive_auth(
        auth_url: String,
        _redirect_port: u16,
    ) -> Result<CallbackParams, OAuthError> {
        let parsed = url::Url::parse(&auth_url).unwrap();
        let state = parsed
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.to_string())
            .unwrap();
        Ok(CallbackParams {
            code: "test_auth_code".to_string(),
            state,
        })
    }

    #[test]
    fn resolve_credential_path_explicit_overrides_default() {
        let result = resolve_credential_path(
            Some("/explicit/path.json"),
            Some(PathBuf::from("/default/path.json")),
        );
        assert_eq!(result.unwrap(), PathBuf::from("/explicit/path.json"));
    }

    #[test]
    fn resolve_credential_path_falls_back_to_default() {
        let result = resolve_credential_path(None, Some(PathBuf::from("/default/path.json")));
        assert_eq!(result.unwrap(), PathBuf::from("/default/path.json"));
    }

    #[test]
    fn resolve_credential_path_no_path_returns_error() {
        let result = resolve_credential_path(None, None);
        let err = result.err().unwrap();
        assert!(matches!(err, OAuthError::CredentialStore { .. }));
        assert!(err.to_string().contains("cannot determine"));
    }

    #[test]
    fn metadata_err_formats_message() {
        let err = metadata_err(AuthError::InternalError("test".to_string()));
        assert!(err.to_string().contains("test"));
        assert!(matches!(err, OAuthError::MetadataDiscovery { .. }));
    }

    #[test]
    fn cred_store_err_formats_message() {
        let err = cred_store_err(AuthError::InternalError("test".to_string()));
        assert!(err.to_string().contains("test"));
        assert!(matches!(err, OAuthError::CredentialStore { .. }));
    }

    #[test]
    fn auth_err_formats_message() {
        let err = auth_err(AuthError::InternalError("test".to_string()));
        assert!(err.to_string().contains("test"));
        assert!(matches!(err, OAuthError::Authorization { .. }));
    }

    #[test]
    fn token_err_formats_message() {
        let err = token_err(AuthError::InternalError("test".to_string()));
        assert!(err.to_string().contains("test"));
        assert!(matches!(err, OAuthError::TokenExchange { .. }));
    }

    #[test]
    fn browser_command_returns_expected_program() {
        let cmd = browser_command();
        let debug = format!("{cmd:?}");
        #[cfg(target_os = "macos")]
        assert!(debug.contains("open"));
        #[cfg(target_os = "windows")]
        assert!(debug.contains("cmd"));
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        assert!(debug.contains("xdg-open"));
    }

    #[tokio::test]
    async fn browser_auth_receives_callback() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut client = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
                .await
                .unwrap();
            tokio::io::AsyncWriteExt::write_all(
                &mut client,
                b"GET /?code=test_code&state=test_state HTTP/1.1\r\nHost: localhost\r\n\r\n",
            )
            .await
            .unwrap();
        });

        let result = browser_auth("http://auth-url".to_string(), port)
            .await
            .unwrap();
        assert_eq!(result.code, "test_code");
        assert_eq!(result.state, "test_state");
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn create_oauth_transport_invalid_url_returns_error() {
        let config = OAuthConfig {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: None,
        };
        let result =
            create_oauth_transport("not a valid url", &config, "test", HashMap::new()).await;
        let err = result.err().unwrap();
        assert!(matches!(err, OAuthError::MetadataDiscovery { .. }));
    }

    #[tokio::test]
    async fn create_oauth_transport_with_metadata_discovery_failure() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}");

        // Empty server — no OAuth metadata endpoints
        let app = axum::Router::new();
        tokio::spawn(async move { axum::serve(listener, app).await.ok() });

        let dir = tempfile::tempdir().unwrap();
        let config = OAuthConfig {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: Some(dir.path().join("creds.json").to_string_lossy().to_string()),
        };

        let result = create_oauth_transport_with(
            &url,
            &config,
            "test",
            HashMap::new(),
            test_interactive_auth,
        )
        .await;
        let err = result.err().unwrap();
        assert!(matches!(err, OAuthError::MetadataDiscovery { .. }));
    }

    #[tokio::test]
    async fn create_oauth_transport_with_stored_credentials() {
        let base_url = start_mock_oauth_server().await;

        let dir = tempfile::tempdir().unwrap();
        let creds_path = dir.path().join("creds.json");
        let creds_content = r#"{
                "client_id": "stored-client",
                "token_response": {
                    "access_token": "stored_token",
                    "token_type": "bearer"
                },
                "granted_scopes": ["read"],
                "token_received_at": 1700000000
            }"#;
        tokio::fs::write(&creds_path, creds_content).await.unwrap();

        let config = OAuthConfig {
            client_id: Some("stored-client".to_string()),
            client_secret: None,
            scopes: vec!["read".to_string()],
            redirect_port: 9876,
            credentials_file: Some(creds_path.to_string_lossy().to_string()),
        };

        let result = create_oauth_transport_with(
            &base_url,
            &config,
            "test",
            HashMap::new(),
            test_interactive_auth,
        )
        .await;
        assert!(result.is_ok());

        // Verify credentials file was not overwritten by an auth flow
        let after = tokio::fs::read_to_string(&creds_path).await.unwrap();
        assert_eq!(after, creds_content);
    }

    #[tokio::test]
    async fn create_oauth_transport_with_auth_flow_configure_client() {
        let base_url = start_mock_oauth_server().await;

        let dir = tempfile::tempdir().unwrap();
        let config = OAuthConfig {
            client_id: Some("my-app".to_string()),
            client_secret: None,
            scopes: vec!["read".to_string()],
            redirect_port: 9876,
            credentials_file: Some(dir.path().join("creds.json").to_string_lossy().to_string()),
        };

        let result = create_oauth_transport_with(
            &base_url,
            &config,
            "test",
            HashMap::new(),
            test_interactive_auth,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn create_oauth_transport_with_auth_flow_register_client() {
        let base_url = start_mock_oauth_server().await;

        let dir = tempfile::tempdir().unwrap();
        let config = OAuthConfig {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: Some(dir.path().join("creds.json").to_string_lossy().to_string()),
        };

        let result = create_oauth_transport_with(
            &base_url,
            &config,
            "test",
            HashMap::new(),
            test_interactive_auth,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn run_authorization_flow_no_metadata_returns_error() {
        let mut auth_manager = AuthorizationManager::new("http://127.0.0.1:1")
            .await
            .unwrap();

        let config = OAuthConfig {
            client_id: Some("app".to_string()),
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: None,
        };

        let result =
            run_authorization_flow(&mut auth_manager, &config, test_interactive_auth).await;
        assert!(result.is_err());
    }
}
