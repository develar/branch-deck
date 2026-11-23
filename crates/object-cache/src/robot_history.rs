//! `robot_history`: the finished robots of the user that the sessions of the repository saw (the app's and those of
//! `brd mcp` alike), which the estimates of the running ones learn from beyond the 7 days a list of Patronus reaches
//! (`domain_policy::robot_estimate`). One value under one key: the cache is the repository's. Its reader merges it with
//! what it learnt before it writes it back (`repository_model::robot_history`); the value is opaque here.

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The key of the one value
const KEY: &[u8] = b"finished";

impl ObjectCache {
  /// The kept history; `None` when there is none, or it is unreadable (another version)
  pub fn get_robot_history<V: DeserializeOwned>(&self) -> Option<V> {
    self.get_many(Store::RobotHistory, &[KEY.to_vec()]).pop().flatten()
  }

  /// Keep `history`, in one transaction
  pub fn put_robot_history<V: Serialize>(&self, history: &V) {
    self.put_many(Store::RobotHistory, &[(KEY.to_vec(), history)]);
  }
}
