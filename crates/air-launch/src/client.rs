//! One request and one response over the socket of an IDE: the client writes one line of JSON, reads one line within
//! the timeout of the response (`RESPONSE_TIMEOUT`, the consent dialog and the launch of the session), and closes.
//! Blocking: a caller of an async runtime runs it on a blocking thread.

use crate::protocol::{Envelope, Launch, LaunchRequest, PROTOCOL_VERSION, Targets, TargetsRequest};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;

/// How long the client waits for the response: the consent dialog of the user (up to 30 s) and the launch of the
/// session (up to 60 s), with a margin
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

/// The longest line, a request or a response: the server answers a longer request with `BAD_REQUEST`
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

/// Why an exchange gave no response
#[derive(Debug, Error)]
pub enum ClientError {
  #[error("Cannot connect to the IDE at {socket}: {source}")]
  Connect { socket: PathBuf, source: io::Error },
  #[error("The request is longer than {MAX_LINE_BYTES} bytes")]
  RequestTooLarge,
  #[error("The IDE did not answer within {} s", .0.as_secs())]
  Timeout(Duration),
  #[error("The IDE closed the connection without an answer")]
  Closed,
  #[error("The answer of the IDE is longer than {MAX_LINE_BYTES} bytes")]
  ResponseTooLarge,
  #[error("The answer of the IDE is not a response of protocol v1: {0}")]
  BadResponse(String),
  #[error("Cannot talk to the IDE: {0}")]
  Io(io::Error),
}

/// The client of the socket of one IDE
#[derive(Clone, Debug)]
pub struct Client {
  socket: PathBuf,
  timeout: Duration,
}

impl Client {
  pub fn new(socket: impl Into<PathBuf>) -> Self {
    Self {
      socket: socket.into(),
      timeout: RESPONSE_TIMEOUT,
    }
  }

  /// The client with another timeout of the response
  #[must_use]
  pub const fn with_timeout(mut self, timeout: Duration) -> Self {
    self.timeout = timeout;
    self
  }

  pub fn socket(&self) -> &Path {
    &self.socket
  }

  /// The agents Air can start for the project of `project`, with their models
  pub fn targets(&self, project: &str) -> Result<Targets, ClientError> {
    self.exchange("targets", &TargetsRequest { project })
  }

  /// Starts a session, or opens a draft of it (`request.start`)
  pub fn launch(&self, request: &LaunchRequest) -> Result<Launch, ClientError> {
    self.exchange("launch", request)
  }

  fn exchange<T: Serialize, R: DeserializeOwned>(&self, method: &'static str, fields: &T) -> Result<R, ClientError> {
    let mut line = serde_json::to_vec(&Envelope {
      v: PROTOCOL_VERSION,
      method,
      fields,
    })
    .map_err(|error| ClientError::Io(io::Error::other(error)))?;
    if line.len() > MAX_LINE_BYTES {
      return Err(ClientError::RequestTooLarge);
    }
    line.push(b'\n');

    let deadline = Instant::now() + self.timeout;
    let mut stream = UnixStream::connect(&self.socket).map_err(|source| ClientError::Connect {
      socket: self.socket.clone(),
      source,
    })?;
    stream.set_write_timeout(Some(self.timeout)).map_err(ClientError::Io)?;
    stream.write_all(&line).map_err(|error| self.io_error(error))?;
    stream.flush().map_err(|error| self.io_error(error))?;
    let response = self.read_line(&mut stream, deadline)?;
    serde_json::from_slice(&response).map_err(|error| ClientError::BadResponse(error.to_string()))
  }

  /// The first line of the response, without its `\n`; a response the server closed without a `\n` is taken whole
  fn read_line(&self, stream: &mut UnixStream, deadline: Instant) -> Result<Vec<u8>, ClientError> {
    let mut response = Vec::new();
    let mut chunk = [0; 16 * 1024];
    loop {
      let left = deadline.saturating_duration_since(Instant::now());
      if left.is_zero() {
        return Err(ClientError::Timeout(self.timeout));
      }
      match stream.set_read_timeout(Some(left)) {
        // macOS refuses the option once the server closed its end: the read then returns at once
        Err(error) if error.kind() != io::ErrorKind::InvalidInput => return Err(ClientError::Io(error)),
        _ => {}
      }
      let read = match stream.read(&mut chunk) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
        Err(error) => return Err(self.io_error(error)),
      };
      if read == 0 {
        return if response.is_empty() { Err(ClientError::Closed) } else { Ok(response) };
      }
      let start = response.len();
      response.extend_from_slice(&chunk[..read]);
      let end = response[start..].iter().position(|byte| *byte == b'\n').map(|end| start + end);
      if end.unwrap_or(response.len()) > MAX_LINE_BYTES {
        return Err(ClientError::ResponseTooLarge);
      }
      if let Some(end) = end {
        response.truncate(end);
        return Ok(response);
      }
    }
  }

  fn io_error(&self, error: io::Error) -> ClientError {
    match error.kind() {
      io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ClientError::Timeout(self.timeout),
      _ => ClientError::Io(error),
    }
  }
}
