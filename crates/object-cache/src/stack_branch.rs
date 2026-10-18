//! `stack_branch`: (version, a hash of the base and the original commits of a stack branch) -> what their build gave,
//! opaque here (`sync_core::stack_branch`): the tip, or the commit that conflicts and its files. The build is a function
//! of those commits (the copies are deterministic), so a sync whose stack has the same commits on the same base runs no
//! merge to know its stack branch is built, and `bd status` knows it without a sync. The version of the build is part
//! of each key, and the first write of a process with another version clears the store, as for `remote_status`

use crate::cache::{ObjectCache, Store, hashed_key};
use serde::{Serialize, de::DeserializeOwned};

impl ObjectCache {
  /// The builds of `keys`, written by the build of `version`, in their order
  pub fn get_stack_builds<V: DeserializeOwned>(&self, version: u32, keys: &[[u8; 32]]) -> Vec<Option<V>> {
    let raw: Vec<Vec<u8>> = keys.iter().map(|key| hashed_key(version, key)).collect();
    self.get_many(Store::StackBranch, &raw)
  }

  /// Write the builds of `version` in one transaction
  pub fn put_stack_builds<V: Serialize>(&self, version: u32, builds: &[([u8; 32], V)]) {
    self.put_hashed(Store::StackBranch, version, builds);
  }
}
