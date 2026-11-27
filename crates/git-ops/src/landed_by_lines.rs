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

use crate::landed_pairing::{CandidateBy, ChangedLines, LandedCandidate, LandedPairing, LandedReads, NetLines, PathSets, net_of, read_changed_lines, same_net};
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
  let email_of: HashMap<&str, &str> = reads
    .local
    .iter()
    .map(|commit| (commit.id.as_str(), commit.author_email.as_str()))
    .chain(reads.upstream.iter().map(|commit| (commit.upstream.id.as_str(), commit.key.email.as_str())))
    .collect();
  let completed = pairing
    .candidates
    .iter()
    .enumerate()
    .filter(|(_, candidate)| candidate.by == CandidateBy::Key)
    .map(|(index, candidate)| Target {
      upstream: candidate.upstream.as_str(),
      email: email_of.get(candidate.commit.as_str()).copied().unwrap_or_default(),
      forced: candidate.unit().collect(),
      candidate: Some(index),
    });
  // newest first as the upstream is read: the oldest upstream commit first
  let untaken = reads
    .upstream
    .iter()
    .rev()
    .filter(|commit| !taken.contains(commit.upstream.id.as_str()))
    .map(|commit| Target {
      upstream: commit.upstream.id.as_str(),
      email: commit.key.email.as_str(),
      forced: Vec::new(),
      candidate: None,
    });

  let free: Vec<&str> = reads.local.iter().map(|commit| commit.id.as_str()).filter(|commit| !taken.contains(commit)).collect();
  let eligible = |target: &Target<'_>, free: &[&'_ str]| -> Vec<String> {
    let Some(wanted) = paths.get(target.upstream) else {
      return Vec::new();
    };
    free
      .iter()
      .filter(|commit| email_of.get(**commit) == Some(&target.email) && paths.get(**commit).is_some_and(|set| !set.is_disjoint(wanted)))
      .map(|commit| (*commit).to_string())
      .collect()
  };
  // a target whose paths its commits cannot cover is never equal: no diff of it
  let mut read: Vec<&str> = Vec::new();
  let mut seen: HashSet<&str> = HashSet::new();
  let targets: Vec<(Target<'_>, Vec<String>)> = completed
    .chain(untaken)
    .filter_map(|target| {
      let commits = eligible(&target, &free);
      let wanted = paths.get(target.upstream)?;
      let covered: BTreeSet<&Vec<u8>> = commits
        .iter()
        .map(String::as_str)
        .chain(target.forced.iter().copied())
        .filter_map(|commit| paths.get(commit))
        .flatten()
        .collect();
      (!commits.is_empty() && wanted.iter().all(|path| covered.contains(path))).then_some((target, commits))
    })
    .collect();
  for (target, commits) in &targets {
    for commit in target.forced.iter().copied().chain([target.upstream]) {
      if seen.insert(commit) {
        read.push(commit);
      }
    }
    for commit in commits {
      if seen.insert(commit.as_str()) {
        read.push(commit.as_str());
      }
    }
  }
  if targets.is_empty() {
    return Ok(pairing);
  }
  let changes = read_changed_lines(git_executor, repo_path, &read)?;
  let position: HashMap<&str, usize> = reads.local.iter().enumerate().map(|(index, commit)| (commit.id.as_str(), index)).collect();

  let mut found: Vec<(Option<usize>, LandedCandidate)> = Vec::new();
  let mut used: HashSet<String> = HashSet::new();
  for (target, commits) in &targets {
    let wanted = net_of(&changes, &[target.upstream]);
    if target.candidate.is_some() && same_net(&net_of(&changes, &target.forced), &wanted) {
      continue;
    }
    let commits: Vec<&str> = commits.iter().map(String::as_str).filter(|commit| !used.contains(*commit)).collect();
    let Some(set) = smallest_set(&changes, (&wanted, target.upstream), (&target.forced, &commits), paths, &position) else {
      continue;
    };
    let mut unit: Vec<&str> = target.forced.iter().copied().chain(set.iter().copied()).collect();
    unit.sort_by_key(|commit| position.get(commit).copied());
    used.extend(set.iter().map(|commit| (*commit).to_string()));
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
  debug!(found = found.len(), targets = targets.len(), "candidates by changed lines");
  for (replaced, candidate) in found {
    match replaced {
      Some(index) => pairing.candidates[index] = candidate,
      None => pairing.candidates.push(candidate),
    }
  }
  pairing.candidates.sort_by_key(|candidate| position.get(candidate.commit.as_str()).copied());
  Ok(pairing)
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
