//! The uncommitted files of the working tree, from one `status --porcelain -z`; read-only. Their diffs come from
//! `file_diffs` (`DiffSource::Uncommitted`).

use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetUncommittedChangesParams {
  pub repository_path: String,
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
}

/// Parse git status porcelain output into file changes
/// This is extracted as a helper to avoid duplication between main code and tests
#[instrument(level = "debug", fields(output_length = status_output.len()))]
pub fn parse_git_status_output(status_output: &str) -> Vec<UncommittedFileChange> {
  // Work on raw bytes to avoid repeated UTF-8 char iteration and reduce overhead
  let bytes = status_output.as_bytes();
  // Rough upper bound for capacity: number of NULs (may over-estimate for renames, which is fine)
  let approx_entries = status_output.matches('\0').count();
  let mut files = Vec::with_capacity(approx_entries);

  let mut entries = bytes.split(|&b| b == 0);
  while let Some(entry) = entries.next() {
    if entry.len() < 3 {
      continue;
    }

    // First two bytes are the staged/unstaged status codes, then a space, then the path
    let staged_status = entry[0] as char;
    let unstaged_status = entry[1] as char;

    // With `-z`, rename and copy entries are `XY new\0old\0`: the original path is a separate field
    if matches!(staged_status, 'R' | 'C') || matches!(unstaged_status, 'R' | 'C') {
      entries.next();
    }

    // Expect a space at index 2 in porcelain format; if not present, fall back conservatively
    let path_start = if entry.len() > 3 && entry[2] == b' ' { 3 } else { 2 };
    if entry.len() <= path_start {
      continue;
    }

    let file_path = String::from_utf8_lossy(&entry[path_start..]).into_owned();

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
      file_path,
      status,
      staged,
      unstaged,
    });
  }

  files
}

/// Get uncommitted changes with only file metadata (no content or diffs)
#[instrument(skip(git_executor), fields(repository_path = %params.repository_path))]
pub fn get_uncommitted_changes(git_executor: &GitCommandExecutor, params: GetUncommittedChangesParams) -> Result<UncommittedChangesResult, String> {
  let repo_path = params.repository_path;

  // Get file status with null termination for robust filename handling
  // Untrimmed, to preserve exact git status formatting (including leading spaces)
  // Note: For large repos, consider using --untracked-files=normal to avoid scanning all untracked files
  let status_output = git_executor
    .cmd(&["status", "--porcelain", "-z"], &repo_path)
    .bytes()
    .map_err(|e| format!("Failed to get repository status: {e}"))?;

  // Parse file changes from status (null-terminated)
  let files = parse_git_status_output(&String::from_utf8_lossy(&status_output));

  // Check if we found any files
  if files.is_empty() {
    return Ok(UncommittedChangesResult {
      has_changes: false,
      files: vec![],
    });
  }

  Ok(UncommittedChangesResult { has_changes: true, files })
}
