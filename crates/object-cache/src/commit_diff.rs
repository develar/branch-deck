//! `commit_diff`: commit id -> the changes of the commit against its first parent, as `bd commits` reads them with
//! `git log -z --raw --no-abbrev -p -U0 -M`

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
  pub additions: u32,
  pub deletions: u32,
  /// Git shows no lines for the file, as `-` in `--numstat`
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
