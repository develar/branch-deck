use crate::git::conflict_analysis::{FileInfo, get_files_content_at_commit};
use crate::git::copy_commit::CopyCommitError;
use crate::git::git_command::GitCommandExecutor;
use crate::git::model::{BranchError, ConflictDetail, ConflictMarkerCommitInfo, MergeConflictInfo};
use crate::progress::SyncEvent;
use anyhow::{Result, anyhow};
use git2::{Commit, Oid, Repository, Tree};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::ipc::Channel;
use tracing::{debug, instrument};

// Store conflict file info with object IDs for each stage
#[derive(Debug)]
struct ConflictFileInfo {
  path: PathBuf,
  base_oid: Option<String>,   // stage 1 - common ancestor
  ours_oid: Option<String>,   // stage 2 - target branch
  theirs_oid: Option<String>, // stage 3 - cherry-picked commit
}

/// Generate diff hunks between two versions of a file
#[allow(clippy::too_many_arguments)]
#[instrument(skip(git_executor, from_content, to_content), fields(file = %file_path))]
fn generate_diff_hunks(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  from_commit: &str,
  to_commit: &str,
  file_path: &str,
  from_content: &str,
  to_content: &str,
  file_ext: &str,
) -> Result<crate::git::conflict_analysis::FileDiff, CopyCommitError> {
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
    let diff_output = git_executor.execute_command(&args, repo_path).unwrap_or_default();

    // Check if git diff produced a complete diff - if so, parse it properly to separate hunks
    if diff_output.contains("---") && diff_output.contains("+++") {
      // Parse the complete git diff output to extract separate hunks with file headers
      let mut file_headers = String::new();
      let mut current_hunk = String::new();
      let mut in_hunk = false;
      let mut found_headers = false;

      for line in diff_output.lines() {
        if line.starts_with("diff --git") || line.starts_with("index ") {
          // Skip git diff metadata lines
          continue;
        } else if line.starts_with("---") {
          // Start of file headers
          file_headers.clear();
          file_headers.push_str(line);
          file_headers.push('\n');
          found_headers = false;
        } else if line.starts_with("+++") {
          // End of file headers
          file_headers.push_str(line);
          file_headers.push('\n');
          found_headers = true;
        } else if line.starts_with("@@") && found_headers {
          // Start of a new hunk - save previous hunk if exists
          if in_hunk && !current_hunk.is_empty() {
            hunks.push((file_headers.clone() + &current_hunk).trim().to_string());
          }
          current_hunk = line.to_string();
          current_hunk.push('\n');
          in_hunk = true;
        } else if in_hunk {
          // Part of current hunk - include all lines (context, additions, deletions, etc.)
          current_hunk.push_str(line);
          current_hunk.push('\n');
        }
      }

      // Add the last hunk if it exists
      if in_hunk && !current_hunk.is_empty() {
        hunks.push((file_headers + &current_hunk).trim().to_string());
      }
    } else {
      // Parse the diff output to extract hunks (fallback for incomplete output)
      // Add file headers to each hunk manually
      let file_headers = format!("--- a/{file_path}\n+++ b/{file_path}\n");
      let mut current_hunk = String::new();
      let mut in_hunk = false;

      for line in diff_output.lines() {
        if line.starts_with("@@") {
          // Start of a new hunk
          if !current_hunk.is_empty() {
            hunks.push((file_headers.clone() + &current_hunk).trim().to_string());
          }
          current_hunk = line.to_string();
          current_hunk.push('\n');
          in_hunk = true;
        } else if in_hunk && (line.starts_with(' ') || line.starts_with('+') || line.starts_with('-')) {
          // Part of the current hunk
          current_hunk.push_str(line);
          current_hunk.push('\n');
        } else if in_hunk && line.starts_with("\\") {
          // "\ No newline at end of file" - include it in the hunk
          current_hunk.push_str(line);
          current_hunk.push('\n');
        } else if in_hunk && line.is_empty() {
          // Empty line in hunk
          current_hunk.push(' ');
          current_hunk.push('\n');
        } else if in_hunk {
          // End of hunk
          if !current_hunk.is_empty() {
            hunks.push((file_headers.clone() + &current_hunk).trim().to_string());
            current_hunk.clear();
          }
          in_hunk = false;
        }
      }

      // Add the last hunk if it exists
      if !current_hunk.is_empty() {
        hunks.push((file_headers + &current_hunk).trim().to_string());
      }
    }

    // If no hunks were found from git diff but content is different, create a manual diff
    if hunks.is_empty() {
      let from_lines = from_content.lines().collect::<Vec<_>>();
      let to_lines = to_content.lines().collect::<Vec<_>>();

      let from_count = from_lines.len();
      let to_count = to_lines.len();

      // Create complete diff with file headers for git-diff-view compatibility
      let mut complete_diff = format!("--- a/{file_path}\n+++ b/{file_path}\n@@ -1,{from_count} +1,{to_count} @@\n");

      // Add removed lines
      for line in from_lines {
        complete_diff.push_str(&format!("-{line}\n"));
      }

      // Add added lines
      for line in to_lines {
        complete_diff.push_str(&format!("+{line}\n"));
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

      // Add all lines as context (unchanged)
      for line in lines {
        complete_diff.push_str(&format!(" {line}\n"));
      }

      hunks.push(complete_diff.trim().to_string());
    }
  }

  Ok(crate::git::conflict_analysis::FileDiff {
    old_file: crate::git::conflict_analysis::FileInfo {
      file_name: file_path.to_string(),
      file_lang: file_ext.to_string(),
      content: from_content.to_string(),
    },
    new_file: crate::git::conflict_analysis::FileInfo {
      file_name: file_path.to_string(),
      file_lang: file_ext.to_string(),
      content: to_content.to_string(),
    },
    hunks,
  })
}

