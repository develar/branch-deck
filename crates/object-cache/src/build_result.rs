//! `build_result`: (version, a hash of what a build of `bd check --build` sees) -> that it passed. The caller hashes the
//! command, the branch and the object ids of what it extracts, so a tree that passed is not built again; a failure is
//! never kept, because a build may fail by chance. The second key that is not an object id, after `review_answer`: an
//! entry stays valid as long as the build depends only on the tree (see `cache.rs`). The version of the key is part of
//! each key, and the first write of a process with another version clears the store, as for `conflict_info`

use crate::cache::{ObjectCache, Store, hashed_key};

impl ObjectCache {
  /// Whether the build of each of `keys`, hashed by the caller of `version`, passed; in their order
  pub fn passed_builds(&self, version: u32, keys: &[[u8; 32]]) -> Vec<bool> {
    let raw: Vec<Vec<u8>> = keys.iter().map(|key| hashed_key(version, key)).collect();
    self.contains_many(Store::BuildResult, &raw)
  }

  /// Remember that the builds of `keys` passed, in one transaction
  pub fn put_passed_builds(&self, version: u32, keys: &[[u8; 32]]) {
    let entries: Vec<([u8; 32], ())> = keys.iter().map(|key| (*key, ())).collect();
    self.put_hashed(Store::BuildResult, version, &entries);
  }
}
