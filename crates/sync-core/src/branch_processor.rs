//! The build of one virtual branch of a sync: its commits copied (`git_ops::copy_commit`) with one `merge-tree --stdin`
//! and one `fast-import`, then its ref moved with compare-and-swap on the tip the sync listed, its remote status
//! computed and its notes returned for the one notes write of the sync.

use crate::commit_key::CommitKey;
use crate::conflict_chain::{ChainMemo, ConflictChain, conflict_ref};
use crate::missing_commits::CommitOwners;
use crate::remote_status::{AncestorCommits, RemoteStatusMemo, StackParent, compute_remote_status_for_branch};
use anyhow::Result;
use domain_types::branch_names::to_final_branch_name;
use domain_types::commit::{Commit, NoteResolution};
use domain_types::conflict::WordResolutionSummary;
use domain_types::stack::BlockReason;
use domain_types::{BranchError, BranchSyncStatus, CommitSyncStatus, ProgressReporter, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_chain::CommitChain;
use git_ops::copy_commit::{CopiedCommit, CopyCommitError, CreateCommitParams, RememberedCopy, VirtualCommit, create_or_update_commit, read_virtual_commits};
use git_ops::notes::MappingNote;
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::{resolve_commit, short_id};
use git_ops::tree_id_cache::TreeIdCache;
use git_ops::word_resolve::{ResolveMode, note_resolution};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
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
  /// The remote statuses on the commits read once per sync (the remote tips, the baseline), shared with the stack
  /// branches of the sync; `None` reads them per branch
  pub remote_statuses: Option<Arc<RemoteStatusMemo>>,
  /// The commits of all existing virtual branches, read once per sync (once per session of checks, which share them);
  /// `None` reads them per branch
  pub existing_commits: Option<Arc<ExistingCommits>>,
  /// Branch -> the identities of the commits of the branches it is built on, for its remote status (`ancestor_keys` in
  /// `sync.rs`); a branch without them reads them when its status needs them
  pub ancestor_keys: HashMap<String, HashSet<CommitKey>>,
  /// Check every mapping note against the tree of its virtual commit, not only by the source it records
  pub verify_mapping_notes: bool,
  /// The branch of each commit, for the missing commits of a conflict
  pub owners: CommitOwners,
  /// The copies of the conflicting branches before their conflicts, remembered by the object cache; only a sync has
  /// them: a check rewrites messages under the same commit ids
  pub conflict_chains: Option<ChainMemo>,
  /// Ref -> tip of the refs that keep those copies (`conflict_chain::CONFLICT_REFS`), read once per sync
  pub conflict_tips: HashMap<String, String>,
  /// Whether a conflict of git resolves by words (`branchdeck.resolveByWords`)
  pub resolve: ResolveMode,
}

/// The commits of every existing virtual branch above the base of the sync, read with one `log` by the first branch
/// that needs them, instead of one per branch
pub(crate) struct ExistingCommits {
  base: String,
  tips: Vec<String>,
  commits: OnceLock<Option<HashMap<String, VirtualCommit>>>,
}

impl ExistingCommits {
  pub(crate) fn new(base: &str, tips: impl IntoIterator<Item = String>) -> Self {
    Self {
      base: base.to_string(),
      tips: tips.into_iter().collect(),
      commits: OnceLock::new(),
    }
  }

  /// The commits of the branch at `tip` down to `base`, as `read_virtual_commits` of the branch alone reads them:
  /// first parents from the tip until `base` or the base of the sync
  fn of_branch(&self, git_executor: &GitCommandExecutor, repository_path: &str, tip: &str, base: &str) -> Result<HashMap<String, VirtualCommit>> {
    let all = self.commits.get_or_init(|| {
      let tips: Vec<&str> = self.tips.iter().map(String::as_str).collect();
      read_virtual_commits(git_executor, repository_path, &tips, &self.base)
        .inspect_err(|e| warn!(error = %e, "Failed to get the existing virtual commits"))
        .ok()
    });
    let all = all.as_ref().ok_or_else(|| anyhow::anyhow!("the existing virtual commits could not be read"))?;
    let mut commits = HashMap::new();
    let mut current = tip;
    while current != base
      && let Some(commit) = all.get(current)
    {
      commits.insert(current.to_string(), commit.clone());
      current = &commit.parent;
    }
    Ok(commits)
  }
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
  /// The tip the sync listed (`existing_tips`), `None` for a new branch: the ref moves only from it
  old_tip: Option<String>,
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
  Built(Box<BuiltBranch>),
  /// A commit could not be copied (`Conflict` or `Error`); its events are sent and the branch is left as it was. A
  /// conflict names what its conflict ref keeps
  Failed(BlockReason, Option<KeptChain>),
}

