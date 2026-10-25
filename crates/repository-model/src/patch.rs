//! The patches of a view (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): what changed between
//! two states of the table, by part (`changed`, `diff_keyed`), and the contract of applying a patch (`apply_patch`), which
//! the page's store keeps as well: a patch applies only to the view at its base, in the same epoch; any other view asks
//! for a snapshot. A lost patch costs one snapshot and never shows a wrong table.

use domain_types::commit::Commit;
use domain_types::session::{
  BranchChange, BranchRemote, BranchSync, BranchView, CommitSync, FactsChanges, NEWS_KEPT, PatronusChange, PatronusPart, PullRequestsChange, PullRequestsView, RepositoryView,
  RobotsChange, TableChanges, Version, ViewPatch,
};
use std::collections::BTreeMap;

/// Why a patch does not apply to a view; the window asks for a snapshot instead
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PatchRejected {
  /// The patch is of another session or process (another epoch)
  #[error("the patch of epoch {patch} does not apply to a view of epoch {view}")]
  ForeignEpoch { view: u32, patch: u32 },
  /// The view is not at the base of the patch: a patch was lost or came twice
  #[error("the patch from {base} does not apply to a view at {view}")]
  MissingBase { view: u32, base: u32 },
  /// The patch adds a branch without its head or its commits
  #[error("the patch adds the branch {0} without its head or its commits")]
  IncompleteBranch(String),
}

/// `now` when it differs from `last`
pub(crate) fn changed<T: PartialEq + Clone>(last: &T, now: &T) -> Option<T> {
  (last != now).then(|| now.clone())
}

/// `now` when a window would show other commits than `last`: the notes a sync reads with them are not part of a view
pub(crate) fn changed_commits(last: &[Commit], now: &[Commit]) -> Option<Vec<Commit>> {
  let same = last.len() == now.len() && last.iter().zip(now).all(|(last, now)| last.shows_as(now));
  (!same).then(|| now.to_vec())
}

/// The entries of `now` that `last` lacks or has otherwise, and the keys of `last` that `now` lacks
pub(crate) fn diff_keyed<T: PartialEq + Clone>(last: &BTreeMap<String, T>, now: &BTreeMap<String, T>) -> (BTreeMap<String, T>, Vec<String>) {
  let set = now
    .iter()
    .filter(|(key, value)| last.get(*key) != Some(*value))
    .map(|(key, value)| (key.clone(), value.clone()))
    .collect();
  let removed = last.keys().filter(|key| !now.contains_key(*key)).cloned().collect();
  (set, removed)
}

/// The parts of `now` that differ from `last`; for a branch `last` lacks, its head, its commits and the parts that differ
/// from the ones of a branch the sync has not reported yet; `None` when none differs
pub(crate) fn branch_change(last: Option<&BranchView>, now: &BranchView) -> Option<BranchChange> {
  let change = match last {
    None => BranchChange {
      head: Some(now.head.clone()),
      commits: Some(now.commits.clone()),
      sync: changed(&BranchSync::SYNCING, &now.sync),
      copies: changed(&vec![CommitSync::default(); now.commits.len()], &now.copies),
      remote: changed(&BranchRemote::default(), &now.remote),
    },
    Some(last) => BranchChange {
      head: changed(&last.head, &now.head),
      commits: changed_commits(&last.commits, &now.commits),
      sync: changed(&last.sync, &now.sync),
      copies: changed(&last.copies, &now.copies),
      remote: changed(&last.remote, &now.remote),
    },
  };
  (change != BranchChange::default()).then_some(change)
}

