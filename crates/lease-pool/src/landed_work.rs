//! Whether the work of a lease landed (spec/cli/lease.spec.md, "Return" and "Takeover"; "Lease" in
//! docs/design/git-operations.md): one rule for the return of a lease, the takeover of an idle one by a get and
//! `bd lease reclaim`. The commits it asks about are the candidates: for a return those no branch, tag or remote holds,
//! for a takeover those the home branch lacks, as the lease's own branch holds them. Each candidate lands only by a
//! proof against the tip of the home branch, rung by rung, each for the candidates the one before left:
//!
//! 1. the dry run of a pick onto it (`pick_dry_run`): a commit of its range the pick would not pick;
//! 2. the patch id of a commit of the branch (`present_by_patch_id`): a reword, a cherry-pick;
//! 3. the ladder of `bd update` (`pair_twins`): a commit of the same identity on the branch that is its cherry-pick
//!    (`replayed`) or changes the same lines (`changed_lines`);
//! 4. the twin of `bd update`: a commit of the same identity whose change, with the fixups of the commit, holds every
//!    changed line of the commit (`interdiffs`, `local_only` empty), as a conflict resolution or a robot's autosquash
//!    leaves it; or one that lacks only lines the commit removes and the parent of the twin no longer had, as the
//!    resolution of a commit that removes lines another commit had rewritten or removed already leaves it (the branch
//!    loses nothing: the line is gone);
//! 5. a commit of the same author and subject at another author time (a commit made anew, a rebase that reset the
//!    author), whose change passes the twin check of the rung before: the content decides, the subject only picks it;
//! 6. the mapping note of a sync (`v-commit-v1`): its virtual copy, which a robot rebased and landed, is on the branch
//!    or proven there by the same rungs.
//!
//! No range comes from the reflog of the branch: the fork point a pick starts from can leave commits out, and a commit
//! that no rung proves is unlanded.

use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::autosquash_targets::target_of_each_fixup;
use git_ops::commit_utils::get_commits_info;
use git_ops::landed_pairing::{
  CommitKey, LandedCandidate, LandedPairing, LandedProof, LandedReads, PathLines, full_interdiffs, interdiffs, pair_by_patch_id, pair_twins, present_by_patch_id, read_local,
  read_upstream,
};
use git_ops::lease_slot::{UnlandedCommit, commits_beyond_branch, commits_since, unlanded_commits};
use git_ops::object_texts::read_texts;
use git_ops::pick::pick_dry_run;
use git_ops::revs::{is_ancestor, merge_base, ref_exists, resolve_commit};
use history_rewrite::stack::stacks_enabled;
use std::collections::{HashMap, HashSet};
use sync_core::baseline::baseline_tracking_ref;
use sync_core::robot_commits::mapped_virtual;
use tracing::debug;

/// How many candidates a check proves; a lease with more is unlanded without a proof, as no session lands that many
/// commits by hand
const CANDIDATE_LIMIT: usize = 500;
/// How many commits of the branch with the author and the subject of a commit the check by subject tries, newest first
const SUBJECT_TWINS_PER_COMMIT: usize = 3;
/// How many unlanded commits an answer names
pub const UNLANDED_LIMIT: usize = 20;
/// The largest file of the parent of a twin that the check of its removed lines reads; a larger one proves nothing
const REMOVED_BEFORE_MAX_BYTES: usize = 8 << 20;

/// Who asks whether the work of a lease landed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
  /// The return of the lease by its holder: work that a branch, a tag or a remote branch holds is not lost, so the
  /// candidates are the commits none holds
  Return,
  /// The takeover of an idle lease, by a get or `bd lease reclaim`: its work must be in the home branch, so the
  /// candidates are the commits the home branch lacks, whatever other branch holds them
  TakeOver,
}

