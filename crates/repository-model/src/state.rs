//! What a session sent its windows of the table (docs/design/rust-domain-authority.md, "Представление: снимок и
//! патчи"): `View` keeps the view as it was sent last, at its version. `diff` compares the table state with it part by
//! part (`PartialEq`, `patch::changes`) and answers the patch of the parts that differ, cloning only those; `snapshot`
//! is the view at the version of the last patch, for a window that opens or lost a patch. Rust decides equality, so the
//! page keeps the identity of every part a patch does not carry.
//!
//! A session seeded from the frame of an earlier run (`warm_start`) starts at the view of that frame (`seeded`): what
//! the rules derive of the seeded state, at the first version of its epoch, which the session keeps as its seed. A
//! window that holds it (the start of the app shows it before the window subscribes) catches up by one patch from the
//! seed to the view sent since (`join`, `patch::diff_views`), not by the whole view, until a window that held it got
//! the table of this run (`told`): a reload of that window, which installs the seed again, gets the snapshot, so the
//! end of the first sync ends once in it.

use crate::action_view::{ActionSources, actions_of};
use crate::derive::{attention_of, landed_states_of, landing_route_of, lands_of, next_steps_of, push_view, session_open_pull_requests};
use crate::facts::FactStore;
use crate::patch::{Now, apply_changes, apply_patch, changed, changes, diff_views, keyed_facts_change, pull_requests_change, robots_change};
use crate::review_view::{review_actions_of, reviews_view};
use crate::session::SubscriptionId;
use crate::table::TableState;
use domain_types::session::{FactsChanges, FactsView, RepositoryView, SyncState, TableChanges, Version, ViewPatch, ViewSubscription};
use std::hash::{BuildHasher, RandomState};

/// The view of a session as its windows hold it
#[derive(Clone, Debug)]
pub struct View {
  last_sent: RepositoryView,
  /// The view of the frame the session was seeded from, at the first version of its epoch (`seeded`), until a window
  /// that held it was told the table of this run (`told`)
  seed: Option<RepositoryView>,
  /// The subscriptions that joined holding the seed and hold the frame still: the patch of the table of this run tells
  /// them the end of the first sync
  seed_holders: Vec<SubscriptionId>,
}

/// What a window that subscribes gets (`View::join`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Joined {
  /// It holds the view at the version of the last patch: the patches after it follow
  Holds,
  /// It holds the seed: the patch from the seed to the view at the version of the last patch
  CatchUp(Box<ViewPatch>),
  /// The view at the version of the last patch
  Snapshot(Box<RepositoryView>),
}

impl Joined {
  /// What the window that subscribed as `id` is answered (`ViewSubscription`): the snapshot, or the catch-up from the
  /// seed it holds, or neither when it holds the view of the last patch
  #[must_use]
  pub fn answer(self, id: SubscriptionId) -> ViewSubscription {
    let (view, catch_up) = match self {
      Self::Holds => (None, None),
      Self::CatchUp(patch) => (None, Some(*patch)),
      Self::Snapshot(view) => (Some(*view), None),
    };
    ViewSubscription { id: id.0, view, catch_up }
  }
}

impl View {
  /// An empty view at the start of `epoch` of a session; the first `diff` sends everything the table has
  #[must_use]
  pub fn new(epoch: u32) -> Self {
    Self {
      last_sent: RepositoryView {
        version: Version { epoch, seq: 0 },
        ..RepositoryView::default()
      },
      seed: None,
      seed_holders: Vec::new(),
    }
  }

  /// The view of a session as it opens: what `state` and `facts` derive, at the first version of `epoch`, so the first
  /// window that subscribes has the prefix of the session's key in its header (`TableHeader.prefix`, which names the
  /// branches the page shows) before any sync
  #[must_use]
  pub fn opened(epoch: u32, state: &TableState, facts: &FactStore) -> Self {
    let mut view = Self::new(epoch);
    let changes = view.changes(state, facts);
    let version = view.version();
    if let Err(rejected) = apply_changes(&mut view.last_sent, version, changes) {
      unreachable!("the changes from the empty view apply to it: {rejected}");
    }
    view
  }

  /// The view of a session seeded from a frame (`warm_start`): `opened` on the seeded table and facts, kept as the seed
  /// a window may hold
  #[must_use]
  pub fn seeded(epoch: u32, state: &TableState, facts: &FactStore) -> Self {
    let mut view = Self::opened(epoch, state, facts);
    view.seed = Some(view.last_sent.clone());
    view
  }

  #[must_use]
  pub const fn version(&self) -> Version {
    self.last_sent.version
  }

  /// The view of the frame the session was seeded from, at the first version of its epoch; none for a session not seeded,
  /// and once a window that held it was told the table of this run
  #[must_use]
  pub const fn seed(&self) -> Option<&RepositoryView> {
    self.seed.as_ref()
  }

  /// The view at the version of the last patch
  #[must_use]
  pub fn snapshot(&self) -> RepositoryView {
    self.last_sent.clone()
  }

