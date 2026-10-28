use crate::land_execute::{Accepted, Current, Planned, PullRequestText, PushLease, PushSettings, PushedRef, StartHost, StartServices, Started, Steps, bounded_lock, execute};
use crate::push::Sent;
use crate::witness::{Witness, read_refs};
use domain_types::RemoteStatusUpdate;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{land_intent, robot};
use domain_types::landing::{BaselinePush, Confirmation, Land, PlannedEffect, StartRoute, Subject};
use domain_types::operation::{EffectStatus, Fingerprint, Intent, OperationId, Plan};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::reason::Reason;
use domain_types::session::Version;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{lock_repository_as_another_process, try_lock_repository_in};
use patronus::fake_patronus::{FakePatronus, robot_json, start_fake};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use pretty_assertions::assert_eq;
use remote_facts::fact_sources::Read;
use repository_model::facts::BaselineRefusal;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::table::RemoteSubject;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_core::transfer_progress::TransferProgress;
use sync_utils::clock::TestClock;
use test_utils::git_test_utils::{TestRepo, git_command, pre_receive_hook, refuse_every_push, stack_ahead_of_bare_origin};

pub(crate) const ALPHA: &str = "refs/heads/test/virtual/alpha";
pub(crate) const ALPHA_REMOTE: &str = "refs/remotes/origin/test/virtual/alpha";

/// The facts of a start as a test holds them: its plan, the robots of the session, and what the start told it
pub(crate) struct Host {
  pub(crate) planned: Planned,
  pub(crate) robots: Mutex<Vec<PatronusRobot>>,
  /// Listed from the end of the push on: a robot another process started meanwhile
  meanwhile: Option<PatronusRobot>,
  /// What the start told the session, in order: `push <subjects>`, `pushed`, `robot <id>`
  told: Mutex<Vec<String>>,
}

impl Host {
  pub(crate) fn told(&self) -> Vec<String> {
    self.told.lock().unwrap().clone()
  }

  fn tell(&self, what: String) {
    self.told.lock().unwrap().push(what);
  }
}

impl StartHost for Host {
  fn plan<'a>(&'a self, _land: &'a Land) -> Read<'a, Result<Option<Planned>, ApiError>> {
    Box::pin(std::future::ready(Ok(Some(self.planned.clone()))))
  }

  fn current(&self) -> Read<'_, Current> {
    let current = Current {
      robots: self.robots.lock().unwrap().clone(),
      pull_requests: Vec::new(),
    };
    Box::pin(std::future::ready(current))
  }

  fn robots_since(&self, _since: u32) -> Read<'_, Result<Vec<PatronusRobot>, ApiError>> {
    Box::pin(std::future::ready(Ok(Vec::new())))
  }

  fn shown_remote<'a>(&'a self, pushes: &'a [PushedRef]) -> Read<'a, Vec<Option<RemoteStatusUpdate>>> {
    Box::pin(std::future::ready(vec![None; pushes.len()]))
  }

  fn begin_push(&self, subjects: Vec<RemoteSubject>) {
    self.tell(format!("push {subjects:?}"));
  }

  fn end_push<'a>(&'a self, _subjects: Vec<RemoteSubject>, sent: Option<&'a Sent>) -> Read<'a, Option<Version>> {
    self.tell(format!("pushed {}", sent.is_some()));
    self.robots.lock().unwrap().extend(self.meanwhile.clone());
    Box::pin(std::future::ready(None))
  }

  fn write_robot(&self, robot: PatronusRobot) -> Read<'_, Option<Version>> {
    self.tell(format!("robot {} {:?}", robot.id, robot.status));
    Box::pin(std::future::ready(None))
  }

  fn pull_request_text<'a>(&'a self, _sent: Option<&'a Sent>) -> Read<'a, Result<PullRequestText, ApiError>> {
    Box::pin(std::future::ready(Err(ApiError::internal("no pull request"))))
  }

  fn write_pull_request(&self, write: PullRequestWrite) -> Read<'_, Option<Version>> {
    self.tell(format!("pull request {write:?}"));
    Box::pin(std::future::ready(None))
  }

  fn baseline_pushed(&self, landed: Vec<(String, BaselinePush)>, refused: Option<BaselineRefusal>) -> Read<'_, Option<Version>> {
    let landed: Vec<String> = landed.into_iter().map(|(key, push)| format!("{key}@{} into {}", push.tip, push.upstream)).collect();
    match refused {
      Some(refused) => self.tell(format!("refused by {}: {}", refused.target, refused.message)),
      None => self.tell(format!("landed {}", landed.join(", "))),
    }
    Box::pin(std::future::ready(None))
  }

  fn baseline_fetched(&self) {
    self.tell("fetched".to_string());
  }
}

