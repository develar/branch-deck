//! The connection of a local app to the local AHP socket of an IDE: the client writes the opener, reads the greeting,
//! writes the envelope with the echo of its nonce and `initialize` (no `access`: the IDE admits a local app), and reads
//! its result. Then each request of `ahp-root://` has its own id, and the client reads frames until the answer of that
//! id, within the timeout of the response (`RESPONSE_TIMEOUT`: the consent dialog and the launch of the session). A
//! notice that comes before the answer waits for `Connection::next_notification`. Blocking: a caller of an async runtime
//! runs it on a blocking thread.

use crate::protocol::{
  CreateSessionResult, Frame, HandshakeLine, HandshakeRefusal, HostParams, HostsResult, INITIALIZE_ID, LaunchRequest, Notification, OPENER_LINE, Refusal, StartKind, Target,
  TargetsResult, envelope_line, handshake_line, initialize_request, parse_frame, request,
};
use crate::watch::RootWatch;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::debug;

#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use uds_windows::UnixStream;

/// How long the client waits for the answer of a request: the consent dialog of the user (up to 30 s) and the launch of
/// the session (up to 60 s), with a margin
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the client waits for the greeting and for the result of `initialize`: the IDE asks no consent for them
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// The longest request line the client sends, in its envelope or after it
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;

/// The longest frame the client reads: a page of 100 rows of at most 8 KiB fits with a margin
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// Why a request gave no answer
#[derive(Debug, Error)]
pub enum ClientError {
  #[error("Cannot connect to the IDE at {socket}: {source}")]
  Connect { socket: PathBuf, source: io::Error },
  #[error("The request is longer than {MAX_REQUEST_BYTES} bytes")]
  RequestTooLarge,
  #[error("The IDE did not answer within {} s", .0.as_secs())]
  Timeout(Duration),
  #[error("The IDE closed the connection without an answer")]
  Closed,
  #[error("A frame of the IDE is longer than {MAX_FRAME_BYTES} bytes")]
  ResponseTooLarge,
  #[error("The answer of the IDE is not a frame of local AHP: {0}")]
  BadResponse(String),
  /// The IDE refused the local peer handshake, and did not read the request
  #[error("The IDE refused the handshake of the connection ({0})")]
  Handshake(HandshakeRefusal),
  /// The IDE answered the request with an error: an answer of the IDE, not a failure of the exchange
  #[error("The IDE refused the request ({0})")]
  Refused(Refusal),
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

  /// The client with another timeout of the answer
  #[must_use]
  pub const fn with_timeout(mut self, timeout: Duration) -> Self {
    self.timeout = timeout;
    self
  }

  pub fn socket(&self) -> &Path {
    &self.socket
  }

  /// The launch directories of the IDE and the last focus of its windows (`x-air-ij/hosts`); the IDE asks no consent
  /// for it
  pub fn hosts(&self) -> Result<HostsResult, ClientError> {
    self.connect()?.call("x-air-ij/hosts", &())
  }

  /// The agents Air can start for the project of `project_directory`, with their models (`x-air-ij/targets`)
  pub fn targets(&self, project_directory: &str) -> Result<Vec<Target>, ClientError> {
    let result: TargetsResult = self.connect()?.call("x-air-ij/targets", &HostParams { project_directory })?;
    Ok(result.targets)
  }

  /// Starts a session (`createSession`), or opens a draft of it (`x-air-ij/openDraft`), as `request.start` says; a draft
  /// has no session to answer
  pub fn launch(&self, request: &LaunchRequest) -> Result<Option<CreateSessionResult>, ClientError> {
    let mut connection = self.connect()?;
    match request.start {
      StartKind::Run => connection.call("createSession", &request.params()).map(Some),
      StartKind::Draft => connection.call::<Value>("x-air-ij/openDraft", &request.params()).map(|_| None),
    }
  }

  /// A watch of the rows of the IDE: its root subscribed, before `RootWatch::list_sessions`
  pub fn watch(&self) -> Result<RootWatch, ClientError> {
    RootWatch::open(self.connect()?)
  }

