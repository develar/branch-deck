//! The candidates by changed lines (openspec/specs/repository/landed-commits/spec.md, "The proof"): after a conflict, a
//! baseline commit of a local author pairs with the one smallest set of local commits whose changed lines without
//! context, summed in order, equal its own (`landed_pairing::same_net`): a commit with its fixups, or the commits of a
//! pull request that GitHub squashed into one commit under another identity, whose later commit rewrote the lines of an
//! earlier one. None of them alone carries the change of the squash, so neither a twin nor a patch id finds it.
//!
//! Never a proof: a candidate (`CandidateBy::ChangedLines`), left out only when the change of its unit is already in the
//! upstream or in the result (`landed_pairing::units_already_in`), and the round of each of its commits offers `--landed`.
//!
//! - the targets: the upstream commit of each candidate by identity that a later local commit may complete, then each
//!   upstream commit of the local authors that nothing took, oldest first
//! - the commits a set may hold: of the author of the target, nothing took them, their paths meet the target's, and a
//!   line of theirs is one of the target's or of another such commit (`closure`): one that changes no line of the rest
//!   can never cancel out of a sum
//! - one `diff-tree -p -U0` of them all, then the search in memory: the smallest sets first, at most
//!   `MAX_SEARCHED_COMMITS` commits; two sets of the same size make no candidate
//!
//! After a clean replay the same search names the squash of the local commits the replay left empty
//! (`squashes_of_gone`): no candidate then, the name of the baseline commit they landed as in the landed report. The
//! targets are the upstream commits of their authors that nothing else names, the sets hold only those commits, and a
//! set that another target's set shares a commit with names neither.

use crate::landed_pairing::{
  CandidateBy, ChangedLines, KeyedCommit, LandedCandidate, LandedPairing, LandedReads, NetLines, PathSets, changed_path_sets, net_of, read_changed_lines, same_net,
};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeSet, HashMap, HashSet};
use tracing::debug;

/// The most commits whose every subset the search of the smallest set tries: 2^16 sums of a few lines each
const MAX_SEARCHED_COMMITS: usize = 16;

/// A baseline commit to find the set of
struct Target<'a> {
  upstream: &'a str,
  email: &'a str,
  /// The commits every set of it holds: the unit of a candidate by identity, none else
  forced: Vec<&'a str>,
  /// The position in `LandedPairing::candidates` of the candidate by identity it completes
  candidate: Option<usize>,
}

/// `pairing` with the candidates by changed lines: each target of `reads` (an upstream commit by a local author) with the
/// one smallest set of the local commits nothing took whose changed lines equal its own. `paths` are the sets of paths
/// that the pairing before the replay read (`BeforeReplay`). One `diff-tree --stdin -p -U0`, none without a target whose
/// paths the local commits cover
pub(crate) fn with_candidates_by_changed_lines(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  reads: &LandedReads,
  paths: &PathSets,
  mut pairing: LandedPairing,
) -> Result<LandedPairing> {
  let mut taken: HashSet<&str> = pairing.drops.iter().flat_map(|drop| [drop.commit.as_str(), drop.upstream.as_str()]).collect();
  taken.extend(pairing.candidates.iter().flat_map(|candidate| candidate.unit().chain([candidate.upstream.as_str()])));
  let pool = Pool::of(
    reads,
    reads.local.iter().map(|commit| commit.id.as_str()).filter(|commit| !taken.contains(commit)).collect(),
  );
  let completed = pairing
    .candidates
    .iter()
    .enumerate()
    .filter(|(_, candidate)| candidate.by == CandidateBy::Key)
    .map(|(index, candidate)| Target {
      upstream: candidate.upstream.as_str(),
      email: pool.email_of.get(candidate.commit.as_str()).copied().unwrap_or_default(),
      forced: candidate.unit().collect(),
      candidate: Some(index),
    });
  // newest first as the upstream is read: the oldest upstream commit first
  let untaken = reads.upstream.iter().rev().filter(|commit| !taken.contains(commit.upstream.id.as_str())).map(Target::of);
  let targets: Vec<Target<'_>> = completed.chain(untaken).collect();
  for (replaced, candidate) in sets_of_targets(git_executor, repo_path, targets, &pool, paths, Shared::ToTheFirst)? {
    match replaced {
      Some(index) => pairing.candidates[index] = candidate,
      None => pairing.candidates.push(candidate),
    }
  }
  pairing.candidates.sort_by_key(|candidate| pool.position.get(candidate.commit.as_str()).copied());
  Ok(pairing)
}