/// `stack_ahead_of_bare_origin` (`alpha` not on the remote yet) and a fake Patronus whose Safe Push `r1` runs on an
/// older commit of `alpha`
pub(crate) struct Fixture {
  pub(crate) repo: TestRepo,
  pub(crate) fake: Arc<FakePatronus>,
  pub(crate) services: StartServices,
}

pub(crate) async fn fixture() -> Fixture {
  let repo = stack_ahead_of_bare_origin();
  let (fake, client) = start_fake().await;
  *fake.robots.lock().unwrap() = Some(vec![robot_json("r1", "RUNNING", ALPHA, "2026-10-14T09:30:00Z")]);
  let service = PatronusService::new(client, |_| {
    TokenLookup::Found(Token {
      value: "t".to_string(),
      stamp: TokenStamp::Env,
    })
  });
  let common_dir = repo.path().join(".git");
  // the bounded wait of `bd`, which a holder refuses at once
  let lock = bounded_lock(common_dir.clone(), Duration::ZERO);
  let services = StartServices {
    git: GitCommandExecutor::new(),
    repository_path: repo.path_str().to_string(),
    git_common_dir: common_dir,
    prefix: "test".to_string(),
    patronus: Some(Arc::new(service)),
    space: "ultimate".to_string(),
    github: None,
    target: "master".to_string(),
    push: PushSettings {
      remote: "origin".to_string(),
      baseline: "origin/master".to_string(),
      size_limit: None,
    },
    lock,
    clock: Arc::new(TestClock::new()),
    wait_for_robot: false,
  };
  Fixture { repo, fake, services }
}

/// The Safe Push of `alpha`
fn alpha() -> Land {
  let Intent::Land(land) = land_intent("alpha", PatronusPushMode::RebaseAutosquash) else {
    unreachable!()
  };
  land
}

impl Fixture {
  /// The plan of the Safe Push of `alpha` with `effects`, its witness read now: the tip of `alpha` and its remote branch
  pub(crate) fn planned(&self, effects: Vec<PlannedEffect>) -> Planned {
    let names = [ALPHA.to_string(), ALPHA_REMOTE.to_string()];
    let refs = read_refs(&self.services.git, &self.services.repository_path, &names).unwrap();
    let pushes = effects.iter().any(|effect| matches!(effect, PlannedEffect::Push { .. })).then(|| PushedRef {
      name: "alpha".to_string(),
      full_name: "test/virtual/alpha".to_string(),
      parent_ref: None,
      whole_stack: false,
      unapplied: false,
      up_to: None,
      lease: PushLease::Frozen,
      my_email: None,
    });
    Planned {
      plan: Plan {
        intent: Intent::Land(alpha()),
        fingerprint: Fingerprint("planned".to_string()),
        lands: vec!["alpha".to_string()],
        effects,
        confirmations: Vec::new(),
        blockers: Vec::new(),
        waits: Vec::new(),
        alternatives: None,
        pull_request: None,
        approvals_skipped_by: None,
      },
      witness: Witness::of(refs, &[], &[]),
      pushes: pushes.into_iter().collect(),
      source_ref: ALPHA.to_string(),
      merge_request: None,
      notes: Vec::new(),
      subject: alpha().subject,
      mode: PatronusPushMode::RebaseAutosquash,
    }
  }

