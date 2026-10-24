//! What the app keeps to answer `get_repository_view` (and the E2E server, with the same code): the branch table of the
//! last sync of each repository and branch prefix, the memo of what its views asked by content, and the landing needs
//! of its branches. A sync that fails keeps the table of the one before: a view is of the last table a sync finished.
//! The checks of the app's push and of its robot actions (`push_guard`, `start_guard`) read the same table and memo with
//! the robots Rust read itself (`guard_facts`).
//!
//! The landing needs (`trial_check::landing_gate::landing_needs`) are a fact with its inputs: the baseline, the tips of
//! the table and the commits merged Safe Pushes landed. A view whose inputs have no answer yet says `unknown` and starts
//! the computation in the background, one trial session for every branch; while the inputs stay, nothing is computed
//! again. `wait_landing_needs` resolves once it finished, and the page asks the view again. The landing check of a start
//! (`landing_check`) is kept with its inputs too, so the start reuses the check of its popover. `forget` drops what is
//! kept of a repository once its computations ended, before the E2E server deletes it.

use crate::branch_table::BranchTable;
use crate::patronus_view::{ContentMemo, landed_commits, match_robots, patronus_view, with_landing_needs};
use crate::push_guard::GuardFacts;
use domain_types::landing::LandingRefusal;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::{GetRepositoryViewParams, PatronusView, RepositoryView, WaitLandingNeedsParams};
use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use object_cache::cache::ObjectCache;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;
use sync_utils::span::in_current_span;
use tracing::{debug, instrument};
use trial_check::landing_gate::{LandingCheck, LandingGateParams, landing_check, landing_check_inputs, landing_needs_of};

/// How many repositories and branch prefixes the app keeps the views of
const KEPT: u64 = 32;

/// How many landing checks of one repository and branch prefix are kept: those of the subjects the popovers asked last
const KEPT_CHECKS: usize = 8;

/// The last landing checks of one repository and branch prefix, each with the hash of its inputs, the oldest first
type KeptChecks = Mutex<VecDeque<(u64, LandingCheck)>>;

/// The landing needs of every branch, by branch; a branch not listed needs nothing
type Needs = BTreeMap<String, Vec<LandingRefusal>>;

/// The landing needs of one repository and branch prefix
#[derive(Default)]
struct LandingState {
  /// The inputs (`landing_inputs`) of the last finished computation and what it found; `None` when it failed
  known: Option<(u64, Option<Arc<Needs>>)>,
  /// The inputs of the computation that runs
  computing: Option<u64>,
  /// The inputs a view asked while another computation ran, with the landed commits: computed next
  wanted: Option<(u64, HashSet<String>)>,
  /// Counts the finished computations
  generation: u32,
}

/// `LandingState` and the signal of a finished computation
#[derive(Default)]
struct LandingFacts {
  state: Mutex<LandingState>,
  finished: Condvar,
}

impl LandingFacts {
  /// The state; a poisoned lock (a panic in a computation) keeps what it had
  fn lock(&self) -> MutexGuard<'_, LandingState> {
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }
}

/// The longest wait of `wait_landing_needs`: a computation that takes longer is waited for again by the next view
const LANDING_WAIT: Duration = Duration::from_secs(120);

/// The tables, the memos and the landing needs, by repository path and branch prefix. Cloning shares them
#[derive(Clone)]
pub struct RepositoryViews {
  tables: Cache<(String, String), Arc<BranchTable>>,
  memos: Cache<(String, String), Arc<Mutex<ContentMemo>>>,
  landing: Cache<(String, String), Arc<LandingFacts>>,
  /// The last landing checks with their inputs (`landing_check`)
  checks: Cache<(String, String), Arc<KeptChecks>>,
}

impl Default for RepositoryViews {
  fn default() -> Self {
    Self::with_capacity(KEPT)
  }
}

impl RepositoryViews {
  /// Keeps the views of at most `repositories` repositories and branch prefixes: the E2E server keeps those of every
  /// repository of a run, many more than the app opens
  pub fn with_capacity(repositories: u64) -> Self {
    Self {
      tables: Cache::new(repositories),
      memos: Cache::new(repositories),
      landing: Cache::new(repositories),
      checks: Cache::new(repositories),
    }
  }

  /// Keeps the table a sync of `repository_path` under `branch_prefix` finished
  pub fn keep(&self, repository_path: &str, branch_prefix: &str, table: BranchTable) {
    self.tables.insert((repository_path.to_string(), branch_prefix.to_string()), Arc::new(table));
  }

