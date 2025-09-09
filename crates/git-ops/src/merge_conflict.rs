use crate::conflict_analysis::{FileInfo, get_files_content_at_commit};
use crate::copy_commit::CopyCommitError;
use crate::model::{ConflictDetail, ConflictMarkerCommitInfo};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, instrument};

// Store conflict file info with object IDs for each stage
#[derive(Debug)]
pub struct ConflictFileInfo {
  pub path: PathBuf,
  pub base_oid: Option<String>,   // stage 1 - common ancestor
  pub ours_oid: Option<String>,   // stage 2 - target branch
  pub theirs_oid: Option<String>, // stage 3 - cherry-picked commit
}

/// Generate diff hunks between two versions of a file
#[allow(clippy::too_many_arguments)]
#[instrument(skip(git_executor, from_content, to_content), fields(file = %file_path))]
pub fn generate_diff_hunks(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  from_commit: &str,
  to_commit: &str,
  file_path: &str,
  from_content: &str,
  to_content: &str,
  file_ext: &str,
) -> Result<crate::conflict_analysis::FileDiff, CopyCommitError> {
  let mut hunks = Vec::new();

  // Use git diff to generate proper hunks if contents are different
  if from_content != to_content {
    let args = vec![
      "-c",
      "merge.conflictStyle=zdiff3",
      "diff",
      "--no-color",
      "--unified=3",
      from_commit,
      to_commit,
      "--",
      file_path,
    ];
    let diff_output = git_executor
      .execute_command(&args, repo_path)
      .map_err(|e| CopyCommitError::Other(anyhow!("git diff failed: {}", e)))?;

    // Parse hunks using helper function
    hunks = parse_diff_hunks(&diff_output, file_path)?;

    // If no hunks were found from git diff but content is different, create a manual diff
    if hunks.is_empty() {
      let from_lines = from_content.lines().collect::<Vec<_>>();
      let to_lines = to_content.lines().collect::<Vec<_>>();

      let from_count = from_lines.len();
      let to_count = to_lines.len();

      // Create complete diff with file headers for git-diff-view compatibility
      let mut complete_diff = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{from_count} +1,{to_count} @@\n");

      // Add removed lines (avoiding format! for performance)
      for line in from_lines {
        complete_diff.push('-');
        complete_diff.push_str(line);
        complete_diff.push('\n');
      }

      // Add added lines (avoiding format! for performance)
      for line in to_lines {
        complete_diff.push('+');
        complete_diff.push_str(line);
        complete_diff.push('\n');
      }

      hunks.push(complete_diff.trim().to_string());
    }
  } else {
    // Contents are identical - create a context-only hunk to show the content
    let lines = from_content.lines().collect::<Vec<_>>();
    if !lines.is_empty() {
      let line_count = lines.len();
      // Create complete diff with file headers for git-diff-view compatibility
      let mut complete_diff = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{line_count} +1,{line_count} @@\n");

      // Add all lines as context (unchanged, avoiding format! for performance)
      for line in lines {
        complete_diff.push(' ');
        complete_diff.push_str(line);
        complete_diff.push('\n');
      }

      hunks.push(complete_diff.trim().to_string());
    }
  }

  Ok(crate::conflict_analysis::FileDiff {
    old_file: crate::conflict_analysis::FileInfo {
      file_name: file_path.to_string(),
      file_lang: file_ext.to_string(),
      content: from_content.to_string(),
    },
    new_file: crate::conflict_analysis::FileInfo {
      file_name: file_path.to_string(),
      file_lang: file_ext.to_string(),
      content: to_content.to_string(),
    },
    hunks,
  })
}

