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