/// Applies `patch` to `view`, all of it or nothing
pub fn apply_patch(view: &mut RepositoryView, patch: ViewPatch) -> Result<(), PatchRejected> {
  check_base(view.version, &patch)?;
  let ViewPatch { next, changes, .. } = patch;
  let TableChanges {
    sync,
    header,
    order,
    branches,
    removed_branches,
    stacks,
    removed_stacks,
    unassigned,
    archived,
    unapplied,
    landed,
    patronus,
    facts,
    news,
  } = changes;
  // a branch the view lacks comes with its head and commits, also when the same patch removes it first
  for (name, change) in &branches {
    let known = view.branches.contains_key(name) && !removed_branches.contains(name);
    if !known && (change.head.is_none() || change.commits.is_none()) {
      return Err(PatchRejected::IncompleteBranch(name.clone()));
    }
  }

  view.version = next;
  set_if_some(&mut view.sync, sync);
  set_if_some(&mut view.header, header);
  set_if_some(&mut view.order, order);
  for name in &removed_branches {
    view.branches.remove(name);
  }
  for (name, change) in branches {
    apply_branch_change(&mut view.branches, name, change);
  }
  for root in &removed_stacks {
    view.stacks.remove(root);
  }
  view.stacks.extend(stacks);
  let sections = &mut view.sections;
  set_if_some(&mut sections.unassigned, unassigned);
  set_if_some(&mut sections.unapplied, unapplied);
  set_if_some(&mut sections.landed, landed);
  if let Some(archived) = archived {
    let section = &mut sections.archived;
    set_if_some(&mut section.names, archived.names);
    set_if_some(&mut section.newly_archived, archived.newly_archived);
    for name in &archived.removed_integration {
      section.integration.remove(name);
    }
    section.integration.extend(archived.integration);
  }
  if let Some(patronus) = patronus {
    apply_patronus_change(&mut view.patronus, patronus);
  }
  let FactsChanges {
    robots,
    pull_requests,
    rules,
    profiles,
    readiness,
    removed_readiness,
    robot_details,
    removed_robot_details,
  } = facts.unwrap_or_default();
  for key in &removed_readiness {
    view.facts.readiness.remove(key);
  }
  view.facts.readiness.extend(readiness);
  for id in &removed_robot_details {
    view.facts.robot_details.remove(id);
  }
  view.facts.robot_details.extend(robot_details);
  if let Some(pull_requests) = pull_requests {
    apply_pull_requests_change(&mut view.facts.pull_requests, pull_requests);
  }
  set_if_some(&mut view.facts.rules, rules);
  set_if_some(&mut view.facts.profiles, profiles);
  if let Some(robots) = robots {
    let RobotsChange {
      head,
      robots,
      removed_robots,
      failing,
      needs_review,
      problems_failed,
    } = robots;
    let view = &mut view.facts.robots;
    set_if_some(&mut view.head, head);
    for id in &removed_robots {
      view.robots.remove(id);
    }
    view.robots.extend(robots);
    set_if_some(&mut view.failing, failing);
    set_if_some(&mut view.needs_review, needs_review);
    set_if_some(&mut view.problems_failed, problems_failed);
  }
  view.news.extend(news);
  let excess = view.news.len().saturating_sub(NEWS_KEPT);
  view.news.drain(..excess);
  Ok(())
}

/// A patch applies only to the view at its base, in the same epoch
fn check_base(view: Version, patch: &ViewPatch) -> Result<(), PatchRejected> {
  if patch.base.epoch != view.epoch || patch.next.epoch != view.epoch {
    return Err(PatchRejected::ForeignEpoch {
      view: view.epoch,
      patch: patch.next.epoch,
    });
  }
  if patch.base != view || patch.next != view.next() {
    return Err(PatchRejected::MissingBase {
      view: view.seq,
      base: patch.base.seq,
    });
  }
  Ok(())
}

fn set_if_some<T>(target: &mut T, value: Option<T>) {
  if let Some(value) = value {
    *target = value;
  }
}

fn apply_branch_change(branches: &mut BTreeMap<String, BranchView>, name: String, change: BranchChange) {
  let BranchChange {
    head,
    commits,
    sync,
    copies,
    remote,
  } = change;
  match branches.get_mut(&name) {
    Some(branch) => {
      set_if_some(&mut branch.head, head);
      set_if_some(&mut branch.commits, commits);
      set_if_some(&mut branch.sync, sync);
      set_if_some(&mut branch.copies, copies);
      set_if_some(&mut branch.remote, remote);
    }
    // the head and the commits are checked by `apply_patch`
    None => {
      if let (Some(head), Some(commits)) = (head, commits) {
        let copies = copies.unwrap_or_else(|| vec![CommitSync::default(); commits.len()]);
        branches.insert(
          name,
          BranchView {
            head,
            commits,
            sync: sync.unwrap_or(BranchSync::SYNCING),
            copies,
            remote: remote.unwrap_or_default(),
          },
        );
      }
    }
  }
}

fn apply_patronus_change(part: &mut PatronusPart, change: PatronusChange) {
  let PatronusChange {
    state,
    landing_needs,
    branches,
    stacks,
    landed_by_robot,
    removed_branches,
    removed_stacks,
  } = change;
  set_if_some(&mut part.state, state);
  set_if_some(&mut part.landing_needs, landing_needs);
  let view = &mut part.view;
  for name in &removed_branches {
    view.branches.remove(name);
  }
  view.branches.extend(branches);
  for root in &removed_stacks {
    view.stacks.remove(root);
  }
  view.stacks.extend(stacks);
  set_if_some(&mut view.landed_by_robot, landed_by_robot);
}

/// The pull requests after a patch: its head when it names one, the pull requests it names replace theirs, those it
/// removes go
fn apply_pull_requests_change(view: &mut PullRequestsView, change: PullRequestsChange) {
  let PullRequestsChange {
    head,
    pull_requests,
    removed_pull_requests,
    stack_pull_requests,
    removed_stack_pull_requests,
  } = change;
  set_if_some(&mut view.head, head);
  for name in &removed_pull_requests {
    view.pull_requests.remove(name);
  }
  view.pull_requests.extend(pull_requests);
  for root in &removed_stack_pull_requests {
    view.stack_pull_requests.remove(root);
  }
  view.stack_pull_requests.extend(stack_pull_requests);
}
