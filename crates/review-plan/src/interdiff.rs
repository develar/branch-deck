//! The interdiff of a local branch since its review (spec/branch/review-plan.spec.md, "Since Your Review"): the commits
//! of `reviewed base..reviewed tip` paired with those of `base..tip` now, each `unchanged`, `rewritten`, `added` or
//! `dropped`, and the plain range of the commits added on top when nothing else changed.
//!
//! The pairing, one to one, oldest first (docs/design/git-operations.md, "Review plan"):
//! 1. the same commit on both sides: `unchanged`;
//! 2. the same identity (`CommitKey`: author email, author time, subject without the `(prefix)`), which a rebase, a
//!    sync's copy and an amend keep: `unchanged` when both change the same lines (`change_ids`), else `rewritten`, with
//!    the lines each side changes and the other does not (`interdiffs`);
//! 3. the rest by the same changed lines, a reworded commit: `unchanged`;
//! 4. what is left: `added` on the side of the tip, `dropped` on the side of the reviewed tip.
//!
//! The scope is the plain range of the commits added on top when nothing else changed; else the reviewed change carried
//! onto the base (`git_ops::carried_review`), whose diff to the tip is what changed since the review, also after a
//! rewrite, a drop or a commit added below; none when that carry conflicts.
//!
//! One `rev-list` per side, one `diff-tree --stdin -p` with one `patch-id` for the commits that are not the same on both
//! sides, one `diff-tree --stdin -p` for the rewritten pairs; none of the last three without such commits; one
//! `merge-tree` and one `fast-import` for a carry onto a moved base. No `git range-diff`: no translated text is read.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::carried_review::{CarryIdentity, carry_reviewed};
use git_ops::landed_pairing::{CommitKey, PathLines, WalkedCommit, change_ids, interdiffs, walk_commits};
use git_ops::reviewed_refs::ReviewedPair;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::debug;

/// How a commit of the branch compares with the reviewed one
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum PairState {
  /// The reviewed commit, or a copy of it that changes the same lines (a rebase, a sync, a reword)
  Unchanged,
  /// A reviewed commit changed: the same identity, other changed lines (an amend, a fixup folded in)
  Rewritten,
  /// A commit the reviewed tip lacks
  Added,
  /// A reviewed commit the branch lacks now
  Dropped,
}

/// The lines a rewritten commit and its reviewed version change differently: per side, per path, `+<line>` added and
/// `-<line>` removed (`mode <old> <new>`, `binary <blob>` for what no line shows), at most 20 paths of 20 lines
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitChanges {
  /// What the commit changes now and its reviewed version did not
  pub new_only: Vec<PathLines>,
  /// What the reviewed version changed and the commit does not any more
  pub reviewed_only: Vec<PathLines>,
}

/// A commit of the branch with the reviewed commit it pairs with
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PairedCommit {
  /// The commit of the branch, `None` when `dropped`
  pub id: Option<String>,
  /// The reviewed commit, `None` when `added`
  pub reviewed_id: Option<String>,
  /// The subject of the commit, of the reviewed one when `dropped`
  pub subject: String,
  pub state: PairState,
  /// `Some` only when `rewritten`
  pub changes: Option<CommitChanges>,
}

/// What changed since the review, as the diff of `base..head`
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RangeScope {
  pub base: String,
  pub head: String,
  /// `base` is the reviewed change carried onto the base of the branch, a commit on no ref: the range is one diff, not
  /// commits of the branch
  pub carried: bool,
}

/// What changed in a branch since its review
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Interdiff {
  /// The reviewed pair, as Mark Reviewed kept it
  pub reviewed_base: String,
  pub reviewed_tip: String,
  /// The commits of the branch, oldest first, then the dropped ones in their reviewed order
  pub commits: Vec<PairedCommit>,
  /// The commits added on top as a plain range, `<parent of the first added>..tip`, when every other commit is
  /// unchanged and none is dropped; `tip..tip`, empty, when nothing changed; else, after a rewrite, a drop or a commit
  /// added below an unchanged one, `<carried>..tip` from the reviewed change carried onto the base; `None` when that
  /// carry conflicts
  pub scope: Option<RangeScope>,
}

