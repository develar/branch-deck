use crate::events::NoEvents;
use crate::land_execute::Accepted;
use crate::push::{PushHost, execute_push, prepare_push};
use crate::registry::Registry;
use crate::registry::registry_test::{operations_until, view_operations};
use domain_types::PushAction;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Confirmation, PlannedEffect, Subject};
use domain_types::operation::{EffectStatus, NewsKey, OperationResult, OperationStep, Outcome, Plan, Push, PushScope, RunningOperation};
use domain_types::session::SyncReason;
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use history_rewrite::repository_lock::{LockWaitCanceled, RepositoryLock, RewriteOptions, lock_repository_in};
use history_rewrite::squash::{SquashCommitsParams, squash_commits_core};
use object_cache::cache::ObjectCache;
use patronus::fake_patronus::start_fake;
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use pretty_assertions::assert_eq;
use remote_facts::fact_sources::Read;
use remote_facts::service_sources::ServiceHost;
use repository_model::facts::Needs;
use repository_model::moved_remote::MovedRemoteAsk;
use repository_model::runtime::{GitSessionHost, RunSetup, SessionEnvironment};
use repository_model::session::{OneClock, SessionKey, Sessions};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::SpaceRepository;
use sync_core::push::{NOT_SYNCED_YET, PushBranchesOutcome, PushBranchesParams, StackPushBranch};
use sync_core::sync::SyncOptions;
use sync_core::transfer_progress::TransferProgress;
use sync_utils::clock::TestClock;
use test_utils::git_test_utils::{TestRepo, stack_ahead_of_bare_origin};

/// The prefix of the branches of `stack_ahead_of_bare_origin`, which the host resolved for the push
const PREFIX: &str = "test";

/// The syncs of the sessions of the tests: the options of a sync of the app, under the prefix of the key
struct TestEnvironment;

impl SessionEnvironment for TestEnvironment {
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    Ok(RunSetup {
      options: SyncOptions::default(),
      prefix: key.prefix.clone(),
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    ApiError::internal(format!("{error:#}"))
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    false
  }

  fn landing_needs(&self, _key: &SessionKey, _landed: &HashSet<String>) -> anyhow::Result<Needs> {
    Ok(Needs::new())
  }
}

/// `stack_ahead_of_bare_origin` with stacks on (`alpha`, and `beta` built on it) and the registry of the sessions of its
/// key, its session synced unless `synced` is false
struct Fixture {
  repo: TestRepo,
  registry: Registry,
  key: SessionKey,
}

async fn fixture(synced: bool) -> Fixture {
  let repo = stack_ahead_of_bare_origin();
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let host = GitSessionHost::new(GitCommandExecutor::new(), TestEnvironment);
  let sessions = Arc::new(Sessions::new(
    tokio::runtime::Handle::current(),
    Arc::new(host),
    Arc::new(OneClock(Arc::new(TestClock::new()))),
  ));
  let key = SessionKey {
    worktree: repo.path_str().to_string(),
    prefix: PREFIX.to_string(),
  };
  if synced {
    sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  }
  Fixture {
    registry: Registry::new(sessions),
    repo,
    key,
  }
}

impl Fixture {
  /// The push host of the app on the repository, its git `git`, no Patronus; a lock no record names refuses after
  /// `unknown_holder_timeout`
  fn host(&self, git: &GitCommandExecutor, unknown_holder_timeout: Duration) -> PushHost {
    PushHost {
      git: git.clone(),
      git_common_dir: self.repo.path().join(".git"),
      registry: self.registry.clone(),
      key: self.key.clone(),
      patronus: None,
      events: Arc::new(NoEvents),
      unknown_holder_timeout,
    }
  }

  /// The push of `branches` of the repository, parents first
  fn push_of(&self, branches: Vec<StackPushBranch>) -> PushBranchesParams {
    PushBranchesParams {
      repository_path: self.repo.path_str().to_string(),
      my_email: None,
      baseline_branch: "origin/master".to_string(),
      branches,
    }
  }

  /// The push of `branches` as the app's command runs it, waiting a minute at most for a holder no record names
  async fn push(&self, branches: Vec<StackPushBranch>) -> anyhow::Result<PushBranchesOutcome> {
    let git = GitCommandExecutor::new();
    let pushed = self.host(&git, Duration::from_mins(1)).push_branches(self.push_of(branches), |_| {}, || false).await;
    pushed.map(|answer| answer.outcome)
  }

