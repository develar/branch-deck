//! Unattended Mode (openspec/specs/repository/unattended-mode/spec.md): while the user is away, the `PermissionRequest`
//! hook of Claude Code (`brd hook permission-request`) denies every permission request of a coding agent, which goes on
//! without it, and records it (`UnattendedDenial`); what `brd unattended` and the app answer of the mode
//! (`UnattendedMode`), and whether the hook is installed (`UnattendedHook`). The journal and the switch are
//! `app_settings::unattended`', the hook and its install `bd-cli`'s.

use serde::{Deserialize, Serialize};

/// A permission request the hook denied while the mode was on: one line of the journal of denials. Never the content of
/// a file or of an edit: the tool and the one field that says what it was asked for (`summary`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct UnattendedDenial {
  /// When, in Unix seconds
  pub time: u32,
  /// The Claude Code session that asked
  #[serde(default)]
  pub session_id: String,
  /// The subagent that asked; none for the main session
  #[serde(default, skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub agent_id: Option<String>,
  /// The kind of that subagent (`general-purpose`, the name of an agent of the user)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub agent_type: Option<String>,
  /// The working directory of the session
  #[serde(default)]
  pub cwd: String,
  /// The tool it asked to run: `Bash`, `Read`, `WebFetch`, an MCP tool
  pub tool_name: String,
  /// What the tool was asked for, cut to `SUMMARY_LIMIT` characters: the command of `Bash`, the path of a file tool, the
  /// URL of a fetch, the pattern of a search; else the names of its arguments
  #[serde(default)]
  pub summary: String,
}

/// The longest summary of a denial, in characters; a longer one is cut and ends with `…`
pub const SUMMARY_LIMIT: usize = 200;

/// The mode as `brd unattended` and the app answer it: on or off, whether the hook is there to deny, and the requests
/// denied since the mode was last turned on (they stay after it is turned off, until it is turned on again)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnattendedMode {
  pub on: bool,
  /// Whether this call turned the mode on or off; false for a read, and for a switch to the state it was in
  pub changed: bool,
  /// A switch off: why the hook stayed in the settings of Claude Code; off all the same, as the hook denies nothing
  /// while the mode is off
  pub warning: Option<String>,
  /// Oldest first
  pub denials: Vec<UnattendedDenial>,
}
