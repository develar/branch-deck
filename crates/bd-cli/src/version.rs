use crate::context::Environment;
use crate::output::SchemaVersion;
use git_executor::git_info::GitInfo;
use schemars::JsonSchema;
use serde::Serialize;

/// The version of Branch Deck, the one the app shows (`src-tauri/tauri.conf.json`, read by `build.rs`)
pub const VERSION: &str = env!("BD_VERSION");

/// `bd version` and `bd --version`
#[derive(Debug, Serialize, JsonSchema)]
pub struct VersionOutput {
  pub schema: SchemaVersion,
  /// The version of Branch Deck
  pub version: String,
  /// The version of the git bd runs (`BD_GIT`, else `git` on `PATH`, else Homebrew's git), also one older than 2.54;
  /// `null` when there is none
  pub git: Option<String>,
}

pub fn version(environment: &Environment) -> VersionOutput {
  let explicit = environment.git.as_deref();
  // a git too old for bd still has a version to report
  let git = GitInfo::discover_for_cli(explicit).or_else(|_| GitInfo::from_path(explicit.unwrap_or("git")));
  VersionOutput {
    schema: SchemaVersion,
    version: VERSION.to_string(),
    git: git.ok().map(|info| info.version),
  }
}
