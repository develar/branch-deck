//! `review_answer`: a hash of what the code-owner pre-check of Patronus decides on -> its answer and when it was
//! fetched. Not keyed by object ids: the reader decides whether the answer still holds (see `cache.rs`).

use crate::cache::{ObjectCache, Store};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewAnswer<V> {
  /// Unix seconds
  pub fetched_at: u32,
  pub answer: V,
}

impl ObjectCache {
  /// The cached answers of `keys`, in their order
  pub fn get_review_answers<V: DeserializeOwned>(&self, keys: &[[u8; 32]]) -> Vec<Option<ReviewAnswer<V>>> {
    let raw: Vec<Vec<u8>> = keys.iter().map(|key| key.to_vec()).collect();
    self.get_many(Store::ReviewAnswer, &raw)
  }

  pub fn put_review_answers<V: Serialize>(&self, answers: &[([u8; 32], ReviewAnswer<V>)]) {
    let entries: Vec<(Vec<u8>, &ReviewAnswer<V>)> = answers.iter().map(|(key, answer)| (key.to_vec(), answer)).collect();
    self.put_many(Store::ReviewAnswer, &entries);
  }
}
