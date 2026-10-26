use crate::events::{EventSink, NoEvents};
use crate::land_execute::Accepted;
use crate::push::{execute_push, prepare_push};
use crate::registry::Registry;
use crate::registry::registry_test::{operations_until, view_operations};
use domain_types::PushAction;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{Confirmation, PlannedEffect, Subject};
use domain_types::operation::{EffectStatus, NewsKey, OperationEvent, OperationResult, OperationStep, Outcome, Plan, Push, PushScope, RunningOperation};
use domain_types::session::SyncReason;
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use history_rewrite::repository_lock::{RepositoryLock, RewriteOptions, lock_repository_in};
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
use sync_core::sync::SyncOptions;
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

/// A host of the engine off GitHub, on Space with the Patronus of `patronus` (the Space repository `ultimate`) or off it:
/// what a push of the app needs of it is git and the robots of its checks
struct Services {
  patronus: Option<Arc<PatronusService>>,
}

/// Off Space and off GitHub
const OFFLINE: Services = Services { patronus: None };

// jscpd:ignore-start: the test host of the engine, as the one of `remote-facts` (`service_sources_test.rs`): a test
// module of another crate shares no code
impl ServiceHost for Services {
  fn git(&self) -> GitCommandExecutor {
    GitCommandExecutor::new()
  }

  fn patronus(&self, _key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    self.patronus.clone().ok_or_else(|| ApiError::new(ErrorCode::NotOnSpace, "Off Space"))
  }

  fn github(&self, _key: &SessionKey) -> Result<Arc<GitHubService>, ApiError> {
    Err(ApiError::internal("No GitHub"))
  }

  fn space_repository<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    let space = self.patronus.as_ref().map(|_| SpaceRepository {
      project_key: "ij".to_string(),
      name: "ultimate".to_string(),
    });
    Box::pin(async { Ok(space) })
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

  /// `push` run on its plan now, `accepted` among its confirmations, off Space
  async fn execute(&self, push: &Push, accepted: Vec<Confirmation>) -> Result<Outcome, ApiError> {
    self.execute_on(push, accepted, &OFFLINE, Arc::new(NoEvents)).await
  }