/// Get commit information for multiple commits in a single batch operation
#[instrument(skip(git_executor, commit_ids))]
fn get_commit_info_batch(git_executor: &GitCommandExecutor, repo_path: &str, commit_ids: &[&str]) -> Result<HashMap<String, ConflictMarkerCommitInfo>, CopyCommitError> {
  let mut result = HashMap::new();

  if commit_ids.is_empty() {
    return Ok(result);
  }

  // Use git log with --no-walk to get info for specific commits efficiently
  let mut args = vec!["log", "--no-walk", "--format=%H%x00%s%x00%ct%x00%an%x00"];
  args.extend(commit_ids);

  let output = git_executor
    .execute_command(&args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to get batch commit info: {}", e)))?;

  // Parse the output - each commit's info is terminated by null character
  for commit_info in output.split('\0').filter(|s| !s.is_empty()) {
    let parts: Vec<&str> = commit_info.splitn(4, '\0').collect();
    if parts.len() >= 4 {
      let commit_info = ConflictMarkerCommitInfo {
        hash: parts[0].to_string(),
        message: parts[1].to_string(),
        timestamp: parts[2].parse::<u32>().unwrap_or(0),
        author: parts[3].to_string(),
      };
      result.insert(parts[0].to_string(), commit_info);
    }
  }

  Ok(result)
}

/// Get merge conflict content with conflict markers for a specific file using the merge tree
#[instrument(skip(git_executor))]
fn get_merge_conflict_content_from_tree(git_executor: &GitCommandExecutor, repo_path: &str, merge_tree_oid: &str, file_path: &str) -> Result<String, CopyCommitError> {
  // Use git cat-file to extract the file content from the merged tree
  // This tree contains conflict markers for conflicted files
  let object_path = format!("{merge_tree_oid}:{file_path}");
  let args = vec!["cat-file", "-p", &object_path];

  let output = git_executor
    .execute_command(&args, repo_path)
    .map_err(|e| CopyCommitError::Other(anyhow::anyhow!("Failed to get file content from merge tree: {}", e)))?;

  debug!(content_length = output.len(), "retrieved conflict content");
  Ok(output)
}

/// Parameters for extract_conflict_details function
struct ConflictDetailsParams<'a> {
  git_executor: &'a GitCommandExecutor,
  repo_path: &'a str,
  conflict_files: &'a HashMap<PathBuf, ConflictFileInfo>,
  merge_tree_oid: &'a str,
  parent_commit_id: &'a str,
  target_commit_id: &'a str,
  cherry_commit_id: &'a str,
}