/// What the conflict ref of a conflicting branch keeps (`conflict_chain`)
pub(crate) struct KeptChain {
  pub full_branch_name: String,
  /// The copy of the last commit before the conflict; `None` when no ref is needed: the conflict is at the first commit,
  /// or that copy is on the virtual branch already
  pub last_copy: Option<String>,
}

/// Result of processing a single commit
enum CommitProcessingResult {
  Success {
    new_commit_hash: String,
    sync_status: CommitSyncStatus,
    mapping_info: Option<MappingNote>,
    /// Boxed: the summary would make the variant much larger than the other one (`large_enum_variant`)
    resolved: Option<Box<WordResolutionSummary>>,
  },
  BranchError(BlockReason),
}

/// Rebuilds one virtual branch and returns the `original -> new` mapping notes of the commits it created; the caller
/// writes the notes. The sync runs the two steps as separate tasks.
#[cfg(test)]
pub(crate) fn process_single_branch<P: ProgressReporter>(ctx: &BranchSyncContext<P>, job: BranchJob) -> Result<Vec<MappingNote>> {
  match build_branch(ctx, job)? {
    BuildOutcome::Built(built) => finish_branch(ctx, *built).map(|notes| notes.mappings),
    BuildOutcome::Failed(..) => Ok(Vec::new()),
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
    existing_commits,
    verify_mapping_notes,
    owners,
    conflict_chains,
    conflict_tips,
    resolve,
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
    let read = match existing_commits {
      Some(existing_commits) => existing_commits.of_branch(git_executor, repository_path, existing_tip, &base_commit),
      None => read_virtual_commits(git_executor, repository_path, &[existing_tip.as_str()], &base_commit),
    };
    match read {
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

  // the copies before the conflict an earlier sync remembered, while they are kept: by the conflict ref, or on the branch
  let remembered = conflict_chains
    .as_ref()
    .and_then(|memo| memo.get(&full_branch_name, &base_commit))
    .filter(|chain| chain.starts(&commits))
    .filter(|chain| {
      chain.last().is_none_or(|last| {
        conflict_tips.get(&conflict_ref(&full_branch_name)).is_some_and(|tip| tip == last) || existing_virtual_commits.as_ref().is_some_and(|commits| commits.contains_key(last))
      })
    });
  let mut copies: Vec<(String, String)> = Vec::new();
  let mut resolutions: Vec<Option<NoteResolution>> = Vec::new();

  let mut current_parent_hash = base_commit.clone();
  // a copy is never older than the commits below it, and its date depends only on the originals and the base
  let mut committer_timestamp = tree_id_cache.get_committer_timestamp(git_executor, repository_path, &base_commit)?;
  let mut last_commit_hash = String::new();
  let mut is_any_commit_changed = false;
  let mut pending_notes: Vec<MappingNote> = Vec::new();
  // one merge-tree and one fast-import process for all commits of the branch
  let chain = RefCell::new(CommitChain::new(git_executor, repository_path));

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
      chain: &chain,
      existing_virtual_commits: existing_virtual_commits.as_ref(),
      verify_notes: *verify_mapping_notes,
      committer_timestamp,
      remembered: remembered.as_ref().and_then(|chain| {
        chain.copies.get(current_commit_idx).map(|(id, tree)| RememberedCopy {
          id,
          tree,
          resolved: chain.resolutions.get(current_commit_idx).copied().flatten(),
        })
      }),
      resolve: *resolve,
    };

    match process_single_commit(
      commit_params,
      &branch_name,
      &commit.id,
      &commits[current_commit_idx + 1..],
      progress,
      &progress_info,
      owners,
    )? {
      CommitProcessingResult::Success {
        new_commit_hash,
        sync_status,
        mapping_info,
        resolved,
      } => {
        if sync_status == CommitSyncStatus::Created {
          is_any_commit_changed = true;
        }
        // the note of a created commit, or the source a reused commit's older note lacked
        pending_notes.extend(mapping_info);
        // every path to a copy leaves its tree in the cache
        let tree = tree_id_cache.get_tree_id(git_executor, repository_path, &new_commit_hash)?;
        copies.push((new_commit_hash.clone(), tree));
        resolutions.push(resolved.as_deref().map(note_resolution));

        current_parent_hash.clone_from(&new_commit_hash);
        last_commit_hash = new_commit_hash;
      }
      CommitProcessingResult::BranchError(reason) => {
        // Error already handled and events sent by process_single_commit
        if reason != BlockReason::Conflict {
          return Ok(BuildOutcome::Failed(reason, None));
        }
        let chain = ConflictChain {
          originals: commits[..current_commit_idx].iter().map(|commit| commit.id.clone()).collect(),
          copies,
          resolutions,
        };
        let last_copy = chain
          .last()
          .filter(|last| !existing_virtual_commits.as_ref().is_some_and(|commits| commits.contains_key(*last)))
          .map(str::to_string);
        if let Some(memo) = conflict_chains
          && remembered.as_ref() != Some(&chain)
        {
          memo.remember(&full_branch_name, &base_commit, chain);
        }
        return Ok(BuildOutcome::Failed(reason, Some(KeptChain { full_branch_name, last_copy })));
      }
    }
  }
  // the branches built on this one read its commits
  chain.into_inner().finish()?;
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
  Ok(BuildOutcome::Built(Box::new(BuiltBranch {
    stack_note,
    head: last_commit_hash,
    branch_name,
    full_branch_name,
    old_tip: existing_tip.cloned(),
    status,
    notes: pending_notes,
    my_email,
    parent_head,
  })))
}

