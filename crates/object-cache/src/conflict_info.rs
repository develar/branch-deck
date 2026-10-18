//! `conflict_info`: (version, part, commit, its parent, the commit it is applied onto) -> a part of the conflict view
//! of that cherry-pick: the view a sync reports (`git_ops::model::MergeConflictInfo`) or the contents a view reads when
//! it shows the conflict (`git_ops::model::ConflictContents`), opaque here. Its analysis reads only those commits and
//! what they reach, so a sync of an unchanged conflicting branch runs no git for its conflict, and decodes no contents.
//! In an environment of its own (`Environment::Conflicts`): the contents are megabytes for a big file or many missing
//! commits, and must not push out the merges. The version of the view is part of each key, and the first write of a
//! process with another version clears the store, as for `commit_patch`; a part above `MAX_CONFLICT_INFO_BYTES`
//! encoded is not kept

use crate::cache::{ObjectCache, Store};
use serde::{Serialize, de::DeserializeOwned};
use tracing::debug;

/// The largest part of a conflict view kept, encoded: the conflict environment holds 8 such contents at least
pub const MAX_CONFLICT_INFO_BYTES: usize = 64 << 20;

/// The full ids of a cherry-pick
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConflictKey {
  pub commit: String,
  pub parent: String,
  pub target: String,
}

/// The two parts of a remembered conflict, each under a key of its own
#[derive(Debug, Clone, Copy)]
enum ConflictPart {
  /// The view a sync reports: commits, file names and the analysis, a few kilobytes
  View = 0,
  /// The file contents and the diffs a view shows
  Contents = 1,
}

impl ConflictKey {
  /// The version, big-endian, the part, then the three ids, NUL-separated
  fn bytes(&self, version: u32, part: ConflictPart) -> Vec<u8> {
    [
      version.to_be_bytes().as_slice(),
      &[part as u8],
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
    self.get_many(Store::ConflictInfo, &[key.bytes(version, ConflictPart::View)]).pop().flatten()
  }

  /// The cached contents of the conflict view of `key`, written by the analysis of `version`
  pub fn get_conflict_contents<C: DeserializeOwned>(&self, version: u32, key: &ConflictKey) -> Option<C> {
    self.get_many(Store::ConflictInfo, &[key.bytes(version, ConflictPart::Contents)]).pop().flatten()
  }

  /// Write the conflict views of the analysis of `version` with their contents, in one transaction; a part above
  /// `MAX_CONFLICT_INFO_BYTES` is left out, so a view may be kept without its contents
  pub fn put_conflict_infos<V: Serialize, C: Serialize>(&self, version: u32, infos: &[(ConflictKey, &V, &C)]) {
    if !self.is_enabled() || infos.is_empty() {
      return;
    }
    self.keep_only_version(Store::ConflictInfo, version);
    let mut keys = Vec::with_capacity(infos.len() * 2);
    let mut values = Vec::with_capacity(infos.len() * 2);
    for (key, info, contents) in infos {
      keys.push(key.bytes(version, ConflictPart::View));
      values.push((key, ConflictPart::View, postcard::to_allocvec(info)));
      keys.push(key.bytes(version, ConflictPart::Contents));
      values.push((key, ConflictPart::Contents, postcard::to_allocvec(contents)));
    }
    let mut encoded = Vec::with_capacity(keys.len());
    for (key, (conflict, part, value)) in keys.iter().zip(values) {
      match value {
        Ok(bytes) if bytes.len() <= MAX_CONFLICT_INFO_BYTES => encoded.push((key.as_slice(), bytes)),
        Ok(bytes) => debug!(commit = %conflict.commit, ?part, bytes = bytes.len(), "the conflict view is too big to keep"),
        Err(e) => debug!(error = %e, ?part, "conflict view not encodable"),
      }
    }
    self.put_encoded(Store::ConflictInfo, &encoded);
  }
}
