//! The messages of the launch socket: the lines of the local peer handshake v2 (the opener, the greeting with its nonce,
//! the envelope and a refusal of the handshake), then the request of protocol v1 (`targets`, `launch`, `watch`) and its
//! response lines, and the refusal codes. A response field the protocol adds later is ignored; a refusal code this client
//! does not know stays as its text (`RefusalCode::Other`, `HandshakeRefusal::Other`). The types the page gets are `Air…`
//! in its bindings (`serde(rename)`), beside the types of the app.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The version of the request this client sends (`"v"` of the request object inside the envelope of the handshake)
pub const PROTOCOL_VERSION: u32 = 1;

/// The `proto` of each line of the local peer handshake, which the IDE shares with the Junie pairing socket
pub const HANDSHAKE_PROTO: &str = "local-peer";

/// The version of the local peer handshake this client speaks
pub const HANDSHAKE_VERSION: u32 = 2;

/// The first line of the client, before the IDE reads the credentials of the peer
pub(crate) const OPENER_LINE: &[u8] = b"{\"proto\":\"local-peer\",\"v\":2}\n";

/// The longest nonce of a greeting this client echoes; the IDE sends 128 bits in base64url, 22 characters
pub(crate) const MAX_NONCE_CHARS: usize = 64;

/// The bytes of the envelope around the request, with the longest nonce and the line end
pub(crate) const ENVELOPE_FRAME_BYTES: usize = r#"{"nonce":"","request":}"#.len() + MAX_NONCE_CHARS + 1;

/// The envelope line of the handshake: the echo of the nonce of the greeting and the request object. The nonce is
/// base64url (`handshake_line` checks it), so it needs no escape
pub(crate) fn envelope_line(nonce: &str, request: &[u8]) -> Vec<u8> {
  let mut line = Vec::with_capacity(ENVELOPE_FRAME_BYTES + request.len());
  line.extend_from_slice(br#"{"nonce":""#);
  line.extend_from_slice(nonce.as_bytes());
  line.extend_from_slice(br#"","request":"#);
  line.extend_from_slice(request);
  line.extend_from_slice(b"}\n");
  line
}

/// A line of the IDE as the handshake reads it
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HandshakeLine {
  /// The greeting: the client echoes its nonce in the envelope
  Greeting { nonce: String },
  /// The IDE refused the handshake, and the handler did not run
  Refused(HandshakeRefusal),
  /// A line of the handshake that is not a greeting this client can echo: another version, no nonce or a bad one
  BadGreeting(String),
  /// Not a line of the handshake: a response line of the application, also the answer of an IDE before handshake v2
  Application,
}

#[derive(Deserialize)]
struct HandshakeFields {
  #[serde(default)]
  proto: Option<String>,
  #[serde(default)]
  v: Option<serde_json::Value>,
  #[serde(default)]
  nonce: Option<serde_json::Value>,
  #[serde(default)]
  error: Option<serde_json::Value>,
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
  if v.as_ref().and_then(serde_json::Value::as_u64) != Some(u64::from(HANDSHAKE_VERSION)) {
    return HandshakeLine::BadGreeting(format!("the greeting is not of handshake v{HANDSHAKE_VERSION}: {}", v.unwrap_or_default()));
  }
  match nonce.as_ref().and_then(serde_json::Value::as_str) {
    Some(nonce) if is_nonce(nonce) => HandshakeLine::Greeting { nonce: nonce.to_owned() },
    _ => HandshakeLine::BadGreeting("the greeting has no nonce of base64url without padding".to_owned()),
  }
}

fn is_nonce(nonce: &str) -> bool {
  (1..=MAX_NONCE_CHARS).contains(&nonce.len()) && nonce.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Why the IDE refused the handshake (`{"proto":"local-peer","v":2,"error":…}`): the handler did not run
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
  /// A kind of a later protocol
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
  /// Starts the session and sends the prompt
  Run,
  /// Opens a New Session tab with the agent, the model, the effort and the prompt, and sends nothing
  Draft,
}

/// The fields of a `launch` request
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename = "AirLaunchRequest", rename_all = "camelCase")]
pub struct LaunchRequest {
  /// The absolute path of the directory: the base of an open project, or a directory in it
  pub project: String,
  pub launch_target_id: String,
  pub interaction_surface_id: String,
  /// An id or a name of a model of the target; none for a target without models
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  /// One effort id; none keeps the default of the agent
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  /// The id of a mode of the target; none keeps the default of the agent
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub mode: Option<String>,
  /// The session starts in Plan; false is not sent. With a mode of kind `plan` of the target, Plan is that mode, and
  /// another `mode` beside it is `MODE_CONFLICT`
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub plan: bool,
  /// The effort id of Plan; only with `plan` (else `BAD_REQUEST`), none keeps the default of the agent
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub plan_effort: Option<String>,
  /// Required for `run`, optional for `draft`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub prompt: Option<String>,
  /// The title the session list shows
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  pub start: StartKind,
}

/// A request object, inside the envelope of the handshake: the version and the method before the fields of the method
#[derive(Serialize)]
pub(crate) struct Envelope<'a, T> {
  pub(crate) v: u32,
  pub(crate) method: &'static str,
  #[serde(flatten)]
  pub(crate) fields: &'a T,
}

/// The fields of a `targets` request
#[derive(Serialize)]
pub(crate) struct TargetsRequest<'a> {
  pub(crate) project: &'a str,
}

/// The fields of a `watch` request
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WatchRequest<'a> {
  pub(crate) project: &'a str,
  pub(crate) session_id: &'a str,
}

/// The response to `targets`
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Targets {
  #[serde(rename = "ok")]
  Listed {
    targets: Vec<Target>,
  },
  Refused(Refusal),
}

