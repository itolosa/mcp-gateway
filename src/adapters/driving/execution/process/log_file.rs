use std::path::PathBuf;

use tokio::sync::broadcast;

pub fn spawn_log_writer(
    path: PathBuf,
    sender: &broadcast::Sender<String>,
) -> tokio::task::JoinHandle<()> {
    let mut receiver = sender.subscribe();
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let Ok(file) = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        else {
            return;
        };
        let mut writer = tokio::io::BufWriter::new(file);
        loop {
            match receiver.recv().await {
                Ok(line) => {
                    let _ = writer.write_all(line.as_bytes()).await;
                    let _ = writer.write_all(b"\n").await;
                    let _ = writer.flush().await;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
        let _ = writer.flush().await;
    })
}

pub async fn read_log(
    path: &std::path::Path,
    follow: bool,
) -> Result<(), super::error::DaemonError> {
    use tokio::io::AsyncBufReadExt;

    let err = |e: std::io::Error| super::error::DaemonError::LogRead {
        message: e.to_string(),
    };

    let file = tokio::fs::File::open(path).await.map_err(err)?;
    let mut reader = tokio::io::BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await.map_err(err)? {
            0 => {
                if follow {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                } else {
                    return Ok(());
                }
            }
            _ => eprint!("{line}"),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_write_broadcast_messages_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let (sender, _) = broadcast::channel::<String>(16);

        let handle = spawn_log_writer(path.clone(), &sender);
        sender.send("hello world".to_string()).unwrap();
        sender.send("second line".to_string()).unwrap();
        drop(sender);
        handle.await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("hello world"));
        assert!(content.contains("second line"));
    }

    #[tokio::test]
    async fn should_handle_lagged_receiver() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lag.log");
        let (sender, _) = broadcast::channel::<String>(1);

        let handle = spawn_log_writer(path.clone(), &sender);
        // Send more than buffer size to cause lag
        sender.send("first".to_string()).unwrap();
        sender.send("second".to_string()).unwrap();
        sender.send("third".to_string()).unwrap();
        drop(sender);
        handle.await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        // At least the last message should be present
        assert!(content.contains("third"));
    }

    #[tokio::test]
    async fn should_not_panic_on_invalid_path() {
        let (sender, _) = broadcast::channel::<String>(16);
        let handle = spawn_log_writer(PathBuf::from("/dev/null/impossible/test.log"), &sender);
        drop(sender);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn should_read_log_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("read.log");
        tokio::fs::write(&path, "line one\nline two\n")
            .await
            .unwrap();

        let result = read_log(&path, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_follow_log_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("follow.log");
        tokio::fs::write(&path, "initial\n").await.unwrap();

        let path2 = path.clone();
        let handle = tokio::spawn(async move { read_log(&path2, true).await });

        // Give the follow loop time to read and sleep
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Append more data
        tokio::fs::write(&path, "initial\nappended\n")
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        // Follow runs forever, so abort it
        handle.abort();
        let result = handle.await;
        assert!(result.is_err()); // JoinError from abort
    }

    #[tokio::test]
    async fn should_return_error_on_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.log");
        let result = read_log(&path, false).await;
        assert!(matches!(
            result,
            Err(super::super::error::DaemonError::LogRead { .. })
        ));
    }
}
