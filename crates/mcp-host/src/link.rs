//! The one JSON-RPC 2.0 connection of `brd mcp` with air-ide: every message brd writes goes through one queue in the
//! order it was sent (`Link::send`), so the changes and the events of a call come before its answer, as they were sent;
//! a request of brd to the IDE (`roots/list`, `x-air-app/openView`) waits for its answer by its id, which brd counts
//! from 1.

use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::{mpsc, oneshot};

/// The answer of a request of brd: its result, or its error object
pub type Reply = Result<Value, Value>;

/// The code of an error of the arguments of a request (JSON-RPC)
pub const INVALID_PARAMS: i64 = -32602;
/// The code of a method that brd does not know (JSON-RPC)
pub const METHOD_NOT_FOUND: i64 = -32601;
/// The code of a failure of brd itself (JSON-RPC)
pub const INTERNAL_ERROR: i64 = -32603;
/// The code of a call that no consent covers, and of a call of an agent (x-air-app)
pub const NO_CONSENT: i64 = -32009;

/// The writing side of the connection, and the requests of brd that wait for their answer
pub struct Link {
  out: mpsc::UnboundedSender<String>,
  next_id: AtomicI64,
  pending: Mutex<HashMap<i64, oneshot::Sender<Reply>>>,
}

impl Link {
  /// A link whose messages come out of the receiver, one JSON object each, in the order they were sent
  pub fn new() -> (Self, mpsc::UnboundedReceiver<String>) {
    let (out, receiver) = mpsc::unbounded_channel();
    let link = Self {
      out,
      next_id: AtomicI64::new(1),
      pending: Mutex::new(HashMap::new()),
    };
    (link, receiver)
  }

  /// Writes `message`; nothing once the connection closed
  pub fn send(&self, message: &Value) {
    match serde_json::to_string(message) {
      Ok(text) => {
        let _ = self.out.send(text);
      }
      Err(error) => tracing::error!(%error, "a message of brd is not JSON"),
    }
  }

  /// The notification `method` with `params`
  pub fn notify(&self, method: &str, params: Value) {
    let mut message = json!({ "jsonrpc": "2.0", "method": method });
    message["params"] = params;
    self.send(&message);
  }

  /// The result of the request `id`
  pub fn respond(&self, id: &Value, result: Value) {
    let mut message = json!({ "jsonrpc": "2.0", "id": id });
    message["result"] = result;
    self.send(&message);
  }

  /// The error of the request `id`, with its reason of x-air-app (`data.reason`)
  pub fn fail(&self, id: &Value, code: i64, message: &str, reason: &str) {
    self.send(&json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message, "data": { "reason": reason } } }));
  }

  /// Sends the request `method` to the IDE: its answer, or an error once the connection closed without one
  pub fn request(&self, method: &str, params: Option<Value>) -> oneshot::Receiver<Reply> {
    let id = self.next_id.fetch_add(1, Ordering::Relaxed);
    let (sender, receiver) = oneshot::channel();
    self.pending().insert(id, sender);
    let mut message = json!({ "jsonrpc": "2.0", "id": id, "method": method });
    if let Some(params) = params {
      message["params"] = params;
    }
    self.send(&message);
    receiver
  }

  /// Sends the request `method` to the IDE, whose answer nothing waits for: a request that changes only the IDE (a
  /// badge, a title)
  pub fn ask(&self, method: &str, params: Value) {
    drop(self.request(method, Some(params)));
  }

  /// The IDE answered the request `id` of brd
  pub fn answered(&self, id: &Value, reply: Reply) {
    let Some(sender) = id.as_i64().and_then(|id| self.pending().remove(&id)) else {
      tracing::debug!(%id, "an answer to no request of brd");
      return;
    };
    let _ = sender.send(reply);
  }

  fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<i64, oneshot::Sender<Reply>>> {
    self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }
}
