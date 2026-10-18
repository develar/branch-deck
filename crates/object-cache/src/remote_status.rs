//! `remote_status`: (version, a hash of the commits a remote status is a function of) -> that status without its push
//! time, opaque here (`sync_core::remote_status`). The commits are the branch's tip, its remote tip, the baseline and a
//! stacked branch's parent; the hash also covers the user's email (an unbounded string, so the key stays short) and
//! where the parent's commits came from. Its walks read only what these commits reach above the baseline, so a `bd`
//! sync of a branch whose commits did not move runs no walk. The version of the status is part of each key, and the
//! first write of a process with another version clears the store, as for `conflict_info`

use crate::cache::{ObjectCache, Store};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::atomic::AtomicU64;

/// The version, big-endian, then the hash
fn key_bytes(version: u32, key: &[u8; 32]) -> Vec<u8> {
  [version.to_be_bytes().as_slice(), key].concat()
}

/// The version this process checked the store for
static CHECKED: AtomicU64 = AtomicU64::new(0);

impl ObjectCache {
  /// The cached status of `key`, written by the status of `version`
  pub fn get_remote_status<V: DeserializeOwned>(&self, version: u32, key: &[u8; 32]) -> Option<V> {
    self.get_many(Store::RemoteStatus, &[key_bytes(version, key)]).pop().flatten()
  }

  /// Write the statuses of `version` in one transaction
  pub fn put_remote_statuses<V: Serialize>(&self, version: u32, statuses: &[([u8; 32], V)]) {
    if !self.is_enabled() || statuses.is_empty() {
      return;
    }
    self.keep_only_version(Store::RemoteStatus, version, &CHECKED);
    let entries: Vec<(Vec<u8>, &V)> = statuses.iter().map(|(key, status)| (key_bytes(version, key), status)).collect();
    self.put_many(Store::RemoteStatus, &entries);
  }
}
