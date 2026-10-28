//! The first-parent history of HEAD that a rewrite of the checked out branch works on: newest first, each commit with
//! its tree and parents, read with one `git log` (docs/design/git-operations.md, "Rewrite history"). Every rewrite reads
//! it through one of two bounds, so a long history (a monorepo has 10⁵–10⁶ commits) is never read whole:
//!
//! - `History::read_to` streams the history and stops as soon as it has every commit the rewrite changes and the
//!   parent of the oldest one: amend, drop, squash and squash fixups, whose requests name no baseline;
//! - `History::read_ahead_of` reads the commits ahead of the baseline and the boundary below them, with their mapping
//!   notes if asked: reword and the move of a hunk, which never rewrite a commit of the baseline.
//!
//! The parents also give the merge guard of the rewrite without another process (`History::check_rewrite`).

use crate::commit_list::is_on_local_baseline;
use crate::notes::mapping_lines;
use crate::revs::{checked_out_head, short_id};
use crate::rewrite_guard::{RewriteTarget, ensure_no_operation_in_progress, merge_refusal};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use tracing::{debug, instrument};

/// Ends each record of the log; no field holds it
const RECORD_END: u8 = 0x1e;
/// Ends each field of a record
const FIELD_END: char = '\u{1f}';

/// A commit of the first-parent history
#[derive(Debug, Clone)]
pub struct HistoryCommit {
  pub id: String,
  pub tree: String,
  /// The first parent; `None` for a root commit
  pub parent: Option<String>,
  /// The commit has more than one parent
  pub merge: bool,
  /// The mapping lines of its note (`notes::mapping_lines`); read only by `read_ahead_of` with notes
  pub note: Option<String>,
}

/// The first-parent history of HEAD as far as a rewrite needs it, newest first
#[derive(Debug)]
pub struct History {
  /// HEAD, then the first parent of each commit
  commits: Vec<HistoryCommit>,
  positions: HashMap<String, usize>,
  /// The first parent of the oldest commit, when it was read: the commit after the oldest target (`read_to`), or the
  /// boundary (`read_ahead_of`)
  below: Option<HistoryCommit>,
  /// The baseline the walk stopped at; `None` for `read_to` and on the local baseline branch itself
  baseline: Option<String>,
}

impl History {
  /// Read the history from HEAD down to the oldest of `targets` and its parent, and no further: one `git log` that is
  /// stopped at that parent. A target not on the first-parent line of HEAD makes it read the whole history, and
  /// `position` refuses that target
  #[instrument(skip(git_executor, targets), fields(targets = targets.len()))]
  pub fn read_to(git_executor: &GitCommandExecutor, repo_path: &str, targets: &[&str]) -> Result<Self> {
    let mut wanted: HashSet<&str> = targets.iter().copied().collect();
    let mut commits: Vec<HistoryCommit> = Vec::new();
    let mut below = None;
    let format = record_format(false);
    read_records(git_executor, repo_path, &["log", "--first-parent", &format, "--end-of-options", "HEAD"], |_, commit| {
      if wanted.is_empty() && !commits.is_empty() {
        below = Some(commit);
        return ControlFlow::Break(());
      }
      wanted.remove(commit.id.as_str());
      commits.push(commit);
      ControlFlow::Continue(())
    })?;
    debug!(commits = commits.len(), stopped = below.is_some(), "read the history");
    Ok(Self::new(commits, below, None))
  }

