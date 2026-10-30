//! The patches of a view (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): what changed between
//! two states of the table, by part (`changes` of the parts as they are now, `Now`, over `changed`, `diff_keyed` and
//! `branch_change`), between two views (`diff_views`: the catch-up of a window that holds the seed of a session, by the
//! same comparisons), and the contract of applying a patch (`apply_patch`), which the page's store keeps as well: a
//! patch applies only to the view at its base, in the same epoch, and brings it to a newer version (a live patch to
//! the next one, a catch-up over several); any other view asks for a snapshot. A lost patch costs one snapshot and
//! never shows a wrong table.

use domain_types::actions::ActionState;
use domain_types::commit::Commit;
use domain_types::landing::{LandDecisions, Landed};
use domain_types::landing_route::LandingRoute;
use domain_types::next_step::NextStepView;
use domain_types::operation::{OperationId, RunningOperation};
use domain_types::pull_request::PullRequestView;
use domain_types::session::{
  ArchivedChanges, BranchChange, BranchPush, BranchRemote, BranchStructure, BranchSync, BranchView, CommitSync, Fact, FactsChanges, FactsView, GitHubLayer, NEWS_KEPT, News,
  PatronusChange, PatronusPart, PullRequestsChange, PullRequestsView, RepositoryView, ReviewsView, RobotAttention, RobotsChange, RobotsView, SyncState, TableChanges, TableHeader,
  TableSections, Version, ViewPatch,
};
use domain_types::stack::StackBranchUpdate;
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
      copies: (!unsynced(now)).then(|| now.copies.clone()),
      remote: changed(&BranchRemote::default(), &now.remote),
      structure: changed(&BranchStructure::default(), &now.structure),
    },
    Some(last) => BranchChange {
      head: changed(&last.head, &now.head),
      commits: changed_commits(&last.commits, &now.commits),
      sync: changed(&last.sync, &now.sync),
      copies: changed(&last.copies, &now.copies),
      remote: changed(&last.remote, &now.remote),
      structure: changed(&last.structure, &now.structure),
    },
  };
  (change != BranchChange::default()).then_some(change)
}

/// Whether the copies of `branch` are the ones a branch the sync has not reported yet has: one default per commit, which
/// a window fills in for a branch whose patch carries none (`apply_branch_change`)
fn unsynced(branch: &BranchView) -> bool {
  branch.copies.len() == branch.commits.len() && branch.copies.iter().all(|copy| *copy == CommitSync::default())
}

/// Applies `patch` to `view`, all of it or nothing
pub fn apply_patch(view: &mut RepositoryView, patch: ViewPatch) -> Result<(), PatchRejected> {
  check_base(view.version, &patch)?;
  let ViewPatch { next, changes, .. } = patch;
  apply_changes(view, next, changes)
}

