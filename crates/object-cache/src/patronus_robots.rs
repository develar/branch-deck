//! `patronus_robots`: a hash of who asks Patronus for which robots (the server and the token, the Space repository, the
//! branch prefix) -> the user's robots as Patronus last answered them, with when, and why it is not asked for a while
//! (a rejected token, a backoff after a failure). Not keyed by object ids: the reader decides how long the answer holds
//! (`bd`'s `robot_list`); the value is opaque here.

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

impl ObjectCache {
  /// The kept answer of `key`; `None` when there is none, or it is unreadable (another version)
  pub fn get_patronus_robots<V: DeserializeOwned>(&self, key: &[u8; 32]) -> Option<V> {
    self.get_many(Store::PatronusRobots, &[key.to_vec()]).pop().flatten()
  }

  /// Keep the answer of `key`, in one transaction
  pub fn put_patronus_robots<V: Serialize>(&self, key: &[u8; 32], value: &V) {
    self.put_many(Store::PatronusRobots, &[(key.to_vec(), value)]);
  }
}
