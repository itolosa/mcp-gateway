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
