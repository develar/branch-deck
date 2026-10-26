use crate::land_execute::{Accepted, Current, Planned, PullRequestText, PushLease, PushSettings, PushedRef, StartHost, StartServices, Started, Steps, bounded_lock, execute};
use crate::push::Sent;
use crate::witness::{Witness, read_refs};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{land_intent, robot};
use domain_types::landing::{Confirmation, Land, PlannedEffect, StartRoute};
use domain_types::operation::{EffectStatus, Fingerprint, Intent, OperationId, Plan};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::reason::Reason;
use domain_types::session::Version;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::lock_repository_in;
use patronus::fake_patronus::{FakePatronus, robot_json, start_fake};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use pretty_assertions::assert_eq;
use remote_facts::fact_sources::Read;
use repository_model::pull_request_facts::PullRequestWrite;
use repository_model::table::RemoteSubject;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_core::transfer_progress::TransferProgress;
use sync_utils::clock::TestClock;
use test_utils::git_test_utils::{TestRepo, git_command, refuse_every_push, stack_ahead_of_bare_origin};

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
      my_email: None,
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
    execute(host, &self.services, steps, OperationId("op".to_string()), &alpha(), &accepted).await
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
      free.store(lock_repository_in(&common_dir, Duration::ZERO, &|| {}).is_ok(), Ordering::SeqCst);
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
  let other = execute(&host, &fixture.services, &Steps::none(), OperationId("op".into()), &alpha(), &older).await;
  assert_eq!(other.unwrap_err().code, ErrorCode::PlanChanged);

  let moved = Fixture::host(fixture.planned(replace_and_start()));
  fixture.repo.git(&["update-ref", ALPHA, &fixture.repo.rev_parse("test/virtual/beta").unwrap()]);
  assert_eq!(fixture.run(&moved, &Steps::none()).await.unwrap_err().code, ErrorCode::PlanChanged);

  let held = lock_repository_in(&fixture.repo.path().join(".git"), Duration::ZERO, &|| {}).unwrap();
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
  let unaccepted = execute(&host, &fixture.services, &Steps::none(), OperationId("op".into()), &alpha(), &none).await;
  assert_eq!(unaccepted.unwrap_err().code, ErrorCode::PlanChanged);

  assert_eq!((fixture.canceled().len(), fixture.starts(), fixture.remote_alpha()), (0, 0, None));
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