/// The baseline commit each of the `gone` local commits of `reads` landed as, by local commit: the one upstream commit of
/// `reads` whose changed lines the smallest set of them sums to, by their author, that `named` does not hold (the baseline
/// commits that a proof, a candidate or an identity of a local commit names). A set names its baseline commit only when no
/// set of another one shares a commit with it: a change that the upstream made, reverted and made again names nothing.
/// `paths` are the sets of paths that the check read (`UpdatePlan::landed_paths`); without them one `diff-tree --stdin
/// --name-only` of the gone commits and the targets. Then one `diff-tree --stdin -p -U0` when the paths of a target are
/// covered; no process without a target
///
/// # Errors
/// Git failed
pub fn squashes_of_gone(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  reads: &LandedReads,
  gone: &[&str],
  named: &HashSet<&str>,
  paths: Option<&PathSets>,
) -> Result<HashMap<String, String>> {
  let gone_set: HashSet<&str> = gone.iter().copied().collect();
  let authors: HashSet<&str> = reads
    .local
    .iter()
    .filter(|commit| gone_set.contains(commit.id.as_str()))
    .map(|commit| commit.author_email.as_str())
    .collect();
  // newest first as the upstream is read: the oldest upstream commit first
  let targets: Vec<Target<'_>> = reads
    .upstream
    .iter()
    .rev()
    .filter(|commit| !named.contains(commit.upstream.id.as_str()) && authors.contains(commit.key.email.as_str()))
    .map(Target::of)
    .collect();
  if targets.is_empty() {
    return Ok(HashMap::new());
  }
  let read;
  let paths = if let Some(paths) = paths {
    paths
  } else {
    read = changed_path_sets(git_executor, repo_path, gone.iter().copied().chain(targets.iter().map(|target| target.upstream)))?;
    &read
  };
  let free = reads.local.iter().map(|commit| commit.id.as_str()).filter(|commit| gone_set.contains(commit)).collect();
  let pool = Pool::of(reads, free);
  let found = sets_of_targets(git_executor, repo_path, targets, &pool, paths, Shared::ToNone)?;
  Ok(
    found
      .iter()
      .flat_map(|(_, squash)| squash.unit().map(|commit| (commit.to_string(), squash.upstream.clone())))
      .collect(),
  )
}

/// Whether the sets of two targets may share a commit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shared {
  /// An earlier target takes the commits of its set: a later one searches without them
  ToTheFirst,
  /// Each target searches every commit of the pool, and a set that shares a commit with the set of another target is
  /// none
  ToNone,
}

impl<'a> Target<'a> {
  /// The upstream commit `commit` as a target of its own
  const fn of(commit: &'a KeyedCommit) -> Self {
    Self {
      upstream: commit.upstream.id.as_str(),
      email: commit.key.email.as_str(),
      forced: Vec::new(),
      candidate: None,
    }
  }
}

/// The local commits the sets of a search may hold, with what the search tells them by
struct Pool<'a> {
  /// The commits a set may hold, in the local order
  free: Vec<&'a str>,
  /// The author email of each local and upstream commit
  email_of: HashMap<&'a str, &'a str>,
  /// The index of each local commit in the local order
  position: HashMap<&'a str, usize>,
}

impl<'a> Pool<'a> {
  fn of(reads: &'a LandedReads, free: Vec<&'a str>) -> Self {
    Self {
      free,
      email_of: reads
        .local
        .iter()
        .map(|commit| (commit.id.as_str(), commit.author_email.as_str()))
        .chain(reads.upstream.iter().map(|commit| (commit.upstream.id.as_str(), commit.key.email.as_str())))
        .collect(),
      position: reads.local.iter().enumerate().map(|(index, commit)| (commit.id.as_str(), index)).collect(),
    }
  }
}