/// Extract conflict details with actual merge conflicts and conflict markers
/// Returns a tuple of (conflict_details, commit_info_map)
#[instrument(skip_all, fields(conflict_files = params.conflict_files.len()))]
fn extract_conflict_details(params: ConflictDetailsParams) -> Result<(Vec<ConflictDetail>, HashMap<String, ConflictMarkerCommitInfo>), CopyCommitError> {
  let mut conflict_details = Vec::new();

  // Pre-fetch commit information for all the commits involved in conflicts

  // Find merge base between parent and target (shows actual divergence point)
  let merge_base_id = match crate::git::conflict_analysis::find_merge_base(params.git_executor, params.repo_path, params.parent_commit_id, params.target_commit_id) {
    Ok(base_id) => base_id,
    Err(_) => params.parent_commit_id.to_string(), // Fallback to parent if merge-base fails
  };

  // Fetch commit info for parent, target, cherry, and merge base
  let commits_to_fetch = vec![params.parent_commit_id, params.target_commit_id, params.cherry_commit_id, &merge_base_id];

  // Use batch operation to fetch all commit info at once
  let commit_info_map: HashMap<String, ConflictMarkerCommitInfo> = get_commit_info_batch(params.git_executor, params.repo_path, &commits_to_fetch)?;

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
        // Create a temporary merge commit using the merge tree and git commit-tree
        let merge_message = "Temporary merge commit for conflict analysis".to_string();
        let commit_tree_args = vec![
          "commit-tree",
          params.merge_tree_oid,
          "-p",
          params.target_commit_id,
          "-p",
          params.cherry_commit_id,
          "-m",
          &merge_message,
        ];

        if let Ok(merge_commit_output) = params.git_executor.execute_command(&commit_tree_args, params.repo_path) {
          let merge_commit_id = merge_commit_output.trim();
          debug!(merge_commit_id, "created temporary merge commit");

          // Use git diff to show the conflict properly
          // We want to show: target state -> conflicted state
          // This will show conflict markers in context

          // First check if the file exists in the target commit
          let check_ref = format!("{}:{}", params.target_commit_id, file_path);
          let check_args = vec!["cat-file", "-e", &check_ref];
          let file_exists_in_target = params.git_executor.execute_command(&check_args, params.repo_path).is_ok();

          let diff_output = if file_exists_in_target {
            // File exists in target, do normal diff
            let target_file_ref = format!("{}:{}", params.target_commit_id, file_path);
            let conflict_file_ref = format!("{}:{}", params.merge_tree_oid, file_path);

            let diff_args = vec![
              "-c",
              "merge.conflictStyle=zdiff3",
              "diff",
              "--no-color",
              "--unified=3",
              &target_file_ref,
              &conflict_file_ref,
            ];

            params
              .git_executor
              .execute_command(&diff_args, params.repo_path)
              .map_err(|e| CopyCommitError::Other(anyhow!("git diff failed: {}", e)))?
          } else {
            // File doesn't exist in target (delete/modify conflict)
            // Show everything as additions
            String::new()
          };

          debug!(diff_output_length = diff_output.len(), "generated git diff");

          // Parse the unified diff output to extract hunks
          let mut hunks = Vec::new();
          let mut current_hunk = String::new();
          let mut in_hunk = false;
          let mut has_headers = false;

          for line in diff_output.lines() {
            if line.starts_with("diff --git") {
              // Skip git metadata
              continue;
            } else if line.starts_with("index ") {
              // Skip index line
              continue;
            } else if line.starts_with("--- ") {
              // Start of file headers
              if in_hunk && !current_hunk.is_empty() {
                hunks.push(current_hunk.trim().to_string());
                current_hunk.clear();
              }
              current_hunk.push_str(&format!("--- a/{file_path}\n"));
              has_headers = true;
              in_hunk = false;
            } else if line.starts_with("+++ ") {
              // Complete file headers
              current_hunk.push_str(&format!("+++ b/{file_path}\n"));
            } else if line.starts_with("@@") && has_headers {
              // Hunk header
              if in_hunk && !current_hunk.is_empty() {
                // Save previous hunk
                hunks.push(current_hunk.trim().to_string());
                current_hunk = format!("--- a/{file_path}\n+++ b/{file_path}\n");
              }
              current_hunk.push_str(line);
              current_hunk.push('\n');
              in_hunk = true;
            } else if in_hunk {
              // Hunk content
              current_hunk.push_str(line);
              current_hunk.push('\n');
            }
          }

          // Add the last hunk
          if in_hunk && !current_hunk.is_empty() {
            hunks.push(current_hunk.trim().to_string());
          }

          // If git diff didn't produce output (e.g., for new files), create a simple diff
          if hunks.is_empty() {
            let line_count = conflict_content.lines().count();
            let mut diff = String::new();
            diff.push_str(&format!("--- a/{file_path}\n"));
            diff.push_str(&format!("+++ b/{file_path}\n"));
            diff.push_str(&format!("@@ -0,0 +1,{line_count} @@\n"));

            for line in conflict_content.lines() {
              diff.push_str(&format!("+{line}\n"));
            }

            hunks.push(diff.trim().to_string());
          }

          // Clean up the temporary merge commit
          let _ = params
            .git_executor
            .execute_command(&["update-ref", "-d", "refs/temp-merge", merge_commit_id], params.repo_path);

          hunks
        } else {
          return Err(CopyCommitError::Other(anyhow!("git commit-tree failed to create temporary merge commit")));
        }
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

      crate::git::conflict_analysis::FileDiff {
        old_file: crate::git::conflict_analysis::FileInfo {
          file_name: file_path.clone(),
          file_lang: file_ext.clone(),
          content: original_content,
        },
        new_file: crate::git::conflict_analysis::FileInfo {
          file_name: file_path.clone(),
          file_lang: file_ext,
          content: conflict_content,
        },
        hunks,
      }
    };

    // Generate individual file info for 3-way merge view
    // Find the merge base between parent of cherry-pick and target branch
    // This shows the actual divergence point that causes the conflict
    let merge_base_id = match crate::git::conflict_analysis::find_merge_base(params.git_executor, params.repo_path, params.parent_commit_id, params.target_commit_id) {
      Ok(base_id) => base_id,
      Err(_) => params.parent_commit_id.to_string(), // Fallback to parent if merge-base fails
    };

    // Get file content for each version
    let base_content = get_files_content_at_commit(params.git_executor, params.repo_path, &merge_base_id, &[file_path.clone()])
      .ok()
      .and_then(|mut contents| contents.remove(&file_path))
      .unwrap_or_default();

    let target_content = get_files_content_at_commit(params.git_executor, params.repo_path, params.target_commit_id, &[file_path.clone()])
      .ok()
      .and_then(|mut contents| contents.remove(&file_path))
      .unwrap_or_default();

    let cherry_content = get_files_content_at_commit(params.git_executor, params.repo_path, params.cherry_commit_id, &[file_path.clone()])
      .ok()
      .and_then(|mut contents| contents.remove(&file_path))
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

