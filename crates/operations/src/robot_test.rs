use crate::http_budget_test::{TIP, table};
use crate::land_execute::land_execute_test::{ALPHA, Fixture, fixture, statuses};
use crate::land_execute::{Accepted, Planned, Steps, run_planned};
use crate::pull_request::{retarget_plan, retarget_request};
use crate::robot::{cancel, cancel_plan, cancel_request, restart_planned, restart_request};
use crate::witness::Witness;
use domain_policy::admission::admit;
use domain_types::fixtures::{pull_request, robot};
use domain_types::landing::{PlannedEffect, Subject};
use domain_types::landing_route::{LandingRoute, RouteGate};
use domain_types::operation::{Admission, EffectStatus, Fingerprint, Intent, OperationId, OperationResult, OperationStep, Origin, PullRequestField, Resource, RunningOperation};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus, PatronusStartOptions};
use domain_types::pull_request::PullRequestState;
use domain_types::reason::Reason;
use domain_types::repository_view::{PatronusView, RestartAction, RobotRestart};
use domain_types::session::TableHeader;
use domain_types::session::{Fact, FactState, LandedSection, PullRequestsView, SubjectReadiness, Version};
use history_rewrite::repository_lock::lock_repository_as_another_process;
use pretty_assertions::assert_eq;
use repository_model::branch_table::{BranchTable, RemoteFact};
use repository_model::derive::LandSources;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// A robot of `auth` on `source`
fn robot_on(id: &str, status: PatronusRobotStatus, source: &str) -> PatronusRobot {
  PatronusRobot {
    source_revision: Some(source.to_owned()),
    ..robot(id, Some("auth"), status, 1)
  }
}

/// What a plan of a restart (and of a pull request, `pull_request_test`) reads, as a test holds it: the table of `auth`
/// at `TIP`, its robots, the restarts the view offers, the pull requests of the session and the review readiness of its
/// subjects
pub(crate) struct Sources {
  pub(crate) table: BranchTable,
  robots: Vec<PatronusRobot>,
  pub(crate) view: PatronusView,
  pub(crate) pull_requests: PullRequestsView,
  profiles: Fact<PatronusStartOptions>,
  baseline: LandedSection,
  pub(crate) readiness: BTreeMap<String, Fact<SubjectReadiness>>,
}

impl Sources {
  /// `auth` at `TIP` with its remote branch at `remote` (`up_to_date` it has the branch), and `robots`, each offering
  /// `action`
  pub(crate) fn new(remote: Option<&str>, up_to_date: bool, robots: Vec<PatronusRobot>, action: RestartAction) -> Self {
    let mut table = table();
    table.remote.branches.insert(
      "auth".to_owned(),
      RemoteFact {
        remote_tip: remote.map(str::to_owned),
        up_to_date,
        known: true,
        ..RemoteFact::default()
      },
    );
    let restarts = robots.iter().map(|robot| (robot.id.clone(), RobotRestart { action, refusal: None })).collect();
    Self {
      table,
      robots,
      view: PatronusView {
        restarts,
        ..PatronusView::default()
      },
      pull_requests: PullRequestsView::default(),
      profiles: Fact::default(),
      baseline: LandedSection::default(),
      readiness: BTreeMap::new(),
    }
  }

  fn planned(&self, robot_id: &str) -> Option<Planned> {
    self.with(|sources| restart_planned(sources, robot_id))
  }