  /// Connects and runs the handshake: the opener, the greeting, the envelope with `initialize`, and its result, each
  /// within `HANDSHAKE_TIMEOUT` (never longer than the timeout of the answer)
  pub fn connect(&self) -> Result<Connection, ClientError> {
    let initialize = serde_json::to_vec(&initialize_request()).map_err(|error| ClientError::Io(io::Error::other(error)))?;
    let mut stream = UnixStream::connect(&self.socket).map_err(|source| ClientError::Connect {
      socket: self.socket.clone(),
      source,
    })?;
    stream.set_write_timeout(Some(self.timeout)).map_err(ClientError::Io)?;
    write_line(&mut stream, OPENER_LINE, self.timeout)?;

    let timeout = self.timeout.min(HANDSHAKE_TIMEOUT);
    let mut lines = Lines::default();
    let greeting = lines.next(&mut stream, Some((Instant::now() + timeout, timeout)))?.ok_or(ClientError::Closed)?;
    let nonce = match handshake_line(&greeting) {
      HandshakeLine::Greeting { nonce } => nonce,
      HandshakeLine::Refused(code) => return Err(ClientError::Handshake(code)),
      HandshakeLine::BadGreeting(reason) => return Err(ClientError::BadResponse(reason)),
      HandshakeLine::Application => return Err(ClientError::BadResponse("the IDE sent no greeting of the local peer handshake".to_owned())),
    };
    write_line(&mut stream, &envelope_line(&nonce, &initialize), self.timeout)?;

    let answer = lines.next(&mut stream, Some((Instant::now() + timeout, timeout)))?.ok_or(ClientError::Closed)?;
    match handshake_line(&answer) {
      HandshakeLine::Refused(code) => return Err(ClientError::Handshake(code)),
      HandshakeLine::Greeting { .. } | HandshakeLine::BadGreeting(_) => return Err(ClientError::BadResponse("a second line of the handshake".to_owned())),
      HandshakeLine::Application => {}
    }
    match parse_frame(&answer).map_err(ClientError::BadResponse)? {
      Frame::Response { id, outcome } if id == INITIALIZE_ID => {
        outcome.map_err(|error| ClientError::Refused(error.refusal()))?;
      }
      other => return Err(ClientError::BadResponse(format!("the first frame is not the result of initialize: {other:?}"))),
    }
    Ok(Connection {
      stream,
      lines,
      last_id: INITIALIZE_ID,
      timeout: self.timeout,
      notices: VecDeque::new(),
      stopped: Arc::default(),
    })
  }
}

fn write_line(stream: &mut UnixStream, line: &[u8], timeout: Duration) -> Result<(), ClientError> {
  if line.len() > MAX_REQUEST_BYTES {
    return Err(ClientError::RequestTooLarge);
  }
  stream.write_all(line).map_err(|error| io_error(error, timeout))?;
  stream.flush().map_err(|error| io_error(error, timeout))
}

fn io_error(error: io::Error, timeout: Duration) -> ClientError {
  match error.kind() {
    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ClientError::Timeout(timeout),
    _ => ClientError::Io(error),
  }
}

/// A connection after `initialize`: requests of `ahp-root://` one after another, and the notices of the IDE
#[derive(Debug)]
pub struct Connection {
  stream: UnixStream,
  lines: Lines,
  /// The id of the last request; the next one takes the next number
  last_id: u64,
  timeout: Duration,
  /// The notices that came before the answer of a request
  notices: VecDeque<Notification>,
  /// A `Stopper` closed the connection: what the read then gives is no frame and no error. A read after the shutdown
  /// ends at once, but how differs: Unix reads the end of the stream, Windows refuses the read (`WSAESHUTDOWN`, os
  /// error 10058)
  stopped: Arc<AtomicBool>,
}

