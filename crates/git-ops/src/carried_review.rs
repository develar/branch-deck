//! The reviewed change of a branch carried onto its base now (spec/branch/review-plan.spec.md, "Since Your Review"): the
//! diff from the reviewed base to the reviewed tip merged onto the base, as a commit on no ref whose parent is the base.
//! The diff from it to the tip is what changed since the review, whatever the branch went through: a rebase, an amend,
//! a squash, a split, a reorder, a dropped commit. One `merge-tree --stdin` and one `fast-import` (`CommitChain`), none
//! when the base did not move: the reviewed tip is then the carried commit. A conflict carries nothing.

use crate::commit_chain::{ChainCommit, CommitChain};
use crate::merge_memo::MergeOutcome;
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{debug, instrument};

/// Who the carried commit names as its author and committer, at what time: the reviewed tip's author, so that the same
/// carry writes the same commit
#[derive(Debug, Clone, Copy)]
pub struct CarryIdentity<'a> {
  pub name: &'a str,
  pub email: &'a str,
  pub time: u32,
}

/// The commit whose tree is the change of `reviewed_base..reviewed_tip` merged onto `base`, with `base` as its parent;
/// `reviewed_tip` itself when `base` is `reviewed_base`; `None` when the merge conflicts
#[instrument(skip(git, identity))]
pub fn carry_reviewed(git: &GitCommandExecutor, repository_path: &str, reviewed_base: &str, reviewed_tip: &str, base: &str, identity: CarryIdentity<'_>) -> Result<Option<String>> {
  if reviewed_base == base {
    return Ok(Some(reviewed_tip.to_string()));
  }
  let mut chain = CommitChain::new(git, repository_path);
  let tree = match chain.merge(reviewed_base, base, reviewed_tip)? {
    MergeOutcome::Clean { tree } => tree,
    MergeOutcome::Conflict { files } => {
      debug!(conflicts = files.len(), "the reviewed change does not carry onto the base");
      chain.finish()?;
      return Ok(None);
    }
  };
  let message = format!("The review of {reviewed_tip} on {base}");
  let carried = chain.write_commit(&ChainCommit {
    tree: &tree,
    parent: base,
    author_name: identity.name,
    author_email: identity.email,
    author_timestamp: identity.time,
    author_timezone: "",
    committer_name: identity.name,
    committer_email: identity.email,
    committer_timestamp: identity.time,
    committer_timezone: "",
    message: &message,
    resolution: None,
  })?;
  chain.finish()?;
  Ok(Some(carried))
}
