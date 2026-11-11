//! `file_diff`: (model version, whitespace policy, the grammar of each side, old blob id, new blob id) -> the diff of
//! two blobs as the views show it (`text_diff::model::TextDiff` with its scopes and its marks, opaque here). Blobs never change, so one entry serves every commit,
//! range and window that shows the pair. In the patch environment (`Environment::Patches`), beside the other bulky
//! values. The version of the model is part of each key; the first write of a process with another version clears the
//! store, as for `blob_summary`

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The key of a pair: the version, big-endian, the policy, the grammar of the old side and of the new one, then the
/// two ids with a space between
fn key(version: u32, pair: &DiffKey<'_>) -> Vec<u8> {
  let mut key = Vec::with_capacity(8 + pair.old_id.len() + pair.new_id.len());
  key.extend_from_slice(&version.to_be_bytes());
  key.push(pair.policy);
  key.extend_from_slice(&pair.grammars);
  key.extend_from_slice(pair.old_id.as_bytes());
  key.push(b' ');
  key.extend_from_slice(pair.new_id.as_bytes());
  key
}

/// The blob pair of a diff with what its diff depends on beside the blobs: the whitespace policy and the grammar the
/// path of each side is read by, the old side first (numbers of the caller, 0 for a side without a grammar), since the
/// scopes and the marks of a diff are those of its grammar. An added or a deleted file has the empty id on its missing
/// side
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiffKey<'a> {
  pub policy: u8,
  pub grammars: [u8; 2],
  pub old_id: &'a str,
  pub new_id: &'a str,
}

impl ObjectCache {
  /// The cached diffs of `pairs` by model `version`, in their order
  pub fn get_file_diffs<V: DeserializeOwned>(&self, version: u32, pairs: &[DiffKey<'_>]) -> Vec<Option<V>> {
    let keys: Vec<Vec<u8>> = pairs.iter().map(|pair| key(version, pair)).collect();
    self.get_many(Store::FileDiff, &keys)
  }

  /// Write the diffs of model `version`, in one transaction
  pub fn put_file_diffs<V: Serialize>(&self, version: u32, diffs: &[(DiffKey<'_>, &V)]) {
    if diffs.is_empty() {
      return;
    }
    self.keep_only_version(Store::FileDiff, version);
    let entries: Vec<(Vec<u8>, &V)> = diffs.iter().map(|(pair, diff)| (key(version, pair), *diff)).collect();
    self.put_many(Store::FileDiff, &entries);
  }
}