  /// `push` run on its plan now over `services`, its events to `events`
  async fn execute_on(&self, push: &Push, accepted: Vec<Confirmation>, services: &dyn ServiceHost, events: Arc<dyn EventSink>) -> Result<Outcome, ApiError> {
    let fingerprint = self.plan(push).await.fingerprint;
    let accepted = Accepted {
      fingerprint,
      confirmations: accepted,
    };
    execute_push(&self.registry, &self.key, push.clone(), accepted, services, events).await
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

/// A push leases on the remote head the row showed, also when the remote-tracking ref says another one (a push from a
/// terminal, a fetch the session has not synced yet): no witness refuses it before the push, the remote refuses the
/// lease, the push fetches what it has and goes again over it, once, as nothing of it is lost
#[tokio::test(flavor = "multi_thread")]
async fn a_push_over_a_remote_head_the_row_no_longer_shows_leases_on_it_and_goes_again_over_the_remote() {
  let fixture = fixture(true).await;
  let push = alone("alpha", Some("0000000000000000000000000000000000000001".to_string()));
  let outcome = fixture.execute(&push, Vec::new()).await;
  assert_done(&outcome);
  let OperationResult::Push(result) = outcome.unwrap().result else {
    panic!("no push result")
  };
  assert_eq!(result.pushed_over.len(), 1, "it went again over the remote it found");
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha");
}

/// The fixture with the unapplied branch `old` at the tip of alpha, synced
async fn with_unapplied_old() -> Fixture {
  let fixture = fixture(false).await;
  let tip = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  fixture.repo.git(&["update-ref", "refs/heads/test/unapplied/old", &tip]);
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  fixture
}

/// The push of an unapplied branch is an intent too: under its own name, no robot read, nothing to retarget
#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_an_unapplied_branch_is_planned_and_run_by_the_engine() {
  let fixture = with_unapplied_old().await;
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

/// The push of `name` with `scope`
fn scoped(name: &str, scope: PushScope) -> Push {
  Push { scope, ..alone(name, None) }
}

/// The push effect of a plan: the branches it sends, root first, and whether it forces
fn pushed_by(plan: &Plan) -> Option<(Vec<String>, bool)> {
  plan.effects.iter().find_map(|effect| match effect {
    PlannedEffect::Push { branches, force } => Some((branches.clone(), *force)),
    _ => None,
  })
}

fn names(names: &[&str]) -> Vec<String> {
  names.iter().map(ToString::to_string).collect()
}

impl Fixture {
  /// A commit of someone else on the remote branch of `branch`, fetched, and the session synced on it: the push of the
  /// branch drops it
  async fn someone_else_on(&self, branch: &str) {
    let full = format!("refs/heads/test/virtual/{branch}");
    let remote = self.remote_tip(branch);
    let tree = self.repo.git(&["rev-parse", &format!("{full}^{{tree}}")]);
    let theirs = self.repo.git(&["--git-dir", ".git/origin.git", "commit-tree", &tree, "-p", &remote, "-m", "Someone else"]);
    self.repo.git(&["--git-dir", ".git/origin.git", "update-ref", &full, &theirs]);
    self.repo.git(&["fetch", "-q", "origin"]);
    self.registry.sessions().sync(&self.key, SyncReason::Manual).wait().await.unwrap();
  }
}

/// Push Stack of a stacked branch, the push of its button, is one atomic push of the branches it is built on that have
/// work, root first, then the branch: it tells the session what it left and the event of the pushes of the branch, and
/// then has nothing left to push; Push with Children of the root has nothing left either
#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_a_stack_is_one_push_of_the_branches_it_covers() {
  let fixture = fixture(true).await;
  let push = scoped("beta", PushScope::WithAncestors);
  let plan = fixture.plan(&push).await;
  assert_eq!((pushed_by(&plan), plan.confirmations.as_slice()), (Some((names(&["alpha", "beta"]), false)), &[][..]));
  let outcome = fixture.execute(&push, Vec::new()).await.unwrap();
  assert_eq!((outcome.effects.len(), &outcome.effects[0].status), (1, &EffectStatus::Done), "{outcome:?}");
  assert_eq!(outcome.news_key, Some(NewsKey::Push { branch: "beta".to_string() }));
  assert!(outcome.version_after.is_some(), "the session has the remote statuses it left");
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
  assert_eq!(fixture.plan(&push).await.effects, Vec::new(), "nothing left to push");
  assert_eq!(fixture.plan(&scoped("alpha", PushScope::WithChildren)).await.effects, Vec::new());
}

/// An ancestor whose push drops remote commits is left to its own row by the push button, which pushes above it; Push
/// Stack, forced (its description named every drop), pushes it too and asks nothing; the branch itself asks first
/// (`ForcePushDrops`), as Push with Children of it does
#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_a_stack_leaves_an_ancestor_that_asks_first_unless_forced() {
  let fixture = fixture(true).await;
  fixture.execute(&alone("alpha", None), Vec::new()).await.unwrap();
  fixture.someone_else_on("alpha").await;

