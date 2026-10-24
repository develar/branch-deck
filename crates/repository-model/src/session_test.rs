use crate::runtime::{GitSessionHost, SessionEnvironment};
use crate::session::SyncTicket;
use crate::session::{IDLE_LIMIT, RunReporter, SessionHost, SessionKey, Sessions, Subscriber, SubscriberGone, SyncRun};
use crate::table::table_test::{branch_status, grouped, grouping, synced};
use crate::views::RepositoryViews;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::session::{BranchStatus, SyncOutcome, SyncReason, SyncState, TableView, ViewPatch};
use domain_types::stack::BlockReason;
use domain_types::{BranchSyncStatus, CommitSyncStatus, ProgressReporter, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_core::sync::{SyncDeferred, SyncOptions, SyncReport};
use sync_utils::clock::TestClock;
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use tokio::sync::Notify;

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

/// One scripted run: its events, then its result once `gate` (if any) is notified
struct Script {
  events: Vec<SyncEvent>,
  result: Result<SyncReport, ApiError>,
  gate: Option<Arc<Notify>>,
}

/// A host whose runs play scripts in order and whose fingerprint is what the test sets
#[derive(Default)]
struct ScriptedHost {
  scripts: Mutex<VecDeque<Script>>,
  inputs: Mutex<Option<String>>,
  runs: Mutex<u32>,
  checks: Mutex<u32>,
  /// The reporter of every run, which a test may keep using after its run ended
  reporters: Mutex<Vec<RunReporter>>,
}

impl ScriptedHost {
  fn push(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>) {
    self.push_gated(events, result, None);
  }

  fn push_gated(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>, gate: Option<Arc<Notify>>) {
    self.scripts.lock().unwrap().push_back(Script { events, result, gate });
  }

  fn set_inputs(&self, inputs: &str) {
    *self.inputs.lock().unwrap() = Some(inputs.to_string());
  }

  fn runs(&self) -> u32 {
    *self.runs.lock().unwrap()
  }
}

impl SessionHost for ScriptedHost {
  fn inputs(&self, _key: &SessionKey) -> Option<String> {
    *self.checks.lock().unwrap() += 1;
    self.inputs.lock().unwrap().clone()
  }

  fn run(&self, _key: &SessionKey, reporter: RunReporter) -> SyncRun {
    *self.runs.lock().unwrap() += 1;
    let script = self.scripts.lock().unwrap().pop_front().expect("a script for every run");
    self.reporters.lock().unwrap().push(reporter.clone());
    Box::pin(async move {
      for event in script.events {
        reporter.send(event).unwrap();
      }
      if let Some(gate) = script.gate {
        gate.notified().await;
      }
      script.result
    })
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    true
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
  let sessions = Sessions::new(tokio::runtime::Handle::current(), Arc::clone(host) as Arc<dyn SessionHost>, Arc::new(clock.clone()));
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

async fn snapshot(sessions: &Sessions, key: &SessionKey) -> TableView {
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
  assert!(sessions.apply(&key, |_| {}));
  assert_eq!(snapshot(&sessions, &key).await, synced);
}

/// Focuses during a run cost one check of the inputs after it, and no run when the run saw them
#[tokio::test]
async fn a_focus_during_a_run_is_one_check_after_it() {
  let host = Arc::new(ScriptedHost::default());
  host.set_inputs("f1");
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
/// too and says nothing failed, its ticket gets the code
#[tokio::test]
async fn a_failed_sync_keeps_the_table() {
  let host = Arc::new(ScriptedHost::default());
  host.push(auth_built(), Ok(report(false, &[])));
  host.push(vec![grouping(vec![grouped("other", &["o1"])])], Err(ApiError::internal("broken")));
  let deferred = ApiError::new(ErrorCode::OperationInProgress, SyncDeferred::OperationInProgress("rebase").to_string());
  host.push(Vec::new(), Err(deferred.clone()));
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

  assert_eq!(outcome(sessions.sync(&key, SyncReason::Manual)).await, Err(deferred));
  let deferred_view = snapshot(&sessions, &key).await;
  assert_eq!(deferred_view.sync, SyncState::Idle);
  assert_eq!(deferred_view.branches, synced.branches);
}

/// The environment of the app's sessions in a test: the default options, a deferred sync with its code
struct TestEnvironment;

impl SessionEnvironment for TestEnvironment {
  fn sync_options(&self, _key: &SessionKey) -> anyhow::Result<SyncOptions> {
    Ok(SyncOptions::default())
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    match error.downcast_ref::<SyncDeferred>() {
      Some(deferred) => ApiError::new(deferred.code(), deferred.to_string()),
      None => ApiError::internal(format!("{error:#}")),
    }
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    true
  }
}

/// Sessions that sync with `git`, on a clock the test moves
fn with_git(git: GitCommandExecutor) -> (Sessions, TestClock) {
  with_git_views(git, RepositoryViews::default())
}

/// `with_git` whose syncs keep their tables in `views`
fn with_git_views(git: GitCommandExecutor, views: RepositoryViews) -> (Sessions, TestClock) {
  let clock = TestClock::new();
  let host = Arc::new(GitSessionHost::new(git, views, TestEnvironment));
  (Sessions::new(tokio::runtime::Handle::current(), host, Arc::new(clock.clone())), clock)
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
fn branch_names(view: &TableView) -> Vec<&str> {
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

  let views = RepositoryViews::default();
  let (sessions, _clock) = with_git_views(GitCommandExecutor::new(), views.clone());
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
  // the table each sync finished answers the repository views of its worktree and prefix
  let git = GitCommandExecutor::new();
  for key in [&main, &other] {
    let facts = views.guard_facts(&git, &key.worktree, &key.prefix, Vec::new());
    assert!(facts.is_some(), "the views keep the table of {key:?}");
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

  sessions.open(&key);
  let reopened = snapshot(&sessions, &key).await;
  assert_ne!(reopened.version.epoch, epoch, "a new epoch: a window of the old session asks for a snapshot");
  assert_eq!(reopened.version.seq, 0);
}
