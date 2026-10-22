use serde::{Deserialize, Serialize};

/// A commit of the baseline that carries the user's work
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpstreamCommit {
  pub id: String,
  pub author: String,
  pub committer: String,
  /// Commit time, Unix seconds
  pub time: u32,
}
