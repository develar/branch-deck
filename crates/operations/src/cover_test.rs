//! What an operation holds once admitted covers the plan it decided then (`engine::Holding::cover`): the rule alone, the
//! builders of the admissions, and the runs of a push, a start, a pull request and a close whose plan needs more than
//! their admission held

use crate::engine::{Cover, Holding, land_request, remote_holds, run};
use crate::events::NoEvents;
use crate::land_execute::{Accepted, Planned};
use crate::push::push_test::{Fixture, OFFLINE, Services, fixture, on_space, test_host};
use crate::registry::registry_test::{operations_until, sessions};
use crate::registry::{Answer, OperationRequest, Registry, RegistryFiles, RunningGuard};
use crate::witness::Witness;
use domain_types::error::{ApiError, ErrorCode, HolderTool};
use domain_types::fixtures::push_intent;
use domain_types::landing::{Land, PlannedEffect, Subject};
use domain_types::operation::{EffectStatus, Fingerprint, Intent, OperationStep, Plan, PullRequestField, Push, PushScope, Resource, RunningOperation};
use domain_types::patronus::PatronusPushMode;
use domain_types::session::SyncReason;
use pretty_assertions::assert_eq;
use repository_model::session::{OneClock, SessionKey, Sessions};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::clock::TestClock;

fn reference(name: &str) -> Resource {
  Resource::Ref { name: name.to_owned() }
}

fn base(number: u32) -> Resource {
  Resource::PullRequest {
    number,
    field: PullRequestField::Base,
  }
}

/// `body` within a minute
async fn soon<T>(what: &str, body: impl Future<Output = T>) -> T {
  tokio::time::timeout(Duration::from_mins(1), body)
    .await
    .unwrap_or_else(|_elapsed| panic!("{what} within a minute"))
}

/// An operation of the test, `name`, that holds `resources` and runs until the test opens it
struct Gate {
  intent: Intent,
  open: tokio::sync::oneshot::Sender<()>,
  answer: tokio::task::JoinHandle<Answer>,
}

impl Gate {
  /// Admitted in the session of `key` of `registry`, and running once this answers
  async fn running(registry: &Registry, key: &SessionKey, name: &str, resources: Vec<Resource>) -> Self {
    let intent = push_intent(name);
    let request = OperationRequest {
      intent: intent.clone(),
      resources,
      step: OperationStep::Checking,
      moves: Vec::new(),
    };
    let (open, opened) = tokio::sync::oneshot::channel();
    let answer = tokio::spawn({
      let (registry, key) = (registry.clone(), key.clone());
      async move {
        run(&registry, &key, request, Arc::new(NoEvents), async |_guard| {
          let _ = opened.await;
          Err(ApiError::new(ErrorCode::Canceled, "the operation of the test ends"))
        })
        .await
      }
    });
    let gate = Self { intent, open, answer };
    let shown = gate.intent.clone();
    soon(
      "the operation of the test runs",
      operations_until(registry.sessions(), key, move |operations| {
        operations
          .iter()
          .any(|operation| operation.intent == shown && !matches!(operation.step, OperationStep::Queued { .. }))
      }),
    )
    .await;
    gate
  }

  /// It ends, and is gone from the view
  async fn end(self) {
    let _ = self.open.send(());
    assert_eq!(self.answer.await.unwrap().unwrap_err().code, ErrorCode::Canceled);
  }
}

/// The operations of the view of `key` once the operation of `intent` waits behind the operation of the test `gate`
async fn until_behind(registry: &Registry, key: &SessionKey, intent: &Intent, gate: &Gate) -> Vec<RunningOperation> {
  let (intent, gated) = (intent.clone(), gate.intent.clone());
  soon(
    "the operation waits behind the operation of the test",
    operations_until(registry.sessions(), key, move |operations| {
      let Some(holder) = operations.iter().find(|operation| operation.intent == gated) else {
        return false;
      };
      operations
        .iter()
        .any(|operation| operation.intent == intent && operation.step == OperationStep::Queued { behind: holder.id.clone() })
    }),
  )
  .await
}

