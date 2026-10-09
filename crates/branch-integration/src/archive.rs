use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, parse_single_commit};
use git_ops::model::{BranchKind, strip_branch_namespace};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::{LazyLock, Mutex, PoisonError};
use tracing::{debug, info, instrument, warn};

/// Days an archived branch is kept after it is known to be fully integrated
pub const ARCHIVE_RETENTION_DAYS: u64 = 7;

/// Process-wide lock serializing branch renames into the archive and unapplied namespaces,
/// so that picking a free name and renaming to it happen atomically.
pub static REF_RENAME_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Rename `from_branch` to `to_base`, appending a numeric suffix (`-1`, `-2`, ...) if that name is taken.
/// The probe loop and the rename run under `REF_RENAME_MUTEX`. Returns the final branch name.
pub fn rename_branch_unique(git: &GitCommandExecutor, repo: &str, from_branch: &str, to_base: &str) -> Result<String> {
  let _guard = REF_RENAME_MUTEX.lock().unwrap_or_else(PoisonError::into_inner);

  let mut target = to_base.to_string();
  let mut suffix: u32 = 1;
  while git.ref_exists(repo, &format!("refs/heads/{target}")) {
    if suffix > 1000 {
      return Err(anyhow::anyhow!("Too many existing branches named like {to_base}"));
    }
    target = format!("{to_base}-{suffix}");
    suffix += 1;
  }

  // git creates the intermediate ref directories itself
  git.execute_command(&["branch", "-m", from_branch, &target], repo)?;
  Ok(target)
}

/// Archive inactive branches using pre-fetched data
/// Returns a map of archived branch name -> commit SHA
#[instrument(skip(git_executor, branch_commits, existing_today_names), fields(branch_count = inactive_branches.len()))]
pub fn batch_archive_inactive_branches(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch_prefix: &str,
  inactive_branches: &[String],
  branch_commits: &HashMap<String, String>,
  existing_today_names: &HashSet<String>,
) -> Result<HashMap<String, String>> {
  if inactive_branches.is_empty() {
    return Ok(HashMap::new());
  }

  let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
  let archive_prefix = format!("{branch_prefix}/archived/{date}");

  // Lock to prevent race conditions
  let _guard = REF_RENAME_MUTEX.lock().unwrap_or_else(PoisonError::into_inner);

  // Build mappings with conflict resolution
  let mut used_names = existing_today_names.clone();
  let mut archive_mappings = Vec::with_capacity(inactive_branches.len());

  for branch_name in inactive_branches {
    let simple_name = strip_branch_namespace(branch_name, branch_prefix, BranchKind::Virtual).unwrap_or(branch_name);

    let mut target_name = simple_name.to_string();
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

/// Delete archived branches in one atomic `update-ref --stdin` transaction (no argv length limit).
/// Returns the number of branches deleted.
#[instrument(skip(git_executor, branches), fields(repo = %repo_path, branch_count = branches.len()))]
pub fn batch_delete_archived_branches(git_executor: &GitCommandExecutor, repo_path: &str, branches: &[String]) -> Result<usize> {
  if branches.is_empty() {
    return Ok(0);
  }

  // Lock to prevent race conditions with concurrent archive renames
  let _guard = REF_RENAME_MUTEX.lock().unwrap_or_else(PoisonError::into_inner);

  let mut commands = String::with_capacity(branches.len() * 64);
  for name in branches {
    writeln!(&mut commands, "delete refs/heads/{name}")?;
  }
  git_executor.execute_command_with_input(&["update-ref", "--stdin"], repo_path, &commands)?;
  Ok(branches.len())
}
