//! What a session sent its windows of the table (docs/design/rust-domain-authority.md, "Представление: снимок и
//! патчи"): `View` keeps the view as it was sent last, at its version. `diff` compares the table state with it part by
//! part (`PartialEq`, `patch::changes`) and answers the patch of the parts that differ, cloning only those; the view
//! then copies the parts of the table and the facts the patch carries and takes the parts the rules derived as they are
//! (`Derived`), which is what the patch leaves of them. `snapshot` is the view at the version of the last patch, for a
//! window that opens or lost a patch. Rust decides equality, so the page keeps the identity of every part a patch does
//! not carry.
//!
//! A session seeded from the frame of an earlier run (`warm_start`) starts at the view of that frame (`seeded`): what
//! the rules derive of the seeded state, at the first version of its epoch, which the session keeps as its seed. A
//! window that holds it (the start of the app shows it before the window subscribes) catches up by one patch from the
//! seed to the view sent since (`join`, `patch::diff_views`), not by the whole view; so does a reload of that window,
//! whose init script installs the same seed, for as long as the session lives. Whether that catch-up ends a run in the
//! window is the page's to know (a Tauri channel to a page that went away never fails a send, so the session can't tell
//! which page got the patch of the table of this run): it remembers the end it saw beside the seed (`src/model/store.ts`).

use crate::action_view::{ActionSources, actions_of};
use crate::derive::{attention_of, landed_states_of, landing_route_of, lands_of, next_steps_of, push_view, session_open_pull_requests};
use crate::facts::FactStore;
use crate::patch::{Now, apply_changes, changed, changes, diff_views, kept_parts, keyed_facts_change, pull_requests_change, robots_change};
use crate::review_view::{review_actions_of, reviews_view};
use crate::session::SubscriptionId;
use crate::table::TableState;
use domain_types::actions::ActionState;
use domain_types::landing::{LandDecisions, Landed};
use domain_types::landing_route::LandingRoute;
use domain_types::next_step::NextStepView;
use domain_types::pull_request::PullRequestView;
use domain_types::session::{BranchPush, FactsChanges, FactsView, RepositoryView, ReviewsView, RobotAttention, SyncState, TableChanges, Version, ViewPatch, ViewSubscription};
use std::collections::BTreeMap;
use std::hash::{BuildHasher, RandomState};

/// The view of a session as its windows hold it
#[derive(Clone, Debug)]
pub struct View {
  last_sent: RepositoryView,
  /// The view of the frame the session was seeded from, at the first version of its epoch (`seeded`)
  seed: Option<RepositoryView>,
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

  /// The view of the frame the session was seeded from, at the first version of its epoch; none for a session not seeded
  #[must_use]
  pub const fn seed(&self) -> Option<&RepositoryView> {
    self.seed.as_ref()
  }

  /// The view at the version of the last patch
  #[must_use]
  pub fn snapshot(&self) -> RepositoryView {
    self.last_sent.clone()
  }

  /// What a window that holds the view at `have` (`None`: nothing) gets when it subscribes: nothing more when it holds
  /// the view of the last patch, the catch-up when it holds the seed (also a reload of a window that got the table of
  /// this run, whose init script installs the seed again), else the snapshot
  #[must_use]
  pub fn join(&self, have: Option<Version>) -> Joined {
    match have {
      Some(have) if have == self.version() => Joined::Holds,
      Some(have) => match self.seed.as_ref().filter(|seed| seed.version == have) {
        Some(seed) => Joined::CatchUp(Box::new(diff_views(seed, &self.last_sent))),
        None => Joined::Snapshot(Box::new(self.snapshot())),
      },
      None => Joined::Snapshot(Box::new(self.snapshot())),
    }
  }

