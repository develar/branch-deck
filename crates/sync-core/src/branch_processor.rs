use crate::remote_status::compute_remote_status_for_branch;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::Commit;
use git_ops::copy_commit::{CopyCommitError, CreateCommitParams, create_or_update_commit};
use git_ops::model::{BranchError, BranchSyncStatus, CommitSyncStatus, to_final_branch_name};
use git_ops::notes::{CommitNoteInfo, write_commit_notes};
use git_ops::progress::ProgressCallback;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use sync_types::{ProgressReporter, SyncEvent};
use tracing::{debug, error, instrument, warn};

/// Parameters for processing a single branch
pub(crate) struct BranchProcessingParams<P: ProgressReporter> {
  pub repository_path: String,
  pub branch_prefix: String,
  pub branch_name: String,
  pub commits: Vec<Commit>,
  pub parent_commit_hash: String,
  pub current_branch_idx: usize,
  pub total_branches: usize,
  pub progress: P,
  pub git_executor: GitCommandExecutor,
  pub tree_id_cache: TreeIdCache,
  pub git_notes_mutex: Arc<Mutex<()>>,
  pub my_email: Option<String>,
  pub baseline_branch: String,
}

/// Result of processing a single commit
enum CommitProcessingResult {
  Success {
    new_commit_hash: String,
    sync_status: CommitSyncStatus,
    mapping_info: Option<CommitNoteInfo>,
  },
  BranchError,
}

/// Progress adapter that implements ProgressCallback for the ProgressReporter trait
pub(crate) struct ProgressReporterAdapter<'a, P: ProgressReporter> {
  reporter: &'a P,
}

impl<'a, P: ProgressReporter> ProgressReporterAdapter<'a, P> {
  pub fn new(reporter: &'a P) -> Self {
    Self { reporter }
  }
}

impl<'a, P: ProgressReporter> ProgressCallback for ProgressReporterAdapter<'a, P> {
  fn send_branch_status(&self, branch_name: String, status: BranchSyncStatus, error: Option<BranchError>) -> Result<()> {
    self.reporter.send(SyncEvent::BranchStatusUpdate { branch_name, status, error })
  }
}

/// Check if a local branch exists (refs/heads/<name>) quickly via show-ref --verify --quiet
fn branch_exists(git: &GitCommandExecutor, repo: &str, branch_name: &str) -> bool {
  let branch_ref = format!("refs/heads/{branch_name}");
  git.execute_command(&["show-ref", "--verify", "--quiet", &branch_ref], repo).is_ok()
}

