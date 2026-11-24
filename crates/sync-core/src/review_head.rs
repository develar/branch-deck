//! The branches of a shown review on the remote (docs/design/git-operations.md, "Review head"; docs/design/landing.md,
//! "Show: a review after landing"): `{prefix}/review/<branch>/head`, the commits it shows, and
//! `{prefix}/review/<branch>/base`, their fork point on the target, never master, so its pull request can never land.
//! Neither is a local branch, nor an input of a sync (`BranchKind::Review`).
//!
//! - `push_review`: both go to `tip` and its fork point in one `push --atomic`, all or nothing, each leased on what the
//!   remote has (`ls-remote`): a new review creates both (an empty lease, which a repeat of the same push answers
//!   `[up to date]`), a review opened after its robot merged moves them to what the robot landed. One `ls-remote`, one
//!   `merge-base` and one push; no push when both are where they go.
//! - `extend_review`: the fixes of the branch that landed after the review opened go onto its head: the commits of each
//!   merged Safe Push past the target (`<source> ^<target>`, what its robot tested and landed) replayed onto the head the
//!   remote has (`git replay`, which writes objects only and drops a commit whose change the head has: a repeat after
//!   a crash adds nothing), signed per `commit.gpgsign`, and pushed leased on that head. A fix that doesn't apply stops
//!   the replay there; the head keeps what applied before it. One `ls-remote`, one `cat-file --batch-check` (a fetch of
//!   the head only when it is not local), one `replay` per fix, one push when the head moved; with signing the
//!   `rev-list`, `cat-file` and `fast-import` of `sign_range`.
//! - `delete_review`: the ones the remote has are deleted in one push, each leased on what it has: a leased delete of a
//!   ref that is gone is `[rejected] (stale info)`, which would fail the atomic push whole. None left is done.

use crate::push_lease::{LeaseSource, PushOptions, PushRef, push_refs};
use anyhow::{Context, Result, anyhow, bail};
use domain_types::branch_names::review_branch_names;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cat_file_batch::object_types;
use git_ops::commit_signing::{Signing, sign_range};
use git_ops::remote_command::{execute_remote_command, execute_remote_command_accepting};
use git_ops::replay::replay_range;
use git_ops::revs::merge_base;
use std::collections::HashMap;
use tracing::instrument;

/// What the remote has of the branches of a shown review: the tip of each, `None` for one it lacks
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteReview {
  pub head: Option<String>,
  pub base: Option<String>,
}

/// The tips of `names` (full branch names) on `remote`, by one `ls-remote`; a branch it lacks is not in the map
fn remote_tips(git: &GitCommandExecutor, repository_path: &str, remote: &str, names: &[&str]) -> Result<HashMap<String, String>> {
  let patterns: Vec<String> = names.iter().map(|name| format!("refs/heads/{name}")).collect();
  let mut args = vec!["ls-remote", "--exit-code", remote];
  args.extend(patterns.iter().map(String::as_str));
  // 2: the remote has none of them
  let output = execute_remote_command_accepting(git, &args, repository_path, &[2])?;
  Ok(
    output
      .lines()
      .filter_map(|line| line.split_once('\t'))
      .filter_map(|(id, reference)| Some((reference.strip_prefix("refs/heads/")?.to_owned(), id.to_owned())))
      .filter(|(name, _)| names.contains(&name.as_str()))
      .collect(),
  )
}

/// What the remote `remote` has of the branches of the shown review of `branch` under `prefix` (one `ls-remote`)
///
/// # Errors
/// A branch name no ref takes, a failed `ls-remote`
pub fn read_remote_review(git: &GitCommandExecutor, repository_path: &str, remote: &str, prefix: &str, branch: &str) -> Result<RemoteReview> {
  let (head, base) = review_branch_names(prefix, branch)?;
  let mut tips = remote_tips(git, repository_path, remote, &[&head, &base])?;
  Ok(RemoteReview {
    head: tips.remove(&head),
    base: tips.remove(&base),
  })
}

/// Where `push_review` left the branches of a shown review
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewPush {
  /// `{prefix}/review/<branch>/head`, at the commit it shows
  pub head: String,
  /// `{prefix}/review/<branch>/base`, at the fork point
  pub base: String,
  /// The fork point of the head on the target, which the base names
  pub fork_point: String,
  /// The push went out: a branch was not where it goes
  pub pushed: bool,
}