/// The resources the operation of `intent` holds in `operations`
fn held_by(operations: &[RunningOperation], intent: &Intent) -> Vec<Resource> {
  operations.iter().find(|operation| operation.intent == *intent).unwrap().resources.clone()
}

/// A plan decided once admitted runs on what the admission held where it needs no more (HEAD aside: the wait for facts
/// waited for the rewrites before it); the first time it needs more, the operation holds that (the view shows it) and
/// decides again; the second time it is refused `plan_changed` before its first effect
#[tokio::test]
async fn a_plan_that_needs_more_holds_it_once_and_is_refused_the_second_time() {
  let sessions = sessions();
  let registry = Registry::new(Arc::clone(&sessions), RegistryFiles::none());
  let key = SessionKey {
    worktree: "/repo".to_owned(),
    prefix: "user".to_owned(),
  };
  let intent = push_intent("auth");
  let request = OperationRequest {
    intent: intent.clone(),
    resources: vec![reference("user/virtual/auth")],
    step: OperationStep::Checking,
    moves: Vec::new(),
  };
  let mut holding = Holding::new(request.resources.clone());
  let answer = run(&registry, &key, request, Arc::new(NoEvents), async |guard: &RunningGuard| -> Answer {
    let held = [reference("user/virtual/auth"), Resource::Head];
    assert_eq!(holding.cover(guard, &held, &intent).await, Ok(Cover::Held));
    let more = [reference("user/virtual/auth"), reference("user/virtual/core")];
    assert_eq!(holding.cover(guard, &more, &intent).await, Ok(Cover::Again { waited: false }));
    let shown = operations_until(&sessions, &key, |operations| operations.len() == 1).await;
    assert_eq!(shown[0].resources, more);
    assert_eq!(holding.cover(guard, &more, &intent).await, Ok(Cover::Held));
    let refused = holding.cover(guard, &[reference("user/virtual/login")], &intent).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::PlanChanged);
    Err(refused)
  })
  .await;
  assert_eq!(answer.unwrap_err().code, ErrorCode::PlanChanged);
}

/// A plan of `intent` of `subject` with `effects`, landing `lands`, and the open pull requests its link links
fn planned(intent: Intent, subject: Subject, lands: &[&str], effects: Vec<PlannedEffect>, linked: &[(&str, u32)]) -> Planned {
  Planned {
    plan: Plan {
      intent,
      fingerprint: Fingerprint(String::new()),
      lands: lands.iter().map(|branch| (*branch).to_owned()).collect(),
      effects,
      confirmations: Vec::new(),
      blockers: Vec::new(),
      waits: Vec::new(),
      alternatives: None,
      pull_request: None,
      approvals_skipped_by: None,
    },
    witness: Witness::default(),
    pushes: Vec::new(),
    source_ref: String::new(),
    merge_request: None,
    notes: Vec::new(),
    subject,
    mode: PatronusPushMode::RebaseAutosquash,
    stack_pull_requests: linked
      .iter()
      .map(|(branch, number)| ((*branch).to_owned(), domain_types::fixtures::pull_request(branch, *number)))
      .collect::<BTreeMap<_, _>>(),
  }
}

