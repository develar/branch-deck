use crate::branch_table::BranchTable;
use crate::facts::{BaselineRefusal, FactResult, Needs, PatronusOf, RobotsDigest, RobotsRead, derive_patronus};
use crate::patronus_view::ContentMemo;
use crate::pull_request_facts::PullRequestWrite;
use crate::readiness_facts::ReadinessAnswer;
use crate::runtime::{GitSessionHost, PATCH_INTERVAL, RunSetup, SessionEnvironment, sleep_until};
use crate::session::SyncTicket;
use crate::session::{IDLE_LIMIT, OneClock, RunReporter, SessionEvent, SessionEventKind, SessionHost, SessionKey, Sessions, Subscriber, SubscriberGone, SyncRun, SyncedTable};
use crate::table::RemoteSubject;
use crate::table::table_test::{branch_status, grouped, grouping, remote_at, synced, unapplied};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{pull_request, robot};
use domain_types::landing::{BaselinePush, LandedBy, LandingFix, LandingRefusal, StackCommit};
use domain_types::landing_route::{LandingRemote, LandingRoute, LandingSetting, RouteGate};
use domain_types::patronus::{BranchReviewReadiness, PatronusAvailability, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult, ReviewReadiness};
use domain_types::pull_request::{PullRequestAvailability, PullRequestsResult, Reviewer, ReviewerState, ViewerPermission};
use domain_types::reason::Reason;
use domain_types::repository_view::{LandingNeeds, PatronusView};
use domain_types::session::{
  BranchStatus, Demand, FactFamily, FactRefreshReason, FactState, PatronusPart, RepositoryView, SyncOutcome, SyncReason, SyncState, SyncStep, Version, ViewPatch,
};
use domain_types::stack::BlockReason;
use domain_types::{BranchSyncStatus, CommitSyncStatus, ProgressReporter, RemoteStatusUpdate, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_core::sync::{SyncDeferred, SyncOptions, SyncReport};
use sync_core::sync_inputs::SyncInputs;
use sync_utils::clock::TestClock;
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use tokio::sync::Notify;
use tokio::time::Instant;

/// The patches a window got
#[derive(Clone, Default)]
struct Window {
  patches: Arc<Mutex<Vec<ViewPatch>>>,
}

impl Window {
  fn take(&self) -> Vec<ViewPatch> {
    std::mem::take(&mut self.patches.lock().unwrap())
  }
}

impl Subscriber for Window {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.patches.lock().map_err(|_poisoned| SubscriberGone)?.push(patch.clone());
    Ok(())
  }
}

/// One scripted run: its events, each followed by `pace` on the runtime's clock, then its result once `gate` (if any)
/// is notified
struct Script {
  events: Vec<SyncEvent>,
  pace: Duration,
  result: Result<SyncReport, ApiError>,
  gate: Option<Arc<Notify>>,
  /// The table the run leaves; empty for most scripts, whose tables are of no repository
  table: BranchTable,
}

/// A host whose runs play scripts in order and whose fingerprint is what the test sets
#[derive(Default)]
struct ScriptedHost {
  scripts: Mutex<VecDeque<Script>>,
  inputs: Mutex<Option<SyncInputs>>,
  runs: Mutex<u32>,
  checks: Mutex<u32>,
  /// The reporter of every run, which a test may keep using after its run ended
  reporters: Mutex<Vec<RunReporter>>,
  /// When each event of the runs was sent, on the runtime's clock
  sent_at: Arc<Mutex<Vec<Instant>>>,
  /// The `keep` of each prune of the review refs, as it ran
  prunes: Mutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<u32>>>>,
}

impl ScriptedHost {
  fn push(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>) {
    self.push_gated(events, result, None);
  }

  fn push_gated(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>, gate: Option<Arc<Notify>>) {
    let pace = Duration::ZERO;
    self.scripts.lock().unwrap().push_back(Script {
      events,
      pace,
      result,
      gate,
      table: BranchTable::default(),
    });
  }

  /// A run that leaves `table`
  fn push_table(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>, table: BranchTable) {
    let pace = Duration::ZERO;
    self.scripts.lock().unwrap().push_back(Script {
      events,
      pace,
      result,
      gate: None,
      table,
    });
  }

  /// A run whose events stream over time: `pace` after each
  fn push_paced(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>, pace: Duration) {
    self.scripts.lock().unwrap().push_back(Script {
      events,
      pace,
      result,
      gate: None,
      table: BranchTable::default(),
    });
  }

  fn set_inputs(&self, rest: u64) {
    *self.inputs.lock().unwrap() = Some(SyncInputs { rest, ..SyncInputs::default() });
  }

  fn runs(&self) -> u32 {
    *self.runs.lock().unwrap()
  }
}

impl SessionHost for ScriptedHost {
  fn inputs(&self, _key: &SessionKey) -> Option<SyncInputs> {
    *self.checks.lock().unwrap() += 1;
    self.inputs.lock().unwrap().clone()
  }

  fn run(&self, _key: &SessionKey, reporter: RunReporter) -> SyncRun {
    *self.runs.lock().unwrap() += 1;
    let script = self.scripts.lock().unwrap().pop_front().expect("a script for every run");
    self.reporters.lock().unwrap().push(reporter.clone());
    let sent_at = Arc::clone(&self.sent_at);
    Box::pin(async move {
      for event in script.events {
        sent_at.lock().unwrap().push(Instant::now());
        reporter.send(event).unwrap();
        if !script.pace.is_zero() {
          tokio::time::sleep(script.pace).await;
        }
      }
      if let Some(gate) = script.gate {
        gate.notified().await;
      }
      let table = script.table;
      script.result.map(|report| SyncedTable { report, table })
    })
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    true
  }

  /// By the hashes only: the scripted tables are of no repository
  fn derive_patronus(&self, key: &SessionKey, of: &PatronusOf<'_>, memo: &mut ContentMemo) -> Result<PatronusView, ApiError> {
    Ok(derive_patronus(&GitCommandExecutor::new(), &key.worktree, of, memo))
  }

  fn landing_needs(&self, _key: &SessionKey, _landed: &HashSet<String>) -> Result<Needs, ApiError> {
    Ok(Needs::new())
  }

  fn prune_review_refs(&self, _key: &SessionKey, keep: &[u32]) {
    if let Some(prunes) = self.prunes.lock().unwrap().as_ref() {
      let _ = prunes.send(keep.to_vec());
    }
  }
}

fn key(worktree: &str) -> SessionKey {
  SessionKey {
    worktree: worktree.to_string(),
    prefix: "test".to_string(),
  }
}

fn scripted(host: &Arc<ScriptedHost>) -> (Sessions, TestClock) {
  let clock = TestClock::new();
  let sessions = Sessions::new(
    tokio::runtime::Handle::current(),
    Arc::clone(host) as Arc<dyn SessionHost>,
    Arc::new(OneClock(Arc::new(clock.clone()))),
  );
  (sessions, clock)
}

/// Sessions on the scripted host, with a window on the session of `/repo`
async fn watched(host: &Arc<ScriptedHost>) -> (Sessions, SessionKey) {
  let (sessions, _clock) = scripted(host);
  let key = key("/repo");
  sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  (sessions, key)
}

/// The events of a sync that builds `auth` with one commit
fn auth_built() -> Vec<SyncEvent> {
  vec![
    grouping(vec![grouped("auth", &["a1"])]),
    synced("auth", "a1", CommitSyncStatus::Created),
    branch_status("auth", BranchSyncStatus::Created),
    SyncEvent::UnappliedBranchesFound { branches: Vec::new() },
  ]
}

fn report(retry: bool, not_built: &[&str]) -> SyncReport {
  SyncReport {
    retry,
    not_built: not_built.iter().map(|name| ((*name).to_string(), BlockReason::Error)).collect(),
    ..SyncReport::default()
  }
}

/// How the request of `ticket` ended, without the version of its answer
async fn outcome(ticket: SyncTicket) -> Result<SyncOutcome, ApiError> {
  ticket.wait().await.map(|answer| answer.outcome)
}

async fn snapshot(sessions: &Sessions, key: &SessionKey) -> RepositoryView {
  sessions.get_snapshot(key).await.unwrap()
}

