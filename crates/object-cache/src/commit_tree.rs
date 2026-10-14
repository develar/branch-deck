//! `commit_tree`: commit id -> tree id

use crate::cache::{ObjectCache, Store};
use std::collections::HashMap;

impl ObjectCache {
  /// The cached trees of `commits`, by commit; missing commits are absent
  pub fn get_commit_trees(&self, commits: &[String]) -> HashMap<String, String> {
    let keys: Vec<Vec<u8>> = commits.iter().map(|id| id.as_bytes().to_vec()).collect();
    commits
      .iter()
      .zip(self.get_many::<String>(Store::CommitTree, &keys))
      .filter_map(|(commit, tree)| tree.map(|tree| (commit.clone(), tree)))
      .collect()
  }

  /// `(commit, tree)` pairs
  pub fn put_commit_trees(&self, pairs: &[(String, String)]) {
    let entries: Vec<(Vec<u8>, &String)> = pairs.iter().map(|(commit, tree)| (commit.as_bytes().to_vec(), tree)).collect();
    self.put_many(Store::CommitTree, &entries);
  }
}
