//! The messages of the local AHP socket of Air for a local app: the lines of the local peer handshake v2 (the opener,
//! the greeting with its nonce, the envelope and a refusal of the handshake), the JSON-RPC 2.0 frames after it (a request
//! of `ahp-root://`, its result or error, a notice of the IDE), the result of the hosts, the params of a draft and the
//! refusal of an error (`error.data.code`). A field the IDE adds later is ignored; a refusal code this client does not
//! know stays as its text.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::path::PathBuf;

/// The `proto` of each line of the local peer handshake
pub const HANDSHAKE_PROTO: &str = "local-peer";

/// The version of the local peer handshake this client speaks
pub const HANDSHAKE_VERSION: u32 = 2;

/// The channel of every request of a local app
pub const ROOT_CHANNEL: &str = "ahp-root://";

/// The AHP version that `initialize` offers
pub const AHP_VERSION: &str = "1.0.0";

/// The `clientId` of `initialize`
pub const CLIENT_ID: &str = "branch-deck";

/// The id of `initialize`, the request of the envelope; the requests after it count on from here
pub(crate) const INITIALIZE_ID: u64 = 1;

/// The first line of the client, before the IDE reads the credentials of the peer
pub(crate) const OPENER_LINE: &[u8] = b"{\"proto\":\"local-peer\",\"v\":2}\n";

/// The longest nonce of a greeting this client echoes; the IDE sends 128 bits in base64url, 22 characters
pub(crate) const MAX_NONCE_CHARS: usize = 64;

/// The bytes of the envelope around the request, with the longest nonce and the line end
pub(crate) const ENVELOPE_FRAME_BYTES: usize = r#"{"nonce":"","request":}"#.len() + MAX_NONCE_CHARS + 1;