  /// What a window that holds the view at `have` (`None`: nothing) gets when it subscribes as `id`: nothing more when it
  /// holds the view of the last patch, the catch-up when it holds the seed, else the snapshot. A catch-up that brings
  /// the table of this run tells the window the end of the first sync, so the seed seeds no window after it: a reload of
  /// that window, which installs the seed again, gets the snapshot, a view whose run it doesn't take for one it saw end;
  /// a window that still holds the frame is told by the patch of that table (`told`)
  pub fn join(&mut self, id: SubscriptionId, have: Option<Version>) -> Joined {
    let joined = match have {
      Some(have) if have == self.version() => Joined::Holds,
      Some(have) => match self.seed.as_ref().filter(|seed| seed.version == have) {
        Some(seed) => Joined::CatchUp(Box::new(diff_views(seed, &self.last_sent))),
        None => Joined::Snapshot(Box::new(self.snapshot())),
      },
      None => Joined::Snapshot(Box::new(self.snapshot())),
    };
    if self.seed.as_ref().is_some_and(|seed| Some(seed.version) == have) {
      if self.last_sent.cached {
        self.seed_holders.push(id);
      } else {
        self.forget_seed();
      }
    }
    joined
  }

  /// The subscription `id` ended: a window that held the seed and leaves before the table of this run holds it no more
  pub fn left(&mut self, id: SubscriptionId) {
    self.seed_holders.retain(|holder| *holder != id);
  }

  /// The patch sent last went to the subscriptions `told`: once the view is the table of this run, a window among them
  /// that held the seed was told the end of the first sync, and the seed seeds no window after it (`join`)
  pub fn told(&mut self, told: impl IntoIterator<Item = SubscriptionId>) {
    if self.last_sent.cached || self.seed_holders.is_empty() {
      return;
    }
    if told.into_iter().any(|id| self.seed_holders.contains(&id)) {
      self.forget_seed();
    }
  }

  fn forget_seed(&mut self) {
    self.seed = None;
    self.seed_holders = Vec::new();
  }

  /// The patch from the view sent last to `state` and `facts`, and the view moves to it; `None` when no part differs
  pub fn diff(&mut self, state: &TableState, facts: &FactStore) -> Option<ViewPatch> {
    let changes = self.changes(state, facts);
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

impl View {
  /// The changes from the view sent last to what `state` and `facts` derive now
  fn changes(&self, state: &TableState, facts: &FactStore) -> TableChanges {
    let last = &self.last_sent;
    let parts = &state.parts;
    // the branch prefix of the session: the virtual branches a pull request may go into, the refs its pushes write
    let prefix = parts.header.prefix.as_str();
    // the remote statuses of a running sync come one by one: the pushes are ordered over the table it leaves, in the
    // patch that ends it (a push waits for the sync), never resent with every status
    let syncing = matches!(state.sync, SyncState::Running { .. });
    let pushes = (!syncing).then(|| push_view(parts, facts));
    let open = session_open_pull_requests(state, facts, prefix);
    // the decisions of the starts, as the pushes: decided over the table a sync leaves, not over each of its statuses
    let lands = (!syncing).then(|| lands_of(state, facts, prefix));
    // what landed each subject, as the starts: over the table a sync leaves, its landing report with it
    let landed = landed_states_of(state, facts);
    // the next steps of the rows as they are, a sync of a branch included (row 1): over the table of the last sync
    let steps = next_steps_of(state, facts, &open, &landed);
    // the actions of the subjects as they are, a sync included (a reason): over the steps just derived, and the pushes
    // and the starts as the view keeps them through a sync
    let sources = ActionSources {
      prefix,
      steps: &steps,
      pushes: pushes.as_ref().unwrap_or(&last.pushes),
      lands: lands.as_ref().unwrap_or(&last.lands),
      open_pull_requests: &open,
      landed: &landed,
    };
    let reviews = reviews_view(state, facts, &steps);
    let mut actions = actions_of(state, facts, &sources);
    actions.extend(review_actions_of(state, facts, &reviews));
    // the robots that need the user, over the rows as they are and the robots, a sync included (a row it builds has no
    // tip, so its robot asks nothing until it is built)
    let attention = attention_of(state, facts);
    let route = landing_route_of(&parts.header, &parts.sections.landed, facts);
    changes(
      last,
      Now {
        cached: &state.cached,
        sync: &state.sync,
        header: &parts.header,
        order: &parts.order,
        branches: &parts.branches,
        stacks: &parts.stacks,
        sections: &parts.sections,
        pushes: pushes.as_ref(),
        open_pull_requests: &open,
        lands: lands.as_ref(),
        landed_states: (!syncing).then_some(&landed),
        next_steps: &steps,
        actions: &actions,
        attention: &attention,
        operations: state.operations().iter().collect(),
        reviews: &reviews,
        patronus: facts.patronus.part(),
        route: &route,
        facts: facts_change(&last.facts, facts),
        news: facts.news(),
      },
    )
  }
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