  /// The branches of the bare origin under the prefix
  fn remote_branches(&self) -> String {
    self.repo.git(&["--git-dir", ".git/origin.git", "for-each-ref", "--format=%(refname)", "refs/heads/test/"])
  }

  /// The commit of `branch` on the bare origin
  fn remote_tip(&self, branch: &str) -> String {
    self.repo.git(&["--git-dir", ".git/origin.git", "rev-parse", &format!("refs/heads/test/virtual/{branch}")])
  }

  /// The repository lock, held by the test as another holder would
  fn lock(&self) -> RepositoryLock {
    lock_repository_in(&self.repo.path().join(".git"), Duration::ZERO, &|| {}).unwrap()
  }

  /// The operations of the view of the session now
  async fn operations(&self) -> Vec<RunningOperation> {
    view_operations(self.registry.sessions(), &self.key).await
  }

  /// The operations of the view once `done` holds of them; the other tasks run meanwhile
  async fn until(&self, done: impl Fn(&[RunningOperation]) -> bool) -> Vec<RunningOperation> {
    operations_until(self.registry.sessions(), &self.key, done).await
  }

  /// The operations of the view once one of them waits in the queue
  async fn until_queued(&self) -> Vec<RunningOperation> {
    self
      .until(|operations| operations.iter().any(|operation| matches!(operation.step, OperationStep::Queued { .. })))
      .await
  }
}

fn branch(name: &str) -> StackPushBranch {
  StackPushBranch {
    branch_name: name.to_string(),
    parent_branch: None,
    expected_remote: None,
    tip: None,
    whole_stack: false,
    unapplied: false,
  }
}

/// `beta`, built on `alpha`
fn beta() -> StackPushBranch {
  StackPushBranch {
    parent_branch: Some("alpha".to_string()),
    ..branch("beta")
  }
}

fn assert_pushed(pushed: &anyhow::Result<PushBranchesOutcome>, refs: usize) {
  assert!(
    matches!(pushed, Ok(PushBranchesOutcome::Pushed { remote_statuses, .. }) if remote_statuses.len() == refs),
    "{pushed:?}"
  );
}

/// The app's push of a stack runs the git processes of the core push (`an_atomic_push_of_a_stack`) and nothing more: the
/// lock is a file of the known common dir, the checks read the table of the session's last sync, no robot is asked off
/// Space; before the first sync it is refused. Once it ended, its operation left the view
#[tokio::test(flavor = "multi_thread")]
async fn a_push_runs_the_processes_of_the_core_push() {
  let unsynced = fixture(false).await;
  let refused = unsynced.push(vec![branch("alpha"), beta()]).await.unwrap_err();
  assert_eq!(refused.to_string(), NOT_SYNCED_YET);

  let fixture = fixture(true).await;
  let git = GitCommandExecutor::new().with_process_stats("test");
  let pushed = fixture
    .host(&git, Duration::from_mins(1))
    .push_branches(fixture.push_of(vec![branch("alpha"), beta()]), |_| {}, || false)
    .await;
  let answer = pushed.unwrap();
  assert!(answer.version.is_some(), "the session has the outcome");
  assert_pushed(&Ok(answer.outcome), 2);
  assert_eq!(
    git.process_stats().unwrap().counts(),
    [
      ("cat-file".to_string(), 1),
      ("config".to_string(), 1),
      ("for-each-ref".to_string(), 2),
      ("log".to_string(), 1),
      ("push".to_string(), 1),
      ("reflog".to_string(), 1)
    ]
  );
  assert_eq!(fixture.operations().await, Vec::new());
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// Another holder of the repository lock (`bd pick --follow` of an agent, a rewrite of the app) keeps the push waiting,
/// without a bound, as long as it lives: the progress of the push names it ("Waiting for `…` (pid …, for 0 s)"), the view
/// shows it waiting for the lock, and once the holder releases the lock, the push goes. Here the holder is the test
/// itself, alive; the push is told it again past the bound for a holder no record names, which a wait that counted it
/// would have refused at. The bound is not zero: the release deletes the record a moment before it frees the lock, and a
/// zero bound would refuse in that moment
#[tokio::test(flavor = "multi_thread")]
async fn a_push_waits_while_the_holder_of_the_lock_lives_and_names_it() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  let git = GitCommandExecutor::new();
  let (progress, mut told) = tokio::sync::mpsc::unbounded_channel::<TransferProgress>();
  let host = fixture.host(&git, Duration::from_millis(500));
  let push = host.push_branches(fixture.push_of(vec![branch("alpha"), beta()]), move |tick| drop(progress.send(tick)), || false);
  let holder = async {
    let waiting = told.recv().await.unwrap();
    assert!(waiting.waiting, "{waiting:?}");
    assert!(waiting.line.starts_with("Waiting for `"), "{waiting:?}");
    assert!(waiting.line.contains(&format!("(pid {}, for ", std::process::id())), "{waiting:?}");
    let [operation] = fixture
      .until(|operations| operations.first().is_some_and(|operation| operation.step == OperationStep::WaitingForLock))
      .await
      .try_into()
      .unwrap();
    assert_eq!(operation.step, OperationStep::WaitingForLock);
    // told again about a second later: past the bound for a holder no record names, the named one keeps it waiting
    let started = std::time::Instant::now();
    assert!(told.recv().await.unwrap().waiting);
    assert!(started.elapsed() >= Duration::from_millis(500), "told again after {:?}", started.elapsed());
    assert_eq!(fixture.remote_branches(), "");
    drop(held);
  };
  let (pushed, ()) = tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(push, holder) }).await.unwrap();
  assert_pushed(&pushed.map(|answer| answer.outcome), 2);
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// Stop Waiting (`canceled`, which the command's wait ticket answers) ends the wait of a push for another holder of the
/// lock with `LockWaitCanceled`: nothing pushed, the lock still the holder's
#[tokio::test(flavor = "multi_thread")]
async fn a_push_whose_wait_is_stopped_pushes_nothing() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  let git = GitCommandExecutor::new();
  let stopped = fixture
    .host(&git, Duration::from_mins(1))
    .push_branches(fixture.push_of(vec![branch("alpha")]), |_| {}, || true)
    .await
    .unwrap_err();
  assert!(stopped.downcast_ref::<LockWaitCanceled>().is_some(), "{stopped:#}");
  assert_eq!(fixture.remote_branches(), "");
  drop(held);
}

