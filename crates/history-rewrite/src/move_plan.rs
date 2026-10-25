//! The plan of a move, "these commits go to branch X", as the smallest regroup (spec/branch/move-commits.spec.md). A
//! whole issue, every commit of its key without a prefix, moves by the issue groups without a rewrite, so the next
//! commit of the issue follows it, also into a stacked branch; a part of an issue and a commit without a key get the
//! prefix `(X)`; a commit that lands in X without its prefix loses it. With stacked branches on, a path `P/X` stacks X
//! on P in the stack parents (`regroup.rs`), so its prefix is the bare `(X)`.

use crate::autosquash::plan_autosquash;
use crate::commit_ids::{ResolvedIds, resolve_commit_ids};
use crate::regroup::{IssueGroupEntries, RegroupError, RegroupPlan};
use crate::snapshot::Snapshot;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use sync_utils::commit_subject::parse_subject;
use sync_utils::issue_pattern::find_issue_number;

/// Where a move goes
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MoveTarget {
  /// The prefix path, as given
  pub path: String,
  /// The branch afterwards: the leaf of the path with stacked branches on
  pub leaf: String,
  /// Only a prefix puts commits there: a flat name with `/` with stacked branches off (an issue group name has none)
  pub prefix_only: bool,
}

pub(crate) fn move_target(snapshot: &Snapshot, name: &str) -> MoveTarget {
  let stacks = snapshot.grouping.stacks;
  let leaf = if stacks { name.rsplit('/').next().unwrap_or(name) } else { name };
  MoveTarget {
    path: name.to_string(),
    leaf: leaf.to_string(),
    prefix_only: !stacks && name.contains('/'),
  }
}

/// The regroup that moves `ids` to `target`, or back (`None`): out of their prefix branch, or out of the issue group of
/// their key. The fixups of a moved commit move with it
pub fn plan_move(snapshot: &Snapshot, ids: &[String], target: Option<&str>) -> Result<RegroupPlan, RegroupError> {
  let mut moved = resolve_ids(snapshot, ids)?;
  let autosquash = plan_autosquash(snapshot.head.clone(), &snapshot.commits, &snapshot.grouping);
  let fixups: Vec<String> = autosquash
    .groups
    .iter()
    .filter(|group| moved.contains(&group.target))
    .flat_map(|group| group.folded.iter().map(|fixup| fixup.commit_id.clone()))
    .collect();
  moved.extend(fixups);

  // the commits of each key without a prefix: they are in one branch, the issue group of the key or its issue branch
  let mut populations: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
  for commit in &snapshot.commits {
    let parsed = parse_subject(&commit.subject);
    if parsed.prefix.is_none()
      && let Some(key) = find_issue_number(parsed.rest)
    {
      populations.entry(key).or_default().push(&commit.id);
    }
  }
  let whole: BTreeSet<&str> = populations
    .iter()
    .filter(|(_, commits)| commits.iter().all(|id| moved.contains(*id)))
    .map(|(key, _)| *key)
    .collect();

  let entries = &snapshot.grouping.issue_groups.entries;
  let moved_commits = snapshot.commits.iter().filter(|commit| moved.contains(&commit.id));
  let mut plan = RegroupPlan::default();
  let Some(target) = target.map(|name| move_target(snapshot, name)) else {
    for commit in moved_commits {
      let parsed = parse_subject(&commit.subject);
      let mapped = find_issue_number(parsed.rest).filter(|key| entries.contains_key(*key));
      match (parsed.prefix, mapped) {
        (Some(_), _) => plan.unassign.push(commit.id.clone()),
        (None, Some(key)) if whole.contains(key) => {
          if !plan.ungroup_issues.iter().any(|it| it == key) {
            plan.ungroup_issues.push(key.to_string());
          }
        }
        // a part of an issue group goes back to the issue branch of its key by a prefix
        (None, Some(key)) => plan.branches.entry(key.to_string()).or_default().push(commit.id.clone()),
        (None, None) => {}
      }
    }
    return Ok(plan);
  };

  let mut after: IssueGroupEntries = entries.clone();
  if snapshot.grouping.stacks && target.path.contains('/') {
    // the path writes its stack parents, also when the issue groups move every commit
    plan.branches.entry(target.path.clone()).or_default();
  }
  if !target.prefix_only {
    for key in &whole {
      if entries.get(*key).map_or(*key, String::as_str) == target.leaf {
        continue;
      }
      if *key == target.leaf {
        // back to its own issue branch
        plan.ungroup_issues.push((*key).to_string());
        after.remove(*key);
      } else {
        plan.issue_groups.entry(target.leaf.clone()).or_default().push((*key).to_string());
        after.insert((*key).to_string(), target.leaf.clone());
      }
    }
  }
  for commit in moved_commits {
    let parsed = parse_subject(&commit.subject);
    let lands = find_issue_number(parsed.rest).map(|key| after.get(key).map_or(key, String::as_str));
    if !target.prefix_only && lands == Some(target.leaf.as_str()) {
      // its key puts it there, the prefix would only repeat it
      if parsed.prefix.is_some() {
        plan.unassign.push(commit.id.clone());
      }
      continue;
    }
    // a path names its parent in the stack parents, so a commit already in the branch stays as it is
    if snapshot.groups.get(&commit.id).is_some_and(|group| group.as_deref() == Some(target.leaf.as_str())) {
      continue;
    }
    plan.branches.entry(target.path.clone()).or_default().push(commit.id.clone());
  }
  Ok(plan)
}

/// The full ids of the request: full or abbreviated ids of commits ahead of the baseline
fn resolve_ids(snapshot: &Snapshot, ids: &[String]) -> Result<HashSet<String>, RegroupError> {
  let candidates: Vec<&str> = snapshot.commits.iter().map(|commit| commit.id.as_str()).collect();
  let ResolvedIds { full, unknown, ambiguous } = resolve_commit_ids(ids, &candidates);
  if !ambiguous.is_empty() {
    return Err(RegroupError::InvalidPlan {
      message: format!("Ambiguous commit ids: {}", ambiguous.join(", ")),
      commits: ambiguous,
    });
  }
  if !unknown.is_empty() {
    return Err(RegroupError::NotAheadOfBaseline { commits: unknown });
  }
  Ok(full)
}
