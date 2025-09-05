use crate::cache::TreeIdCache;
use crate::commit_list::Commit;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::{debug, info, instrument};

#[derive(Debug, Clone)]
pub struct RewordCommitParams {
  pub commit_id: String,
  pub new_message: String,
}

/// Reword multiple commits efficiently using git plumbing commands.
/// Returns a mapping of old commit IDs to new commit IDs.
#[instrument(skip(git_executor))]
pub fn reword_commits_batch(git_executor: &GitCommandExecutor, repo_path: &str, rewrites: Vec<RewordCommitParams>) -> Result<HashMap<String, String>> {
  if rewrites.is_empty() {
    return Ok(HashMap::new());
  }

  info!("Rewording {} commits", rewrites.len());

  // Get current branch
  let current_branch = get_current_branch(git_executor, repo_path)?;

  // Create a map for quick lookup
  let rewrite_map: HashMap<String, String> = rewrites.into_iter().map(|r| (r.commit_id, r.new_message)).collect();

  // Get all commits from the oldest rewrite to HEAD
  let (commits_to_process, original_tip) = get_commits_to_process(git_executor, repo_path, &rewrite_map)?;

  // Create a shared cache for tree IDs
  let _tree_id_cache = TreeIdCache::new();

  // Process commits from oldest to newest, creating new commits as needed
  let mut id_mapping: HashMap<String, String> = HashMap::new();

  for commit_id in &commits_to_process {
    // Determine the new parent (if the parent was rewritten, use the new ID)
    let parent_id = get_commit_parent(git_executor, repo_path, commit_id)?;
    let new_parent_id = parent_id.as_ref().map(|p| id_mapping.get(p).cloned().unwrap_or_else(|| p.clone()));

    // Check if this commit needs rewording or its parent was rewritten
    let needs_new_commit = rewrite_map.contains_key(commit_id) || parent_id.as_ref().is_some_and(|p| id_mapping.contains_key(p));

    if needs_new_commit {
      // Get commit info
      let commit_info = get_commit_info(git_executor, repo_path, commit_id)?;

      // Use the new message if this commit needs rewording, otherwise keep original
      let message = rewrite_map.get(commit_id).cloned().unwrap_or(commit_info.message.clone());

      // Create new commit
      let new_commit_id = create_commit_with_info(git_executor, repo_path, &commit_info, new_parent_id.as_deref(), &message)?;

      id_mapping.insert(commit_id.clone(), new_commit_id.clone());

      if rewrite_map.contains_key(commit_id) {
        debug!("Reworded commit {} -> {}", commit_id, new_commit_id);
      } else {
        debug!("Recreated commit {} -> {} (parent changed)", commit_id, new_commit_id);
      }
    }
  }

  // Update the branch to point to the new tip
  let new_tip = id_mapping.get(&original_tip).cloned().unwrap_or(original_tip);
  update_branch_ref(git_executor, repo_path, &current_branch, &new_tip)?;

  info!("Successfully reworded {} commits", rewrite_map.len());

  // Return only the mapping for commits that were actually reworded
  Ok(id_mapping.into_iter().filter(|(old_id, _)| rewrite_map.contains_key(old_id)).collect())
}