/// The ticket of a request resolves when the run that serves it ends: a request during a run waits for the quiet
/// follow-up, the first one does not; every patch the window got leads to the snapshot
#[tokio::test]
async fn a_ticket_resolves_when_its_run_ends_not_the_chain() {
  let host = Arc::new(ScriptedHost::default());
  let gate = Arc::new(Notify::new());
  host.push(auth_built(), Ok(report(false, &[])));
  host.push_gated(auth_built(), Ok(report(false, &[])), Some(Arc::clone(&gate)));
  let (sessions, _clock) = scripted(&host);
  let key = key("/repo");
  let window = Window::default();
  let (_, empty) = sessions.subscribe(&key, Box::new(window.clone())).await.unwrap();

  let first = sessions.sync(&key, SyncReason::Manual);
  let second = sessions.sync(&key, SyncReason::AfterRewrite);
  // the follow-up is held by its gate: the first ticket does not wait for it
  assert_eq!(outcome(first).await, Ok(SyncOutcome::Synced));
  gate.notify_one();
  let second = second.wait().await.unwrap();
  assert_eq!(second.outcome, SyncOutcome::Synced);
  assert_eq!(host.runs(), 2);

  let mut view = empty;
  for patch in window.take() {
    crate::patch::apply_patch(&mut view, patch).unwrap();
  }
  let snapshot = snapshot(&sessions, &key).await;
  assert_eq!(view, snapshot);
  // the answer names the version its run's last patch brought the view to
  assert_eq!(second.version, snapshot.version);
  assert_eq!(snapshot.sync, SyncState::Idle);
  assert_eq!(snapshot.branches["auth"].sync.status, BranchStatus::Created);
}

/// The events of a run that is not in flight any more never reach the table: the run is the actor's, not the sender's
#[tokio::test]
async fn the_late_events_of_an_ended_run_are_dropped() {
  let host = Arc::new(ScriptedHost::default());
  host.push(auth_built(), Ok(report(false, &[])));
  host.push(auth_built(), Ok(report(false, &[])));
  let (sessions, key) = watched(&host).await;
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  let synced = snapshot(&sessions, &key).await;

  let reporters = host.reporters.lock().unwrap().clone();
  for reporter in reporters {
    reporter.send(grouping(vec![grouped("ghost", &["g1"])])).unwrap();
  }
  // a change outside a sync sends its patch at once, with whatever else changed
  assert_eq!(sessions.apply(&key, |_| {}).version().await, Some(synced.version));
  assert_eq!(snapshot(&sessions, &key).await, synced);
}

/// Focuses during a run cost one check of the inputs after it, and no run when the run saw them
#[tokio::test]
async fn a_focus_during_a_run_is_one_check_after_it() {
  let host = Arc::new(ScriptedHost::default());
  host.set_inputs(1);
  host.push(auth_built(), Ok(report(false, &[])));
  let gate = Arc::new(Notify::new());
  host.push_gated(auth_built(), Ok(report(false, &[])), Some(Arc::clone(&gate)));
  let (sessions, key) = watched(&host).await;
  assert_eq!(outcome(sessions.sync(&key, SyncReason::Open)).await, Ok(SyncOutcome::Synced));
  let checks_after_open = *host.checks.lock().unwrap();

  let manual = sessions.sync(&key, SyncReason::Manual);
  let focuses = [sessions.set_window_focus(&key, true).unwrap(), sessions.set_window_focus(&key, true).unwrap()];
  gate.notify_one();
  assert_eq!(outcome(manual).await, Ok(SyncOutcome::Synced));
  for focus in focuses {
    assert_eq!(outcome(focus).await, Ok(SyncOutcome::Unchanged));
  }
  assert_eq!(host.runs(), 2);
  // one read before the manual run, one check after the chain
  assert_eq!(*host.checks.lock().unwrap() - checks_after_open, 2);
}

/// A run whose report asks for a retry runs once more, its unbuilt branches `Syncing` meanwhile; a second ask is not
/// retried
#[tokio::test]
async fn a_moved_branch_is_retried_once_and_syncing_until_then() {
  let host = Arc::new(ScriptedHost::default());
  let gate = Arc::new(Notify::new());
  let mut moved = auth_built();
  moved[2] = branch_status("auth", BranchSyncStatus::Error);
  host.push(moved.clone(), Ok(report(true, &["auth"])));
  host.push_gated(Vec::new(), Ok(report(true, &["auth"])), Some(Arc::clone(&gate)));
  let (sessions, key) = watched(&host).await;
  let ticket = sessions.sync(&key, SyncReason::Manual);
  // the retry holds: the row says `Syncing`, and the sync still runs
  while host.runs() < 2 {
    tokio::task::yield_now().await;
  }
  let during = loop {
    let view = snapshot(&sessions, &key).await;
    if view.branches.get("auth").is_some_and(|branch| branch.sync.status == BranchStatus::Syncing) {
      break view;
    }
    tokio::task::yield_now().await;
  };
  assert!(matches!(during.sync, SyncState::Running { .. }));
  gate.notify_one();
  assert_eq!(outcome(ticket).await, Ok(SyncOutcome::Synced));
  assert_eq!(host.runs(), 2, "one retry");
}

/// A failed sync keeps the table of the sync before, its rows as they were, with the error; a deferred one keeps it
/// too and says nothing failed, only why it waits (`Deferred`), until the next run; its ticket gets the code
#[tokio::test]
async fn a_failed_sync_keeps_the_table() {
  let host = Arc::new(ScriptedHost::default());
  host.push(auth_built(), Ok(report(false, &[])));
  host.push(vec![grouping(vec![grouped("other", &["o1"])])], Err(ApiError::internal("broken")));
  let deferred = ApiError::new(ErrorCode::OperationInProgress, SyncDeferred::OperationInProgress("rebase").to_string());
  host.push(Vec::new(), Err(deferred.clone()));
  host.push(auth_built(), Ok(report(false, &[])));
  let (sessions, key) = watched(&host).await;
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  let synced = snapshot(&sessions, &key).await;

  assert_eq!(outcome(sessions.sync(&key, SyncReason::Manual)).await, Err(ApiError::internal("broken")));
  let failed = snapshot(&sessions, &key).await;
  assert_eq!(
    failed.sync,
    SyncState::Failed {
      error: ApiError::internal("broken")
    }
  );
  assert_eq!((&failed.order, &failed.branches), (&synced.order, &synced.branches));

  assert_eq!(outcome(sessions.sync(&key, SyncReason::Manual)).await, Err(deferred.clone()));
  let deferred_view = snapshot(&sessions, &key).await;
  assert_eq!(deferred_view.sync, SyncState::Deferred { error: deferred });
  assert_eq!(deferred_view.branches, synced.branches);

  assert_eq!(outcome(sessions.sync(&key, SyncReason::Manual)).await, Ok(SyncOutcome::Synced));
  assert_eq!(snapshot(&sessions, &key).await.sync, SyncState::Idle);
}

/// A failed sync is not repeated on every focus: a focus that finds what the failed run read reads the fingerprint (two
/// processes), answers the error and leaves the view as it is; a commit since runs again
#[tokio::test]
async fn a_focus_after_a_failed_sync_reads_only_the_fingerprint() {
  // no commit yet: the sync fails
  let repo = TestRepo::new();
  let key = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  let (git, sessions, window) = watched_with_focus(&key, true).await;
  let error = outcome(sessions.sync(&key, SyncReason::Open)).await.unwrap_err();
  let failed = snapshot(&sessions, &key).await;
  assert_eq!(failed.sync, SyncState::Failed { error: error.clone() });
  window.take();

  let before = git.process_stats().unwrap().counts();
  assert_eq!(outcome(sessions.set_window_focus(&key, true).unwrap()).await, Err(error.clone()));
  assert_eq!(processes_since(&git, &before), [("for-each-ref".to_string(), 1), ("rev-parse".to_string(), 1)]);
  assert_eq!(snapshot(&sessions, &key).await, failed, "no run");
  assert_eq!(window.take(), []);

  repo.create_commit("(auth) Add auth", "auth.txt", "auth");
  assert!(sessions.set_window_focus(&key, false).is_none());
  assert_eq!(
    outcome(sessions.set_window_focus(&key, true).unwrap()).await,
    Ok(SyncOutcome::Synced),
    "a commit since: the sync runs again"
  );
}

/// The environment of the app's sessions in a test: the default options, a deferred sync with its code, whether a
/// focus syncs (Auto-sync on Focus), the prefix the git config names (`None`: the session's own), and what the landing
/// needs and the view of the robots answer
#[derive(Default)]
struct TestEnvironment {
  syncs_on_focus: bool,
  config_prefix: Option<String>,
  facts: Arc<HostFacts>,
}

/// What the landing needs answer (nothing, by default) with the landed commits of each computation, and whether the view
/// of the robots fails
#[derive(Default)]
struct HostFacts {
  needs: Mutex<Needs>,
  computed: Mutex<Vec<HashSet<String>>>,
  fails_view: Mutex<bool>,
}

