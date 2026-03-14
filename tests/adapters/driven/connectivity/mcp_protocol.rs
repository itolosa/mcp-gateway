use std::collections::BTreeMap;
use std::sync::Arc;

use mcp_gateway::adapters::driven::cli_operation_runner::NullCliRunner;
use mcp_gateway::adapters::driven::configuration::error::ConfigError;
use mcp_gateway::adapters::driven::configuration::model::{HttpConfig, OAuthConfig, StdioConfig};
use mcp_gateway::adapters::driven::connectivity::mcp_protocol::error::ProxyError;
use mcp_gateway::adapters::driven::connectivity::mcp_protocol::proxy::{
    create_http_transport, create_oauth_http_transport, gateway_router, serve_proxy,
    serve_proxy_http, spawn_transport,
};
use mcp_gateway::adapters::driven::connectivity::mcp_protocol::McpAdapter;
use mcp_gateway::adapters::driven::connectivity::oauth::OAuthError;
use mcp_gateway::adapters::driven::provider_client::RmcpProviderClient;
use mcp_gateway::hexagon::usecases::gateway::{
    create_policy, DefaultPolicy, Gateway, ProviderHandle,
};
use mcp_gateway::hexagon::usecases::registry_error::RegistryError;
use rmcp::model::*;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

// -- downstream.rs tests (gateway_error_to_mcp via MCP transport) --

use crate::common::gateway_helpers::MockServerA;

fn mcp_error_code(err: rmcp::service::ServiceError) -> ErrorCode {
    match err {
        rmcp::service::ServiceError::McpError(e) => e.code,
        other => panic!("expected McpError, got: {other}"),
    }
}

