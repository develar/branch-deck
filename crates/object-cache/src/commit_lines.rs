//! `commit_lines`: commit id -> the changed lines of the commit against its first parent in the source files whose
//! declarations the dangling-reference check reads (`trial_check::dangling_references`), with `-U0 --no-renames`

use crate::cache::{ObjectCache, Store};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The lines a commit removes from and adds to one file, in the order of the patch
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileLines {
  pub path: String,
  pub removed: Vec<String>,
  pub added: Vec<String>,
}

impl ObjectCache {
  /// The cached lines of `ids`, by id; missing ids are absent
  pub fn get_commit_lines(&self, ids: &[String]) -> HashMap<String, Vec<FileLines>> {
    let keys: Vec<Vec<u8>> = ids.iter().map(|id| id.as_bytes().to_vec()).collect();
    ids
      .iter()
      .zip(self.get_many::<Vec<FileLines>>(Store::CommitLines, &keys))
      .filter_map(|(id, files)| files.map(|files| (id.clone(), files)))
      .collect()
  }

  pub fn put_commit_lines<'a>(&self, lines: impl IntoIterator<Item = (&'a str, &'a Vec<FileLines>)>) {
    let entries: Vec<(Vec<u8>, &Vec<FileLines>)> = lines.into_iter().map(|(id, files)| (id.as_bytes().to_vec(), files)).collect();
    self.put_many(Store::CommitLines, &entries);
  }
}
