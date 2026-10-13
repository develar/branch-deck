use crate::commit_utils::{create_commit_with_metadata, get_commit_info, prefetch_commit_infos_map};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, instrument};

#[derive(Debug, Clone)]
pub struct RewordCommitParams {
  pub commit_id: String,
  pub new_message: String,
}

/// Reword multiple commits efficiently using git plumbing commands.
/// Returns a mapping of old commit IDs to new commit IDs.
///
/// The trees stay the same, so the working tree and the index are not touched. Fails while a rebase, merge,
/// cherry-pick or revert is in progress, and if the branch moved while the new commits were created.
#[instrument(skip(git_executor))]
pub fn reword_commits_batch(git_executor: &GitCommandExecutor, repo_path: &str, rewrites: Vec<RewordCommitParams>) -> Result<HashMap<String, String>> {
  if rewrites.is_empty() {
    return Ok(HashMap::new());
  }

  info!("Rewording {} commits", rewrites.len());

  ensure_no_operation_in_progress(git_executor, repo_path)?;

  // Get current branch
  let current_branch = get_current_branch(git_executor, repo_path)?;

  // Create a map for quick lookup
  let rewrite_map: HashMap<String, String> = rewrites.into_iter().map(|r| (r.commit_id, r.new_message)).collect();

  // Get all commits from the oldest rewrite to HEAD with their parents, and the range used
  let (commits_to_process, original_tip, process_range) = get_commits_to_process(git_executor, repo_path, &rewrite_map)?;

  // Prefetch commit infos for the entire range in one go
  let commit_info_map = prefetch_commit_infos_map(git_executor, repo_path, &process_range)?;

  // Process commits from oldest to newest, creating new commits as needed
  let mut id_mapping: HashMap<String, String> = HashMap::new();

  for (commit_id, parent_id) in &commits_to_process {
    // Determine the new parent (if the parent was rewritten, use the new ID)
    let new_parent_id = parent_id.as_ref().map(|p| id_mapping.get(p).cloned().unwrap_or_else(|| p.clone()));

    // Check if this commit needs rewording or its parent was rewritten
    let needs_new_commit = rewrite_map.contains_key(commit_id) || parent_id.as_ref().is_some_and(|p| id_mapping.contains_key(p));

    if needs_new_commit {
      // Get commit info from prefetch map or fall back to single query
      let commit_info = match commit_info_map.get(commit_id).cloned() {
        Some(ci) => ci,
        None => get_commit_info(git_executor, repo_path, commit_id)?,
      };

      // Use the new message if this commit needs rewording, otherwise keep original
      let message = rewrite_map.get(commit_id).cloned().unwrap_or(commit_info.message.clone());

      // Create new commit
      let new_commit_id = create_commit_with_metadata(git_executor, repo_path, &commit_info.tree_id, new_parent_id.as_deref(), &commit_info, &message)?;

      id_mapping.insert(commit_id.clone(), new_commit_id.clone());

      if rewrite_map.contains_key(commit_id) {
        debug!("Reworded commit {} -> {}", commit_id, new_commit_id);
      } else {
        debug!("Recreated commit {} -> {} (parent changed)", commit_id, new_commit_id);
      }
    }
  }

  // Update the branch to point to the new tip
  let new_tip = id_mapping.get(&original_tip).cloned().unwrap_or_else(|| original_tip.clone());
  // a commit made meanwhile (e.g. in a terminal) is not dropped: the update fails instead
  update_branch_ref(git_executor, repo_path, &current_branch, &new_tip, Some(&original_tip))
    .map_err(|e| anyhow!("Branch {current_branch} changed while rewording, sync and try again: {e:#}"))?;

  info!("Successfully reworded {} commits", rewrite_map.len());

  // Return only the mapping for commits that were actually reworded
  Ok(id_mapping.into_iter().filter(|(old_id, _)| rewrite_map.contains_key(old_id)).collect())
}