/// Get commit information for multiple commits in a single batch operation
#[instrument(skip(git_executor, commit_ids))]
pub fn get_commit_info_batch(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids: &[&str]) -> Result<HashMap<String, ConflictMarkerCommitInfo>, CopyCommitError> {
  let mut result = HashMap::new();

  if commit_ids.is_empty() {
    return Ok(result);
  }

  // Use git log with --no-walk to get info for specific commits efficiently
  let mut args = vec!["log", "--no-walk", "--format=%H%x00%s%x00%at%x00%ct%x00%an"];
  args.extend(commit_ids);

  let output = git_executor
    .execute_command(&args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get batch commit info: {}", e)))?;

  // Parse the output - each line is a commit with null-separated fields
  for line in output.lines() {
    let parts: Vec<&str> = line.split('\0').collect();
    if parts.len() >= 5 {
      let commit_info = ConflictMarkerCommitInfo {
        hash: parts[0].to_string(),
        message: parts[1].to_string(),
        author_time: parts[2].parse::<u32>().unwrap_or(0),
        committer_time: parts[3].parse::<u32>().unwrap_or(0),
        author: parts[4].to_string(),
      };
      result.insert(parts[0].to_string(), commit_info);
    }
  }

  Ok(result)
}

/// Get merge conflict content with conflict markers for a specific file using the merge tree
#[instrument(skip(git_executor))]
pub fn get_merge_conflict_content_from_tree(git_executor: &GitCommandExecutor, repo_path: &str, merge_tree_oid: &str, file_path: &str) -> Result<String, CopyCommitError> {
  // Use git cat-file to extract the file content from the merged tree
  // This tree contains conflict markers for conflicted files
  let object_path = format!("{merge_tree_oid}:{file_path}");
  let args = vec!["cat-file", "-p", &object_path];

  let output = match git_executor.execute_command(&args, repo_path) {
    Ok(content) => content,
    Err(e) => {
      let error_msg = e.to_string();
      return if error_msg.contains("does not exist") {
        // File doesn't exist in merge tree (likely renamed/moved) - return empty content
        debug!(file_path = %file_path, merge_tree_oid = %merge_tree_oid, "File not found in merge tree, returning empty content");
        Ok(String::new())
      } else {
        Err(CopyCommitError::Other(anyhow::anyhow!("Failed to get file content from merge tree: {}", e)))
      };
    }
  };

  debug!(content_length = output.len(), "retrieved conflict content");
  Ok(output)
}

/// Generate conflict diff hunks by comparing target commit with merge tree
#[instrument(skip(git_executor), fields(file = %file_path))]
fn generate_conflict_diff_hunks(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  target_commit_id: &str,
  merge_tree_oid: &str,
  file_path: &str,
) -> Result<Vec<String>, CopyCommitError> {
  // First check if the file exists in the target commit
  let target_file_ref = format!("{}:{}", target_commit_id, file_path);
  let check_args = vec!["cat-file", "-e", &target_file_ref];
  let file_exists_in_target = git_executor.execute_command(&check_args, repo_path).is_ok();

  if !file_exists_in_target {
    // File doesn't exist in target (new file conflict) - show everything as additions
    let conflict_file_ref = format!("{}:{}", merge_tree_oid, file_path);
    let conflict_content = git_executor
      .execute_command(&["cat-file", "-p", &conflict_file_ref], repo_path)
      .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get conflict content: {}", e)))?;

    if conflict_content.is_empty() {
      return Ok(vec![]);
    }

    let line_count = conflict_content.lines().count();
    let mut diff = String::with_capacity(conflict_content.len() + 100);
    diff.push_str(&format!("--- /dev/null\n+++ b/{}\n@@ -0,0 +1,{} @@\n", file_path, line_count));

    for line in conflict_content.lines() {
      diff.push('+');
      diff.push_str(line);
      diff.push('\n');
    }

    return Ok(vec![diff]);
  }

  // File exists in target, do normal diff
  let conflict_file_ref = format!("{}:{}", merge_tree_oid, file_path);
  let diff_args = vec![
    "-c",
    "merge.conflictStyle=zdiff3",
    "diff",
    "--no-color",
    "--unified=3",
    &target_file_ref,
    &conflict_file_ref,
  ];

  let diff_output = git_executor
    .execute_command(&diff_args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("git diff failed: {}", e)))?;

  if diff_output.is_empty() {
    return Ok(vec![]);
  }

  // Parse hunks more efficiently
  parse_diff_hunks(&diff_output, file_path)
}

