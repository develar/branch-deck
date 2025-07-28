use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BranchSuggestion {
  pub name: String,
  pub confidence: f32,
  pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", content = "data")]
pub enum SuggestionProgress {
  Started { total: u32 },
  SuggestionReady { suggestion: BranchSuggestion, index: u32 },
  Completed,
  Cancelled,
  Error { message: String },
  ModelDownloadInProgress { model_name: String, model_size: String },
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestBranchNameParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub commits: Vec<git_ops::model::CommitInfo>,
}
