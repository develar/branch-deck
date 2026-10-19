//! `conflict_info`: (version, resolve mode, commit, its parent, the commit it is applied onto) -> the conflict view a sync
//! reports of that cherry-pick (`git_ops::model::MergeConflictInfo`), opaque here. Its analysis reads only those commits and what
//! they reach, so a sync of an unchanged conflicting branch runs no git for its conflict. In the conflict environment
//! (`Environment::Conflicts`), beside the chains of `conflict_chain`. The version of the view is part of each key, and
//! the first write of a process with another version clears the store, as for `blob_summary`; a view above
//! `MAX_CONFLICT_INFO_BYTES` encoded is not kept

use crate::cache::{ObjectCache, Store};
use serde::{Serialize, de::DeserializeOwned};
use tracing::debug;

/// The largest conflict view kept, encoded: a view is a few kilobytes, so only a pathological one is left out
pub const MAX_CONFLICT_INFO_BYTES: usize = 64 << 20;

/// The full ids of a cherry-pick, and the resolve mode of the sync (`git_ops::word_resolve::ResolveMode::key`): a
/// conflict that a sync with the resolve off reports may resolve with it on
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConflictKey {
  pub resolve: u32,
  pub commit: String,
  pub parent: String,
  pub target: String,
}

impl ConflictKey {
  /// The version and the resolve mode, big-endian, then the three ids, NUL-separated
  fn bytes(&self, version: u32) -> Vec<u8> {
    [
      version.to_be_bytes().as_slice(),
      self.resolve.to_be_bytes().as_slice(),
      self.commit.as_bytes(),
      b"\0",
      self.parent.as_bytes(),
      b"\0",
      self.target.as_bytes(),
    ]
    .concat()
  }
}

impl ObjectCache {
  /// The cached conflict view of `key`, written by the analysis of `version`
  pub fn get_conflict_info<V: DeserializeOwned>(&self, version: u32, key: &ConflictKey) -> Option<V> {
    self.get_many(Store::ConflictInfo, &[key.bytes(version)]).pop().flatten()
  }

  /// Write the conflict views of the analysis of `version`, in one transaction; a view above `MAX_CONFLICT_INFO_BYTES`
  /// encoded is left out
  pub fn put_conflict_infos<V: Serialize>(&self, version: u32, infos: &[(ConflictKey, &V)]) {
    if !self.is_enabled() || infos.is_empty() {
      return;
    }
    self.keep_only_version(Store::ConflictInfo, version);
    let keys: Vec<Vec<u8>> = infos.iter().map(|(key, _)| key.bytes(version)).collect();
    let mut encoded = Vec::with_capacity(keys.len());
    for (bytes_key, (key, info)) in keys.iter().zip(infos) {
      match postcard::to_allocvec(info) {
        Ok(bytes) if bytes.len() <= MAX_CONFLICT_INFO_BYTES => encoded.push((bytes_key.as_slice(), bytes)),
        Ok(bytes) => debug!(commit = %key.commit, bytes = bytes.len(), "the conflict view is too big to keep"),
        Err(e) => debug!(error = %e, "conflict view not encodable"),
      }
    }
    self.put_encoded(Store::ConflictInfo, &encoded);
  }
}