/// Applies `changes` to `view`, which is at `next` after them, all of them or nothing: the patch of `apply_patch` once
/// its base is checked, and the view of a seed (`state::View::seeded`), which keeps the first version of its epoch
pub(crate) fn apply_changes(view: &mut RepositoryView, next: Version, changes: TableChanges) -> Result<(), PatchRejected> {
  let TableChanges {
    cached,
    sync,
    header,
    order,
    branches,
    removed_branches,
    stacks,
    removed_stacks,
    pushes,
    removed_pushes,
    open_pull_requests,
    removed_open_pull_requests,
    lands,
    removed_lands,
    landed_states,
    removed_landed_states,
    next_steps,
    removed_next_steps,
    actions,
    removed_actions,
    attention,
    removed_attention,
    github_layers,
    removed_github_layers,
    operations,
    removed_operations,
    undo,
    unassigned,
    archived,
    unapplied,
    landed,
    reviews,
    patronus,
    route,
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
  set_if_some(&mut view.cached, cached);
  set_if_some(&mut view.sync, sync);
  set_if_some(&mut view.header, header);
  set_if_some(&mut view.order, order);
  for name in &removed_branches {
    view.branches.remove(name);
  }
  let added = branches
    .into_iter()
    .filter_map(|(name, change)| apply_branch_change(&mut view.branches, &name, change).map(|branch| (name, branch)))
    .collect();
  apply_keyed(&mut view.branches, added, &[]);
  apply_keyed(&mut view.stacks, stacks, &removed_stacks);
  apply_keyed(&mut view.pushes, pushes, &removed_pushes);
  apply_keyed(&mut view.open_pull_requests, open_pull_requests, &removed_open_pull_requests);
  apply_keyed(&mut view.lands, lands, &removed_lands);
  apply_keyed(&mut view.landed_states, landed_states, &removed_landed_states);
  apply_keyed(&mut view.next_steps, next_steps, &removed_next_steps);
  apply_keyed(&mut view.actions, actions, &removed_actions);
  apply_keyed(&mut view.attention, attention, &removed_attention);
  apply_keyed(&mut view.github_layers, github_layers, &removed_github_layers);
  apply_keyed(&mut view.operations, operations, &removed_operations);
  set_if_some(&mut view.undo, undo);
  set_if_some(&mut view.reviews, reviews);
  set_if_some(&mut view.route, route);
  let sections = &mut view.sections;
  set_if_some(&mut sections.unassigned, unassigned);
  set_if_some(&mut sections.unapplied, unapplied);
  set_if_some(&mut sections.landed, landed);
  if let Some(archived) = archived {
    let section = &mut sections.archived;
    set_if_some(&mut section.names, archived.names);
    set_if_some(&mut section.newly_archived, archived.newly_archived);
    apply_keyed(&mut section.integration, archived.integration, &archived.removed_integration);
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
    review_requests,
    auth,
  } = facts.unwrap_or_default();
  set_if_some(&mut view.facts.review_requests, review_requests);
  set_if_some(&mut view.facts.auth, auth);
  apply_keyed(&mut view.facts.readiness, readiness, &removed_readiness);
  apply_keyed(&mut view.facts.robot_details, robot_details, &removed_robot_details);
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
    apply_keyed(&mut view.robots, robots, &removed_robots);
    set_if_some(&mut view.failing, failing);
    set_if_some(&mut view.needs_review, needs_review);
    set_if_some(&mut view.problems_failed, problems_failed);
  }
  view.news.extend(news);
  let excess = view.news.len().saturating_sub(NEWS_KEPT);
  view.news.drain(..excess);
  Ok(())
}

/// A copy of the parts of `changes` that the table and the facts hold, without the parts the view derives of them (the
/// rules' answers: the pushes, the open pull requests, the starts, what landed, the next steps, the actions, the
/// attention, the reviews and the route), which `state::View` moves into the view it keeps instead of copying them
pub(crate) fn kept_parts(changes: &TableChanges) -> TableChanges {
  let TableChanges {
    cached,
    sync,
    header,
    order,
    branches,
    removed_branches,
    stacks,
    removed_stacks,
    pushes: _,
    removed_pushes: _,
    open_pull_requests: _,
    removed_open_pull_requests: _,
    lands: _,
    removed_lands: _,
    landed_states: _,
    removed_landed_states: _,
    next_steps: _,
    removed_next_steps: _,
    actions: _,
    removed_actions: _,
    attention: _,
    removed_attention: _,
    github_layers: _,
    removed_github_layers: _,
    operations,
    removed_operations,
    undo,
    unassigned,
    archived,
    unapplied,
    landed,
    reviews: _,
    patronus,
    route: _,
    facts,
    news,
  } = changes;
  TableChanges {
    cached: *cached,
    sync: sync.clone(),
    header: header.clone(),
    order: order.clone(),
    branches: branches.clone(),
    removed_branches: removed_branches.clone(),
    stacks: stacks.clone(),
    removed_stacks: removed_stacks.clone(),
    operations: operations.clone(),
    removed_operations: removed_operations.clone(),
    undo: undo.clone(),
    unassigned: unassigned.clone(),
    archived: archived.clone(),
    unapplied: unapplied.clone(),
    landed: landed.clone(),
    patronus: patronus.clone(),
    facts: facts.clone(),
    news: news.clone(),
    ..TableChanges::default()
  }
}