impl SessionEnvironment for TestEnvironment {
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    Ok(RunSetup {
      options: SyncOptions::default(),
      prefix: self.config_prefix.clone().unwrap_or_else(|| key.prefix.clone()),
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    match error.downcast_ref::<SyncDeferred>() {
      Some(deferred) => ApiError::new(deferred.code(), deferred.to_string()),
      None => ApiError::internal(format!("{error:#}")),
    }
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    self.syncs_on_focus
  }

  fn landing_needs(&self, _key: &SessionKey, landed: &HashSet<String>) -> anyhow::Result<Needs> {
    self.facts.computed.lock().unwrap_or_else(PoisonError::into_inner).push(landed.clone());
    Ok(self.facts.needs.lock().unwrap_or_else(PoisonError::into_inner).clone())
  }

  fn before_patronus_view(&self, _key: &SessionKey) -> Result<(), ApiError> {
    if *self.facts.fails_view.lock().unwrap_or_else(PoisonError::into_inner) {
      return Err(ApiError::internal("the test fails the view"));
    }
    Ok(())
  }
}

/// Sessions that sync with `git`, on a clock the test moves; a focus syncs
fn with_git(git: GitCommandExecutor) -> (Sessions, TestClock) {
  with_git_facts(git, Arc::default())
}

/// `with_git` whose host answers the landing needs and the view of the robots with `facts`
fn with_git_facts(git: GitCommandExecutor, facts: Arc<HostFacts>) -> (Sessions, TestClock) {
  with_environment(
    git,
    TestEnvironment {
      syncs_on_focus: true,
      facts,
      ..TestEnvironment::default()
    },
  )
}

/// Sessions that sync with `git` in `environment`
fn with_environment(git: GitCommandExecutor, environment: TestEnvironment) -> (Sessions, TestClock) {
  let clock = TestClock::new();
  let host = Arc::new(GitSessionHost::new(git, environment));
  (Sessions::new(tokio::runtime::Handle::current(), host, Arc::new(OneClock(Arc::new(clock.clone())))), clock)
}

/// The processes `git` started since its counts were `before`, by subcommand
fn processes_since(git: &GitCommandExecutor, before: &[(String, u32)]) -> Vec<(String, u32)> {
  let earlier = |command: &str| before.iter().find(|(name, _)| name == command).map_or(0, |(_, count)| *count);
  git
    .process_stats()
    .unwrap()
    .counts()
    .into_iter()
    .map(|(command, count)| {
      let since = count - earlier(&command);
      (command, since)
    })
    .filter(|(_, count)| *count > 0)
    .collect()
}

/// A repository on a bare `origin` that has its first commit, with a commit ahead of it for each subject
fn ahead_of_origin(subjects: &[&str]) -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let origin = repo.path().join(".git/origin.git");
  let origin = origin.to_str().unwrap();
  repo.git(&["init", "--bare", "-q", "--initial-branch=master", origin]);
  repo.git(&["remote", "add", "origin", origin]);
  repo.git(&["push", "-q", "origin", "master"]);
  for (index, subject) in subjects.iter().enumerate() {
    repo.create_commit(subject, &format!("file{index}.txt"), subject);
  }
  repo
}

/// The branches of a view by name
fn branch_names(view: &RepositoryView) -> Vec<&str> {
  let mut names: Vec<&str> = view.order.iter().map(String::as_str).collect();
  names.sort_unstable();
  names
}

/// Two worktrees of one repository are two sessions, each with the table of its own HEAD
#[tokio::test]
async fn two_worktrees_of_one_repository_are_two_sessions() {
  let repo = ahead_of_origin(&["(alpha) Alpha", "(beta) Beta"]);
  let lane = TestWorktree::detached(&repo);
  lane.git(&["checkout", "-q", "-b", "work", "origin/master"]);
  lane.write("gamma.txt", "gamma");
  lane.git(&["add", "gamma.txt"]);
  lane.git(&["commit", "-q", "-m", "(gamma) Add gamma"]);

  let (sessions, _clock) = with_git(GitCommandExecutor::new());
  let main = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  // each agent lane has a prefix of its own, or the syncs of both would archive each other's branches
  let other = SessionKey {
    worktree: lane.path.clone(),
    prefix: "lane".to_string(),
  };
  for key in [&main, &other] {
    sessions.subscribe(key, Box::new(Window::default())).await.unwrap();
    assert_eq!(outcome(sessions.sync(key, SyncReason::Open)).await, Ok(SyncOutcome::Synced));
  }
  let (main_view, other_view) = (snapshot(&sessions, &main).await, snapshot(&sessions, &other).await);
  assert_eq!(branch_names(&main_view), ["alpha", "beta"]);
  assert_eq!(branch_names(&other_view), ["gamma"]);
  assert_ne!(main_view.version.epoch, other_view.version.epoch);
  // the table each sync finished is the table of the session of its worktree and prefix, which the guards read
  for key in [&main, &other] {
    let table = sessions.table(key).await.expect("the session keeps the table of its sync");
    assert_eq!(table.table.branches.len(), branch_names(&snapshot(&sessions, key).await).len(), "{key:?}");
  }
}

/// Closing the sessions of a worktree drops its windows at once, whatever keeps the session; the sessions of another
/// worktree stay
#[tokio::test]
async fn closing_a_worktree_drops_its_sessions_and_their_windows() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let (gone, kept) = (key("/gone"), key("/kept"));
  let window = Window::default();
  sessions.subscribe(&gone, Box::new(window.clone())).await.unwrap();
  let other_prefix = SessionKey {
    prefix: "other".to_string(),
    ..gone.clone()
  };
  sessions.subscribe(&other_prefix, Box::new(Window::default())).await.unwrap();
  sessions.subscribe(&kept, Box::new(Window::default())).await.unwrap();

  assert_eq!(sessions.close_worktree("/gone"), 2);
  assert!(!sessions.is_open(&gone) && !sessions.is_open(&other_prefix));
  assert!(sessions.is_open(&kept));
  // the actor dropped the subscriber: the window's stream ends
  while Arc::strong_count(&window.patches) > 1 {
    tokio::task::yield_now().await;
  }
}

/// A session whose last window left runs nothing: a focus starts no git process; it is removed once idle for
/// `IDLE_LIMIT` on its clock, and opened again it starts a new epoch
#[tokio::test]
async fn a_session_without_subscribers_runs_nothing_and_goes_away() {
  let repo = ahead_of_origin(&["(auth) Add auth"]);
  let git = GitCommandExecutor::new().with_process_stats("sessions");
  let (sessions, clock) = with_git(git.clone());
  let key = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  let (id, _) = sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  sessions.sync(&key, SyncReason::Open).wait().await.unwrap();
  let epoch = snapshot(&sessions, &key).await.version.epoch;
  sessions.unsubscribe(id);

  let processes = git.process_stats().unwrap().total();
  repo.create_commit("(ui) Add ui", "ui.rs", "// ui");
  let focus = sessions.set_window_focus(&key, true).unwrap();
  assert_eq!(outcome(focus).await, Ok(SyncOutcome::Skipped));
  assert_eq!(git.process_stats().unwrap().total(), processes, "no git process for a session no window shows");

  clock.advance(IDLE_LIMIT.checked_sub(Duration::from_secs(1)).unwrap());
  assert_eq!(sessions.sweep(), Vec::<SessionKey>::new());
  assert!(sessions.is_open(&key));
  clock.advance(Duration::from_secs(1));
  assert_eq!(sessions.sweep(), std::slice::from_ref(&key));
  assert!(!sessions.is_open(&key));
  assert_eq!(sessions.get_snapshot(&key).await, None);

  let (_, reopened) = sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  assert_ne!(reopened.version.epoch, epoch, "a new epoch: a window of the old session asks for a snapshot");
  assert_eq!(reopened.version.seq, 0);
}

/// The sessions of a git that counts its processes, Auto-sync on Focus `syncs_on_focus`, with a window on `key`
async fn watched_with_focus(key: &SessionKey, syncs_on_focus: bool) -> (GitCommandExecutor, Sessions, Window) {
  let git = GitCommandExecutor::new().with_process_stats("sessions");
  let environment = TestEnvironment {
    syncs_on_focus,
    ..TestEnvironment::default()
  };
  let (sessions, _clock) = with_environment(git.clone(), environment);
  let window = Window::default();
  sessions.subscribe(key, Box::new(window.clone())).await.unwrap();
  (git, sessions, window)
}

