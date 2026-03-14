use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;

use super::error::ProxyError;
use crate::adapters::driven::configuration::model::HttpConfig;
use crate::adapters::driven::configuration::model::OAuthConfig;
use crate::adapters::driven::configuration::model::StdioConfig;
use crate::adapters::driven::connectivity::oauth;

pub async fn serve_proxy<H, T, E, A>(
    handler: Arc<H>,
    downstream_transport: T,
) -> Result<(), ProxyError>
where
    H: rmcp::ServerHandler,
    T: rmcp::transport::IntoTransport<rmcp::RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let service =
        handler
            .serve(downstream_transport)
            .await
            .map_err(|e| ProxyError::DownstreamInit {
                message: e.to_string(),
            })?;
    let _ = service.waiting().await;
    Ok(())
}

pub fn gateway_router(log_sender: broadcast::Sender<String>) -> axum::Router {
    axum::Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_metadata_handler),
        )
        .route("/register", axum::routing::post(register_handler))
        .route("/authorize", get(authorize_handler))
        .route("/token", axum::routing::post(token_handler))
        .route("/logs", get(logs_handler))
        .with_state(log_sender)
}

pub(crate) async fn serve_proxy_http_on_listener<H>(
    handler: Arc<H>,
    listener: tokio::net::TcpListener,
    ct: CancellationToken,
    log_sender: broadcast::Sender<String>,
) -> Result<(), ProxyError>
where
    H: rmcp::ServerHandler + 'static,
{
    let defaults = StreamableHttpServerConfig::default();
    let config = StreamableHttpServerConfig {
        cancellation_token: ct.clone(),
        sse_keep_alive: defaults.sse_keep_alive,
        sse_retry: defaults.sse_retry,
        stateful_mode: defaults.stateful_mode,
        json_response: defaults.json_response,
    };
    let h = handler;
    let service: StreamableHttpService<Arc<H>, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(Arc::clone(&h)),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let router = gateway_router(log_sender).nest_service("/mcp", service);
    #[rustfmt::skip]
    let result = axum::serve(listener, router).with_graceful_shutdown(ct.cancelled_owned()).await.map_err(|e| ProxyError::DownstreamInit { message: e.to_string() });
    result
}

async fn oauth_metadata_handler(
    request: axum::extract::Request,
) -> axum::response::Json<serde_json::Value> {
    let host = request
        .headers()
        .get("x-forwarded-host")
        .or_else(|| request.headers().get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let base = if host.contains("localhost") || host.starts_with("127.0.0.1") {
        format!(
            "http://host.docker.internal:{}",
            host.split(':').next_back().unwrap_or("8080")
        )
    } else {
        format!("http://{host}")
    };
    axum::response::Json(serde_json::json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"),
        "registration_endpoint": format!("{base}/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "token_endpoint_auth_methods_supported": ["none"],
        "code_challenge_methods_supported": ["S256"]
    }))
}

async fn register_handler(
    body: axum::extract::Json<serde_json::Value>,
) -> axum::response::Json<serde_json::Value> {
    let client_name = body
        .get("client_name")
        .and_then(|v| v.as_str())
        .unwrap_or("mcp-client");
    let redirect_uris = body
        .get("redirect_uris")
        .cloned()
        .unwrap_or(serde_json::json!([]));
    axum::response::Json(serde_json::json!({
        "client_id": format!("{client_name}-local"),
        "client_secret": "not-a-secret",
        "redirect_uris": redirect_uris,
        "token_endpoint_auth_method": "none"
    }))
}

async fn authorize_handler(
    query: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Redirect {
    let redirect_uri = query.get("redirect_uri").cloned().unwrap_or_default();
    let state = query.get("state").cloned().unwrap_or_default();
    let redirect = format!("{redirect_uri}?code=local-grant&state={state}");
    axum::response::Redirect::temporary(&redirect)
}

async fn token_handler() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({
        "access_token": "mcp-gateway-local",
        "token_type": "Bearer",
        "expires_in": 86400
    }))
}

async fn logs_handler(
    State(sender): State<broadcast::Sender<String>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let receiver = sender.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|result| match result {
        Ok(line) => Some(Ok::<_, Infallible>(Event::default().data(line))),
        Err(_) => None,
    });
    Sse::new(stream)
}

pub async fn serve_proxy_http<H: rmcp::ServerHandler + 'static>(
    handler: Arc<H>,
    port: u16,
    ct: CancellationToken,
    log_sender: broadcast::Sender<String>,
) -> Result<(), ProxyError> {
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|e| ProxyError::PortInUse {
            port,
            message: e.to_string(),
        })?;
    serve_proxy_http_on_listener(handler, listener, ct, log_sender).await
}

pub fn spawn_transport(
    config: &StdioConfig,
    inherit_stderr: bool,
) -> Result<TokioChildProcess, ProxyError> {
    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(&config.args);
    for (key, value) in &config.env {
        cmd.env(key, value);
    }
    let stderr = if inherit_stderr {
        std::process::Stdio::inherit()
    } else {
        std::process::Stdio::null()
    };
    let (process, _) = TokioChildProcess::builder(cmd)
        .stderr(stderr)
        .spawn()
        .map_err(|e| ProxyError::UpstreamSpawn { source: e })?;
    Ok(process)
}

pub fn create_http_transport(
    config: &HttpConfig,
) -> Result<StreamableHttpClientTransport<reqwest::Client>, ProxyError> {
    let custom_headers = config
        .headers
        .iter()
        .map(|(key, value)| {
            let header_name = http::HeaderName::try_from(key.as_str()).map_err(|e| {
                ProxyError::HttpTransport {
                    message: e.to_string(),
                }
            })?;
            let header_value = http::HeaderValue::try_from(value.as_str()).map_err(|e| {
                ProxyError::HttpTransport {
                    message: e.to_string(),
                }
            })?;
            Ok((header_name, header_value))
        })
        .collect::<Result<HashMap<_, _>, ProxyError>>()?;
    let transport_config = StreamableHttpClientTransportConfig::with_uri(config.url.as_str())
        .custom_headers(custom_headers);
    Ok(StreamableHttpClientTransport::from_config(transport_config))
}

fn http_transport_err(e: impl std::fmt::Display) -> ProxyError {
    ProxyError::HttpTransport {
        message: e.to_string(),
    }
}

pub async fn create_oauth_http_transport(
    config: &HttpConfig,
    server_name: &str,
) -> Result<StreamableHttpClientTransport<AuthClient<reqwest::Client>>, ProxyError> {
    let custom_headers = config
        .headers
        .iter()
        .map(|(key, value)| {
            let header_name =
                http::HeaderName::try_from(key.as_str()).map_err(http_transport_err)?;
            let header_value =
                http::HeaderValue::try_from(value.as_str()).map_err(http_transport_err)?;
            Ok((header_name, header_value))
        })
        .collect::<Result<HashMap<_, _>, ProxyError>>()?;

    let default_config = OAuthConfig::default();
    let oauth_config = config.auth.as_ref().unwrap_or(&default_config);

    Ok(
        oauth::create_oauth_transport(&config.url, oauth_config, server_name, custom_headers)
            .await?,
    )
}