/// A patch applies only to the view at its base, in the same epoch, and brings it to a newer version: the next one, or
/// a later one of a catch-up (`diff_views`)
fn check_base(view: Version, patch: &ViewPatch) -> Result<(), PatchRejected> {
  if patch.base.epoch != view.epoch || patch.next.epoch != view.epoch {
    return Err(PatchRejected::ForeignEpoch {
      view: view.epoch,
      patch: patch.next.epoch,
    });
  }
  if patch.base != view || patch.next.seq <= view.seq {
    return Err(PatchRejected::MissingBase {
      view: view.seq,
      base: patch.base.seq,
    });
  }
  Ok(())
}

/// `map` without the keys `removed`, with the entries of `set`; a map left empty takes `set` as it is (the first patch of
/// a view, a part that had nothing)
fn apply_keyed<K: Ord, V>(map: &mut BTreeMap<K, V>, set: BTreeMap<K, V>, removed: &[K]) {
  for key in removed {
    map.remove(key);
  }
  if map.is_empty() {
    *map = set;
  } else {
    map.extend(set);
  }
}

fn set_if_some<T>(target: &mut T, value: Option<T>) {
  if let Some(value) = value {
    *target = value;
  }
}

/// `change` applied to the branch `name` of `branches`; of a branch `branches` lacks, the branch it adds
fn apply_branch_change(branches: &mut BTreeMap<String, BranchView>, name: &str, change: BranchChange) -> Option<BranchView> {
  let BranchChange {
    head,
    commits,
    sync,
    copies,
    remote,
    structure,
  } = change;
  let Some(branch) = branches.get_mut(name) else {
    // the head and the commits are checked by `apply_patch`
    let (head, commits) = (head?, commits?);
    let copies = copies.unwrap_or_else(|| vec![CommitSync::default(); commits.len()]);
    return Some(BranchView {
      head,
      commits,
      sync: sync.unwrap_or(BranchSync::SYNCING),
      copies,
      remote: remote.unwrap_or_default(),
      structure: structure.unwrap_or_default(),
    });
  };
  set_if_some(&mut branch.head, head);
  set_if_some(&mut branch.commits, commits);
  set_if_some(&mut branch.sync, sync);
  set_if_some(&mut branch.copies, copies);
  set_if_some(&mut branch.remote, remote);
  set_if_some(&mut branch.structure, structure);
  None
}

fn apply_patronus_change(part: &mut PatronusPart, change: PatronusChange) {
  let PatronusChange {
    state,
    landing_needs,
    branches,
    stacks,
    landed_by_robot,
    restarts,
    removed_branches,
    removed_stacks,
  } = change;
  set_if_some(&mut part.state, state);
  set_if_some(&mut part.landing_needs, landing_needs);
  let view = &mut part.view;
  apply_keyed(&mut view.branches, branches, &removed_branches);
  apply_keyed(&mut view.stacks, stacks, &removed_stacks);
  set_if_some(&mut view.landed_by_robot, landed_by_robot);
  set_if_some(&mut view.restarts, restarts);
}

/// The pull requests after a patch: its head when it names one, with the repository and the permission of that answer,
/// the pull requests it names replace theirs, those it removes go
fn apply_pull_requests_change(view: &mut PullRequestsView, change: PullRequestsChange) {
  let PullRequestsChange {
    head,
    repository,
    viewer_permission,
    merge_methods,
    pull_requests,
    removed_pull_requests,
    stack_pull_requests,
    removed_stack_pull_requests,
  } = change;
  if let Some(head) = head {
    view.head = head;
    view.repository = repository;
    view.viewer_permission = viewer_permission;
    view.merge_methods = merge_methods;
  }
  apply_keyed(&mut view.pull_requests, pull_requests, &removed_pull_requests);
  apply_keyed(&mut view.stack_pull_requests, stack_pull_requests, &removed_stack_pull_requests);
}