  /// The view of the table kept for `branch_prefix` with the asker's robots, matches and remote facts; empty before the
  /// first sync under that prefix. Runs git for the robots the hashes and the memo don't answer: call from a blocking
  /// context. The landing needs come from the last computation of the same inputs, else they are unknown and computed in
  /// the background, through `cache`
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branch_prefix))]
  pub fn view(&self, git_executor: &GitCommandExecutor, cache: &ObjectCache, branch_prefix: &str, params: &GetRepositoryViewParams) -> RepositoryView {
    let key = (params.repository_path.clone(), branch_prefix.to_owned());
    let Some(table) = self.tables.get(&key) else {
      return RepositoryView {
        landing_generation: 0,
        patronus: PatronusView::default(),
      };
    };
    let mut patronus = self.with_memo(&key, |memo| {
      patronus_view(git_executor, &params.repository_path, &table, &params.remote, &params.robots, &params.matched, memo)
    });
    let (needs, landing_generation) = self.landing_needs(git_executor, cache, &key, &table, landed_commits(&patronus));
    if let Some(needs) = needs {
      with_landing_needs(&mut patronus, &table, &needs);
    }
    RepositoryView { landing_generation, patronus }
  }

  /// The facts the checks of the app's push read (`push_guard`): the kept table, `robots` as Rust read them, matched as
  /// the rows match them, and the Patronus view of both on the table's remote facts, through the memo of the views;
  /// `None` before the first sync. Runs git for the robots the hashes and the memo don't answer: call from a blocking
  /// context
  #[instrument(skip_all, fields(repository_path = %repository_path, robots = robots.len()))]
  pub fn guard_facts(&self, git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, robots: Vec<PatronusRobot>) -> Option<GuardFacts> {
    let key = (repository_path.to_string(), branch_prefix.to_string());
    let synced = self.tables.get(&key)?;
    let table = &*synced;
    let matched = match_robots(table, &table.remote, &robots);
    let view = self.with_memo(&key, |memo| patronus_view(git_executor, repository_path, table, &table.remote, &robots, &matched, memo));
    Some(GuardFacts::new(synced, robots, view))
  }

  /// Runs `ask` with the memo of `key`; a poisoned memo (a panic in an earlier view) starts again
  fn with_memo<T>(&self, key: &(String, String), ask: impl FnOnce(&mut ContentMemo) -> T) -> T {
    let memo = self.memos.get_with(key.clone(), || Arc::new(Mutex::new(ContentMemo::default())));
    let mut memo = memo.lock().unwrap_or_else(|poisoned| {
      memo.clear_poison();
      poisoned.into_inner()
    });
    ask(&mut memo)
  }

  /// The landing needs of `table` with `landed`, when the last computation was of these inputs; else `None`, and the
  /// computation of them starts unless it runs, failed for them, or the table has no branch. With the count of finished
  /// computations
  fn landing_needs(
    &self,
    git_executor: &GitCommandExecutor,
    cache: &ObjectCache,
    key: &(String, String),
    table: &BranchTable,
    landed: HashSet<String>,
  ) -> (Option<Arc<Needs>>, u32) {
    if table.branches.is_empty() {
      return (Some(Arc::default()), 0);
    }
    let inputs = landing_inputs(table, &landed);
    let facts = self.landing.get_with(key.clone(), Arc::default);
    let mut state = facts.lock();
    match &state.known {
      Some((known, needs)) if *known == inputs => return (needs.clone(), state.generation),
      _ => {}
    }
    match state.computing {
      Some(computing) if computing == inputs => state.wanted = None,
      Some(_) => state.wanted = Some((inputs, landed)),
      None => {
        state.computing = Some(inputs);
        let (git, cache, facts, (repository_path, branch_prefix)) = (git_executor.clone(), cache.clone(), Arc::clone(&facts), key.clone());
        std::thread::spawn(in_current_span(move || {
          compute_landing_needs(&git, &cache, &facts, &repository_path, &branch_prefix, inputs, landed);
        }));
      }
    }
    (None, state.generation)
  }

  /// The landing check of the start `params` with `landed` (`trial_check::landing_gate::landing_check`): the one kept for
  /// the same inputs (`landing_check_inputs`: the refs and the config, the subject, the overrides and the landed
  /// commits), else a check now, kept. The start right after the check of its popover, and a second click, run the two
  /// processes of the inputs instead of a session and a trial; a commit, a sync, a fetch or a moved store in between is
  /// another input, checked anew. Call from a blocking context
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branch_name = %params.branch_name, stack = params.stack))]
  pub fn landing_check(
    &self,
    git_executor: &GitCommandExecutor,
    cache: &ObjectCache,
    branch_prefix: &str,
    params: &LandingGateParams,
    landed: &HashSet<String>,
  ) -> anyhow::Result<LandingCheck> {
    let inputs = landing_check_inputs(git_executor, branch_prefix, params, landed)?;
    let kept = self.checks.get_with((params.repository_path.clone(), branch_prefix.to_owned()), Arc::default);
    if let Some((_, check)) = lock_checks(&kept).iter().find(|(known, _)| *known == inputs) {
      debug!("the landing check of these inputs is kept");
      return Ok(check.clone());
    }
    let check = landing_check(git_executor, cache, branch_prefix, params, landed)?;
    let mut kept = lock_checks(&kept);
    if kept.len() == KEPT_CHECKS {
      kept.pop_front();
    }
    kept.push_back((inputs, check.clone()));
    Ok(check)
  }

  /// Waits until no computation of the landing needs of the repository under `branch_prefix` runs, or until one finished after the
  /// `after`-th, at most `LANDING_WAIT`; returns the count of finished computations, which the page compares with the
  /// one of its view. Call from a blocking context
  #[instrument(skip_all, fields(repository_path = %params.repository_path, branch_prefix, after = params.after))]
  pub fn wait_landing_needs(&self, branch_prefix: &str, params: &WaitLandingNeedsParams) -> u32 {
    let Some(facts) = self.landing.get(&(params.repository_path.clone(), branch_prefix.to_owned())) else {
      return 0;
    };
    let state = facts.lock();
    let (state, _) = facts
      .finished
      .wait_timeout_while(state, LANDING_WAIT, |state| state.computing.is_some() && state.generation == params.after)
      .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.generation
  }

  /// Forgets the tables, the memos and the landing needs of `repository_path` under every branch prefix, once the
  /// computations of its landing needs that run ended, at most `LANDING_WAIT` each: the E2E server deletes the directory
  /// of a test's repository after it, so no git of a computation runs in a deleted directory. Call it once no command
  /// asks a view of the repository any more (a view starts a computation again), from a blocking context. Returns
  /// whether every computation ended
  pub fn forget(&self, repository_path: &str) -> bool {
    let mut ended = true;
    for (key, facts) in self.landing.iter().filter(|(key, _)| key.0 == repository_path) {
      let state = facts.lock();
      let (state, wait) = facts
        .finished
        .wait_timeout_while(state, LANDING_WAIT, |state| state.computing.is_some())
        .unwrap_or_else(std::sync::PoisonError::into_inner);
      drop(state);
      ended &= !wait.timed_out();
      self.landing.invalidate(&*key);
    }
    for (key, _) in self.tables.iter().filter(|(key, _)| key.0 == repository_path) {
      self.tables.invalidate(&*key);
    }
    for (key, _) in self.memos.iter().filter(|(key, _)| key.0 == repository_path) {
      self.memos.invalidate(&*key);
    }
    for (key, _) in self.checks.iter().filter(|(key, _)| key.0 == repository_path) {
      self.checks.invalidate(&*key);
    }
    ended
  }
}