/// A lock no record names a live holder of (a holder that writes none, an older Branch Deck) refuses the push after the
/// bounded wait, and nothing is pushed
#[tokio::test(flavor = "multi_thread")]
async fn a_push_while_a_holder_no_record_names_holds_the_lock_is_refused() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  std::fs::remove_file(fixture.repo.path().join(".git/branch-deck.lock.holder")).unwrap();
  let git = GitCommandExecutor::new();
  let refused = fixture
    .host(&git, Duration::from_millis(100))
    .push_branches(fixture.push_of(vec![branch("alpha")]), |_| {}, || false)
    .await
    .unwrap_err();
  assert_eq!(
    refused.to_string(),
    "Nothing was pushed: another Branch Deck operation holds the repository; try again when it is done"
  );
  drop(held);
  assert_eq!(fixture.remote_branches(), "");
}

/// Pushes again over the new remote tip when the remote has no commits the branch lacks: nothing is lost. The remote
/// branch of `alpha` was deleted since the row showed it: the lease on the head the user saw is refused, the push fetches
/// it and goes again, once, the same commit it froze under the lock, leased on what the remote has now (no lock, no
/// checks again), and says what it went over (`pushed_over`). Someone's commit on the remote instead is told, never
/// pushed over (`over_moved_remote`)
#[tokio::test(flavor = "multi_thread")]
async fn pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks() {
  let fixture = fixture(true).await;
  let first = fixture.push(vec![branch("alpha")]).await.unwrap();
  let PushBranchesOutcome::Pushed { remote_statuses, .. } = first else {
    panic!("{first:?}");
  };
  let seen = remote_statuses[0].remote_tip.clone();
  let alpha = "refs/heads/test/virtual/alpha";
  fixture.repo.git(&["--git-dir", ".git/origin.git", "update-ref", "-d", alpha]);
  let local_tip = fixture.repo.git(&["rev-parse", alpha]);
  let over_seen = vec![StackPushBranch {
    expected_remote: seen,
    ..branch("alpha")
  }];

  let counted = GitCommandExecutor::new().with_process_stats("test");
  let outcome = fixture
    .host(&counted, Duration::from_mins(1))
    .push_branches(fixture.push_of(over_seen), |_| {}, || false)
    .await
    .unwrap()
    .outcome;
  // the refused push (its tips, the size check of what it adds, the push, the fetch of the branches and the status of
  // what the remote has now), then the push again (its size check, the push, the status of what it left): two pushes,
  // never a third
  assert_eq!(
    counted.process_stats().unwrap().counts(),
    [
      ("cat-file".to_string(), 1),
      ("config".to_string(), 1),
      ("fetch".to_string(), 1),
      ("for-each-ref".to_string(), 3),
      ("log".to_string(), 2),
      ("ls-remote".to_string(), 1),
      ("push".to_string(), 2),
      ("reflog".to_string(), 1),
      ("rev-list".to_string(), 1),
      ("update-ref".to_string(), 1)
    ]
  );
  let PushBranchesOutcome::Pushed { remote_statuses, pushed_over } = outcome else {
    panic!("{outcome:?}");
  };
  assert_eq!(
    pushed_over.iter().map(|status| (status.remote_exists, status.action)).collect::<Vec<_>>(),
    [(false, PushAction::Create)],
    "the push went over the deleted remote branch"
  );
  assert_eq!(remote_statuses[0].remote_tip.as_deref(), Some(local_tip.as_str()));
  assert_eq!(fixture.remote_tip("alpha"), local_tip);

  // someone's commit on the remote branch: the push is told what the remote has, and pushes nothing over it
  let tree = fixture.repo.git(&["rev-parse", &format!("{alpha}^{{tree}}")]);
  let theirs = fixture
    .repo
    .git(&["--git-dir", ".git/origin.git", "commit-tree", &tree, "-p", &local_tip, "-m", "Someone else"]);
  fixture.repo.git(&["--git-dir", ".git/origin.git", "update-ref", alpha, &theirs]);
  let over_pushed = vec![StackPushBranch {
    expected_remote: Some(local_tip),
    ..branch("alpha")
  }];
  let outcome = fixture.push(over_pushed).await.unwrap();
  assert!(
    matches!(&outcome, PushBranchesOutcome::RemoteMoved { remote_statuses, .. } if remote_statuses[0].remote_tip.as_deref() == Some(theirs.as_str())),
    "{outcome:?}"
  );
  assert_eq!(fixture.remote_tip("alpha"), theirs, "nothing was pushed over it");
}

