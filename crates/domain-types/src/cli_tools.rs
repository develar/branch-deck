//! The command-line tools of the team that Settings → Tools shows, installs and signs in (`cli-tools` crate,
//! `src-tauri/src/commands/cli_tools.rs`, spec/repository/cli-tools.spec.md).

use serde::{Deserialize, Serialize};

/// A CLI that agents and the app use
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum CliTool {
  /// JetBrains/teamcity-cli: the logs and tests of the builds of a robot
  Teamcity,
  /// JetBrains/youtrack-cli: the issues of the commits
  Youtrack,
}

impl CliTool {
  pub const ALL: [Self; 2] = [Self::Teamcity, Self::Youtrack];

  /// The name of its binary, as the user types it
  pub const fn command(self) -> &'static str {
    match self {
      Self::Teamcity => "teamcity",
      Self::Youtrack => "youtrack",
    }
  }
}

/// How a CLI is signed in to one server
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum CliToolSignInState {
  Authenticated,
  /// TeamCity's guest access: read-only, no token
  Guest,
  /// The CLI has the server but its token is missing, rejected or unreadable
  Failed,
}

/// A server the CLI is configured for
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CliToolServer {
  /// e.g. `https://buildserver.labs.intellij.net`, without a trailing `/`
  pub url: String,
  /// The login the token belongs to
  pub user: Option<String>,
  pub state: CliToolSignInState,
  /// Why `state` is `Failed`, in the CLI's words
  pub error: Option<String>,
  /// Unix seconds when the token expires; None for a token without an expiry (a permanent one)
  pub expires_at: Option<u32>,
}

/// The agent skill a CLI bundles (`teamcity skill`, `youtrack skill`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CliToolSkill {
  /// e.g. `teamcity-cli`
  pub name: String,
  /// The `version:` of the installed `SKILL.md`; None while it is not installed
  pub installed: Option<String>,
  /// The version this release of the CLI installs
  pub bundled: Option<String>,
  /// What the line of the skill offers besides Remove: `InstallSkill` while the agents have none, `UpdateSkill` while
  /// theirs is older than the bundled one
  pub action: Option<CliToolAction>,
}

/// A package manager that installs and updates a CLI
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PackageManager {
  Homebrew,
  Winget,
  Scoop,
}

/// The one thing a row offers next, decided in Rust; the skill has a line of its own
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum CliToolAction {
  Install,
  /// The CLI is signed in to no server
  SignIn,
  /// A newer release than the installed one
  Update,
  /// The agents get the skill the CLI bundles
  InstallSkill,
  /// The agents have an older skill than the one the CLI bundles
  UpdateSkill,
  /// The agents lose the skill
  RemoveSkill,
}

/// A CLI as Settings → Tools shows it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CliToolStatus {
  pub tool: CliTool,
  /// The binary found, e.g. `/opt/homebrew/bin/teamcity`; None while it is not installed
  pub binary: Option<String>,
  /// e.g. `1.6.0`; None when it is not installed or does not tell
  pub version: Option<String>,
  pub servers: Vec<CliToolServer>,
  /// The skill the CLI bundles for the agents; without the CLI, the one the agents may have of an earlier install
  pub skill: Option<CliToolSkill>,
  /// The newest release, once known; None before the first check or offline
  pub latest: Option<String>,
  /// The manager the app installs and updates it with; None when the machine has none the CLI is packaged for
  pub manager: Option<PackageManager>,
  pub action: Option<CliToolAction>,
  /// The command of `action` for a terminal, while the app can't run it itself (no package manager, a sign-in it
  /// doesn't start); None when the app runs it
  pub command: Option<String>,
  /// Why a part could not be read (the CLI timed out, answered what the app can't parse), in a sentence
  pub problem: Option<String>,
}