  let button = fixture.plan(&scoped("beta", PushScope::WithAncestors)).await;
  assert_eq!((pushed_by(&button), button.confirmations.as_slice()), (Some((names(&["beta"]), false)), &[][..]));
  let forced = fixture
    .plan(&Push {
      force: true,
      ..scoped("beta", PushScope::WithAncestors)
    })
    .await;
  assert_eq!((pushed_by(&forced), forced.confirmations.as_slice()), (Some((names(&["alpha", "beta"]), true)), &[][..]));
  // the remote head the row of alpha shows: the push replaces it only while it is there
  let seen = fixture.repo.rev_parse("refs/remotes/origin/test/virtual/alpha").ok();
  let with_children = Push {
    expect_remote: seen,
    ..scoped("alpha", PushScope::WithChildren)
  };
  let children = fixture.plan(&with_children).await;
  assert_eq!(pushed_by(&children), Some((names(&["alpha", "beta"]), true)));
  assert_eq!(
    children.confirmations,
    [Confirmation::ForcePushDrops {
      branch: "alpha".to_string(),
      commits: 1
    }]
  );
  let outcome = fixture.execute(&with_children, children.confirmations.clone()).await.unwrap();
  assert_eq!(outcome.effects[0].status, EffectStatus::Done, "{outcome:?}");
  assert_eq!(
    fixture.remote_tip("alpha"),
    fixture.repo.rev_parse("test/virtual/alpha").unwrap(),
    "the push dropped the commit it asked about"
  );
}

/// Push Up to Here pushes the branches the commit is built on that have work, whole, then its branch up to the commit:
/// the commits after it stay unpushed. A commit no Push Up to Here is offered of (the remote has it, a sync rebuilt the
/// branch) has no plan, and a run of it pushes nothing
#[tokio::test(flavor = "multi_thread")]
async fn push_up_to_here_pushes_the_branch_up_to_the_commit_after_what_it_is_built_on() {
  let fixture = fixture(false).await;
  fixture.repo.create_commit("(alpha/beta) Beta, part 2", "beta-2.txt", "beta 2\n");
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  // the common answer, which the view sends as none: every commit the remote lacks offers it, after alpha
  let view = fixture.registry.sessions().get_snapshot(&fixture.key).await.unwrap();
  assert_eq!(view.pushes.get("beta").and_then(|push| push.up_to.clone()), None);
  let unpushed = view.branches["beta"].remote.status.as_ref().unwrap().unpushed_commits.clone();
  assert_eq!(unpushed.len(), 2);
  let first = unpushed[0].clone();
  let push = scoped("beta", PushScope::UpTo { commit: first.clone() });
  assert_eq!(pushed_by(&fixture.plan(&push).await), Some((names(&["alpha", "beta"]), false)));
  let outcome = fixture.execute(&push, Vec::new()).await.unwrap();
  assert_eq!(outcome.effects[0].status, EffectStatus::Done, "{outcome:?}");
  assert_eq!(fixture.remote_tip("beta"), first, "the branch up to the commit");
  assert_eq!(fixture.remote_tip("alpha"), fixture.repo.rev_parse("test/virtual/alpha").unwrap());
  let view = fixture.registry.sessions().get_snapshot(&fixture.key).await.unwrap();
  let left = &view.branches["beta"].remote.status.as_ref().unwrap().unpushed_commits;
  assert_eq!(left, &unpushed[1..], "the commit after it stays unpushed");

  assert!(prepare_push(fixture.registry.sessions(), &fixture.key, &push).await.is_none(), "the remote has it now");
  let accepted = Accepted {
    fingerprint: domain_types::operation::Fingerprint(String::new()),
    confirmations: Vec::new(),
  };
  let refused = execute_push(&fixture.registry, &fixture.key, push, accepted, &OFFLINE, Arc::new(NoEvents))
    .await
    .unwrap_err();
  assert_eq!(refused.code, ErrorCode::PlanChanged, "{refused:?}");
}

