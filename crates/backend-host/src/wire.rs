//! The frames of the wire (docs/design/backend-protocol.md, "The wire", "Errors"): one JSON-RPC 2.0 object per line,
//! of [`MAX_MESSAGE`] at most; what a line is ([`Incoming`]), the errors of the protocol with their `data.reason`
//! ([`Failure`]), and the frames the host writes ([`Frame`]): text, or the bytes of a raw command that the in-process
//! transport passes as they are and a socket writes in base64.

use base64::Engine;
use serde_json::{Value, json};

/// The largest message either side sends: a line of 16 MiB, its newline left out
pub const MAX_MESSAGE: usize = 16 * 1024 * 1024;

/// The root channel: the connection commands and the root commands
pub const ROOT: &str = "ahp-root://";

/// The prefix of the methods of Branch Deck beside the AHP names
pub const EXTENSION: &str = "x-branch-deck/";

/// The version of AHP the host serves: a client's caret range of the same major takes it
pub const AHP_MAJOR: u64 = 1;

/// The version of the vocabulary `x-branch-deck`
pub const VOCABULARY_VERSION: &str = "1.0.0";

/// A frame the host sends
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
  /// One JSON-RPC object, without its newline
  Text(String),
  /// The answer `{bytes}` of a raw command to the request `id`
  Bytes { id: Value, bytes: Vec<u8> },
}

impl Frame {
  /// The JSON of `message`
  pub fn of(message: &Value) -> Self {
    Self::Text(message.to_string())
  }

