//! The fetch of the remote branches someone else pushed to (spec/repository/outgoing.spec.md, "Remote commits of
//! others"; docs/design/remote-status.md, "Remote tip"): a remote status compares with the remote-tracking ref, which
//! moves only by a fetch, and the virtual branches are fetched only by a push the remote refused. A pull request shows
//! the head of its branch on GitHub, so the session asks this fetch for a branch whose open pull request shows a head
//! that is not the tip of its remote-tracking ref (`domain_policy::refresh::moved_remote_fetch_due`).
//!
//! One `for-each-ref` reads the tips, one `cat-file --batch-check` tells which heads are here; a head the tip has (a
//! pull request GitHub still shows at an older head after a push of the user) fetches nothing, after one
//! `merge-base --is-ancestor`. The rest is one `fetch_remote_branches` of all of them, then the remote status of each
//! against the tip it fetched. The lease of a push is the head of the status the user saw, so this fetch never makes a
//! push overwrite what the user didn't see: the status it leaves lists the fetched commits as the ones a push drops.

use crate::push_lease::fetch_remote_branches;
use crate::remote_status::{AncestorCommits, StackParent, StatusBranch, compute_remote_statuses};
use anyhow::Result;
use domain_types::RemoteStatusUpdate;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::object_types;
use git_ops::revs::{is_ancestor, ref_tips};
use tracing::{info, instrument};

/// A virtual branch whose pull request shows `head`
#[derive(Debug, Clone, Copy)]
pub struct MovedBranch<'a> {
  /// `{prefix}/virtual/<name>`
  pub full_name: &'a str,
  pub branch_name: &'a str,
  /// The full name of the branch it is built on (`BranchStack::effective_base`)
  pub parent: Option<&'a str>,
  /// The head its pull request shows
  pub head: &'a str,
  /// The author whose commits are the user's (`GroupedBranchInfo::my_email`)
  pub my_email: Option<&'a str>,
}

/// What the fetch of a branch found
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedFetch {
  /// The remote status against the tip it fetched; `None` when it fetched nothing (the tip has the head already)
  pub status: Option<RemoteStatusUpdate>,
  /// The head is the tip of the remote-tracking ref now or an ancestor of it: nothing more to fetch for it. Not settled:
  /// the remote doesn't have the head GitHub shows yet (a mirror that lags)
  pub settled: bool,
}

/// Fetches those of `branches` from `remote` whose remote-tracking ref lacks the head of their pull request, and answers
/// for each, in their order, the status against what it fetched
///
/// # Errors
/// A git that fails, the fetch of an unreachable remote too
#[instrument(skip_all, fields(remote = %remote, branches = branches.len()))]
pub fn fetch_moved_remotes(git_executor: &GitCommandExecutor, repository_path: &str, remote: &str, baseline: &str, branches: &[MovedBranch<'_>]) -> Result<Vec<MovedFetch>> {
  let remote_refs: Vec<String> = branches.iter().map(|branch| format!("refs/remotes/{remote}/{}", branch.full_name)).collect();
  let patterns: Vec<&str> = remote_refs.iter().map(String::as_str).collect();
  let tips = ref_tips(git_executor, repository_path, &patterns)?;
  let heads: Vec<&str> = branches.iter().map(|branch| branch.head).collect();
  let present = object_types(git_executor, repository_path, &heads)?;
  let mut has_head = Vec::with_capacity(branches.len());
  for ((branch, remote_ref), kind) in branches.iter().zip(&remote_refs).zip(&present) {
    let has = match tips.get(remote_ref) {
      Some(tip) if tip == branch.head => true,
      Some(tip) if *kind == Some("commit") => is_ancestor(git_executor, repository_path, branch.head, tip)?,
      _ => false,
    };
    has_head.push(has);
  }

  let fetched: Vec<&MovedBranch<'_>> = branches.iter().zip(&has_head).filter(|(_, has)| !**has).map(|(branch, _)| branch).collect();
  if fetched.is_empty() {
    return Ok(branches.iter().map(|_| MovedFetch { status: None, settled: true }).collect());
  }
  let names: Vec<String> = fetched.iter().map(|branch| branch.full_name.to_string()).collect();
  info!(branches = ?names, "the pull requests show heads the remote-tracking refs lack: fetching them");
  fetch_remote_branches(git_executor, repository_path, remote, &names)?;

  branches
    .iter()
    .zip(has_head)
    .map(|(branch, has)| {
      if has {
        return Ok(MovedFetch { status: None, settled: true });
      }
      let parent = branch.parent.map(|base| StackParent {
        base,
        ancestors: AncestorCommits::ReadWhenNeeded,
      });
      let status_branch = StatusBranch {
        local_ref: branch.full_name,
        branch_name: branch.branch_name,
        parent,
      };
      let status = compute_remote_statuses(git_executor, repository_path, remote, &[status_branch], branch.my_email, baseline)?.remove(0);
      let settled = match status.remote_tip.as_deref() {
        Some(tip) if tip == branch.head => true,
        Some(tip) => object_types(git_executor, repository_path, &[branch.head])?[0] == Some("commit") && is_ancestor(git_executor, repository_path, branch.head, tip)?,
        None => false,
      };
      Ok(MovedFetch { status: Some(status), settled })
    })
    .collect()
}
