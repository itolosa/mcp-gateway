use std::time::Duration;

use super::error::OAuthError;

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
    run_callback_on_listener_with_timeout(listener, timeout).await
}

pub(crate) async fn run_callback_on_listener(
    listener: tokio::net::TcpListener,
) -> Result<CallbackParams, OAuthError> {
    run_callback_on_listener_with_timeout(listener, CALLBACK_TIMEOUT).await
}

async fn run_callback_on_listener_with_timeout(
    listener: tokio::net::TcpListener,
    timeout: Duration,
) -> Result<CallbackParams, OAuthError> {
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

    let (code, state) = url::form_urlencoded::parse(query.as_bytes()).fold(
        (None, None),
        |(code, state), (key, value)| match key.as_ref() {
            "code" => (Some(value.into_owned()), state),
            "state" => (code, Some(value.into_owned())),
            _ => (code, state),
        },
    );

    Some(CallbackParams {
        code: code?,
        state: state?,
    })
}