/// The envelope line of the handshake: the echo of the nonce of the greeting and the request object, with no `access`:
/// the IDE admits the connection as a local app. The nonce is base64url (`handshake_line` checks it), so it needs no
/// escape
pub(crate) fn envelope_line(nonce: &str, request: &[u8]) -> Vec<u8> {
  let mut line = Vec::with_capacity(ENVELOPE_FRAME_BYTES + request.len());
  line.extend_from_slice(br#"{"nonce":""#);
  line.extend_from_slice(nonce.as_bytes());
  line.extend_from_slice(br#"","request":"#);
  line.extend_from_slice(request);
  line.extend_from_slice(b"}\n");
  line
}

/// The `initialize` request of the envelope
pub(crate) fn initialize_request() -> Value {
  serde_json::json!({
    "jsonrpc": "2.0",
    "id": INITIALIZE_ID,
    "method": "initialize",
    "params": {"channel": ROOT_CHANNEL, "protocolVersions": [AHP_VERSION], "clientId": CLIENT_ID},
  })
}

/// The request `id` of `method`: `params`, an object, with the channel `ahp-root://` first
///
/// # Errors
/// `params` is not an object
pub(crate) fn request(id: u64, method: &str, params: &impl Serialize) -> Result<Value, serde_json::Error> {
  let mut object = serde_json::Map::new();
  object.insert("channel".to_owned(), Value::from(ROOT_CHANNEL));
  match serde_json::to_value(params)? {
    Value::Object(fields) => object.extend(fields),
    Value::Null => {}
    other => return Err(serde::ser::Error::custom(format!("the params of {method} are not an object: {other}"))),
  }
  Ok(serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": object}))
}

/// A line of the IDE as the handshake reads it
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HandshakeLine {
  /// The greeting: the client echoes its nonce in the envelope
  Greeting { nonce: String },
  /// The IDE refused the handshake, and the host did not read the request
  Refused(HandshakeRefusal),
  /// A line of the handshake that is not a greeting this client can echo: another version, no nonce or a bad one
  BadGreeting(String),
  /// Not a line of the handshake: a frame of JSON-RPC
  Application,
}

#[derive(Deserialize)]
struct HandshakeFields {
  #[serde(default)]
  proto: Option<String>,
  #[serde(default)]
  v: Option<Value>,
  #[serde(default)]
  nonce: Option<Value>,
  #[serde(default)]
  error: Option<Value>,
}

/// What a line of the IDE is to the handshake: a line with `"proto":"local-peer"` is a greeting or a refusal, any other
/// line is the application's
pub(crate) fn handshake_line(line: &[u8]) -> HandshakeLine {
  let Ok(HandshakeFields { proto, v, nonce, error }) = serde_json::from_slice::<HandshakeFields>(line) else {
    return HandshakeLine::Application;
  };
  if proto.as_deref() != Some(HANDSHAKE_PROTO) {
    return HandshakeLine::Application;
  }
  if let Some(error) = error {
    return HandshakeLine::Refused(HandshakeRefusal::from(error.as_str().map_or_else(|| error.to_string(), str::to_owned)));
  }
  if v.as_ref().and_then(Value::as_u64) != Some(u64::from(HANDSHAKE_VERSION)) {
    return HandshakeLine::BadGreeting(format!("the greeting is not of handshake v{HANDSHAKE_VERSION}: {}", v.unwrap_or_default()));
  }
  match nonce.as_ref().and_then(Value::as_str) {
    Some(nonce) if is_nonce(nonce) => HandshakeLine::Greeting { nonce: nonce.to_owned() },
    _ => HandshakeLine::BadGreeting("the greeting has no nonce of base64url without padding".to_owned()),
  }
}

fn is_nonce(nonce: &str) -> bool {
  (1..=MAX_NONCE_CHARS).contains(&nonce.len()) && nonce.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Why the IDE refused the handshake (`{"proto":"local-peer","v":2,"error":…}`): the host did not read the request
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandshakeRefusal {
  /// The first line of the client is not the opener
  BadOpener,
  /// The envelope does not echo the nonce of the greeting
  BadNonce,
  /// Another process wrote the envelope than the one whose credentials the IDE read before the greeting
  PeerChanged,
  /// A code of a later handshake
  Other(String),
}

impl HandshakeRefusal {
  /// The code as the handshake writes it
  pub fn as_str(&self) -> &str {
    match self {
      Self::BadOpener => "BAD_OPENER",
      Self::BadNonce => "BAD_NONCE",
      Self::PeerChanged => "PEER_CHANGED",
      Self::Other(code) => code,
    }
  }
}

impl From<String> for HandshakeRefusal {
  fn from(code: String) -> Self {
    match code.as_str() {
      "BAD_OPENER" => Self::BadOpener,
      "BAD_NONCE" => Self::BadNonce,
      "PEER_CHANGED" => Self::PeerChanged,
      _ => Self::Other(code),
    }
  }
}

impl fmt::Display for HandshakeRefusal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

/// A frame of the IDE after the handshake
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Frame {
  /// The answer of the request `id`: its result, or its error
  Response { id: Value, outcome: Result<Value, RpcError> },
  /// A notice of the IDE (`root/sessionAdded`): a local app follows none
  Notification { method: String },
  /// A request of the IDE: a local app serves none
  Request { method: String },
}

#[derive(Deserialize)]
struct FrameFields {
  #[serde(default)]
  id: Option<Value>,
  #[serde(default)]
  method: Option<String>,
  #[serde(default)]
  result: Option<Value>,
  #[serde(default)]
  error: Option<RpcError>,
}

/// The frame of `line`
///
/// # Errors
/// The line is not JSON, or not a frame of JSON-RPC
pub(crate) fn parse_frame(line: &[u8]) -> Result<Frame, String> {
  let FrameFields { id, method, result, error } = serde_json::from_slice(line).map_err(|error| error.to_string())?;
  match (id, method) {
    (None | Some(Value::Null), Some(method)) => Ok(Frame::Notification { method }),
    (Some(_), Some(method)) => Ok(Frame::Request { method }),
    (Some(id), None) => Ok(Frame::Response {
      id,
      outcome: error.map_or_else(|| Ok(result.unwrap_or_default()), Err),
    }),
    (None, None) => Err("a frame without an id and a method".to_owned()),
  }
}

/// The error object of a JSON-RPC error
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RpcError {
  pub code: i64,
  #[serde(default)]
  pub message: String,
  #[serde(default)]
  pub data: Option<Value>,
}

impl RpcError {
  /// The refusal of the error: the code of `data.code`, else the code of the JSON-RPC error number; the message and the
  /// hint of `data`, else the message of the error
  pub fn refusal(&self) -> Refusal {
    let data = self.data.as_ref();
    let text = |field: &str| data.and_then(|data| data.get(field)).and_then(Value::as_str).map(str::to_owned);
    let code = text("code").unwrap_or_else(|| match self.code {
      -32700 | -32600 => "BAD_REQUEST".to_owned(),
      -32601 => "UNKNOWN_METHOD".to_owned(),
      -32602 => "BAD_PARAMS".to_owned(),
      code => format!("ERROR_{code}"),
    });
    Refusal {
      code,
      message: text("message").or_else(|| (!self.message.is_empty()).then(|| self.message.clone())),
      hint: text("hint"),
    }
  }
}

/// Why the IDE did not do what a request asked: the code of `error.data.code` (`DENIED`), and the English message and
/// hint of the IDE for a log
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
  pub code: String,
  pub message: Option<String>,
  pub hint: Option<String>,
}

impl fmt::Display for Refusal {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.message {
      Some(message) => write!(f, "{}: {message}", self.code),
      None => f.write_str(&self.code),
    }
  }
}

/// The result of `x-air-ij/hosts`. The IDE asks no consent for it, so it never opens a dialog
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsResult {
  /// The product code (`IU`)
  #[serde(default)]
  pub product: String,
  #[serde(default)]
  pub build: String,
  /// The launch directory of each open project once
  pub hosts: Vec<Host>,
  /// Unix milliseconds of the last focus of a window of the IDE; none before the first focus
  #[serde(default)]
  pub focused_at: Option<u64>,
}

/// An open project of the IDE
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Host {
  /// The launch directory of the project, absolute
  pub project_directory: PathBuf,
}

/// The params of `x-air-ij/openDraft`: the project and the prompt, and no route, so the IDE takes the default agent of
/// its New Session button
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DraftParams<'a> {
  /// The absolute path of the directory: the base of an open project, or a directory in it
  pub(crate) project_directory: &'a str,
  pub(crate) prompt: &'a str,
}