  /// The host of `planned` whose session lists `r1` running on `alpha`
  pub(crate) fn host(planned: Planned) -> Host {
    Host {
      planned,
      robots: Mutex::new(vec![robot("r1", Some("alpha"), PatronusRobotStatus::Running, 1)]),
      meanwhile: None,
      told: Mutex::new(Vec::new()),
    }
  }

  /// The run of the plan of `host`, accepted as planned
  async fn run(&self, host: &Host, steps: &Steps) -> Result<Started, ApiError> {
    let accepted = Accepted {
      fingerprint: Fingerprint("planned".to_string()),
      confirmations: host.planned.plan.confirmations.clone(),
    };
    execute(host, &self.services, steps, OperationId("op".to_string()), &alpha(), &accepted, false).await
  }

  /// The commit of `alpha` on the bare origin; `None` while it has none
  pub(crate) fn remote_alpha(&self) -> Option<String> {
    let listed = self.repo.git(&["--git-dir", ".git/origin.git", "for-each-ref", "--format=%(objectname)", ALPHA]);
    (!listed.is_empty()).then_some(listed)
  }

  fn starts(&self) -> usize {
    self.fake.starts.lock().unwrap().len()
  }

  pub(crate) fn canceled(&self) -> Vec<String> {
    self.fake.canceled.lock().unwrap().clone()
  }
}

/// The effects of the start of the Safe Push of `alpha` that replaces `r1`: its cancel, the push, the start
fn replace_and_start() -> Vec<PlannedEffect> {
  vec![
    PlannedEffect::CancelRobot { robot_id: "r1".to_string() },
    PlannedEffect::Push {
      branches: vec!["alpha".to_string()],
      force: false,
    },
    PlannedEffect::StartRobot {
      route: StartRoute::Api { profile: None },
    },
  ]
}

pub(crate) fn statuses(started: &Started) -> Vec<&EffectStatus> {
  started.outcome.effects.iter().map(|record| &record.status).collect()
}

/// A start runs the effects of its plan in their order: the cancel of the robot it replaces, the push of the commit it
/// froze, the start of the robot on its virtual branch; each is told to the session (the canceled robot, the push and
/// its end, the started robot). The lock is held only for the witness and the freeze: while the push sends, another
/// writer takes it at once
#[tokio::test(flavor = "multi_thread")]
async fn a_start_cancels_the_robot_it_replaces_pushes_and_starts_in_the_order_of_its_plan() {
  let fixture = fixture().await;
  let frozen = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let host = Fixture::host(fixture.planned(replace_and_start()));
  let common_dir = fixture.repo.path().join(".git");
  let free_while_pushing = Arc::new(AtomicBool::new(false));
  let free = Arc::clone(&free_while_pushing);
  let steps = Steps::new(
    Arc::new(|_| {}),
    Arc::new(move |_: &TransferProgress| {
      free.store(try_lock_repository_in(&common_dir).unwrap().is_some(), Ordering::SeqCst);
    }),
    Arc::new(AtomicBool::new(false)),
  );
  let started = fixture.run(&host, &steps).await.unwrap();
  assert_eq!(statuses(&started), [&EffectStatus::Done, &EffectStatus::Done, &EffectStatus::Done]);
  assert_eq!(fixture.canceled(), ["r1"]);
  assert_eq!(fixture.remote_alpha(), Some(frozen));
  assert_eq!(fixture.fake.last_start()["sourceBranch"], ALPHA);
  let robot = started.robot.expect("the started robot");
  assert_eq!(
    host.told(),
    [
      "robot r1 Running".to_string(),
      "push [Branch(\"alpha\")]".to_string(),
      "pushed true".to_string(),
      format!("robot {} {:?}", robot.id, robot.status)
    ]
  );
  assert!(free_while_pushing.load(Ordering::SeqCst), "the push sends without the lock");
}