/// How the home branch proves that it has a commit of the lease
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
  /// The dry run of a pick onto the branch would not pick it
  Picked,
  /// A commit of the branch has its patch id
  PatchId,
  /// A commit of the branch of its identity is its cherry-pick
  Replayed,
  /// A commit of the branch of its identity changes the same lines
  ChangedLines,
  /// A commit of the branch of its identity has every line it changes, with its fixups (the twin of `bd update`)
  Twin,
  /// A commit of the branch of its identity has every line it adds, with its fixups; the lines it removes and the twin
  /// does not were gone from the parent of the twin already
  TwinRemovedBefore,
  /// A commit of the branch of the same author and subject, at another author time, passes the twin check
  SubjectTwin,
  /// Its virtual copy (`v-commit-v1`) landed on the branch
  Note { virtual_commit: String },
}

impl Proof {
  const fn of(proof: LandedProof) -> Self {
    match proof {
      LandedProof::PatchId => Self::PatchId,
      LandedProof::Replayed => Self::Replayed,
      LandedProof::ChangedLines => Self::ChangedLines,
    }
  }
}

/// A commit of the lease that the home branch has
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandedCommit {
  pub id: String,
  pub subject: String,
  pub proof: Proof,
}

/// Which check decided what is unlanded
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LandedCheck {
  /// No branch, tag or remote branch lacks a commit of HEAD (beyond the leased commit); or, with `reason`, no check by
  /// content could run
  Reachability { reason: Option<String> },
  /// The proofs against the branch of the source: what none proves is unlanded
  Content { branch: String },
}

impl LandedCheck {
  /// The words of a refusal
  pub fn describe(&self) -> String {
    match self {
      Self::Reachability { reason: None } => "no branch, tag or remote branch holds".to_string(),
      Self::Reachability { reason: Some(reason) } => format!("no branch, tag or remote branch holds (no check by content: {reason})"),
      Self::Content { branch } => format!("{branch} lacks by content: no pick, patch id, identity, twin of `bd update` or mapping note of a sync proves them there"),
    }
  }
}

/// What the check of a lease found
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandedWork {
  /// The candidates the home branch proved to have, newest first
  pub landed: Vec<LandedCommit>,
  /// The candidates no rung proved, newest first, at most `UNLANDED_LIMIT`
  pub unlanded: Vec<UnlandedCommit>,
  pub check: LandedCheck,
}

/// Whether the work of the lease in `slot` landed, for `purpose`: `source_head` is the commit it was leased at, whose
/// commits are never its work, and `source_branch` the home branch. A candidate lands only by a proof against the tip of
/// that branch (the module's rungs); without the branch, nothing is proven
pub fn landed_work(git_executor: &GitCommandExecutor, slot: &str, source_head: &str, source_branch: Option<&str>, purpose: Purpose) -> Result<LandedWork> {
  let base = Some(source_head).filter(|head| !head.is_empty());
  let unproven = |candidates: Vec<UnlandedCommit>, check: LandedCheck| LandedWork {
    landed: Vec::new(),
    unlanded: candidates.into_iter().take(UNLANDED_LIMIT).collect(),
    check,
  };
  let reachability = |reason: Option<String>| LandedCheck::Reachability { reason };
  let candidates = match (purpose, source_branch) {
    (Purpose::Return, _) => {
      let candidates = unlanded_commits(git_executor, slot, base, CANDIDATE_LIMIT)?;
      if candidates.is_empty() {
        return Ok(unproven(candidates, reachability(None)));
      }
      candidates
    }
    (Purpose::TakeOver, Some(branch)) if ref_exists(git_executor, slot, &format!("refs/heads/{branch}")) => {
      let candidates = commits_beyond_branch(git_executor, slot, branch, base, CANDIDATE_LIMIT)?;
      if candidates.is_empty() {
        return Ok(unproven(candidates, LandedCheck::Content { branch: branch.to_string() }));
      }
      candidates
    }
    (Purpose::TakeOver, _) => {
      // no home branch to land in: only a lease with no commit of its own is landed, whatever branch holds its commits
      let candidates = commits_since(git_executor, slot, base, CANDIDATE_LIMIT)?;
      return Ok(unproven(candidates, reachability(Some(no_branch_reason(source_branch)))));
    }
  };
  let Some(branch) = source_branch else {
    return Ok(unproven(candidates, reachability(Some(no_branch_reason(None)))));
  };
  if !ref_exists(git_executor, slot, &format!("refs/heads/{branch}")) {
    return Ok(unproven(candidates, reachability(Some(no_branch_reason(Some(branch))))));
  }
  let check = LandedCheck::Content { branch: branch.to_string() };
  if candidates.len() == CANDIDATE_LIMIT {
    debug!(slot = %slot, "too many candidates to prove");
    return Ok(unproven(candidates, check));
  }
  let mut proofs = Proofs::new(&candidates);
  prove(git_executor, slot, branch, base, &mut proofs)?;
  Ok(proofs.finish(check))
}