/// The stack branch of a whole stack that forks is pushed alone (the push a refused start of the whole stack offers),
/// and Push with Children of the root takes it along once it is on the remote; a stack that doesn't fork has none
#[tokio::test(flavor = "multi_thread")]
async fn the_stack_branch_of_a_whole_stack_is_pushed_alone_and_goes_along_once_on_the_remote() {
  let fixture = fixture(true).await;
  let whole = Push {
    subject: Subject::whole_stack("alpha"),
    ..alone("alpha", None)
  };
  let straight = fixture.plan(&whole).await;
  assert_eq!(straight.blockers, [domain_types::reason::Reason::NoStackBranch { root: "alpha".to_string() }]);

  fixture.repo.create_commit("(alpha/gamma) Gamma", "gamma.txt", "gamma\n");
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  let plan = fixture.plan(&whole).await;
  assert_eq!((pushed_by(&plan), plan.blockers.as_slice()), (Some((names(&["stack:alpha"]), false)), &[][..]));
  let outcome = fixture.execute(&whole, Vec::new()).await.unwrap();
  assert_eq!(outcome.effects[0].status, EffectStatus::Done, "{outcome:?}");
  assert_eq!(
    outcome.news_key,
    Some(NewsKey::Push {
      branch: "stack:alpha".to_string()
    })
  );
  assert_eq!(fixture.remote_branches(), "refs/heads/test/stack/alpha");

  // a new commit of a branch of it: Push with Children of the root sends the branches and the stack branch, last
  fixture.repo.create_commit("(alpha/gamma) Gamma, part 2", "gamma-2.txt", "gamma 2\n");
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  let children = scoped("alpha", PushScope::WithChildren);
  let plan = fixture.plan(&children).await;
  assert_eq!(pushed_by(&plan), Some((names(&["alpha", "gamma", "beta", "stack:alpha"]), false)));
  let outcome = fixture.execute(&children, Vec::new()).await.unwrap();
  assert_eq!(outcome.effects[0].status, EffectStatus::Done, "{outcome:?}");
  assert_eq!(fixture.plan(&whole).await.effects, Vec::new(), "the stack branch went along");
}

/// Where a held push tells it holds (`held`): its first progress, the network push after the lock, waits until the
/// sender of `released` sends or drops
struct HeldPush {
  released: std::sync::Mutex<Option<mpsc::Receiver<()>>>,
  holds: tokio::sync::mpsc::UnboundedSender<&'static str>,
}

impl EventSink for HeldPush {
  fn send(&self, event: OperationEvent) {
    if !matches!(event, OperationEvent::Progress { .. }) {
      return;
    }
    let released = self.released.lock().unwrap().take();
    if let Some(released) = released {
      self.holds.send("pushes").unwrap();
      // an error is a dropped sender (a failed step of the test), which ends the hold as well
      let _released = released.recv_timeout(Duration::from_mins(1));
    }
  }
}

impl Fixture {
  /// `push` held in its network push, after the lock, until the sender it answers sends or drops; the receiver hears
  /// "pushes" once it holds
  fn held(
    &self,
    push: Push,
  ) -> (
    impl Future<Output = Result<Outcome, ApiError>>,
    tokio::sync::mpsc::UnboundedReceiver<&'static str>,
    mpsc::Sender<()>,
  ) {
    let (holds, heard) = tokio::sync::mpsc::unbounded_channel();
    let (release, released) = mpsc::channel::<()>();
    let sink = Arc::new(HeldPush {
      released: std::sync::Mutex::new(Some(released)),
      holds,
    });
    (async move { self.execute_on(&push, Vec::new(), &OFFLINE, sink).await }, heard, release)
  }
}

/// The outcome pushed everything it planned
fn assert_done(outcome: &Result<Outcome, ApiError>) {
  let statuses: Option<Vec<&EffectStatus>> = outcome.as_ref().ok().map(|outcome| outcome.effects.iter().map(|record| &record.status).collect());
  assert_eq!(statuses, Some(vec![&EffectStatus::Done]), "{outcome:?}");
}