/// The first effect of a start into a pull request of a virtual branch is its retarget into the target (D11): GitHub gets
/// the new base before the push sends, and the robot starts after both; a retarget GitHub refuses of the pull request
/// the start goes from stops it before anything is pushed
#[tokio::test(flavor = "multi_thread")]
async fn a_start_retargets_the_pull_request_out_of_a_virtual_branch_before_it_pushes_and_starts() {
  let mut fixture = fixture().await;
  let (github, service) = github::fake_github::start_fake_service("token").await;
  let pull = "/repos/acme/monorepo/pulls/8";
  github.answer("PATCH", pull, github::fake_github::FakeAnswer::ok(serde_json::json!({"number": 8})));
  let repository = sync_core::github_repository::GitHubRepository {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
  };
  fixture.services.github = Some((Arc::new(service), repository));
  let retarget = PlannedEffect::Retarget {
    branch: "alpha".to_string(),
    number: 8,
    from: "test/virtual/base".to_string(),
    to: "master".to_string(),
  };
  let effects = [vec![retarget.clone()], replace_and_start()[1..].to_vec()].concat();
  let host = Fixture::host(fixture.planned(effects.clone()));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  assert_eq!(statuses(&started), [&EffectStatus::Done, &EffectStatus::Done, &EffectStatus::Done]);
  assert_eq!(github.requests_to("PATCH", pull)[0].json(), serde_json::json!({"base": "master"}));
  assert_eq!(fixture.starts(), 1);

  github.answer(
    "PATCH",
    pull,
    github::fake_github::FakeAnswer::status(422, serde_json::json!({"message": "Validation Failed"})),
  );
  let mut planned = fixture.planned(effects);
  planned.plan.pull_request = Some(8);
  let refused = fixture.run(&Fixture::host(planned), &Steps::none()).await.unwrap();
  let [retargeted, pushed, start] = statuses(&refused).try_into().unwrap();
  assert!(matches!(retargeted, EffectStatus::Failed { .. }), "{retargeted:?}");
  assert_eq!((pushed, start), (&EffectStatus::Skipped { reason: None }, &EffectStatus::Skipped { reason: None }));
  assert_eq!(fixture.starts(), 1, "no second start");
}

/// The first effect that fails stops the start: a push the remote refuses starts no robot, and the robot canceled
/// before it stays canceled; a cancel Patronus fails pushes nothing
#[tokio::test(flavor = "multi_thread")]
async fn the_first_effect_that_fails_stops_the_start_and_the_ones_before_it_stay_done() {
  let fixture = fixture().await;
  refuse_every_push(&fixture.repo.path().join(".git/origin.git"));
  let host = Fixture::host(fixture.planned(replace_and_start()));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  let [canceled, pushed, start] = statuses(&started).try_into().unwrap();
  assert_eq!(canceled, &EffectStatus::Done);
  assert!(matches!(pushed, EffectStatus::Failed { error } if error.code == ErrorCode::PushRefused), "{pushed:?}");
  assert_eq!(start, &EffectStatus::Skipped { reason: None });
  assert_eq!((fixture.canceled().len(), fixture.starts(), fixture.remote_alpha()), (1, 0, None));

  let fixture = self::fixture().await;
  fixture.fake.set_failing("PUT", "/app/rest/v1/robots/*/cancel", true);
  let host = Fixture::host(fixture.planned(replace_and_start()));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  assert!(matches!(statuses(&started)[0], EffectStatus::Failed { error } if error.code == ErrorCode::PatronusError));
  assert_eq!(&statuses(&started)[1..], [&EffectStatus::Skipped { reason: None }, &EffectStatus::Skipped { reason: None }]);
  assert_eq!((fixture.starts(), fixture.remote_alpha()), (0, None));
}

