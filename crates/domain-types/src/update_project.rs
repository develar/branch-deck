//! Update Project: whether the checked out branch can be brought onto its upstream, and what the update did
//! (`git-ops`: `update_branch`, `word_replay`). The update of the app is `update`.
//!
//! The limits the `///` comments name are constants of the functions that build these types, not of this crate:
//! `MAX_ATTRIBUTED_FILES` in `git_ops::paths`, `MAX_UPSTREAM_COMMITS` and `MAX_BLOCKED_PATHS` in
//! `git_ops::update_branch`. The `///` comments themselves stay as they are: specta writes them into `bindings.ts`.

use crate::conflict::ResolvedFileSummary;
use serde::{Deserialize, Serialize};

/// How the branch reaches the upstream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UpdateKind {
  /// No local commits: the branch moves to the upstream
  FastForward,
  /// The local commits are replayed onto the upstream
  Rebase,
}

/// A commit of a conflicting update: a local one that would be replayed, or an upstream one
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateCommit {
  pub id: String,
  pub subject: String,
  pub author: String,
  /// Author time, Unix seconds
  pub time: u32,
  /// Who wrote the commit object: the author, or whoever rebased or landed it (a Safe Push robot)
  pub committer: String,
  /// Committer time, Unix seconds: for an upstream commit, about when it reached the upstream
  pub committer_time: u32,
  /// The likely conflicting files that the commit changes; empty for most commits
  pub files: Vec<String>,
}

/// Whether the branch can be updated, relative to the last fetch of the upstream
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UpdateState {
  /// The upstream has nothing the branch lacks
  UpToDate,
  /// The update applies without conflicts and without touching local changes
  Clean { kind: UpdateKind },
  /// Replaying the local commits onto the upstream conflicts; nothing was changed
  Conflict {
    /// The files that likely conflict, at most the first `MAX_ATTRIBUTED_FILES`: the ones with their commits
    files: Vec<String>,
    /// All files that likely conflict: a monorepo update can conflict in thousands
    #[serde(rename = "fileCount")]
    file_count: u32,
    /// The local commits, oldest first
    commits: Vec<UpdateCommit>,
    /// The upstream commits that changed the likely conflicting files, oldest first, at most the newest
    /// `MAX_UPSTREAM_COMMITS`
    #[serde(rename = "upstreamCommits")]
    upstream_commits: Vec<UpdateCommit>,
  },
  /// Uncommitted or staged changes, or untracked files, sit on paths the update would write; nothing was changed
  Blocked {
    /// The paths, at most the first `MAX_BLOCKED_PATHS`
    paths: Vec<String>,
    /// All paths: untracked files that the upstream adds can be thousands
    #[serde(rename = "pathCount")]
    path_count: u32,
  },
}

/// The result of checking the branch against its upstream
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
  /// The checked out branch, short name
  pub branch: String,
  /// The upstream as the user knows it, e.g. `origin/master`
  pub upstream: String,
  pub head: String,
  pub upstream_head: String,
  /// The merge base of HEAD and the upstream
  pub base: String,
  pub state: UpdateState,
  /// When the upstream was last fetched (Unix seconds), if known
  pub fetched_at: Option<u32>,
  /// The local commits whose conflict of git the replay resolved by words (`branchdeck.resolveByWords`), oldest first;
  /// empty when the replay applies as it is or conflicts
  #[cfg_attr(feature = "specta", specta(optional))]
  pub resolved_by_words: Vec<UpdateResolvedCommit>,
}

/// What an applied update did
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppliedUpdate {
  pub kind: UpdateKind,
  pub new_head: String,
  /// Local commits replayed onto the upstream
  pub replayed: u32,
  /// Local commits whose changes are already upstream: `git replay` drops a commit that becomes empty
  pub dropped_already_upstream: u32,
}

/// A local commit whose conflict of git an update resolved by words
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateResolvedCommit {
  /// The local commit
  pub commit: String,
  pub subject: String,
  /// The commit the update writes for it; a preview check writes it with a fixed committer date
  pub new_id: String,
  /// The conflicts of git that the words resolved: the `<<<<<<<` lines git would have left
  pub conflicts: u32,
  pub files: Vec<ResolvedFileSummary>,
}
