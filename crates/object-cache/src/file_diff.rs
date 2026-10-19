//! `file_diff`: (model version, whitespace policy, old blob id, new blob id) -> the diff of two blobs as the views show
//! it (`text_diff::model::TextDiff` with its scopes, opaque here). Blobs never change, so one entry serves every commit,
//! range and window that shows the pair. In the patch environment (`Environment::Patches`), beside the other bulky
//! values. The version of the model is part of each key; the first write of a process with another version clears the
//! store, as for `blob_summary`

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The key of a pair: the version, big-endian, the policy, then the two ids with a space between
fn key(version: u32, policy: u8, old_id: &str, new_id: &str) -> Vec<u8> {
  let mut key = Vec::with_capacity(6 + old_id.len() + new_id.len());
  key.extend_from_slice(&version.to_be_bytes());
  key.push(policy);
  key.extend_from_slice(old_id.as_bytes());
  key.push(b' ');
  key.extend_from_slice(new_id.as_bytes());
  key
}

/// The blob pair of a diff and its policy; an added or a deleted file has the empty id on its missing side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiffKey<'a> {
  pub policy: u8,
  pub old_id: &'a str,
  pub new_id: &'a str,
}

impl ObjectCache {
  /// The cached diffs of `pairs` by model `version`, in their order
  pub fn get_file_diffs<V: DeserializeOwned>(&self, version: u32, pairs: &[DiffKey<'_>]) -> Vec<Option<V>> {
    let keys: Vec<Vec<u8>> = pairs.iter().map(|pair| key(version, pair.policy, pair.old_id, pair.new_id)).collect();
    self.get_many(Store::FileDiff, &keys)
  }

  /// Write the diffs of model `version`, in one transaction
  pub fn put_file_diffs<V: Serialize>(&self, version: u32, diffs: &[(DiffKey<'_>, &V)]) {
    if diffs.is_empty() {
      return;
    }
    self.keep_only_version(Store::FileDiff, version);
    let entries: Vec<(Vec<u8>, &V)> = diffs.iter().map(|&(pair, diff)| (key(version, pair.policy, pair.old_id, pair.new_id), diff)).collect();
    self.put_many(Store::FileDiff, &entries);
  }
}