/// Parse diff output into separate hunks
fn parse_diff_hunks(diff_output: &str, file_path: &str) -> Result<Vec<String>, CopyCommitError> {
  let mut hunks = Vec::new();
  let mut current_hunk = String::new();
  let mut in_hunk = false;
  let file_header = format!("--- a/{}\n+++ b/{}\n", file_path, file_path);

  for line in diff_output.lines() {
    if line.starts_with("diff --git") || line.starts_with("index ") {
      continue; // Skip git metadata
    } else if line.starts_with("--- ") || line.starts_with("+++ ") {
      continue; // Skip original headers, we'll add our own
    } else if line.starts_with("@@") {
      // Start of new hunk
      if in_hunk && !current_hunk.is_empty() {
        hunks.push(format!("{}{}", file_header, current_hunk.trim()));
        current_hunk.clear();
      }
      current_hunk.push_str(line);
      current_hunk.push('\n');
      in_hunk = true;
    } else if in_hunk {
      current_hunk.push_str(line);
      current_hunk.push('\n');
    }
  }

  // Add the last hunk
  if in_hunk && !current_hunk.is_empty() {
    hunks.push(format!("{}{}", file_header, current_hunk.trim()));
  }

  Ok(hunks)
}

/// Parameters for extract_conflict_details function
pub struct ConflictDetailsParams<'a> {
  pub git_executor: &'a GitCommandExecutor,
  pub repo_path: &'a str,
  pub conflict_files: &'a HashMap<PathBuf, ConflictFileInfo>,
  pub merge_tree_oid: &'a str,
  pub parent_commit_id: &'a str,
  pub target_commit_id: &'a str,
  pub cherry_commit_id: &'a str,
}