/// What the effects on the remote hold is the same in the admission of every family (`remote_holds`): the base a retarget
/// moves, the bases of the open pull requests of the chain a link brings in line, the robot a cancel cancels, the robot a
/// restart restarts with what it lands (the branches of the plan, else its subject); and the admission of a start, a
/// push, a pull request and a restart holds them
#[test]
fn the_admission_of_every_family_holds_what_its_effects_on_the_remote_hold() {
  let effects = vec![
    PlannedEffect::Retarget {
      branch: "auth".to_owned(),
      number: 5,
      from: "user/virtual/core".to_owned(),
      to: "master".to_owned(),
    },
    PlannedEffect::CancelRobot { robot_id: "r1".to_owned() },
    PlannedEffect::RestartRobot { robot_id: "r2".to_owned() },
    PlannedEffect::LinkStack {
      chain: vec!["core".to_owned(), "auth".to_owned()],
      join: false,
    },
  ];
  let held = [
    base(5),
    Resource::Robot { id: "r1".to_owned() },
    Resource::Robot { id: "r2".to_owned() },
    Resource::Lands { branch: "auth".to_owned() },
    base(4),
    base(6),
  ];
  let auth = Subject::branch("auth");
  let push = Push {
    subject: auth.clone(),
    scope: PushScope::Alone,
    expect_remote: None,
    force: false,
    restart_running: true,
    rebased: false,
  };
  let of_push = planned(Intent::Push(push.clone()), auth.clone(), &[], effects.clone(), &[("core", 4), ("auth", 6), ("other", 9)]);
  assert_eq!(remote_holds(&of_push), [&held[..4], &[base(4), base(6)]].concat());
  let land = Land {
    subject: auth.clone(),
    mode: PatronusPushMode::RebaseAutosquash,
    ..match domain_types::fixtures::land_intent("auth", PatronusPushMode::RebaseAutosquash) {
      Intent::Land(land) => land,
      _ => unreachable!(),
    }
  };
  let of_land = planned(Intent::Land(land.clone()), auth.clone(), &["auth"], effects.clone(), &[("core", 4), ("auth", 6)]);
  let restart = Intent::RestartRobot { robot_id: "r2".to_owned() };
  let of_restart = planned(restart, auth.clone(), &["auth"], effects.clone(), &[("core", 4), ("auth", 6)]);
  let create = Intent::LinkStack { branch: "auth".to_owned() };
  let of_link = planned(create.clone(), auth, &[], effects, &[("core", 4), ("auth", 6)]);
  for (family, request) in [
    ("start", land_request("user", &land, Some(&of_land))),
    ("push", crate::push::push_request("user", &push, Some(&of_push))),
    ("restart", crate::robot::restart_request("r2", Some(&of_restart))),
    ("link", crate::pull_request::pull_request_request(&create, Some(&of_link))),
  ] {
    let missing: Vec<&Resource> = held.iter().filter(|resource| !request.resources.contains(resource)).collect();
    assert_eq!(missing, Vec::<&Resource>::new(), "{family}: {:?}", request.resources);
  }
}

/// alpha and beta (built on alpha) of the fixture, `more` committed besides, synced, then the branches `pushed` on the
/// remote and synced again: their remote status is up to date
async fn pushed_fixture(more: &[(&str, &str)], pushed: &[&str]) -> Fixture {
  let fixture = fixture(false).await;
  for (subject, file) in more {
    fixture.repo.create_commit(subject, file, "more\n");
  }
  let sessions = fixture.registry.sessions();
  sessions.sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  for branch in pushed {
    fixture.repo.git(&["push", "-q", "origin", &format!("test/virtual/{branch}")]);
  }
  sessions.sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  fixture
}

impl Fixture {
  /// The remote branch of `branch` is gone (another clone deleted it), and the session synced since: the branch has
  /// work for the remote again
  async fn delete_remote(&self, branch: &str) {
    self.repo.git(&["push", "-q", "origin", "--delete", &format!("test/virtual/{branch}")]);
    self.registry.sessions().sync(&self.key, SyncReason::Manual).wait().await.unwrap();
  }
}

/// The push of `subject` with the branches it is built on
fn with_ancestors(subject: &str) -> Push {
  Push {
    subject: Subject::branch(subject),
    scope: PushScope::WithAncestors,
    expect_remote: None,
    force: false,
    restart_running: false,
    rebased: false,
  }
}

