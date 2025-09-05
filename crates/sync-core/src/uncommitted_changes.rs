use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::conflict_analysis::{FileDiff, FileInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetUncommittedChangesParams {
  pub repository_path: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetFileContentForDiffParams {
  pub repository_path: String,
  pub file_path: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UncommittedFileChange {
  pub file_path: String,
  pub status: String, // "added", "modified", "deleted", "renamed", "copied"
  pub staged: bool,
  pub unstaged: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UncommittedChangesResult {
  pub has_changes: bool,
  pub files: Vec<UncommittedFileChange>,
  pub file_diffs: Vec<FileDiff>,
}

/// Get uncommitted changes with only file metadata (no content or diffs)
pub fn get_uncommitted_changes(git_executor: &GitCommandExecutor, params: GetUncommittedChangesParams) -> Result<UncommittedChangesResult, String> {
  let git = git_executor.clone();
  let repo_path = params.repository_path;

  // Get file status with null termination for robust filename handling
  // Use execute_command_raw to preserve exact git status formatting (including leading spaces)
  let status_output = git
    .execute_command_raw(&["status", "--porcelain", "-z"], &repo_path)
    .map_err(|e| format!("Failed to get repository status: {}", e))?;

  // Parse file changes from status (null-terminated)
  let mut files = Vec::new();

  for line in status_output.split('\0') {
    if line.len() >= 3 {
      let staged_status = line.chars().next().unwrap_or(' ');
      let unstaged_status = line.chars().nth(1).unwrap_or(' ');
      let file_path = &line[3..];

      let staged = staged_status != ' ' && staged_status != '?';
      let unstaged = unstaged_status != ' ';

      let status = match (staged_status, unstaged_status) {
        ('A', _) | ('?', '?') => "added",
        ('M', _) | (_, 'M') => "modified",
        ('D', _) | (_, 'D') => "deleted",
        ('R', _) => "renamed",
        ('C', _) => "copied",
        _ => "modified",
      }
      .to_string();

      files.push(UncommittedFileChange {
        file_path: file_path.to_string(),
        status,
        staged,
        unstaged,
      });
    }
  }

  // Check if we found any files
  if files.is_empty() {
    return Ok(UncommittedChangesResult {
      has_changes: false,
      files: vec![],
      file_diffs: vec![], // Empty - content loaded on demand
    });
  }

  Ok(UncommittedChangesResult {
    has_changes: true,
    files,
    file_diffs: vec![], // Empty - will be populated on demand
  })
}

// Helper functions removed - no longer needed with lazy loading approach

/// Get file content for diff display when user expands a file in the UI
pub fn get_file_content_for_diff(git_executor: &GitCommandExecutor, params: GetFileContentForDiffParams) -> Result<FileDiff, String> {
  let git = git_executor.clone();
  let repo_path = params.repository_path;
  let file_path = params.file_path;

  // Determine file language from extension
  let file_lang = std::path::Path::new(&file_path).extension().and_then(|ext| ext.to_str()).unwrap_or("txt").to_string();

  // Get old content from HEAD (for modified files)
  let old_content = git.execute_command(&["show", &format!("HEAD:{}", file_path)], &repo_path).unwrap_or_default();

  // Get current working tree content
  let working_tree_path = std::path::Path::new(&repo_path).join(&file_path);
  let new_content = if working_tree_path.exists() {
    std::fs::read_to_string(working_tree_path).unwrap_or_default()
  } else {
    String::new()
  };

  // Create FileDiff with full content - git-diff-view will compute the diff
  Ok(FileDiff {
    old_file: FileInfo {
      file_name: file_path.clone(),
      file_lang: file_lang.clone(),
      content: old_content,
    },
    new_file: FileInfo {
      file_name: file_path,
      file_lang,
      content: new_content,
    },
    hunks: vec![], // git-diff-view will compute hunks from the content
  })
}
