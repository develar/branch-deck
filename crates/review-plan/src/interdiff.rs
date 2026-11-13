//! The interdiff of a local branch since its review (openspec/specs/branch/review-plan/spec.md, "Since Your Review"): the commits
//! of `reviewed base..reviewed tip` paired with those of `base..tip` now, each `unchanged`, `reworded`, `rewritten`,
//! `added` or `dropped`, and the plain range of the commits added on top when nothing else changed.
//!
//! The pairing, one to one, oldest first (docs/design/git-operations.md, "Review plan"):
//! 1. the same commit on both sides: `unchanged`;
//! 2. the same identity (`CommitKey`: author email, author time, subject without the `(prefix)`), which a rebase, a
//!    sync's copy and an amend keep: when both change the same lines (`change_ids`) `unchanged`, `reworded` when their
//!    messages differ; else `rewritten`, with the lines each side changes and the other does not (`interdiffs`);
//! 3. the rest by the same changed lines, a reworded subject: `reworded`, `unchanged` when the messages are the same
//!    (only the author time moved);
//! 4. what is left: `added` on the side of the tip, `dropped` on the side of the reviewed tip.
//!
//! The message of a `reworded` or a `rewritten` commit is compared with its reviewed one (`message_change`): the
//! subject without its `(prefix)`, and the blocks of the bodies by the one grammar of a body (`commit_body`), an LCS
//! of whole blocks, so a hard wrap moved or a trailer added changes nothing. The messages come with the `rev-list` of
//! each side: no process more.
//!
//! The scope is the plain range of the commits added on top when nothing else changed; else the reviewed change carried
//! onto the base (`git_ops::carried_review`), whose diff to the tip is what changed since the review, also after a
//! rewrite, a drop or a commit added below; none when that carry conflicts.
//!
//! One `rev-list` per side, one `diff-tree --stdin -p` with one `patch-id` for the commits that are not the same on both
//! sides, one `diff-tree --stdin -p` for the rewritten pairs; none of the last three without such commits; one
//! `merge-tree` and one `fast-import` for a carry onto a moved base. No `git range-diff`: no translated text is read.

use crate::steps::split_message;
use anyhow::Result;
use domain_types::commit_body::{CommitBodyBlock, LineCode};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::carried_review::{CarryIdentity, carry_reviewed};
use git_ops::landed_pairing::{CommitKey, PathLines, WalkedCommit, change_ids, interdiffs, walk_commits_with_messages};
use git_ops::reviewed_refs::ReviewedPair;
use line_diff::lcs::diff::{Change, build_changes};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use sync_utils::commit_body::{code_of, commit_body};
use sync_utils::commit_subject::stripped_subject;
use tracing::debug;

/// How a commit of the branch compares with the reviewed one
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum PairState {
  /// The reviewed commit, or a copy of it with the same lines and the same message (a rebase, a sync)
  Unchanged,
  /// A copy of a reviewed commit that changes the same lines, its message changed (`PairedCommit::message_change`)
  Reworded,
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
  /// How its message differs from the reviewed one: `Some` when `reworded`, of a `rewritten` one when its message
  /// changed too
  pub message_change: Option<MessageChange>,
  /// Its raw message, of the reviewed one when `dropped`: what the steps of the review tell (`steps`)
  #[serde(skip)]
  #[cfg_attr(feature = "specta", specta(skip))]
  pub message: String,
}

/// How the message of a commit differs from its reviewed version: its subject, and its body by the blocks of its grammar
/// (`CommitBody::blocks`); the trailers and the line breaks of a paragraph aside
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct MessageChange {
  /// The subject of the reviewed version without its `(prefix)`, when that differs: only of a reword by the lines, a
  /// commit of the same identity has the same subject
  pub subject_before: Option<String>,
  /// What changed in the body, in its order
  pub hunks: Vec<BodyHunk>,
}

/// A change of the body of a commit since its review: blocks it has now, reviewed blocks it lacks, or both in one place
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct BodyHunk {
  /// Its place in the body now (`CommitBody::blocks` as its story reads it): its first added block, else the block the
  /// removed ones stood before (the count of the blocks after the last)
  pub start: u32,
  /// The blocks the body has now that the reviewed one lacked: `start..start + added.blocks.len()`
  pub added: BodyBlocks,
  /// The reviewed blocks the body lacks now, which stood right before `start`
  pub removed: BodyBlocks,
}

/// Blocks of a body with the code of their lines, as `CommitBody` holds them (`LineCode::block` counts these blocks)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct BodyBlocks {
  pub blocks: Vec<CommitBodyBlock>,
  pub code: Vec<LineCode>,
}

impl BodyBlocks {
  fn of(blocks: &[CommitBodyBlock]) -> Self {
    Self {
      blocks: blocks.to_vec(),
      code: code_of(blocks),
    }
  }
}