/// A rewrite of the virtual branch while its push sends (after the lock, before the start): the push sends the commit
/// it froze, the start is skipped as `plan_changed`, and the outcome tells the cancel and the push it made before
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_while_the_push_sends_skips_the_start_and_the_outcome_tells_what_was_done() {
  let fixture = fixture().await;
  let frozen = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let rewritten = fixture.repo.rev_parse("test/virtual/beta").unwrap();
  let host = Fixture::host(fixture.planned(replace_and_start()));
  let path = fixture.repo.path().to_path_buf();
  let steps = Steps::new(
    Arc::new(|_| {}),
    Arc::new(move |_: &TransferProgress| {
      let moved = git_command().args(["update-ref", ALPHA, &rewritten]).current_dir(&path).status().unwrap();
      assert!(moved.success());
    }),
    Arc::new(AtomicBool::new(false)),
  );
  let started = fixture.run(&host, &steps).await.unwrap();
  let [canceled, pushed, start] = statuses(&started).try_into().unwrap();
  assert_eq!((canceled, pushed), (&EffectStatus::Done, &EffectStatus::Done));
  assert!(
    matches!(start, EffectStatus::Skipped { reason: Some(reason) } if reason.code == ErrorCode::PlanChanged),
    "{start:?}"
  );
  assert_eq!((fixture.remote_alpha(), fixture.starts()), (Some(frozen), 0));
}

/// What refuses a start before its first effect does nothing: a fingerprint the plan no longer has, a ref that moved
/// since the plan, a lock another holds, a blocker of the plan, a confirmation the caller did not accept
#[tokio::test(flavor = "multi_thread")]
async fn a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing() {
  let fixture = fixture().await;
  let host = Fixture::host(fixture.planned(replace_and_start()));
  let older = Accepted {
    fingerprint: Fingerprint("older".into()),
    confirmations: Vec::new(),
  };
  let other = execute(&host, &fixture.services, &Steps::none(), OperationId("op".into()), &alpha(), &older, false).await;
  assert_eq!(other.unwrap_err().code, ErrorCode::PlanChanged);

  let moved = Fixture::host(fixture.planned(replace_and_start()));
  fixture.repo.git(&["update-ref", ALPHA, &fixture.repo.rev_parse("test/virtual/beta").unwrap()]);
  assert_eq!(fixture.run(&moved, &Steps::none()).await.unwrap_err().code, ErrorCode::PlanChanged);

  let held = lock_repository_as_another_process(&fixture.repo.path().join(".git"), "bd push auth").unwrap();
  let locked = Fixture::host(fixture.planned(replace_and_start()));
  assert_eq!(fixture.run(&locked, &Steps::none()).await.unwrap_err().code, ErrorCode::Locked);
  drop(held);

  let mut planned = fixture.planned(replace_and_start());
  planned.plan.blockers.push(Reason::Parked {
    branch: "alpha".to_string(),
    with: None,
    note: String::new(),
  });
  assert_eq!(fixture.run(&Fixture::host(planned), &Steps::none()).await.unwrap_err().code, ErrorCode::Parked);

  let mut planned = fixture.planned(replace_and_start());
  planned.plan.confirmations.push(Confirmation::CancelsRobot { robot_id: "r1".to_string() });
  let host = Fixture::host(planned);
  let none = Accepted {
    fingerprint: Fingerprint("planned".into()),
    confirmations: Vec::new(),
  };
  let unaccepted = execute(&host, &fixture.services, &Steps::none(), OperationId("op".into()), &alpha(), &none, false).await;
  assert_eq!(unaccepted.unwrap_err().code, ErrorCode::PlanChanged);

  assert_eq!((fixture.canceled().len(), fixture.starts(), fixture.remote_alpha()), (0, 0, None));
}