impl Interdiff {
  /// Only the commits the reviewer has not read: `added` and `rewritten`
  pub fn retain_unreviewed(&mut self) {
    self.commits.retain(|commit| matches!(commit.state, PairState::Added | PairState::Rewritten));
  }
}

/// How a commit of the branch found its reviewed one
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Match {
  /// The same commit
  Same,
  /// The same `CommitKey`
  Identity,
  /// The same changed lines
  Changes,
}

/// The commits of `base..tip`, oldest first, without merges: one `rev-list`
pub(crate) fn commits_of(git: &GitCommandExecutor, repository_path: &str, base: &str, tip: &str) -> Result<Vec<WalkedCommit>> {
  walk_commits(git, repository_path, &["--reverse", "--no-merges"], &[tip, &format!("^{base}")])
}

/// The interdiff of the branch at `base..tip` against its `reviewed` pair
pub fn interdiff(git: &GitCommandExecutor, repository_path: &str, reviewed: &ReviewedPair, base: &str, tip: &str) -> Result<Interdiff> {
  let old = commits_of(git, repository_path, &reviewed.base, &reviewed.tip)?;
  let new = commits_of(git, repository_path, base, tip)?;
  let mut matched: Vec<Option<(usize, Match)>> = vec![None; new.len()];
  let mut taken = vec![false; old.len()];

  // the same commit
  let old_index: HashMap<&str, usize> = old.iter().enumerate().map(|(index, commit)| (commit.id.as_str(), index)).collect();
  for (index, commit) in new.iter().enumerate() {
    if let Some(&reviewed) = old_index.get(commit.id.as_str()) {
      matched[index] = Some((reviewed, Match::Same));
      taken[reviewed] = true;
    }
  }

  // the same identity, the oldest of a key with the oldest
  let mut by_key: HashMap<&CommitKey, VecDeque<usize>> = HashMap::new();
  for (index, commit) in old.iter().enumerate().filter(|(index, _)| !taken[*index]) {
    by_key.entry(&commit.key).or_default().push_back(index);
  }
  for (index, commit) in new.iter().enumerate() {
    if matched[index].is_none()
      && let Some(reviewed) = by_key.get_mut(&commit.key).and_then(VecDeque::pop_front)
    {
      matched[index] = Some((reviewed, Match::Identity));
      taken[reviewed] = true;
    }
  }

  // the changed lines of every commit that is not on both sides: one batch
  let same: HashSet<usize> = matched.iter().flatten().filter(|(_, how)| *how == Match::Same).map(|(reviewed, _)| *reviewed).collect();
  let compared: Vec<&str> = new
    .iter()
    .zip(&matched)
    .filter(|(_, matched)| !matches!(matched, Some((_, Match::Same))))
    .map(|(commit, _)| commit.id.as_str())
    .chain(old.iter().enumerate().filter(|(index, _)| !same.contains(index)).map(|(_, commit)| commit.id.as_str()))
    .collect();
  let change_of = change_ids(git, repository_path, &compared)?;

  // the rest by the same changed lines (a reword)
  let mut by_change: HashMap<&str, VecDeque<usize>> = HashMap::new();
  for (index, commit) in old.iter().enumerate().filter(|(index, _)| !taken[*index]) {
    if let Some(change) = change_of.get(&commit.id) {
      by_change.entry(change.as_str()).or_default().push_back(index);
    }
  }
  for (index, commit) in new.iter().enumerate() {
    if matched[index].is_none()
      && let Some(reviewed) = change_of
        .get(&commit.id)
        .and_then(|change| by_change.get_mut(change.as_str()))
        .and_then(VecDeque::pop_front)
    {
      matched[index] = Some((reviewed, Match::Changes));
      taken[reviewed] = true;
    }
  }

  let mut commits: Vec<PairedCommit> = new
    .iter()
    .zip(&matched)
    .map(|(commit, matched)| {
      let (reviewed_id, state) = match matched {
        None => (None, PairState::Added),
        Some((reviewed, how)) => {
          let reviewed = &old[*reviewed];
          let same = *how != Match::Identity || change_of.get(&commit.id) == change_of.get(&reviewed.id);
          (Some(reviewed.id.clone()), if same { PairState::Unchanged } else { PairState::Rewritten })
        }
      };
      PairedCommit {
        id: Some(commit.id.clone()),
        reviewed_id,
        subject: commit.subject.clone(),
        state,
        changes: None,
      }
    })
    .collect();
  let scope = match scope_of(&commits, taken.iter().all(|taken| *taken), base, tip) {
    Some(scope) => Some(scope),
    None => carried_scope(git, repository_path, reviewed, old.last(), base, tip)?,
  };
  commits.extend(old.iter().zip(&taken).filter(|(_, taken)| !**taken).map(|(commit, _)| PairedCommit {
    id: None,
    reviewed_id: Some(commit.id.clone()),
    subject: commit.subject.clone(),
    state: PairState::Dropped,
    changes: None,
  }));

  with_changes(git, repository_path, &mut commits)?;
  debug!(new = new.len(), reviewed = old.len(), scoped = scope.is_some(), "interdiff since the review");
  Ok(Interdiff {
    reviewed_base: reviewed.base.clone(),
    reviewed_tip: reviewed.tip.clone(),
    commits,
    scope,
  })
}