/// The push of `branches`, held in its first progress (the network push, after the lock) until the sender it answers
/// sends or drops; the receiver it answers hears `pushes` once it is held
fn held_push(
  fixture: &Fixture,
  branches: Vec<StackPushBranch>,
) -> (
  impl Future<Output = anyhow::Result<PushBranchesOutcome>>,
  tokio::sync::mpsc::UnboundedReceiver<&'static str>,
  mpsc::Sender<()>,
) {
  let (events, heard) = tokio::sync::mpsc::unbounded_channel();
  let (go, gone) = mpsc::channel::<()>();
  let mut held = Some(gone);
  let on_progress = move |_| {
    if let Some(gone) = held.take() {
      events.send("pushes").unwrap();
      // an error is a dropped sender (a failed step of the test), which ends the hold as well
      let _released = gone.recv_timeout(Duration::from_mins(1));
    }
  };
  let pushed = async move {
    let git = GitCommandExecutor::new();
    let pushed = fixture
      .host(&git, Duration::from_mins(1))
      .push_branches(fixture.push_of(branches), on_progress, || false)
      .await;
    pushed.map(|answer| answer.outcome)
  };
  (pushed, heard, go)
}

/// Two pushes of the app of different branches run side by side: the registry admits each per ref it writes and the
/// lock is held only while a push is decided, so while the first one pushes (held in its progress) the lock is free, and
/// the second one, which waits for no holder at all (`Duration::ZERO`), decides, pushes and ends before the first does
#[tokio::test(flavor = "multi_thread")]
async fn the_pushes_of_two_branches_run_side_by_side() {
  let fixture = fixture(true).await;
  let (first, mut event, go) = held_push(&fixture, vec![branch("alpha")]);
  let second = async {
    assert_eq!(event.recv().await, Some("pushes"));
    drop(fixture.lock());
    let git = GitCommandExecutor::new();
    let second = fixture.host(&git, Duration::ZERO).push_branches(fixture.push_of(vec![beta()]), |_| {}, || false).await;
    assert_pushed(&second.map(|answer| answer.outcome), 1);
    go.send(()).unwrap();
  };
  let (first, ()) = tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(first, second) }).await.unwrap();
  assert_pushed(&first, 1);
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// A push of a ref another push of the process writes (Push Stack while the push of its root runs, two whole stacks of
/// one stack branch) waits for it in the registry (`Queued` behind it), without a bound, as both lease on the head the
/// user saw, and then finds the remote at the commit the first sent instead of a lease the first broke. A second click
/// of the same push joins it, and then pushes what the remote still lacks: nothing. All of them push
#[tokio::test(flavor = "multi_thread")]
async fn the_pushes_of_one_ref_wait_for_each_other_and_both_push() {
  let fixture = fixture(true).await;
  let (first, mut event, go) = held_push(&fixture, vec![branch("alpha")]);
  let stack = async {
    assert_eq!(event.recv().await, Some("pushes"));
    tokio::join!(fixture.push(vec![branch("alpha"), beta()]), fixture.push(vec![branch("alpha")]))
  };
  let release = async {
    let operations = fixture.until_queued().await;
    assert_eq!(operations.len(), 2, "the push of the stack waits, the second click joined: {operations:?}");
    go.send(()).unwrap();
  };
  let (first, (stack, again), ()) = Box::pin(tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(first, stack, release) }))
    .await
    .unwrap();
  assert_pushed(&first, 1);
  assert_pushed(&stack, 2);
  assert_pushed(&again, 1);
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// A push holds no lock while it sends (C1): held in its network push (the fifo of the test: its first progress), the
/// push of another branch, a squash of the user's history (a rewrite under the lock with `RewriteOptions::default()`,
/// which a held lock would refuse) and a sync of the session that rebuilds the branch all finish; released, the push
/// sends the commit it froze under the lock, not the one the sync built since
#[tokio::test(flavor = "multi_thread")]
async fn a_push_that_sends_holds_no_lock_and_sends_the_commit_it_froze() {
  let fixture = fixture(true).await;
  let frozen = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let (first, mut event, go) = held_push(&fixture, vec![branch("alpha")]);
  let others = async {
    assert_eq!(event.recv().await, Some("pushes"));
    assert_pushed(&fixture.push(vec![beta()]).await, 1);
    let params = SquashCommitsParams {
      repository_path: fixture.repo.path_str().to_string(),
      commit_ids: vec![fixture.repo.rev_parse("HEAD~1").unwrap(), fixture.repo.rev_parse("HEAD").unwrap()],
      message: "(alpha) Alpha and beta".to_string(),
    };
    tokio::task::block_in_place(|| squash_commits_core(&GitCommandExecutor::new(), &params, &RewriteOptions::default())).unwrap();
    fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
    assert_ne!(fixture.repo.rev_parse("test/virtual/alpha").unwrap(), frozen, "the sync rebuilt alpha meanwhile");
    go.send(()).unwrap();
  };
  let (first, ()) = tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(first, others) }).await.unwrap();
  assert!(matches!(first, Ok(PushBranchesOutcome::Pushed { .. })), "{first:?}");
  assert_eq!(fixture.remote_tip("alpha"), frozen);
}