  /// Read the commits of HEAD ahead of `baseline_branch` (all of them on the local baseline branch itself) and the
  /// boundary below them, with their mapping notes when `notes`: one `git log`
  #[instrument(skip(git_executor))]
  pub fn read_ahead_of(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str, notes: bool) -> Result<Self> {
    let baseline = (!is_on_local_baseline(git_executor, repo_path, baseline_branch)?).then(|| baseline_branch.to_string());
    let exclude = baseline.as_ref().map(|baseline| format!("^{baseline}"));
    let format = record_format(notes);
    let mut args = vec!["log", "--first-parent", "--boundary", &format, "--end-of-options", "HEAD"];
    args.extend(exclude.as_deref());
    let mut commits = Vec::new();
    // the first parent of the oldest commit, and the second parents of the merges among them
    let mut boundary = Vec::new();
    read_records(git_executor, repo_path, &args, |is_boundary, commit| {
      if is_boundary {
        boundary.push(commit);
      } else {
        commits.push(commit);
      }
      ControlFlow::Continue(())
    })?;
    let below = commits
      .last()
      .and_then(|oldest| oldest.parent.as_ref())
      .and_then(|parent| boundary.into_iter().find(|commit| commit.id == *parent));
    Ok(Self::new(commits, below, baseline))
  }

  fn new(commits: Vec<HistoryCommit>, below: Option<HistoryCommit>, baseline: Option<String>) -> Self {
    let positions = commits.iter().enumerate().map(|(position, commit)| (commit.id.clone(), position)).collect();
    Self {
      commits,
      positions,
      below,
      baseline,
    }
  }

  /// The tip the history was read from; `None` when no commit was read (HEAD is no commit ahead of the baseline)
  #[must_use]
  pub fn head(&self) -> Option<&str> {
    self.commits.first().map(|commit| commit.id.as_str())
  }

  /// The commits read, newest first
  #[must_use]
  pub fn commits(&self) -> &[HistoryCommit] {
    &self.commits
  }

  /// The commit at `position`, 0 for HEAD
  #[must_use]
  pub fn commit(&self, position: usize) -> &HistoryCommit {
    &self.commits[position]
  }

  /// The id of the commit at `position`
  #[must_use]
  pub fn id(&self, position: usize) -> &str {
    &self.commits[position].id
  }

  /// The position of the commit; `None` if it is not among the commits read
  #[must_use]
  pub fn find(&self, commit_id: &str) -> Option<usize> {
    self.positions.get(commit_id).copied()
  }

  /// The position of the commit, refused if it is not among the commits read
  pub fn position(&self, commit_id: &str) -> Result<usize> {
    self
      .find(commit_id)
      .ok_or_else(|| anyhow!("Commit {} is not on the current branch, sync and try again", short_id(commit_id)))
  }

  /// The first parent of the commit at `position`; `None` for the root commit
  #[must_use]
  pub fn parent(&self, position: usize) -> Option<&str> {
    self.commits.get(position)?.parent.as_deref()
  }

  /// The tree of the first parent of the commit at `position`; `None` for the root commit, or when the parent is below
  /// the commits read and was not read
  #[must_use]
  pub fn parent_tree(&self, position: usize) -> Option<&str> {
    let parent = self.parent(position)?;
    self
      .commits
      .get(position + 1)
      .into_iter()
      .chain(&self.below)
      .find(|commit| commit.id == parent)
      .map(|commit| commit.tree.as_str())
  }

  /// The tree of a commit read, the parent of the oldest included
  #[must_use]
  pub fn tree_of(&self, commit_id: &str) -> Option<&str> {
    match self.find(commit_id) {
      Some(position) => Some(&self.commits[position].tree),
      None => self.below.as_ref().filter(|below| below.id == commit_id).map(|below| below.tree.as_str()),
    }
  }

  /// The baseline the commits are ahead of (`read_ahead_of`); `None` on the local baseline branch itself and for
  /// `read_to`
  #[must_use]
  pub fn baseline(&self) -> Option<&str> {
    self.baseline.as_deref()
  }

  /// The guards of a rewrite of the commits from `oldest_position` up to HEAD: no rebase, merge, cherry-pick or revert
  /// in progress, HEAD on a branch, and no merge commit among them (from the parents read, no process). The target's
  /// `head` is HEAD as it is now; the caller refuses a rewrite whose history ends elsewhere
  #[instrument(skip(self, git_executor))]
  pub fn check_rewrite(&self, git_executor: &GitCommandExecutor, repo_path: &str, oldest_position: usize) -> Result<RewriteTarget> {
    ensure_no_operation_in_progress(git_executor, repo_path)?;
    let (branch, head) = checked_out_head(git_executor, repo_path)?;
    self.check_no_merges(oldest_position)?;
    Ok(RewriteTarget { branch, head })
  }

