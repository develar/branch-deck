use serde::{Deserialize, Serialize};

/// Confidence level for integration detection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum IntegrationConfidence {
  Exact, // Git confirms via branch --merged (100% confident)
  High,  // Cherry-pick detection found (90% confident - likely rebase)
}

impl PartialOrd for IntegrationConfidence {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for IntegrationConfidence {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    use IntegrationConfidence::*;
    match (self, other) {
      (Exact, Exact) => std::cmp::Ordering::Equal,
      (Exact, High) => std::cmp::Ordering::Greater,
      (High, Exact) => std::cmp::Ordering::Less,
      (High, High) => std::cmp::Ordering::Equal,
    }
  }
}

impl Eq for IntegrationConfidence {}

/// Unified branch integration status
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BranchIntegrationStatus {
  #[serde(rename_all = "camelCase")]
  Integrated {
    integrated_at: Option<u32>,
    confidence: IntegrationConfidence,
    commit_count: u32,
  },
  #[serde(rename_all = "camelCase")]
  NotIntegrated {
    total_commit_count: u32,
    integrated_count: u32,
    orphaned_count: u32,
    integrated_at: Option<u32>,
  },
  #[serde(rename_all = "camelCase")]
  Partial { missing: u32 },
}

/// Unified branch integration info
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchIntegrationInfo {
  pub name: String,
  pub summary: String,
  pub status: BranchIntegrationStatus,
}