/// A start that waited in the queue of the session (behind the push of its branch pressed just before it) runs on the
/// plan the wait left, not on the one its click accepted: the push went, so the start pushes nothing more and starts
/// the robot. What the plan after the wait asks that the user did not accept refuses it, in the words of the start
#[tokio::test(flavor = "multi_thread")]
async fn a_start_that_waited_runs_on_the_plan_the_wait_left_while_the_user_accepted_what_it_asks() {
  let fixture = fixture().await;
  let start = vec![PlannedEffect::StartRobot {
    route: StartRoute::Api { profile: None },
  }];
  // the click accepted the plan with the push, before the push it waited for went
  let before_the_push = Accepted {
    fingerprint: Fingerprint("with the push".into()),
    confirmations: Vec::new(),
  };
  let host = Fixture::host(fixture.planned(start.clone()));
  let started = execute(&host, &fixture.services, &Steps::none(), OperationId("op".into()), &alpha(), &before_the_push, true)
    .await
    .unwrap();
  assert_eq!(statuses(&started), [&EffectStatus::Done]);
  assert_eq!((fixture.starts(), fixture.remote_alpha()), (1, None), "one start, no push of its own");

  let mut asks = fixture.planned(start);
  asks.plan.confirmations.push(Confirmation::CancelsRobot { robot_id: "r1".to_string() });
  let host = Fixture::host(asks);
  let error = execute(&host, &fixture.services, &Steps::none(), OperationId("op".into()), &alpha(), &before_the_push, true)
    .await
    .unwrap_err();
  assert_eq!(error.code, ErrorCode::PlanChanged);
  assert_eq!(
    error.message,
    "What the Safe Push of alpha was decided on changed before it began (it asks to accept more than was accepted); nothing was done: start it again"
  );
  assert_eq!(fixture.starts(), 1);
}

/// A Safe Push of what the start lands that another process started while the push sent (one the facts had not listed
/// before the run) skips the start as `plan_changed`: two robots would land the same commits
#[tokio::test(flavor = "multi_thread")]
async fn a_robot_of_what_it_lands_started_meanwhile_skips_the_start() {
  let fixture = fixture().await;
  let mut host = Fixture::host(fixture.planned(replace_and_start()[1..].to_vec()));
  host.meanwhile = Some(robot("r9", Some("alpha"), PatronusRobotStatus::Running, 2));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  let [pushed, start] = statuses(&started).try_into().unwrap();
  assert_eq!(pushed, &EffectStatus::Done);
  assert!(
    matches!(start, EffectStatus::Skipped { reason: Some(reason) } if reason.code == ErrorCode::PlanChanged),
    "{start:?}"
  );
  assert_eq!(fixture.starts(), 0);
}

/// The push of the copy of `alpha` at `tip` into master
fn push_alpha(tip: &str) -> Vec<PlannedEffect> {
  vec![PlannedEffect::PushBaseline {
    subject: Subject::branch("alpha"),
    tip: tip.to_string(),
    remote: "origin".to_string(),
    target: "master".to_string(),
  }]
}

/// The commit of master on the bare origin
fn remote_master(fixture: &Fixture) -> String {
  fixture.repo.git(&["--git-dir", ".git/origin.git", "rev-parse", "refs/heads/master"])
}

/// The push of the copy of `alpha` into master, which fails: its error, and what the session heard
async fn failed_push(fixture: &Fixture) -> (ApiError, Vec<String>) {
  let tip = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let host = Fixture::host(fixture.planned(push_alpha(&tip)));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  match statuses(&started)[..] {
    [EffectStatus::Failed { error }] => (error.clone(), host.told()),
    ref statuses => panic!("the push fails: {statuses:?}"),
  }
}

