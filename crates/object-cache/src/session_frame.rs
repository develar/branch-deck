//! `session_frame`: (the version of the frame's schema, a hash of a worktree) -> the last frame its session committed
//! (`repository_model::warm_start`): the parts of its table and the values of its facts, from which the next start of
//! the app shows the branches before its first sync. Not keyed by object ids: one frame per worktree, which the next
//! save replaces, and the session that reads it checks it is of its own key. The value is the frame as its writer
//! encoded it (JSON: the parts of a view are tagged by a field, which postcard can't read back), opaque here. The
//! version is part of each key, and the first write of a process with another version clears the store, so a frame of
//! another schema is never read

use crate::cache::{ObjectCache, Store};

impl ObjectCache {
  /// The frame of the worktree of `key`, saved by the schema `version`
  pub fn get_session_frame(&self, version: u32, key: &[u8; 32]) -> Option<String> {
    self.get_hashed(Store::SessionFrame, version, key)
  }

  /// Keep `frame` as the frame of the worktree of `key`, of the schema `version`, in one transaction
  pub fn put_session_frame(&self, version: u32, key: &[u8; 32], frame: &str) {
    self.put_hashed(Store::SessionFrame, version, &[(*key, frame)]);
  }
}
