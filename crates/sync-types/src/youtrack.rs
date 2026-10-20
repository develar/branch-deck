//! The YouTrack sign-in and issues as the app shows them (`youtrack` crate, `src-tauri/src/commands/youtrack.rs`).

use serde::{Deserialize, Serialize};

/// Where the YouTrack token came from, in the order they are tried (`youtrack::service`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum YouTrackTokenSource {
  /// `YOUTRACK_TOKEN` with `YOUTRACK_BASE_URL`, as the `youtrack` CLI reads them
  Environment,
  /// The app's own sign-in in the browser (Hub OAuth with a refresh token)
  SignIn,
  /// A token the user pasted in Settings
  PastedToken,
}

/// Why the token is not used any more
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum YouTrackAuthProblem {
  /// YouTrack answered 401 to the token
  Rejected,
  /// Hub refused the refresh token: its time is over, or it was revoked
  Expired,
}

/// The user a token belongs to
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct YouTrackUser {
  pub login: String,
  pub full_name: String,
}

/// The YouTrack sign-in as Settings and the issue popover show it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct YouTrackAuthStatus {
  /// The YouTrack the token is for, e.g. `https://youtrack.example.com`; None without a token
  pub url: Option<String>,
  pub signed_in_as: Option<YouTrackUser>,
  pub source: Option<YouTrackTokenSource>,
  pub problem: Option<YouTrackAuthProblem>,
}

/// The page to open in the browser for the app's own sign-in; `wait_youtrack_sign_in` waits for its answer
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct YouTrackSignIn {
  pub authorize_url: String,
}

/// A single-value field of an issue, by the name YouTrack gives it: "Priority: Major", "Assignee: Jane Doe"
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct YouTrackField {
  pub name: String,
  pub value: String,
}

/// The state field of an issue: the field whose type is a state (`StateIssueCustomField`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct YouTrackState {
  /// The name of the field, usually "State"
  pub field: String,
  pub value: Option<String>,
  /// The value is a resolved state (Fixed, Duplicate, Obsolete…)
  pub resolved: bool,
  /// The field has the value `Fixed` (asked only for an issue that is not resolved): Mark Fixed sets it
  pub has_fixed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct YouTrackIssue {
  /// `IJPL-1`
  pub key: String,
  pub summary: String,
  /// The issue page: `<YouTrack>/issue/IJPL-1`
  pub url: String,
  /// Unix seconds; None while it is open
  pub resolved: Option<u32>,
  pub state: Option<YouTrackState>,
  /// The other single-value fields with a value, in YouTrack's order (enum, user, version, owned)
  pub fields: Vec<YouTrackField>,
}

/// The answer to an issue lookup: the issue, or why there is none
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum YouTrackIssueAnswer {
  Found {
    issue: YouTrackIssue,
  },
  /// No token, or the key is not of the YouTrack the token is for
  NotSignedIn,
  /// The token was rejected, or the sign-in expired
  SignInProblem {
    problem: YouTrackAuthProblem,
  },
  /// YouTrack answers 404 also for an issue the user may not see
  NotFound,
  Error {
    message: String,
  },
}

/// The state an issue had before a change of it, for its undo
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct YouTrackStateChange {
  pub field: String,
  pub previous: Option<String>,
}