/// A push into the target lands the copy the witness froze as a fast-forward, by the push URL: the remote-tracking ref of
/// master stays where the last fetch left it, and the session hears what landed, each branch at its tip, until a fetch
/// shows it; no robot is asked, no virtual branch pushed
#[tokio::test(flavor = "multi_thread")]
async fn a_push_into_the_target_lands_the_copy_as_a_fast_forward_and_tells_what_it_landed() {
  let fixture = fixture().await;
  let tip = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let fetched = fixture.repo.rev_parse("refs/remotes/origin/master").unwrap();
  let host = Fixture::host(fixture.planned(push_alpha(&tip)));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  assert_eq!(statuses(&started), [&EffectStatus::Done]);
  assert_eq!(remote_master(&fixture), tip);
  assert_eq!(fixture.repo.rev_parse("refs/remotes/origin/master").unwrap(), fetched);
  assert_eq!((fixture.starts(), fixture.remote_alpha()), (0, None));
  assert_eq!(host.told(), [format!("landed alpha@{tip} into origin/master")]);
}

/// A target that moved since the copy was built on it refuses the push as no fast-forward: the start fetches the
/// baseline and asks the sync that builds the copy anew, and fails as `baseline_moved`, pushing nothing again
#[tokio::test(flavor = "multi_thread")]
async fn a_target_that_moved_is_fetched_and_synced_and_the_push_is_baseline_moved() {
  let fixture = fixture().await;
  let base = remote_master(&fixture);
  let tree = fixture.repo.git(&["rev-parse", &format!("{base}^{{tree}}")]);
  let other = fixture.repo.git(&["commit-tree", &tree, "-p", &base, "-m", "Someone else"]);
  fixture.repo.git(&["push", "-q", ".git/origin.git", &format!("{other}:refs/heads/master")]);
  let (error, told) = failed_push(&fixture).await;
  assert_eq!(error.code, ErrorCode::BaselineMoved, "{}", error.message);
  assert_eq!(remote_master(&fixture), other);
  assert_eq!(fixture.repo.rev_parse("refs/remotes/origin/master").unwrap(), other);
  assert_eq!(told, ["fetched"]);
}

/// The server refusing the push (a hook, a protected branch) fails it as `baseline_refused` in the server's words, which
/// go into the facts: the gate of the route asks about them before the next push
#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_of_the_server_is_baseline_refused_in_its_words_and_goes_into_the_facts() {
  let fixture = fixture().await;
  pre_receive_hook(&fixture.repo.path().join(".git/origin.git"), "echo 'master is protected' >&2\nexit 1");
  let base = remote_master(&fixture);
  let (error, told) = failed_push(&fixture).await;
  assert_eq!(error.code, ErrorCode::BaselineRefused);
  assert!(error.message.contains("master is protected"), "{}", error.message);
  assert_eq!(remote_master(&fixture), base);
  assert!(
    told.len() == 1 && told[0].starts_with("refused by master: ") && told[0].contains("master is protected"),
    "{told:?}"
  );
}

/// A fake GitHub of acme/monorepo whose pull request #7 shows `head` and takes every mutation, the services of the
/// fixture on it
async fn on_github(fixture: &mut Fixture, head: &str) -> Arc<github::fake_github::FakeGitHub> {
  use github::fake_github::{FakeAnswer, start_fake_service};
  let (fake, service) = start_fake_service("t").await;
  let node = serde_json::json!({"data": {"repository": {"pullRequest": {"id": "PR_7", "headRefOid": head}}}});
  fake.answer_with_body("POST", "/graphql", "pullRequest(number: $number)", FakeAnswer::ok(node));
  fake.answer("POST", "/graphql", FakeAnswer::ok(serde_json::json!({"data": {"done": {"pullRequest": {"number": 7}}}})));
  let repository = sync_core::github_repository::GitHubRepository {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  };
  fixture.services.github = Some((Arc::new(service), repository));
  fake
}

fn merge_alpha(head: &str, auto: bool) -> PlannedEffect {
  let (subject, number, method, head) = (Subject::branch("alpha"), 7, domain_types::pull_request::MergeMethod::Rebase, head.to_string());
  if auto {
    PlannedEffect::EnableAutoMerge { subject, number, method, head }
  } else {
    PlannedEffect::MergePullRequest { subject, number, method, head }
  }
}