/// A focus of a session a window shows, with nothing a sync reads changed, reads the fingerprint (two processes) and
/// syncs nothing; a lost focus and, with Auto-sync on Focus off, a gained one ask nothing and start no git process
#[tokio::test]
async fn a_focus_with_unchanged_inputs_reads_only_the_fingerprint() {
  let repo = ahead_of_origin(&["(auth) Add auth"]);
  let key = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  for syncs_on_focus in [true, false] {
    let (git, sessions, window) = watched_with_focus(&key, syncs_on_focus).await;
    sessions.sync(&key, SyncReason::Open).wait().await.unwrap();
    let synced = snapshot(&sessions, &key).await;
    window.take();

    let before = git.process_stats().unwrap().counts();
    assert!(sessions.set_window_focus(&key, false).is_none(), "a lost focus asks nothing");
    let focus = sessions.set_window_focus(&key, true);
    if syncs_on_focus {
      assert_eq!(outcome(focus.unwrap()).await, Ok(SyncOutcome::Unchanged));
      assert_eq!(processes_since(&git, &before), [("for-each-ref".to_string(), 1), ("rev-parse".to_string(), 1)]);
    } else {
      assert!(focus.is_none(), "Auto-sync on Focus off: the session is told, nothing syncs");
      assert_eq!(processes_since(&git, &before), []);
    }
    // no run: the view stays at its version, and the window got no patch
    assert_eq!(snapshot(&sessions, &key).await, synced);
    assert_eq!(window.take(), []);
  }
}

/// What a push of the virtual branch `branch` sends
fn pushing(branch: &str) -> Vec<RemoteSubject> {
  vec![RemoteSubject::Branch(branch.to_string())]
}

/// The remote statuses a push of `branch` left: the remote at `tip`
fn pushed(branch: &str, tip: &str) -> Vec<RemoteStatusUpdate> {
  vec![remote_at(branch, tip)]
}

/// The push of `subjects` in the session of `key` as the engine tells it (`operations::land_execute`): begun, then
/// ended with the remote statuses it left, or failed; answers the version of the patch of its end
async fn push(sessions: &Sessions, key: &SessionKey, subjects: Vec<RemoteSubject>, outcome: anyhow::Result<Vec<RemoteStatusUpdate>>) -> anyhow::Result<Option<Version>> {
  drop(sessions.begin_push(key, subjects.clone()));
  let pushed = outcome.is_ok();
  let version = sessions.tell_push(key, subjects, outcome.as_ref().ok().cloned(), pushed).version().await;
  outcome.map(|_| version)
}

/// The remote tip `branch` shows in the view
fn remote_tip(view: &RepositoryView, branch: &str) -> Option<String> {
  view.branches.get(branch)?.remote.status.as_ref()?.remote_tip.clone()
}

/// A window on a session synced once by `events`
async fn synced_window(sessions: &Sessions, host: &ScriptedHost, worktree: &str, events: Vec<SyncEvent>) -> (SessionKey, Window) {
  host.push(events, Ok(report(false, &[])));
  let key = key(worktree);
  let window = Window::default();
  sessions.subscribe(&key, Box::new(window.clone())).await.unwrap();
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  window.take();
  (key, window)
}

/// A push without a sync: its outcome reaches the window as one patch of the remote status it left, and the answer names
/// the version of that patch; nothing syncs. A failed push changes nothing, after which a sync shows what it computes
/// again
#[tokio::test]
async fn a_push_without_a_sync_patches_the_remote_status_it_left() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let (key, window) = synced_window(&sessions, &host, "/repo", auth_built()).await;

  let version = push(&sessions, &key, pushing("auth"), Ok(pushed("auth", "pushed"))).await.unwrap();
  let patches = window.take();
  assert_eq!(patches.len(), 1, "{patches:?}");
  let change = &patches[0].changes.branches["auth"];
  assert_eq!((change.head.is_none(), change.sync.is_none()), (true, true), "only the remote status: {change:?}");
  assert_eq!(version, Some(patches[0].next));
  assert_eq!(remote_tip(&snapshot(&sessions, &key).await, "auth").as_deref(), Some("pushed"));
  assert_eq!(host.runs(), 1, "no sync after the push");

  push(&sessions, &key, pushing("auth"), Err(anyhow::anyhow!("rejected"))).await.unwrap_err();
  let mut events = auth_built();
  events.push(SyncEvent::RemoteStatusUpdate(remote_at("auth", "fetched")));
  host.push(events, Ok(report(false, &[])));
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  assert_eq!(remote_tip(&snapshot(&sessions, &key).await, "auth").as_deref(), Some("fetched"), "no push runs any more");
}

/// A focus right after the session's own push syncs nothing (C4): the push moved the remote-tracking ref of what it sent,
/// which a sync reads, and the session puts the ref into the inputs it knows without a git process, so the check reads
/// the two processes of the inputs and finds them unchanged. A remote-tracking ref moved from elsewhere still syncs
#[tokio::test]
async fn a_focus_after_the_sessions_own_push_runs_no_sync() {
  let repo = ahead_of_origin(&["(auth) Add auth"]);
  let key = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  let git = GitCommandExecutor::new().with_process_stats("sessions");
  let (sessions, _clock) = with_git(git.clone());
  sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  assert_eq!(outcome(sessions.sync(&key, SyncReason::Open)).await, Ok(SyncOutcome::Synced));

  let tip = repo.rev_parse("test/virtual/auth").unwrap();
  drop(sessions.begin_push(&key, pushing("auth")));
  repo.git(&["push", "-q", "origin", "test/virtual/auth"]);
  sessions
    .tell_push(&key, pushing("auth"), Some(vec![remote_at("auth", &tip)]), true)
    .version()
    .await
    .unwrap();
  assert_eq!(repo.rev_parse("refs/remotes/origin/test/virtual/auth").unwrap(), tip, "the push moved the ref a sync reads");
  let before = git.process_stats().unwrap().counts();
  assert_eq!(outcome(sessions.set_window_focus(&key, true).unwrap()).await, Ok(SyncOutcome::Unchanged));
  assert_eq!(processes_since(&git, &before), [("for-each-ref".to_string(), 1), ("rev-parse".to_string(), 1)]);

  let baseline = repo.rev_parse("origin/master").unwrap();
  repo.git(&["update-ref", "refs/remotes/origin/test/virtual/auth", &baseline]);
  assert_eq!(outcome(sessions.set_window_focus(&key, true).unwrap()).await, Ok(SyncOutcome::Synced));
}

/// The session keeps the table of its last successful sync with the memo of its views: none before the first sync, the
/// one a sync finished after it, and that one still after a sync that failed or was deferred
#[tokio::test]
async fn the_session_keeps_the_table_of_its_last_successful_sync() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, repo) = watched(&host).await;
  assert!(sessions.table(&repo).await.is_none(), "no sync yet");
  host.push(auth_built(), Ok(report(false, &[])));
  sessions.sync(&repo, SyncReason::Manual).wait().await.unwrap();
  let synced = sessions.table(&repo).await.expect("the table of the sync");

  let deferred = ApiError::new(ErrorCode::OperationInProgress, "Not synced: rebase is in progress");
  host.push(Vec::new(), Err(deferred));
  host.push(Vec::new(), Err(ApiError::internal("the sync failed")));
  sessions.sync(&repo, SyncReason::Manual).wait().await.unwrap_err();
  sessions.sync(&repo, SyncReason::Manual).wait().await.unwrap_err();
  let kept = sessions.table(&repo).await.expect("the table of the last successful sync");
  assert!(Arc::ptr_eq(&kept.table, &synced.table));
  assert!(Arc::ptr_eq(&kept.memo, &synced.memo));
  assert!(sessions.table(&key("/other")).await.is_none(), "a session that is not open");
}

/// The outcome of a push goes to the session of the key its command resolved: the branch of the same name in another
/// worktree's session (the window switched to it while the push ran) keeps what it showed, and its window gets nothing
#[tokio::test]
async fn the_outcome_of_a_push_in_one_session_leaves_the_branch_of_the_same_name_in_another() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let (pushed_in, _window) = synced_window(&sessions, &host, "/a", auth_built()).await;
  let (other, other_window) = synced_window(&sessions, &host, "/b", auth_built()).await;
  let before = snapshot(&sessions, &other).await;

  push(&sessions, &pushed_in, pushing("auth"), Ok(pushed("auth", "pushed"))).await.unwrap();
  assert_eq!(remote_tip(&snapshot(&sessions, &pushed_in).await, "auth").as_deref(), Some("pushed"));
  assert_eq!(snapshot(&sessions, &other).await, before);
  assert_eq!(other_window.take(), []);
}

