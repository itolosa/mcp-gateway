use super::error::DaemonError;

fn attach_err(e: impl std::fmt::Display) -> DaemonError {
    DaemonError::AttachFailed {
        message: e.to_string(),
    }
}

fn process_chunk(bytes: &[u8], writer: &mut impl std::io::Write) -> Result<(), DaemonError> {
    let text = String::from_utf8_lossy(bytes);
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            writeln!(writer, "{data}").map_err(attach_err)?;
        }
    }
    writer.flush().map_err(attach_err)
}

pub async fn attach(port: u16, writer: &mut impl std::io::Write) -> Result<(), DaemonError> {
    let url = format!("http://127.0.0.1:{port}/logs");
    let mut stream = reqwest::get(&url).await.map_err(attach_err)?;
    loop {
        match stream.chunk().await.map_err(attach_err)? {
            Some(bytes) => process_chunk(&bytes, writer)?,
            None => return Ok(()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    async fn start_mock_sse_server() -> (u16, CancellationToken, tokio::task::JoinHandle<()>) {
        use axum::response::sse::{Event, Sse};
        use std::convert::Infallible;

        async fn mock_logs() -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
            Sse::new(tokio_stream::iter(vec![
                Ok::<_, Infallible>(Event::default().data("hello world")),
                Ok(Event::default().data("second line")),
            ]))
        }

        let router = axum::Router::new().route("/logs", axum::routing::get(mock_logs));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let ct = CancellationToken::new();
        let ct_inner = ct.clone();
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(ct_inner.cancelled_owned())
                .await;
        });
        (port, ct, handle)
    }

    struct FailingWriter {
        fail_write: bool,
    }

    impl std::io::Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.fail_write {
                Err(std::io::Error::other("write"))
            } else {
                Ok(buf.len())
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("flush"))
        }
    }

    #[test]
    fn should_return_error_when_write_fails() {
        let result = process_chunk(b"data: hello\n", &mut FailingWriter { fail_write: true });
        assert!(matches!(result, Err(DaemonError::AttachFailed { .. })));
    }

    #[test]
    fn should_return_error_when_flush_fails() {
        let result = process_chunk(b"data: hello\n", &mut FailingWriter { fail_write: false });
        assert!(matches!(result, Err(DaemonError::AttachFailed { .. })));
    }

    #[test]
    fn should_skip_non_data_lines() {
        let mut buf = Vec::new();
        let result = process_chunk(b"not a data line\nignored\n", &mut buf);
        assert!(result.is_ok());
        assert!(buf.is_empty());
    }

    #[tokio::test]
    async fn should_return_error_when_connection_refused() {
        let mut buf = Vec::new();
        let result = attach(1, &mut buf).await;
        assert!(matches!(result, Err(DaemonError::AttachFailed { .. })));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn should_stream_sse_data_lines_to_writer() {
        let (port, ct, handle) = start_mock_sse_server().await;

        let mut buf = Vec::new();
        let result = attach(port, &mut buf).await;
        assert!(result.is_ok());
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("hello world"));
        assert!(output.contains("second line"));

        ct.cancel();
        let _ = handle.await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn should_return_error_when_stream_interrupted() {
        use tokio::io::AsyncWriteExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\n\
                      Transfer-Encoding: chunked\r\n\r\n\
                      5\r\nhello\r\n\
                      INVALID\r\n",
                )
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        });

        let mut buf = Vec::new();
        let result = attach(port, &mut buf).await;
        assert!(matches!(result, Err(DaemonError::AttachFailed { .. })));
        let _ = server.await;
    }
}
