use std::collections::HashMap;
use std::path::PathBuf;

use mcp_gateway::adapters::driven::configuration::model::OAuthConfig;
use mcp_gateway::adapters::driven::connectivity::oauth::callback::CallbackParams;
use mcp_gateway::adapters::driven::connectivity::oauth::create_oauth_transport;
use mcp_gateway::adapters::driven::connectivity::oauth::create_oauth_transport_with;
use mcp_gateway::adapters::driven::connectivity::oauth::credentials::FileCredentialStore;
use mcp_gateway::adapters::driven::connectivity::oauth::error::OAuthError;
use rmcp::transport::auth::{CredentialStore, StoredCredentials};

// -- OAuthError tests (from oauth/error.rs) --

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

// -- FileCredentialStore tests (from oauth/credentials.rs) --

#[tokio::test]
async fn load_missing_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileCredentialStore::new(dir.path().join("nonexistent.json"));
    let result = store.load().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("creds.json");
    let store = FileCredentialStore::new(path);

    let creds = StoredCredentials {
        client_id: "test-app".to_string(),
        token_response: None,
        granted_scopes: vec!["read".to_string()],
        token_received_at: Some(1000),
    };

    store.save(creds.clone()).await.unwrap();
    let loaded = store.load().await.unwrap().unwrap();
    assert_eq!(loaded.client_id, "test-app");
    assert_eq!(loaded.granted_scopes, vec!["read"]);
    assert_eq!(loaded.token_received_at, Some(1000));
}

#[tokio::test]
async fn save_creates_parent_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deep").join("nested").join("creds.json");
    let store = FileCredentialStore::new(path.clone());

    let creds = StoredCredentials {
        client_id: "app".to_string(),
        token_response: None,
        granted_scopes: vec![],
        token_received_at: None,
    };

    store.save(creds).await.unwrap();
    assert!(path.exists());
}

#[tokio::test]
async fn clear_removes_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("creds.json");
    tokio::fs::write(&path, "{}").await.unwrap();

    let store = FileCredentialStore::new(path.clone());
    store.clear().await.unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn clear_missing_file_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileCredentialStore::new(dir.path().join("nope.json"));
    store.clear().await.unwrap();
}

#[tokio::test]
async fn load_invalid_json_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    tokio::fs::write(&path, "not json").await.unwrap();

    let store = FileCredentialStore::new(path);
    let result = store.load().await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("parse credentials"));
}

#[tokio::test]
async fn default_path_contains_server_name() {
    let path = FileCredentialStore::default_path("my-server").unwrap();
    assert!(path.to_string_lossy().contains("my-server.json"));
    assert!(path.to_string_lossy().contains(".mcp-gateway"));
    assert!(path.to_string_lossy().contains("credentials"));
}

#[tokio::test]
async fn load_permission_error_returns_error() {
    // Use a path that can't be read (directory as file)
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir");
    tokio::fs::create_dir(&path).await.unwrap();

    let store = FileCredentialStore::new(path);
    let result = store.load().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("read credentials"));
}

#[tokio::test]
async fn save_to_readonly_dir_returns_error() {
    // Save to an impossible path
    let store = FileCredentialStore::new(PathBuf::from("/dev/null/impossible/creds.json"));
    let creds = StoredCredentials {
        client_id: "app".to_string(),
        token_response: None,
        granted_scopes: vec![],
        token_received_at: None,
    };
    let result = store.save(creds).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn save_write_to_directory_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("creds_dir");
    tokio::fs::create_dir(&path).await.unwrap();

    // path is a directory, so writing to it will fail
    let store = FileCredentialStore::new(path);
    let creds = StoredCredentials {
        client_id: "app".to_string(),
        token_response: None,
        granted_scopes: vec![],
        token_received_at: None,
    };
    let result = store.save(creds).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("write credentials"));
}

#[tokio::test]
async fn clear_permission_error_returns_error() {
    // Try to clear a path that is a directory, not a file
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir");
    tokio::fs::create_dir(&path).await.unwrap();

    let store = FileCredentialStore::new(path);
    let result = store.clear().await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("remove credentials"));
}