/// What a test runs of its fixture: an operation of the engine on its own task, admitted while a rewrite of the test
/// runs, which it waits behind
struct Behind {
  intent: Intent,
  rewrite: Gate,
  answer: tokio::task::JoinHandle<Answer>,
}

impl Fixture {
  /// The names of the branches `planned` pushes
  fn names(planned: &Planned) -> Vec<&str> {
    planned.pushes.iter().map(|pushed| pushed.name.as_str()).collect()
  }

  /// `intent`, accepted on the plan of `fingerprint`, run once a rewrite of the test runs (`execute`), and waiting behind
  /// it
  async fn behind_a_rewrite(&self, intent: Intent, fingerprint: &Fingerprint, execute: impl FnOnce(Registry, SessionKey, Accepted) -> tokio::task::JoinHandle<Answer>) -> Behind {
    let rewrite = Gate::running(&self.registry, &self.key, "a-rewrite", vec![Resource::Head]).await;
    let accepted = Accepted {
      fingerprint: fingerprint.clone(),
      confirmations: Vec::new(),
    };
    let answer = execute(self.registry.clone(), self.key.clone(), accepted);
    until_behind(&self.registry, &self.key, &intent, &rewrite).await;
    Behind { intent, rewrite, answer }
  }

  /// The push `push` run over `services` behind a rewrite of the test (`behind_a_rewrite`), its plan now sending `sends`
  async fn push_behind_a_rewrite(&self, push: Push, sends: &[&str], services: Arc<Services>) -> Behind {
    let planned = crate::push::prepare_push(self.registry.sessions(), &self.key, &push).await.unwrap();
    assert_eq!(Self::names(&planned), sends);
    self
      .behind_a_rewrite(Intent::Push(push.clone()), &planned.plan.fingerprint, move |registry, key, accepted| {
        tokio::spawn(async move { crate::push::execute_push(&registry, &key, push, accepted, services.as_ref(), Arc::new(NoEvents)).await })
      })
      .await
  }

  /// The remote branch of `branch` goes (`delete_remote`), and a push of the test that holds its ref runs
  async fn pushed_by_another(&self, branch: &str) -> Gate {
    self.delete_remote(branch).await;
    Gate::running(&self.registry, &self.key, &format!("{branch}-push"), vec![reference(&format!("test/virtual/{branch}"))]).await
  }
}

