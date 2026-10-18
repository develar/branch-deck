//! `blob_summary`: (extractor version, language, blob id) -> the syntax summary of a source blob
//! (`code_symbols::summary::FileSummary`, opaque here, so that this crate needs no parser). The version of the extractor
//! is part of each key, so a summary of another extractor is never read; the first write of a process with another
//! version clears the store, so that the old summaries don't wait for a full map

use crate::cache::{ObjectCache, Store};
use serde::Serialize;
use serde::de::DeserializeOwned;

fn key(version: u32, language: u8, blob: &str) -> Vec<u8> {
  let mut key = Vec::with_capacity(5 + blob.len());
  key.extend_from_slice(&version.to_be_bytes());
  key.push(language);
  key.extend_from_slice(blob.as_bytes());
  key
}

impl ObjectCache {
  /// The cached summaries of `blobs` ((language, blob id)) by extractor `version`, in their order
  pub fn get_blob_summaries<V: DeserializeOwned>(&self, version: u32, blobs: &[(u8, &str)]) -> Vec<Option<V>> {
    let keys: Vec<Vec<u8>> = blobs.iter().map(|&(language, blob)| key(version, language, blob)).collect();
    self.get_many(Store::BlobSummary, &keys)
  }

  /// Write the summaries of extractor `version`
  pub fn put_blob_summaries<V: Serialize>(&self, version: u32, summaries: &[((u8, &str), &V)]) {
    if summaries.is_empty() {
      return;
    }
    self.keep_only_version(Store::BlobSummary, version);
    let entries: Vec<(Vec<u8>, &V)> = summaries.iter().map(|&((language, blob), summary)| (key(version, language, blob), summary)).collect();
    self.put_many(Store::BlobSummary, &entries);
  }
}