/// The parts of a view as they are now, borrowed, which `changes` compares with a view: of a session, its table state
/// and what it derived of it and its facts (`state::View::diff`); of another view, that view (`diff_views`)
pub(crate) struct Now<'a> {
  pub(crate) cached: &'a bool,
  pub(crate) sync: &'a SyncState,
  pub(crate) header: &'a TableHeader,
  pub(crate) order: &'a Vec<String>,
  pub(crate) branches: &'a BTreeMap<String, BranchView>,
  pub(crate) stacks: &'a BTreeMap<String, StackBranchUpdate>,
  pub(crate) sections: &'a TableSections,
  /// `None` keeps the ones the view has (a running sync decides none)
  pub(crate) pushes: Option<&'a BTreeMap<String, BranchPush>>,
  pub(crate) open_pull_requests: &'a BTreeMap<String, PullRequestView>,
  /// `None` keeps the ones the view has, as `pushes`
  pub(crate) lands: Option<&'a BTreeMap<String, LandDecisions>>,
  /// `None` keeps the ones the view has, as `pushes`
  pub(crate) landed_states: Option<&'a BTreeMap<String, Landed>>,
  pub(crate) next_steps: &'a BTreeMap<String, NextStepView>,
  pub(crate) actions: &'a BTreeMap<String, Vec<ActionState>>,
  pub(crate) attention: &'a BTreeMap<String, RobotAttention>,
  pub(crate) github_layers: &'a BTreeMap<String, GitHubLayer>,
  /// In the order they were admitted
  pub(crate) operations: Vec<&'a RunningOperation>,
  pub(crate) undo: Option<&'a OperationId>,
  pub(crate) reviews: &'a ReviewsView,
  pub(crate) patronus: &'a PatronusPart,
  pub(crate) route: &'a LandingRoute,
  /// What changed of the facts, which the caller compares by what it holds them in
  pub(crate) facts: Option<FactsChanges>,
  /// Oldest first
  pub(crate) news: &'a [News],
}

/// The changes from `last` to `now`, by part: only the parts that differ, each cloned once
pub(crate) fn changes(last: &RepositoryView, now: Now<'_>) -> TableChanges {
  let branches = now
    .branches
    .iter()
    .filter_map(|(name, branch)| Some((name.clone(), branch_change(last.branches.get(name), branch)?)))
    .collect();
  let removed_branches = last.branches.keys().filter(|name| !now.branches.contains_key(*name)).cloned().collect();
  let (stacks, removed_stacks) = diff_keyed(&last.stacks, now.stacks);
  let (pushes, removed_pushes) = now.pushes.map_or_else(|| (BTreeMap::new(), Vec::new()), |pushes| diff_keyed(&last.pushes, pushes));
  let (open_pull_requests, removed_open_pull_requests) = diff_keyed(&last.open_pull_requests, now.open_pull_requests);
  let (lands, removed_lands) = now.lands.map_or_else(|| (BTreeMap::new(), Vec::new()), |lands| diff_keyed(&last.lands, lands));
  let (landed_states, removed_landed_states) = now
    .landed_states
    .map_or_else(|| (BTreeMap::new(), Vec::new()), |landed| diff_keyed(&last.landed_states, landed));
  let (next_steps, removed_next_steps) = diff_keyed(&last.next_steps, now.next_steps);
  let (actions, removed_actions) = diff_keyed(&last.actions, now.actions);
  let (attention, removed_attention) = diff_keyed(&last.attention, now.attention);
  let (github_layers, removed_github_layers) = diff_keyed(&last.github_layers, now.github_layers);
  let (operations, removed_operations) = operations_change(&last.operations, &now.operations);
  let (last_archived, archived) = (&last.sections.archived, &now.sections.archived);
  let (integration, removed_integration) = diff_keyed(&last_archived.integration, &archived.integration);
  let archived_changes = ArchivedChanges {
    names: changed(&last_archived.names, &archived.names),
    newly_archived: changed(&last_archived.newly_archived, &archived.newly_archived),
    integration,
    removed_integration,
  };
  TableChanges {
    cached: changed(&last.cached, now.cached),
    sync: changed(&last.sync, now.sync),
    header: changed(&last.header, now.header),
    order: changed(&last.order, now.order),
    branches,
    removed_branches,
    stacks,
    removed_stacks,
    pushes,
    removed_pushes,
    open_pull_requests,
    removed_open_pull_requests,
    lands,
    removed_lands,
    landed_states,
    removed_landed_states,
    next_steps,
    removed_next_steps,
    actions,
    removed_actions,
    attention,
    removed_attention,
    github_layers,
    removed_github_layers,
    operations,
    removed_operations,
    undo: (last.undo.as_ref() != now.undo).then(|| now.undo.cloned()),
    unassigned: changed_commits(&last.sections.unassigned, &now.sections.unassigned),
    archived: (archived_changes != ArchivedChanges::default()).then_some(archived_changes),
    unapplied: changed(&last.sections.unapplied, &now.sections.unapplied),
    landed: changed(&last.sections.landed, &now.sections.landed),
    reviews: changed(&last.reviews, now.reviews),
    patronus: patronus_change(&last.patronus, now.patronus),
    route: changed(&last.route, now.route),
    facts: now.facts,
    news: new_news(&last.news, now.news),
  }
}