/// The kept landing checks; a poisoned lock (a panic in a check) keeps what it had
fn lock_checks(kept: &KeptChecks) -> MutexGuard<'_, VecDeque<(u64, LandingCheck)>> {
  kept.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The inputs of the landing needs: the baseline, the tips of the branches of the table and the commits merged Safe
/// Pushes landed
fn landing_inputs(table: &BranchTable, landed: &HashSet<String>) -> u64 {
  let mut hasher = DefaultHasher::new();
  table.baseline.hash(&mut hasher);
  for branch in &table.branches {
    branch.name.hash(&mut hasher);
    table.tip_of(&branch.name).hash(&mut hasher);
  }
  let landed: BTreeSet<&String> = landed.iter().collect();
  for id in landed {
    id.hash(&mut hasher);
  }
  hasher.finish()
}

/// The background computation of the landing needs of `inputs`, then of the inputs a view asked meanwhile: one trial
/// session each. A failure is an answer of nothing known for these inputs: the needs stay unknown until they change
fn compute_landing_needs(
  git_executor: &GitCommandExecutor,
  cache: &ObjectCache,
  facts: &LandingFacts,
  repository_path: &str,
  branch_prefix: &str,
  mut inputs: u64,
  mut landed: HashSet<String>,
) {
  loop {
    let needs = match landing_needs_of(git_executor, cache, repository_path, branch_prefix, &landed) {
      Ok(needs) => Some(Arc::new(needs)),
      Err(error) => {
        debug!(error = %format!("{error:#}"), "no landing needs: unknown until the inputs change");
        None
      }
    };
    let mut state = facts.lock();
    state.known = Some((inputs, needs));
    state.generation = state.generation.wrapping_add(1);
    if let Some((next, next_landed)) = state.wanted.take().filter(|(wanted, _)| *wanted != inputs) {
      state.computing = Some(next);
      (inputs, landed) = (next, next_landed);
      drop(state);
      facts.finished.notify_all();
    } else {
      state.computing = None;
      drop(state);
      facts.finished.notify_all();
      return;
    }
  }
}

#[cfg(test)]
#[path = "views_test.rs"]
mod views_test;
