use std::collections::HashMap;

use rmcp::transport::auth::AuthClient;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::ServiceExt;

use crate::cli_tools::CliToolExecutor;
use crate::config::model::HttpConfig;
use crate::config::model::StdioConfig;
use crate::filter::ToolFilter;
use crate::oauth;
use crate::proxy::error::ProxyError;
use crate::proxy::handler::ProxyHandler;

pub async fn serve_proxy<T, E, A, F>(
    upstream: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    downstream_transport: T,
    filter: F,
    cli_tools: Option<CliToolExecutor>,
) -> Result<(), ProxyError>
where
    T: rmcp::transport::IntoTransport<rmcp::RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
    F: ToolFilter + 'static,
{
    let proxy = ProxyHandler::new(upstream, filter, cli_tools)?;
    let service =
        proxy
            .serve(downstream_transport)
            .await
            .map_err(|e| ProxyError::DownstreamInit {
                message: e.to_string(),
            })?;
    let _ = service.waiting().await;
    Ok(())
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

    let oauth_config = config.auth.as_ref().ok_or_else(|| ProxyError::OAuthAuth {
        message: "missing OAuth config".to_string(),
    })?;

    Ok(
        oauth::create_oauth_transport(&config.url, oauth_config, server_name, custom_headers)
            .await?,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::model::HttpConfig;
    use crate::filter::AllowlistFilter;
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
            ServerInfo {
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                ..Default::default()
            }
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

        // Start mock upstream server
        let upstream_handle = tokio::spawn(async move {
            let s = MinimalServer.serve(upstream_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        // Connect upstream client
        let upstream = ().serve(upstream_client_t).await.unwrap();

        // Create a downstream transport that immediately closes
        let (downstream_server_t, downstream_client_t) = tokio::io::duplex(4096);
        drop(downstream_client_t); // Close immediately

        let result = serve_proxy(
            upstream,
            downstream_server_t,
            AllowlistFilter::new(vec![]),
            None,
        )
        .await;
        assert!(matches!(result, Err(ProxyError::DownstreamInit { .. })));

        // Wait for upstream mock to shut down cleanly
        let _ = upstream_handle.await;
    }

    #[tokio::test]
    async fn serve_proxy_forwards_and_exits_on_disconnect() {
        let (upstream_server_t, upstream_client_t) = tokio::io::duplex(4096);
        let (downstream_server_t, downstream_client_t) = tokio::io::duplex(4096);

        // Start mock upstream server
        let upstream_handle = tokio::spawn(async move {
            let s = MinimalServer.serve(upstream_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        // Connect upstream client
        let upstream = ().serve(upstream_client_t).await.unwrap();

        // Start proxy in background
        let proxy_handle = tokio::spawn(async move {
            serve_proxy(
                upstream,
                downstream_server_t,
                AllowlistFilter::new(vec![]),
                None,
            )
            .await
        });

        // Connect downstream client, verify it works, then disconnect
        let client = ().serve(downstream_client_t).await.unwrap();
        let tools = client.list_tools(None).await.unwrap();
        assert!(tools.tools.is_empty());

        // Drop the client which closes the downstream transport
        drop(client);

        // Proxy should exit cleanly
        let result = proxy_handle.await.unwrap();
        assert!(result.is_ok());

        // Wait for upstream mock to shut down cleanly
        let _ = upstream_handle.await;
    }

    #[tokio::test]
    async fn create_oauth_http_transport_missing_auth_config_returns_error() {
        let config = HttpConfig {
            url: "http://localhost:8080/mcp".to_string(),
            headers: BTreeMap::new(),
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: None,
        };
        let result = create_oauth_http_transport(&config, "test").await;
        let err = result.err().unwrap();
        assert!(matches!(err, ProxyError::OAuthAuth { .. }));
        assert!(err.to_string().contains("missing OAuth config"));
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
}
