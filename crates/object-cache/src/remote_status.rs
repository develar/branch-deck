//! `remote_status`: (version, a hash of the commits a remote status is a function of) -> that status without its push
//! time, opaque here (`sync_core::remote_status`). The commits are the branch's tip, its remote tip, the baseline and a
//! stacked branch's parent; the hash also covers the user's email (an unbounded string, so the key stays short) and
//! where the parent's commits came from. Its walks read only what these commits reach above the baseline, so a `bd`
//! sync of a branch whose commits did not move runs no walk. The version of the status is part of each key, and the
//! first write of a process with another version clears the store, as for `conflict_info`

use crate::cache::{ObjectCache, Store};
use serde::{Serialize, de::DeserializeOwned};

impl ObjectCache {
  /// The cached status of `key`, written by the status of `version`
  pub fn get_remote_status<V: DeserializeOwned>(&self, version: u32, key: &[u8; 32]) -> Option<V> {
    self.get_hashed(Store::RemoteStatus, version, key)
  }

  /// Write the statuses of `version` in one transaction
  pub fn put_remote_statuses<V: Serialize>(&self, version: u32, statuses: &[([u8; 32], V)]) {
    self.put_hashed(Store::RemoteStatus, version, statuses);
  }
}
