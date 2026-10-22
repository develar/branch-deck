//! The commits of the branch as `git-ops` reads them (`commit_list`, the mapping notes of `notes`) and as the page names
//! them back

use serde::{Deserialize, Serialize};

/// Simple commit information with hash and message.
/// Used for passing commit data between frontend and backend.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CommitInfo {
  pub hash: String,
  pub message: String,
}

/// Struct to hold commit data returned by git CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Commit {
  #[serde(rename = "originalHash")]
  pub id: String,
  pub stripped_subject: String, // Subject with prefix removed (or same as subject if no prefix)
  pub message: String,          // Full commit message (including subject)
  #[serde(rename = "author")]
  pub author_name: String,
  #[serde(skip)]
  pub author_email: String,
  #[serde(rename = "authorTime")]
  pub author_timestamp: u32,
  #[serde(rename = "committerTime")]
  pub committer_timestamp: u32,
  /// Offset of the author date, as git stores it (`+0200`); empty if unknown
  #[serde(skip)]
  pub author_timezone: String,
  /// Offset of the committer date (`+0200`); empty if unknown
  #[serde(skip)]
  pub committer_timezone: String,
  #[serde(skip)]
  pub subject: String, // Original first line of the commit message
  #[serde(skip)]
  pub parent_id: Option<String>,
  #[serde(skip)]
  pub tree_id: String,
  #[serde(skip)]
  pub note: Option<String>,
  #[serde(skip)]
  pub mapped_commit_id: Option<String>, // Extracted from note if it has the `notes::PREFIX` prefix
  /// The trees the virtual commit of `mapped_commit_id` was copied from (`notes::SOURCE_PREFIX`); `None` for an older note
  #[serde(skip)]
  pub mapped_source: Option<NoteSource>,
  /// The resolve by words the virtual commit of `mapped_commit_id` took (`notes::RESOLVED_PREFIX`)
  #[serde(skip)]
  pub mapped_resolution: Option<NoteResolution>,
}

/// The trees a virtual commit was copied from: the original's parent tree and its tree
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSource {
  pub parent_tree: String,
  pub tree: String,
}

/// The resolve by words that a virtual commit took (`RESOLVED_PREFIX`): the version of the resolve, the conflicts of
/// git it resolved and in how many files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteResolution {
  pub version: u32,
  pub conflicts: u32,
  pub files: u32,
}