/// The notes a branch leaves for the sync to write, and where its ref is after its finish
pub(crate) struct BranchNotes {
  /// The mapping notes of the created commits, and of the reused ones whose note had no source
  pub mappings: Vec<MappingNote>,
  /// `(tip, base)` of a stacked branch, see `BuiltBranch::stack_note`
  pub stack_note: Option<(String, String)>,
  /// The full name of the branch and its tip: the head the sync built, or where another process moved it (`None`:
  /// deleted it)
  pub tip: (String, Option<String>),
  /// Another process moved the branch while the sync built it: its tip is not this sync's build
  pub moved_elsewhere: bool,
}

/// Moves the branch to its new head if it changed, reports its final status and its remote status, and returns the
/// notes to write. The ref moves only from the tip the sync listed (compare-and-swap): the app's sync and `bd sync` don't
/// share the repository lock. A ref another process moved to the same head (the copies are deterministic) counts as
/// written; one it moved elsewhere keeps that tip, and the branch reports an error, without notes, for the next sync to
/// rebuild. Any other failure of the move (a ref of the name is in the way) fails the branch's task
#[instrument(skip(ctx, built), fields(branch_name = %built.branch_name))]
pub(crate) fn finish_branch<P: ProgressReporter>(ctx: &BranchSyncContext<P>, built: BuiltBranch) -> Result<BranchNotes> {
  let BuiltBranch {
    head,
    branch_name,
    full_branch_name,
    old_tip,
    status,
    notes,
    stack_note,
    my_email,
    parent_head,
  } = built;
  let git_executor = &ctx.git_executor;
  let repository_path = &ctx.repository_path;

  // only update the branch if it's new or changed
  if status != BranchSyncStatus::Unchanged
    && let BranchMove::MovedElsewhere(moved) = move_branch(git_executor, repository_path, &full_branch_name, &head, old_tip.as_deref())?
  {
    warn!(branch = %full_branch_name, expected = ?old_tip, actual = ?moved, "another process moved the virtual branch during the sync");
    let _ = ctx.progress.send(SyncEvent::BranchStatusUpdate {
      branch_name,
      status: BranchSyncStatus::Error,
      error: Some(BranchError::Generic(match &moved {
        Some(tip) => format!(
          "Not updated: another process moved {full_branch_name} to {} while this sync built it; sync again",
          short_id(tip)
        ),
        None => format!("Not updated: another process deleted {full_branch_name} while this sync built it; sync again"),
      })),
    });
    return Ok(BranchNotes {
      mappings: Vec::new(),
      stack_note: None,
      tip: (full_branch_name, moved),
      moved_elsewhere: true,
    });
  }

  debug!(name = %branch_name, status = ?status, "Sending final branch status");
  let _ = ctx.progress.send(SyncEvent::BranchStatusUpdate {
    branch_name: branch_name.clone(),
    status,
    error: None,
  });

  let parent = parent_head.as_deref().map(|base| StackParent {
    base,
    ancestors: ctx.ancestor_keys.get(&branch_name).map_or(AncestorCommits::ReadWhenNeeded, AncestorCommits::Known),
  });
  let remote_status = match &ctx.remote_statuses {
    // an unchanged branch whose remote and baseline did not move has the status of the previous sync, or of the object
    // cache
    Some(statuses) => statuses.status(&full_branch_name, &branch_name, &head, my_email.as_deref(), parent),
    None => compute_remote_status_for_branch(
      git_executor,
      repository_path,
      "origin",
      &full_branch_name,
      &branch_name,
      my_email.as_deref(),
      &ctx.baseline_branch,
      parent,
    ),
  };
  match remote_status {
    Ok(remote_status) => {
      let _ = ctx.progress.send(SyncEvent::RemoteStatusUpdate(remote_status));
    }
    // the branch keeps the remote status it had: the sync is not failed for it, the next sync computes it again
    Err(e) => warn!(branch = %branch_name, error = ?e, "Cannot compute the remote status"),
  }

  Ok(BranchNotes {
    mappings: notes,
    stack_note,
    tip: (full_branch_name, Some(head)),
    moved_elsewhere: false,
  })
}

