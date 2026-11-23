//! A connection over a stream of bytes (a local socket, a named pipe): each line read is one message, of
//! `MAX_MESSAGE` at most (a longer one is refused and skipped), each frame of the connection one line written. The end
//! of the stream ends the connection.

use crate::connection::Connection;
use crate::wire::{Failure, MAX_MESSAGE};
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tracing::Instrument;

/// Serves `connection` over `stream` until either side ends it
pub async fn serve_stream<S>(connection: Arc<Connection>, stream: S)
where
  S: AsyncRead + AsyncWrite + Send + 'static,
{
  let (read, mut write) = tokio::io::split(stream);
  let writer = {
    let connection = Arc::clone(&connection);
    tokio::spawn(
      async move {
        while let Some(frame) = connection.next_frame().await {
          let mut line = frame.into_text();
          line.push('\n');
          if write.write_all(line.as_bytes()).await.is_err() || write.flush().await.is_err() {
            connection.close();
            break;
          }
        }
        let _ = write.shutdown().await;
      }
      .in_current_span(),
    )
  };
  let mut read = BufReader::new(read);
  let mut line = Vec::new();
  loop {
    line.clear();
    match (&mut read).take(MAX_MESSAGE as u64 + 1).read_until(b'\n', &mut line).await {
      Ok(0) | Err(_) => break,
      Ok(_) if line.last() != Some(&b'\n') && line.len() > MAX_MESSAGE => {
        connection.fail(&Value::Null, &Failure::invalid("invalidRequest", "A message is 16 MiB at most"));
        if skip_line(&mut read).await.is_err() {
          break;
        }
      }
      Ok(_) => {
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\n', '\r']);
        if !text.trim().is_empty() {
          connection.receive(text);
        }
      }
    }
  }
  connection.close();
  let _ = writer.await;
}

/// Reads past the end of the line it is in
async fn skip_line<R: AsyncBufReadExt + Unpin>(read: &mut R) -> std::io::Result<()> {
  loop {
    let buffer = read.fill_buf().await?;
    if buffer.is_empty() {
      return Ok(());
    }
    if let Some(at) = buffer.iter().position(|byte| *byte == b'\n') {
      read.consume(at + 1);
      return Ok(());
    }
    let len = buffer.len();
    read.consume(len);
  }
}
