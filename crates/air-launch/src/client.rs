//! One request and its response over the socket of an IDE, after the local peer handshake v2: the client writes the
//! opener, reads the greeting, writes the envelope with the echo of its nonce and the request, reads one line within the
//! timeout of the response (`RESPONSE_TIMEOUT`, the consent dialog and the launch of the session), and closes. A
//! `watch` reads its lines one by one without a timeout, until its end or a close (`Watch`). Blocking: a caller of an
//! async runtime runs it on a blocking thread.
//!
//! An IDE before handshake v2 answers the opener as a request: its line is the answer (a refusal such as
//! `UNSUPPORTED_VERSION`), and the client sends nothing more.

use crate::protocol::{
  ENVELOPE_FRAME_BYTES, Envelope, HandshakeLine, HandshakeRefusal, Launch, LaunchRequest, OPENER_LINE, PROTOCOL_VERSION, Targets, TargetsRequest, WatchLine, WatchRequest,
  envelope_line, handshake_line,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{self, Read, Write};
use std::net::Shutdown;
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

/// How long the client waits for the greeting: the IDE reads the opener within 5 s and greets before any dialog. It
/// bounds the greeting of a watch too, which has no timeout of its lines
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest line, a request in its envelope or a response: the server answers a longer line with `BAD_REQUEST`
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
  #[error("The answer of the IDE is not a line of the launch protocol: {0}")]
  BadResponse(String),
  /// The IDE refused the local peer handshake, and did not read the request
  #[error("The IDE refused the handshake of the connection ({0})")]
  Handshake(HandshakeRefusal),
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

  /// Watches the session `session_id` of the project of `project`: the IDE holds the connection and sends a line now
  /// and after each change, with no timeout of the client; the watch ends with its last line or a close
  pub fn watch(&self, project: &str, session_id: &str) -> Result<Watch, ClientError> {
    let Connection { stream, lines, answer } = self.open("watch", &WatchRequest { project, session_id }, HANDSHAKE_TIMEOUT)?;
    match stream.set_read_timeout(None) {
      // macOS refuses the option once the IDE closed its end: the read then returns at once
      Err(error) if error.kind() != io::ErrorKind::InvalidInput => return Err(ClientError::Io(error)),
      _ => {}
    }
    Ok(Watch {
      stream,
      lines,
      answer,
      first: true,
      ended: false,
    })
  }

  fn exchange<T: Serialize, R: DeserializeOwned>(&self, method: &'static str, fields: &T) -> Result<R, ClientError> {
    let deadline = Instant::now() + self.timeout;
    let Connection { mut stream, mut lines, answer } = self.open(method, fields, self.timeout.min(HANDSHAKE_TIMEOUT))?;
    let response = match answer {
      Some(answer) => answer,
      None => lines.next(&mut stream, Some((deadline, self.timeout)))?.ok_or(ClientError::Closed)?,
    };
    parse_first(&response)
  }

  /// Connects and runs the handshake: the opener, the greeting within `greeting_timeout`, and the envelope with the
  /// request of `method`
  fn open<T: Serialize>(&self, method: &'static str, fields: &T, greeting_timeout: Duration) -> Result<Connection, ClientError> {
    let request = serde_json::to_vec(&Envelope {
      v: PROTOCOL_VERSION,
      method,
      fields,
    })
    .map_err(|error| ClientError::Io(io::Error::other(error)))?;
    if request.len() + ENVELOPE_FRAME_BYTES > MAX_LINE_BYTES {
      return Err(ClientError::RequestTooLarge);
    }

    let mut stream = UnixStream::connect(&self.socket).map_err(|source| ClientError::Connect {
      socket: self.socket.clone(),
      source,
    })?;
    stream.set_write_timeout(Some(self.timeout)).map_err(ClientError::Io)?;
    self.write_line(&mut stream, OPENER_LINE)?;

    let mut lines = Lines::default();
    let first = lines
      .next(&mut stream, Some((Instant::now() + greeting_timeout, greeting_timeout)))?
      .ok_or(ClientError::Closed)?;
    match handshake_line(&first) {
      HandshakeLine::Greeting { nonce } => {
        self.write_line(&mut stream, &envelope_line(&nonce, &request))?;
        Ok(Connection { stream, lines, answer: None })
      }
      HandshakeLine::Refused(code) => Err(ClientError::Handshake(code)),
      HandshakeLine::BadGreeting(reason) => Err(ClientError::BadResponse(reason)),
      // an IDE before handshake v2 read the opener as a request
      HandshakeLine::Application => Ok(Connection {
        stream,
        lines,
        answer: Some(first),
      }),
    }
  }

  fn write_line(&self, stream: &mut UnixStream, line: &[u8]) -> Result<(), ClientError> {
    stream.write_all(line).map_err(|error| io_error(error, self.timeout))?;
    stream.flush().map_err(|error| io_error(error, self.timeout))
  }
}