/// Extract conflict details with actual merge conflicts and conflict markers
/// Returns a tuple of (conflict_details, commit_info_map)
#[instrument(skip_all, fields(conflict_files = params.conflict_files.len()))]
pub fn extract_conflict_details(params: ConflictDetailsParams) -> Result<(Vec<ConflictDetail>, HashMap<String, ConflictMarkerCommitInfo>), CopyCommitError> {
  let mut conflict_details = Vec::new();

  // Pre-fetch commit information for all the commits involved in conflicts

  // Find merge base between parent and target (shows actual divergence point)
  let merge_base_id = match crate::conflict_analysis::find_merge_base(params.git_executor, params.repo_path, params.parent_commit_id, params.target_commit_id) {
    Ok(base_id) => base_id,
    Err(_) => params.parent_commit_id.to_string(), // Fallback to parent if merge-base fails
  };

  // Fetch commit info for parent, target, cherry, and merge base
  let commits_to_fetch = vec![params.parent_commit_id, params.target_commit_id, params.cherry_commit_id, &merge_base_id];

  // Use batch operation to fetch all commit info at once
  let commit_info_map: HashMap<String, ConflictMarkerCommitInfo> = get_commit_info_batch(params.git_executor, params.repo_path, &commits_to_fetch)?;

  // Collect all files we need to fetch content for
  let all_file_paths: Vec<String> = params.conflict_files.keys().map(|p| p.display().to_string()).collect();

  // Batch fetch all file contents for all commits at once
  let mut all_file_contents: HashMap<String, HashMap<String, String>> = HashMap::new();

  // Fetch base content
  all_file_contents.insert(
    merge_base_id.clone(),
    get_files_content_at_commit(params.git_executor, params.repo_path, &merge_base_id, &all_file_paths).unwrap_or_default(),
  );

  // Fetch target content
  all_file_contents.insert(
    params.target_commit_id.to_string(),
    get_files_content_at_commit(params.git_executor, params.repo_path, params.target_commit_id, &all_file_paths).unwrap_or_default(),
  );

  // Fetch cherry content
  all_file_contents.insert(
    params.cherry_commit_id.to_string(),
    get_files_content_at_commit(params.git_executor, params.repo_path, params.cherry_commit_id, &all_file_paths).unwrap_or_default(),
  );

  for info in params.conflict_files.values() {
    let file_path = info.path.display().to_string();

    // Get the actual merge conflict content with conflict markers from the merge tree

    let conflict_content = get_merge_conflict_content_from_tree(params.git_executor, params.repo_path, params.merge_tree_oid, &file_path)?; // Don't hide errors - if we detected conflict, content must exist

    // Create a FileDiff that shows the merge conflict with proper hunks
    // The conflict_content should always contain conflict markers for files in conflict
    let file_diff = {
      // Get the original file content for comparison
      // For conflict display, we want to show: original content -> conflict content
      // This will show conflict markers as additions, not deletions
      let original_content = String::new(); // Show empty as "before" so conflict markers appear as additions

      let file_ext = file_path.split('.').next_back().unwrap_or("txt").to_string();

      // Use git diff --cc with a temporary merge commit to get proper 3-way conflict diffs
      let hunks = if original_content != conflict_content {
        // Generate diff directly without creating temporary commit objects
        generate_conflict_diff_hunks(params.git_executor, params.repo_path, params.target_commit_id, params.merge_tree_oid, &file_path)?
      } else {
        // If same content, show as context
        let lines = conflict_content.lines().collect::<Vec<_>>();
        if !lines.is_empty() {
          let line_count = lines.len();
          let complete_diff = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{line_count} +1,{line_count} @@\n");

          let mut diff_content = complete_diff;
          for line in lines {
            diff_content.push_str(&format!(" {line}\n"));
          }

          vec![diff_content.trim().to_string()]
        } else {
          vec![]
        }
      };

      crate::conflict_analysis::FileDiff {
        old_file: FileInfo {
          file_name: file_path.clone(),
          file_lang: file_ext.clone(),
          content: original_content,
        },
        new_file: FileInfo {
          file_name: file_path.clone(),
          file_lang: file_ext,
          content: conflict_content,
        },
        hunks,
      }
    };

    // Generate individual file info for 3-way merge view
    // The merge_base_id was already calculated above, reuse it

    // Get file content for each version from pre-fetched data
    let base_content = all_file_contents
      .get(&merge_base_id)
      .and_then(|contents| contents.get(&file_path))
      .cloned()
      .unwrap_or_default();

    let target_content = all_file_contents
      .get(params.target_commit_id)
      .and_then(|contents| contents.get(&file_path))
      .cloned()
      .unwrap_or_default();

    let cherry_content = all_file_contents
      .get(params.cherry_commit_id)
      .and_then(|contents| contents.get(&file_path))
      .cloned()
      .unwrap_or_default();

    // Create FileInfo structs for each version
    let file_ext = file_path.split('.').next_back().unwrap_or("txt").to_string();

    let base_file = Some(FileInfo {
      file_name: file_path.clone(),
      file_lang: file_ext.clone(),
      content: base_content.clone(),
    });

    let target_file = Some(FileInfo {
      file_name: file_path.clone(),
      file_lang: file_ext.clone(),
      content: target_content.clone(),
    });

    let cherry_file = Some(FileInfo {
      file_name: file_path.clone(),
      file_lang: file_ext.clone(),
      content: cherry_content.clone(),
    });

    // Generate diff hunks for 3-way merge view
    let base_to_target_diff = generate_diff_hunks(
      params.git_executor,
      params.repo_path,
      &merge_base_id,
      params.target_commit_id,
      &file_path,
      &base_content,
      &target_content,
      &file_ext,
    )?;
    let base_to_cherry_diff = generate_diff_hunks(
      params.git_executor,
      params.repo_path,
      &merge_base_id,
      params.cherry_commit_id,
      &file_path,
      &base_content,
      &cherry_content,
      &file_ext,
    )?;

    conflict_details.push(ConflictDetail {
      file: file_path,
      status: "modified".to_string(),
      file_diff,
      base_file,
      target_file,
      cherry_file,
      base_to_target_diff,
      base_to_cherry_diff,
    });
  }

  Ok((conflict_details, commit_info_map))
}