async fn connect_mcp_client(port: u16) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let url: Arc<str> = format!("http://127.0.0.1:{port}/mcp").into();
    let config =
        rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(url);
    let transport = rmcp::transport::StreamableHttpClientTransport::from_config(config);
    ().serve(transport).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_tool_invalid_mapping_returns_invalid_params() {
    let adapter = empty_adapter();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let ct = CancellationToken::new();
    let ct2 = ct.clone();
    let sender = dummy_log_sender();
    tokio::spawn(async move { serve_proxy_http(adapter, port, ct2, sender).await });
    drop(listener);

    let client = connect_mcp_client(port).await;
    let request = CallToolRequestParams::new("no_prefix");
    let err = client.call_tool(request).await.unwrap_err();
    assert_eq!(mcp_error_code(err), ErrorCode::INVALID_PARAMS);
    drop(client);
    ct.cancel();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_tool_unknown_provider_returns_invalid_params() {
    let adapter = empty_adapter();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let ct = CancellationToken::new();
    let ct2 = ct.clone();
    let sender = dummy_log_sender();
    tokio::spawn(async move { serve_proxy_http(adapter, port, ct2, sender).await });
    drop(listener);

    let client = connect_mcp_client(port).await;
    let request = CallToolRequestParams::new("nonexistent__tool");
    let err = client.call_tool(request).await.unwrap_err();
    assert_eq!(mcp_error_code(err), ErrorCode::INVALID_PARAMS);
    drop(client);
    ct.cancel();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_tool_operation_not_allowed_returns_invalid_params() {
    let filter = create_policy(vec![], vec!["echo".to_string()]);
    let handle = ProviderHandle {
        client: MockServerA,
        filter,
    };
    let providers = BTreeMap::from([("srv".to_string(), handle)]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let adapter = Arc::new(McpAdapter::new(gateway));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let ct = CancellationToken::new();
    let ct2 = ct.clone();
    let sender = dummy_log_sender();
    tokio::spawn(async move { serve_proxy_http(adapter, port, ct2, sender).await });
    drop(listener);

    let client = connect_mcp_client(port).await;
    let request = CallToolRequestParams::new("srv__echo");
    let err = client.call_tool(request).await.unwrap_err();
    assert_eq!(mcp_error_code(err), ErrorCode::INVALID_PARAMS);
    drop(client);
    ct.cancel();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_tool_provider_error_returns_internal_error() {
    let filter = passthrough_filter();
    let handle = ProviderHandle {
        client: MockServerA,
        filter,
    };
    let providers = BTreeMap::from([("srv".to_string(), handle)]);
    let gateway = Gateway::new(providers, NullCliRunner);
    let adapter = Arc::new(McpAdapter::new(gateway));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let ct = CancellationToken::new();
    let ct2 = ct.clone();
    let sender = dummy_log_sender();
    tokio::spawn(async move { serve_proxy_http(adapter, port, ct2, sender).await });
    drop(listener);

    let client = connect_mcp_client(port).await;
    let request = CallToolRequestParams::new("srv__nonexistent");
    let err = client.call_tool(request).await.unwrap_err();
    assert_eq!(mcp_error_code(err), ErrorCode::INTERNAL_ERROR);
    drop(client);
    ct.cancel();
}

// -- error.rs tests (ProxyError) --

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
fn port_in_use_display() {
    let err = ProxyError::PortInUse {
        port: 8080,
        message: "address in use".to_string(),
    };
    assert!(err.to_string().contains("8080"));
    assert!(err.to_string().contains("address in use"));
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

// -- proxy.rs tests --

fn passthrough_filter() -> DefaultPolicy {
    create_policy(vec![], vec![])
}

fn empty_adapter() -> Arc<McpAdapter<RmcpProviderClient, NullCliRunner, DefaultPolicy>> {
    let gateway = Gateway::new(BTreeMap::new(), NullCliRunner);
    Arc::new(McpAdapter::new(gateway))
}

fn adapter_with_upstreams(
    upstreams: BTreeMap<String, ProviderHandle<RmcpProviderClient, DefaultPolicy>>,
) -> Arc<McpAdapter<RmcpProviderClient, NullCliRunner, DefaultPolicy>> {
    let gateway = Gateway::new(upstreams, NullCliRunner);
    Arc::new(McpAdapter::new(gateway))
}

fn dummy_log_sender() -> broadcast::Sender<String> {
    broadcast::channel::<String>(16).0
}

#[test]
fn spawn_transport_invalid_command_returns_error() {
    let config = StdioConfig {
        command: "/nonexistent/path/to/binary".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
    };
    let result = spawn_transport(&config, false);
    assert!(matches!(result, Err(ProxyError::UpstreamSpawn { .. })));
}

#[tokio::test]
async fn create_http_transport_valid_config_succeeds() {
    let config = HttpConfig {
        url: "http://localhost:8080/mcp".to_string(),
        headers: BTreeMap::from([
            ("Authorization".to_string(), "Bearer token123".to_string()),
            ("X-Custom".to_string(), "value".to_string()),
        ]),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    };
    let result = create_http_transport(&config);
    assert!(result.is_ok());
}

#[tokio::test]
async fn create_http_transport_empty_headers_succeeds() {
    let config = HttpConfig {
        url: "http://localhost:8080/mcp".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    };
    let result = create_http_transport(&config);
    assert!(result.is_ok());
}

#[test]
fn create_http_transport_invalid_header_name_returns_error() {
    let config = HttpConfig {
        url: "http://localhost:8080/mcp".to_string(),
        headers: BTreeMap::from([("bad\nname".to_string(), "value".to_string())]),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    };
    let result = create_http_transport(&config);
    assert!(matches!(result, Err(ProxyError::HttpTransport { .. })));
}

#[test]
fn create_http_transport_invalid_header_value_returns_error() {
    let config = HttpConfig {
        url: "http://localhost:8080/mcp".to_string(),
        headers: BTreeMap::from([("X-Custom".to_string(), "bad\nvalue".to_string())]),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    };
    let result = create_http_transport(&config);
    assert!(matches!(result, Err(ProxyError::HttpTransport { .. })));
}

#[tokio::test]
async fn spawn_transport_with_args_and_env() {
    let config = StdioConfig {
        command: "cat".to_string(),
        args: vec!["--help".to_string()],
        env: BTreeMap::from([("MY_VAR".to_string(), "value".to_string())]),
        allowed_operations: vec![],
        denied_operations: vec![],
    };
    let result = spawn_transport(&config, true);
    assert!(result.is_ok());
}

struct MinimalServer;

impl ServerHandler for MinimalServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult {
            tools: vec![],
            next_cursor: None,
            meta: None,
        })
    }
}

#[tokio::test]
async fn serve_proxy_downstream_init_error() {
    let (upstream_server_t, upstream_client_t) = tokio::io::duplex(4096);

    let upstream_handle = tokio::spawn(async move {
        let s = MinimalServer.serve(upstream_server_t).await.unwrap();
        let _ = s.waiting().await;
    });

    let upstream = ().serve(upstream_client_t).await.unwrap();

    let (downstream_server_t, downstream_client_t) = tokio::io::duplex(4096);
    drop(downstream_client_t);

    let mut upstreams = BTreeMap::new();
    upstreams.insert(
        "test".to_string(),
        ProviderHandle {
            client: RmcpProviderClient::new(upstream),
            filter: passthrough_filter(),
        },
    );
    let handler = adapter_with_upstreams(upstreams);

    let result = serve_proxy(handler, downstream_server_t).await;
    assert!(matches!(result, Err(ProxyError::DownstreamInit { .. })));

    let _ = upstream_handle.await;
}

#[tokio::test]
async fn serve_proxy_forwards_and_exits_on_disconnect() {
    let (upstream_server_t, upstream_client_t) = tokio::io::duplex(4096);
    let (downstream_server_t, downstream_client_t) = tokio::io::duplex(4096);

    let upstream_handle = tokio::spawn(async move {
        let s = MinimalServer.serve(upstream_server_t).await.unwrap();
        let _ = s.waiting().await;
    });

    let upstream = ().serve(upstream_client_t).await.unwrap();

    let mut upstreams = BTreeMap::new();
    upstreams.insert(
        "test".to_string(),
        ProviderHandle {
            client: RmcpProviderClient::new(upstream),
            filter: passthrough_filter(),
        },
    );
    let handler = adapter_with_upstreams(upstreams);

    let proxy_handle = tokio::spawn(async move { serve_proxy(handler, downstream_server_t).await });

    let client = ().serve(downstream_client_t).await.unwrap();
    let tools = client.list_tools(None).await.unwrap();
    assert!(tools.tools.is_empty());

    // Also exercise call_tool to cover error paths for this type instantiation
    let result = client
        .call_tool(rmcp::model::CallToolRequestParams::new("no_prefix"))
        .await;
    assert!(result.is_err());

    let result = client
        .call_tool(rmcp::model::CallToolRequestParams::new("unknown__tool"))
        .await;
    assert!(result.is_err());

    drop(client);

    let result = proxy_handle.await.unwrap();
    assert!(result.is_ok());

    let _ = upstream_handle.await;
}

#[tokio::test]
async fn create_oauth_http_transport_no_auth_config_uses_default() {
    let config = HttpConfig {
        url: "not a valid url".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    };
    let result = create_oauth_http_transport(&config, "test").await;
    assert!(matches!(result, Err(ProxyError::OAuthAuth { .. })));
}

#[tokio::test]
async fn create_oauth_http_transport_invalid_url_returns_error() {
    let config = HttpConfig {
        url: "not a valid url".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: Some(OAuthConfig {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: None,
        }),
    };
    let result = create_oauth_http_transport(&config, "test").await;
    assert!(matches!(result, Err(ProxyError::OAuthAuth { .. })));
}

#[tokio::test]
async fn create_oauth_http_transport_valid_headers_fails_on_oauth() {
    let config = HttpConfig {
        url: "not a valid url".to_string(),
        headers: BTreeMap::from([
            ("Authorization".to_string(), "Bearer token".to_string()),
            ("X-Custom".to_string(), "value".to_string()),
        ]),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: Some(OAuthConfig {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: None,
        }),
    };
    let result = create_oauth_http_transport(&config, "test").await;
    assert!(matches!(result, Err(ProxyError::OAuthAuth { .. })));
}

// NOTE: http_transport_err is a private function in proxy.rs. Its behavior
// is tested indirectly through create_oauth_http_transport_invalid_header_*
// tests below, which trigger the same error path.

#[tokio::test]
async fn create_oauth_http_transport_invalid_header_value_returns_error() {
    let config = HttpConfig {
        url: "http://localhost:8080/mcp".to_string(),
        headers: BTreeMap::from([("X-Custom".to_string(), "bad\nvalue".to_string())]),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: Some(OAuthConfig {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: None,
        }),
    };
    let result = create_oauth_http_transport(&config, "test").await;
    assert!(matches!(result, Err(ProxyError::HttpTransport { .. })));
}

#[tokio::test]
async fn create_oauth_http_transport_invalid_header_returns_error() {
    let config = HttpConfig {
        url: "http://localhost:8080/mcp".to_string(),
        headers: BTreeMap::from([("bad\nname".to_string(), "value".to_string())]),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: Some(OAuthConfig {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: 9876,
            credentials_file: None,
        }),
    };
    let result = create_oauth_http_transport(&config, "test").await;
    assert!(matches!(result, Err(ProxyError::HttpTransport { .. })));
}

// NOTE: serve_proxy_http_on_listener is pub(crate) and cannot be called from
// integration tests. The following tests exercise the same code paths through
// the public serve_proxy_http function instead.

#[tokio::test]
async fn serve_proxy_http_starts_on_free_port_and_cancels() {
    let handler = empty_adapter();
    let ct = CancellationToken::new();
    let ct2 = ct.clone();
    let sender = dummy_log_sender();
    let handle = tokio::spawn(async move { serve_proxy_http(handler, 0, ct2, sender).await });
    ct.cancel();
    let result = handle.await.unwrap();
    assert!(result.is_ok());
}

#[tokio::test]
async fn serve_proxy_http_port_in_use_returns_error() {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handler = empty_adapter();
    let ct = CancellationToken::new();
    let sender = dummy_log_sender();
    let result = serve_proxy_http(handler, port, ct, sender).await;
    assert!(matches!(result, Err(ProxyError::PortInUse { .. })));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn serve_proxy_http_accepts_mcp_client_connection() {
    use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
    use rmcp::transport::StreamableHttpClientTransport;

    // Find a free port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let handler = empty_adapter();
    let ct = CancellationToken::new();
    let ct2 = ct.clone();
    let sender = dummy_log_sender();
    let handle = tokio::spawn(async move { serve_proxy_http(handler, port, ct2, sender).await });

    // Wait for server to be ready
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Connect an MCP client via HTTP — exercises the session factory closure
    let url: Arc<str> = format!("http://127.0.0.1:{port}/mcp").into();
    let transport_config = StreamableHttpClientTransportConfig::with_uri(url);
    let transport = StreamableHttpClientTransport::from_config(transport_config);
    let client = ().serve(transport).await.unwrap();

    let result = client.list_tools(None).await.unwrap();
    assert!(result.tools.is_empty());

    drop(client);
    ct.cancel();
    let _ = handle.await;
}

// -- gateway_router handler tests (via tower::ServiceExt::oneshot) --

use http::{Request, StatusCode};
use tower::ServiceExt as TowerServiceExt;

async fn response_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn oauth_metadata_returns_endpoints_for_remote_host() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .uri("/.well-known/oauth-authorization-server")
        .header("host", "example.com:8080")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body(response).await;
    assert_eq!(body["issuer"], "http://example.com:8080");
    assert_eq!(
        body["authorization_endpoint"],
        "http://example.com:8080/authorize"
    );
    assert_eq!(body["token_endpoint"], "http://example.com:8080/token");
    assert_eq!(
        body["registration_endpoint"],
        "http://example.com:8080/register"
    );
}

#[tokio::test]
async fn oauth_metadata_rewrites_localhost_to_docker_host() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .uri("/.well-known/oauth-authorization-server")
        .header("host", "localhost:8080")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let body = response_body(response).await;
    assert_eq!(body["issuer"], "http://host.docker.internal:8080");
    assert_eq!(
        body["authorization_endpoint"],
        "http://host.docker.internal:8080/authorize"
    );
}

#[tokio::test]
async fn oauth_metadata_rewrites_127_to_docker_host() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .uri("/.well-known/oauth-authorization-server")
        .header("host", "127.0.0.1:9090")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let body = response_body(response).await;
    assert_eq!(body["issuer"], "http://host.docker.internal:9090");
}