/// A connection after the handshake
struct Connection {
  stream: UnixStream,
  lines: Lines,
  /// The answer of an IDE before handshake v2 to the opener: the first line of the application, already read
  answer: Option<Vec<u8>>,
}

fn parse<R: DeserializeOwned>(line: &[u8]) -> Result<R, ClientError> {
  serde_json::from_slice(line).map_err(|error| ClientError::BadResponse(error.to_string()))
}

/// The first line after the envelope: a refusal of the handshake (the IDE did not run the request), else the response
fn parse_first<R: DeserializeOwned>(line: &[u8]) -> Result<R, ClientError> {
  match handshake_line(line) {
    HandshakeLine::Refused(code) => Err(ClientError::Handshake(code)),
    HandshakeLine::Greeting { .. } | HandshakeLine::BadGreeting(_) => Err(ClientError::BadResponse("a second line of the handshake".to_owned())),
    HandshakeLine::Application => parse(line),
  }
}

fn io_error(error: io::Error, timeout: Duration) -> ClientError {
  match error.kind() {
    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ClientError::Timeout(timeout),
    _ => ClientError::Io(error),
  }
}

/// A watch of one session: its lines one by one. Blocking: each `next` waits for the IDE as long as it takes
#[derive(Debug)]
pub struct Watch {
  stream: UnixStream,
  lines: Lines,
  /// The first line of an IDE before handshake v2, read during the handshake
  answer: Option<Vec<u8>>,
  /// No line came yet: the first can be a refusal of the handshake
  first: bool,
  /// The IDE sent the end or a refusal: no line comes after it
  ended: bool,
}

impl Watch {
  /// What stops the watch from another thread: the `next` that waits then answers none
  ///
  /// # Errors
  /// The socket cannot be shared
  pub fn stopper(&self) -> Result<WatchStopper, ClientError> {
    self.stream.try_clone().map(WatchStopper).map_err(ClientError::Io)
  }
}

impl Iterator for Watch {
  type Item = Result<WatchLine, ClientError>;

  /// The next line; none after the end, a refusal or a close (also a close of `WatchStopper::stop`)
  fn next(&mut self) -> Option<Self::Item> {
    if self.ended {
      return None;
    }
    let line = match self.answer.take() {
      Some(answer) => Ok(answer),
      None => self.lines.next(&mut self.stream, None).transpose()?,
    };
    let first = std::mem::replace(&mut self.first, false);
    let line = line.and_then(|line| if first { parse_first::<WatchLine>(&line) } else { parse::<WatchLine>(&line) });
    self.ended = !matches!(line, Ok(WatchLine::Session { .. }));
    Some(line)
  }
}

/// Stops a watch from another thread
#[derive(Debug)]
pub struct WatchStopper(UnixStream);

impl WatchStopper {
  /// Closes the connection: the IDE ends the watch, and the `next` that waits answers none
  pub fn stop(&self) {
    // the IDE may have closed it already
    let _ = self.0.shutdown(Shutdown::Both);
  }
}

/// The lines of one connection: the bytes after a line wait in `buffer` for the next one
#[derive(Debug, Default)]
struct Lines {
  buffer: Vec<u8>,
}

impl Lines {
  /// The next line, without its `\n`, within the deadline if any; the rest of a connection the server closed without a
  /// `\n` is taken whole, none after the last line
  fn next(&mut self, stream: &mut UnixStream, deadline: Option<(Instant, Duration)>) -> Result<Option<Vec<u8>>, ClientError> {
    let mut scanned = 0;
    let mut chunk = [0; 16 * 1024];
    loop {
      if let Some(end) = self.buffer[scanned..].iter().position(|byte| *byte == b'\n').map(|end| scanned + end) {
        if end > MAX_LINE_BYTES {
          return Err(ClientError::ResponseTooLarge);
        }
        let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
        line.pop();
        return Ok(Some(line));
      }
      scanned = self.buffer.len();
      if scanned > MAX_LINE_BYTES {
        return Err(ClientError::ResponseTooLarge);
      }
      // a read without a deadline blocks: it never times out
      let timeout = deadline.map_or(Duration::ZERO, |(_, timeout)| timeout);
      if let Some((deadline, timeout)) = deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
          return Err(ClientError::Timeout(timeout));
        }
        match stream.set_read_timeout(Some(left)) {
          // macOS refuses the option once the server closed its end: the read then returns at once
          Err(error) if error.kind() != io::ErrorKind::InvalidInput => return Err(ClientError::Io(error)),
          _ => {}
        }
      }
      let read = match stream.read(&mut chunk) {
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
        Err(error) => return Err(io_error(error, timeout)),
      };
      if read == 0 {
        return Ok((!self.buffer.is_empty()).then(|| std::mem::take(&mut self.buffer)));
      }
      self.buffer.extend_from_slice(&chunk[..read]);
    }
  }
}
