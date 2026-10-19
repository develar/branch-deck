//! `commit_files`: commit id -> the changed files of the commit against its first parent with their line counts, as the
//! app lists them (`diff-tree --raw --numstat`). Unlike `commit_diff`, no hunks: counting needs no patch.

use crate::cache::{ObjectCache, Store};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How a file changed, from the status letter of `--raw`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FileStatus {
  Added,
  Modified,
  Deleted,
  Renamed,
  Copied,
  /// A file became a symlink or a submodule, or the other way round
  TypeChanged,
}

impl FileStatus {
  /// The status of a `--raw` status letter; another letter (`U`, `X`) is a modification
  pub const fn from_letter(letter: u8) -> Self {
    match letter {
      b'A' => Self::Added,
      b'D' => Self::Deleted,
      b'R' => Self::Renamed,
      b'C' => Self::Copied,
      b'T' => Self::TypeChanged,
      _ => Self::Modified,
    }
  }
}

/// A changed file of a commit
// the serde names are the app's; the cache encoding (postcard) is positional and doesn't see them
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
  /// The path after the commit
  pub path: String,
  pub status: FileStatus,
  /// The path before a rename or copy
  pub old_path: Option<String>,
  /// Added lines; 0 for a binary file
  pub additions: u32,
  /// Deleted lines; 0 for a binary file
  pub deletions: u32,
  /// Git counts no lines for the file (`-` in `--numstat`)
  pub binary: bool,
  /// The blob id before the commit, the commit of a submodule; `None` for an added file
  pub old_id: Option<String>,
  /// The blob id after the commit, the commit of a submodule; `None` for a deleted file
  pub new_id: Option<String>,
  /// A side is a submodule (mode 160000): its id is the commit the submodule points to
  pub submodule: bool,
}

impl ObjectCache {
  /// The cached files of `ids`, by id; missing ids are absent
  pub fn get_commit_files(&self, ids: &[String]) -> HashMap<String, Vec<CommitFile>> {
    let keys: Vec<Vec<u8>> = ids.iter().map(|id| id.as_bytes().to_vec()).collect();
    ids
      .iter()
      .zip(self.get_many::<Vec<CommitFile>>(Store::CommitFiles, &keys))
      .filter_map(|(id, files)| files.map(|files| (id.clone(), files)))
      .collect()
  }

  pub fn put_commit_files<'a>(&self, files: impl IntoIterator<Item = (&'a str, &'a Vec<CommitFile>)>) {
    let entries: Vec<(Vec<u8>, &Vec<CommitFile>)> = files.into_iter().map(|(id, files)| (id.as_bytes().to_vec(), files)).collect();
    self.put_many(Store::CommitFiles, &entries);
  }
}