/// The deletion of an archived or unapplied branch is a patch of its section, the version of which the change answers;
/// no sync runs for it
#[tokio::test]
async fn deleting_an_archived_or_unapplied_branch_answers_a_patch_and_runs_no_sync() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let mut events = auth_built();
  events.pop();
  events.push(SyncEvent::ArchivedBranchesFound {
    branch_names: vec!["test/archived/old".to_string()],
    newly_archived: Vec::new(),
  });
  events.push(SyncEvent::UnappliedBranchesFound {
    branches: vec![unapplied("docs", None)],
  });
  let (key, window) = synced_window(&sessions, &host, "/repo", events).await;

  let version = sessions.apply(&key, |state| state.remove_archived("test/archived/old")).version().await;
  let patches = window.take();
  assert_eq!(patches.len(), 1);
  assert_eq!(patches[0].changes.archived.as_ref().and_then(|archived| archived.names.clone()), Some(Vec::new()));
  assert_eq!(version, Some(patches[0].next));

  let version = sessions.apply(&key, |state| state.remove_unapplied("test/unapplied/docs")).version().await;
  let patches = window.take();
  assert_eq!((patches.len(), patches[0].changes.unapplied.clone()), (1, Some(Vec::new())));
  assert_eq!(version, Some(patches[0].next));
  assert_eq!(host.runs(), 1, "no sync after a deletion");
  let not_open = self::key("/not-open");
  assert_eq!(sessions.apply(&not_open, |state| state.remove_archived("test/archived/old")).version().await, None);
}

/// A run whose git config names the branches by another prefix than its session's (a `git config` in a terminal before
/// any command resolved the prefix again) builds nothing: its request gets `prefix_changed`, which says nothing failed,
/// and no branch is written under the prefix the config left
#[tokio::test]
async fn a_run_under_a_prefix_the_config_left_builds_nothing() {
  let repo = ahead_of_origin(&["(auth) Add auth"]);
  let environment = TestEnvironment {
    syncs_on_focus: true,
    config_prefix: Some("team".to_string()),
    ..TestEnvironment::default()
  };
  let (sessions, _clock) = with_environment(GitCommandExecutor::new(), environment);
  let key = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  let (_, empty) = sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();

  let refused = outcome(sessions.sync(&key, SyncReason::Manual)).await.unwrap_err();
  assert_eq!(refused.code, ErrorCode::PrefixChanged, "{refused:?}");
  let view = snapshot(&sessions, &key).await;
  assert_eq!(view.sync, SyncState::Idle, "nothing failed: the window resolves the prefix again");
  assert_eq!(view.order, empty.order);
  assert_eq!(
    repo.git(&["for-each-ref", "--format=%(refname)", "refs/heads/test/", "refs/heads/team/"]),
    "",
    "no branch built"
  );
}

/// A session whose actor panicked opens again in a new epoch: `sweep` removes it at once (its subscribers, which the
/// actor never counted out, went with it), and so does the next call that opens it, whose sync runs in the fresh session
#[tokio::test]
async fn a_session_whose_actor_panicked_opens_again_in_a_new_epoch() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let (key, _window) = synced_window(&sessions, &host, "/repo", auth_built()).await;
  let epoch = snapshot(&sessions, &key).await.version.epoch;
  let panic_in_actor = |sessions: &Sessions| sessions.apply(&key, |_| panic!("a defect of the actor")).version();

  assert_eq!(panic_in_actor(&sessions).await, None, "the change never got its patch");
  assert_eq!(sessions.get_snapshot(&key).await, None, "a window of the dead session subscribes again");
  assert_eq!(sessions.sweep(), std::slice::from_ref(&key), "removed without waiting for IDLE_LIMIT");

  let (_, reopened) = sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  assert_ne!(reopened.version.epoch, epoch);
  assert_eq!(panic_in_actor(&sessions).await, None);
  host.push(auth_built(), Ok(report(false, &[])));
  assert_eq!(outcome(sessions.sync(&key, SyncReason::Manual)).await, Ok(SyncOutcome::Synced));
  let fresh = snapshot(&sessions, &key).await;
  assert!(fresh.version.epoch != epoch && fresh.version.epoch != reopened.version.epoch, "{fresh:?}");
  assert_eq!(fresh.branches["auth"].sync.status, BranchStatus::Created);
}

/// The patches a window got, each with when it came on the runtime's clock
#[derive(Clone, Default)]
struct TimedWindow {
  patches: Arc<Mutex<Vec<(Instant, ViewPatch)>>>,
}

impl Subscriber for TimedWindow {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.patches.lock().map_err(|_poisoned| SubscriberGone)?.push((Instant::now(), patch.clone()));
    Ok(())
  }
}

/// While the events of a run stream, its patches go out at most one per `PATCH_INTERVAL` (B15), so a run of T ms costs
/// ⌈T / 16⌉ + 2 patches at most (B1), not one per event; the end of the build goes out at once with the event that ended
/// it (`UnappliedBranchesFound`): the tips are known before the detection and the update check end, whose own patches
/// and the run's end come after it
#[tokio::test(start_paused = true)]
async fn the_patches_of_a_run_keep_the_interval_and_leave_at_once_when_the_build_ends() {
  const PACE: Duration = Duration::from_millis(3);
  let commits: Vec<String> = (0..32).map(|index| format!("c{index}")).collect();
  let commits: Vec<&str> = commits.iter().map(String::as_str).collect();
  let mut events = vec![grouping(vec![grouped("auth", &commits)])];
  events.extend(commits.iter().map(|commit| synced("auth", commit, CommitSyncStatus::Created)));
  events.push(branch_status("auth", BranchSyncStatus::Created));
  let build_end = events.len();
  events.push(SyncEvent::UnappliedBranchesFound { branches: Vec::new() });
  // the detection: the remote status of the branch moves on
  events.extend((0..5).map(|index| SyncEvent::RemoteStatusUpdate(remote_at("auth", &format!("tip{index}")))));
  let event_count = events.len();
  let host = Arc::new(ScriptedHost::default());
  host.push_paced(events, Ok(report(false, &[])), PACE);
  let (sessions, _clock) = scripted(&host);
  let key = key("/repo");
  let window = TimedWindow::default();
  let (_, empty) = sessions.subscribe(&key, Box::new(window.clone())).await.unwrap();

  let started = Instant::now();
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  let elapsed = started.elapsed();
  let patches = std::mem::take(&mut *window.patches.lock().unwrap());
  let sent_at = host.sent_at.lock().unwrap().clone();
  assert_eq!(sent_at.len(), event_count);

  let budget = usize::try_from(elapsed.as_millis().div_ceil(PATCH_INTERVAL.as_millis())).unwrap() + 2;
  assert!(
    patches.len() <= budget,
    "{} patches for {event_count} events in {elapsed:?}, B1 allows {budget}",
    patches.len()
  );
  let detection = patches
    .iter()
    .position(|(_, patch)| patch.changes.sync == Some(SyncState::Running { step: SyncStep::Detection }))
    .expect("a patch that says the detection runs");
  assert_eq!(patches[detection].0, sent_at[build_end], "the end of the build is not held for the interval");
  assert!(patches[detection].1.changes.branches.contains_key("auth"), "the built branch comes with it");
  let last = patches.len() - 1;
  assert!(detection < last, "the detection and the end of the run come in later patches");
  assert_eq!(patches[last].1.changes.sync, Some(SyncState::Idle));
  // the patches between the start, the end of the build and the end of the run come by the interval
  for index in (1..last).filter(|index| *index != detection) {
    let since = patches[index].0 - patches[index - 1].0;
    assert!(since >= PATCH_INTERVAL, "patch {index} came {since:?} after the one before");
  }
  // each patch follows the one before, and together they make the snapshot
  let mut view = empty;
  for (_, patch) in patches {
    crate::patch::apply_patch(&mut view, patch).unwrap();
  }
  assert_eq!(view, snapshot(&sessions, &key).await);
}

fn names(names: &[&str]) -> Vec<String> {
  names.iter().map(ToString::to_string).collect()
}