/// How the message `now` differs from `reviewed` (`MessageChange`); `None` when it says the same: the same subject
/// without its `(prefix)` and the same blocks of the body, so a trailer added or a paragraph wrapped anew changes
/// nothing
pub fn message_change(reviewed: &str, now: &str) -> Option<MessageChange> {
  let (reviewed_subject, reviewed_body) = split_message(reviewed);
  let (subject, body) = split_message(now);
  let reviewed_subject = stripped_subject(reviewed_subject);
  let subject_before = (reviewed_subject.trim() != stripped_subject(subject).trim()).then(|| reviewed_subject.trim().to_string());
  let before = commit_body(reviewed_body).blocks;
  let after = commit_body(body).blocks;
  // a body too different for an exact answer in time is one change whole
  let changes = build_changes(&before, &after).unwrap_or_else(|_| {
    vec![Change {
      line0: 0,
      line1: 0,
      deleted: before.len(),
      inserted: after.len(),
    }]
  });
  let hunks: Vec<BodyHunk> = changes
    .into_iter()
    .filter(|change| change.deleted > 0 || change.inserted > 0)
    .map(|change| BodyHunk {
      start: u32::try_from(change.line1).unwrap_or(u32::MAX),
      added: BodyBlocks::of(&after[change.line1..change.line1 + change.inserted]),
      removed: BodyBlocks::of(&before[change.line0..change.line0 + change.deleted]),
    })
    .collect();
  (subject_before.is_some() || !hunks.is_empty()).then_some(MessageChange { subject_before, hunks })
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
  /// Only the commits the reviewer has not read: `added`, `rewritten` and `reworded`
  pub fn retain_unreviewed(&mut self) {
    self.commits.retain(|commit| commit.state.unreviewed());
  }
}

impl PairState {
  /// The reviewer has not read it as it is: `added`, `rewritten`, or `reworded`, whose message is new
  pub const fn unreviewed(self) -> bool {
    matches!(self, Self::Added | Self::Rewritten | Self::Reworded)
  }

  /// Its code is as reviewed: `unchanged` or `reworded`
  pub const fn same_code(self) -> bool {
    matches!(self, Self::Unchanged | Self::Reworded)
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

/// The commits of `base..tip`, oldest first, without merges, each with its raw message: one `rev-list`
pub(crate) fn commits_of(git: &GitCommandExecutor, repository_path: &str, base: &str, tip: &str) -> Result<Vec<(WalkedCommit, String)>> {
  walk_commits_with_messages(git, repository_path, &["--reverse", "--no-merges"], &[tip, &format!("^{base}")])
}

/// The interdiff of the branch at `base..tip` against its `reviewed` pair
pub fn interdiff(git: &GitCommandExecutor, repository_path: &str, reviewed: &ReviewedPair, base: &str, tip: &str) -> Result<Interdiff> {
  let (old, old_messages): (Vec<WalkedCommit>, Vec<String>) = commits_of(git, repository_path, &reviewed.base, &reviewed.tip)?.into_iter().unzip();
  let (new, new_messages): (Vec<WalkedCommit>, Vec<String>) = commits_of(git, repository_path, base, tip)?.into_iter().unzip();
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
    .zip(new_messages)
    .map(|((commit, matched), message)| {
      let (reviewed_id, state, message_change) = match matched {
        None => (None, PairState::Added, None),
        // the same commit, the same message
        Some((reviewed, Match::Same)) => (Some(old[*reviewed].id.clone()), PairState::Unchanged, None),
        Some((reviewed_index, how)) => {
          let reviewed = &old[*reviewed_index];
          let same = *how == Match::Changes || change_of.get(&commit.id) == change_of.get(&reviewed.id);
          let message_change = message_change(&old_messages[*reviewed_index], &message);
          let state = match (same, &message_change) {
            (false, _) => PairState::Rewritten,
            (true, Some(_)) => PairState::Reworded,
            (true, None) => PairState::Unchanged,
          };
          (Some(reviewed.id.clone()), state, message_change)
        }
      };
      PairedCommit {
        id: Some(commit.id.clone()),
        reviewed_id,
        subject: commit.subject.clone(),
        state,
        changes: None,
        message_change,
        message,
      }
    })
    .collect();
  let scope = match scope_of(&commits, taken.iter().all(|taken| *taken), base, tip) {
    Some(scope) => Some(scope),
    None => carried_scope(git, repository_path, reviewed, old.last(), base, tip)?,
  };
  commits.extend(
    old
      .iter()
      .zip(&taken)
      .zip(old_messages)
      .filter(|((_, taken), _)| !**taken)
      .map(|((commit, _), message)| PairedCommit {
        id: None,
        reviewed_id: Some(commit.id.clone()),
        subject: commit.subject.clone(),
        state: PairState::Dropped,
        changes: None,
        message_change: None,
        message,
      }),
  );

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
  // a reworded commit changes the same lines: the range around it is still the one the reviewer read
  let first = commits.iter().position(|commit| !commit.state.same_code()).unwrap_or(commits.len());
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
