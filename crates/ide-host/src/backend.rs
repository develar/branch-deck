//! The bridge's connection to the backend (docs/design/backend-protocol.md; openspec/specs/repository/ide-embedding/spec.md,
//! "Bridge"): a client of `backend-host`, over the local socket of the desktop app (`backend.sock`, the named pipe on
//! Windows) or, in the fallback, over the in-process transport of a backend of its own. One JSON-RPC 2.0 object per
//! line both ways; the bridge counts the ids of its requests from 1, the host its own.
//!
//! - [`handshake`]: `initialize` with the build of this binary and the host request the bridge answers, `openWindow`:
//!   the IDE shows its views itself, so it takes no `showWindow`.
//! - [`BackendLink`]: what the bridge sends, its requests waiting for their answers. An answer is handed to its waiter
//!   by the reader of the connection in the order of the lines ([`BackendSource::run`]): a waiter that writes to the IDE
//!   (`then`) writes before the frames after the answer, so the order of the backend is the order of the IDE.

use backend_host::in_process::InProcess;
use backend_host::socket::Stream;
use backend_host::wire::{ROOT, VOCABULARY_VERSION};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

/// The answer of a request of the bridge: its result, or its error object
pub type Reply = Result<Value, Value>;

/// The name of the bridge as a client of the backend
pub const CLIENT_NAME: &str = "branch-deck-ide";

/// The host requests the bridge answers: the IDE shows a view itself, so `showWindow` is none of them (a view is never
/// built hidden, and the deadlines of a hidden window never reach the IDE)
pub const HOST_REQUESTS: [&str; 1] = ["openWindow"];

/// What waits for the answer of a request
enum Waiter {
  Answer(oneshot::Sender<Reply>),
  /// Runs on the reader, before the line after the answer
  Then(Box<dyn FnOnce(Reply) + Send>),
}

/// The writing side of the connection to the backend and the requests that wait for their answers
pub struct BackendLink {
  out: mpsc::UnboundedSender<String>,
  last_id: AtomicU64,
  pending: Mutex<HashMap<u64, Waiter>>,
}

impl BackendLink {
  /// A link whose lines come out of the receiver, in the order they were sent
  pub(crate) fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<String>) {
    let (out, lines) = mpsc::unbounded_channel();
    let link = Self {
      out,
      last_id: AtomicU64::new(1),
      pending: Mutex::default(),
    };
    (Arc::new(link), lines)
  }

  fn pending(&self) -> MutexGuard<'_, HashMap<u64, Waiter>> {
    self.pending.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn send(&self, message: &Value) {
    let _ = self.out.send(message.to_string());
  }

  /// The id of the next request, for a caller that keeps what the frames of the request name (its call events) before
  /// it sends it (`request_with`)
  pub fn next_id(&self) -> u64 {
    self.last_id.fetch_add(1, Ordering::SeqCst) + 1
  }

  /// The params of a root method: `channel` the root unless they name one
  fn rooted(mut params: Value) -> Value {
    if !params.is_object() {
      params = json!({});
    }
    if params.get("channel").is_none() {
      params["channel"] = Value::String(ROOT.to_owned());
    }
    params
  }

  /// The notification `method` with `params`
  pub fn notify(&self, method: &str, params: Value) {
    self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": Self::rooted(params) }));
  }

  /// The request `method`: its answer, an error once the connection ended without one
  pub fn request(&self, method: &str, params: Value) -> oneshot::Receiver<Reply> {
    let (reply, answer) = oneshot::channel();
    self.request_with(self.next_id(), method, params, Waiter::Answer(reply));
    answer
  }

  /// The request `method`, its answer handed to `then` on the reader of the connection, before every line after it;
  /// nothing when the connection ends first
  pub fn request_then(&self, method: &str, params: Value, then: impl FnOnce(Reply) + Send + 'static) {
    self.request_with(self.next_id(), method, params, Waiter::Then(Box::new(then)));
  }

  /// `request_then` with the id `id` of `next_id`
  pub fn request_as(&self, id: u64, method: &str, params: Value, then: impl FnOnce(Reply) + Send + 'static) {
    self.request_with(id, method, params, Waiter::Then(Box::new(then)));
  }

  fn request_with(&self, id: u64, method: &str, params: Value, waiter: Waiter) {
    self.pending().insert(id, waiter);
    self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": Self::rooted(params) }));
  }

  /// The result of the host request `id`
  pub fn respond(&self, id: &Value, result: &Value) {
    self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
  }

  /// The error of the host request `id`
  pub fn fail(&self, id: &Value, code: i64, message: &str, reason: &str) {
    self.send(&json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message, "data": { "reason": reason } } }));
  }

  /// The backend answered the request `id`
  pub(crate) fn answered(&self, id: &Value, reply: Reply) {
    let Some(waiter) = id.as_u64().and_then(|id| self.pending().remove(&id)) else {
      tracing::debug!(%id, "an answer of the backend to no request of the bridge");
      return;
    };
    match waiter {
      Waiter::Answer(sender) => {
        let _ = sender.send(reply);
      }
      Waiter::Then(then) => then(reply),
    }
  }

  /// The connection ended: no request is answered any more
  fn ended(&self) {
    self.pending().clear();
  }

  /// Ends the writing side: the backend reads the end of the connection, which it then ends
  pub fn close(&self) {
    self.pending().clear();
    let _ = self.out.send(String::new());
  }
}