/// The branches of the shown review of `branch` under `prefix` go to `tip` and to its fork point on `target_ref` (the
/// remote-tracking ref of the target, `refs/remotes/origin/master`) on `remote`, in one atomic push leased on what the
/// remote has (module docs)
///
/// # Errors
/// A branch name no ref takes, a `tip` that shares no history with the target, the failure of a git process, a push the
/// remote refused (a branch moved since the `ls-remote`)
#[instrument(skip(git))]
pub fn push_review(git: &GitCommandExecutor, repository_path: &str, remote: &str, prefix: &str, branch: &str, tip: &str, target_ref: &str) -> Result<ReviewPush> {
  let (head, base) = review_branch_names(prefix, branch)?;
  let remote_review = read_remote_review(git, repository_path, remote, prefix, branch)?;
  let fork_point = merge_base(git, repository_path, tip, target_ref)?.ok_or_else(|| anyhow!("{tip} shares no history with {target_ref}"))?;
  let mut refs = Vec::new();
  for (name, to, lease) in [(&head, tip, remote_review.head), (&base, fork_point.as_str(), remote_review.base)] {
    if lease.as_deref() == Some(to) {
      continue;
    }
    refs.push(PushRef {
      full_name: name.clone(),
      tip: to.to_owned(),
      source: if lease.is_some() { LeaseSource::Expected } else { LeaseSource::Absent },
      lease,
    });
  }
  let pushed = !refs.is_empty();
  if pushed {
    let report = push_refs(git, repository_path, remote, &refs, PushOptions { atomic: true, dry_run: false }, None).context("Cannot push the branches of the shown review")?;
    refused(&report, &refs)?;
  }
  Ok(ReviewPush { head, base, fork_point, pushed })
}

/// Where `extend_review` left the head of a shown review
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewExtension {
  /// The head the remote had, the lease of the push
  pub old_head: String,
  /// The head now: the commits of the fixes that applied on top of `old_head`; `old_head` when none added a change
  pub head: String,
  /// The index in the fixes of the first that doesn't apply on the head (the ones after it were not tried); `None` when
  /// every fix applied
  pub conflict: Option<usize>,
}

/// The fixes a shown review takes (`extend_review`)
#[derive(Debug, Clone, Copy)]
pub struct Fixes<'a> {
  /// The source revisions of the merged Safe Pushes of the fixes, oldest first
  pub sources: &'a [String],
  /// The remote-tracking ref of the target (`refs/remotes/origin/master`): what a robot took past it is its fix
  pub target_ref: &'a str,
  /// `commit.gpgsign` of the repository
  pub signing: Signing,
}

/// The fixes `fixes` of `branch` go onto the head of its shown review under `prefix` on `remote`, pushed leased on the
/// head the remote has (module docs)
///
/// # Errors
/// A branch name no ref takes, no head on the remote (the review was closed), a source this repository lacks, the
/// failure of a git process, a push the remote refused (the head moved since the `ls-remote`)
#[instrument(skip(git))]
pub fn extend_review(git: &GitCommandExecutor, repository_path: &str, remote: &str, prefix: &str, branch: &str, fixes: Fixes<'_>) -> Result<ReviewExtension> {
  let Fixes { sources, target_ref, signing } = fixes;
  let (head_name, _) = review_branch_names(prefix, branch)?;
  let old_head = read_remote_review(git, repository_path, remote, prefix, branch)?
    .head
    .ok_or_else(|| anyhow!("The remote has no {head_name}: the shown review was closed"))?;
  let mut wanted: Vec<&str> = vec![old_head.as_str()];
  wanted.extend(sources.iter().map(String::as_str));
  let types = object_types(git, repository_path, &wanted)?;
  if let Some(missing) = sources.iter().zip(&types[1..]).find(|(_, kind)| **kind != Some("commit")).map(|(source, _)| source) {
    bail!("This repository lacks {missing}, the commit the Safe Push of the fix of {branch} landed");
  }
  if types[0] != Some("commit") {
    fetch_head(git, repository_path, remote, &head_name)?;
  }
  let exclusion = format!("^{target_ref}");
  let mut head = old_head.clone();
  let mut conflict = None;
  for (index, source) in sources.iter().enumerate() {
    let replayed = replay_range(git, repository_path, &head, &[source, &exclusion]).map_err(|error| anyhow!("Cannot replay {source} onto {head}: {error}"))?;
    let Some(tip) = replayed else {
      conflict = Some(index);
      break;
    };
    head = tip;
  }
  if head != old_head {
    let signed = sign_range(git, repository_path, Some(&old_head), &head, signing)?;
    if let Some(tip) = signed.as_ref().and_then(|signed| signed.ids.last()) {
      head.clone_from(tip);
    }
    let refs = [PushRef {
      full_name: head_name,
      tip: head.clone(),
      lease: Some(old_head.clone()),
      source: LeaseSource::Expected,
    }];
    let pushed = push_refs(git, repository_path, remote, &refs, PushOptions { atomic: true, dry_run: false }, None).context("Cannot push the head of the shown review");
    if let Some(signed) = &signed {
      signed.discard(git, repository_path);
    }
    refused(&pushed?, &refs)?;
  }
  Ok(ReviewExtension { old_head, head, conflict })
}

