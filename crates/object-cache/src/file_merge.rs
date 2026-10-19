//! `file_merge`: (model version, options, base blob id, left blob id, right blob id) -> the merge of three blobs as the
//! conflict view shows it (`text_diff::merge_model::TextMerge`, or the two-way diff of a one-sided conflict; opaque
//! here). Blobs never change, so one entry serves every conflict with the same three blobs. In the patch environment,
//! beside `file_diff`; the first write of a process with another version clears the store, as for `blob_summary`

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The three blobs of a merge and its options; a missing side has the empty id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileMergeKey<'a> {
  pub options: u8,
  pub base_id: &'a str,
  pub left_id: &'a str,
  pub right_id: &'a str,
}

/// The key of a merge: the version, big-endian, the options, then the three ids with a space between each
fn key(version: u32, merge: &FileMergeKey<'_>) -> Vec<u8> {
  let mut key = Vec::with_capacity(7 + merge.base_id.len() + merge.left_id.len() + merge.right_id.len());
  key.extend_from_slice(&version.to_be_bytes());
  key.push(merge.options);
  for (index, id) in [merge.base_id, merge.left_id, merge.right_id].into_iter().enumerate() {
    if index > 0 {
      key.push(b' ');
    }
    key.extend_from_slice(id.as_bytes());
  }
  key
}

impl ObjectCache {
  /// The cached merges of `merges` by model `version`, in their order
  pub fn get_file_merges<V: DeserializeOwned>(&self, version: u32, merges: &[FileMergeKey<'_>]) -> Vec<Option<V>> {
    let keys: Vec<Vec<u8>> = merges.iter().map(|merge| key(version, merge)).collect();
    self.get_many(Store::FileMerge, &keys)
  }

  /// Write the merges of model `version`, in one transaction
  pub fn put_file_merges<V: Serialize>(&self, version: u32, merges: &[(FileMergeKey<'_>, &V)]) {
    if merges.is_empty() {
      return;
    }
    self.keep_only_version(Store::FileMerge, version);
    let entries: Vec<(Vec<u8>, &V)> = merges.iter().map(|(merge, value)| (key(version, merge), *value)).collect();
    self.put_many(Store::FileMerge, &entries);
  }
}