/// The patch from the view `from` to the view `to` of the same session (a later version of its epoch), by the
/// comparisons of the live patches: `apply_patch` of it brings `from` to `to`. A window that holds the seed of a session
/// (`state::View::seeded`) catches up by it to the view the session has sent since
#[must_use]
pub fn diff_views(from: &RepositoryView, to: &RepositoryView) -> ViewPatch {
  let now = Now {
    cached: &to.cached,
    sync: &to.sync,
    header: &to.header,
    order: &to.order,
    branches: &to.branches,
    stacks: &to.stacks,
    sections: &to.sections,
    pushes: Some(&to.pushes),
    open_pull_requests: &to.open_pull_requests,
    lands: Some(&to.lands),
    landed_states: Some(&to.landed_states),
    next_steps: &to.next_steps,
    actions: &to.actions,
    attention: &to.attention,
    github_layers: &to.github_layers,
    operations: to.operations.values().collect(),
    undo: to.undo.as_ref(),
    reviews: &to.reviews,
    patronus: &to.patronus,
    route: &to.route,
    facts: facts_view_change(&from.facts, &to.facts),
    news: &to.news,
  };
  ViewPatch {
    base: from.version,
    next: to.version,
    changes: changes(from, now),
  }
}

/// What changed of the facts from one view to another, by part
fn facts_view_change(from: &FactsView, to: &FactsView) -> Option<FactsChanges> {
  let (readiness, removed_readiness) = keyed_facts_change(&from.readiness, to.readiness.iter(), |key| to.readiness.contains_key(key));
  let (robot_details, removed_robot_details) = keyed_facts_change(&from.robot_details, to.robot_details.iter(), |id| to.robot_details.contains_key(id));
  let change = FactsChanges {
    robots: robots_change(&from.robots, &to.robots),
    pull_requests: pull_requests_change(&from.pull_requests, &to.pull_requests),
    rules: changed(&from.rules, &to.rules),
    profiles: changed(&from.profiles, &to.profiles),
    readiness,
    removed_readiness,
    robot_details,
    removed_robot_details,
    review_requests: changed(&from.review_requests, &to.review_requests),
    auth: changed(&from.auth, &to.auth),
  };
  (change != FactsChanges::default()).then_some(change)
}