impl Connection {
  /// The result of `method` with `params` (an object, or `()` for none), within the timeout of the answer. An error of
  /// the IDE is `ClientError::Refused`
  pub fn call<R: DeserializeOwned>(&mut self, method: &str, params: &impl Serialize) -> Result<R, ClientError> {
    self.last_id += 1;
    let id = self.last_id;
    let mut line =
      serde_json::to_vec(&request(id, method, params).map_err(|error| ClientError::Io(io::Error::other(error)))?).map_err(|error| ClientError::Io(io::Error::other(error)))?;
    line.push(b'\n');
    write_line(&mut self.stream, &line, self.timeout)?;

    let deadline = Instant::now() + self.timeout;
    loop {
      let line = self.lines.next(&mut self.stream, Some((deadline, self.timeout)))?.ok_or(ClientError::Closed)?;
      match parse_frame(&line).map_err(ClientError::BadResponse)? {
        Frame::Response { id: answered, outcome } if answered == id => {
          let result = outcome.map_err(|error| ClientError::Refused(error.refusal()))?;
          return serde_json::from_value(result).map_err(|error| ClientError::BadResponse(format!("the result of {method}: {error}")));
        }
        Frame::Response { id: answered, .. } => debug!(%answered, method, "skipped the answer of another request"),
        Frame::Notification(notice) => self.notices.push_back(notice),
        Frame::Request { method: asked } => debug!(asked, "skipped a request of the IDE: a local app serves none"),
      }
    }
  }

  /// The next notice of the IDE, with no timeout: first those that came before an answer. None once the IDE closed the
  /// connection or a `Stopper` stopped it
  pub fn next_notification(&mut self) -> Option<Result<Notification, ClientError>> {
    if let Some(notice) = self.notices.pop_front() {
      return Some(Ok(notice));
    }
    loop {
      let read = self.lines.next(&mut self.stream, None);
      if self.stopped.load(Ordering::SeqCst) {
        return None;
      }
      let line = match read {
        Ok(Some(line)) => line,
        Ok(None) => return None,
        Err(error) => return Some(Err(error)),
      };
      match parse_frame(&line) {
        Ok(Frame::Notification(notice)) => return Some(Ok(notice)),
        Ok(Frame::Response { id, .. }) => debug!(%id, "skipped the answer of no waiting request"),
        Ok(Frame::Request { method }) => debug!(method, "skipped a request of the IDE: a local app serves none"),
        Err(reason) => return Some(Err(ClientError::BadResponse(reason))),
      }
    }
  }

  /// What stops the connection from another thread: the `next_notification` that waits then answers none
  ///
  /// # Errors
  /// The socket cannot be shared
  pub fn stopper(&self) -> Result<Stopper, ClientError> {
    let stream = self.stream.try_clone().map_err(ClientError::Io)?;
    Ok(Stopper {
      stream,
      stopped: Arc::clone(&self.stopped),
    })
  }
}

/// Stops a connection from another thread
#[derive(Debug)]
pub struct Stopper {
  stream: UnixStream,
  stopped: Arc<AtomicBool>,
}

impl Stopper {
  /// Closes the connection: the IDE ends its subscription, and the read that waits answers none
  pub fn stop(&self) {
    // before the shutdown: the read it ends finds the flag set
    self.stopped.store(true, Ordering::SeqCst);
    // the IDE may have closed it already
    let _ = self.stream.shutdown(Shutdown::Both);
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
    if deadline.is_none() {
      set_read_timeout(stream, None)?;
    }
    loop {
      if let Some(end) = self.buffer[scanned..].iter().position(|byte| *byte == b'\n').map(|end| scanned + end) {
        if end > MAX_FRAME_BYTES {
          return Err(ClientError::ResponseTooLarge);
        }
        let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
        line.pop();
        return Ok(Some(line));
      }
      scanned = self.buffer.len();
      if scanned > MAX_FRAME_BYTES {
        return Err(ClientError::ResponseTooLarge);
      }
      // a read without a deadline blocks: it never times out
      let timeout = deadline.map_or(Duration::ZERO, |(_, timeout)| timeout);
      if let Some((deadline, timeout)) = deadline {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
          return Err(ClientError::Timeout(timeout));
        }
        set_read_timeout(stream, Some(left))?;
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

fn set_read_timeout(stream: &UnixStream, timeout: Option<Duration>) -> Result<(), ClientError> {
  match stream.set_read_timeout(timeout) {
    // macOS refuses the option once the server closed its end: the read then returns at once
    Err(error) if error.kind() != io::ErrorKind::InvalidInput => Err(ClientError::Io(error)),
    _ => Ok(()),
  }
}
