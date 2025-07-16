use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, specta::Type)]
pub struct CommitInfo {
  pub hash: String,
  pub message: String,
}