  /// `decide` over these sources
  pub(crate) fn with<T>(&self, decide: impl FnOnce(&LandSources<'_>) -> T) -> T {
    let readiness = |key: &str| self.readiness.get(key);
    let route = LandingRoute::Patronus { gate: RouteGate::Open };
    let sources = LandSources {
      route: &route,
      pushed: None,
      header: &TableHeader::default(),
      table: &self.table,
      remote: &self.table.remote,
      view: &self.view,
      robots: &self.robots,
      target: "master",
      prefix: "user",
      github: true,
      profiles: &self.profiles,
      pull_requests: &self.pull_requests,
      readiness: &readiness,
      rules: None,
      landing_needs: &FactState::Fresh,
      baseline: &self.baseline,
      gate: None,
      target_conflict: None,
    };
    decide(&sources)
  }
}

const AUTH: &str = "refs/heads/user/virtual/auth";
const AUTH_REMOTE: &str = "refs/remotes/origin/user/virtual/auth";

/// A robot whose remote branch has the branch restarts alone: no push, no ref to witness; one of a branch that changed
/// since is pushed first, the running one canceled before the push into its branch, the restart last; a push that
/// drops commits of the remote the branch lacks refuses the restart, and so does what the view refuses it by
#[test]
fn a_restart_restarts_alone_while_the_remote_has_the_branch_and_pushes_first_a_branch_that_changed() {
  let failed = robot_on("f", PatronusRobotStatus::Failure, TIP);
  let plain = Sources::new(Some(TIP), true, vec![failed], RestartAction::Restart).planned("f").unwrap();
  assert_eq!(plain.plan.intent, Intent::RestartRobot { robot_id: "f".to_owned() });
  assert_eq!(plain.plan.effects, [PlannedEffect::RestartRobot { robot_id: "f".to_owned() }]);
  assert_eq!(
    (plain.plan.lands.as_slice(), plain.witness.refs.len(), plain.pushes.len()),
    (&["auth".to_owned()][..], 0, 0)
  );
  assert_eq!((plain.subject, plain.mode), (Subject::branch("auth"), PatronusPushMode::RebaseAutosquash));

  let running = robot_on("r", PatronusRobotStatus::Running, "older");
  let changed = Sources::new(Some("older"), false, vec![running], RestartAction::PushAndRestart);
  let pushed = changed.planned("r").unwrap();
  assert_eq!(
    pushed.plan.effects,
    [
      PlannedEffect::CancelRobot { robot_id: "r".to_owned() },
      PlannedEffect::Push {
        branches: vec!["auth".to_owned()],
        force: false,
      },
      PlannedEffect::RestartRobot { robot_id: "r".to_owned() },
    ]
  );
  let refs = BTreeMap::from([(AUTH.to_owned(), Some(TIP.to_owned())), (AUTH_REMOTE.to_owned(), Some("older".to_owned()))]);
  assert_eq!(pushed.witness.refs, refs);
  assert_eq!(pushed.pushes.iter().map(|pushed| pushed.full_name.as_str()).collect::<Vec<_>>(), ["user/virtual/auth"]);
  assert!(pushed.plan.blockers.is_empty(), "{:?}", pushed.plan.blockers);
  // the push and the restart are planned on the tips: a branch that moved since is another plan
  let mut moved = changed;
  moved.table.tips.insert("user/virtual/auth".to_owned(), TIP.replace('1', "2"));
  assert_ne!(moved.planned("r").unwrap().plan.fingerprint, pushed.plan.fingerprint);

  let mut drops = Sources::new(
    Some("theirs"),
    false,
    vec![robot_on("d", PatronusRobotStatus::Failure, "older")],
    RestartAction::PushAndRestart,
  );
  drops.table.remote.branches.get_mut("auth").unwrap().drops = 2;
  let refused = drops.planned("d").unwrap();
  assert_eq!(
    refused.plan.blockers,
    [Reason::DropsRemote {
      branch: "auth".to_owned(),
      behind: 2,
    }]
  );

  let parked = Reason::Parked {
    branch: "auth".to_owned(),
    with: None,
    note: String::new(),
  };
  let mut kept = Sources::new(Some(TIP), true, vec![robot_on("p", PatronusRobotStatus::Failure, TIP)], RestartAction::Restart);
  kept.view.restarts.get_mut("p").unwrap().refusal = Some(parked.clone());
  assert_eq!(kept.planned("p").unwrap().plan.blockers, [parked]);
  // a robot the user's robots lack, or one the view offers no restart of: no plan
  assert!(kept.planned("unknown").is_none());
  kept.view.restarts.clear();
  assert!(kept.planned("p").is_none());
}

impl Fixture {
  /// The restart of `r1` of `alpha` with `effects`: the plan of a start with the intent of the restart, its witness the
  /// refs of `alpha` read now when it pushes, none else
  fn restart(&self, effects: Vec<PlannedEffect>) -> Planned {
    let pushes = effects.iter().any(|effect| matches!(effect, PlannedEffect::Push { .. }));
    let mut planned = self.planned(effects);
    planned.plan.intent = Intent::RestartRobot { robot_id: "r1".to_owned() };
    planned.source_ref = String::new();
    if !pushes {
      planned.witness = Witness::default();
    }
    planned
  }

