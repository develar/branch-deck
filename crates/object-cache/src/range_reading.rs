//! `range_reading`: (reading version, whitespace policy, base commit, head commit) -> what the range `base..head` leaves
//! to read once no later read can tell more (`domain_types::range_reading::RangeReading`, opaque here): the readings of
//! the review windows and of the read ahead of the waiting requests of Your Turn (`app_api::reads::review_plan`), so a
//! start of the app shows them without reading a diff. Two commits never change, so an entry is never stale; the version
//! of the readings is part of each key (the model of the diffs and the rules of the reading), and the first write of a
//! process with another version clears the store, as for `blob_summary`. A few hundred bytes each, in the main
//! environment.

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The key of a range: the version, big-endian, the policy, then the two ids with a space between
fn key(version: u32, policy: u8, base: &str, head: &str) -> Vec<u8> {
  let mut key = Vec::with_capacity(6 + base.len() + head.len());
  key.extend_from_slice(&version.to_be_bytes());
  key.push(policy);
  key.extend_from_slice(base.as_bytes());
  key.push(b' ');
  key.extend_from_slice(head.as_bytes());
  key
}

impl ObjectCache {
  /// The readings of `ranges` (`(base, head)`) under `policy` by reading `version`, in their order
  pub fn get_range_readings<V: DeserializeOwned>(&self, version: u32, policy: u8, ranges: &[(&str, &str)]) -> Vec<Option<V>> {
    let keys: Vec<Vec<u8>> = ranges.iter().map(|(base, head)| key(version, policy, base, head)).collect();
    self.get_many(Store::RangeReading, &keys)
  }

  /// Write the readings of `version` under `policy`, in one transaction
  pub fn put_range_readings<V: Serialize>(&self, version: u32, policy: u8, readings: &[((&str, &str), &V)]) {
    if readings.is_empty() {
      return;
    }
    self.keep_only_version(Store::RangeReading, version);
    let entries: Vec<(Vec<u8>, &V)> = readings.iter().map(|((base, head), reading)| (key(version, policy, base, head), *reading)).collect();
    self.put_many(Store::RangeReading, &entries);
  }
}