/// The operations of `now` that `last` lacks or has otherwise, and the ids of the ones that ended
fn operations_change(last: &BTreeMap<OperationId, RunningOperation>, now: &[&RunningOperation]) -> (BTreeMap<OperationId, RunningOperation>, Vec<OperationId>) {
  let changed = now
    .iter()
    .filter(|operation| last.get(&operation.id) != Some(**operation))
    .map(|operation| (operation.id.clone(), (*operation).clone()))
    .collect();
  let removed = last.keys().filter(|id| !now.iter().any(|operation| operation.id == **id)).cloned().collect();
  (changed, removed)
}

/// Of a part of facts by key (the readiness by subject, the details by robot), the facts of `now` that changed and the
/// keys `now` has no longer (`contains`): only those are cloned
pub(crate) fn keyed_facts_change<'a, T: PartialEq + Clone + 'a>(
  last: &BTreeMap<String, Fact<T>>,
  now: impl Iterator<Item = (&'a String, &'a Fact<T>)>,
  contains: impl Fn(&str) -> bool,
) -> (BTreeMap<String, Fact<T>>, Vec<String>) {
  let changed = now
    .filter(|(key, fact)| last.get(*key) != Some(*fact))
    .map(|(key, fact)| (key.clone(), fact.clone()))
    .collect();
  let removed = last.keys().filter(|key| !contains(key)).cloned().collect();
  (changed, removed)
}

/// What changed of the pull requests: the head when it differs, the pull requests by branch and by stack
pub(crate) fn pull_requests_change(last: &PullRequestsView, now: &PullRequestsView) -> Option<PullRequestsChange> {
  let (pull_requests, removed_pull_requests) = diff_keyed(&last.pull_requests, &now.pull_requests);
  let (stack_pull_requests, removed_stack_pull_requests) = diff_keyed(&last.stack_pull_requests, &now.stack_pull_requests);
  let head = changed(&last.head, &now.head);
  // they change only with an answer, which changes the head
  let (repository, viewer_permission, merge_methods) = match head {
    Some(_) => (now.repository.clone(), now.viewer_permission, now.merge_methods),
    None => (None, None, None),
  };
  let change = PullRequestsChange {
    repository,
    viewer_permission,
    merge_methods,
    head,
    pull_requests,
    removed_pull_requests,
    stack_pull_requests,
    removed_stack_pull_requests,
  };
  (change != PullRequestsChange::default()).then_some(change)
}

/// What changed of the robots: the head when it differs, the robots by id
pub(crate) fn robots_change(last: &RobotsView, now: &RobotsView) -> Option<RobotsChange> {
  let (robots, removed_robots) = diff_keyed(&last.robots, &now.robots);
  let change = RobotsChange {
    head: changed(&last.head, &now.head),
    robots,
    removed_robots,
    failing: changed(&last.failing, &now.failing),
    needs_review: changed(&last.needs_review, &now.needs_review),
    problems_failed: changed(&last.problems_failed, &now.problems_failed),
  };
  (change != RobotsChange::default()).then_some(change)
}

/// What changed of the Patronus view: its states when they differ, the branches and the stacks by key
fn patronus_change(last: &PatronusPart, now: &PatronusPart) -> Option<PatronusChange> {
  let (branches, removed_branches) = diff_keyed(&last.view.branches, &now.view.branches);
  let (stacks, removed_stacks) = diff_keyed(&last.view.stacks, &now.view.stacks);
  let change = PatronusChange {
    state: changed(&last.state, &now.state),
    landing_needs: changed(&last.landing_needs, &now.landing_needs),
    branches,
    stacks,
    landed_by_robot: changed(&last.view.landed_by_robot, &now.view.landed_by_robot),
    restarts: changed(&last.view.restarts, &now.view.restarts),
    removed_branches,
    removed_stacks,
  };
  (change != PatronusChange::default()).then_some(change)
}

/// The news of `now` that `last` lacks, oldest first
fn new_news(last: &[News], now: &[News]) -> Vec<News> {
  let sent = last.last().map_or(0, |news| news.id);
  now.iter().filter(|news| news.id > sent).cloned().collect()
}
