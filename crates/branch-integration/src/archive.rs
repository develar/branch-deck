use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, parse_single_commit};
use git_ops::model::extract_branch_name_from_final;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::{LazyLock, Mutex};
use tracing::{debug, info, instrument, warn};

// Global mutex to prevent race conditions when creating archive directories
pub static ARCHIVE_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Rename a branch into an archive namespace, ensuring no collisions by appending a numeric suffix if needed.
/// Returns the final archive ref name.
pub fn archive_ref_unique(git: &GitCommandExecutor, repo: &str, from_branch: &str, to_prefix: &str, name: &str) -> Result<String> {
  let base = format!("{to_prefix}/{name}");
  let mut target = base.clone();
  let mut suffix: u32 = 1;
  loop {
    let full_ref = format!("refs/heads/{target}");
    let exists = git.execute_command(&["show-ref", "--verify", &full_ref], repo).is_ok();
    if !exists {
      break;
    }
    target = format!("{base}-{suffix}");
    suffix += 1;
    if suffix > 1000 {
      warn!("archive_ref_unique: excessive collisions for {}", base);
      break;
    }
  }

  // Lock to prevent race conditions when multiple tasks try to create branches in same directory
  let _guard = ARCHIVE_MUTEX.lock().map_err(|e| anyhow::anyhow!("Failed to acquire archive mutex: {}", e))?;

  // The archive path (e.g., user/archived/2025-08-11) requires directory structure in .git/refs.
  // Since virtual branches don't contain '/', only the archive prefix creates nested paths.
  // Create a temporary branch to establish the directory structure.
  if target.matches('/').count() > 1
    && let Some((parent_path, _)) = target.rsplit_once('/')
  {
    // Create temporary branch to establish the directory structure
    // This uses git's own mechanisms rather than manipulating .git directly
    let temp_branch = format!("{parent_path}/.archive-temp");

    // Try to create the temp branch (ignore errors if it already exists)
    let _ = git.execute_command(&["branch", &temp_branch, "HEAD"], repo);

    // Immediately delete it (the directory structure remains in git)
    let _ = git.execute_command(&["branch", "-D", &temp_branch], repo);
  }

  // Now rename the branch to its archive location
  git.execute_command(&["branch", "-m", from_branch, &target], repo)?;
  Ok(target)
}

/// Archive a branch by moving it to the archive namespace
/// Returns the full archived branch name
#[instrument(skip(git_executor), fields(from = %branch_name, prefix = %branch_prefix))]
pub fn archive_branch(git_executor: &GitCommandExecutor, repo_path: &str, branch_name: &str, branch_prefix: &str) -> anyhow::Result<String> {
  let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
  let simple_name = extract_branch_name_from_final(branch_name, branch_prefix).unwrap_or_else(|| branch_name.to_string());
  let archive_prefix = format!("{branch_prefix}/archived/{date}");

  let target = archive_ref_unique(git_executor, repo_path, branch_name, &archive_prefix, &simple_name)?;
  info!(to = %target, "Successfully archived branch");
  Ok(target)
}

/// Archive inactive branches using pre-fetched data
/// Returns a map of archived branch name -> commit SHA
#[instrument(skip(git_executor, branch_commits, existing_today_names), fields(branch_count = inactive_branches.len()))]
pub fn batch_archive_inactive_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  inactive_branches: Vec<String>,
  branch_commits: &HashMap<String, String>,
  existing_today_names: &HashSet<String>,
) -> Result<HashMap<String, String>> {
  if inactive_branches.is_empty() {
    return Ok(HashMap::new());
  }

  let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
  let archive_prefix = format!("{branch_prefix}/archived/{date}");

  // Lock to prevent race conditions
  let _guard = ARCHIVE_MUTEX.lock().map_err(|e| anyhow::anyhow!("Failed to acquire archive mutex: {}", e))?;

  // Build mappings with conflict resolution
  let mut used_names = existing_today_names.clone();
  let mut archive_mappings = Vec::with_capacity(inactive_branches.len());

  for branch_name in &inactive_branches {
    let simple_name = extract_branch_name_from_final(branch_name, branch_prefix).unwrap_or_else(|| branch_name.to_string());

    let mut target_name = simple_name.clone();
    let mut suffix = 1u32;

    // Check for conflicts with existing archives and current batch
    while used_names.contains(&target_name) {
      target_name = format!("{simple_name}-{suffix}");
      suffix += 1;
      if suffix > 100 {
        warn!("Excessive naming conflicts for {}", simple_name);
        break;
      }
    }

    used_names.insert(target_name.clone());
    let archived_name = format!("{archive_prefix}/{target_name}");
    archive_mappings.push((branch_name.clone(), archived_name));
  }

  // Create archive directory structure
  let temp_branch = format!("{archive_prefix}/archive-temp");
  let _ = git_executor.execute_command(&["branch", &temp_branch, "HEAD"], repo_path);
  let _ = git_executor.execute_command(&["branch", "-D", &temp_branch], repo_path);

  // Build batch commands efficiently
  let mut batch_commands = String::with_capacity(archive_mappings.len() * 150 + 20);
  batch_commands.push_str("start\n");

  let mut newly_archived = HashMap::with_capacity(archive_mappings.len());

  for (original_branch, archived_branch) in &archive_mappings {
    if let Some(commit_hash) = branch_commits.get(original_branch) {
      debug!(
        original_branch = %original_branch,
        archived_branch = %archived_branch,
        commit_hash = %commit_hash,
        "Archiving branch with commit mapping"
      );
      // Use write! for efficiency
      writeln!(&mut batch_commands, "create refs/heads/{archived_branch} {commit_hash}")?;
      writeln!(&mut batch_commands, "delete refs/heads/{original_branch} {commit_hash}")?;
      newly_archived.insert(archived_branch.clone(), commit_hash.clone());
    } else {
      warn!(branch = %original_branch, "Branch not found in commit map, skipping archive");
    }
  }

  batch_commands.push_str("commit\n");

  // Execute the batch update atomically
  git_executor.execute_command_with_input(&["update-ref", "--stdin"], repo_path, &batch_commands)?;

  info!(
    archived_count = newly_archived.len(),
    archive_prefix = %archive_prefix,
    "Successfully archived branches in batch"
  );

  Ok(newly_archived)
}