#[tokio::test]
async fn register_handler_returns_client_credentials() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .method("POST")
        .uri("/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({
                "client_name": "my-app",
                "redirect_uris": ["http://localhost/callback"]
            })
            .to_string(),
        ))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body(response).await;
    assert_eq!(body["client_id"], "my-app-local");
    assert_eq!(body["client_secret"], "not-a-secret");
    assert_eq!(body["redirect_uris"][0], "http://localhost/callback");
}

#[tokio::test]
async fn register_handler_defaults_client_name() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .method("POST")
        .uri("/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::from("{}"))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let body = response_body(response).await;
    assert_eq!(body["client_id"], "mcp-client-local");
}

#[tokio::test]
async fn authorize_handler_redirects_with_code_and_state() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .uri("/authorize?redirect_uri=http%3A%2F%2Flocalhost%2Fcallback&state=abc123")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("code=local-grant"));
    assert!(location.contains("state=abc123"));
}

#[tokio::test]
async fn authorize_handler_defaults_empty_params() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .uri("/authorize")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.contains("code=local-grant"));
    assert!(location.contains("state="));
}

#[tokio::test]
async fn token_handler_returns_bearer_token() {
    let router = gateway_router(dummy_log_sender());
    let request = Request::builder()
        .method("POST")
        .uri("/token")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_body(response).await;
    assert_eq!(body["access_token"], "mcp-gateway-local");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["expires_in"], 86400);
}

#[tokio::test]
async fn logs_handler_streams_broadcast_events() {
    let (sender, _receiver) = broadcast::channel::<String>(16);
    let router = gateway_router(sender.clone());
    let request = Request::builder()
        .uri("/logs")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/event-stream"));

    // Send a message and drop sender so the SSE stream terminates
    sender.send("hello from test".to_string()).unwrap();
    drop(sender);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("hello from test"));
}

#[tokio::test]
async fn logs_handler_skips_lagged_messages() {
    // Buffer of 1 — sending 2 messages causes the first to be lagged
    let (sender, _receiver) = broadcast::channel::<String>(1);
    let router = gateway_router(sender.clone());
    let request = Request::builder()
        .uri("/logs")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();

    // Overflow the buffer to cause lag
    sender.send("first".to_string()).unwrap();
    sender.send("second".to_string()).unwrap();
    sender.send("third".to_string()).unwrap();
    drop(sender);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    // At least the last message should be present; lagged ones are skipped
    assert!(text.contains("third"));
}