  /// The text of the frame on a socket: the bytes of a raw answer in base64
  pub fn into_text(self) -> String {
    match self {
      Self::Text(text) => text,
      Self::Bytes { id, bytes } => json!({ "jsonrpc": "2.0", "id": id, "result": { "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) } }).to_string(),
    }
  }

  /// The bytes the frame holds in a queue
  pub const fn size(&self) -> usize {
    match self {
      Self::Text(text) => text.len(),
      Self::Bytes { bytes, .. } => bytes.len(),
    }
  }
}

/// What a line of a client is
#[derive(Debug, PartialEq, Eq)]
pub enum Incoming {
  /// A request with its id, its method and its params (an object with `channel`)
  Request { id: Value, method: String, params: Value },
  /// A notification
  Notification { method: String, params: Value },
  /// The answer of a request of the host: its result, or its error object
  Response { id: Value, answer: Result<Value, Value> },
}

/// A failure of the protocol: a JSON-RPC error with its code and its `data.reason`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
  pub code: i64,
  pub reason: &'static str,
  pub message: String,
  /// The fields of `data` beside `reason`
  pub data: serde_json::Map<String, Value>,
}

impl Failure {
  pub fn new(code: i64, reason: &'static str, message: impl Into<String>) -> Self {
    Self {
      code,
      reason,
      message: message.into(),
      data: serde_json::Map::new(),
    }
  }

  /// `-32700 parse`
  pub fn parse(message: impl Into<String>) -> Self {
    Self::new(-32700, "parse", message)
  }

  /// `-32600 <reason>`: `invalidRequest`, `notInitialized`, `alreadyInitialized`
  pub fn invalid(reason: &'static str, message: impl Into<String>) -> Self {
    Self::new(-32600, reason, message)
  }

  /// `-32601 unknownMethod`
  pub fn unknown_method(method: &str) -> Self {
    Self::new(-32601, "unknownMethod", format!("No method is named {method}"))
  }

  /// `-32602 badParams`
  pub fn bad_params(message: impl Into<String>) -> Self {
    Self::new(-32602, "badParams", message)
  }

  /// `-32603 apiError`: app-api refused a method that has no answer of its own, its `ApiError` in `data.apiError`
  pub fn api_error(error: &domain_types::error::ApiError) -> Self {
    Self::new(-32603, "apiError", error.message.clone()).with("apiError", serde_json::to_value(error).unwrap_or(Value::Null))
  }

  /// `-32008 <reason>`: `unknownChannel`, `notSubscribed`
  pub fn channel(reason: &'static str, message: impl Into<String>) -> Self {
    Self::new(-32008, reason, message)
  }

  /// `data.<field>` is `value`
  #[must_use]
  pub fn with(mut self, field: &str, value: Value) -> Self {
    self.data.insert(field.to_owned(), value);
    self
  }

  /// The error answer of the request `id`
  pub fn answer(&self, id: &Value) -> Value {
    let mut data = self.data.clone();
    data.insert("reason".to_owned(), Value::String(self.reason.to_owned()));
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": self.code, "message": self.message, "data": data } })
  }
}

/// The result answer of the request `id`
pub fn result(id: &Value, result: Value) -> Value {
  message([("id", id.clone()), ("result", result)])
}

/// The notification `method` with `params`
pub fn notification(method: &str, params: Value) -> Value {
  message([("method", Value::String(method.to_owned())), ("params", params)])
}

/// A JSON-RPC 2.0 object of `fields`
fn message<const N: usize>(fields: [(&str, Value); N]) -> Value {
  let mut object = serde_json::Map::new();
  object.insert("jsonrpc".to_owned(), Value::String("2.0".to_owned()));
  for (name, value) in fields {
    object.insert(name.to_owned(), value);
  }
  Value::Object(object)
}

/// What `line` is: a request, a notification or a response; the failure to answer when it is none, with the id it
/// names (`null` when it names none)
///
/// # Errors
/// The line is not JSON (`parse`), is a batch or no JSON-RPC 2.0 object (`invalidRequest`), or its `params` is no
/// object with `channel` (`badParams`)
pub fn read(line: &str) -> Result<Incoming, (Value, Failure)> {
  let value: Value = serde_json::from_str(line).map_err(|error| (Value::Null, Failure::parse(format!("The line is not JSON: {error}"))))?;
  let Value::Object(mut object) = value else {
    return Err((Value::Null, Failure::invalid("invalidRequest", "A message is one JSON-RPC object, never a batch")));
  };
  let id = object.remove("id");
  if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
    return Err((id.unwrap_or(Value::Null), Failure::invalid("invalidRequest", "A message has jsonrpc 2.0")));
  }
  let Some(method) = object.get("method").and_then(Value::as_str).map(str::to_owned) else {
    return match (id, object.remove("result"), object.remove("error")) {
      (Some(id), Some(result), None) => Ok(Incoming::Response { id, answer: Ok(result) }),
      (Some(id), None, Some(error)) => Ok(Incoming::Response { id, answer: Err(error) }),
      (id, _, _) => Err((
        id.unwrap_or(Value::Null),
        Failure::invalid("invalidRequest", "A message is a request, a notification or an answer"),
      )),
    };
  };
  let params = object.remove("params").unwrap_or(Value::Null);
  let has_channel = params.get("channel").is_some_and(Value::is_string);
  match id {
    Some(id) if !has_channel => Err((id, Failure::bad_params(format!("The params of {method} name their channel")))),
    Some(id) => Ok(Incoming::Request { id, method, params }),
    None => Ok(Incoming::Notification { method, params }),
  }
}

/// The highest of `offered`, AHP versions `MAJOR.MINOR.PATCH`, in the caret range of the host (`AHP_MAJOR`)
pub fn negotiate(offered: &[Value]) -> Option<String> {
  offered
    .iter()
    .filter_map(Value::as_str)
    .filter_map(|version| semver(version).map(|parts| (parts, version)))
    .filter(|((major, _, _), _)| *major == AHP_MAJOR)
    .max_by_key(|(parts, _)| *parts)
    .map(|(_, version)| version.to_owned())
}

/// The parts of a `MAJOR.MINOR.PATCH` version
pub fn semver(version: &str) -> Option<(u64, u64, u64)> {
  let mut parts = version.split('.').map(str::parse::<u64>);
  let parsed = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
  parts.next().is_none().then_some(parsed)
}

#[cfg(test)]
#[path = "wire_test.rs"]
mod wire_test;