/// The response to `launch`
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Launch {
  /// The session started; `session_id` is the id of its provider, none for a draft and for an agent of a terminal
  /// that has no id yet: then there is nothing to watch
  #[serde(rename_all = "camelCase")]
  Accepted {
    #[serde(default)]
    session_id: Option<String>,
  },
  Refused(Refusal),
}

/// A line of a `watch`: the status of the session now and after each change, then at most one end. A refusal before
/// the first status is any refusal of v1; after it, `NO_SESSION`
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum WatchLine {
  Session {
    session: Session,
  },
  /// The IDE stopped the watch: `timeout` after 4 h; a reason of a later protocol stays as its text
  End {
    reason: String,
  },
  Refused(Refusal),
}

/// The status of a session as a watch reports it
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
  pub session_id: String,
  #[serde(default)]
  pub title: Option<String>,
  pub activity: Activity,
  /// Unix milliseconds of the last change, if the IDE knows it
  #[serde(default)]
  pub updated_at: Option<u64>,
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
  /// An activity of a later protocol
  #[serde(other)]
  Other,
}

/// Why the IDE did not do what a request asked
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Refusal {
  pub code: RefusalCode,
  /// English text for a log, not for the user
  #[serde(default)]
  pub message: Option<String>,
}

/// The refusal codes of protocol v1. The client shows the code to the user and sends again only after a change
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "String")]
pub enum RefusalCode {
  /// The user denied the client, or the consent timed out
  Denied,
  /// The IDE cannot show the consent dialog (headless)
  Unavailable,
  /// Too many connections, or another consent is in flight
  Busy,
  /// More than 10 connections in 60 s
  RateLimited,
  UnsupportedVersion,
  UnknownMethod,
  /// The line is not JSON, a field has a wrong type, or the line is longer than 1 MiB
  BadRequest,
  /// No open project holds the directory
  NoProject,
  UntrustedProject,
  /// No available route has this launch target and interaction surface
  TargetUnavailable,
  /// The target lists models, and none has this id or name
  ModelUnavailable,
  /// Several models of the target have this name: send the id
  ModelAmbiguous,
  InvalidReasoningEffort,
  /// `run` with a blank prompt
  EmptyPrompt,
  /// The launch did not start; the message says why
  LaunchFailed,
  /// The target has no mode of this id
  ModeUnavailable,
  /// `plan` with another `mode`, while Plan is a mode of the target
  ModeConflict,
  /// `plan` for a target that cannot start in Plan
  PlanUnavailable,
  InvalidPlanReasoningEffort,
  /// The watched session is gone, or never was there
  NoSession,
  /// A code of a later protocol
  Other(String),
}

impl RefusalCode {
  /// The code as the protocol writes it
  pub fn as_str(&self) -> &str {
    match self {
      Self::Denied => "DENIED",
      Self::Unavailable => "UNAVAILABLE",
      Self::Busy => "BUSY",
      Self::RateLimited => "RATE_LIMITED",
      Self::UnsupportedVersion => "UNSUPPORTED_VERSION",
      Self::UnknownMethod => "UNKNOWN_METHOD",
      Self::BadRequest => "BAD_REQUEST",
      Self::NoProject => "NO_PROJECT",
      Self::UntrustedProject => "UNTRUSTED_PROJECT",
      Self::TargetUnavailable => "TARGET_UNAVAILABLE",
      Self::ModelUnavailable => "MODEL_UNAVAILABLE",
      Self::ModelAmbiguous => "MODEL_AMBIGUOUS",
      Self::InvalidReasoningEffort => "INVALID_REASONING_EFFORT",
      Self::EmptyPrompt => "EMPTY_PROMPT",
      Self::LaunchFailed => "LAUNCH_FAILED",
      Self::ModeUnavailable => "MODE_UNAVAILABLE",
      Self::ModeConflict => "MODE_CONFLICT",
      Self::PlanUnavailable => "PLAN_UNAVAILABLE",
      Self::InvalidPlanReasoningEffort => "INVALID_PLAN_REASONING_EFFORT",
      Self::NoSession => "NO_SESSION",
      Self::Other(code) => code,
    }
  }
}

impl From<String> for RefusalCode {
  fn from(code: String) -> Self {
    match code.as_str() {
      "DENIED" => Self::Denied,
      "UNAVAILABLE" => Self::Unavailable,
      "BUSY" => Self::Busy,
      "RATE_LIMITED" => Self::RateLimited,
      "UNSUPPORTED_VERSION" => Self::UnsupportedVersion,
      "UNKNOWN_METHOD" => Self::UnknownMethod,
      "BAD_REQUEST" => Self::BadRequest,
      "NO_PROJECT" => Self::NoProject,
      "UNTRUSTED_PROJECT" => Self::UntrustedProject,
      "TARGET_UNAVAILABLE" => Self::TargetUnavailable,
      "MODEL_UNAVAILABLE" => Self::ModelUnavailable,
      "MODEL_AMBIGUOUS" => Self::ModelAmbiguous,
      "INVALID_REASONING_EFFORT" => Self::InvalidReasoningEffort,
      "EMPTY_PROMPT" => Self::EmptyPrompt,
      "LAUNCH_FAILED" => Self::LaunchFailed,
      "MODE_UNAVAILABLE" => Self::ModeUnavailable,
      "MODE_CONFLICT" => Self::ModeConflict,
      "PLAN_UNAVAILABLE" => Self::PlanUnavailable,
      "INVALID_PLAN_REASONING_EFFORT" => Self::InvalidPlanReasoningEffort,
      "NO_SESSION" => Self::NoSession,
      _ => Self::Other(code),
    }
  }
}

impl fmt::Display for RefusalCode {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}
