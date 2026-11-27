//! The candidates by pull request (openspec/specs/repository/landed-commits/spec.md, "The proof"): after a conflict,
//! the local commits that a pull request GitHub merged pushed pair with the commit GitHub merged it as (`mergeCommit`,
//! `domain_types::pull_request::MergedPullRequest`). An exact fact of GitHub, where the changed lines of the squash
//! differ from those of the local commits (an edit of the review made on GitHub) or the squash is of an author no local
//! commit has (a noreply address), so neither an identity nor the changed lines find it.
//!
//! Never a proof: a candidate (`CandidateBy::PullRequest`), left out only when the change of its unit is already in the
//! upstream or in the result (`landed_pairing::units_already_in`); the round of each of its commits offers `--landed`,
//! which asks the user when the lines differ.
//!
//! - asked only after a conflict, last (after the proofs, the same patch and the changed lines), and only when a local
//!   commit is left that nothing took: since the oldest author time of those (`MergedPullRequests::merged_since`)
//! - the merge commit: nothing took it, no local commit is it, it is in the upstream since the merge base (the commits
//!   the pairing read, else one `log` of that range by the authors of the merge commits), and no local commit has its
//!   identity (a rebase merge, whose merge commit is the last rebased commit: the keys pair it)
//! - the unit: the local commits that nothing took among what the pull request pushed, the commits of its head that the
//!   upstream lacks, by id, by identity (`CommitKey`) or by the mapping note of a local commit to its copy; one
//!   `rev-list` of the heads of every pull request together, a head the repository lacks left out. A commit is of the
//!   pull request merged first that pushed it: a later layer of a stack pushed the earlier layer too

use crate::landed_pairing::{CandidateBy, CommitKey, KeyedCommit, LandedCandidate, LandedPairing, LandedReads, LandedScope, read_keyed, walk_args};
use anyhow::Result;
use domain_types::commit::Commit;
use domain_types::pull_request::{MergedPullRequest, MergedPullRequests};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeSet, HashMap, HashSet};
use tracing::debug;

/// A pull request merged a day before the oldest author time of a commit may still hold it: the clocks of the authors
/// and of GitHub differ
const MERGED_SLACK_SECONDS: u32 = 24 * 60 * 60;

/// What the pairing after a conflict asked of the merged pull requests (`MergedPullRequests`): the oldest author time
/// of the local commits that nothing else took, and the revision of the answer, `None` when none answered. A check
/// that asked is valid only while the same question gets the same answer (`update_branch`, the preview cache)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergedAsk {
  pub since: u32,
  pub revision: Option<u64>,
}

impl MergedAsk {
  /// Whether `merged` answers the question of this ask as it was answered
  pub fn answered_alike(&self, merged: Option<&dyn MergedPullRequests>) -> bool {
    merged.and_then(|merged| merged.merged_since(self.since)).map(|known| known.revision) == self.revision
  }
}