fn no_branch_reason(branch: Option<&str>) -> String {
  match branch {
    None => "the source had a detached HEAD".to_string(),
    Some(branch) => format!("the branch {branch} is gone"),
  }
}

/// The candidates and the proofs found so far
struct Proofs<'a> {
  candidates: &'a [UnlandedCommit],
  proven: HashMap<String, Proof>,
}

impl<'a> Proofs<'a> {
  fn new(candidates: &'a [UnlandedCommit]) -> Self {
    Self {
      candidates,
      proven: HashMap::new(),
    }
  }

  /// The candidates no rung proved yet, newest first
  fn left(&self) -> Vec<&'a str> {
    self
      .candidates
      .iter()
      .map(|commit| commit.id.as_str())
      .filter(|id| !self.proven.contains_key(*id))
      .collect()
  }

  fn is_done(&self) -> bool {
    self.proven.len() == self.candidates.len()
  }

  /// Record `proof` of `id` when it is a candidate not proven yet
  fn add(&mut self, id: &str, proof: Proof) {
    if self.candidates.iter().any(|commit| commit.id == id) && !self.proven.contains_key(id) {
      self.proven.insert(id.to_string(), proof);
    }
  }

  fn finish(mut self, check: LandedCheck) -> LandedWork {
    let mut landed = Vec::new();
    let mut unlanded = Vec::new();
    for commit in self.candidates {
      match self.proven.remove(&commit.id) {
        Some(proof) => landed.push(LandedCommit {
          id: commit.id.clone(),
          subject: commit.subject.clone(),
          proof,
        }),
        None => unlanded.push(commit.clone()),
      }
    }
    unlanded.truncate(UNLANDED_LIMIT);
    LandedWork { landed, unlanded, check }
  }
}