/// A window tells its session what it shows and needs: the session keeps the demand of each subscription, in place of
/// what that window said before, and asks for the union of all its windows; the demand of a window goes with its
/// subscription, and the demand of a subscription of another session is that session's
#[tokio::test]
async fn the_demand_of_a_session_is_the_union_of_its_windows_and_goes_with_each() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let (repo, other) = (key("/repo"), key("/other"));
  let (main, _) = sessions.subscribe(&repo, Box::new(Window::default())).await.unwrap();
  let (review, _) = sessions.subscribe(&repo, Box::new(Window::default())).await.unwrap();
  let (elsewhere, _) = sessions.subscribe(&other, Box::new(Window::default())).await.unwrap();
  assert_eq!(sessions.demand(&repo).await, Some(Demand::default()));

  let shown = Demand {
    focused: true,
    online: true,
    visible_branches: names(&["ui", "auth"]),
    needed: names(&["auth"]),
    ..Demand::default()
  };
  sessions.set_demand(main, &shown);
  let reviewed = Demand {
    online: true,
    visible_branches: names(&["api", "auth"]),
    watched_robots: names(&["robot-7"]),
    ..Demand::default()
  };
  sessions.set_demand(review, &reviewed);
  let details = Demand {
    robot_details: names(&["robot-1"]),
    ..Demand::default()
  };
  sessions.set_demand(elsewhere, &details);
  let both = Demand {
    focused: true,
    online: true,
    visible_branches: names(&["api", "auth", "ui"]),
    needed: names(&["auth"]),
    watched_robots: names(&["robot-7"]),
    ..Demand::default()
  };
  assert_eq!(sessions.demand(&repo).await, Some(both));

  sessions.set_demand(main, &Demand::default());
  assert_eq!(sessions.demand(&repo).await, Some(reviewed), "the main window said it shows nothing now");
  sessions.unsubscribe(review);
  assert_eq!(sessions.demand(&repo).await, Some(Demand::default()), "gone with the subscription");
  assert_eq!(sessions.demand(&other).await, Some(details));
  // the details a window shows are in the view from its demand on, `Loading` until read, and go with it
  let shown_details = |view: RepositoryView| view.facts.robot_details.into_iter().map(|(id, fact)| (id, fact.state)).collect::<Vec<_>>();
  assert_eq!(shown_details(snapshot(&sessions, &other).await), vec![("robot-1".to_owned(), FactState::Loading)]);
  sessions.set_demand(elsewhere, &Demand::default());
  assert_eq!(sessions.demand(&other).await, Some(Demand::default()));
  assert_eq!(shown_details(snapshot(&sessions, &other).await), vec![]);
}

/// The review refs are those of the pull requests the review windows of the session show: each change of them (a
/// window shows one, another, none, or goes) prunes the others, and a demand that keeps them prunes nothing
#[tokio::test]
async fn a_change_of_the_review_windows_prunes_the_review_refs_of_the_others() {
  let host = Arc::new(ScriptedHost::default());
  let (sent, mut prunes) = tokio::sync::mpsc::unbounded_channel();
  *host.prunes.lock().unwrap() = Some(sent);
  let (sessions, _clock) = scripted(&host);
  let repo = key("/repo");
  let (main, _) = sessions.subscribe(&repo, Box::new(Window::default())).await.unwrap();
  let (review, _) = sessions.subscribe(&repo, Box::new(Window::default())).await.unwrap();
  let windows = |numbers: &[u32]| Demand {
    review_windows: numbers.to_vec(),
    ..Demand::default()
  };
  sessions.set_demand(main, &windows(&[7]));
  assert_eq!(prunes.recv().await, Some(vec![7]), "a window shows #7: the refs of the others go");
  sessions.set_demand(review, &windows(&[7, 8]));
  assert_eq!(prunes.recv().await, Some(vec![7, 8]));
  // the same windows with a focus, and the main window leaving #7 to the other, prune nothing: the next prune is the
  // one of the last window going
  sessions.set_demand(
    review,
    &Demand {
      focused: true,
      ..windows(&[8, 7])
    },
  );
  sessions.set_demand(main, &windows(&[]));
  sessions.unsubscribe(review);
  assert_eq!(prunes.recv().await, Some(vec![]), "no review window: every review ref goes");
}

/// A timer of a session waits on the session clock: on a test clock until an advance reaches its time, not before; a
/// time that has come waits nothing
#[tokio::test]
async fn a_timer_waits_on_the_test_clock_until_an_advance_reaches_its_time() {
  let clock = TestClock::new();
  sleep_until(&clock, Duration::ZERO).await;
  let timer = tokio::spawn({
    let clock = clock.clone();
    async move { sleep_until(&clock, Duration::from_mins(1)).await }
  });
  for _ in 0..10 {
    tokio::task::yield_now().await;
  }
  clock.advance(Duration::from_secs(59));
  for _ in 0..10 {
    tokio::task::yield_now().await;
  }
  assert!(!timer.is_finished(), "59 s of 60");
  clock.advance(Duration::from_secs(1));
  tokio::time::timeout(Duration::from_secs(5), timer).await.unwrap().unwrap();
}

/// What the scheduler of the facts heard of the session since the last call, by kind
fn heard(events: &mut tokio::sync::mpsc::UnboundedReceiver<SessionEvent>) -> Vec<String> {
  let mut kinds = Vec::new();
  while let Ok(SessionEvent { kind, .. }) = events.try_recv() {
    kinds.push(match kind {
      SessionEventKind::Watched(watched) => format!("watched {watched}"),
      SessionEventKind::Closed => "closed".to_owned(),
      SessionEventKind::Focus(focused) => format!("focus {focused}"),
      SessionEventKind::Demand(demand) => format!("demand focused {}", demand.focused),
      SessionEventKind::TableCommitted { .. } => "table committed".to_owned(),
      SessionEventKind::PushEnded { heads } => format!("pushed {:?}", heads.iter().map(|head| head.subject.name()).collect::<Vec<_>>()),
      SessionEventKind::PullRequestWritten => "pull request written".to_owned(),
      SessionEventKind::Robots {
        digest: RobotsDigest { active, .. },
        ..
      } => format!("robots {:?}", active.iter().map(|robot| robot.id.as_str()).collect::<Vec<_>>()),
      SessionEventKind::Readiness(digest) => format!("readiness {:?}", digest.subjects.iter().map(|subject| subject.key.as_str()).collect::<Vec<_>>()),
      SessionEventKind::Refresh { .. } => "refresh".to_owned(),
    });
  }
  kinds
}

