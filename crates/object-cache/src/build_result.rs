//! `build_result`: (version, a hash of what a build of `bd check --build` sees) -> that it passed. The caller hashes the
//! command, the branch and the object ids of what it extracts, so a tree that passed is not built again; a failure is
//! never kept, because a build may fail by chance. The second key that is not an object id, after `review_answer`: an
//! entry stays valid as long as the build depends only on the tree (see `cache.rs`). The version of the key is part of
//! each key, and the first write of a process with another version clears the store, as for `conflict_info`

use crate::cache::{ObjectCache, Store};
use std::sync::atomic::AtomicU64;

/// The version this process checked the store for
static CHECKED: AtomicU64 = AtomicU64::new(0);

/// The version, big-endian, then the hash
fn key_bytes(version: u32, key: &[u8; 32]) -> Vec<u8> {
  [version.to_be_bytes().as_slice(), key].concat()
}

impl ObjectCache {
  /// Whether the build of each of `keys`, hashed by the caller of `version`, passed; in their order
  pub fn passed_builds(&self, version: u32, keys: &[[u8; 32]]) -> Vec<bool> {
    let raw: Vec<Vec<u8>> = keys.iter().map(|key| key_bytes(version, key)).collect();
    self.contains_many(Store::BuildResult, &raw)
  }

  /// Remember that the builds of `keys` passed, in one transaction
  pub fn put_passed_builds(&self, version: u32, keys: &[[u8; 32]]) {
    if !self.is_enabled() || keys.is_empty() {
      return;
    }
    self.keep_only_version(Store::BuildResult, version, &CHECKED);
    let entries: Vec<(Vec<u8>, &())> = keys.iter().map(|key| (key_bytes(version, key), &())).collect();
    self.put_many(Store::BuildResult, &entries);
  }
}
