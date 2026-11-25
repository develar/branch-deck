//! Whether branches that moved since their push are the pushed commits rebuilt onto another base (the sync of an Update
//! Project rebuilt them onto the new baseline): the same changes in the same order, each with the same version (author,
//! author time and message), so the remote branch is up to date with them as the remote status tells it
//! (docs/design/git-operations.md, "Push and land"). One `rev-list` for all of them.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;

/// A branch that moved since its push: the commit it pushed and its tip now
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedBranch {
  pub pushed: String,
  pub now: String,
}

/// Whether each of `moved` is its pushed commits rebuilt onto another base, above `baseline`: one
/// `rev-list --left-right --cherry-mark` of the last of them (the leaf, whose chain holds the branches it is built on, as
/// a push sends parents first), the commits of each side told by the chain of first parents from its tip, as `=` takes
/// the place of the side mark. The two chains have the same length, every commit of them a patch-equivalent copy (`=`)
/// with the version of the commit at the same place on the other side, and every other branch is at the same place of
/// both chains. A reorder of two commits of the same version is not told apart. Nothing moved is no rebuild
///
/// # Errors
/// The errors of the walk
pub fn rebuilt_with_the_same_commits(git_executor: &GitCommandExecutor, repository_path: &str, baseline: &str, moved: &[MovedBranch]) -> Result<bool> {
  let Some(leaf) = moved.last() else {
    return Ok(false);
  };
  let range = format!("{}...{}", leaf.pushed, leaf.now);
  let exclude = format!("^{baseline}");
  let walked = walk(git_executor, repository_path, &[range.as_str(), exclude.as_str()])?;
  let by_id: HashMap<&str, &RebuiltCommit> = walked.iter().map(|commit| (commit.id.as_str(), commit)).collect();
  let (pushed, now) = (chain(&by_id, &leaf.pushed), chain(&by_id, &leaf.now));
  if pushed.is_empty() || pushed.len() != now.len() {
    return Ok(false);
  }
  let copies = pushed.iter().zip(&now).all(|(old, new)| old.mark == '=' && new.mark == '=' && old.version == new.version);
  let place = |chain: &[&RebuiltCommit], tip: &str| chain.iter().position(|commit| commit.id == tip);
  let in_place = moved[..moved.len() - 1]
    .iter()
    .all(|branch| place(&pushed, &branch.pushed).is_some_and(|at| place(&now, &branch.now) == Some(at)));
  Ok(copies && in_place)
}

/// `%m`, the commit and its parents, then what a copy keeps (`CommitVersion`), the message last; each commit ended by
/// `\x01`, which no message holds
const FORMAT: &str = "--format=%m%x00%H%x00%P%x00%an%x00%ae%x00%at%x00%B%x01";

/// A commit of the walk: its mark (`=` for a patch-equivalent copy of one of the other side), its first parent and its
/// version
#[derive(Debug)]
struct RebuiltCommit {
  mark: char,
  id: String,
  parent: Option<String>,
  version: CommitVersion,
}

/// What a copy keeps and a reword or another author changes, as the remote status compares them: the author, the author
/// time and the raw message
#[derive(Debug, PartialEq, Eq)]
struct CommitVersion {
  author: String,
  email: String,
  time: String,
  message: String,
}

/// The commits of `rev-list --left-right --cherry-mark <revs>`
fn walk(git_executor: &GitCommandExecutor, repository_path: &str, revs: &[&str]) -> Result<Vec<RebuiltCommit>> {
  let args = crate::landed_pairing::walk_args(FORMAT, &["--left-right", "--cherry-mark"], revs);
  let output = git_executor.cmd(&args, repository_path).bytes()?;
  Ok(crate::landed_pairing::walk_records(&String::from_utf8_lossy(&output)).filter_map(parse).collect())
}

/// A commit of `FORMAT`
fn parse(record: &str) -> Option<RebuiltCommit> {
  let mut fields = record.splitn(7, '\0');
  let (mark, id, parents) = (fields.next()?.chars().next()?, fields.next()?, fields.next()?);
  let (author, email, time, message) = (fields.next()?, fields.next()?, fields.next()?, fields.next()?);
  Some(RebuiltCommit {
    mark,
    id: id.to_owned(),
    parent: parents.split(' ').next().filter(|parent| !parent.is_empty()).map(str::to_owned),
    version: CommitVersion {
      author: author.to_owned(),
      email: email.to_owned(),
      time: time.to_owned(),
      message: message.to_owned(),
    },
  })
}

/// The commits of the walk from `tip` down its first parents while the walk has them, oldest first
fn chain<'a>(by_id: &HashMap<&str, &'a RebuiltCommit>, tip: &str) -> Vec<&'a RebuiltCommit> {
  let mut chain = Vec::new();
  let mut next = Some(tip);
  while let Some(commit) = next.and_then(|id| by_id.get(id)) {
    chain.push(*commit);
    next = commit.parent.as_deref();
  }
  chain.reverse();
  chain
}

#[cfg(test)]
#[path = "rebuilt_branch_test.rs"]
mod rebuilt_branch_test;
