//! `commit_patch`: (limits version, subject id) -> the patch of a commit, or of a range `<base>..<head>`, cut into file
//! sections as the diff view reads it (`git_ops::commit_files::CommitPatch`, opaque here). In its own environment
//! (`Environment::Patches`), so that the patches never push out the other stores. The version of the limits that cut
//! the patch is part of each key; the first write of a process with another version clears the store, as for
//! `blob_summary`

use crate::cache::{ObjectCache, Store};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::atomic::AtomicU64;

/// The key of the patch of `id`: the version, big-endian, then the id
fn key(version: u32, id: &str) -> Vec<u8> {
  [version.to_be_bytes().as_slice(), id.as_bytes()].concat()
}

/// The version this process checked the store for
static CHECKED: AtomicU64 = AtomicU64::new(0);

impl ObjectCache {
  /// The cached patch of `id` cut by the limits of `version`
  pub fn get_commit_patch<V: DeserializeOwned>(&self, version: u32, id: &str) -> Option<V> {
    self.get_many(Store::CommitPatch, &[key(version, id)]).pop().flatten()
  }

  /// Whether a patch of each of `ids` by the limits of `version` is cached, in their order; reads no value
  pub fn has_commit_patches(&self, version: u32, ids: &[String]) -> Vec<bool> {
    let keys: Vec<Vec<u8>> = ids.iter().map(|id| key(version, id)).collect();
    self.contains_many(Store::CommitPatch, &keys)
  }

  /// Write the patches cut by the limits of `version`, in one transaction
  pub fn put_commit_patches<V: Serialize>(&self, version: u32, patches: &[(&str, &V)]) {
    if patches.is_empty() {
      return;
    }
    self.keep_only_version(Store::CommitPatch, version, &CHECKED);
    let entries: Vec<(Vec<u8>, &V)> = patches.iter().map(|&(id, patch)| (key(version, id), patch)).collect();
    self.put_many(Store::CommitPatch, &entries);
  }
}
