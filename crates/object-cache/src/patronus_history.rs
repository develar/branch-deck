//! `patronus_history`: what the Safe Push statistics keep of Patronus (`sync_core::robot_stats`), each value keyed by a
//! hash its reader builds: a week of everyone's robots of a repository with when it was listed, the run of one of the
//! user's finished robots, and the salt of the owner keys. A finished robot does not change, so the reader decides
//! which entry it asks again; the value is opaque here.

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

impl ObjectCache {
  /// The kept values of `keys`, in their order; `None` for a key without one, or an unreadable one (another version)
  pub fn get_patronus_history<V: DeserializeOwned>(&self, keys: &[[u8; 32]]) -> Vec<Option<V>> {
    let keys: Vec<Vec<u8>> = keys.iter().map(|key| key.to_vec()).collect();
    self.get_many(Store::PatronusHistory, &keys)
  }

  /// Keep the values of `entries`, in one transaction
  pub fn put_patronus_history<V: Serialize>(&self, entries: &[([u8; 32], V)]) {
    let entries: Vec<(Vec<u8>, &V)> = entries.iter().map(|(key, value)| (key.to_vec(), value)).collect();
    self.put_many(Store::PatronusHistory, &entries);
  }
}
