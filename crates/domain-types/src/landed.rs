use crate::upstream_commit::UpstreamCommit;
use serde::{Deserialize, Serialize};

/// A local commit whose change is in the baseline
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandedCommit {
  /// The local commit, as it was before the update
  pub id: String,
  /// The subject without the `(prefix)`
  pub subject: String,
  /// The baseline commit with the same author, author time and subject (without the prefix); `None` when the baseline
  /// has the change in another commit (squashed upstream)
  pub upstream: Option<UpstreamCommit>,
}

/// The landed commits of one branch
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandedBranch {
  /// The virtual branch the commits are grouped into; `None` for unassigned commits
  pub name: Option<String>,
  /// Oldest first
  pub commits: Vec<LandedCommit>,
  /// The commits of the branch that stay local
  pub remaining: u32,
  /// When Branch Deck found the commits in the baseline, Unix seconds
  pub landed_at: u32,
}