// -- callback.rs tests --
// NOTE: parse_callback_params, bind_err, timeout_err, accept_err, read_err,
// and accept_callback are all private functions in callback.rs. They cannot be
// tested directly from integration tests.
//
// run_callback_on_listener and run_callback_server_with_timeout are pub(crate).
//
// The following tests exercise the callback server through the public
// run_callback_server function where possible.

#[tokio::test]
async fn run_callback_server_bind_conflict_returns_error() {
    use mcp_gateway::adapters::driven::connectivity::oauth::callback::run_callback_server;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Try to bind same port
    let result = run_callback_server(port).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("bind to port"));
    // Keep listener alive
    drop(listener);
}

// NOTE: The following callback tests from the original code cannot be migrated
// because they depend on private or pub(crate) functions:
//   - parse_valid_callback_params (private parse_callback_params)
//   - parse_url_encoded_params (private parse_callback_params)
//   - parse_missing_code_returns_none (private parse_callback_params)
//   - parse_missing_state_returns_none (private parse_callback_params)
//   - parse_no_query_string_returns_none (private parse_callback_params)
//   - parse_empty_request_returns_none (private parse_callback_params)
//   - parse_extra_params_ignored (private parse_callback_params)
//   - bind_err_formats_message (private bind_err)
//   - timeout_err_returns_timeout_message (private timeout_err)
//   - accept_err_formats_message (private accept_err)
//   - read_err_formats_message (private read_err)
//   - callback_server_receives_valid_request (private accept_callback)
//   - callback_server_bad_request_returns_error (private accept_callback)
//   - run_callback_server_timeout_returns_error (pub(crate) run_callback_server_with_timeout)
//
// These tests must remain in the inline #[cfg(test)] module in callback.rs.

// -- service.rs tests --
// NOTE: The service module is private (`mod service;` in oauth/mod.rs).
// Only create_oauth_transport is re-exported as pub. The following functions
// are private and their tests cannot be migrated:
//   - resolve_credential_path
//   - metadata_err
//   - cred_store_err
//   - auth_err
//   - token_err
//   - default_browser
//   - browser_auth
//   - run_authorization_flow
//   - initialize_auth_manager
//   - create_oauth_transport_with (pub(crate))
//
// Tests that CAN be migrated use only create_oauth_transport (the public wrapper):

#[tokio::test]
async fn create_oauth_transport_invalid_url_returns_error() {
    let config = OAuthConfig {
        client_id: None,
        client_secret: None,
        scopes: vec![],
        redirect_port: 9876,
        credentials_file: None,
    };
    let result = create_oauth_transport("not a valid url", &config, "test", HashMap::new()).await;
    let err = result.err().unwrap();
    assert!(matches!(err, OAuthError::MetadataDiscovery { .. }));
}

