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
use crate::adapters::driven::oauth;
use crate::config::model::HttpConfig;
use crate::config::model::OAuthConfig;
use crate::config::model::StdioConfig;

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

pub(crate) async fn serve_proxy_http_on_listener<H>(
    handler: Arc<H>,
    listener: tokio::net::TcpListener,
    ct: CancellationToken,
    log_sender: broadcast::Sender<String>,
) -> Result<(), ProxyError>
where
    H: rmcp::ServerHandler + 'static,
{
    let config = StreamableHttpServerConfig {
        cancellation_token: ct.clone(),
        ..Default::default()
    };
    let h = handler;
    let service: StreamableHttpService<Arc<H>, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(Arc::clone(&h)),
        Arc::new(LocalSessionManager::default()),
        config,
    );
    let router = axum::Router::new()
        .route("/logs", get(logs_handler))
        .with_state(log_sender)
        .nest_service("/mcp", service);
    axum::serve(listener, router)
        .with_graceful_shutdown(ct.cancelled_owned())
        .await
        .map_err(downstream_init_err)
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

fn downstream_init_err(e: impl std::fmt::Display) -> ProxyError {
    ProxyError::DownstreamInit {
        message: e.to_string(),
    }
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

pub fn spawn_transport(config: &StdioConfig) -> Result<TokioChildProcess, ProxyError> {
    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(&config.args);
    for (key, value) in &config.env {
        cmd.env(key, value);
    }
    TokioChildProcess::new(cmd).map_err(|e| ProxyError::UpstreamSpawn { source: e })
}

pub fn create_http_transport(
    config: &HttpConfig,
) -> Result<StreamableHttpClientTransport<reqwest::Client>, ProxyError> {
    let mut custom_headers = HashMap::new();
    for (key, value) in &config.headers {
        let header_name =
            http::HeaderName::try_from(key.as_str()).map_err(|e| ProxyError::HttpTransport {
                message: e.to_string(),
            })?;
        let header_value =
            http::HeaderValue::try_from(value.as_str()).map_err(|e| ProxyError::HttpTransport {
                message: e.to_string(),
            })?;
        custom_headers.insert(header_name, header_value);
    }
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
    let mut custom_headers = HashMap::new();
    for (key, value) in &config.headers {
        let header_name = http::HeaderName::try_from(key.as_str()).map_err(http_transport_err)?;
        let header_value =
            http::HeaderValue::try_from(value.as_str()).map_err(http_transport_err)?;
        custom_headers.insert(header_name, header_value);
    }

    let default_config = OAuthConfig::default();
    let oauth_config = config.auth.as_ref().unwrap_or(&default_config);

    Ok(
        oauth::create_oauth_transport(&config.url, oauth_config, server_name, custom_headers)
            .await?,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::adapters::driven::{NullCliRunner, RmcpUpstreamClient};
    use crate::adapters::driving::McpAdapter;
    use crate::config::model::HttpConfig;
    use crate::hexagon::usecases::{Gateway, UpstreamEntry};
    use rmcp::model::*;
    use rmcp::ServerHandler;
    use std::collections::BTreeMap;

    #[test]
    fn spawn_transport_invalid_command_returns_error() {
        let config = StdioConfig {
            command: "/nonexistent/path/to/binary".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_tools: vec![],
            denied_tools: vec![],
        };
        let result = spawn_transport(&config);
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
            allowed_tools: vec![],
            denied_tools: vec![],
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
            allowed_tools: vec![],
            denied_tools: vec![],
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
            allowed_tools: vec![],
            denied_tools: vec![],
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
            allowed_tools: vec![],
            denied_tools: vec![],
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
            allowed_tools: vec![],
            denied_tools: vec![],
        };
        let result = spawn_transport(&config);
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

    use crate::adapters::driven::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};

    type TestFilter = CompoundFilter<AllowlistFilter, DenylistFilter>;

    fn passthrough_filter() -> TestFilter {
        CompoundFilter::new(AllowlistFilter::new(vec![]), DenylistFilter::new(vec![]))
    }

    fn empty_adapter() -> Arc<McpAdapter<RmcpUpstreamClient, NullCliRunner, TestFilter>> {
        let gateway = Gateway::new(BTreeMap::new(), NullCliRunner);
        Arc::new(McpAdapter::new(gateway))
    }

    fn adapter_with_upstreams(
        upstreams: BTreeMap<String, UpstreamEntry<RmcpUpstreamClient, TestFilter>>,
    ) -> Arc<McpAdapter<RmcpUpstreamClient, NullCliRunner, TestFilter>> {
        let gateway = Gateway::new(upstreams, NullCliRunner);
        Arc::new(McpAdapter::new(gateway))
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
            UpstreamEntry {
                client: RmcpUpstreamClient::new(upstream),
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
            UpstreamEntry {
                client: RmcpUpstreamClient::new(upstream),
                filter: passthrough_filter(),
            },
        );
        let handler = adapter_with_upstreams(upstreams);

        let proxy_handle =
            tokio::spawn(async move { serve_proxy(handler, downstream_server_t).await });

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
            allowed_tools: vec![],
            denied_tools: vec![],
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
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: Some(crate::config::model::OAuthConfig {
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
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: Some(crate::config::model::OAuthConfig {
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

    #[test]
    fn http_transport_err_formats_message() {
        let err = http_transport_err("bad header");
        assert!(matches!(err, ProxyError::HttpTransport { .. }));
        assert!(err.to_string().contains("bad header"));
    }

    #[tokio::test]
    async fn create_oauth_http_transport_invalid_header_value_returns_error() {
        let config = HttpConfig {
            url: "http://localhost:8080/mcp".to_string(),
            headers: BTreeMap::from([("X-Custom".to_string(), "bad\nvalue".to_string())]),
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: Some(crate::config::model::OAuthConfig {
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
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: Some(crate::config::model::OAuthConfig {
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

    fn dummy_log_sender() -> broadcast::Sender<String> {
        broadcast::channel::<String>(16).0
    }

    #[tokio::test]
    async fn serve_proxy_http_on_listener_starts_and_cancels() {
        let handler = empty_adapter();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let ct = CancellationToken::new();
        let ct2 = ct.clone();
        let sender = dummy_log_sender();
        let handle = tokio::spawn(async move {
            serve_proxy_http_on_listener(handler, listener, ct2, sender).await
        });
        ct.cancel();
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn serve_proxy_http_on_listener_accepts_mcp_client() {
        let handler = empty_adapter();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let ct = CancellationToken::new();
        let ct2 = ct.clone();
        let sender = dummy_log_sender();
        let handle = tokio::spawn(async move {
            serve_proxy_http_on_listener(handler, listener, ct2, sender).await
        });

        let url = format!("http://127.0.0.1:{port}/mcp");
        let transport_config =
            rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(
                &*url,
            );
        let transport =
            rmcp::transport::StreamableHttpClientTransport::from_config(transport_config);
        let client: rmcp::service::RunningService<rmcp::RoleClient, ()> =
            ().serve(transport).await.unwrap();
        let tools = client.list_tools(None).await.unwrap();
        assert!(tools.tools.is_empty());
        ct.cancel();
        let _ = handle.await;
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

    async fn start_logs_server(
        sender: broadcast::Sender<String>,
    ) -> (u16, CancellationToken, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = axum::Router::new()
            .route("/logs", get(logs_handler))
            .with_state(sender);
        let ct = CancellationToken::new();
        let ct_inner = ct.clone();
        let handle = tokio::spawn(async move {
            tokio::select! {
                result = axum::serve(listener, router) => { let _ = result; }
                () = ct_inner.cancelled() => {}
            }
        });
        (port, ct, handle)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn logs_endpoint_streams_broadcast_events() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (sender, _) = broadcast::channel::<String>(16);
        let (port, ct, handle) = start_logs_server(sender.clone()).await;

        let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        tcp.write_all(
            b"GET /logs HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 4096];
        let n = tokio::time::timeout(std::time::Duration::from_secs(1), tcp.read(&mut buf))
            .await
            .unwrap()
            .unwrap();
        let headers = String::from_utf8_lossy(&buf[..n]);
        assert!(headers.contains("200 OK"));
        assert!(headers.contains("text/event-stream"));

        sender.send("test log line".to_string()).unwrap();
        let n = tokio::time::timeout(std::time::Duration::from_secs(1), tcp.read(&mut buf))
            .await
            .unwrap()
            .unwrap();
        let body = String::from_utf8_lossy(&buf[..n]);
        assert!(body.contains("data: test log line"));

        ct.cancel();
        let _ = handle.await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn logs_endpoint_skips_lagged_messages() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Channel capacity = 1 so sending 2 messages before reading causes lag
        let (sender, _) = broadcast::channel::<String>(1);
        let (port, ct, handle) = start_logs_server(sender.clone()).await;

        let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        tcp.write_all(
            b"GET /logs HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\n\r\n",
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 4096];
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), tcp.read(&mut buf))
            .await
            .unwrap()
            .unwrap();

        // Send 2 messages with capacity=1 to cause lagged error on receiver
        sender.send("first".to_string()).unwrap();
        sender.send("second".to_string()).unwrap();

        // Read until we see "data: second" (may arrive in separate TCP reads)
        let mut accumulated = String::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        while !accumulated.contains("data: second") {
            let n = tokio::time::timeout_at(deadline, tcp.read(&mut buf))
                .await
                .unwrap()
                .unwrap();
            accumulated.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
        assert!(accumulated.contains("data: second"));

        ct.cancel();
        let _ = handle.await;
    }

    #[test]
    fn downstream_init_err_formats_message() {
        let err = downstream_init_err("test error");
        assert!(matches!(err, ProxyError::DownstreamInit { .. }));
        assert!(err.to_string().contains("test error"));
    }
}
