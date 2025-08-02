use serde::{Deserialize, Serialize};

/// Branch name suggestion
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct BranchSuggestion {
  pub name: String,
  pub reason: Option<String>,
}

/// Progress events for branch name suggestion generation
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

/// Parameters for requesting branch name suggestions
#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestBranchNameParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub commits: Vec<git_ops::model::CommitInfo>,
}

/// Progress events for model download operations
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", content = "data")]
pub enum DownloadProgress {
  Started {
    #[serde(rename = "totalFiles")]
    total_files: u32,
  },
  FileStarted {
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "fileSize")]
    file_size: Option<u32>,
  },
  Progress {
    #[serde(rename = "fileName")]
    file_name: String,
    downloaded: u32,
    total: u32,
    #[serde(rename = "bytesPerSecond")]
    bytes_per_second: Option<u32>,
    #[serde(rename = "secondsRemaining")]
    seconds_remaining: Option<u32>,
  },
  FileCompleted {
    #[serde(rename = "fileName")]
    file_name: String,
  },
  Completed,
  Cancelled,
  Error {
    message: String,
  },
}