/// Cherry-pick implementation using Git plumbing commands (git merge-tree)
/// This performs the cherry-pick without touching the working directory
/// This version is for production use with GitCommandExecutor
#[instrument(skip_all)]
pub fn perform_fast_cherry_pick_with_context<'a>(
  repo: &'a Repository,
  cherry_commit: &'a Commit,
  target_commit: &'a Commit,
  git_executor: &GitCommandExecutor,
  progress: Option<(&Channel<SyncEvent>, &str, i16)>, // (channel, branch_name, task_index)
) -> Result<Tree<'a>, CopyCommitError> {
  // Fast path: if parent tree matches target tree, reuse commit tree
  if cherry_commit.parent_count() > 0 {
    let parent = cherry_commit.parent(0)?;
    if parent.tree_id() == target_commit.tree_id() {
      debug!("parent tree matches target tree, reusing commit tree");
      return Ok(cherry_commit.tree()?);
    }
  }

  // Get repository path
  let repo_path = repo.path().parent().ok_or_else(|| CopyCommitError::Other(anyhow!("Could not get repository path")))?;
  let repo_path_str = repo_path.to_str().ok_or_else(|| CopyCommitError::Other(anyhow!("Repository path is not valid UTF-8")))?;

  // Get the commit's tree and parent
  let commit_tree_id = cherry_commit.tree_id();
  let parent = cherry_commit
    .parent(0)
    .map_err(|e| CopyCommitError::Other(anyhow!("Cannot cherry-pick a root commit: {}", e)))?;
  let parent_tree_id = parent.tree_id();
  let target_tree_id = target_commit.tree_id();

  debug!(
    base_parent = %parent_tree_id,
    ours_target = %target_tree_id, 
    theirs_commit = %commit_tree_id,
    "running git merge-tree"
  );

  // Use GitCommandExecutor to run git merge-tree
  let parent_id = parent.id().to_string();
  let target_id = target_commit.id().to_string();
  let cherry_id = cherry_commit.id().to_string();

  let args = vec![
    "-c",
    "merge.conflictStyle=zdiff3", // Set conflict style to include base content
    "merge-tree",
    "--write-tree",
    "-z", // Use NUL character as separator for better parsing
    "--merge-base",
    &parent_id,
    &target_id,
    &cherry_id,
  ];

  let output = git_executor
    .execute_command(&args, repo_path_str)
    .map_err(|e| CopyCommitError::Other(anyhow!("Failed to execute git merge-tree: {}", e)))?;

  debug!(output_length = output.len(), "git merge-tree completed");

  // Check if command produced output
  if output.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("git merge-tree did not produce output")));
  }

  // Parse the NUL-separated output
  let parts: Vec<&str> = output.trim_end_matches('\0').split('\0').collect();

  if parts.is_empty() || parts[0].is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No output from git merge-tree")));
  }

  let tree_oid = parts[0]
    .parse::<Oid>()
    .map_err(|e| CopyCommitError::Other(anyhow!("Invalid tree OID '{}' from merge-tree: {}", parts[0], e)))?;

  // Check if there were conflicts by looking for file entries
  if parts.len() > 1 {
    let mut conflict_files: HashMap<PathBuf, ConflictFileInfo> = HashMap::new();

    // Parse file entries (mode object stage\tfilename)
    for part in parts.iter().skip(1).take_while(|p| !p.is_empty()) {
      // File entries have format: "<mode> <object> <stage>\t<filename>"
      if let Some(tab_pos) = part.find('\t') {
        let (prefix, filename) = part.split_at(tab_pos);
        let filename = &filename[1..]; // Skip the tab
        let path = PathBuf::from(filename);

        // Parse mode, object, stage
        let prefix_parts: Vec<&str> = prefix.split_whitespace().collect();
        if prefix_parts.len() == 3 {
          let object_id = prefix_parts[1].to_string();
          let stage = prefix_parts[2];

          let entry = conflict_files.entry(path.clone()).or_insert(ConflictFileInfo {
            path,
            base_oid: None,
            ours_oid: None,
            theirs_oid: None,
          });

          match stage {
            "1" => entry.base_oid = Some(object_id),
            "2" => entry.ours_oid = Some(object_id),
            "3" => entry.theirs_oid = Some(object_id),
            _ => {}
          }
        }
      }
    }

    // Skip empty separator and subsequent sections
    // We only care about the actual conflicting file paths

    if !conflict_files.is_empty() {
      // Send branch status event for conflict analysis if progress channel is available
      if let Some((progress_channel, branch_name, _task_index)) = &progress {
        let _ = progress_channel.send(SyncEvent::BranchStatusUpdate {
          branch_name: branch_name.to_string(),
          status: crate::git::model::BranchSyncStatus::AnalyzingConflict,
        });
      }

      // Get detailed conflict information with diffs
      let original_parent = cherry_commit.parent(0)?;
      let (detailed_conflicts, conflict_marker_commits) = extract_conflict_details(ConflictDetailsParams {
        git_executor,
        repo_path: repo_path_str,
        conflict_files: &conflict_files,
        merge_tree_oid: parts[0], // merge_tree_oid
        parent_commit_id: &original_parent.id().to_string(),
        target_commit_id: &target_commit.id().to_string(),
        cherry_commit_id: &cherry_commit.id().to_string(),
      })?;

      // Analyze the conflict to find missing commits
      let conflicting_paths: Vec<PathBuf> = conflict_files.keys().cloned().collect();
      let conflict_analysis = match crate::git::conflict_analysis::analyze_conflict(
        git_executor,
        repo_path_str,
        &original_parent.id().to_string(),
        &target_commit.id().to_string(),
        &conflicting_paths,
      ) {
        Ok(analysis) => analysis,
        Err(e) => {
          // If conflict analysis fails, create a default analysis with empty data
          debug!(error = %e, "failed to analyze conflict");
          crate::git::conflict_analysis::ConflictAnalysis {
            missing_commits: vec![],
            merge_base_hash: String::new(),
            merge_base_message: String::new(),
            merge_base_time: 0,
            merge_base_author: String::new(),
            divergence_summary: crate::git::conflict_analysis::DivergenceSummary {
              commits_ahead_in_source: 0,
              commits_ahead_in_target: 0,
              common_ancestor_distance: 0,
            },
          }
        }
      };

      return Err(CopyCommitError::BranchError(BranchError::MergeConflict(Box::new(MergeConflictInfo {
        commit_message: cherry_commit.summary().unwrap_or_default().to_string(),
        commit_hash: cherry_commit.id().to_string(),
        commit_time: cherry_commit.time().seconds() as u32,
        original_parent_message: original_parent.summary().unwrap_or_default().to_string(),
        original_parent_hash: original_parent.id().to_string(),
        original_parent_time: original_parent.time().seconds() as u32,
        target_branch_message: target_commit.summary().unwrap_or_default().to_string(),
        target_branch_hash: target_commit.id().to_string(),
        target_branch_time: target_commit.time().seconds() as u32,
        conflicting_files: detailed_conflicts,
        conflict_analysis,
        conflict_marker_commits,
      }))));
    }
  }

  // No conflicts, return the merged tree
  let tree = repo.find_tree(tree_oid)?;
  Ok(tree)
}

/// Backward compatibility function that creates its own GitCommandExecutor
/// Used by existing tests and code that hasn't been updated yet
pub fn perform_fast_cherry_pick<'a>(repo: &'a Repository, cherry_commit: &'a Commit, target_commit: &'a Commit) -> Result<Tree<'a>, CopyCommitError> {
  let git_executor = GitCommandExecutor::new();
  perform_fast_cherry_pick_with_context(repo, cherry_commit, target_commit, &git_executor, None)
}
