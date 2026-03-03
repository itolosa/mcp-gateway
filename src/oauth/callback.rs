use std::time::Duration;

use crate::oauth::error::OAuthError;

#[derive(Debug)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

const SUCCESS_RESPONSE: &str = "\
HTTP/1.1 200 OK\r\n\
Content-Type: text/html\r\n\
Connection: close\r\n\
\r\n\
<html><body><h1>Authorization successful</h1><p>You can close this tab.</p></body></html>";

const BAD_REQUEST_RESPONSE: &str = "\
HTTP/1.1 400 Bad Request\r\n\
Content-Type: text/html\r\n\
Connection: close\r\n\
\r\n\
<html><body><h1>Bad Request</h1><p>Missing code or state parameter.</p></body></html>";

fn bind_err(port: u16, e: std::io::Error) -> OAuthError {
    OAuthError::CallbackServer {
        message: format!("bind to port {port}: {e}"),
    }
}

fn timeout_err() -> OAuthError {
    OAuthError::CallbackServer {
        message: "timed out waiting for authorization callback".to_string(),
    }
}

fn accept_err(e: std::io::Error) -> OAuthError {
    OAuthError::CallbackServer {
        message: format!("accept connection: {e}"),
    }
}

fn read_err(e: std::io::Error) -> OAuthError {
    OAuthError::CallbackServer {
        message: format!("read request: {e}"),
    }
}

pub async fn run_callback_server(port: u16) -> Result<CallbackParams, OAuthError> {
    run_callback_server_with_timeout(port, CALLBACK_TIMEOUT).await
}

pub(crate) async fn run_callback_server_with_timeout(
    port: u16,
    timeout: Duration,
) -> Result<CallbackParams, OAuthError> {
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|e| bind_err(port, e))?;

    tokio::time::timeout(timeout, accept_callback(&listener))
        .await
        .unwrap_or_else(|_| Err(timeout_err()))
}

async fn accept_callback(listener: &tokio::net::TcpListener) -> Result<CallbackParams, OAuthError> {
    let (mut stream, _addr) = listener.accept().await.map_err(accept_err)?;

    let mut buf = vec![0u8; 4096];
    let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
        .await
        .map_err(read_err)?;

    let request_bytes = buf.get(..n).unwrap_or_default();
    let request = String::from_utf8_lossy(request_bytes);
    let params = parse_callback_params(&request);

    match params {
        Some(p) => {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stream, SUCCESS_RESPONSE.as_bytes()).await;
            Ok(p)
        }
        None => {
            let _ =
                tokio::io::AsyncWriteExt::write_all(&mut stream, BAD_REQUEST_RESPONSE.as_bytes())
                    .await;
            Err(OAuthError::CallbackServer {
                message: "missing code or state parameter".to_string(),
            })
        }
    }
}

fn parse_callback_params(request: &str) -> Option<CallbackParams> {
    let request_line = request.lines().next()?;
    let path = request_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;

    let mut code = None;
    let mut state = None;

    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            _ => {}
        }
    }

    Some(CallbackParams {
        code: code?,
        state: state?,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_callback_params() {
        let request = "GET /?code=abc123&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let params = parse_callback_params(request).unwrap();
        assert_eq!(params.code, "abc123");
        assert_eq!(params.state, "xyz");
    }

    #[test]
    fn parse_url_encoded_params() {
        let request = "GET /?code=a%20b&state=c%3Dd HTTP/1.1\r\n\r\n";
        let params = parse_callback_params(request).unwrap();
        assert_eq!(params.code, "a b");
        assert_eq!(params.state, "c=d");
    }

    #[test]
    fn parse_missing_code_returns_none() {
        let request = "GET /?state=xyz HTTP/1.1\r\n\r\n";
        assert!(parse_callback_params(request).is_none());
    }

    #[test]
    fn parse_missing_state_returns_none() {
        let request = "GET /?code=abc HTTP/1.1\r\n\r\n";
        assert!(parse_callback_params(request).is_none());
    }

    #[test]
    fn parse_no_query_string_returns_none() {
        let request = "GET / HTTP/1.1\r\n\r\n";
        assert!(parse_callback_params(request).is_none());
    }

    #[test]
    fn parse_empty_request_returns_none() {
        assert!(parse_callback_params("").is_none());
    }

    #[test]
    fn parse_extra_params_ignored() {
        let request = "GET /?code=abc&extra=val&state=xyz HTTP/1.1\r\n\r\n";
        let params = parse_callback_params(request).unwrap();
        assert_eq!(params.code, "abc");
        assert_eq!(params.state, "xyz");
    }

    #[test]
    fn bind_err_formats_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "in use");
        let err = bind_err(8080, io_err);
        assert!(err.to_string().contains("bind to port 8080"));
        assert!(err.to_string().contains("in use"));
    }

    #[test]
    fn timeout_err_returns_timeout_message() {
        let err = timeout_err();
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn accept_err_formats_message() {
        let io_err = std::io::Error::other("refused");
        let err = accept_err(io_err);
        assert!(err.to_string().contains("accept connection"));
        assert!(err.to_string().contains("refused"));
    }

    #[test]
    fn read_err_formats_message() {
        let io_err = std::io::Error::other("broken");
        let err = read_err(io_err);
        assert!(err.to_string().contains("read request"));
        assert!(err.to_string().contains("broken"));
    }

    #[tokio::test]
    async fn callback_server_receives_valid_request() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_handle = tokio::spawn(async move { accept_callback(&listener).await });

        // Simulate browser callback
        let mut client = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut client,
            b"GET /?code=authcode&state=csrftoken HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let result = server_handle.await.unwrap().unwrap();
        assert_eq!(result.code, "authcode");
        assert_eq!(result.state, "csrftoken");
    }

    #[tokio::test]
    async fn callback_server_bad_request_returns_error() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_handle = tokio::spawn(async move { accept_callback(&listener).await });

        let mut client = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut client,
            b"GET /no-params HTTP/1.1\r\nHost: localhost\r\n\r\n",
        )
        .await
        .unwrap();

        let result = server_handle.await.unwrap();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing code or state"));
    }

    #[tokio::test]
    async fn run_callback_server_bind_conflict_returns_error() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        // Try to bind same port
        let result = run_callback_server(port).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bind to port"));
        // Keep listener alive
        drop(listener);
    }

    #[tokio::test]
    async fn run_callback_server_timeout_returns_error() {
        // Use port 0 so the OS picks a free port — avoids drop-then-rebind race condition
        let result = run_callback_server_with_timeout(0, Duration::from_millis(1)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }
}