/// The push of beta with its ancestors is admitted with the ref of beta alone, alpha on the remote; while it waits for a
/// rewrite before it, the remote branch of alpha goes, and a push of alpha runs. Decided again once admitted the push
/// sends alpha too: it holds the ref of alpha first, after the push that holds it, and sends both once that ended
#[tokio::test(flavor = "multi_thread")]
async fn a_push_whose_plan_once_admitted_sends_a_branch_more_holds_it_after_the_operation_that_holds_it() {
  let fixture = pushed_fixture(&[], &["alpha", "beta"]).await;
  fixture.repo.create_commit("(alpha/beta) Beta, part 2", "beta-2.txt", "beta 2\n");
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  let Behind { intent, rewrite, answer: pushing } = fixture.push_behind_a_rewrite(with_ancestors("beta"), &["beta"], Arc::new(OFFLINE)).await;
  assert_eq!(held_by(&fixture.operations().await, &intent), [reference("test/virtual/beta")]);

  let alpha = fixture.pushed_by_another("alpha").await;
  rewrite.end().await;
  let behind_alpha = until_behind(&fixture.registry, &fixture.key, &intent, &alpha).await;
  assert_eq!(held_by(&behind_alpha, &intent), [reference("test/virtual/beta"), reference("test/virtual/alpha")]);
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/beta", "nothing pushed while it waits");

  alpha.end().await;
  let outcome = soon("the push", pushing).await.unwrap().unwrap();
  assert!(
    outcome
      .effects
      .iter()
      .any(|record| matches!(&record.effect, PlannedEffect::Push { branches, .. } if branches == &["alpha", "beta"]) && record.status == EffectStatus::Done),
    "{outcome:?}"
  );
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// The push of gamma (built on beta, on alpha) holds the ref of beta once its plan sends beta too; while it waits for
/// the push that holds beta, the remote branch of alpha goes as well: decided again, it would send alpha, which it held
/// not, a second time it needs more, so it is refused `plan_changed` and pushes nothing
#[tokio::test(flavor = "multi_thread")]
async fn a_push_whose_plan_needs_more_a_second_time_is_refused_before_its_first_effect() {
  let fixture = pushed_fixture(&[("(beta/gamma) Gamma", "gamma.txt")], &["alpha", "beta"]).await;
  let Behind { intent, rewrite, answer: pushing } = fixture.push_behind_a_rewrite(with_ancestors("gamma"), &["gamma"], Arc::new(OFFLINE)).await;
  let beta = fixture.pushed_by_another("beta").await;
  rewrite.end().await;
  until_behind(&fixture.registry, &fixture.key, &intent, &beta).await;
  fixture.delete_remote("alpha").await;
  beta.end().await;
  let refused = soon("the push", pushing).await.unwrap().unwrap_err();
  assert_eq!(refused.code, ErrorCode::PlanChanged, "{refused:?}");
  assert_eq!(fixture.remote_branches(), "", "nothing pushed");
}

/// The push of gamma with its ancestors, admitted with gamma's ref, holds beta's ref once beta's remote branch went,
/// after a start of beta that holds it; Patronus lists the Safe Push of beta that start started. Decided again after
/// that wait, the push reads the robots again and is decided on them: beta, which its Safe Push holds, is not pushed
#[tokio::test(flavor = "multi_thread")]
async fn a_push_that_waited_for_what_it_needs_more_is_decided_on_the_robots_as_patronus_has_them_then() {
  use patronus::fake_patronus::robot_json;
  use sync_utils::timestamp::{format_utc_timestamp, unix_now};
  let (fake, services) = on_space().await;
  *fake.robots.lock().unwrap() = Some(Vec::new());
  let fixture = pushed_fixture(&[("(beta/gamma) Gamma", "gamma.txt")], &["alpha", "beta"]).await;
  let Behind { intent, rewrite, answer: pushing } = fixture.push_behind_a_rewrite(with_ancestors("gamma"), &["gamma"], Arc::new(services)).await;
  fixture.delete_remote("beta").await;
  let start = Gate::running(
    &fixture.registry,
    &fixture.key,
    "beta-start",
    vec![reference("test/virtual/beta"), Resource::Lands { branch: "beta".to_owned() }],
  )
  .await;
  rewrite.end().await;
  until_behind(&fixture.registry, &fixture.key, &intent, &start).await;
  let mut listed = robot_json("beta-safe-push", "RUNNING", "refs/heads/test/virtual/beta", &format_utc_timestamp(unix_now()));
  listed["sourceRevision"] = serde_json::json!(fixture.repo.rev_parse("refs/heads/test/virtual/beta").unwrap());
  *fake.robots.lock().unwrap() = Some(vec![listed]);
  start.end().await;
  let answer = soon("the push", pushing).await.unwrap();
  assert!(!fixture.remote_branches().contains("beta"), "beta pushed under its running Safe Push: {answer:?}");
}

/// A start of gamma has no plan as it is asked (the last sync has no gamma); gamma is committed and synced while it waits
/// behind a rewrite, and decided once admitted it lands by a robot of Patronus. It runs over the host of that route:
/// off Space that host refuses it (`not_on_space`) before anything is pushed, where the host that needs no Patronus,
/// which planned it, would push gamma and fail at the start of its robot
#[tokio::test(flavor = "multi_thread")]
async fn a_start_whose_plan_once_admitted_lands_by_a_robot_runs_over_the_host_of_patronus() {
  let fixture = fixture(true).await;
  let Intent::Land(land) = domain_types::fixtures::land_intent("gamma", PatronusPushMode::RebaseAutosquash) else {
    unreachable!()
  };
  let sessions = fixture.registry.sessions();
  assert_eq!(crate::land::prepare(sessions, &fixture.key, land.clone()).await, None, "no gamma yet");
  let asked = land.clone();
  let Behind { rewrite, answer, .. } = fixture
    .behind_a_rewrite(Intent::Land(land.clone()), &Fingerprint(String::new()), move |registry, key, accepted| {
      tokio::spawn(async move { crate::engine::execute_land(&registry, &key, asked, accepted, OFFLINE.hosts(), Arc::new(NoEvents)).await })
    })
    .await;
  fixture.repo.create_commit("(gamma) Gamma", "gamma.txt", "gamma\n");
  sessions.sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
  let planned = crate::land::prepare(sessions, &fixture.key, land).await.expect("the sync has gamma");
  assert!(!crate::engine::pushes_into_target(&planned), "a robot of Patronus lands it");
  rewrite.end().await;
  let refused = soon("the start", answer).await.unwrap().unwrap_err();
  assert_eq!(refused.code, ErrorCode::NotOnSpace, "{refused:?}");
  assert_eq!(fixture.remote_branches(), "", "nothing pushed");
}

/// A start of gamma has no plan as it is asked (the last sync has no gamma) and is admitted holding the branch alone; while it
/// waits for a rewrite before it, gamma is committed and synced, and a landing into the target runs. Decided again once
/// admitted it lands gamma into the target: it holds what that reserves (the branch, the remote-tracking ref of the
/// target) after the landing that holds the target, and pushes once that ended
#[tokio::test(flavor = "multi_thread")]
async fn a_start_whose_plan_once_admitted_reserves_more_holds_it_after_the_operation_that_holds_it() {
  let (repo, registry, key) = crate::update::update_test::push_landing_fixture().await;
  let Intent::Land(land) = domain_types::fixtures::land_intent("gamma", PatronusPushMode::RebaseAutosquash) else {
    unreachable!()
  };
  // what the start of gamma asks to accept, read on a commit of gamma the test takes back: the user accepts it at the
  // click of the start
  repo.create_commit("(gamma) Gamma", "gamma.txt", "gamma\n");
  registry.sessions().sync(&key, SyncReason::Manual).wait().await.unwrap();
  let confirmations = soon("the plan of gamma", async {
    loop {
      if let Some(plan) = crate::land::prepare(registry.sessions(), &key, land.clone()).await
        && plan.waits.is_empty()
      {
        break plan.confirmations;
      }
      tokio::task::yield_now().await;
    }
  })
  .await;
  repo.git(&["reset", "-q", "--hard", "HEAD~1"]);
  registry.sessions().sync(&key, SyncReason::Manual).wait().await.unwrap();
  assert_eq!(crate::land::prepare(registry.sessions(), &key, land.clone()).await, None);
  let rewrite = Gate::running(&registry, &key, "a-rewrite", vec![Resource::Head]).await;
  let accepted = Accepted {
    fingerprint: Fingerprint(String::new()),
    confirmations,
  };
  let starting = tokio::spawn({
    let (registry, key, land) = (registry.clone(), key.clone(), land.clone());
    async move { crate::engine::execute_land(&registry, &key, land, accepted, OFFLINE.hosts(), Arc::new(NoEvents)).await }
  });
  let intent = Intent::Land(land);
  let waiting = until_behind(&registry, &key, &intent, &rewrite).await;
  assert_eq!(held_by(&waiting, &intent), [Resource::Lands { branch: "gamma".to_owned() }]);

  repo.create_commit("(gamma) Gamma", "gamma.txt", "gamma\n");
  registry.sessions().sync(&key, SyncReason::Manual).wait().await.unwrap();
  let target = reference("refs/remotes/origin/master");
  let landing = Gate::running(&registry, &key, "a-landing", vec![target.clone()]).await;
  rewrite.end().await;
  let behind = until_behind(&registry, &key, &intent, &landing).await;
  let held = held_by(&behind, &intent);
  assert!(held.contains(&target) && held.contains(&Resource::Lands { branch: "gamma".to_owned() }), "{held:?}");
  let master = || repo.git(&["--git-dir", ".git/origin.git", "log", "--format=%s", "master"]);
  assert_eq!(master(), "Initial commit", "nothing landed while it waits");

  landing.end().await;
  let outcome = soon("the start", starting).await.unwrap().unwrap();
  assert!(
    outcome
      .effects
      .iter()
      .any(|record| matches!(record.effect, PlannedEffect::PushBaseline { .. }) && record.status == EffectStatus::Done),
    "{outcome:?}"
  );
  assert_eq!(master(), "Gamma\nInitial commit");
}

/// The creation of the pull request of beta is admitted with the refs of its push of beta, alpha on the remote; while it
/// waits for a rewrite before it, the remote branch of alpha goes, and a push of alpha runs. Decided again once admitted
/// its push sends alpha too: it holds the ref of alpha after the push that holds it, and pushes once that ended
#[tokio::test(flavor = "multi_thread")]
async fn a_pull_request_whose_plan_once_admitted_pushes_more_holds_it_after_the_operation_that_holds_it() {
  use domain_types::operation::CreatePullRequest;
  let fixture = pushed_fixture(&[], &["alpha"]).await;
  let intent = Intent::CreatePullRequest(CreatePullRequest {
    subject: Subject::branch("beta"),
    title: "Beta".to_owned(),
    body: String::new(),
    assign_self: false,
    reviewers: Vec::new(),
    on_github: false,
  });
  let planned = crate::pull_request::prepare_planned(fixture.registry.sessions(), &fixture.key, &intent).await.unwrap();
  assert_eq!(Fixture::names(&planned), ["beta"]);
  let asked = intent.clone();
  let Behind {
    intent,
    rewrite,
    answer: creating,
  } = fixture
    .behind_a_rewrite(intent, &planned.plan.fingerprint, move |registry, key, accepted| {
      tokio::spawn(async move { crate::pull_request::execute_pull_request(&registry, &key, asked, accepted, &OFFLINE, Arc::new(NoEvents)).await })
    })
    .await;

  let alpha = fixture.pushed_by_another("alpha").await;
  rewrite.end().await;
  let behind = until_behind(&fixture.registry, &fixture.key, &intent, &alpha).await;
  assert!(held_by(&behind, &intent).contains(&reference("test/virtual/alpha")));
  assert_eq!(fixture.remote_branches(), "", "nothing pushed while it waits");

  alpha.end().await;
  let outcome = soon("the creation", creating).await.unwrap().unwrap();
  assert!(
    outcome
      .effects
      .iter()
      .any(|record| matches!(&record.effect, PlannedEffect::Push { branches, .. } if branches.contains(&"alpha".to_owned())) && record.status == EffectStatus::Done),
    "{outcome:?}"
  );
  assert_eq!(fixture.remote_branches(), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// The closes of a landing have no plan as they are asked (the session told no landing yet) and are admitted holding
/// nothing; while their admission reads the registry of the repository, a push of alpha and beta into the target tells
/// the landing that closes #7 of alpha, and an operation that holds its state runs. Decided again once admitted the
/// closes hold the state of #7 after that one, and close once it ended
#[tokio::test(flavor = "multi_thread")]
async fn a_close_whose_plan_once_admitted_closes_more_holds_it_after_the_operation_that_holds_it() {
  use domain_types::landing::{BaselinePush, LandingKey};
  use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestsResult};
  use repository_model::facts::FactResult;
  use std::sync::Mutex;
  use std::sync::mpsc;
  let repo = test_utils::git_test_utils::stack_ahead_of_bare_origin();
  let sessions = Arc::new(Sessions::new(
    tokio::runtime::Handle::current(),
    test_host(),
    Arc::new(OneClock(Arc::new(TestClock::new()))),
  ));
  let key = SessionKey {
    worktree: repo.path_str().to_owned(),
    prefix: "test".to_owned(),
  };
  sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
  let mut listed = PullRequestsResult::unavailable(PullRequestAvailability::Available {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
  });
  // its head is the tip of alpha, into another base than the target: the landing of alpha closes it, GitHub does not
  let alpha = PullRequest {
    head_ref_name: "test/virtual/alpha".to_owned(),
    head_ref_oid: repo.rev_parse("refs/heads/test/virtual/alpha").unwrap(),
    base_ref_name: "release".to_owned(),
    ..domain_types::fixtures::pull_request("alpha", 7)
  };
  listed.pull_requests.insert("alpha".to_owned(), alpha);
  let ticket = sessions.begin(&key).await.unwrap();
  sessions.apply_facts(&key, ticket, FactResult::PullRequests(listed)).await.unwrap();
  // the first admission stops in the read of the directory of the registry until the test lets it go
  let (entered, entering) = mpsc::channel::<()>();
  let (let_go, held) = mpsc::channel::<()>();
  let first = Mutex::new(Some((entered, held)));
  let git = repo.path().join(".git");
  let files = RegistryFiles::new(HolderTool::App, move |_key| {
    let stop = first.lock().unwrap().take();
    if let Some((entered, held)) = stop {
      entered.send(()).unwrap();
      held.recv().unwrap();
    }
    Some(git.clone())
  })
  .unflushed();
  let registry = Registry::new(Arc::clone(&sessions), files);
  let intent = Intent::CloseLandedPullRequests {
    landing: LandingKey::Told { id: "landing-1".to_owned() },
  };
  assert_eq!(crate::close::prepare_close(&sessions, &key, &intent).await, None);
  let closing = tokio::spawn({
    let (registry, key, intent) = (registry.clone(), key.clone(), intent.clone());
    let accepted = Accepted {
      fingerprint: Fingerprint(String::new()),
      confirmations: Vec::new(),
    };
    async move { crate::close::execute_close(&registry, &key, intent, accepted, &OFFLINE, Arc::new(NoEvents)).await }
  });
  tokio::task::spawn_blocking(move || entering.recv().unwrap()).await.unwrap();

  let pushed = |tip: &str| BaselinePush {
    tip: tip.to_owned(),
    upstream: "origin/master".to_owned(),
  };
  sessions
    .tell_baseline_push(&key, vec![("alpha".to_owned(), pushed("alpha-copy")), ("beta".to_owned(), pushed("beta-copy"))], None)
    .await
    .unwrap();
  let told = crate::close::prepare_close(&sessions, &key, &intent).await.expect("the push told a landing");
  assert!(
    told.effects.iter().any(|effect| matches!(effect, PlannedEffect::ClosePullRequest { number: 7, .. })),
    "{told:?}"
  );
  let state = Resource::PullRequest {
    number: 7,
    field: PullRequestField::State,
  };
  let other = Gate::running(&registry, &key, "a-close", vec![state.clone()]).await;
  let_go.send(()).unwrap();
  let behind = until_behind(&registry, &key, &intent, &other).await;
  assert_eq!(held_by(&behind, &intent), [state]);

  other.end().await;
  let outcome = soon("the closes", closing).await.unwrap().unwrap();
  // off GitHub its close fails as its effect: it ran, after the operation it waited for
  assert!(
    outcome
      .effects
      .iter()
      .any(|record| matches!(record.effect, PlannedEffect::ClosePullRequest { number: 7, .. }) && matches!(record.status, EffectStatus::Failed { .. })),
    "{outcome:?}"
  );
}
