//! Branch name suggestions from the user's Claude Code (`spec/branch/branch-name-suggestions.spec.md`)

use git_ops::model::CommitInfo;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SuggestBranchNamesParams {
  pub repository_path: String,
  /// The commits of the new branch; the message is not sent, the backend reads it from git with the changed files
  pub commits: Vec<CommitInfo>,
}

/// What a request for branch names ended with; a failure of git or of `claude` is an outcome, not an error
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum BranchNameSuggestionOutcome {
  /// One or two distinct names, the best first
  Suggested { names: Vec<String> },
  /// A newer request for this window replaced it: `claude` was killed
  Superseded,
  /// The `claude` CLI is not installed
  ClaudeMissing,
  /// `claude` is not logged in (`claude`, then `/login`)
  NotLoggedIn,
  /// git or `claude` failed, e.g. offline, overloaded or timed out
  Error { message: String },
}
