//! The messages of the local AHP socket of Air for a local app: the lines of the local peer handshake v2 (the opener,
//! the greeting with its nonce, the envelope and a refusal of the handshake), the JSON-RPC 2.0 frames after it (a request
//! of `ahp-root://`, its result or error, a notice of the root), the results a local app reads and the refusal of an
//! error (`error.data.code`). A field the IDE adds later is ignored; a refusal code this client does not know stays as its
//! text. The types the page gets are `Air…` in its bindings (`serde(rename)`), beside the types of the app.

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
  /// A notice of the root (`root/sessionAdded`)
  Notification(Notification),
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
  params: Option<Value>,
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
  let FrameFields {
    id,
    method,
    params,
    result,
    error,
  } = serde_json::from_slice(line).map_err(|error| error.to_string())?;
  match (id, method) {
    (None | Some(Value::Null), Some(method)) => Ok(Frame::Notification(Notification {
      method,
      params: params.unwrap_or_default(),
    })),
    (Some(_), Some(method)) => Ok(Frame::Request { method }),
    (Some(id), None) => Ok(Frame::Response {
      id,
      outcome: error.map_or_else(|| Ok(result.unwrap_or_default()), Err),
    }),
    (None, None) => Err("a frame without an id and a method".to_owned()),
  }
}

/// A notice of the IDE: its method and its params
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
  pub method: String,
  pub params: Value,
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

/// The result of `x-air-ij/targets`
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct TargetsResult {
  pub targets: Vec<Target>,
}

/// An agent that Air can start, with the models it reports
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirTarget", rename_all = "camelCase")]
pub struct Target {
  /// The agent (`codex`); two targets can have the same agent on another surface
  pub agent_id: String,
  pub launch_target_id: String,
  pub interaction_surface_id: String,
  /// The name to show ("Codex")
  pub name: String,
  /// Empty when the agent reports no list: then a launch names no model, and any effort passes to the agent
  #[serde(default)]
  pub models: Vec<Model>,
  /// The modes a launch can name (`LaunchRequest::mode`); empty when the agent has none
  #[serde(default)]
  pub modes: Vec<Mode>,
  /// A launch can start in Plan (`LaunchRequest::plan`)
  #[serde(default)]
  pub plan: bool,
  /// The IDE answered before the agent listed its models and modes: they can still change, and the targets asked again
  /// later have them
  #[serde(default)]
  pub lists_loading: bool,
}

/// A mode of a target
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirMode", rename_all = "camelCase")]
pub struct Mode {
  pub id: String,
  pub name: String,
  pub kind: ModeKind,
}

/// What a mode does. A target whose Plan is a mode has a mode of kind `plan`: `plan` of a launch takes it
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirModeKind", rename_all = "snake_case")]
pub enum ModeKind {
  Plan,
  AutoReview,
  Standard,
  FullAccess,
  /// A kind of a later IDE
  #[serde(other)]
  Other,
}

/// A model of a target
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirModel", rename_all = "camelCase")]
pub struct Model {
  pub id: String,
  pub name: String,
  /// What tells two models of the same name apart, if anything
  #[serde(default)]
  pub qualifier: Option<String>,
  /// The model the agent takes when a launch names none
  #[serde(default)]
  pub is_default: bool,
  /// The lowercase effort ids the model takes; `auto` is not one: a launch without an effort keeps the agent's default
  #[serde(default)]
  pub reasoning_efforts: Vec<String>,
}

/// What a launch does in the IDE
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirStartKind", rename_all = "camelCase")]
pub enum StartKind {
  /// Starts the session and sends the prompt (`createSession`)
  Run,
  /// Opens a New Session tab with the agent, the model, the effort and the prompt, and sends nothing
  /// (`x-air-ij/openDraft`)
  Draft,
}

/// A launch as the page asks it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirLaunchRequest", rename_all = "camelCase")]
pub struct LaunchRequest {
  /// The absolute path of the directory: the base of an open project, or a directory in it
  pub project: String,
  /// The target, with `interaction_surface_id`; none starts the default of the New Session button of the IDE
  #[serde(default)]
  pub launch_target_id: Option<String>,
  /// The surface of the target; only with `launch_target_id`
  #[serde(default)]
  pub interaction_surface_id: Option<String>,
  /// An id or a name of a model of the target; none for a target without models
  #[serde(default)]
  pub model: Option<String>,
  /// One effort id; none keeps the default of the agent
  #[serde(default)]
  pub effort: Option<String>,
  /// The id of a mode of the target; none keeps the default of the agent
  #[serde(default)]
  pub mode: Option<String>,
  /// The session starts in Plan; false is not sent. With a mode of kind `plan` of the target, Plan is that mode, and
  /// another `mode` beside it is `MODE_CONFLICT`
  #[serde(default)]
  pub plan: bool,
  /// The effort id of Plan; only with `plan`, none keeps the default of the agent
  #[serde(default)]
  pub plan_effort: Option<String>,
  /// Required for `run`; a draft without one opens with an empty prompt
  #[serde(default)]
  pub prompt: Option<String>,
  /// The title the session list shows; only for `run`
  #[serde(default)]
  pub title: Option<String>,
  pub start: StartKind,
}

impl LaunchRequest {
  /// The params of `createSession` (`run`) or `x-air-ij/openDraft` (`draft`), which has no title
  pub(crate) fn params(&self) -> SessionParams<'_> {
    SessionParams {
      project_directory: &self.project,
      prompt: self.prompt.as_deref().unwrap_or_default(),
      launch_target_id: self.launch_target_id.as_deref(),
      interaction_surface_id: self.launch_target_id.as_ref().and(self.interaction_surface_id.as_deref()),
      model: self.model.as_deref(),
      effort: self.effort.as_deref(),
      mode: self.mode.as_deref(),
      plan: self.plan,
      plan_effort: self.plan_effort.as_deref().filter(|_| self.plan),
      title: self.title.as_deref().filter(|_| self.start == StartKind::Run),
    }
  }
}