/// `pairing` with the candidates by pull request, and what the pairing asked (`None` when no local commit was left
/// that nothing took, which asks nothing). `merged` answers the merged pull requests; without it the pairing records the
/// question with no answer. One `log` when a merge commit is not among the upstream commits the pairing read, one
/// `rev-list` of the heads of the pull requests whose merge commits are in the upstream, none else
pub(crate) fn with_candidates_by_pull_request(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  scope: LandedScope<'_>,
  reads: &LandedReads,
  merged: Option<&dyn MergedPullRequests>,
  mut pairing: LandedPairing,
) -> Result<(LandedPairing, Option<MergedAsk>)> {
  let mut taken: HashSet<&str> = pairing.drops.iter().flat_map(|drop| [drop.commit.as_str(), drop.upstream.as_str()]).collect();
  taken.extend(pairing.candidates.iter().flat_map(|candidate| candidate.unit().chain([candidate.upstream.as_str()])));
  let untaken: Vec<&Commit> = reads.local.iter().filter(|commit| !taken.contains(commit.id.as_str())).collect();
  let Some(since) = untaken.iter().map(|commit| commit.author_timestamp).min() else {
    return Ok((pairing, None));
  };
  let answer = merged.and_then(|merged| merged.merged_since(since));
  let ask = MergedAsk {
    since,
    revision: answer.as_ref().map(|known| known.revision),
  };
  let Some(answer) = answer else {
    return Ok((pairing, Some(ask)));
  };
  let local_ids: HashSet<&str> = reads.local.iter().map(|commit| commit.id.as_str()).collect();
  let mut pulls: Vec<&MergedPullRequest> = answer
    .merged
    .iter()
    .filter(|pull| {
      !pull.merge_commit.is_empty()
        && !pull.head_ref_oid.is_empty()
        && pull.merged_at.saturating_add(MERGED_SLACK_SECONDS) >= since
        && !taken.contains(pull.merge_commit.as_str())
        && !local_ids.contains(pull.merge_commit.as_str())
    })
    .collect();
  pulls.sort_by_key(|pull| (pull.merged_at, pull.number));
  if pulls.is_empty() {
    return Ok((pairing, Some(ask)));
  }
  let merge_commits = merge_commits_upstream(git_executor, repo_path, scope, reads, &pulls)?;
  let local_keys: HashSet<CommitKey> = reads.local.iter().map(key_of).collect();
  // a rebase merge: its merge commit is the last rebased commit, which the keys pair
  pulls.retain(|pull| merge_commits.get(pull.merge_commit.as_str()).is_some_and(|commit| !local_keys.contains(&commit.key)));
  if pulls.is_empty() {
    return Ok((pairing, Some(ask)));
  }
  let pushed = pushed_commits(git_executor, repo_path, scope.upstream_head, &pulls)?;

  let mut assigned: HashSet<&str> = HashSet::new();
  let mut found: Vec<LandedCandidate> = Vec::new();
  for pull in &pulls {
    let Some(range) = pushed.get(pull.head_ref_oid.as_str()) else {
      continue;
    };
    let unit: Vec<&str> = untaken
      .iter()
      .filter(|commit| !assigned.contains(commit.id.as_str()) && range.holds(commit))
      .map(|commit| commit.id.as_str())
      .collect();
    let Some((first, rest)) = unit.split_first() else {
      continue;
    };
    assigned.extend(unit.iter().copied());
    found.push(LandedCandidate {
      commit: (*first).to_string(),
      upstream: pull.merge_commit.clone(),
      landed_with: rest.iter().map(|commit| (*commit).to_string()).collect(),
      by: CandidateBy::PullRequest(pull.number),
    });
  }
  debug!(found = found.len(), pull_requests = pulls.len(), "candidates by pull request");
  pairing.candidates.extend(found);
  let position: HashMap<&str, usize> = reads.local.iter().enumerate().map(|(index, commit)| (commit.id.as_str(), index)).collect();
  pairing.candidates.sort_by_key(|candidate| position.get(candidate.commit.as_str()).copied());
  Ok((pairing, Some(ask)))
}

/// The identity of a local commit
fn key_of(commit: &Commit) -> CommitKey {
  CommitKey::from_message(&commit.author_email, commit.author_timestamp, &commit.message)
}

/// The merge commits of `pulls` in the upstream since the merge base, by id: among the commits the pairing read (by the
/// local authors), else one `log` of the range by the authors of the rest, which lists only commits of the range. A
/// merge commit of no known author is none
fn merge_commits_upstream<'a>(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  scope: LandedScope<'_>,
  reads: &'a LandedReads,
  pulls: &[&MergedPullRequest],
) -> Result<HashMap<String, KeyedCommit>> {
  let read: HashMap<&'a str, &'a KeyedCommit> = reads.upstream.iter().map(|commit| (commit.upstream.id.as_str(), commit)).collect();
  let mut found: HashMap<String, KeyedCommit> = HashMap::new();
  let mut authors: BTreeSet<&str> = BTreeSet::new();
  for pull in pulls {
    match read.get(pull.merge_commit.as_str()) {
      Some(commit) => {
        found.insert(pull.merge_commit.clone(), (*commit).clone());
      }
      None => authors.extend(pull.merge_author_email.as_deref().filter(|email| !email.is_empty())),
    }
  }
  if authors.is_empty() {
    return Ok(found);
  }
  let wanted: HashSet<&str> = pulls.iter().map(|pull| pull.merge_commit.as_str()).collect();
  let mut filters = vec!["--fixed-strings".to_string()];
  filters.extend(authors.iter().map(|email| format!("--author=<{email}>")));
  for commit in read_keyed(git_executor, repo_path, &[format!("{}..{}", scope.base, scope.upstream_head)], &filters)? {
    if wanted.contains(commit.upstream.id.as_str()) {
      found.insert(commit.upstream.id.clone(), commit);
    }
  }
  Ok(found)
}