/// The one smallest set of `pool` of each of `targets`, in their order, as a candidate by changed lines with the position
/// of the candidate by identity it completes; two sets share a commit as `shared` says. A set holds
/// only commits of the author of its target that meet its paths, and a target whose paths they cannot cover has none.
/// One `diff-tree --stdin -p -U0` of the targets and their commits, none without such a target
fn sets_of_targets<'a>(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  targets: Vec<Target<'a>>,
  pool: &Pool<'a>,
  paths: &PathSets,
  shared: Shared,
) -> Result<Vec<(Option<usize>, LandedCandidate)>> {
  let eligible = |target: &Target<'_>| -> Vec<&'a str> {
    let Some(wanted) = paths.get(target.upstream) else {
      return Vec::new();
    };
    pool
      .free
      .iter()
      .copied()
      .filter(|commit| pool.email_of.get(*commit) == Some(&target.email) && paths.get(*commit).is_some_and(|set| !set.is_disjoint(wanted)))
      .collect()
  };
  // a target whose paths its commits cannot cover is never equal: no diff of it
  let targets: Vec<(Target<'a>, Vec<&'a str>)> = targets
    .into_iter()
    .filter_map(|target| {
      let commits = eligible(&target);
      let wanted = paths.get(target.upstream)?;
      let covered: BTreeSet<&Vec<u8>> = commits.iter().chain(&target.forced).filter_map(|commit| paths.get(*commit)).flatten().collect();
      (!commits.is_empty() && wanted.iter().all(|path| covered.contains(path))).then_some((target, commits))
    })
    .collect();
  if targets.is_empty() {
    return Ok(Vec::new());
  }
  let mut read: Vec<&str> = Vec::new();
  let mut seen: HashSet<&str> = HashSet::new();
  for (target, commits) in &targets {
    for commit in target.forced.iter().copied().chain([target.upstream]).chain(commits.iter().copied()) {
      if seen.insert(commit) {
        read.push(commit);
      }
    }
  }
  let changes = read_changed_lines(git_executor, repo_path, &read)?;

  let mut found: Vec<(Option<usize>, LandedCandidate)> = Vec::new();
  let mut used: HashSet<&str> = HashSet::new();
  let mut shared_commits: HashSet<&str> = HashSet::new();
  for (target, commits) in &targets {
    let wanted = net_of(&changes, &[target.upstream]);
    if target.candidate.is_some() && same_net(&net_of(&changes, &target.forced), &wanted) {
      continue;
    }
    let commits: Vec<&str> = commits.iter().copied().filter(|commit| shared == Shared::ToNone || !used.contains(commit)).collect();
    let Some(set) = smallest_set(&changes, (&wanted, target.upstream), (&target.forced, &commits), paths, &pool.position) else {
      continue;
    };
    let mut unit: Vec<&str> = target.forced.iter().copied().chain(set.iter().copied()).collect();
    unit.sort_by_key(|commit| pool.position.get(commit).copied());
    for commit in set {
      // a commit in two sets marks both
      if !used.insert(commit) {
        shared_commits.insert(commit);
      }
    }
    found.push((
      target.candidate,
      LandedCandidate {
        commit: unit[0].to_string(),
        upstream: target.upstream.to_string(),
        landed_with: unit[1..].iter().map(|commit| (*commit).to_string()).collect(),
        by: CandidateBy::ChangedLines,
      },
    ));
  }
  if !shared_commits.is_empty() {
    debug!(shared = shared_commits.len(), "sets of two baseline commits share a commit: neither names it");
    found.retain(|(_, candidate)| candidate.unit().all(|commit| !shared_commits.contains(commit)));
  }
  debug!(found = found.len(), targets = targets.len(), "candidates by changed lines");
  Ok(found)
}

