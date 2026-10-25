//! The target of each `fixup!`, `squash!` and `amend!` commit of a range, with the rules of `git rebase --autosquash`
//! (`todo_list_rearrange_squash`). Pure, no git: the plan of a squash (`history_rewrite::autosquash::plan_autosquash`)
//! and the landed pairing (`landed_pairing`, a fixup landed with its target) read it.

use domain_types::commit::Commit;
use domain_types::stack::leaf_of;
use std::collections::HashMap;
use sync_utils::commit_subject::{first_marker, parse_subject};

/// How a commit of the range resolved, by position
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixupTarget {
  /// Not a fixup
  Commit,
  /// A fixup of the commit at this position (the final target of a chain)
  Fixup(usize),
  /// A fixup without a target among the older commits
  Orphan,
}

/// The target of each commit of `commits` (oldest first), by position: all leading markers are stripped, then the
/// target is the oldest older commit with
///   1. exactly that subject;
///   2. an id starting with it, if it is a hash prefix of at least 4 hex digits that matches one commit;
///   3. the same subject without the `(prefix)`, if the prefixes agree when the fixup names one;
///   4. a subject (or, prefixes agreeing, a subject without the `(prefix)`) starting with it.
///
/// A fixup of a fixup (by hash) goes to the final target. A fixup without a target is an orphan. With stacked branches
/// on (`stacks`), prefixes agree when they name the same branch, so `fixup! (login) X` finds `(auth/login) X`.
#[must_use]
pub fn fixup_targets(commits: &[Commit], stacks: bool) -> Vec<FixupTarget> {
  let mut resolutions: Vec<FixupTarget> = Vec::with_capacity(commits.len());
  // full subject -> oldest position
  let mut by_subject: HashMap<&str, usize> = HashMap::new();
  for (index, commit) in commits.iter().enumerate() {
    let resolution = match first_marker(&commit.subject) {
      None => FixupTarget::Commit,
      Some(_) => match find_target(commits, &by_subject, index, stacks) {
        None => FixupTarget::Orphan,
        Some(target) => match resolutions[target] {
          FixupTarget::Commit => FixupTarget::Fixup(target),
          FixupTarget::Fixup(final_target) => FixupTarget::Fixup(final_target),
          FixupTarget::Orphan => FixupTarget::Orphan,
        },
      },
    };
    resolutions.push(resolution);
    by_subject.entry(commit.subject.as_str()).or_insert(index);
  }
  resolutions
}

/// Fixup id -> the id of its target (`fixup_targets`)
#[must_use]
pub fn target_of_each_fixup(commits: &[Commit], stacks: bool) -> HashMap<String, String> {
  commits
    .iter()
    .zip(fixup_targets(commits, stacks))
    .filter_map(|(commit, resolution)| match resolution {
      FixupTarget::Fixup(target) => Some((commit.id.clone(), commits[target].id.clone())),
      FixupTarget::Commit | FixupTarget::Orphan => None,
    })
    .collect()
}

/// The position of the target of the fixup at `index`, among the older commits
fn find_target(commits: &[Commit], by_subject: &HashMap<&str, usize>, index: usize, stacks: bool) -> Option<usize> {
  let parsed = parse_subject(&commits[index].subject);
  let wanted = &commits[index].subject[parsed.autosquash.len()..];
  if wanted.is_empty() {
    return None;
  }
  if let Some(&target) = by_subject.get(wanted) {
    return Some(target);
  }

  let older = &commits[..index];
  if wanted.len() >= 4 && wanted.bytes().all(|b| b.is_ascii_hexdigit()) {
    let wanted = wanted.to_ascii_lowercase();
    let mut matches = older.iter().enumerate().filter(|(_, commit)| commit.id.starts_with(&wanted));
    if let (Some((target, _)), None) = (matches.next(), matches.next()) {
      return Some(target);
    }
  }

  // fixups are not targets of a subject match: their own subjects start with a marker
  let candidates = || {
    older
      .iter()
      .enumerate()
      .map(|(position, commit)| (position, parse_subject(&commit.subject)))
      .filter(|(_, candidate)| candidate.autosquash.is_empty())
  };
  let prefixes_agree = |candidate: Option<&str>| {
    parsed.prefix.is_none_or(|prefix| match candidate {
      Some(candidate) if stacks => leaf_of(prefix).is_some_and(|leaf| leaf_of(candidate).as_ref() == Some(&leaf)),
      candidate => Some(prefix) == candidate,
    })
  };
  if let Some((target, _)) = candidates().find(|(_, candidate)| candidate.rest == parsed.rest && prefixes_agree(candidate.prefix)) {
    return Some(target);
  }
  candidates()
    .find(|&(position, ref candidate)| {
      older[position].subject.starts_with(wanted) || (!parsed.rest.is_empty() && prefixes_agree(candidate.prefix) && candidate.rest.starts_with(parsed.rest))
    })
    .map(|(target, _)| target)
}
