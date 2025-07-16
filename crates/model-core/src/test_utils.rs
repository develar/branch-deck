use serde::{Deserialize, Serialize};

/// Test commit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
  pub hash: String,
  pub message: String,
}

/// Test commit diff information
#[derive(Debug, Clone)]
pub struct CommitDiff {
  pub commit_hash: String,
  pub files: Vec<FileDiff>,
}

/// Test file diff information
#[derive(Debug, Clone)]
pub struct FileDiff {
  pub path: String,
  pub additions: i32,
  pub deletions: i32,
  #[allow(dead_code)]
  pub patch: Option<String>,
}

/// Convert test data to raw git log format for prompt generation testing
pub fn convert_to_raw_git_format(commits: &[CommitInfo], diffs: &[CommitDiff]) -> String {
  let mut git_output = String::new();

  for commit in commits {
    // Add commit message
    git_output.push_str(&commit.message);
    git_output.push('\n');

    // Add file changes for this commit
    if let Some(diff) = diffs.iter().find(|d| d.commit_hash == commit.hash) {
      for file in &diff.files {
        let status = if file.deletions == 0 {
          "A" // Added
        } else if file.additions == 0 {
          "D" // Deleted
        } else {
          "M" // Modified
        };
        git_output.push_str(&format!("{}\t{}\n", status, file.path));
      }
    }
    git_output.push('\n'); // Separate commits
  }

  git_output.trim().to_string()
}
