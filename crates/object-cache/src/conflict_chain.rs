//! `conflict_chain`: (version, a hash of a virtual branch's full name and the commit it is built on) -> the copies a sync
//! wrote of the branch's commits before its conflict, with the originals they copy, opaque here
//! (`sync_core::conflict_chain`). Its copies are deterministic, so an unchanged sync of a conflicting branch takes them
//! instead of merging and writing them again; `refs/branch-deck/conflict/` keeps them from `git gc`. In the conflict
//! environment, beside the views of those conflicts. The version is part of each key, and the first write of a process
//! with another version clears the store, as for `conflict_info`

use crate::cache::{ObjectCache, Store};
use serde::{Serialize, de::DeserializeOwned};

impl ObjectCache {
  /// The cached chain of `key`, written by the copies of `version`
  pub fn get_conflict_chain<V: DeserializeOwned>(&self, version: u32, key: &[u8; 32]) -> Option<V> {
    self.get_hashed(Store::ConflictChain, version, key)
  }

  /// Write the chains of `version` in one transaction
  pub fn put_conflict_chains<V: Serialize>(&self, version: u32, chains: &[([u8; 32], V)]) {
    self.put_hashed(Store::ConflictChain, version, chains);
  }
}
