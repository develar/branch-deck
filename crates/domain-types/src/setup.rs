//! What in the setup of the machine waits for the user: the dot on Settings and its menu
//! (spec/repository/setup-attention.spec.md). The rule is `cli_tools::setup`; the page names the items.

use crate::cli_tools::CliTool;
use serde::{Deserialize, Serialize};

/// One thing to do, with what its action needs
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SetupItem {
  /// The update of Branch Deck is downloaded: Install and Restart
  AppUpdate { version: String },
  /// A newer release of a CLI than the installed one
  ToolUpdate { tool: CliTool, version: String },
  /// The agents have an older skill of the CLI than it bundles
  SkillUpdate { tool: CliTool, version: String },
  /// The token of a CLI for a server expires within a week
  SignInExpiring { tool: CliTool, server: String, expires_at: u32 },
  /// The token of a CLI for a server expired; it can't be skipped
  SignInExpired { tool: CliTool, server: String },
  /// The app's own YouTrack sign-in stopped working; it can't be skipped
  YouTrackSignIn,
}

impl SetupItem {
  /// The key a skip of it is kept by: an update by its version, a sign-in by its server
  pub fn key(&self) -> String {
    match self {
      Self::AppUpdate { version } => format!("app-update:{version}"),
      Self::ToolUpdate { tool, version } => format!("tool-update:{}:{version}", tool.command()),
      Self::SkillUpdate { tool, version } => format!("skill-update:{}:{version}", tool.command()),
      Self::SignInExpiring { tool, server, .. } => format!("sign-in-expiring:{}:{server}", tool.command()),
      Self::SignInExpired { tool, server } => format!("sign-in-expired:{}:{server}", tool.command()),
      Self::YouTrackSignIn => "youtrack-sign-in".to_string(),
    }
  }

  /// Skip This Version, Remind Tomorrow; a broken sign-in stays until it is fixed
  pub const fn skippable(&self) -> bool {
    !matches!(self, Self::SignInExpired { .. } | Self::YouTrackSignIn)
  }
}

/// A skipped item: hidden until `until` (Unix seconds) or for good (an update: until another version)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SetupSkip {
  pub key: String,
  pub until: Option<u32>,
}

/// The items, each with its key and whether it can be skipped, for the page
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SetupEntry {
  pub key: String,
  pub item: SetupItem,
  pub skippable: bool,
}