/// The one smallest set of `commits` that, with `forced`, sums to `wanted` (the net lines of `upstream`) in the local
/// order (`position`); `None` when no set does or two of the same size do. Only the commits of the `closure` of
/// `upstream` take part; past `MAX_SEARCHED_COMMITS` of them only those within the paths of `upstream`, and past it
/// again only all of them at once
fn smallest_set<'a>(
  changes: &ChangedLines,
  (wanted, upstream): (&NetLines, &str),
  (forced, commits): (&[&'a str], &[&'a str]),
  paths: &PathSets,
  position: &HashMap<&str, usize>,
) -> Option<Vec<&'a str>> {
  let mut pool = closure(changes, upstream, forced, commits);
  if !covers_lines(changes, wanted, forced, &pool) {
    return None;
  }
  if pool.len() > MAX_SEARCHED_COMMITS {
    let within = paths.get(upstream);
    pool.retain(|commit| paths.get(*commit).zip(within).is_some_and(|(set, within)| set.is_subset(within)));
  }
  let sum_of = |set: &[&str]| -> NetLines {
    let mut unit: Vec<&str> = forced.iter().chain(set).copied().collect();
    unit.sort_by_key(|commit| position.get(commit).copied());
    net_of(changes, &unit)
  };
  if pool.len() > MAX_SEARCHED_COMMITS {
    debug!(commits = pool.len(), %upstream, "too many commits for the smallest set: only all of them");
    return same_net(&sum_of(&pool), wanted).then_some(pool);
  }
  let count = pool.len();
  for size in usize::from(forced.is_empty())..=count {
    let mut matches = (0u32..1 << count)
      .filter(|mask| mask.count_ones() as usize == size)
      .map(|mask| {
        pool
          .iter()
          .enumerate()
          .filter(|(index, _)| mask & (1 << index) != 0)
          .map(|(_, commit)| *commit)
          .collect::<Vec<_>>()
      })
      .filter(|set| same_net(&sum_of(set), wanted));
    let Some(first) = matches.next() else {
      continue;
    };
    if matches.next().is_some() {
      debug!(%upstream, size, "two sets of commits sum to the upstream commit: no candidate");
      return None;
    }
    return Some(first);
  }
  None
}

/// A changed line of `changes` (or a line of a `PathNet`) without its sign or its count, as two commits share it: the
/// text of `+<line>` and `-<line>`, `binary` and `mode` of a path
fn line_key(line: &str) -> &str {
  if line.starts_with("binary ") {
    "binary"
  } else if line.starts_with("mode ") {
    "mode"
  } else {
    line.get(1..).unwrap_or_default()
  }
}

/// The keys (path, `line_key`) of the changed lines of a commit
fn keys_of<'c>(changes: &'c ChangedLines, commit: &str) -> HashSet<(&'c str, &'c str)> {
  changes
    .get(commit)
    .into_iter()
    .flatten()
    .flat_map(|(path, lines)| lines.iter().map(move |line| (path.as_str(), line_key(line))))
    .collect()
}

/// The commits of `commits` (in their order) that change a line `upstream` or `forced` changes, or one that another such
/// commit changes: a commit outside of it changes only lines that nothing else of a set changes, so a sum with it is
/// never equal
fn closure<'a>(changes: &ChangedLines, upstream: &str, forced: &[&str], commits: &[&'a str]) -> Vec<&'a str> {
  let mut keys = keys_of(changes, upstream);
  for commit in forced {
    keys.extend(keys_of(changes, commit));
  }
  let mut joined = vec![false; commits.len()];
  loop {
    let mut grew = false;
    for (index, commit) in commits.iter().enumerate() {
      let own = keys_of(changes, commit);
      if !joined[index] && !own.is_disjoint(&keys) {
        joined[index] = true;
        keys.extend(own);
        grew = true;
      }
    }
    if !grew {
      break;
    }
  }
  commits.iter().zip(joined).filter(|(_, joined)| *joined).map(|(commit, _)| *commit).collect()
}

/// Whether every line `wanted` changes is a line `forced` or `pool` changes: else no set sums to it
fn covers_lines(changes: &ChangedLines, wanted: &NetLines, forced: &[&str], pool: &[&str]) -> bool {
  let keys: HashSet<(&str, &str)> = forced.iter().chain(pool).flat_map(|commit| keys_of(changes, commit)).collect();
  wanted.iter().all(|(path, net)| net.lines().all(|(key, _)| keys.contains(&(path.as_str(), line_key(key)))))
}
