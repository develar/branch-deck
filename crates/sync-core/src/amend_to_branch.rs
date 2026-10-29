//! The app's amend of uncommitted changes into a commit of the checked out branch (`git_ops::amend_operations`): the
//! amended commit and its replayed descendants are computed before the repository lock, which holds only the witness of
//! HEAD and the move; a refusal of the rewrite is a `BranchError` that the app shows.

use anyhow::Result;
use domain_types::rewrite::{AmendCommandResult, AmendResult};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::amend_operations::{AmendToCommitParams, apply_amend, prepare_amend};
use git_ops::copy_commit::CopyCommitError;
use git_ops::history::History;
use git_ops::revs::with_dirs_of;
use git_ops::rewrite_plan::PreparedRewrite;
use history_rewrite::repository_lock::RewriteOptions;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AmendUncommittedToBranchParams {
  pub repository_path: String,
  pub branch_name: String,
  pub original_commit_id: String,
  pub files: Vec<String>,
}

/// Amends uncommitted changes into a commit of the checked out branch (`amend_to_commit_in_main`). The history, the
/// merge, the amended commit and the replay of its descendants come before the repository lock, on the HEAD `options`
/// expects, checked before the commit is looked for; under it only the witness of HEAD and the compare-and-swap move of
/// the branch, the amended files staged after it (`RewriteOptions::lock_prepared`). A refusal of the rewrite is a
/// `BranchError`; the lock not taken and another HEAD are errors of their own type (`RepositoryLocked`,
/// `LockWaitCanceled`, `HeadMoved`)
pub fn amend_uncommitted_to_branch_core(git_executor: &GitCommandExecutor, params: AmendUncommittedToBranchParams, options: &RewriteOptions) -> Result<AmendCommandResult> {
  let AmendUncommittedToBranchParams {
    repository_path,
    branch_name: _,
    original_commit_id,
    files,
  } = params;
  let repo = repository_path.as_str();
  // the guards and the lock take the directories read here
  let git_executor = &with_dirs_of(git_executor, repo)?;
  // the HEAD of the history is the one `options` expects before the amended commit is looked for on it: a HEAD another
  // process rewrote has a copy of that commit, not the commit, and the refusal is `HeadMoved`, which the engine
  // resolves again on that HEAD
  let prepare = || -> Result<PreparedRewrite, CopyCommitError> {
    let history = History::read_to(git_executor, repo, &[original_commit_id.as_str()]).map_err(|error| failed(error.into()))?;
    options
      .ensure_head(git_executor, repo, history.head().unwrap_or_default())
      .map_err(|moved| CopyCommitError::Other(moved.into()))?;
    let amend = AmendToCommitParams {
      original_commit_id: original_commit_id.clone(),
      files: files.clone(),
    };
    prepare_amend(git_executor, repo, &history, amend).map_err(failed)
  };
  // under the lock, once its witness missed: a HEAD other than the expected one is refused by one `rev-parse`, before
  // the history is read again, which on a history another process rewrote goes down to its root
  let again = || {
    options.expected_full_head(git_executor, repo)?;
    prepare()
  };
  let locked = options.lock_prepared(git_executor, repo, prepare(), again);
  let applied = locked.and_then(|(prepared, _lock)| apply_amend(git_executor, repo, &prepared, &original_commit_id).map_err(failed));
  match applied {
    Ok(result) => Ok(AmendCommandResult::Ok(AmendResult {
      amended_commit_id: result.amended_commit_id,
      rebased_to_commit: result.rebased_to_commit,
    })),
    // a conflict opens the conflict view
    Err(CopyCommitError::BranchError(branch_error)) => Ok(AmendCommandResult::BranchError(branch_error)),
    Err(CopyCommitError::Other(error)) => Err(error),
  }
}

/// A failure of the amend itself, not of the lock or of the expected HEAD, named as one
fn failed(error: CopyCommitError) -> CopyCommitError {
  match error {
    CopyCommitError::Other(error) => CopyCommitError::Other(error.context("Failed to amend commit")),
    refusal @ CopyCommitError::BranchError(_) => refusal,
  }
}