/// What a pull request pushed: the commits of its head that the upstream lacks, by id and by identity
#[derive(Debug, Default)]
struct Pushed {
  ids: HashSet<String>,
  keys: HashSet<CommitKey>,
}

impl Pushed {
  /// Whether the pull request pushed `commit`: the commit itself, a copy of its identity, or the copy its mapping note
  /// names (a copy of a virtual branch, unless the commit changed since: `sync_core::robot_commits::mapped_virtual`)
  fn holds(&self, commit: &Commit) -> bool {
    let mapped = commit
      .mapped_commit_id
      .as_deref()
      .filter(|_| commit.mapped_source.as_ref().is_none_or(|source| source.tree == commit.tree_id));
    self.ids.contains(&commit.id) || mapped.is_some_and(|mapped| self.ids.contains(mapped)) || self.keys.contains(&key_of(commit))
  }
}

/// `\0` between the fields: the commit, its parents, its author email and time; the subject is last
const PUSHED_FORMAT: &str = "--format=%H%x00%P%x00%ae%x00%at%x00%s";

/// What each head of `pulls` pushed (`Pushed`), by head: one `rev-list` of every head the repository has, without what
/// `upstream_head` has, then each head's commits in memory along their parents. A head the repository lacks
/// (`--ignore-missing`), or one the upstream has, pushed nothing
fn pushed_commits(git_executor: &GitCommandExecutor, repo_path: &str, upstream_head: &str, pulls: &[&MergedPullRequest]) -> Result<HashMap<String, Pushed>> {
  let excluded = format!("^{upstream_head}");
  let mut revs: Vec<&str> = pulls.iter().map(|pull| pull.head_ref_oid.as_str()).collect();
  revs.push(&excluded);
  let args = walk_args(PUSHED_FORMAT, &["--ignore-missing"], &revs);
  let lines = git_executor.execute_command_lines(&args, repo_path)?;
  // the commit, its parents and its identity
  let mut walked: HashMap<&str, (Vec<&str>, CommitKey)> = HashMap::new();
  for line in &lines {
    let mut fields = line.splitn(5, '\0');
    let (Some(id), Some(parents), Some(email), Some(time)) = (fields.next(), fields.next(), fields.next(), fields.next()) else {
      continue;
    };
    let key = CommitKey::from_subject(email, time.parse().unwrap_or(0), fields.next().unwrap_or_default());
    walked.insert(id, (parents.split(' ').filter(|parent| !parent.is_empty()).collect(), key));
  }
  let mut pushed: HashMap<String, Pushed> = HashMap::new();
  for pull in pulls {
    if pushed.contains_key(&pull.head_ref_oid) || !walked.contains_key(pull.head_ref_oid.as_str()) {
      continue;
    }
    let mut of_head = Pushed::default();
    let mut stack = vec![pull.head_ref_oid.as_str()];
    while let Some(id) = stack.pop() {
      let Some((parents, key)) = walked.get(id) else { continue };
      if of_head.ids.insert(id.to_string()) {
        of_head.keys.insert(key.clone());
        stack.extend(parents.iter().copied());
      }
    }
    pushed.insert(pull.head_ref_oid.clone(), of_head);
  }
  Ok(pushed)
}

#[cfg(test)]
#[path = "landed_by_pull_request_test.rs"]
mod landed_by_pull_request_test;
