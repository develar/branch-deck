//! What a session sent its windows of the table (docs/design/rust-domain-authority.md, "Представление: снимок и
//! патчи"): `View` keeps the view as it was sent last, at its version. `diff` compares the table state with it part by
//! part (`PartialEq`) and answers the patch of the parts that differ, cloning only those; `snapshot` is the view at the
//! version of the last patch, for a window that opens or lost a patch. Rust decides equality, so the page keeps the
//! identity of every part a patch does not carry.

use crate::action_view::{ActionSources, actions_of};
use crate::derive::{attention_of, landed_states_of, lands_of, next_steps_of, push_view, session_open_pull_requests};
use crate::facts::FactStore;
use crate::patch::{apply_patch, branch_change, changed, changed_commits, diff_keyed};
use crate::review_view::{review_actions_of, reviews_view};
use crate::table::TableState;
use domain_types::operation::{OperationId, RunningOperation};
use domain_types::session::{
  ArchivedChanges, Fact, FactsChanges, FactsView, News, PatronusChange, PatronusPart, PullRequestsChange, PullRequestsView, RepositoryView, RobotsChange, RobotsView, SyncState,
  TableChanges, Version, ViewPatch,
};
use std::collections::BTreeMap;
use std::hash::{BuildHasher, RandomState};

/// The view of a session as its windows hold it
#[derive(Clone, Debug)]
pub struct View {
  last_sent: RepositoryView,
  /// The branch prefix of the session: the virtual branches a pull request may go into
  prefix: String,
}

impl View {
  /// An empty view at the start of `epoch` of the session of `prefix`; the first `diff` sends everything the table has
  #[must_use]
  pub fn new(epoch: u32, prefix: &str) -> Self {
    Self {
      last_sent: RepositoryView {
        version: Version { epoch, seq: 0 },
        ..RepositoryView::default()
      },
      prefix: prefix.to_owned(),
    }
  }

  #[must_use]
  pub const fn version(&self) -> Version {
    self.last_sent.version
  }

  /// The view at the version of the last patch
  #[must_use]
  pub fn snapshot(&self) -> RepositoryView {
    self.last_sent.clone()
  }

  /// The patch from the view sent last to `state` and `facts`, and the view moves to it; `None` when no part differs
  pub fn diff(&mut self, state: &TableState, facts: &FactStore) -> Option<ViewPatch> {
    let last = &self.last_sent;
    let parts = &state.parts;
    let mut branches = BTreeMap::new();
    for (name, branch) in &parts.branches {
      if let Some(change) = branch_change(last.branches.get(name), branch) {
        branches.insert(name.clone(), change);
      }
    }
    let removed_branches = last.branches.keys().filter(|name| !parts.branches.contains_key(*name)).cloned().collect();
    let (stacks, removed_stacks) = diff_keyed(&last.stacks, &parts.stacks);
    // the remote statuses of a running sync come one by one: the pushes are ordered over the table it leaves, in the
    // patch that ends it (a push waits for the sync), never resent with every status
    let syncing = matches!(state.sync, SyncState::Running { .. });
    let pushes_now = (!syncing).then(|| push_view(parts, facts));
    let (pushes, removed_pushes) = pushes_now.as_ref().map_or_else(|| (BTreeMap::new(), Vec::new()), |now| diff_keyed(&last.pushes, now));
    let open = session_open_pull_requests(state, facts, &self.prefix);
    let (open_pull_requests, removed_open_pull_requests) = diff_keyed(&last.open_pull_requests, &open);
    // the decisions of the starts, as the pushes: decided over the table a sync leaves, not over each of its statuses
    let lands_now = (!syncing).then(|| lands_of(state, facts, &self.prefix));
    let (lands, removed_lands) = lands_now.as_ref().map_or_else(|| (BTreeMap::new(), Vec::new()), |now| diff_keyed(&last.lands, now));
    // what landed each subject, as the starts: over the table a sync leaves, its landing report with it
    let landed = landed_states_of(state, facts);
    let (landed_states, removed_landed_states) = if syncing {
      (BTreeMap::new(), Vec::new())
    } else {
      diff_keyed(&last.landed_states, &landed)
    };
    // the next steps of the rows as they are, a sync of a branch included (row 1): over the table of the last sync
    let steps = next_steps_of(state, facts, &open, &landed);
    let (next_steps, removed_next_steps) = diff_keyed(&last.next_steps, &steps);
    // the actions of the subjects as they are, a sync included (a reason): over the steps just derived, and the pushes
    // and the starts as the view keeps them through a sync
    let sources = ActionSources {
      prefix: &self.prefix,
      steps: &steps,
      pushes: pushes_now.as_ref().unwrap_or(&last.pushes),
      lands: lands_now.as_ref().unwrap_or(&last.lands),
      open_pull_requests: &open,
      landed: &landed,
    };
    let reviews = reviews_view(state, facts, &steps);
    let mut actions_now = actions_of(state, facts, &sources);
    actions_now.extend(review_actions_of(state, facts, &reviews));
    let (actions, removed_actions) = diff_keyed(&last.actions, &actions_now);
    // the robots that need the user, over the rows as they are and the robots, a sync included (a row it builds has no
    // tip, so its robot asks nothing until it is built)
    let (attention, removed_attention) = diff_keyed(&last.attention, &attention_of(state, facts));
    let (operations, removed_operations) = operations_change(&last.operations, state.operations());
    let (last_archived, archived) = (&last.sections.archived, &parts.sections.archived);
    let (integration, removed_integration) = diff_keyed(&last_archived.integration, &archived.integration);
    let archived_changes = ArchivedChanges {
      names: changed(&last_archived.names, &archived.names),
      newly_archived: changed(&last_archived.newly_archived, &archived.newly_archived),
      integration,
      removed_integration,
    };
    let changes = TableChanges {
      sync: changed(&last.sync, &state.sync),
      header: changed(&last.header, &parts.header),
      order: changed(&last.order, &parts.order),
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
      operations,
      removed_operations,
      unassigned: changed_commits(&last.sections.unassigned, &parts.sections.unassigned),
      archived: (archived_changes != ArchivedChanges::default()).then_some(archived_changes),
      unapplied: changed(&last.sections.unapplied, &parts.sections.unapplied),
      landed: changed(&last.sections.landed, &parts.sections.landed),
      reviews: changed(&last.reviews, &reviews),
      patronus: patronus_change(&last.patronus, facts.patronus.part()),
      facts: facts_change(&last.facts, facts),
      news: new_news(&last.news, facts.news()),
    };
    if changes.is_empty() {
      return None;
    }
    let patch = ViewPatch {
      base: self.last_sent.version,
      next: self.last_sent.version.next(),
      changes,
    };
    // the view moves as a window's does: by the patch, so only the parts that changed are copied
    if let Err(rejected) = apply_patch(&mut self.last_sent, patch.clone()) {
      unreachable!("a patch from the view applies to it: {rejected}");
    }
    Some(patch)
  }
}