/// The push of an unapplied branch reads no robots (C3): no Safe Push runs on one. The push of a virtual branch reads
/// one full list, before the lock
#[tokio::test(flavor = "multi_thread")]
async fn the_push_of_an_unapplied_branch_reads_no_robots() {
  let fixture = fixture(true).await;
  let (fake, client) = start_fake().await;
  let service = PatronusService::new(client, |_| {
    TokenLookup::Found(Token {
      value: "t".to_string(),
      stamp: TokenStamp::Env,
    })
  });
  let tip = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  fixture.repo.git(&["update-ref", "refs/heads/test/unapplied/old", &tip]);
  let git = GitCommandExecutor::new();
  let host = PushHost {
    patronus: Some((Arc::new(service), "ultimate".to_string())),
    ..fixture.host(&git, Duration::from_mins(1))
  };
  let unapplied = StackPushBranch { unapplied: true, ..branch("old") };
  let pushed = host.push_branches(fixture.push_of(vec![unapplied]), |_| {}, || false).await;
  assert_pushed(&pushed.map(|answer| answer.outcome), 1);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 0);
  let pushed = host.push_branches(fixture.push_of(vec![branch("alpha")]), |_| {}, || false).await;
  assert_pushed(&pushed.map(|answer| answer.outcome), 1);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 1);
}

