use crate::commit_key::CommitKey;
use crate::remote_status::{AncestorCommits, StackParent, compute_remote_status_for_branch};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::Commit;
use git_ops::copy_commit::{CopiedCommit, CopyCommitError, CreateCommitParams, create_or_update_commit, read_virtual_commits};
use git_ops::model::{BranchError, BranchSyncStatus, CommitSyncStatus, to_final_branch_name};
use git_ops::notes::MappingNote;
use git_ops::ref_transaction::RefTransaction;
use std::collections::{HashMap, HashSet};
use sync_types::stack::BlockReason;
use sync_types::{ProgressReporter, SyncEvent};
use tracing::{debug, instrument, warn};

/// State shared by all branch tasks of one sync
pub(crate) struct BranchSyncContext<P: ProgressReporter> {
  pub repository_path: String,
  pub branch_prefix: String,
  pub baseline_branch: String,
  pub total_branches: usize,
  pub progress: P,
  pub git_executor: GitCommandExecutor,
  pub tree_id_cache: TreeIdCache,
  /// Full name -> tip of every existing virtual branch, read with one `for-each-ref` per sync
  pub existing_tips: HashMap<String, String>,
  /// Branch -> the identities of the commits of the branches it is built on, for its remote status (`ancestor_keys` in
  /// `sync.rs`); a branch without them reads them when its status needs them
  pub ancestor_keys: HashMap<String, HashSet<CommitKey>>,
  /// Check every mapping note against the tree of its virtual commit, not only by the source it records
  pub verify_mapping_notes: bool,
}

/// One branch to rebuild
pub(crate) struct BranchJob {
  pub branch_name: String,
  /// Commits oldest first
  pub commits: Vec<Commit>,
  pub my_email: Option<String>,
  pub branch_idx: usize,
  /// The commit the branch is built on: the base of the sync (the parent of the oldest commit ahead of the baseline),
  /// or the new head of its parent branch
  pub base_commit: String,
  /// `base_commit` is the new head of the parent branch: the branch's tip gets a stack note naming the base, and the
  /// commits up to the base are the parent's, not unpushed commits of the branch
  pub on_parent: bool,
}

/// A branch whose commits are written; its ref, notes and remote status are not updated yet
pub(crate) struct BuiltBranch {
  /// The new tip, which the children of the branch are built on
  pub head: String,
  branch_name: String,
  full_branch_name: String,
  status: BranchSyncStatus,
  /// The mapping notes of the created commits, and of the reused ones whose note had no source
  notes: Vec<MappingNote>,
  /// `(new tip, base)` of a branch built on its parent whose tip changed, for `STACK_NOTES_REF`
  stack_note: Option<(String, String)>,
  my_email: Option<String>,
  /// The new head of the parent the branch was built on. The remote status excludes the parent's commits by it, not by
  /// the parent's ref: the parent's `finish_branch` may not have moved its ref yet (absent for a new stack, the old head
  /// for an existing one)
  parent_head: Option<String>,
}

impl BuiltBranch {
  pub(crate) fn branch_name(&self) -> &str {
    &self.branch_name
  }
}

pub(crate) enum BuildOutcome {
  Built(BuiltBranch),
  /// A commit could not be copied (`Conflict` or `Error`); its events are sent and the branch is left as it was
  Failed(BlockReason),
}

/// Result of processing a single commit
enum CommitProcessingResult {
  Success {
    new_commit_hash: String,
    sync_status: CommitSyncStatus,
    mapping_info: Option<MappingNote>,
  },
  BranchError(BlockReason),
}

/// Rebuilds one virtual branch and returns the `original -> new` mapping notes of the commits it created; the caller
/// writes the notes. The sync runs the two steps as separate tasks.
#[cfg(test)]
pub(crate) fn process_single_branch<P: ProgressReporter>(ctx: &BranchSyncContext<P>, job: BranchJob) -> Result<Vec<MappingNote>> {
  match build_branch(ctx, job)? {
    BuildOutcome::Built(built) => finish_branch(ctx, built).map(|notes| notes.mappings),
    BuildOutcome::Failed(_) => Ok(Vec::new()),
  }
}

