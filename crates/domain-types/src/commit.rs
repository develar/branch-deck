//! The commits of the branch as `git-ops` reads them (`commit_list`, the mapping notes of `notes`) and as the page names
//! them back

use crate::stack::ParentSource;
use serde::{Deserialize, Serialize};

/// Simple commit information with hash and message.
/// Used for passing commit data between frontend and backend.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CommitInfo {
  pub hash: String,
  pub message: String,
}

/// Struct to hold commit data returned by git CLI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
  /// `subject` as the grouping reads it (`sync_utils::commit_subject::subject_parts`), for the page that shows and words
  /// it; the commit list writes it with `subject`
  pub subject_parts: SubjectParts,
}

/// A commit subject split by the grammar of the grouping (`sync_utils::commit_subject`), so the page shows its parts
/// (every field written: the object cache keeps it in `postcard`, which knows no skipped field)
/// without a grammar of its own: `fixup! (auth) IJPL-1: Add login` is the markers `fixup! `, the prefix `auth`, the rest
/// `IJPL-1: Add login` with its issue key, and what a row shows of it led by that key
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SubjectParts {
  /// The leading autosquash markers with the whitespace after them (`fixup! `, `fixup! squash! `); empty for none
  pub autosquash: String,
  /// The text of the `(prefix)` after the markers, unsanitized, without the parentheses
  pub prefix: Option<String>,
  /// The subject after the markers and the prefix
  pub rest: String,
  /// The issue key of the rest (`find_issue_number`: after an optional `[category]` and `type(scope):`), which groups a
  /// subject without a prefix
  pub issue_key: Option<String>,
  /// What a row shows of the subject (the subject of its virtual commit without the markers: the rest, or for a prefix
  /// the rest with the key of the prefix, `subject_after_prefix`) split at the issue key it starts with
  pub lead: Option<IssueLead>,
}

/// A title split at the issue key it starts with (`IJPL-1: Fix`, `[IJPL-1] Fix`, `IJPL-1 - Fix`), for a row that shows
/// the key apart (`sync_utils::issue_pattern::issue_lead`)
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IssueLead {
  /// The issue key
  pub key: String,
  /// What follows the key and its separator (`:`, `,`, `-`, `–`, `—`); empty for a title that is only the key
  pub text: String,
}

/// The branch a commit message groups into (`history_rewrite::commit_grouper::group_of`), as the forms that type a
/// message or a branch name preview it (`preview_group`)
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitGroup {
  /// The branch; with stacked branches the leaf of `(parent/leaf)`
  pub branch: String,
  pub origin: GroupOrigin,
  /// With stacked branches: the parent the prefix declares, else the stack parent of the branch; `None` for none and
  /// without stacked branches
  pub parent: Option<String>,
  /// With stacked branches and a parent: where it comes from
  pub parent_source: Option<ParentSource>,
  /// With stacked branches: the prefix has an empty level (`(a//b)`), so the branch reports an error
  pub invalid: bool,
}

/// The parameters of `preview_group`
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PreviewGroupParams {
  pub repository_path: String,
  /// The message typed (its subject as git reads it decides), or `(<name>)` for a typed branch name
  pub message: String,
}

/// The parameters of `commit_body_blocks`
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitBodyParams {
  /// The whole message: its body is the lines after the subject line
  pub message: String,
}

/// What groups a commit into its branch
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum GroupOrigin {
  /// An explicit `(branch)` subject prefix
  Prefix,
  /// The issue group of the subject's issue key (`refs/branch-deck/issue-groups`)
  IssueGroup,
  /// The issue key of the subject
  Issue,
}

impl Commit {
  /// The same commit as a window shows it: the fields that serialize, not the notes, the trees and the parent a sync
  /// reads with it. A sync after the one that wrote the mapping notes reads the same commits with their notes
  pub fn shows_as(&self, other: &Self) -> bool {
    self.id == other.id
      && self.stripped_subject == other.stripped_subject
      && self.message == other.message
      && self.author_name == other.author_name
      && self.author_timestamp == other.author_timestamp
      && self.committer_timestamp == other.committer_timestamp
  }
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