/// The scheduler hears what the facts of a session are asked by: a window watches it, its table is committed, the
/// robots the poll asks about (those of its branches, on the tips of the table); a read numbers itself when it starts,
/// its answer reaches the windows as a patch, and the answer of a read of a session that closed meanwhile applies to the
/// session opened again under the key no more
#[tokio::test]
async fn the_reads_of_the_facts_reach_the_windows_and_the_scheduler_hears_what_it_asks_by() {
  let host = Arc::new(ScriptedHost::default());
  host.push(auth_built(), Ok(report(false, &[])));
  let (sessions, _clock) = scripted(&host);
  let mut events = sessions.events().unwrap();
  assert!(sessions.events().is_none(), "one consumer");
  let key = key("/repo");
  let window = Window::default();
  sessions.subscribe(&key, Box::new(window.clone())).await.unwrap();
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  assert_eq!(heard(&mut events), vec!["watched true", "table committed"]);

  let ticket = sessions.begin(&key).await.unwrap();
  let listed = PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots: vec![robot("auth-robot", Some("auth"), PatronusRobotStatus::Running, 10)],
    github_repository: None,
  };
  let read = RobotsRead::List { full: true };
  let applied = sessions.apply_facts(&key, ticket, FactResult::Robots { read, result: listed.clone() }).await.unwrap();
  assert_eq!(heard(&mut events), vec![r#"robots ["auth-robot"]"#]);
  let version = applied.version;
  assert_eq!(
    applied.robots.unknown_failing,
    vec!["auth-robot".to_owned()],
    "a running robot of the table, not known failing"
  );
  // the view of the robots derives from the answer in the blocking pool: its patch may have come after this one already
  let patch = window.take().into_iter().find(|patch| patch.next == version).expect("the patch of the answer");
  assert_eq!(patch.changes.facts.unwrap().robots.unwrap().robots.keys().collect::<Vec<_>>(), vec!["auth-robot"]);

  // a start's robot: newer than every read before, polled from now on
  let started = robot("started", Some("auth"), PatronusRobotStatus::Running, 20);
  sessions.write_robot(&key, started).await.unwrap();
  assert_eq!(heard(&mut events), vec![r#"robots ["started"]"#]);
  assert_eq!(snapshot(&sessions, &key).await.facts.robots.robots["started"].status, PatronusRobotStatus::Running);

  let late = sessions.begin(&key).await.unwrap();
  sessions.close_worktree("/repo");
  sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  assert_eq!(heard(&mut events), vec!["closed", "watched true"]);
  assert_eq!(sessions.apply_facts(&key, late, FactResult::Robots { read, result: listed }).await, None);
  assert!(snapshot(&sessions, &key).await.facts.robots.robots.is_empty());
}

/// The readiness: a subject a window needs is in the view at once (`Loading` until Patronus answers), the scheduler hears
/// what to ask with the tip the row shows; its answer reaches the windows; a new tip makes it `Stale` in the patch that
/// shows the tip, and Check Approvals Again makes it `Stale` at once and tells the scheduler to recheck it
#[tokio::test]
async fn the_readiness_a_window_needs_is_in_its_view_and_stale_in_the_patch_of_a_new_tip() {
  let host = Arc::new(ScriptedHost::default());
  host.push(auth_built(), Ok(report(false, &[])));
  let (sessions, _clock) = scripted(&host);
  let sessions = Arc::new(sessions);
  let mut events = sessions.events().unwrap();
  let key = key("/repo");
  let window = Window::default();
  let (id, _) = sessions.subscribe(&key, Box::new(window.clone())).await.unwrap();
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  let ticket = sessions.begin(&key).await.unwrap();
  let listed = PatronusRobotsResult::from(PatronusAvailability::Available);
  sessions
    .apply_facts(
      &key,
      ticket,
      FactResult::Robots {
        read: RobotsRead::List { full: true },
        result: listed,
      },
    )
    .await
    .unwrap();
  drop(heard(&mut events));

  let needs = Demand {
    needed: vec!["auth".to_owned()],
    ..Demand::default()
  };
  sessions.set_demand(id, &needs);
  assert_eq!(snapshot(&sessions, &key).await.facts.readiness["auth"].state, FactState::Loading);
  assert_eq!(heard(&mut events), vec!["demand focused false", r#"readiness ["auth"]"#]);
  let subject = sessions.readiness_digest(&key).await.subjects.remove(0);
  assert_eq!((subject.ask.tip.as_str(), subject.recheck), ("copy-a1", false));

  let ticket = sessions.begin(&key).await.unwrap();
  let answer = ReadinessAnswer {
    result: Ok(BranchReviewReadiness {
      branch_name: "auth".to_owned(),
      tip: "copy-a1".to_owned(),
      requirement: None,
      readiness: ReviewReadiness::NotRequired,
    }),
    subject,
  };
  sessions.apply_facts(&key, ticket, FactResult::Readiness(vec![answer])).await.unwrap();
  let fact = snapshot(&sessions, &key).await.facts.readiness["auth"].clone();
  assert_eq!(
    (fact.state, fact.value.map(|value| value.readiness)),
    (FactState::Fresh, Some(ReviewReadiness::NotRequired))
  );

  // the copy of a new sync moves the tip: the patch that shows it has the readiness stale
  drop(window.take());
  sessions
    .apply(&key, |state| state.apply(synced("auth", "a1", CommitSyncStatus::Created)))
    .version()
    .await
    .unwrap();
  let version = sessions
    .apply(&key, |state| {
      state.apply(SyncEvent::CommitSynced {
        branch_name: "auth".to_owned(),
        commit_hash: "a1".to_owned(),
        new_hash: "copy-a1-new".to_owned(),
        status: CommitSyncStatus::Created,
        resolved: None,
      });
    })
    .version()
    .await
    .unwrap();
  // the view of the robots and its landing needs derive in the blocking pool: their patches may still come after it
  let patch = window.take().into_iter().find(|patch| patch.next == version).expect("the patch of the new tip");
  assert!(patch.changes.branches.contains_key("auth"));
  assert_eq!(patch.changes.facts.unwrap().readiness["auth"].state, FactState::Stale { reason: None });
  drop(heard(&mut events));

  // Check Approvals Again: stale at once, the scheduler asks it with the recheck
  let refresh = tokio::spawn({
    let (sessions, key) = (Arc::clone(&sessions), key.clone());
    async move { sessions.refresh(&key, FactFamily::Readiness, FactRefreshReason::Manual, Some("auth".to_owned())).await }
  });
  let mut told = Vec::new();
  while told.len() < 2 {
    told.push(events.recv().await.unwrap().kind);
  }
  assert!(matches!(&told[0], SessionEventKind::Readiness(digest) if digest.subjects[0].recheck));
  let SessionEventKind::Refresh { subject, reply, .. } = told.remove(1) else {
    panic!("{told:?}")
  };
  assert_eq!(subject.as_deref(), Some("auth"));
  drop(reply);
  assert_eq!(refresh.await.unwrap(), None);
}

/// A listing of `acme/monorepo` with the pull request of `auth` into `master`, which alice approved, and what the token
/// may do there
fn approved_listing(viewer_permission: Option<ViewerPermission>) -> PullRequestsResult {
  let mut approved = pull_request("auth", 7);
  approved.reviewers.push(Reviewer {
    login: "alice".to_owned(),
    state: ReviewerState::Approved,
    requested: false,
  });
  PullRequestsResult {
    viewer_permission,
    pull_requests: [("auth".to_owned(), approved)].into(),
    ..PullRequestsResult::unavailable(PullRequestAvailability::Available {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    })
  }
}

/// The pull requests: a listing reaches the windows as a patch and tells the scheduler the bases whose rules it reads; a
/// retarget of the app applies at once, newer than the listing that started before it, which keeps it, and lists them
/// again (a close lists nothing); a push that pushed lists them again with the heads it left, an unapplied branch none
#[tokio::test]
async fn the_writes_of_the_pull_requests_apply_at_once_and_a_late_listing_keeps_them() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let mut events = sessions.events().unwrap();
  let (key, window) = synced_window(&sessions, &host, "/repo", auth_built()).await;
  let listed = approved_listing(Some(ViewerPermission::Write));
  let ticket = sessions.begin(&key).await.unwrap();
  let applied = sessions.apply_facts(&key, ticket, FactResult::PullRequests(listed.clone())).await.unwrap();
  assert_eq!(applied.rule_bases, vec!["master".to_owned()]);
  let patch = window.take().pop().unwrap();
  let pull_requests = patch.changes.facts.unwrap().pull_requests.unwrap();
  assert_eq!(pull_requests.pull_requests.keys().collect::<Vec<_>>(), vec!["auth"]);
  // what the token may do there goes with the answer
  assert_eq!(
    (pull_requests.repository.as_deref(), pull_requests.viewer_permission),
    (Some("acme/monorepo"), Some(ViewerPermission::Write))
  );
  drop(heard(&mut events));

  let late = sessions.begin(&key).await.unwrap();
  let retarget = PullRequestWrite::Retargeted {
    number: 7,
    base: "release".to_owned(),
  };
  let version = sessions.write_pull_request(&key, retarget).await.unwrap();
  assert_eq!(window.take().pop().unwrap().next, version);
  assert_eq!(heard(&mut events), vec!["pull request written"]);
  sessions.apply_facts(&key, late, FactResult::PullRequests(listed)).await.unwrap();
  assert_eq!(snapshot(&sessions, &key).await.facts.pull_requests.pull_requests["auth"].base_ref_name, "release");
  sessions.write_pull_request(&key, PullRequestWrite::Closed { number: 7 }).await.unwrap();
  assert_eq!(heard(&mut events), Vec::<String>::new(), "a close lists nothing");

  push(&sessions, &key, pushing("auth"), Ok(pushed("auth", "pushed"))).await.unwrap();
  assert_eq!(heard(&mut events), vec![r#"pushed ["auth"]"#]);
  let unapplied = vec![RemoteSubject::Unapplied("old".to_owned())];
  push(&sessions, &key, unapplied, Ok(pushed("old", "pushed"))).await.unwrap();
  assert_eq!(heard(&mut events), Vec::<String>::new(), "an unapplied branch has no pull request");
}

/// A repository that lands by a push into its target on GitHub reads the rules of the target besides those of the
/// approved bases, each once; the route a sync read reaches the header of the view
#[tokio::test]
async fn a_push_route_reads_the_rules_of_the_target() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let mut events = auth_built();
  let landing = LandingRemote {
    setting: Some(LandingSetting::Push),
    on_space: false,
    push_repository: Some("acme/monorepo".to_owned()),
    target: Some("main".to_owned()),
    remote: Some("origin".to_owned()),
  };
  if let SyncEvent::BranchesGrouped { landing: read, .. } = &mut events[0] {
    **read = landing.clone();
  }
  let (key, _window) = synced_window(&sessions, &host, "/repo", events).await;
  assert_eq!(snapshot(&sessions, &key).await.header.landing, landing);
  let ticket = sessions.begin(&key).await.unwrap();
  let applied = sessions.apply_facts(&key, ticket, FactResult::PullRequests(approved_listing(None))).await.unwrap();
  assert_eq!(applied.rule_bases, vec!["main".to_owned(), "master".to_owned()]);
}

/// What a push into the target told the session: the copy it landed lands its subject at the tip it pushed, until a
/// fetch shows it, so a second click pushes nothing; a refusal of the server stays for the gate of the route, which asks
/// about it before the next push, and a push that lands forgets it. The route reaches the view as its own part
#[tokio::test]
async fn a_push_into_the_target_lands_its_subject_and_a_refusal_of_the_server_stays_for_the_gate() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let mut events = auth_built();
  // off GitHub: no gate but the server's
  if let SyncEvent::BranchesGrouped { landing, .. } = &mut events[0] {
    **landing = LandingRemote {
      setting: Some(LandingSetting::Push),
      target: Some("master".to_owned()),
      remote: Some("origin".to_owned()),
      ..LandingRemote::default()
    };
  }
  // the table of the sync: `auth` built at its copy
  let table = BranchTable {
    prefix: "test".to_owned(),
    baseline: "origin/master".to_owned(),
    branches: vec![grouped("auth", &["a1"])],
    tips: [("test/virtual/auth".to_owned(), "copy-a1".to_owned())].into(),
    ..BranchTable::default()
  };
  host.push_table(events, Ok(report(false, &[])), table);
  let key = key("/repo");
  sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  let open = LandingRoute::Push {
    gate: RouteGate::Open,
    remote: Some("origin".to_owned()),
  };
  assert_eq!(snapshot(&sessions, &key).await.route, open);
  assert_eq!(snapshot(&sessions, &key).await.landed_states.get("auth"), None);

  let pushed = |tip: &str| BaselinePush {
    tip: tip.to_owned(),
    upstream: "origin/master".to_owned(),
  };
  let refusal = BaselineRefusal {
    target: "master".to_owned(),
    message: "remote: master is protected".to_owned(),
  };
  sessions.tell_baseline_push(&key, Vec::new(), Some(refusal)).await.unwrap();
  let view = snapshot(&sessions, &key).await;
  assert_eq!(
    view.route,
    LandingRoute::Push {
      gate: RouteGate::Confirm {
        confirmations: vec![Reason::BaselineRefusedPush {
          target: "master".to_owned(),
          message: "remote: master is protected".to_owned(),
        }],
      },
      remote: Some("origin".to_owned()),
    }
  );
  assert_eq!(view.landed_states.get("auth"), None, "a refused push landed nothing");

  sessions.tell_baseline_push(&key, vec![("auth".to_owned(), pushed("copy-a1"))], None).await.unwrap();
  let view = snapshot(&sessions, &key).await;
  assert_eq!(view.route, open, "the server took this one");
  assert_eq!(
    view.landed_states.get("auth").map(|landed| &landed.by),
    Some(&LandedBy::Baseline {
      upstream: "origin/master".to_owned()
    })
  );
  // a copy built anew since (another tip) is new work
  sessions.tell_baseline_push(&key, vec![("auth".to_owned(), pushed("older-copy"))], None).await.unwrap();
  assert_eq!(snapshot(&sessions, &key).await.landed_states.get("auth"), None);
}

/// The Patronus part of the session's view once `done` says it is, asked again until then (the view and the landing
/// needs are derived in the blocking pool)
async fn patronus_when(sessions: &Sessions, key: &SessionKey, done: impl Fn(&PatronusPart) -> bool) -> PatronusPart {
  for _ in 0..2000 {
    let part = snapshot(sessions, key).await.patronus;
    if done(&part) {
      return part;
    }
    tokio::time::sleep(Duration::from_millis(5)).await;
  }
  panic!("the Patronus part never came: {:?}", snapshot(sessions, key).await.patronus)
}

/// Patronus answered the session of `key` with `robots`, a full list
async fn list_robots(sessions: &Sessions, key: &SessionKey, robots: Vec<PatronusRobot>) {
  let ticket = sessions.begin(key).await.unwrap();
  let listed = PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots,
    github_repository: None,
  };
  let read = RobotsRead::List { full: true };
  sessions.apply_facts(key, ticket, FactResult::Robots { read, result: listed }).await.unwrap();
}

/// `util` adds a name `flow` uses, synced in a session with a window; the host answers that `flow` needs `util`
async fn util_and_flow(facts: &Arc<HostFacts>) -> (TestRepo, Sessions, SessionKey) {
  let repo = ahead_of_origin(&["(util) Add controlUrl", "(flow) Start with controlUrl"]);
  let declared_by = StackCommit {
    id: "util-commit".to_string(),
    subject: "Add controlUrl".to_string(),
    branch: "util".to_string(),
  };
  let need = LandingRefusal::MissingDeclaration {
    branch: "flow".to_string(),
    identifier: "controlUrl".to_string(),
    file: "src/b/Use.kt".to_string(),
    line: 3,
    used_by: StackCommit {
      branch: "flow".to_string(),
      ..declared_by.clone()
    },
    declared_by,
    fix: LandingFix::Restack {
      branch: "flow".to_string(),
      onto: "util".to_string(),
    },
    landing_in: None,
  };
  *facts.needs.lock().unwrap() = Needs::from([("flow".to_string(), vec![need])]);
  let (sessions, _clock) = with_git_facts(GitCommandExecutor::new(), Arc::clone(facts));
  let key = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  assert_eq!(outcome(sessions.sync(&key, SyncReason::Open)).await, Ok(SyncOutcome::Synced));
  (repo, sessions, key)
}

/// The session derives what the robots land, landed and hold once Patronus answered (`Loading` until then, the table
/// alone tells nothing), names the robot of each row, then computes the landing needs of the table with what the view
/// names landed, once: a missing declaration names the running Safe Push of the branch that adds the name, and an
/// answer of the same robots derives and computes nothing again
#[tokio::test]
async fn the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered() {
  let facts = Arc::new(HostFacts::default());
  let (_repo, sessions, key) = util_and_flow(&facts).await;
  let before = snapshot(&sessions, &key).await.patronus;
  assert_eq!((before.state, before.view), (FactState::Loading, PatronusView::default()), "Patronus has not answered");

  let table = sessions.table(&key).await.unwrap();
  let util_tip = table.table.tip_of("util").unwrap().to_string();
  let running = PatronusRobot {
    source_revision: Some(util_tip),
    ..robot("robot-1", Some("util"), PatronusRobotStatus::Running, 100)
  };
  list_robots(&sessions, &key, vec![running.clone()]).await;
  let part = patronus_when(&sessions, &key, |part| part.landing_needs == FactState::Fresh).await;
  assert_eq!(part.state, FactState::Fresh);
  let util = &part.view.branches["util"];
  assert_eq!(util.robot_id.as_deref(), Some("robot-1"));
  let LandingNeeds::Known { refusals } = &part.view.branches["flow"].landing_needs else {
    panic!("{part:?}")
  };
  let [LandingRefusal::MissingDeclaration { landing_in, .. }] = &refusals[..] else {
    panic!("{refusals:?}")
  };
  assert_eq!(landing_in.as_ref().map(|robot| robot.robot_id.as_str()), Some("robot-1"));
  assert_eq!(part.view.branches["util"].landing_needs, LandingNeeds::Known { refusals: Vec::new() });
  assert_eq!(facts.computed.lock().unwrap().len(), 1);

  list_robots(&sessions, &key, vec![running]).await;
  assert_eq!(snapshot(&sessions, &key).await.patronus, part, "the same robots derive nothing again");
  assert_eq!(facts.computed.lock().unwrap().len(), 1, "nor compute the needs again");
}

/// A view the host can't derive is `Unavailable` with why, never an empty view; the next answer of Patronus derives it
/// again, also of the same robots
#[tokio::test]
async fn a_view_that_failed_is_derived_again_after_the_next_answer_of_patronus() {
  let facts = Arc::new(HostFacts::default());
  let (_repo, sessions, key) = util_and_flow(&facts).await;
  *facts.fails_view.lock().unwrap() = true;
  list_robots(&sessions, &key, Vec::new()).await;
  let failed = patronus_when(&sessions, &key, |part| part.state != FactState::Loading).await;
  assert_eq!(
    failed.state,
    FactState::Unavailable {
      reason: ApiError::internal("the test fails the view")
    }
  );
  assert_eq!(failed.view, PatronusView::default());

  *facts.fails_view.lock().unwrap() = false;
  list_robots(&sessions, &key, Vec::new()).await;
  // without robots the view is Fresh and empty before its landing needs come: each branch has them after
  let part = patronus_when(&sessions, &key, |part| part.landing_needs == FactState::Fresh).await;
  assert_eq!(part.state, FactState::Fresh);
  assert!(part.view.branches.contains_key("util"), "{part:?}");
}