/// The commits of the head `head_name` of a shown review from `remote`, which this repository lacks (the objects only:
/// no ref, no `FETCH_HEAD`)
fn fetch_head(git: &GitCommandExecutor, repository_path: &str, remote: &str, head_name: &str) -> Result<()> {
  let refspec = format!("refs/heads/{head_name}");
  let args = [
    "fetch",
    "--no-tags",
    "--no-write-fetch-head",
    "--no-recurse-submodules",
    "--no-auto-maintenance",
    "--no-prune",
    "--refmap=",
    "--end-of-options",
    remote,
    refspec.as_str(),
  ];
  execute_remote_command(git, &args, repository_path).with_context(|| format!("Cannot fetch {head_name}"))?;
  Ok(())
}

/// Deletes the branches of the shown review of `branch` under `prefix` that `remote` has, each leased on what it has
/// (module docs); the names of those it deleted
///
/// # Errors
/// A branch name no ref takes, the failure of a git process, a delete the remote refused
#[instrument(skip(git))]
pub fn delete_review(git: &GitCommandExecutor, repository_path: &str, remote: &str, prefix: &str, branch: &str) -> Result<Vec<String>> {
  let (head, base) = review_branch_names(prefix, branch)?;
  let remote_review = read_remote_review(git, repository_path, remote, prefix, branch)?;
  let refs: Vec<PushRef> = [(head, remote_review.head), (base, remote_review.base)]
    .into_iter()
    .filter_map(|(name, tip)| {
      // an empty tip: `:refs/heads/<name>`, the delete
      tip.map(|tip| PushRef {
        full_name: name,
        tip: String::new(),
        lease: Some(tip),
        source: LeaseSource::Expected,
      })
    })
    .collect();
  if refs.is_empty() {
    return Ok(Vec::new());
  }
  let report = push_refs(git, repository_path, remote, &refs, PushOptions { atomic: true, dry_run: false }, None).context("Cannot delete the branches of the shown review")?;
  refused(&report, &refs)?;
  Ok(refs.into_iter().map(|reference| reference.full_name).collect())
}

/// The error of a push whose ref the remote refused, with what it said
fn refused(report: &crate::push_lease::PushRefsReport, refs: &[PushRef]) -> Result<()> {
  let rejected: Vec<String> = refs
    .iter()
    .filter_map(|reference| {
      let status = report.refs.get(&format!("refs/heads/{}", reference.full_name));
      match status {
        Some(status) if status.accepted() => None,
        Some(status) => Some(format!("{} {}", reference.full_name, status.summary)),
        None => Some(format!("{} not pushed", reference.full_name)),
      }
    })
    .collect();
  if rejected.is_empty() {
    return Ok(());
  }
  let said = if report.messages.is_empty() {
    String::new()
  } else {
    format!(": {}", report.messages.join("; "))
  };
  bail!("The remote refused the branches of the shown review ({}){said}", rejected.join(", "))
}

#[cfg(test)]
#[path = "review_head_test.rs"]
mod review_head_test;