  /// Runs `planned` as accepted, as `execute_restart` runs it once admitted
  async fn run_restart(&self, planned: Planned) -> (crate::land_execute::land_execute_test::Host, crate::land_execute::Started) {
    let host = Self::host(planned.clone());
    let accepted = Accepted {
      fingerprint: Fingerprint("planned".to_owned()),
      confirmations: Vec::new(),
    };
    let started = run_planned(&host, &self.services, &Steps::none(), OperationId("op".to_owned()), planned, &accepted)
      .await
      .unwrap();
    (host, started)
  }
}

/// A restart that pushes first cancels the running robot of an older commit, pushes the commit it froze and restarts the
/// robot after the push, so Patronus takes the pushed commit; the outcome names the new robot and the canceled one
#[tokio::test(flavor = "multi_thread")]
async fn a_restart_that_pushes_first_cancels_the_running_robot_pushes_and_restarts_it_on_the_pushed_commit() {
  let fixture = fixture().await;
  let frozen = fixture.repo.rev_parse("test/virtual/alpha").unwrap();
  let effects = vec![
    PlannedEffect::CancelRobot { robot_id: "r1".to_owned() },
    PlannedEffect::Push {
      branches: vec!["alpha".to_owned()],
      force: false,
    },
    PlannedEffect::RestartRobot { robot_id: "r1".to_owned() },
  ];
  let (host, started) = fixture.run_restart(fixture.restart(effects)).await;
  assert_eq!(statuses(&started), [&EffectStatus::Done, &EffectStatus::Done, &EffectStatus::Done]);
  assert_eq!(
    (fixture.canceled(), fixture.fake.restarts.lock().unwrap().clone()),
    (vec!["r1".to_owned()], vec!["r1".to_owned()])
  );
  assert_eq!(fixture.remote_alpha(), Some(frozen));
  let OperationResult::Robot(result) = &started.outcome.result else {
    panic!("a restart answers a robot: {:?}", started.outcome.result)
  };
  let restarted = result.robot.clone().expect("the restarted robot");
  assert_eq!((result.canceled.as_ref().map(|robot| robot.id.as_str()), result.already_finished), (Some("r1"), false));
  let told = host.told();
  assert_eq!(
    told[1..],
    [
      "push [Branch(\"alpha\")]".to_owned(),
      "pushed true".to_owned(),
      format!("robot {} {:?}", restarted.id, restarted.status)
    ]
  );
}

/// A restart of a robot whose remote branch has the branch moves and leases no ref: it reads no witness and takes no
/// repository lock, so another holder of it holds nothing back
#[tokio::test(flavor = "multi_thread")]
async fn a_restart_on_the_remote_branch_takes_no_lock() {
  let fixture = fixture().await;
  let _held = lock_repository_as_another_process(&fixture.repo.path().join(".git"), "bd push auth").unwrap();
  let (_, started) = fixture.run_restart(fixture.restart(vec![PlannedEffect::RestartRobot { robot_id: "r1".to_owned() }])).await;
  assert_eq!(statuses(&started), [&EffectStatus::Done]);
  assert_eq!(fixture.fake.restarts.lock().unwrap().clone(), ["r1"]);
  assert_eq!(fixture.canceled(), [] as [String; 0]);
}

/// A cancel of a robot that finished, as the facts of the session or its status since say, cancels nothing: the robot
/// is answered as it ended (`already_finished`), and only one that finished since it was listed goes into the facts;
/// a running one is canceled. The plan of a cancel is the cancel, of whatever the robot does by then
#[tokio::test(flavor = "multi_thread")]
async fn a_cancel_of_a_robot_that_finished_is_as_it_ended_and_cancels_nothing() {
  let fixture = fixture().await;
  let patronus = fixture.services.patronus.as_deref().expect("the fake Patronus");
  let written = Mutex::new(Vec::new());
  let write = |robot: PatronusRobot| {
    written.lock().unwrap().push(format!("{} {:?}", robot.id, robot.status));
    std::future::ready(None::<Version>)
  };
  let failed = robot("r1", Some("alpha"), PatronusRobotStatus::Failure, 1);
  let canceled = cancel(patronus, "test", std::slice::from_ref(&failed), "r1", &write).await.unwrap();
  assert_eq!((canceled.robot, canceled.already_finished), (failed, true));
  assert!(fixture.fake.status_calls.lock().unwrap().is_empty(), "the facts say it finished: nothing is asked");

  let running = robot("r1", Some("alpha"), PatronusRobotStatus::Running, 1);
  fixture.fake.state_only.store(true, std::sync::atomic::Ordering::SeqCst);
  *fixture.fake.robots.lock().unwrap() = Some(vec![patronus::fake_patronus::robot_json("r1", "FAILURE", ALPHA, "2026-10-14T09:30:00Z")]);
  let finished = cancel(patronus, "test", std::slice::from_ref(&running), "r1", &write).await.unwrap();
  assert_eq!((finished.robot.status, finished.already_finished), (PatronusRobotStatus::Failure, true));
  assert!(fixture.canceled().is_empty(), "a finished robot is not canceled");
  assert_eq!(written.lock().unwrap().clone(), ["r1 Failure"]);

  *fixture.fake.robots.lock().unwrap() = Some(vec![patronus::fake_patronus::robot_json("r1", "RUNNING", ALPHA, "2026-10-14T09:30:00Z")]);
  let asked = cancel(patronus, "test", std::slice::from_ref(&running), "r1", &write).await.unwrap();
  assert!(!asked.already_finished);
  assert_eq!(fixture.canceled(), ["r1"]);

  let plan = cancel_plan(&[running], "r1").unwrap();
  assert_eq!(plan.effects, [PlannedEffect::CancelRobot { robot_id: "r1".to_owned() }]);
  assert_eq!(cancel_plan(&[], "r1"), None);
}

/// The operation that runs a request of the registry, as it is in the session
pub(crate) fn running(id: &str, request: &crate::registry::OperationRequest) -> RunningOperation {
  RunningOperation {
    id: OperationId(id.to_owned()),
    intent: request.intent.clone(),
    resources: request.resources.clone(),
    step: request.step.clone(),
    origin: Origin::ThisProcess,
    moves: request.moves.clone(),
    joined: 0,
  }
}

/// A second restart of a robot joins the first (a double click, the same item in a toast), and its cancel waits for it,
/// as does a Safe Push of what the robot lands; the restart moves the row of its subject from the click on
#[test]
fn a_second_restart_of_a_robot_joins_the_first_and_its_cancel_and_a_start_of_its_branch_wait_for_it() {
  let sources = Sources::new(Some(TIP), true, vec![robot_on("f", PatronusRobotStatus::Failure, TIP)], RestartAction::Restart);
  let planned = sources.planned("f");
  let restart = restart_request("f", planned.as_ref());
  assert_eq!(restart.resources, [Resource::Robot { id: "f".to_owned() }, Resource::Lands { branch: "auth".to_owned() }]);
  assert_eq!(restart.step, OperationStep::Checking);
  let runs = [running("restart", &restart)];
  let joined = Admission::Join {
    op: OperationId("restart".to_owned()),
  };
  assert_eq!(admit(&runs, &restart.intent, &restart.resources), joined);
  let queued = Admission::Queue {
    op: OperationId("restart".to_owned()),
  };
  let cancel = cancel_request("f");
  assert_eq!(admit(&runs, &cancel.intent, &cancel.resources), queued);
  let land = domain_types::fixtures::land_intent("auth", PatronusPushMode::RebaseAutosquash);
  assert_eq!(admit(&runs, &land, &[Resource::Lands { branch: "auth".to_owned() }]), queued);
  // the cancel of another robot runs beside it
  let other = cancel_request("other");
  assert_eq!(admit(&runs, &other.intent, &other.resources), Admission::Run);
}

/// A retarget of a pull request holds its base: a review request of the same pull request, which holds its reviewers,
/// runs beside it, and a second retarget of it joins the first
#[test]
fn a_retarget_during_a_review_request_of_the_same_pull_request_runs_beside_it() {
  let review = crate::registry::OperationRequest {
    intent: Intent::RequestReview {
      subject: Subject::branch("auth"),
      reviewers: vec!["alice".to_owned()],
      push_first: false,
    },
    resources: vec![Resource::PullRequest {
      number: 7,
      field: PullRequestField::Reviewers,
    }],
    step: OperationStep::Writing,
    moves: Vec::new(),
  };
  let retarget = retarget_request(7);
  let runs = [running("review", &review)];
  assert_eq!(admit(&runs, &retarget.intent, &retarget.resources), Admission::Run);
  let both = [running("review", &review), running("retarget", &retarget)];
  let joined = Admission::Join {
    op: OperationId("retarget".to_owned()),
  };
  assert_eq!(admit(&both, &retarget.intent, &retarget.resources), joined);
}

/// The retarget the user asks of a pull request goes from its base into the target: of an open one into a virtual
/// branch or another base; nothing of one into the target already; a merged one is refused, a closed one or one the
/// session doesn't list has no plan. The plan is planned on the pull request as listed
#[test]
fn a_retarget_moves_an_open_pull_request_into_the_target_and_nothing_else() {
  let into_parent = domain_types::pull_request::PullRequest {
    base_ref_name: "user/virtual/base".to_owned(),
    ..pull_request("auth", 7)
  };
  let mut listed = PullRequestsView::default();
  listed.pull_requests.insert("auth".to_owned(), into_parent);
  let plan = retarget_plan(&listed, 7, "master").unwrap();
  assert_eq!(plan.intent, Intent::Retarget { number: 7 });
  assert_eq!(
    plan.effects,
    [PlannedEffect::Retarget {
      branch: "auth".to_owned(),
      number: 7,
      from: "user/virtual/base".to_owned(),
      to: "master".to_owned(),
    }]
  );
  assert_eq!(plan.blockers, [] as [Reason; 0]);
  let mut moved = listed.clone();
  moved.pull_requests.get_mut("auth").unwrap().base_ref_name = "release".to_owned();
  assert_ne!(retarget_plan(&moved, 7, "master").unwrap().fingerprint, plan.fingerprint);

  listed.pull_requests.insert("auth".to_owned(), pull_request("auth", 7));
  assert!(retarget_plan(&listed, 7, "master").unwrap().effects.is_empty(), "it goes into the target already");
  listed.pull_requests.get_mut("auth").unwrap().state = PullRequestState::Merged;
  assert_eq!(retarget_plan(&listed, 7, "master").unwrap().blockers, [Reason::PullRequestMerged { number: 7 }]);
  listed.pull_requests.get_mut("auth").unwrap().state = PullRequestState::Closed;
  assert_eq!(retarget_plan(&listed, 7, "master"), None);
  assert_eq!(retarget_plan(&listed, 8, "master"), None);
}