/// The push of a stack runs the git processes of a run of the engine and of the core push, and nothing more: the
/// common dir of the lock (`rev-parse`) and the size limit (`config`) of the start, the cheap witness and the freeze
/// under the lock (`for-each-ref` of the tips and the leases), the size check (`log`, `cat-file`), one `push`, the
/// remote statuses after it (`for-each-ref`, `reflog`); no robot is asked off Space. Before the first sync it has no
/// plan
#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_a_stack_runs_the_processes_of_the_core_push() {
  let unsynced = fixture(false).await;
  assert!(
    prepare_push(unsynced.registry.sessions(), &unsynced.key, &scoped("beta", PushScope::WithAncestors))
      .await
      .is_none()
  );

  let fixture = fixture(true).await;
  let git = GitCommandExecutor::new().with_process_stats("test");
  let services = Counted { git: git.clone() };
  let outcome = fixture
    .execute_on(&scoped("beta", PushScope::WithAncestors), Vec::new(), &services, Arc::new(NoEvents))
    .await;
  assert_done(&outcome);
  assert_eq!(
    git.process_stats().unwrap().counts(),
    PROCESSES_OF_A_STACK_PUSH.map(|(name, count)| (name.to_string(), count))
  );
  assert_eq!(fixture.operations().await, Vec::new());
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// The processes of `a_push_of_a_stack_runs_the_processes_of_the_core_push`
const PROCESSES_OF_A_STACK_PUSH: [(&str, u32); 7] = [
  ("cat-file", 1),
  ("config", 1),
  ("for-each-ref", 3),
  ("log", 1),
  ("push", 1),
  ("reflog", 1),
  ("rev-parse", 1),
];

/// `OFFLINE` with the git of the test: what the run of the engine starts is counted
struct Counted {
  git: GitCommandExecutor,
}

impl ServiceHost for Counted {
  fn git(&self) -> GitCommandExecutor {
    self.git.clone()
  }

  fn patronus(&self, key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    OFFLINE.patronus(key)
  }

  fn github(&self, key: &SessionKey) -> Result<Arc<GitHubService>, ApiError> {
    OFFLINE.github(key)
  }

  fn space_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    OFFLINE.space_repository(key)
  }

  fn github_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    OFFLINE.github_repository(key)
  }

  fn new_api(&self, key: &SessionKey) -> bool {
    OFFLINE.new_api(key)
  }

  fn object_cache<'a>(&'a self, key: &'a SessionKey) -> Read<'a, ObjectCache> {
    OFFLINE.object_cache(key)
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    OFFLINE.api_error(error)
  }

  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    OFFLINE.fetch_moved_remotes(key, asks)
  }

  fn config_git(&self, _key: &SessionKey) -> GitCommandExecutor {
    self.git.clone()
  }
}

/// Another holder of the repository lock (`bd pick --follow` of an agent, a rewrite of the app) keeps the push waiting
/// as long as it lives: the view shows it waiting for the lock and nothing is pushed; once the holder released the lock,
/// the push goes. Stop Waiting (`Registry::cancel`) ends such a wait instead: nothing pushed, the lock still the holder's
#[tokio::test(flavor = "multi_thread")]
async fn a_push_waits_while_the_holder_of_the_lock_lives_and_stops_when_told() {
  let fixture = fixture(true).await;
  let held = fixture.lock();
  let waiting = |operations: &[RunningOperation]| operations.first().is_some_and(|operation| operation.step == OperationStep::WaitingForLock);
  let push = scoped("beta", PushScope::WithAncestors);
  let holder = async {
    fixture.until(waiting).await;
    assert_eq!(fixture.remote_branches(), "");
    drop(held);
  };
  let (pushed, ()) = tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(fixture.execute(&push, Vec::new()), holder) })
    .await
    .unwrap();
  assert_done(&pushed);
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");

  let fixture = fixture_again(fixture).await;
  let held = fixture.lock();
  let stop = async {
    let [operation] = fixture.until(waiting).await.try_into().unwrap();
    assert!(fixture.registry.cancel(&operation.id));
  };
  let alpha = alone("alpha", None);
  let (stopped, ()) = tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(fixture.execute(&alpha, Vec::new()), stop) })
    .await
    .unwrap();
  assert_eq!(stopped.map_err(|error| error.code), Err(ErrorCode::Canceled));
  assert_eq!(fixture.remote_branches(), "");
  drop(held);
}