#[tokio::test]
async fn create_oauth_transport_completes_browser_auth_flow() {
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

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");
    let app = axum::Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(metadata_handler),
        )
        .route("/token", post(token_handler))
        .route("/register", post(register_handler))
        .with_state(MockOAuthState {
            base_url: base_url.clone(),
        });
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });

    let dir = tempfile::tempdir().unwrap();
    let script_path = dir.path().join("fake_browser.sh");
    std::fs::write(
        &script_path,
        "#!/bin/sh\n\
         STATE=$(echo \"$1\" | sed 's/.*state=//;s/&.*//')\n\
         REDIR=$(echo \"$1\" | sed 's/.*redirect_uri=//;s/&.*//')\n\
         REDIR=$(echo \"$REDIR\" | sed 's/%3A/:/g; s/%2F/\\//g')\n\
         curl -s \"${REDIR}?code=test_code&state=${STATE}\" >/dev/null 2>&1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::env::set_var("BROWSER", script_path.to_str().unwrap());

    let redirect_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_port = redirect_listener.local_addr().unwrap().port();
    drop(redirect_listener);

    let config = OAuthConfig {
        client_id: Some("my-app".to_string()),
        client_secret: None,
        scopes: vec!["read".to_string()],
        redirect_port,
        credentials_file: Some(dir.path().join("creds.json").to_string_lossy().to_string()),
    };

    let result = create_oauth_transport(&base_url, &config, "test", HashMap::new()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn create_oauth_transport_with_register_client_flow() {
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
            "client_id": "dynamically_registered",
            "redirect_uris": ["http://127.0.0.1:0"]
        }))
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");
    let app = axum::Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(metadata_handler),
        )
        .route("/token", post(token_handler))
        .route("/register", post(register_handler))
        .with_state(MockOAuthState {
            base_url: base_url.clone(),
        });
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });

    let dir = tempfile::tempdir().unwrap();

    let redirect_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_port = redirect_listener.local_addr().unwrap().port();
    drop(redirect_listener);

    let config = OAuthConfig {
        client_id: None,
        client_secret: None,
        scopes: vec!["read".to_string()],
        redirect_port,
        credentials_file: Some(dir.path().join("creds.json").to_string_lossy().to_string()),
    };

    let result = create_oauth_transport_with(
        &base_url,
        &config,
        "test",
        HashMap::new(),
        |auth_url, _listener| async move {
            // Extract state from the authorization URL
            let url = url::Url::parse(&auth_url).unwrap();
            let state = url
                .query_pairs()
                .find(|(k, _)| k == "state")
                .unwrap()
                .1
                .to_string();

            Ok(CallbackParams {
                code: "test_code".to_string(),
                state,
            })
        },
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn create_oauth_transport_with_bind_conflict_returns_error() {
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

    async fn register_handler() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "client_id": "registered_client",
            "redirect_uris": ["http://127.0.0.1:0"]
        }))
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");
    let app = axum::Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(metadata_handler),
        )
        .route("/register", post(register_handler))
        .with_state(MockOAuthState {
            base_url: base_url.clone(),
        });
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });

    let dir = tempfile::tempdir().unwrap();

    // Bind the redirect port first so create_oauth_transport_with will fail
    let blocker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_port = blocker.local_addr().unwrap().port();

    let config = OAuthConfig {
        client_id: Some("my-app".to_string()),
        client_secret: None,
        scopes: vec!["read".to_string()],
        redirect_port,
        credentials_file: Some(dir.path().join("creds.json").to_string_lossy().to_string()),
    };

    let result = create_oauth_transport_with(
        &base_url,
        &config,
        "test",
        HashMap::new(),
        |_auth_url, _listener| async move {
            unreachable!("interactive callback should not be called when bind fails")
        },
    )
    .await;

    // Keep blocker alive to ensure port conflict
    drop(blocker);

    let err = match result {
        Ok(_) => panic!("expected bind error"),
        Err(e) => e,
    };
    assert!(matches!(err, OAuthError::CallbackServer { .. }));
    assert!(err.to_string().contains("bind to port"));
}

#[tokio::test]
async fn create_oauth_transport_with_stored_credentials_skips_auth_flow() {
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
            "redirect_uris": ["http://127.0.0.1:0"]
        }))
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{port}");
    let app = axum::Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(metadata_handler),
        )
        .route("/token", post(token_handler))
        .route("/register", post(register_handler))
        .with_state(MockOAuthState {
            base_url: base_url.clone(),
        });
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });

    let dir = tempfile::tempdir().unwrap();
    let creds_path = dir.path().join("creds.json");

    let redirect_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_port = redirect_listener.local_addr().unwrap().port();
    drop(redirect_listener);

    let config = OAuthConfig {
        client_id: None,
        client_secret: None,
        scopes: vec!["read".to_string()],
        redirect_port,
        credentials_file: Some(creds_path.to_string_lossy().to_string()),
    };

    // First call: run the auth flow to store credentials
    let result = create_oauth_transport_with(
        &base_url,
        &config,
        "test",
        HashMap::new(),
        |auth_url, _listener| async move {
            let url = url::Url::parse(&auth_url).unwrap();
            let state = url
                .query_pairs()
                .find(|(k, _)| k == "state")
                .unwrap()
                .1
                .to_string();
            Ok(CallbackParams {
                code: "test_code".to_string(),
                state,
            })
        },
    )
    .await;
    assert!(result.is_ok());
    assert!(creds_path.exists());

    // Second call: stored credentials should be found, auth flow skipped
    let result = create_oauth_transport_with(
        &base_url,
        &config,
        "test",
        HashMap::new(),
        |_auth_url, _listener| async move {
            panic!("interactive callback should not be called when credentials are stored")
        },
    )
    .await;
    assert!(result.is_ok());
}
