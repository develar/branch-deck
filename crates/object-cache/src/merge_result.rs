//! `merge_result`: (base tree, commit tree, target tree) -> what `git merge-tree --write-tree` produced, so that
//! restacks and `bd check` variants don't merge the same trees again

use crate::cache::{ObjectCache, Store};
use serde::{Deserialize, Serialize};

/// The three trees of a cherry-pick: the commit's parent tree, the commit's tree and the tree it is applied onto
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MergeKey {
  pub base_tree: String,
  pub commit_tree: String,
  pub target_tree: String,
}

impl MergeKey {
  fn bytes(&self) -> Vec<u8> {
    [self.base_tree.as_str(), self.commit_tree.as_str(), self.target_tree.as_str()].join("\0").into_bytes()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeResult {
  Clean { tree: String },
  Conflict { files: Vec<String> },
}

impl ObjectCache {
  /// The cached results of `keys`, in their order
  pub fn get_merge_results(&self, keys: &[MergeKey]) -> Vec<Option<MergeResult>> {
    let raw: Vec<Vec<u8>> = keys.iter().map(MergeKey::bytes).collect();
    self.get_many(Store::MergeResult, &raw)
  }

  pub fn put_merge_results(&self, results: &[(MergeKey, MergeResult)]) {
    let entries: Vec<(Vec<u8>, &MergeResult)> = results.iter().map(|(key, result)| (key.bytes(), result)).collect();
    self.put_many(Store::MergeResult, &entries);
  }
}