  /// The patch from the view sent last to `state` and `facts`, and the view moves to it; `None` when no part differs
  pub fn diff(&mut self, state: &TableState, facts: &FactStore) -> Option<ViewPatch> {
    let derived = Derived::of(&self.last_sent, state, facts);
    let changes = changes(&self.last_sent, derived.now(&self.last_sent, state, facts));
    if changes.is_empty() {
      return None;
    }
    let patch = ViewPatch {
      base: self.last_sent.version,
      next: self.last_sent.version.next(),
      changes,
    };
    // the view moves as a window's does: by the patch, so only the parts of the table and the facts that changed are
    // copied, and the parts derived of them move in as they are, which is what the patch leaves of them
    if let Err(rejected) = apply_changes(&mut self.last_sent, patch.next, kept_parts(&patch.changes)) {
      unreachable!("a patch from the view applies to it: {rejected}");
    }
    derived.move_into(&mut self.last_sent);
    Some(patch)
  }
}

impl View {
  /// The changes from the view sent last to what `state` and `facts` derive now
  fn changes(&self, state: &TableState, facts: &FactStore) -> TableChanges {
    let derived = Derived::of(&self.last_sent, state, facts);
    changes(&self.last_sent, derived.now(&self.last_sent, state, facts))
  }
}

/// The parts of the view a patch derives of the table and the facts (the rules' answers), owned: `changes` compares them
/// with the view sent last, and the view takes them as they are once the patch is sent (`move_into`)
struct Derived {
  /// A running sync decides no pushes, no starts and what landed: the view keeps the ones it has
  syncing: bool,
  pushes: Option<BTreeMap<String, BranchPush>>,
  open: BTreeMap<String, PullRequestView>,
  lands: Option<BTreeMap<String, LandDecisions>>,
  landed: BTreeMap<String, Landed>,
  steps: BTreeMap<String, NextStepView>,
  actions: BTreeMap<String, Vec<ActionState>>,
  attention: BTreeMap<String, RobotAttention>,
  reviews: ReviewsView,
  route: LandingRoute,
}

impl Derived {
  /// What the rules derive of `state` and `facts`, over the pushes and the starts of `last` while a sync runs
  fn of(last: &RepositoryView, state: &TableState, facts: &FactStore) -> Self {
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
    Self {
      syncing,
      pushes,
      open,
      lands,
      landed,
      steps,
      actions,
      attention,
      reviews,
      route,
    }
  }

  /// The parts of the view as they are now: the table of `state`, these derived parts, and what changed of the facts
  /// since `last`
  fn now<'a>(&'a self, last: &RepositoryView, state: &'a TableState, facts: &'a FactStore) -> Now<'a> {
    let parts = &state.parts;
    Now {
      cached: &state.cached,
      sync: &state.sync,
      header: &parts.header,
      order: &parts.order,
      branches: &parts.branches,
      stacks: &parts.stacks,
      sections: &parts.sections,
      pushes: self.pushes.as_ref(),
      open_pull_requests: &self.open,
      lands: self.lands.as_ref(),
      landed_states: (!self.syncing).then_some(&self.landed),
      next_steps: &self.steps,
      actions: &self.actions,
      attention: &self.attention,
      operations: state.operations().iter().collect(),
      reviews: &self.reviews,
      patronus: facts.patronus.part(),
      route: &self.route,
      facts: facts_change(&last.facts, facts),
      news: facts.news(),
    }
  }

  /// `view` with these parts, as a patch of them from `view` leaves it: every keyed part is the one derived now (the
  /// entries that changed come with the patch, the ones it removes go, the others are equal), the pushes, the starts and
  /// what landed stay while a sync runs
  fn move_into(self, view: &mut RepositoryView) {
    if let Some(pushes) = self.pushes {
      view.pushes = pushes;
    }
    view.open_pull_requests = self.open;
    if let Some(lands) = self.lands {
      view.lands = lands;
    }
    if !self.syncing {
      view.landed_states = self.landed;
    }
    view.next_steps = self.steps;
    view.actions = self.actions;
    view.attention = self.attention;
    view.reviews = self.reviews;
    view.route = self.route;
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