/// A connection to a backend, before its pumps run: the socket of the app, or a connection of a host of the process
pub enum Transport {
  Socket(Stream),
  InProcess(InProcess),
}

/// The reading side of a transport
enum Source {
  Socket(BufReader<ReadHalf<Stream>>),
  InProcess(InProcess),
}

/// The writing side of a transport
enum Sink {
  Socket(WriteHalf<Stream>),
  InProcess(Arc<backend_host::connection::Connection>),
}

impl Source {
  /// The next line of the backend, none once the connection ended
  async fn recv(&mut self) -> Option<String> {
    match self {
      Self::Socket(read) => {
        let mut line = String::new();
        match read.read_line(&mut line).await {
          Ok(0) | Err(_) => None,
          Ok(_) => Some(line.trim_end_matches(['\n', '\r']).to_owned()),
        }
      }
      Self::InProcess(client) => client.next().await.map(backend_host::wire::Frame::into_text),
    }
  }
}

impl Sink {
  /// Writes `line`; an empty line ends the writing side
  async fn send(&mut self, line: &str) -> std::io::Result<()> {
    match self {
      Self::Socket(write) => {
        if line.is_empty() {
          return write.shutdown().await;
        }
        write.write_all(line.as_bytes()).await?;
        write.write_all(b"\n").await?;
        write.flush().await
      }
      Self::InProcess(connection) => {
        if line.is_empty() {
          connection.close();
        } else {
          connection.receive(line);
        }
        Ok(())
      }
    }
  }
}

impl Transport {
  fn split(self) -> (Source, Sink) {
    match self {
      Self::Socket(stream) => {
        let (read, write) = tokio::io::split(stream);
        (Source::Socket(BufReader::new(read)), Sink::Socket(write))
      }
      Self::InProcess(client) => {
        let connection = Arc::clone(client.connection());
        (Source::InProcess(client), Sink::InProcess(connection))
      }
    }
  }
}

/// Why the backend refused the bridge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
  /// The backend is of another build (`otherBuild`): it exits after the answer when it is stale and has no window
  OtherBuild { build: String, exits: bool },
  /// Any other refusal, or a connection that ended first
  Other(String),
}

/// The reading side of a connection to the backend, which hands each line to the bridge
pub struct BackendSource {
  source: Source,
}

impl BackendSource {
  /// Reads the backend until the connection ends: each answer to its waiter, each notification and host request to
  /// `handle`, in the order of the lines
  pub async fn run(mut self, link: &BackendLink, mut handle: impl FnMut(Value)) {
    while let Some(line) = self.source.recv().await {
      let Ok(message) = serde_json::from_str::<Value>(&line) else {
        tracing::warn!("a line of the backend is not JSON");
        continue;
      };
      if message.get("method").is_none() {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let reply = message
          .get("error")
          .map_or_else(|| Ok(message.get("result").cloned().unwrap_or(Value::Null)), |error| Err(error.clone()));
        link.answered(&id, reply);
      } else {
        handle(message);
      }
    }
    link.ended();
  }
}

/// `initialize` of the bridge on `transport`, then its pumps: the link and its reading side; the refusal of the backend
/// else
///
/// # Errors
/// The backend refused the bridge (another build, another version), or the connection ended first
pub async fn handshake(transport: Transport, build: &str) -> Result<(Arc<BackendLink>, BackendSource), Refusal> {
  let (mut source, mut sink) = transport.split();
  let initialize = json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": {
      "channel": ROOT,
      "protocolVersions": ["1.0.0"],
      "clientId": format!("{CLIENT_NAME}-{}", std::process::id()),
      "clientInfo": { "name": CLIENT_NAME, "build": build },
      "_meta": { "x-branch-deck": { "version": VOCABULARY_VERSION, "capabilities": { "hostRequests": HOST_REQUESTS } } },
    },
  });
  sink
    .send(&initialize.to_string())
    .await
    .map_err(|error| Refusal::Other(format!("the backend is not reached: {error}")))?;
  loop {
    let Some(line) = source.recv().await else {
      return Err(Refusal::Other("the backend closed the connection before it answered initialize".to_owned()));
    };
    let Ok(answer) = serde_json::from_str::<Value>(&line) else {
      continue;
    };
    if answer["id"] != 1 {
      continue;
    }
    if let Some(error) = answer.get("error") {
      let data = &error["data"];
      return Err(if data["reason"] == "otherBuild" {
        Refusal::OtherBuild {
          build: data["build"].as_str().unwrap_or_default().to_owned(),
          exits: data["exits"].as_bool().unwrap_or(false),
        }
      } else {
        Refusal::Other(error["message"].as_str().unwrap_or("no reason").to_owned())
      });
    }
    break;
  }
  let (link, mut lines) = BackendLink::new();
  tokio::spawn(
    async move {
      while let Some(line) = lines.recv().await {
        let last = line.is_empty();
        if sink.send(&line).await.is_err() || last {
          break;
        }
      }
    }
    .in_current_span(),
  );
  Ok((link, BackendSource { source }))
}
