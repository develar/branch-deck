use crate::commit_grouper::CommitGrouper;
use crate::issue_navigation::load_issue_navigation_config;
use crate::progress::{GroupedBranchInfo, ProgressReporter, SyncEvent};
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::{get_commit_list_with_handler, Commit};
use git_ops::copy_commit::{create_or_update_commit, CopyCommitError, CreateCommitParams};
use git_ops::git_command::GitCommandExecutor;
use git_ops::model::{to_final_branch_name, BranchError, BranchSyncStatus, CommitSyncStatus};
use git_ops::notes::{write_commit_notes, CommitNoteInfo};
use git_ops::progress::ProgressCallback;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, instrument};

/// Parameters for processing a single branch
struct BranchProcessingParams<'a> {
  repository_path: String,
  branch_prefix: String,
  branch_name: String,
  commits: Vec<Commit>,
  parent_commit_hash: String,
  current_branch_idx: usize,
  total_branches: usize,
  progress: &'a dyn ProgressReporter,
  git_notes_mutex: Arc<Mutex<()>>,
  git_executor: GitCommandExecutor,
  tree_id_cache: TreeIdCache,
}

/// Progress adapter that implements ProgressCallback for the ProgressReporter trait
struct ProgressReporterAdapter<'a> {
  reporter: &'a dyn ProgressReporter,
}

impl<'a> ProgressReporterAdapter<'a> {
  fn new(reporter: &'a dyn ProgressReporter) -> Self {
    Self { reporter }
  }
}

impl<'a> ProgressCallback for ProgressReporterAdapter<'a> {
  fn send_branch_status(&self, branch_name: String, status: BranchSyncStatus, error: Option<BranchError>) -> anyhow::Result<()> {
    self.reporter.send(SyncEvent::BranchStatusUpdate { branch_name, status, error })
  }
}

/// Core sync branches logic without Tauri dependencies
#[instrument(skip(git_executor, progress), fields(repository_path = %repository_path, branch_prefix = %branch_prefix))]
pub async fn sync_branches_core(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, progress: &dyn ProgressReporter) -> anyhow::Result<()> {
  // Load and send issue navigation config at the beginning
  let issue_config = load_issue_navigation_config(repository_path);
  progress.send(SyncEvent::IssueNavigationConfig { config: issue_config })?;

  // Detect the baseline branch (origin/master, origin/main, or local master/main)
  let baseline_branch = git_executor.detect_baseline_branch(repository_path, "master")?;

  // Use streaming commit processing
  let mut grouper = CommitGrouper::new();

  get_commit_list_with_handler(git_executor, repository_path, &baseline_branch, |commit| {
    grouper.add_commit(commit);
    Ok(())
  })?;

  // Check if we have any commits
  if grouper.commit_count == 0 {
    // No commits to sync
    progress.send(SyncEvent::Completed)?;
    return Ok(());
  }

  let oldest_head_commit = grouper
    .oldest_commit
    .as_ref()
    .ok_or_else(|| anyhow::anyhow!("No oldest commit found despite having commits"))?;

  // Get parent commit hash using git CLI
  let parent_commit_hash = {
    let parent_ref = format!("{}^", oldest_head_commit.id);
    let args = vec!["rev-parse", &parent_ref];
    let output = git_executor.execute_command(&args, repository_path)?;
    output.trim().to_string()
  };

  // group commits by prefix first to get all branch names
  let (grouped_commits, unassigned_commits) = grouper.finish();

  // extract commits for processing - no remapping needed anymore
  let grouped_commit_data: Vec<(String, Vec<Commit>)> = grouped_commits.into_iter().collect();

  let total_branches = grouped_commit_data.len();

  info!("Fetched and grouped commits. Total branches: {total_branches}.");

  // Send unassigned commits first if any
  if !unassigned_commits.is_empty() {
    let unassigned_commits_for_ui: Vec<Commit> = unassigned_commits
      .into_iter()
      .rev() // Reverse to show newest commits first
      .collect();

    progress.send(SyncEvent::UnassignedCommits {
      commits: unassigned_commits_for_ui,
    })?;
  }

  // Send grouped branch info so UI can render the structure
  // Create branches with latest commit time for sorting
  let mut grouped_branches_for_ui: Vec<GroupedBranchInfo> = grouped_commit_data
    .iter()
    .map(|(branch_name, commits)| {
      // Find the latest committer time in this branch
      let latest_commit_time = commits.iter().map(|commit| commit.committer_timestamp).max().unwrap_or(0);

      GroupedBranchInfo {
        name: branch_name.clone(),
        latest_commit_time,
        commits: commits
          .iter()
          .rev() // Reverse to show newest commits first within branch
          .cloned()
          .collect(),
      }
    })
    .collect();

  // Sort branches by latest commit time (newest first)
  grouped_branches_for_ui.sort_by(|a, b| b.latest_commit_time.cmp(&a.latest_commit_time));

  progress.send(SyncEvent::BranchesGrouped {
    branches: grouped_branches_for_ui,
  })?;

  // Create a mutex for this sync operation to serialize git notes access
  let git_notes_mutex = Arc::new(Mutex::new(()));

  // Create a shared cache for tree IDs for this sync operation
  let tree_id_cache = TreeIdCache::new();

  // Process branches sequentially (we can make this parallel later)
  for (current_branch_idx, (branch_name, commits)) in grouped_commit_data.into_iter().enumerate() {
    debug!("Processing branch {} of {total_branches}: {branch_name}", current_branch_idx + 1);

    let params = BranchProcessingParams {
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      branch_name,
      commits,
      parent_commit_hash: parent_commit_hash.clone(),
      current_branch_idx,
      total_branches,
      progress,
      git_notes_mutex: git_notes_mutex.clone(),
      git_executor: git_executor.clone(),
      tree_id_cache: tree_id_cache.clone(),
    };

    if let Err(e) = process_single_branch(params).await {
      error!("Failed to process branch: {e}");
      // Error status has already been sent by process_single_branch
    }
  }

  progress.send(SyncEvent::Completed)?;

  Ok(())
}