/// Copies the commits of a branch onto its base, reusing the existing ones that still fit, and returns the new head
/// without moving the branch: `finish_branch` does that, off the critical path of the branches built on this one
#[instrument(
  skip(ctx, job),
  fields(
    branch_name = %job.branch_name,
    branch_idx = job.branch_idx + 1,
    total_branches = ctx.total_branches,
    commit_count = job.commits.len(),
  )
)]
pub(crate) fn build_branch<P: ProgressReporter>(ctx: &BranchSyncContext<P>, job: BranchJob) -> Result<BuildOutcome> {
  let BranchSyncContext {
    repository_path,
    branch_prefix,
    total_branches,
    progress,
    git_executor,
    tree_id_cache,
    existing_tips,
    verify_mapping_notes,
    ..
  } = ctx;
  let total_branches = *total_branches;
  let BranchJob {
    branch_name,
    commits,
    my_email,
    branch_idx: current_branch_idx,
    base_commit,
    on_parent,
  } = job;

  let full_branch_name = to_final_branch_name(branch_prefix, &branch_name)?;

  let existing_tip = existing_tips.get(&full_branch_name);
  let is_existing_branch = existing_tip.is_some();
  debug!(name = %full_branch_name, exists = is_existing_branch, "Checking if branch exists");

  // If branch exists, read all its commits in one call: their parents for reuse, their trees, messages and authors to
  // check the mapping notes against
  let existing_virtual_commits = if let Some(existing_tip) = existing_tip {
    match read_virtual_commits(git_executor, repository_path, existing_tip, &base_commit) {
      Ok(commits) => {
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

  let mut current_parent_hash = base_commit.clone();
  // a copy is never older than the commits below it, and its date depends only on the originals and the base
  let mut committer_timestamp = tree_id_cache.get_committer_timestamp(git_executor, repository_path, &base_commit)?;
  let mut last_commit_hash = String::new();
  let mut is_any_commit_changed = false;
  let mut pending_notes: Vec<MappingNote> = Vec::new();

  // recreate each commit on top of the last one
  let total_commits_in_branch = commits.len();
  let on_status = |status: BranchSyncStatus| {
    let _ = progress.send(SyncEvent::BranchStatusUpdate {
      branch_name: branch_name.clone(),
      status,
      error: None,
    });
  };

  for (current_commit_idx, commit) in commits.iter().enumerate() {
    // If any commit in the branch's history up to this point has changed, we still need to copy this commit —
    // even if its own content didn't change — so that its parent reference is updated.
    let reuse_if_possible = is_existing_branch && !is_any_commit_changed;
    committer_timestamp = committer_timestamp.max(commit.committer_timestamp);

    let progress_info = git_ops::copy_commit::ProgressInfo {
      branch_name: &branch_name,
      current_commit_idx,
      total_commits_in_branch,
      current_branch_idx,
      total_branches,
    };

    let commit_params = CreateCommitParams {
      commit,
      new_parent_oid: &current_parent_hash,
      reuse_if_possible,
      repo_path: repository_path,
      on_status: &on_status,
      progress_info: &progress_info,
      git_executor,
      tree_id_cache,
      existing_virtual_commits: existing_virtual_commits.as_ref(),
      verify_notes: *verify_mapping_notes,
      committer_timestamp,
    };

    match process_single_commit(commit_params, &branch_name, &commit.id, &commits[current_commit_idx + 1..], progress, &progress_info)? {
      CommitProcessingResult::Success {
        new_commit_hash,
        sync_status,
        mapping_info,
      } => {
        if sync_status == CommitSyncStatus::Created {
          is_any_commit_changed = true;
        }
        // the note of a created commit, or the source a reused commit's older note lacked
        pending_notes.extend(mapping_info);

        current_parent_hash.clone_from(&new_commit_hash);
        last_commit_hash = new_commit_hash;
      }
      CommitProcessingResult::BranchError(reason) => {
        // Error already handled and events sent by process_single_commit
        return Ok(BuildOutcome::Failed(reason));
      }
    }
  }
  // a child copies its first commit onto the head; its date is the one computed here, also for a reused head
  tree_id_cache.insert_committer_timestamp(&last_commit_hash, committer_timestamp);

  let status = if !is_existing_branch {
    debug!(name = %branch_name, "Branch was created");
    BranchSyncStatus::Created
  } else if is_any_commit_changed || existing_tip != Some(&last_commit_hash) {
    // every commit may be reused, but commits at the end may have left the branch
    debug!(name = %branch_name, "Branch was updated with new commits");
    BranchSyncStatus::Updated
  } else {
    debug!(name = %branch_name, "Branch is unchanged");
    BranchSyncStatus::Unchanged
  };

  let parent_head = on_parent.then_some(base_commit);
  let stack_note = parent_head
    .as_ref()
    .filter(|_| status != BranchSyncStatus::Unchanged)
    .map(|base| (last_commit_hash.clone(), base.clone()));
  Ok(BuildOutcome::Built(BuiltBranch {
    stack_note,
    head: last_commit_hash,
    branch_name,
    full_branch_name,
    status,
    notes: pending_notes,
    my_email,
    parent_head,
  }))
}

/// The notes a branch leaves for the sync to write
#[derive(Default)]
pub(crate) struct BranchNotes {
  /// The mapping notes of the created commits, and of the reused ones whose note had no source
  pub mappings: Vec<MappingNote>,
  /// `(tip, base)` of a stacked branch, see `BuiltBranch::stack_note`
  pub stack_note: Option<(String, String)>,
}

/// Moves the branch to its new head if it changed, reports its final status and its remote status, and returns the
/// notes to write
#[instrument(skip(ctx, built), fields(branch_name = %built.branch_name))]
pub(crate) fn finish_branch<P: ProgressReporter>(ctx: &BranchSyncContext<P>, built: BuiltBranch) -> Result<BranchNotes> {
  let BuiltBranch {
    head,
    branch_name,
    full_branch_name,
    status,
    notes,
    stack_note,
    my_email,
    parent_head,
  } = built;
  let git_executor = &ctx.git_executor;
  let repository_path = &ctx.repository_path;

  // only update the branch if it's new or changed
  if status != BranchSyncStatus::Unchanged {
    RefTransaction::new()
      .update(&format!("refs/heads/{full_branch_name}"), &head, None)
      .commit(git_executor, repository_path, "sync")?;
  }

  debug!(name = %branch_name, status = ?status, "Sending final branch status");
  let _ = ctx.progress.send(SyncEvent::BranchStatusUpdate {
    branch_name: branch_name.clone(),
    status,
    error: None,
  });

  let remote_status = compute_remote_status_for_branch(
    git_executor,
    repository_path,
    "origin",
    &full_branch_name,
    &branch_name,
    my_email.as_deref(),
    &ctx.baseline_branch,
    parent_head.as_deref().map(|base| StackParent {
      base,
      ancestors: ctx.ancestor_keys.get(&branch_name).map_or(AncestorCommits::ReadWhenNeeded, AncestorCommits::Known),
    }),
  );
  match remote_status {
    Ok(remote_status) => {
      let _ = ctx.progress.send(SyncEvent::RemoteStatusUpdate(remote_status));
    }
    // the branch keeps the remote status it had: the sync is not failed for it, the next sync computes it again
    Err(e) => warn!(branch = %branch_name, error = ?e, "Cannot compute the remote status"),
  }

  Ok(BranchNotes { mappings: notes, stack_note })
}

#[instrument(
  skip(commit_params, remaining_commits, progress, progress_info),
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
  remaining_commits: &[Commit],
  progress: &P,
  progress_info: &git_ops::copy_commit::ProgressInfo<'_>,
) -> Result<CommitProcessingResult> {
  let result = create_or_update_commit(commit_params);

  match result {
    Ok(CopiedCommit {
      hash: new_commit_hash,
      status: sync_status,
      note: mapping_info,
    }) => {
      // Send success event with status
      let _ = progress.send(SyncEvent::CommitSynced {
        branch_name: branch_name.to_string(),
        commit_hash: original_hash.to_string(),
        new_hash: new_commit_hash.clone(),
        status: sync_status,
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
      let blocked_hashes: Vec<String> = remaining_commits.iter().map(|c| c.id.clone()).collect();

      if !blocked_hashes.is_empty() {
        let _ = progress.send(SyncEvent::CommitsBlocked {
          branch_name: branch_name.to_string(),
          blocked_commit_hashes: blocked_hashes,
        });
      }

      // Send branch completed event with appropriate error status
      let (status, reason) = match &branch_error {
        BranchError::MergeConflict(_) => (BranchSyncStatus::MergeConflict, BlockReason::Conflict),
        BranchError::Generic(_) => (BranchSyncStatus::Error, BlockReason::Error),
      };

      let _ = progress.send(SyncEvent::BranchStatusUpdate {
        branch_name: branch_name.to_string(),
        status,
        error: Some(branch_error),
      });

      Ok(CommitProcessingResult::BranchError(reason))
    }
    Err(CopyCommitError::Other(e)) => Err(e),
  }
}

#[cfg(test)]
#[path = "branch_processor_test.rs"]
mod branch_processor_test;
