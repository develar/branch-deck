//! Line counts per file, of a cached commit diff (`FileChange`) or of a diff of a tip against a base (`CommitFile`,
//! `git_ops::commit_files::read_files_from_base`).

use object_cache::commit_diff::FileChange;
use object_cache::commit_files::CommitFile;

/// The counts of one changed file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumstatEntry {
  /// The path after the change
  pub path: String,
  /// The path before a rename
  pub old_path: Option<String>,
  /// `None` for a binary file (`-` in `--numstat`)
  pub additions: Option<u32>,
  pub deletions: Option<u32>,
}

impl From<&FileChange> for NumstatEntry {
  fn from(change: &FileChange) -> Self {
    Self {
      path: change.path.clone(),
      old_path: change.old_path.clone(),
      additions: (!change.binary).then_some(change.additions),
      deletions: (!change.binary).then_some(change.deletions),
    }
  }
}

impl From<&CommitFile> for NumstatEntry {
  fn from(file: &CommitFile) -> Self {
    Self {
      path: file.path.clone(),
      old_path: file.old_path.clone(),
      additions: (!file.binary).then_some(file.additions),
      deletions: (!file.binary).then_some(file.deletions),
    }
  }
}