#[instrument(
  skip(params),
  fields(
    branch_name = %params.branch_name,
    branch_idx = params.current_branch_idx + 1,
    total_branches = params.total_branches,
    commit_count = params.commits.len(),
  )
)]
pub(crate) fn process_single_branch<P: ProgressReporter + Clone>(params: BranchProcessingParams<P>) -> Result<()> {
  let BranchProcessingParams {
    repository_path,
    branch_prefix,
    branch_name,
    commits,
    parent_commit_hash,
    current_branch_idx,
    total_branches,
    progress,
    git_executor,
    tree_id_cache,
    git_notes_mutex,
    my_email,
    baseline_branch,
  } = params;

  let task_index = current_branch_idx as i16;
  let full_branch_name = to_final_branch_name(&branch_prefix, &branch_name)?;

  let is_existing_branch = branch_exists(&git_executor, &repository_path, &full_branch_name);
  debug!(name = %full_branch_name, exists = is_existing_branch, "Checking if branch exists");

  // If branch exists, get all its commits in one call for efficient reuse checking
  let existing_virtual_commits = if is_existing_branch {
    match git_executor.execute_command(&["rev-list", &full_branch_name, &format!("^{parent_commit_hash}")], &repository_path) {
      Ok(output) => {
        let commits: HashSet<String> = output.lines().map(|line| line.trim().to_string()).filter(|s| !s.is_empty()).collect();
        debug!(name = %full_branch_name, commit_count = commits.len(), "Got existing virtual commits for reuse checking");
        Some(commits)
      }
      Err(e) => {
        warn!(name = %full_branch_name, error = %e, "Failed to get existing commits, will skip reuse optimization");
        None
      }
    }
  } else {
    None
  };

  let mut current_parent_hash = parent_commit_hash;
  let mut last_commit_hash = String::new();
  let mut is_any_commit_changed = false;
  let mut pending_notes: Vec<CommitNoteInfo> = Vec::new();

  // recreate each commit on top of the last one
  let total_commits_in_branch = commits.len();

  // Collect all commit hashes for potential blocking notifications
  let all_commit_hashes: Vec<String> = commits.iter().map(|c| c.id.to_string()).collect();

  for (current_commit_idx, commit) in commits.into_iter().enumerate() {
    // If any commit in the branch's history up to this point has changed, we still need to copy this commit —
    // even if its own content didn't change — so that its parent reference is updated.
    let reuse_if_possible = is_existing_branch && !is_any_commit_changed;

    let progress_info = git_ops::copy_commit::ProgressInfo {
      branch_name: &branch_name,
      current_commit_idx,
      total_commits_in_branch,
      current_branch_idx,
      total_branches,
    };

    let progress_adapter = ProgressReporterAdapter::new(&progress);
    let commit_params = CreateCommitParams {
      commit: &commit,
      new_parent_oid: current_parent_hash.clone(),
      reuse_if_possible,
      repo_path: &repository_path,
      progress: &progress_adapter,
      progress_info: &progress_info,
      task_index,
      git_executor: &git_executor,
      tree_id_cache: &tree_id_cache,
      existing_virtual_commits: existing_virtual_commits.as_ref(),
    };

    let original_hash = commit.id.to_string();

    match process_single_commit(commit_params, &branch_name, &original_hash, &all_commit_hashes, progress.clone(), &progress_info)? {
      CommitProcessingResult::Success {
        new_commit_hash,
        sync_status,
        mapping_info,
      } => {
        if sync_status == CommitSyncStatus::Created {
          is_any_commit_changed = true;
        }

        // Collect mapping info if present for git notes
        if let Some(mapping) = mapping_info {
          // Only write notes for created commits
          if sync_status == CommitSyncStatus::Created {
            pending_notes.push(mapping);
          }
        }

        current_parent_hash = new_commit_hash.clone();
        last_commit_hash = new_commit_hash;
      }
      CommitProcessingResult::BranchError => {
        // Error already handled and events sent by process_single_commit
        return Ok(());
      }
    }
  }

  let branch_sync_status: BranchSyncStatus;
  if is_existing_branch {
    if is_any_commit_changed {
      branch_sync_status = BranchSyncStatus::Updated;
      debug!(name = %branch_name, "Branch was updated with new commits");
    } else {
      branch_sync_status = BranchSyncStatus::Unchanged;
      debug!(name = %branch_name, "Branch is unchanged");
    }
  } else {
    branch_sync_status = BranchSyncStatus::Created;
    debug!(name = %branch_name, "Branch was created");
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
    debug!(count = pending_notes.len(), name = %branch_name, "Writing commit notes for branch");
    if let Err(e) = write_commit_notes(&git_executor, &repository_path, pending_notes, &git_notes_mutex) {
      error!(name = %branch_name, error = %e, "Failed to write commit notes");
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
  debug!(name = %branch_name, status = ?branch_sync_status, "Sending final branch status");
  let _ = progress.send(SyncEvent::BranchStatusUpdate {
    branch_name: branch_name.clone(),
    status: branch_sync_status.clone(),
    error: None,
  });

  // Compute and emit remote status for this branch
  let local_ref = full_branch_name.clone(); // e.g., "prefix/virtual/name"
  if let Ok(remote_status) = compute_remote_status_for_branch(
    &git_executor,
    &repository_path,
    &local_ref,
    &branch_name,
    my_email.as_deref(),
    total_commits_in_branch as u32,
    &baseline_branch,
  ) {
    let _ = progress.send(SyncEvent::RemoteStatusUpdate(remote_status));
  }

  Ok(())
}

#[instrument(
  skip(commit_params, all_commit_hashes, progress, progress_info),
  fields(
    commit_hash = %original_hash,
    commit_idx = progress_info.current_commit_idx + 1,
    total_commits = progress_info.total_commits_in_branch,
    branch_name = %branch_name,
  )
)]
fn process_single_commit<P: ProgressReporter>(
  commit_params: CreateCommitParams<'_>,
  branch_name: &str,
  original_hash: &str,
  all_commit_hashes: &[String],
  progress: P,
  progress_info: &git_ops::copy_commit::ProgressInfo<'_>,
) -> Result<CommitProcessingResult> {
  let result = create_or_update_commit(commit_params);

  match result {
    Ok((new_commit_hash, sync_status, mapping_info)) => {
      // Send success event with status
      let _ = progress.send(SyncEvent::CommitSynced {
        branch_name: branch_name.to_string(),
        commit_hash: original_hash.to_string(),
        new_hash: new_commit_hash.clone(),
        status: sync_status.clone(),
      });

      Ok(CommitProcessingResult::Success {
        new_commit_hash,
        sync_status,
        mapping_info,
      })
    }
    Err(CopyCommitError::BranchError(branch_error)) => {
      // Send error event for this commit
      let _ = progress.send(SyncEvent::CommitError {
        branch_name: branch_name.to_string(),
        commit_hash: original_hash.to_string(),
        error: branch_error.clone(),
      });

      // Send blocked events for remaining commits
      let blocked_hashes: Vec<String> = all_commit_hashes.iter().skip(progress_info.current_commit_idx + 1).cloned().collect();

      if !blocked_hashes.is_empty() {
        let _ = progress.send(SyncEvent::CommitsBlocked {
          branch_name: branch_name.to_string(),
          blocked_commit_hashes: blocked_hashes,
        });
      }

      // Send branch completed event with appropriate error status
      let status = match &branch_error {
        BranchError::MergeConflict(_) => BranchSyncStatus::MergeConflict,
        BranchError::Generic(_) => BranchSyncStatus::Error,
      };

      let _ = progress.send(SyncEvent::BranchStatusUpdate {
        branch_name: branch_name.to_string(),
        status,
        error: Some(branch_error),
      });

      Ok(CommitProcessingResult::BranchError)
    }
    Err(CopyCommitError::Other(e)) => Err(e),
  }
}
