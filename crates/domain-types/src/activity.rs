//! Activity (spec/repository/activity.spec.md): the log of what Branch Deck did in a repository is read from journals.
//! The operations of the remote, of the app and of every `bd`, are the journal of their repository
//! (`operation::JournalEntry`, `$GIT_COMMON_DIR/branch-deck/journal.jsonl`); what only the page knows (an error it told,
//! an update, what landed, a rewrite, the news of a review request, an approval or a robot) is a `PageEntry` of the
//! journal of the app data directory, which `record_activity` writes. `read_activity` answers both (`ActivityLog`), and
//! the page words them.

use crate::landed::LandedBranch;
use crate::operation::{JournalEntry, OperationId};
use serde::{Deserialize, Serialize};

/// What a page entry is about: the chip of Activity it counts in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ActivityKind {
  /// An Update Project, applied or refused
  Update,
  /// A landed report a sync found after an update made elsewhere
  Landed,
  /// News of a Patronus robot: it merged, failed or is failing
  Patronus,
  /// A history rewrite with an Undo
  Rewrite,
  /// A review request or an approval
  Review,
  /// An error the page told
  Error,
}

/// What a page entry expands to, or opens
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ActivityDetails {
  /// The landed report, as the island showed it
  Landed { upstream: String, report: Vec<LandedBranch> },
  /// The robot page in the Patronus web UI
  Robot { url: String },
  /// A pull request on GitHub
  PullRequest { url: String },
}

/// What the page records (`record_activity`): of its repository, in the words of its toast
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PageActivity {
  /// The repository it is of, as the page names it (the path of a recent project)
  pub repository_path: String,
  pub kind: ActivityKind,
  pub title: String,
  pub description: Option<String>,
  /// A refused update, a failing robot
  #[serde(default)]
  pub warning: bool,
  pub details: Option<ActivityDetails>,
  /// The operation of a failed command (`ApiError.operation_id`): the id of its lines in the log, and of its entry in
  /// the journal of its repository when it was an operation of the remote
  pub operation_id: Option<OperationId>,
}

/// A line of the journal of the page
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PageEntry {
  /// A ULID, unique in the journal: the Undo of a rewrite is kept by it in the page
  pub id: String,
  /// Unix seconds of the record
  pub ended_at: u32,
  pub activity: PageActivity,
}

/// The journal of the repository of one or more recent projects (the worktrees of one repository share it)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RepositoryJournal {
  /// The file, as `subscribe_view` remembered it for the projects
  pub path: String,
  /// The paths of the recent projects whose repository it is the journal of
  pub repositories: Vec<String>,
  /// It answered, by the bound of the read (`ACTIVITY_READ_TIMEOUT`) or before it (`entries` its last answer then);
  /// false: its disk is gone, it can't be read or it never answered, `entries` is empty
  pub available: bool,
  /// Oldest first, as the file has them
  pub entries: Vec<JournalEntry>,
}

/// What Activity shows, as `read_activity` read it: the journals of the recent projects, each once, and the journal of
/// the page
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ActivityLog {
  pub journals: Vec<RepositoryJournal>,
  /// The entries of the page, of every repository, oldest first
  pub page: Vec<PageEntry>,
}

/// The parameters of `clear_activity`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ClearActivityParams {
  /// The recent project whose log goes: the journal of its repository and the page entries of it
  pub repository_path: String,
}