/// Get commits for an archived branch (integrated or orphaned)
/// This function retrieves all commits from a virtual branch, regardless of whether
/// it's integrated or orphaned. For orphaned branches, the commits still exist in the
/// virtual branch even though the original source commits are gone from HEAD.
#[instrument(skip(git_executor))]
pub fn get_archived_branch_commits(git_executor: &GitCommandExecutor, repository_path: &str, branch_name: &str, baseline_branch: &str) -> Result<Vec<Commit>> {
  // The branch_name is the full branch path (e.g., "user/archived/2025-08-11/feature-auth")
  let actual_branch_name = branch_name.to_string();

  // Get the merge-base to find where the branch diverged from baseline
  let merge_base = git_executor.execute_command(&["merge-base", baseline_branch, &actual_branch_name], repository_path)?;
  let merge_base = merge_base.trim();

  // Get all commits from merge-base to branch tip
  let range = format!("{merge_base}..{actual_branch_name}");
  debug!(branch = %actual_branch_name, merge_base = %merge_base, range = %range, "Getting all commits on branch since divergence");
  let args = vec![
    "--no-pager",
    "log",
    "--reverse",
    "--no-merges",
    "--pretty=format:%H%x1f%B%x1f%an%x1f%ae%x1f%at%x1f%ct%x1f%P%x1f%T%x1f%N%x1e",
    &range,
  ];

  let output = git_executor.execute_command(&args, repository_path)?;

  // Parse commits
  let mut commits = Vec::new();
  for record in output.split('\x1e') {
    let record = record.trim();
    if !record.is_empty()
      && let Ok(commit) = parse_single_commit(record)
    {
      commits.push(commit);
    }
  }

  debug!(branch = %actual_branch_name, commit_count = commits.len(), "Retrieved commits from archived branch");
  Ok(commits)
}

/// Batch-delete archived branches using `git branch -D branch1 branch2 ...` with a mutex to avoid races
/// Returns the number of branches deleted (best-effort; on fallback it counts successful deletions)
#[instrument(skip(git_executor, branches), fields(repo = %repo_path, branch_count = branches.len()))]
pub fn batch_delete_archived_branches(git_executor: &GitCommandExecutor, repo_path: &str, branches: &[String]) -> Result<usize> {
  if branches.is_empty() {
    return Ok(0);
  }

  // Lock to prevent race conditions during deletion
  let _guard = ARCHIVE_MUTEX.lock().map_err(|e| anyhow::anyhow!("Failed to acquire archive mutex: {}", e))?;

  // Build args for a single git call: `git branch -D <branches...>`
  let mut args: Vec<&str> = Vec::with_capacity(2 + branches.len());
  args.push("branch");
  args.push("-D");
  for name in branches.iter() {
    args.push(name.as_str());
  }

  match git_executor.execute_command(&args, repo_path) {
    Ok(_) => Ok(branches.len()),
    Err(e) => {
      // Fallback: attempt individual deletions to make best effort
      tracing::warn!(error = %e, "Batch delete via git branch -D failed, attempting individual deletions");
      let mut fallback_deleted = 0usize;
      for name in branches.iter() {
        if git_executor.execute_command(["branch", "-D", name.as_str()].as_slice(), repo_path).is_ok() {
          fallback_deleted += 1;
        }
      }
      Ok(fallback_deleted)
    }
  }
}
