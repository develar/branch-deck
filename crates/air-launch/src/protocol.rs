//! The messages of the launch socket, protocol v1: a request line (`targets`, `launch`, `watch`) and its response
//! lines, and the refusal codes. A response field the protocol adds later is ignored; a refusal code this client does not know stays
//! as its text (`RefusalCode::Other`). The types the page gets are `Air…` in its bindings (`serde(rename)`), beside the
//! types of the app.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The version of the protocol this client speaks (`"v"` of a request)
pub const PROTOCOL_VERSION: u32 = 1;

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

/// A request line: the version and the method before the fields of the method
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
