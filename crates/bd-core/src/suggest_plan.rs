//! The plan `bd suggest` prints and `bd check --plan` and `bd pick` read, without the clustering behind it

use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;

/// The plan of `bd group` (`RegroupPlan`)
#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct SuggestPlan {
  pub branches: BTreeMap<String, Vec<String>>,
  pub unassign: Vec<String>,
  /// Issue group name -> the issue keys it gets: whole issue branches, grouped without a rewrite
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub issue_groups: BTreeMap<String, Vec<String>>,
  /// The keys whose issue group goes: their commits go back to the issue branch of the key
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub ungroup_issues: Vec<String>,
  /// The branches whose stack parent goes: they become roots
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub unstack: Vec<String>,
}