/// The operations of `now` that the view sent last lacks or has otherwise, and the ids of the ones that ended
fn operations_change(last: &BTreeMap<OperationId, RunningOperation>, now: &[RunningOperation]) -> (BTreeMap<OperationId, RunningOperation>, Vec<OperationId>) {
  let changed = now
    .iter()
    .filter(|operation| last.get(&operation.id) != Some(*operation))
    .map(|operation| (operation.id.clone(), operation.clone()))
    .collect();
  let removed = last.keys().filter(|id| !now.iter().any(|operation| operation.id == **id)).cloned().collect();
  (changed, removed)
}

/// What changed of the facts of the remote services, by part
fn facts_change(last: &FactsView, facts: &FactStore) -> Option<FactsChanges> {
  let rules_changed = last.rules.len() != facts.rules.len() || facts.rules.iter().any(|(branch, rules)| last.rules.get(branch) != Some(rules.fact()));
  let readiness = &facts.readiness;
  let (readiness, removed_readiness) = keyed_facts_change(&last.readiness, readiness.facts(), |key| readiness.fact(key).is_some());
  let details = &facts.robot_details;
  let (robot_details, removed_robot_details) = keyed_facts_change(&last.robot_details, details.iter().map(|(id, record)| (id, record.fact())), |id| details.contains_key(id));
  let change = FactsChanges {
    robots: robots_change(&last.robots, facts.robots.view()),
    pull_requests: pull_requests_change(&last.pull_requests, facts.pull_requests.view()),
    rules: rules_changed.then(|| facts.rules.iter().map(|(branch, rules)| (branch.clone(), rules.fact().clone())).collect()),
    profiles: changed(&last.profiles, facts.profiles.fact()),
    readiness,
    removed_readiness,
    robot_details,
    removed_robot_details,
    review_requests: changed(&last.review_requests, facts.review_requests.view()),
    auth: changed(&last.auth, facts.auth.fact()),
  };
  (change != FactsChanges::default()).then_some(change)
}

/// Of a part of facts by key (the readiness by subject, the details by robot), the facts of `now` that changed and the
/// keys `now` has no longer (`contains`): only those are cloned
fn keyed_facts_change<'a, T: PartialEq + Clone + 'a>(
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
fn pull_requests_change(last: &PullRequestsView, now: &PullRequestsView) -> Option<PullRequestsChange> {
  let (pull_requests, removed_pull_requests) = diff_keyed(&last.pull_requests, &now.pull_requests);
  let (stack_pull_requests, removed_stack_pull_requests) = diff_keyed(&last.stack_pull_requests, &now.stack_pull_requests);
  let change = PullRequestsChange {
    head: changed(&last.head, &now.head),
    pull_requests,
    removed_pull_requests,
    stack_pull_requests,
    removed_stack_pull_requests,
  };
  (change != PullRequestsChange::default()).then_some(change)
}

/// What changed of the robots: the head when it differs, the robots by id
fn robots_change(last: &RobotsView, now: &RobotsView) -> Option<RobotsChange> {
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

/// The news of `now` that the view sent last lacks, oldest first
fn new_news(last: &[News], now: &[News]) -> Vec<News> {
  let sent = last.last().map_or(0, |news| news.id);
  now.iter().filter(|news| news.id > sent).cloned().collect()
}

/// A random epoch of a session: random per process and per call
#[must_use]
pub fn new_epoch() -> u32 {
  // each `RandomState` has its own random keys; the hash of nothing is as random as they are
  let hash = RandomState::new().hash_one(());
  u32::try_from(hash >> 32).unwrap_or_default()
}

#[cfg(test)]
#[path = "state_test.rs"]
mod state_test;