fn get_current_branch(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String> {
  let output = git_executor.execute_command(&["symbolic-ref", "--short", "HEAD"], repo_path)?;

  let branch = output.trim().to_string();
  if branch.is_empty() {
    return Err(anyhow!("Not on any branch (detached HEAD state)"));
  }

  Ok(branch)
}

fn get_commits_to_process(git_executor: &GitCommandExecutor, repo_path: &str, rewrite_map: &HashMap<String, String>) -> Result<(Vec<String>, String)> {
  // Get all commits to HEAD to find the oldest one chronologically
  let all_commits = git_executor.execute_command_lines(&["rev-list", "HEAD"], repo_path)?;

  // Find the oldest commit that needs rewording (appears last in rev-list output)
  let mut oldest_index = None;
  for (i, commit) in all_commits.iter().enumerate() {
    if rewrite_map.contains_key(commit) {
      oldest_index = Some(i);
    }
  }

  let oldest_index = oldest_index.ok_or_else(|| anyhow!("No commits to reword found in history"))?;
  let oldest_commit = &all_commits[oldest_index];

  // Check if oldest commit has a parent
  let parent_check = git_executor.execute_command(&["rev-parse", &format!("{oldest_commit}^")], repo_path);

  // Get all commits from oldest (or its parent) to HEAD in reverse order (oldest first)
  let commits = if parent_check.is_ok() {
    // Has parent, use parent as starting point
    let range = format!("{oldest_commit}^..HEAD");
    git_executor.execute_command_lines(&["rev-list", "--reverse", &range], repo_path)?
  } else {
    // No parent (root commit), include all commits
    git_executor.execute_command_lines(&["rev-list", "--reverse", "HEAD"], repo_path)?
  };

  if commits.is_empty() {
    return Err(anyhow!("No commits found in range"));
  }

  let tip = commits.last().unwrap().clone();

  Ok((commits, tip))
}

fn get_commit_parent(git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<Option<String>> {
  let args = vec!["rev-list", "--parents", "-n", "1", commit_id];
  let output = git_executor.execute_command(&args, repo_path)?;

  let parts: Vec<&str> = output.split_whitespace().collect();
  if parts.len() > 1 { Ok(Some(parts[1].to_string())) } else { Ok(None) }
}

pub fn get_commit_info(git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<Commit> {
  // Use git show with format to get all commit info at once
  let format = "%an%n%ae%n%at%n%ct%n%T%n%P%n%B";
  let format_arg = format!("--format={format}");
  let args = vec!["show", "-s", &format_arg, commit_id];

  let output = git_executor.execute_command(&args, repo_path)?;

  let lines: Vec<&str> = output.lines().collect();
  if lines.len() < 7 {
    return Err(anyhow!("Invalid commit info format"));
  }

  // Parse the output
  let author_name = lines[0].to_string();
  let author_email = lines[1].to_string();
  let author_timestamp: u32 = lines[2].parse().map_err(|_| anyhow!("Invalid author time"))?;
  let committer_timestamp: u32 = lines[3].parse().map_err(|_| anyhow!("Invalid committer time"))?;
  let tree_id = lines[4].to_string();
  let parent_id = if lines[5].is_empty() { None } else { Some(lines[5].to_string()) };
  let message = lines[6..].join("\n");
  let subject = message.lines().next().unwrap_or("").to_string();

  Ok(Commit {
    id: commit_id.to_string(),
    subject: subject.clone(),
    message,
    author_name,
    author_email,
    author_timestamp,
    committer_timestamp,
    parent_id,
    tree_id,
    note: None,
    stripped_subject: subject, // Same as subject since we're not stripping
    mapped_commit_id: None,    // Not relevant for rewording
  })
}

/// Create a commit using existing Commit, optionally with a different parent and message
fn create_commit_with_info(git_executor: &GitCommandExecutor, repo_path: &str, commit: &Commit, new_parent_id: Option<&str>, message: &str) -> Result<String> {
  let mut args = vec!["commit-tree", &commit.tree_id];

  if let Some(parent) = new_parent_id.or(commit.parent_id.as_deref()) {
    args.push("-p");
    args.push(parent);
  }

  args.push("-m");
  args.push(message);

  // Preserve original author and committer info
  let author_date = commit.author_timestamp.to_string();
  let committer_date = commit.committer_timestamp.to_string();

  let env_vars = vec![
    ("GIT_AUTHOR_NAME", commit.author_name.as_str()),
    ("GIT_AUTHOR_EMAIL", commit.author_email.as_str()),
    ("GIT_AUTHOR_DATE", &author_date),
    ("GIT_COMMITTER_NAME", commit.author_name.as_str()),
    ("GIT_COMMITTER_EMAIL", commit.author_email.as_str()),
    ("GIT_COMMITTER_DATE", &committer_date),
  ];

  let output = git_executor
    .execute_command_with_env(&args, repo_path, &env_vars)
    .map_err(|e| anyhow!("Failed to create commit: {}", e))?;

  Ok(output.trim().to_string())
}

pub fn update_branch_ref(git_executor: &GitCommandExecutor, repo_path: &str, branch_name: &str, new_commit_id: &str) -> Result<()> {
  let ref_name = format!("refs/heads/{branch_name}");
  let args = vec!["update-ref", &ref_name, new_commit_id];

  git_executor.execute_command(&args, repo_path)?;

  info!("Updated branch {} to {}", branch_name, new_commit_id);

  Ok(())
}