  /// `check_rewrite` for a caller that read the checked out `branch` before this history, whose HEAD it is: HEAD is not
  /// read again, since the witness of the repository lock checks both before the rewrite is written (`bd move-hunk`)
  ///
  /// # Errors
  /// As `check_rewrite`, and a history without commits
  pub fn check_rewrite_on(&self, git_executor: &GitCommandExecutor, repo_path: &str, oldest_position: usize, branch: &str) -> Result<RewriteTarget> {
    ensure_no_operation_in_progress(git_executor, repo_path)?;
    let head = self.head().ok_or_else(|| anyhow!("The checked out branch has no commits yet"))?;
    self.check_no_merges(oldest_position)?;
    Ok(RewriteTarget {
      branch: branch.to_string(),
      head: head.to_string(),
    })
  }

  /// No merge among the commits down to `oldest_position`
  fn check_no_merges(&self, oldest_position: usize) -> Result<()> {
    // newest first, as `rev-list --min-parents=2 -n1` names one
    match self.commits.iter().take(oldest_position + 1).find(|commit| commit.merge) {
      Some(merge) => Err(merge_refusal(&merge.id)),
      None => Ok(()),
    }
  }
}

/// `--format` of one record: the mark (`-` for a boundary commit), id, tree, parents and, with `notes`, the note
fn record_format(notes: bool) -> String {
  let note = if notes { "%x1f%N" } else { "" };
  format!("--format=%m%H%x1f%T%x1f%P{note}%x1e")
}

/// Run `git log` with a `record_format` and pass each commit to `on_commit` as it arrives, with whether it is a
/// boundary commit; `Break` stops git
fn read_records(git_executor: &GitCommandExecutor, repo_path: &str, args: &[&str], mut on_commit: impl FnMut(bool, HistoryCommit) -> ControlFlow<()>) -> Result<()> {
  let mut buffer: Vec<u8> = Vec::new();
  git_executor.cmd(args, repo_path).stream(|chunk| {
    // only the new bytes can end a record: the buffer holds no record end
    let search_from = buffer.len();
    buffer.extend_from_slice(chunk);
    let mut start = 0;
    for end in memchr::memchr_iter(RECORD_END, &buffer[search_from..]).map(|end| end + search_from) {
      let record = &buffer[start..end];
      start = end + 1;
      if let Some((is_boundary, commit)) = parse_record(record)?
        && on_commit(is_boundary, commit).is_break()
      {
        return Ok(ControlFlow::Break(()));
      }
    }
    buffer.drain(..start);
    Ok(ControlFlow::Continue(()))
  })
}

/// One record of `record_format`: whether it is a boundary commit, and the commit; `None` for the whitespace between
/// records
fn parse_record(record: &[u8]) -> Result<Option<(bool, HistoryCommit)>> {
  let record = String::from_utf8_lossy(record);
  // the newline that ends the record before
  let record = record.trim_start();
  if record.is_empty() {
    return Ok(None);
  }
  let is_boundary = record.starts_with('-');
  let mut fields = record.trim_start_matches(['>', '<', '-']).split(FIELD_END);
  let (Some(id), Some(tree), Some(parents)) = (fields.next(), fields.next(), fields.next()) else {
    return Err(anyhow!("Unexpected git log record: {record:?}"));
  };
  let mut parents = parents.split_whitespace();
  let parent = parents.next().map(str::to_string);
  let merge = parents.next().is_some();
  Ok(Some((
    is_boundary,
    HistoryCommit {
      id: id.to_string(),
      tree: tree.to_string(),
      parent,
      merge,
      note: fields.next().and_then(mapping_lines),
    },
  )))
}