/// A push whose caller goes away (its future dropped, the window closed) runs to its end all the same, in a task of its
/// own: it stays in the view while its `git push` sends, so no push of its refs is admitted meanwhile, and once it ended
/// its outcome is in the session (the remote status it left) and it left the view
#[tokio::test(flavor = "multi_thread")]
async fn a_push_whose_caller_goes_away_runs_to_its_end() {
  let fixture = fixture(true).await;
  let frozen = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let (first, mut event, go) = held_push(&fixture, vec![branch("alpha")]);
  // the select drops the future of the push once it is held
  tokio::select! {
    pushed = first => panic!("the held push ended: {pushed:?}"),
    sent = event.recv() => assert_eq!(sent, Some("pushes")),
  }
  let [running] = fixture.operations().await.try_into().unwrap();
  assert_eq!(running.step, OperationStep::Pushing);
  go.send(()).unwrap();
  fixture.until(<[RunningOperation]>::is_empty).await;
  assert_eq!(fixture.remote_tip("alpha"), frozen);
  let view = fixture.registry.sessions().get_snapshot(&fixture.key).await.unwrap();
  let status = view.branches["alpha"].remote.status.as_ref().unwrap();
  assert_eq!(status.remote_tip.as_deref(), Some(frozen.as_str()), "the session has the status the push left");
}

/// A host of the engine off Space and off GitHub: what a push of the app needs of it is git
struct Offline;

// jscpd:ignore-start: the test host of the engine, as the one of `remote-facts` (`service_sources_test.rs`): a test
// module of another crate shares no code
impl ServiceHost for Offline {
  fn git(&self) -> GitCommandExecutor {
    GitCommandExecutor::new()
  }

  fn patronus(&self, _key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    Err(ApiError::new(ErrorCode::NotOnSpace, "Off Space"))
  }

  fn github(&self, _key: &SessionKey) -> Result<Arc<GitHubService>, ApiError> {
    Err(ApiError::internal("No GitHub"))
  }

  fn space_repository<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    Box::pin(async { Ok(None) })
  }

  fn github_repository<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    Box::pin(async { Ok(None) })
  }

  fn new_api(&self, _key: &SessionKey) -> bool {
    false
  }

  fn object_cache<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, ObjectCache> {
    Box::pin(std::future::ready(ObjectCache::disabled()))
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    ApiError::internal(format!("{error:#}"))
  }

  fn fetch_moved_remotes<'a>(&'a self, _key: &'a SessionKey, _asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    Box::pin(async {})
  }
}
// jscpd:ignore-end

/// The push of `name` alone over the remote head the row showed (`expect_remote`)
fn alone(name: &str, expect_remote: Option<String>) -> Push {
  Push {
    subject: Subject::branch(name),
    scope: PushScope::Alone,
    expect_remote,
    force: false,
    restart_running: false,
  }
}

impl Fixture {
  /// The plan of `push` in the session now
  async fn plan(&self, push: &Push) -> Plan {
    prepare_push(self.registry.sessions(), &self.key, push).await.unwrap().plan
  }

  /// `push` run on its plan now, `accepted` among its confirmations
  async fn execute(&self, push: &Push, accepted: Vec<Confirmation>) -> Result<Outcome, ApiError> {
    let fingerprint = self.plan(push).await.fingerprint;
    let accepted = Accepted {
      fingerprint,
      confirmations: accepted,
    };
    execute_push(&self.registry, &self.key, push.clone(), accepted, &Offline, Arc::new(NoEvents)).await
  }
}

