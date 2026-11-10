//! `commit_diff`: commit id -> the changes of the commit against its first parent, as `brd commits` reads them with
//! `git log -z --raw --numstat --no-abbrev -M` and the blocks of the diff views (`trial_check::diff_scan`)

use crate::cache::{ObjectCache, Store};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One `@@ -old_start,old_lines +new_start,new_lines @@` header
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
  pub old_start: u32,
  pub old_lines: u32,
  pub new_start: u32,
  pub new_lines: u32,
}

/// A changed file of a commit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
  /// `A`, `M`, `D`, `R` (rename), `T` (type change) or `C` (copy)
  pub status: char,
  /// The path after the commit
  pub path: String,
  /// The path before a rename or copy
  pub old_path: Option<String>,
  /// Its lines added and removed as the engine counts them, from the blocks of the diff views under `Default`
  pub additions: u32,
  pub deletions: u32,
  /// Its lines added and removed as git counts them (`--numstat`, the default algorithm pinned), 0 of a file git shows
  /// as binary: the engine aligns some blocks otherwise than git (a closing brace deleted and added again), so the two
  /// can differ. What the Shape of a review counts, which says what `git diff --stat` says
  pub git_additions: u32,
  pub git_deletions: u32,
  /// The engine shows no lines for the file: binary, a symlink, a submodule, over 8 MiB
  pub binary: bool,
  /// The blob of the file before the commit (of `old_path` for a rename or copy) and after it, full ids; `None` where
  /// the file doesn't exist, or is a symlink or a submodule: nothing to read as source
  pub old_blob: Option<String>,
  pub new_blob: Option<String>,
  /// In the order of the patch, ascending
  pub hunks: Vec<Hunk>,
}

impl ObjectCache {
  /// The cached changes of `ids`, by id; missing ids are absent
  pub fn get_commit_diffs(&self, ids: &[String]) -> HashMap<String, Vec<FileChange>> {
    let keys: Vec<Vec<u8>> = ids.iter().map(|id| id.as_bytes().to_vec()).collect();
    ids
      .iter()
      .zip(self.get_many::<Vec<FileChange>>(Store::CommitDiff, &keys))
      .filter_map(|(id, files)| files.map(|files| (id.clone(), files)))
      .collect()
  }

  pub fn put_commit_diffs<'a>(&self, diffs: impl IntoIterator<Item = (&'a str, &'a Vec<FileChange>)>) {
    let entries: Vec<(Vec<u8>, &Vec<FileChange>)> = diffs.into_iter().map(|(id, files)| (id.as_bytes().to_vec(), files)).collect();
    self.put_many(Store::CommitDiff, &entries);
  }
}
