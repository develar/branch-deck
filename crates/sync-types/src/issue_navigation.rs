//! Issue navigation types for IntelliJ IDEA configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IssueNavigationLink {
  pub issue_regexp: String,
  pub link_regexp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct IssueNavigationConfig {
  pub links: Vec<IssueNavigationLink>,
}
