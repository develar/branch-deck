use crate::branch_table::BranchTable;
use crate::facts::{FactResult, RobotsDigest, RobotsRead};
use crate::runtime::{GitSessionHost, PATCH_INTERVAL, RunSetup, SessionEnvironment, sleep_until};
use crate::session::SyncTicket;
use crate::session::{IDLE_LIMIT, RunReporter, SessionEvent, SessionEventKind, SessionHost, SessionKey, Sessions, Subscriber, SubscriberGone, SyncRun, SyncedTable};
use crate::table::RemoteSubject;
use crate::table::table_test::{branch_status, grouped, grouping, remote_at, synced, unapplied};
use crate::views::RepositoryViews;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::robot;
use domain_types::patronus::{PatronusAvailability, PatronusRobotStatus, PatronusRobotsResult};
use domain_types::session::{BranchStatus, Demand, SyncOutcome, SyncReason, SyncState, SyncStep, TableView, ViewPatch};
use domain_types::stack::BlockReason;
use domain_types::{BranchSyncStatus, CommitSyncStatus, ProgressReporter, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_core::push::{PushBranchesOutcome, StackPushBranch};
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
}

impl ScriptedHost {
  fn push(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>) {
    self.push_gated(events, result, None);
  }

  fn push_gated(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>, gate: Option<Arc<Notify>>) {
    let pace = Duration::ZERO;
    self.scripts.lock().unwrap().push_back(Script { events, pace, result, gate });
  }

  /// A run whose events stream over time: `pace` after each
  fn push_paced(&self, events: Vec<SyncEvent>, result: Result<SyncReport, ApiError>, pace: Duration) {
    self.scripts.lock().unwrap().push_back(Script { events, pace, result, gate: None });
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
      script.result.map(|report| SyncedTable {
        report,
        table: BranchTable::default(),
      })
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

/// The environment of the app's sessions in a test: the default options, a deferred sync with its code, whether a
/// focus syncs (Auto-sync on Focus), and the prefix the git config names (`None`: the session's own)
struct TestEnvironment {
  syncs_on_focus: bool,
  config_prefix: Option<String>,
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
}

/// Sessions that sync with `git`, on a clock the test moves; a focus syncs
fn with_git(git: GitCommandExecutor) -> (Sessions, TestClock) {
  with_git_views(git, RepositoryViews::default())
}

/// `with_git` whose syncs keep their tables in `views`
fn with_git_views(git: GitCommandExecutor, views: RepositoryViews) -> (Sessions, TestClock) {
  with_environment(
    git,
    views,
    TestEnvironment {
      syncs_on_focus: true,
      config_prefix: None,
    },
  )
}

/// Sessions that sync with `git` in `environment`, their tables kept in `views`
fn with_environment(git: GitCommandExecutor, views: RepositoryViews, environment: TestEnvironment) -> (Sessions, TestClock) {
  let clock = TestClock::new();
  let host = Arc::new(GitSessionHost::new(git, views, environment));
  (Sessions::new(tokio::runtime::Handle::current(), host, Arc::new(clock.clone())), clock)
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

  let (_, reopened) = sessions.subscribe(&key, Box::new(Window::default())).await.unwrap();
  assert_ne!(reopened.version.epoch, epoch, "a new epoch: a window of the old session asks for a snapshot");
  assert_eq!(reopened.version.seq, 0);
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
    let git = GitCommandExecutor::new().with_process_stats("sessions");
    let environment = TestEnvironment {
      syncs_on_focus,
      config_prefix: None,
    };
    let (sessions, _clock) = with_environment(git.clone(), RepositoryViews::default(), environment);
    let window = Window::default();
    sessions.subscribe(&key, Box::new(window.clone())).await.unwrap();
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

/// What a push of the virtual branch `branch` sends, as its parameters name it
fn pushing(branch: &str) -> Vec<RemoteSubject> {
  let branch = StackPushBranch {
    branch_name: branch.to_string(),
    parent_branch: None,
    expected_remote: None,
    tip: None,
    whole_stack: false,
    unapplied: false,
  };
  vec![RemoteSubject::of(&branch)]
}

fn pushed(branch: &str, tip: &str) -> PushBranchesOutcome {
  PushBranchesOutcome::Pushed {
    remote_statuses: vec![remote_at(branch, tip)],
  }
}

/// The remote tip `branch` shows in the view
fn remote_tip(view: &TableView, branch: &str) -> Option<String> {
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
/// the version of that patch; nothing syncs. A failed push changes nothing, and so does one whose future was dropped,
/// after which a sync shows what it computes again
#[tokio::test]
async fn a_push_without_a_sync_patches_the_remote_status_it_left() {
  let host = Arc::new(ScriptedHost::default());
  let (sessions, _clock) = scripted(&host);
  let (key, window) = synced_window(&sessions, &host, "/repo", auth_built()).await;

  let answer = sessions.push(&key, pushing("auth"), async { Ok(pushed("auth", "pushed")) }).await.unwrap();
  let patches = window.take();
  assert_eq!(patches.len(), 1, "{patches:?}");
  let change = &patches[0].changes.branches["auth"];
  assert_eq!((change.head.is_none(), change.sync.is_none()), (true, true), "only the remote status: {change:?}");
  assert_eq!(answer.version, Some(patches[0].next));
  assert_eq!(remote_tip(&snapshot(&sessions, &key).await, "auth").as_deref(), Some("pushed"));
  assert_eq!(host.runs(), 1, "no sync after the push");

  sessions.push(&key, pushing("auth"), async { Err(anyhow::anyhow!("rejected")) }).await.unwrap_err();
  let dropped = sessions.push(&key, pushing("auth"), std::future::pending());
  tokio::time::timeout(Duration::from_millis(20), dropped).await.unwrap_err();
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
  let push = async {
    repo.git(&["push", "-q", "origin", "test/virtual/auth"]);
    Ok(pushed("auth", &tip))
  };
  sessions.push(&key, pushing("auth"), push).await.unwrap();
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

  sessions.push(&pushed_in, pushing("auth"), async { Ok(pushed("auth", "pushed")) }).await.unwrap();
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
  };
  let (sessions, _clock) = with_environment(GitCommandExecutor::new(), RepositoryViews::default(), environment);
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
    review_windows: names(&["auth"]),
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
    review_windows: names(&["auth"]),
    ..Demand::default()
  };
  assert_eq!(sessions.demand(&repo).await, Some(both));

  sessions.set_demand(main, &Demand::default());
  assert_eq!(sessions.demand(&repo).await, Some(reviewed), "the main window said it shows nothing now");
  sessions.unsubscribe(review);
  assert_eq!(sessions.demand(&repo).await, Some(Demand::default()), "gone with the subscription");
  assert_eq!(sessions.demand(&other).await, Some(details));
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
      SessionEventKind::TableCommitted => "table committed".to_owned(),
      SessionEventKind::Robots(RobotsDigest { active, .. }) => format!("robots {:?}", active.iter().map(|robot| robot.id.as_str()).collect::<Vec<_>>()),
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
  let version = sessions.apply_facts(&key, ticket, FactResult::Robots { read, result: listed.clone() }).await.unwrap();
  assert_eq!(heard(&mut events), vec![r#"robots ["auth-robot"]"#]);
  let patch = window.take().pop().unwrap();
  assert_eq!(patch.next, version);
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