/// A fresh repository of `fixture`'s shape, synced
async fn fixture_again(previous: Fixture) -> Fixture {
  drop(previous);
  fixture(true).await
}

/// Pushes again over the new remote tip when the remote has no commits the branch lacks: nothing is lost. The remote
/// branch of `alpha` was deleted since the row showed it: the lease on the head the user saw is refused, the push fetches
/// it and goes again, once, the same commit it froze under the lock, leased on what the remote has now, and says what it
/// went over (`pushed_over`). Someone's commit on the remote instead is told, never pushed over
#[tokio::test(flavor = "multi_thread")]
async fn pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks() {
  let fixture = fixture(true).await;
  assert_done(&fixture.execute(&alone("alpha", None), Vec::new()).await);
  let alpha = "refs/heads/test/virtual/alpha";
  let seen = fixture.remote_tip("alpha");
  fixture.repo.git(&["--git-dir", ".git/origin.git", "update-ref", "-d", alpha]);
  let local_tip = fixture.repo.git(&["rev-parse", alpha]);
  // a rewrite of alpha since: the push has work, leased on the head the row shows
  fixture.repo.create_commit("(alpha) Alpha, part 2", "alpha-2.txt", "alpha 2\n");
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  let rebuilt = fixture.repo.git(&["rev-parse", alpha]);
  assert_ne!(rebuilt, local_tip);

  let outcome = fixture.execute(&alone("alpha", Some(seen)), Vec::new()).await;
  assert_done(&outcome);
  let OperationResult::Push(result) = outcome.unwrap().result else {
    panic!("no push result")
  };
  assert_eq!(
    result.pushed_over.iter().map(|status| (status.remote_exists, status.action)).collect::<Vec<_>>(),
    [(false, PushAction::Create)],
    "the push went over the deleted remote branch"
  );
  assert_eq!(fixture.remote_tip("alpha"), rebuilt);

  // someone's commit on the remote branch: the push is told what the remote has, and pushes nothing over it
  let tree = fixture.repo.git(&["rev-parse", &format!("{alpha}^{{tree}}")]);
  let theirs = fixture
    .repo
    .git(&["--git-dir", ".git/origin.git", "commit-tree", &tree, "-p", &rebuilt, "-m", "Someone else"]);
  fixture.repo.git(&["--git-dir", ".git/origin.git", "update-ref", alpha, &theirs]);
  fixture.repo.create_commit("(alpha) Alpha, part 3", "alpha-3.txt", "alpha 3\n");
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  let outcome = fixture.execute(&alone("alpha", Some(rebuilt)), Vec::new()).await.unwrap();
  let EffectStatus::Failed { error } = &outcome.effects[0].status else {
    panic!("{outcome:?}")
  };
  assert_eq!(error.code, ErrorCode::LeaseRejected);
  assert_eq!(fixture.remote_tip("alpha"), theirs, "nothing was pushed over it");
}