/// The rungs of the module against the tip of `branch`, each for the candidates the one before left
fn prove(git_executor: &GitCommandExecutor, slot: &str, branch: &str, base: Option<&str>, proofs: &mut Proofs<'_>) -> Result<()> {
  // 1. the dry run of a pick: only a commit of its range that it would not pick
  let upstream = baseline_tracking_ref(git_executor, slot).ok().and_then(|(_, upstream)| upstream);
  match pick_dry_run(git_executor, slot, branch, slot, upstream.as_deref()) {
    Ok(dry_run) => {
      let kept: HashSet<&str> = dry_run.kept.iter().map(|commit| commit.id.as_str()).collect();
      for id in dry_run.range.iter().filter(|id| !kept.contains(id.as_str())) {
        proofs.add(id, Proof::Picked);
      }
    }
    Err(error) => debug!(slot = %slot, error = %format!("{error:#}"), "no dry run of a pick"),
  }
  if proofs.is_done() {
    return Ok(());
  }
  let Some(tip) = resolve_commit(git_executor, slot, &format!("refs/heads/{branch}"))? else {
    return Ok(());
  };
  let head = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], slot)?;

  // 2. the patch id of a commit of the branch's side
  for drop in present_by_patch_id(git_executor, slot, &tip, &head, &proofs.left())? {
    proofs.add(&drop.commit, Proof::PatchId);
  }
  if proofs.is_done() {
    return Ok(());
  }

  // 3. and 4. the ladder and the twins of `bd update`, over the commits of the lease the branch lacks
  let Some(merge_base) = merge_base(git_executor, slot, &head, &tip)? else {
    return Ok(());
  };
  let mut revs = vec![head, format!("^{tip}")];
  revs.extend(base.map(|base| format!("^{base}")));
  let local = read_local(git_executor, slot, &revs)?;
  if local.is_empty() {
    return Ok(());
  }
  let upstream = read_upstream(git_executor, slot, &local, &merge_base, &tip)?;
  let stacks = stacks_enabled(git_executor, slot)?;
  let reads = LandedReads { local, upstream };
  for (commit, proof) in landed_by_pairing(git_executor, slot, &pair_twins(git_executor, slot, &reads, stacks)?)? {
    proofs.add(&commit, proof);
  }
  if proofs.is_done() {
    return Ok(());
  }

  // 5. the same author and subject at another author time
  for (commit, _) in landed_by_pairing(git_executor, slot, &subject_twins(&reads, stacks, proofs))? {
    proofs.add(&commit, Proof::SubjectTwin);
  }
  if proofs.is_done() {
    return Ok(());
  }

  // 6. the virtual copy of a sync, by its mapping note
  let copies: HashMap<&str, &str> = reads
    .local
    .iter()
    .filter(|commit| proofs.left().contains(&commit.id.as_str()))
    .filter_map(|commit| Some((mapped_virtual(commit)?, commit.id.as_str())))
    .collect();
  if copies.is_empty() {
    return Ok(());
  }
  let note = |copy: &str| Proof::Note { virtual_commit: copy.to_string() };
  let mut left_copies: Vec<&str> = Vec::new();
  for (copy, original) in &copies {
    if is_ancestor(git_executor, slot, copy, &tip).unwrap_or(false) {
      proofs.add(original, note(copy));
    } else {
      left_copies.push(copy);
    }
  }
  if left_copies.is_empty() {
    return Ok(());
  }
  let upstream_ids: Vec<&str> = reads.upstream.iter().map(|commit| commit.upstream.id.as_str()).collect();
  for drop in pair_by_patch_id(git_executor, slot, &left_copies, &upstream_ids)? {
    if let Some(original) = copies.get(drop.commit.as_str()) {
      proofs.add(original, note(&drop.commit));
    }
  }
  let unproven: Vec<&str> = left_copies
    .into_iter()
    .filter(|copy| copies.get(copy).is_some_and(|original| !proofs.proven.contains_key(*original)))
    .collect();
  if unproven.is_empty() {
    return Ok(());
  }
  let commits = get_commits_info(git_executor, slot, &unproven)?;
  let copy_reads = LandedReads {
    local: unproven.iter().filter_map(|copy| commits.get(*copy).cloned()).collect(),
    upstream: reads.upstream,
  };
  for (copy, _) in landed_by_pairing(git_executor, slot, &pair_twins(git_executor, slot, &copy_reads, stacks)?)? {
    if let Some(original) = copies.get(copy.as_str()) {
      proofs.add(original, note(&copy));
    }
  }
  Ok(())
}

/// The pairs of each commit no rung proved yet with the commits of the branch of its author and subject at another
/// author time (the newest `SUBJECT_TWINS_PER_COMMIT`), each with the fixups of the commit still unproven: candidates
/// for the twin check (`landed_by_pairing`), which decides by their content alone. A fixup is no commit of its own here
fn subject_twins(reads: &LandedReads, stacks: bool, proofs: &Proofs<'_>) -> LandedPairing {
  let left: HashSet<&str> = proofs.left().into_iter().collect();
  let target_of = target_of_each_fixup(&reads.local, stacks);
  let mut fixups: HashMap<&str, Vec<String>> = HashMap::new();
  for commit in reads.local.iter().filter(|commit| left.contains(commit.id.as_str())) {
    if let Some(target) = target_of.get(&commit.id) {
      fixups.entry(target.as_str()).or_default().push(commit.id.clone());
    }
  }
  // newest first, as the upstream is read
  let mut by_subject: HashMap<(&str, &str), Vec<(&str, u32)>> = HashMap::new();
  for commit in &reads.upstream {
    by_subject
      .entry((commit.key.email.as_str(), commit.key.subject.as_str()))
      .or_default()
      .push((commit.upstream.id.as_str(), commit.key.time));
  }
  let mut pairing = LandedPairing::default();
  for commit in reads.local.iter().filter(|commit| left.contains(commit.id.as_str()) && !target_of.contains_key(&commit.id)) {
    let key = CommitKey::from_message(&commit.author_email, commit.author_timestamp, &commit.message);
    let twins = by_subject.get(&(key.email.as_str(), key.subject.as_str())).map_or(&[][..], Vec::as_slice);
    // the same time was the ladder's pair already
    for (upstream, _) in twins.iter().filter(|(_, time)| *time != key.time).take(SUBJECT_TWINS_PER_COMMIT) {
      pairing.candidates.push(LandedCandidate {
        commit: commit.id.clone(),
        upstream: (*upstream).to_string(),
        fixups: fixups.get(commit.id.as_str()).cloned().unwrap_or_default(),
      });
    }
  }
  pairing
}