/// The params of `createSession` and `x-air-ij/openDraft`: an option that is not set is not sent
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionParams<'a> {
  pub(crate) project_directory: &'a str,
  pub(crate) prompt: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) launch_target_id: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) interaction_surface_id: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) model: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) effort: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) mode: Option<&'a str>,
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub(crate) plan: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) plan_effort: Option<&'a str>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) title: Option<&'a str>,
}

/// The params of a method of one host: `x-air-ij/targets`
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostParams<'a> {
  pub(crate) project_directory: &'a str,
}

/// The result of `createSession`
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionResult {
  pub session: SessionIdentity,
  /// The id that the agent gives the session, when the start knows it
  #[serde(default)]
  pub agent_session_id: Option<String>,
}

/// The local identity of a session: the key of its row
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentity {
  pub project_directory: String,
  /// The `resource` of the row of the session
  pub local_session_id: String,
}

/// The params of `listSessions`
#[derive(Debug, Serialize)]
pub(crate) struct ListSessionsParams<'a> {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) cursor: Option<&'a str>,
}

/// The result of `listSessions`: a page of the rows, newest first
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListSessionsResult {
  pub(crate) items: Vec<SessionSummary>,
  /// Present exactly when more rows follow
  #[serde(default)]
  pub(crate) next_cursor: Option<String>,
}

/// The row of a session, as `listSessions` and the root notices give it
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct SessionSummary {
  /// The local session id
  pub resource: String,
  #[serde(default)]
  pub title: Option<String>,
  /// The facts of Air of the row: `_meta["x-air-ij"]`, with its `activity`
  #[serde(default, rename = "_meta")]
  pub meta: Option<Value>,
}

impl SessionSummary {
  /// What the session does now (`_meta["x-air-ij"].activity`)
  pub fn activity(&self) -> Activity {
    Activity::of_meta(self.meta.as_ref())
  }
}

/// The fields of a row that moved (`root/sessionSummaryChanged`); a field that did not move is absent
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct SummaryChanges {
  #[serde(default)]
  pub title: Option<String>,
  /// The facts of Air of the row, whole, when one of them moved
  #[serde(default, rename = "_meta")]
  pub meta: Option<Value>,
}

impl SummaryChanges {
  /// The activity of the facts of Air that moved; none when they did not move
  pub fn activity(&self) -> Option<Activity> {
    self.meta.as_ref().map(|meta| Activity::of_meta(Some(meta)))
  }
}

/// A change of the rows of the root, of a notice of the root
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootChange {
  Added(SessionSummary),
  Removed { session: String },
  Changed { session: String, changes: SummaryChanges },
}

#[derive(Deserialize)]
struct AddedParams {
  summary: SessionSummary,
}

#[derive(Deserialize)]
struct RemovedParams {
  session: String,
}

#[derive(Deserialize)]
struct ChangedParams {
  session: String,
  changes: SummaryChanges,
}

impl RootChange {
  /// The change of a notice; none for a notice of another method
  ///
  /// # Errors
  /// A notice of the root with params of another shape
  pub fn of(notification: &Notification) -> Result<Option<Self>, String> {
    let params = || notification.params.clone();
    let parsed = match notification.method.as_str() {
      "root/sessionAdded" => serde_json::from_value::<AddedParams>(params()).map(|it| Self::Added(it.summary)),
      "root/sessionRemoved" => serde_json::from_value::<RemovedParams>(params()).map(|it| Self::Removed { session: it.session }),
      "root/sessionSummaryChanged" => serde_json::from_value::<ChangedParams>(params()).map(|it| Self::Changed {
        session: it.session,
        changes: it.changes,
      }),
      _ => return Ok(None),
    };
    parsed.map(Some).map_err(|error| format!("{}: {error}", notification.method))
  }
}

/// What a session does now
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirSessionActivity", rename_all = "snake_case")]
pub enum Activity {
  /// The agent waits for a prompt: the turn is done, and the user read it
  Ready,
  /// A New Session tab that sent nothing yet
  Draft,
  /// The agent works
  Processing,
  /// The changes of the agent wait for the review of the user
  Reviewing,
  /// The agent asks the user
  NeedsInput,
  /// The turn is done, and the user did not read it yet
  Unread,
  /// An activity of a later IDE, or none
  Other,
}

impl Activity {
  /// The activity of the facts of Air of a row, `_meta` with `x-air-ij.activity`
  fn of_meta(meta: Option<&Value>) -> Self {
    Self::of(meta.and_then(|meta| meta.get("x-air-ij")).and_then(|facts| facts.get("activity")))
  }

  /// The activity of its name: the name of `AgentSessionActivity` of the IDE (`NEEDS_INPUT`), in any case and with or
  /// without `_`; any other value is `Other`
  pub fn of(activity: Option<&Value>) -> Self {
    let Some(name) = activity.and_then(Value::as_str) else {
      return Self::Other;
    };
    let name: String = name.chars().filter(|it| *it != '_').map(|it| it.to_ascii_lowercase()).collect();
    match name.as_str() {
      "ready" => Self::Ready,
      "draft" => Self::Draft,
      "processing" => Self::Processing,
      "reviewing" => Self::Reviewing,
      "needsinput" => Self::NeedsInput,
      "unread" => Self::Unread,
      _ => Self::Other,
    }
  }
}