/// The commits added on top as a plain range: every commit before the first added one unchanged, every one from it on
/// added, none dropped
fn scope_of(commits: &[PairedCommit], none_dropped: bool, base: &str, tip: &str) -> Option<RangeScope> {
  if !none_dropped {
    return None;
  }
  let first = commits.iter().position(|commit| commit.state != PairState::Unchanged).unwrap_or(commits.len());
  if commits[first..].iter().any(|commit| commit.state != PairState::Added) {
    return None;
  }
  let below = if first == 0 { Some(base) } else { commits[first - 1].id.as_deref() };
  Some(RangeScope {
    base: below.unwrap_or(tip).to_string(),
    head: tip.to_string(),
    carried: false,
  })
}

/// `<carried>..tip`: the change of the `reviewed` pair carried onto `base`, in the name of the author of the reviewed tip
/// (`reviewed_tip`, the last reviewed commit), so the same carry writes the same commit; `None` when it conflicts or
/// the review has no commit
fn carried_scope(
  git: &GitCommandExecutor,
  repository_path: &str,
  reviewed: &ReviewedPair,
  reviewed_tip: Option<&WalkedCommit>,
  base: &str,
  tip: &str,
) -> Result<Option<RangeScope>> {
  let Some(reviewed_tip) = reviewed_tip else { return Ok(None) };
  let identity = CarryIdentity {
    name: &reviewed_tip.author,
    email: &reviewed_tip.key.email,
    time: reviewed_tip.key.time,
  };
  Ok(
    carry_reviewed(git, repository_path, &reviewed.base, &reviewed.tip, base, identity)?.map(|carried| RangeScope {
      base: carried,
      head: tip.to_string(),
      carried: true,
    }),
  )
}

/// The changed lines of each rewritten commit against its reviewed version: one `interdiffs`, none without rewrites
fn with_changes(git: &GitCommandExecutor, repository_path: &str, commits: &mut [PairedCommit]) -> Result<()> {
  let rewritten: Vec<usize> = commits
    .iter()
    .enumerate()
    .filter(|(_, commit)| commit.state == PairState::Rewritten)
    .map(|(index, _)| index)
    .collect();
  let pairs: Vec<(Vec<&str>, &str)> = rewritten
    .iter()
    .filter_map(|index| Some((vec![commits[*index].id.as_deref()?], commits[*index].reviewed_id.as_deref()?)))
    .collect();
  let found = interdiffs(git, repository_path, &pairs)?;
  for (index, interdiff) in rewritten.into_iter().zip(found) {
    commits[index].changes = Some(CommitChanges {
      new_only: interdiff.local_only,
      reviewed_only: interdiff.upstream_only,
    });
  }
  Ok(())
}
