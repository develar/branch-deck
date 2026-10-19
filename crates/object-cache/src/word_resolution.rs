//! `word_resolution`: (resolve version, base tree, commit tree, target tree) -> whether the conflicting cherry-pick of
//! those trees resolves by its words (`git_ops::word_resolve`): the tree that holds the resolved files and git's
//! conflicts in each, or that it does not resolve. The trees decide the merge and the resolve, so an entry never
//! becomes stale; `merge_result` keeps what git gives, and only a sync with the resolve on reads this store. In the merge
//! environment, beside `merge_result`. The version of the resolve is part of each key, and the first write of a process
//! with another version clears the store, as for `blob_summary`

use crate::cache::{ObjectCache, Store};
use crate::merge_result::MergeKey;
use serde::{Deserialize, Serialize};

/// What the resolve by words gave for the trees of a cherry-pick
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WordResolutionResult {
  /// The tree with the resolved files, and each file with the conflicts of git in it, in git's order
  Resolved {
    tree: String,
    files: Vec<(String, u32)>,
  },
  NotResolved,
}

fn key(version: u32, trees: &MergeKey) -> Vec<u8> {
  let mut key = version.to_be_bytes().to_vec();
  key.extend_from_slice(&trees.bytes());
  key
}

impl ObjectCache {
  /// The cached resolutions of the resolve of `version` for `keys`, in their order
  pub fn get_word_resolutions(&self, version: u32, keys: &[MergeKey]) -> Vec<Option<WordResolutionResult>> {
    let raw: Vec<Vec<u8>> = keys.iter().map(|trees| key(version, trees)).collect();
    self.get_many(Store::WordResolution, &raw)
  }

  /// Write the resolutions of the resolve of `version`, in one transaction
  pub fn put_word_resolutions(&self, version: u32, results: &[(MergeKey, WordResolutionResult)]) {
    if results.is_empty() {
      return;
    }
    self.keep_only_version(Store::WordResolution, version);
    let entries: Vec<(Vec<u8>, &WordResolutionResult)> = results.iter().map(|(trees, result)| (key(version, trees), result)).collect();
    self.put_many(Store::WordResolution, &entries);
  }
}