/// How the move of a virtual branch ended
pub(crate) enum BranchMove {
  /// The ref is at the new head: moved by this sync, or by another one to the same copy
  Written,
  /// Another process moved the ref to this tip (`None`: deleted it) since the sync listed it
  MovedElsewhere(Option<String>),
}

/// Moves `refs/heads/<full_branch_name>` from `old_tip` (`None`: no such ref) to `head` in one CAS transaction. A
/// failed transaction whose ref is at neither `head` nor `old_tip` any more lost to another writer; any other failure is
/// not about the old value and is returned
pub(crate) fn move_branch(git_executor: &GitCommandExecutor, repository_path: &str, full_branch_name: &str, head: &str, old_tip: Option<&str>) -> Result<BranchMove> {
  let reference = format!("refs/heads/{full_branch_name}");
  let mut transaction = RefTransaction::new();
  match old_tip {
    Some(old_tip) => transaction.update(&reference, head, Some(old_tip)),
    None => transaction.create(&reference, head),
  };
  let Err(error) = transaction.commit(git_executor, repository_path, "sync") else {
    return Ok(BranchMove::Written);
  };
  let actual = resolve_commit(git_executor, repository_path, &reference)?;
  match actual.as_deref() {
    Some(actual) if actual == head => Ok(BranchMove::Written),
    actual if actual == old_tip => Err(error),
    _ => Ok(BranchMove::MovedElsewhere(actual)),
  }
}

#[instrument(
  skip(commit_params, remaining_commits, progress, progress_info, owners),
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
  owners: &CommitOwners,
) -> Result<CommitProcessingResult> {
  let result = create_or_update_commit(commit_params);

  match result {
    Ok(CopiedCommit {
      hash: new_commit_hash,
      status: sync_status,
      note: mapping_info,
      resolved,
    }) => {
      // Send success event with status
      let _ = progress.send(SyncEvent::CommitSynced {
        branch_name: branch_name.to_string(),
        commit_hash: original_hash.to_string(),
        new_hash: new_commit_hash.clone(),
        status: sync_status,
        resolved: resolved.clone(),
      });

      Ok(CommitProcessingResult::Success {
        new_commit_hash,
        sync_status,
        mapping_info,
        resolved: resolved.map(Box::new),
      })
    }
    Err(CopyCommitError::BranchError(mut branch_error)) => {
      owners.refine(branch_name, &mut branch_error);
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