/// The local commits a pairing proves landed: its drops with their proof, then each candidate whose upstream commit
/// holds every changed line of the candidate and its fixups (`interdiffs`, `local_only` empty), with its fixups: the
/// twin rule of `bd update` (`LandedTwin::is_same` of update-core); then each candidate whose twin lacks only removals
/// that its parent no longer had (`removed_before`)
fn landed_by_pairing(git_executor: &GitCommandExecutor, slot: &str, pairing: &LandedPairing) -> Result<Vec<(String, Proof)>> {
  let mut landed: Vec<(String, Proof)> = pairing.drops.iter().map(|drop| (drop.commit.clone(), Proof::of(drop.proof))).collect();
  let pairs: Vec<(Vec<&str>, &str)> = pairing
    .candidates
    .iter()
    .map(|candidate| (candidate.unit().collect(), candidate.upstream.as_str()))
    .collect();
  let mut removals = Vec::new();
  for (candidate, interdiff) in pairing.candidates.iter().zip(interdiffs(git_executor, slot, &pairs)?) {
    if interdiff.local_only.is_empty() {
      landed.extend(candidate.unit().map(|commit| (commit.to_string(), Proof::Twin)));
    } else if only_removals(&interdiff.local_only) {
      removals.push(candidate);
    }
  }
  for candidate in removed_before(git_executor, slot, &removals)? {
    landed.extend(candidate.unit().map(|commit| (commit.to_string(), Proof::TwinRemovedBefore)));
  }
  Ok(landed)
}

/// Whether every line one side of an interdiff has and the other lacks is a removed line: no added line, no mode, no
/// binary file
fn only_removals(lines: &[PathLines]) -> bool {
  lines.iter().all(|path| path.lines.iter().all(|line| line.starts_with('-')))
}

/// The `candidates` whose twin lacks only lines that the candidate and its fixups remove and that the file of the parent
/// of the twin does not hold at all: the whole interdiff (`full_interdiffs`, no cut), then the files of the parents in
/// one `cat-file --batch`. A file that is gone there, binary or larger than `REMOVED_BEFORE_MAX_BYTES` proves nothing
fn removed_before<'a>(git_executor: &GitCommandExecutor, slot: &str, candidates: &[&'a LandedCandidate]) -> Result<Vec<&'a LandedCandidate>> {
  if candidates.is_empty() {
    return Ok(Vec::new());
  }
  let pairs: Vec<(Vec<&str>, &str)> = candidates.iter().map(|candidate| (candidate.unit().collect(), candidate.upstream.as_str())).collect();
  let interdiffs = full_interdiffs(git_executor, slot, &pairs)?;
  let names: Vec<String> = candidates
    .iter()
    .zip(&interdiffs)
    .flat_map(|(candidate, interdiff)| interdiff.local_only.iter().map(|path| format!("{}^:{}", candidate.upstream, path.path)))
    .collect();
  let mut texts = read_texts(git_executor, slot, &names, REMOVED_BEFORE_MAX_BYTES)?.into_iter();
  let mut proven = Vec::new();
  for (candidate, interdiff) in candidates.iter().zip(&interdiffs) {
    let mut gone = only_removals(&interdiff.local_only);
    for path in &interdiff.local_only {
      let text = texts.next().flatten();
      // by `\n` alone: a line of the patch keeps its `\r`
      gone &= text.is_some_and(|text| path.lines.iter().all(|line| !text.split('\n').any(|held| Some(held) == line.strip_prefix('-'))));
    }
    if gone {
      proven.push(*candidate);
    }
  }
  Ok(proven)
}