/// The mutations the fake GitHub got, by the name of each
fn mutations(fake: &github::fake_github::FakeGitHub) -> Vec<String> {
  fake
    .requests_to("POST", "/graphql")
    .iter()
    .filter_map(|request| {
      let query = request.json()["query"].as_str()?.to_owned();
      let name = query.strip_prefix("mutation(")?.split_once("{\n  ")?.1.split('(').next()?.to_owned();
      Some(format!("{name} {}", request.json()["variables"]))
    })
    .collect()
}

/// GitHub merges the pull request of the subject on the head the plan saw, which goes into the facts as merged (it lands
/// its subject at once); a head that moved since sends nothing and skips it as `plan_changed`; GitHub's refusal is
/// `not_mergeable` in its words
#[tokio::test(flavor = "multi_thread")]
async fn a_merge_of_the_pull_request_goes_into_the_facts_and_a_moved_head_or_a_refusal_of_github_merges_nothing() {
  let head = "0123456789abcdef0123456789abcdef01234567";
  let mut fixture = fixture().await;
  let fake = on_github(&mut fixture, head).await;
  let host = Fixture::host(fixture.planned(vec![merge_alpha(head, false)]));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  assert_eq!(statuses(&started), [&EffectStatus::Done]);
  assert_eq!(mutations(&fake), [format!(r#"mergePullRequest {{"head":"{head}","id":"PR_7","method":"REBASE"}}"#)]);
  assert_eq!(host.told(), ["pull request Merged { number: 7 }"]);
  assert_eq!(fixture.starts(), 0);

  let mut fixture = self::fixture().await;
  let fake = on_github(&mut fixture, "fedcba9876543210fedcba9876543210fedcba98").await;
  let host = Fixture::host(fixture.planned(vec![merge_alpha(head, false)]));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  assert!(
    matches!(&started.outcome.effects[0].status, EffectStatus::Skipped { reason: Some(error) } if error.code == ErrorCode::PlanChanged),
    "{:?}",
    started.outcome.effects
  );
  assert_eq!(mutations(&fake), Vec::<String>::new());

  let mut fixture = self::fixture().await;
  let fake = on_github(&mut fixture, head).await;
  let refusal = serde_json::json!({"data": null, "errors": [{"message": "Pull request is not mergeable"}]});
  fake.answer("POST", "/graphql", github::fake_github::FakeAnswer::ok(refusal));
  let host = Fixture::host(fixture.planned(vec![merge_alpha(head, false)]));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  let EffectStatus::Failed { error } = &started.outcome.effects[0].status else {
    panic!("{:?}", started.outcome.effects)
  };
  assert_eq!(error.code, ErrorCode::NotMergeable);
  assert!(error.message.contains("Pull request is not mergeable"), "{}", error.message);
  assert_eq!(host.told(), Vec::<String>::new());
}

/// A pull request that showed another commit: the push of the branch first, then the auto-merge on the pushed commit,
/// which goes into the facts
#[tokio::test(flavor = "multi_thread")]
async fn the_auto_merge_after_the_push_of_the_branch_goes_on_the_pushed_commit() {
  let mut fixture = fixture().await;
  let tip = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let fake = on_github(&mut fixture, &tip).await;
  let push = PlannedEffect::Push {
    branches: vec!["alpha".to_string()],
    force: false,
  };
  let host = Fixture::host(fixture.planned(vec![push, merge_alpha(&tip, true)]));
  let started = fixture.run(&host, &Steps::none()).await.unwrap();
  assert_eq!(statuses(&started), [&EffectStatus::Done, &EffectStatus::Done]);
  assert_eq!(fixture.remote_alpha(), Some(tip.clone()));
  assert_eq!(
    mutations(&fake),
    [format!(r#"enablePullRequestAutoMerge {{"head":"{tip}","id":"PR_7","method":"REBASE"}}"#)]
  );
  assert_eq!(host.told()[2..], ["pull request AutoMerge { number: 7, method: Some(Rebase) }".to_string()]);
}