/// Fails if git is in the middle of a rebase, merge, cherry-pick or revert: rewriting history then would corrupt it.
pub fn ensure_no_operation_in_progress(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
  const MARKERS: [(&str, &str); 5] = [
    ("rebase-merge", "a rebase"),
    ("rebase-apply", "a rebase"),
    ("MERGE_HEAD", "a merge"),
    ("CHERRY_PICK_HEAD", "a cherry-pick"),
    ("REVERT_HEAD", "a revert"),
  ];

  let mut args = vec!["rev-parse"];
  for (marker, _) in MARKERS {
    args.push("--git-path");
    args.push(marker);
  }
  // one path per marker, relative to the repository directory (or absolute in a linked worktree)
  let paths = git_executor.execute_command_lines(&args, repo_path)?;
  for ((_, operation), path) in MARKERS.iter().zip(paths) {
    if Path::new(repo_path).join(path).exists() {
      return Err(anyhow!("Cannot rewrite commits while {operation} is in progress"));
    }
  }
  Ok(())
}

/// Short name of the checked out branch; fails in the detached HEAD state
pub fn get_current_branch(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String> {
  let output = git_executor.execute_command(&["symbolic-ref", "--short", "HEAD"], repo_path)?;

  let branch = output.trim().to_string();
  if branch.is_empty() {
    return Err(anyhow!("Not on any branch (detached HEAD state)"));
  }

  Ok(branch)
}

/// Commit ID with its first parent (None for a root commit)
type CommitWithParent = (String, Option<String>);

fn get_commits_to_process(git_executor: &GitCommandExecutor, repo_path: &str, rewrite_map: &HashMap<String, String>) -> Result<(Vec<CommitWithParent>, String, String)> {
  // Get all commits on first-parent to HEAD to find the oldest one chronologically among rewrites
  let all_commits = git_executor.execute_command_lines(&["rev-list", "--first-parent", "HEAD"], repo_path)?;

  // Find the oldest commit that needs rewording (appears last in rev-list output)
  let mut oldest_index = None;
  for (i, commit) in all_commits.iter().enumerate() {
    if rewrite_map.contains_key(commit) {
      oldest_index = Some(i);
    }
  }

  let oldest_index = oldest_index.ok_or_else(|| anyhow!("No commits to reword found in history"))?;
  let oldest_commit = &all_commits[oldest_index];

  // Construct the processing range: if oldest has a parent, start from parent, else from root (HEAD traversal covers all)
  let parent_check = git_executor.execute_command(&["rev-parse", &format!("{oldest_commit}^")], repo_path);
  let range = if parent_check.is_ok() { format!("{oldest_commit}^..HEAD") } else { "HEAD".to_string() };

  // Get all commits (first-parent) from range in reverse with parents so we can avoid per-commit parent lookups
  let lines = git_executor.execute_command_lines(&["rev-list", "--first-parent", "--reverse", "--parents", &range], repo_path)?;

  if lines.is_empty() {
    return Err(anyhow!("No commits found in range"));
  }

  let mut commits_with_parents = Vec::with_capacity(lines.len());
  for line in lines {
    let mut parts = line.split_whitespace();
    if let Some(commit) = parts.next() {
      let parent = parts.next().map(std::string::ToString::to_string);
      commits_with_parents.push((commit.to_string(), parent));
    }
  }

  let tip = commits_with_parents.last().map(|(id, _)| id.clone()).ok_or_else(|| anyhow!("Failed to determine tip"))?;

  Ok((commits_with_parents, tip, range))
}

/// Points the branch at `new_commit_id`. With `expected_commit_id`, fails unless the branch still points at it.
pub fn update_branch_ref(git_executor: &GitCommandExecutor, repo_path: &str, branch_name: &str, new_commit_id: &str, expected_commit_id: Option<&str>) -> Result<()> {
  let ref_name = format!("refs/heads/{branch_name}");
  let mut args = vec!["update-ref", &ref_name, new_commit_id];
  if let Some(expected_commit_id) = expected_commit_id {
    args.push(expected_commit_id);
  }

  git_executor.execute_command(&args, repo_path)?;

  info!("Updated branch {} to {}", branch_name, new_commit_id);

  Ok(())
}