/// Two pushes of different branches run side by side: the registry admits each per ref it writes and the lock is held
/// only while a push is decided, so while the first one pushes (held in its network push) the second one decides,
/// pushes and ends before the first does
#[tokio::test(flavor = "multi_thread")]
async fn the_pushes_of_two_branches_run_side_by_side() {
  let fixture = fixture(true).await;
  let (first, mut holds, release) = fixture.held(alone("alpha", None));
  let second = async {
    assert_eq!(holds.recv().await, Some("pushes"));
    drop(fixture.lock());
    assert_done(&fixture.execute(&alone("beta", None), Vec::new()).await);
    release.send(()).unwrap();
  };
  let (first, ()) = Box::pin(tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(first, second) })).await.unwrap();
  assert_done(&first);
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// A push of a ref another push writes (Push Stack while the push of its root runs) waits for it in the registry
/// (`Queued` behind it), is decided again on what it left (the root is on the remote: only the branch built on it goes),
/// and pushes it; a second click of the same push joins it
#[tokio::test(flavor = "multi_thread")]
async fn the_pushes_of_one_ref_wait_for_each_other_and_both_push() {
  let fixture = fixture(true).await;
  let (first, mut holds, release) = fixture.held(alone("alpha", None));
  let (stack_push, again_push) = (scoped("beta", PushScope::WithAncestors), alone("alpha", None));
  let stack = async {
    assert_eq!(holds.recv().await, Some("pushes"));
    tokio::join!(fixture.execute(&stack_push, Vec::new()), fixture.execute(&again_push, Vec::new()))
  };
  let wait = async {
    let operations = fixture.until_queued().await;
    assert_eq!(operations.len(), 2, "the push of the stack waits, the second click joined: {operations:?}");
    release.send(()).unwrap();
  };
  let (first, (stack, again), ()) = Box::pin(tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(first, stack, wait) }))
    .await
    .unwrap();
  assert_done(&first);
  assert_eq!(
    first.as_ref().map(|outcome| outcome.op.clone()).ok(),
    again.as_ref().map(|outcome| outcome.op.clone()).ok(),
    "one push"
  );
  assert_done(&stack);
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// A push holds no lock while it sends (C1): held in its network push, the push of another branch, a squash of the
/// user's history (a rewrite under the lock with `RewriteOptions::default()`, which a held lock would refuse) and a sync
/// of the session that rebuilds the branch all finish; released, the push sends the commit it froze under the lock, not
/// the one the sync built since
#[tokio::test(flavor = "multi_thread")]
async fn a_push_that_sends_holds_no_lock_and_sends_the_commit_it_froze() {
  let fixture = fixture(true).await;
  let frozen = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let (first, mut holds, release) = fixture.held(alone("alpha", None));
  let others = async {
    assert_eq!(holds.recv().await, Some("pushes"));
    assert_done(&fixture.execute(&alone("beta", None), Vec::new()).await);
    let params = SquashCommitsParams {
      repository_path: fixture.repo.path_str().to_string(),
      commit_ids: vec![fixture.repo.rev_parse("HEAD~1").unwrap(), fixture.repo.rev_parse("HEAD").unwrap()],
      message: "(alpha) Alpha and beta".to_string(),
    };
    tokio::task::block_in_place(|| squash_commits_core(&GitCommandExecutor::new(), &params, &RewriteOptions::default())).unwrap();
    fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
    assert_ne!(fixture.repo.rev_parse("test/virtual/alpha").unwrap(), frozen, "the sync rebuilt alpha meanwhile");
    release.send(()).unwrap();
  };
  let (first, ()) = Box::pin(tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(first, others) })).await.unwrap();
  assert_done(&first);
  assert_eq!(fixture.remote_tip("alpha"), frozen);
}

/// The push of an unapplied branch reads no robots (C3): no Safe Push runs on one. The push of a virtual branch reads
/// one full list, before the lock
#[tokio::test(flavor = "multi_thread")]
async fn the_push_of_an_unapplied_branch_reads_no_robots() {
  let fixture = with_unapplied_old().await;
  let (fake, client) = start_fake().await;
  let service = PatronusService::new(client, |_| {
    TokenLookup::Found(Token {
      value: "t".to_string(),
      stamp: TokenStamp::Env,
    })
  });
  let on_space = Services {
    patronus: Some(Arc::new(service)),
  };
  let unapplied = Push {
    scope: PushScope::Unapplied,
    ..alone("old", None)
  };
  assert_done(&fixture.execute_on(&unapplied, Vec::new(), &on_space, Arc::new(NoEvents)).await);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 0);
  assert_done(&fixture.execute_on(&alone("alpha", None), Vec::new(), &on_space, Arc::new(NoEvents)).await);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 1);
}