async fn process_single_branch(params: BranchProcessingParams<'_>) -> anyhow::Result<()> {
  let BranchProcessingParams {
    repository_path,
    branch_prefix,
    branch_name,
    commits,
    parent_commit_hash,
    current_branch_idx,
    total_branches,
    progress,
    git_notes_mutex,
    git_executor,
    tree_id_cache,
  } = params;

  let task_index = current_branch_idx as i16;
  let full_branch_name = to_final_branch_name(&branch_prefix, &branch_name)?;

  let is_existing_branch = check_branch_exists(&git_executor, &repository_path, &full_branch_name);
  debug!("Branch {full_branch_name} exists: {is_existing_branch}");

  let mut current_parent_hash = parent_commit_hash;
  let mut last_commit_hash = String::new();
  let mut is_any_commit_changed = false;
  let mut pending_notes: Vec<CommitNoteInfo> = Vec::new();

  // recreate each commit on top of the last one
  let total_commits_in_branch = commits.len();

  // Collect all commit hashes for potential blocking notifications
  let all_commit_hashes: Vec<String> = commits.iter().map(|c| c.id.to_string()).collect();

  for (current_commit_idx, commit) in commits.into_iter().enumerate() {
    debug!(
      "Processing commit {}/{total_commits_in_branch} in branch {branch_name}: {}",
      current_commit_idx + 1,
      commit.id
    );
    // If any commit in the branch's history up to this point has changed, we still need to copy this commit —
    // even if its own content didn't change — so that its parent reference is updated.
    let reuse_if_possible = is_existing_branch && !is_any_commit_changed;
    // check if we can reuse the tree directly (avoid merge)
    let progress_info = git_ops::copy_commit::ProgressInfo {
      branch_name: &branch_name,
      current_commit_idx,
      total_commits_in_branch,
      current_branch_idx,
      total_branches,
    };

    let progress_adapter = ProgressReporterAdapter::new(progress);
    let commit_params = CreateCommitParams {
      commit: &commit,
      new_parent_oid: current_parent_hash,
      reuse_if_possible,
      repo_path: &repository_path,
      progress: &progress_adapter,
      progress_info: &progress_info,
      task_index,
      git_executor: &git_executor,
      tree_id_cache: &tree_id_cache,
    };

    let original_hash = commit.id.to_string();

    let result = create_or_update_commit(commit_params);

    match result {
      Ok((new_commit_hash, sync_status, note_info)) => {
        // Send success event with status
        let _ = progress.send(SyncEvent::CommitSynced {
          branch_name: branch_name.clone(),
          commit_hash: original_hash.clone(),
          new_hash: new_commit_hash.clone(),
          status: sync_status.clone(),
        });

        if sync_status == CommitSyncStatus::Created {
          is_any_commit_changed = true;
        }

        // Collect note info if present
        if let Some(note) = note_info {
          pending_notes.push(note);
        }

        current_parent_hash = new_commit_hash.clone();
        last_commit_hash = new_commit_hash;
      }
      Err(CopyCommitError::BranchError(branch_error)) => {
        // Send error event for this commit
        let _ = progress.send(SyncEvent::CommitError {
          branch_name: branch_name.clone(),
          commit_hash: original_hash.clone(),
          error: branch_error.clone(),
        });

        // Send blocked events for remaining commits
        let blocked_hashes: Vec<String> = all_commit_hashes.iter().skip(current_commit_idx + 1).cloned().collect();

        if !blocked_hashes.is_empty() {
          let _ = progress.send(SyncEvent::CommitsBlocked {
            branch_name: branch_name.clone(),
            blocked_commit_hashes: blocked_hashes,
          });
        }

        // Send branch completed event with appropriate error status
        let status = match &branch_error {
          BranchError::MergeConflict(_) => BranchSyncStatus::MergeConflict,
          BranchError::Generic(_) => BranchSyncStatus::Error,
        };

        let _ = progress.send(SyncEvent::BranchStatusUpdate {
          branch_name: branch_name.clone(),
          status,
          error: Some(branch_error),
        });

        // Return early - error already sent via events
        return Ok(());
      }
      Err(CopyCommitError::Other(e)) => return Err(e),
    }
  }

  let branch_sync_status: BranchSyncStatus;
  if is_existing_branch {
    if is_any_commit_changed {
      branch_sync_status = BranchSyncStatus::Updated;
      debug!("Branch {branch_name} was updated with new commits");
    } else {
      branch_sync_status = BranchSyncStatus::Unchanged;
      debug!("Branch {branch_name} is unchanged");
    }
  } else {
    branch_sync_status = BranchSyncStatus::Created;
    debug!("Branch {branch_name} was created");
  }

  // only update the branch if it's new or changed
  if branch_sync_status != BranchSyncStatus::Unchanged {
    // Use git CLI to update branch reference
    let commit_hash_str = last_commit_hash.to_string();
    let args = vec!["branch", "-f", &full_branch_name, &commit_hash_str];
    git_executor.execute_command(&args, &repository_path)?;
  }

  // Write all commit notes after successful branch sync
  if !pending_notes.is_empty() {
    debug!("Writing {} commit notes for branch {branch_name}", pending_notes.len());
    if let Err(e) = write_commit_notes(&git_executor, &repository_path, pending_notes, &git_notes_mutex) {
      error!("Failed to write commit notes for branch {branch_name}: {e}");
      // Send error status for git notes failure
      let _ = progress.send(SyncEvent::BranchStatusUpdate {
        branch_name: branch_name.clone(),
        status: BranchSyncStatus::Error,
        error: Some(BranchError::Generic(format!("Failed to write commit notes: {e}"))),
      });

      return Err(e);
    }
  }

  // Send branch status update event
  debug!("Sending final branch status for {branch_name}: {:?}", branch_sync_status);
  let _ = progress.send(SyncEvent::BranchStatusUpdate {
    branch_name: branch_name.clone(),
    status: branch_sync_status.clone(),
    error: None,
  });

  Ok(())
}

pub fn check_branch_exists(git_executor: &GitCommandExecutor, repo_path: &str, branch_name: &str) -> bool {
  // Use git show-ref to check if branch exists - more efficient than rev-parse
  let branch_ref = format!("refs/heads/{branch_name}");
  let args = vec!["show-ref", "--verify", "--quiet", &branch_ref];
  git_executor.execute_command(&args, repo_path).is_ok()
}

#[cfg(test)]
#[path = "sync_test.rs"]
mod tests;