/// The push of a branch alone is an intent of the engine: its plan pushes the branch and nothing it is built on, its run
/// sends it, tells the session the remote status it left (the version after its patch), answers the event of the
/// pushes of the branch and leaves the view; then its plan has nothing left to push
#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_a_branch_alone_is_planned_and_run_by_the_engine() {
  let fixture = fixture(true).await;
  let push = alone("beta", None);
  let plan = fixture.plan(&push).await;
  assert_eq!(
    (plan.effects.as_slice(), plan.confirmations.as_slice(), plan.blockers.as_slice()),
    (
      &[PlannedEffect::Push {
        branches: vec!["beta".to_string()],
        force: false
      }][..],
      &[][..],
      &[][..]
    )
  );

  let outcome = fixture.execute(&push, Vec::new()).await.unwrap();
  assert_eq!(
    outcome.effects.iter().map(|record| record.status.clone()).collect::<Vec<_>>(),
    [EffectStatus::Done],
    "{outcome:?}"
  );
  assert_eq!(outcome.news_key, Some(NewsKey::Push { branch: "beta".to_string() }));
  assert_eq!(outcome.result, OperationResult::Push(Box::default()));
  assert!(outcome.version_after.is_some(), "the session has the remote status it left");
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/beta", "the branch alone, not the one it is built on");
  assert_eq!(fixture.operations().await, Vec::new());
  let tip = fixture.repo.rev_parse("test/virtual/beta").unwrap();
  let pushed = alone("beta", Some(tip));
  assert_eq!(fixture.plan(&pushed).await.effects, Vec::new(), "nothing left to push");
}

/// The same push twice side by side is one push (the second joins the first); another push of the same ref waits for
/// it (`Queued` behind it, here behind the repository lock the test holds) and is decided again on what it left: the
/// remote has the branch, so it pushes nothing, and no `plan_changed` refuses it
#[tokio::test(flavor = "multi_thread")]
async fn a_push_queued_behind_a_push_of_its_ref_is_decided_again_on_what_that_one_left() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  let push = alone("alpha", None);
  let forced = Push { force: true, ..push.clone() };
  let release = async {
    let operations = fixture.until_queued().await;
    assert_eq!(operations.len(), 2, "the second click joined the first, the forced push waits: {operations:?}");
    drop(held);
  };
  let (first, again, queued, ()) = tokio::time::timeout(Duration::from_mins(1), async {
    tokio::join!(
      fixture.execute(&push, Vec::new()),
      fixture.execute(&push, Vec::new()),
      fixture.execute(&forced, Vec::new()),
      release
    )
  })
  .await
  .unwrap();
  let (first, again, queued) = (first.unwrap(), again.unwrap(), queued.unwrap());
  assert_eq!(first.op, again.op, "one push");
  assert_eq!(first.effects.len(), 1);
  assert_eq!(queued.effects, Vec::new(), "the remote had the branch already: {queued:?}");
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha");
}

/// A push planned over the remote head the row showed refuses, before anything, once the remote-tracking ref moved
/// (`plan_changed`): the witness is the remote head the user saw
#[tokio::test(flavor = "multi_thread")]
async fn a_push_over_a_remote_head_the_row_no_longer_shows_is_refused_before_anything() {
  let fixture = fixture(true).await;
  let push = alone("alpha", Some("0000000000000000000000000000000000000001".to_string()));
  let refused = fixture.execute(&push, Vec::new()).await.unwrap_err();
  assert_eq!(refused.code, ErrorCode::PlanChanged, "{refused:?}");
  assert_eq!(fixture.remote_branches(), "");
}

/// The push of an unapplied branch is an intent too: under its own name, no robot read, nothing to retarget
#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_an_unapplied_branch_is_planned_and_run_by_the_engine() {
  let fixture = fixture(false).await;
  let tip = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  fixture.repo.git(&["update-ref", "refs/heads/test/unapplied/old", &tip]);
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  let push = Push {
    scope: PushScope::Unapplied,
    ..alone("old", None)
  };
  let plan = fixture.plan(&push).await;
  assert_eq!(
    plan.effects,
    [PlannedEffect::Push {
      branches: vec!["old".to_string()],
      force: false
    }]
  );
  let outcome = fixture.execute(&push, Vec::new()).await.unwrap();
  assert_eq!(outcome.effects[0].status, EffectStatus::Done, "{outcome:?}");
  assert_eq!(fixture.remote_branches(), "refs/heads/test/unapplied/old");
}
