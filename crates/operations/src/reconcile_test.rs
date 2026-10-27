use crate::engine::request_of;
use crate::land_execute::land_execute_test::{ALPHA, Fixture as StartFixture, fixture as start_fixture, statuses};
use crate::land_execute::{Accepted, Steps, execute};
use crate::push::push_test::Services;
use crate::reconcile::{Listed, ReconcileSources, Reconciled, SETTLED_AFTER, SettleWith, admit_settled, by_robots, reconcile, robot_of};
use crate::registry::{ListedRobots, Registry, RegistryFiles};
use crate::registry_file::{PendingStart, ProcessRegistry, Record, RecordStatus};
use domain_types::error::{ErrorCode, ErrorDetails, HolderTool};
use domain_types::fixtures::{land_intent, robot};
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::operation::{EffectStatus, Fingerprint, Intent, OperationId, OperationResult, OperationStep};
use domain_types::patronus::{PatronusAvailability, PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult};
use domain_types::session::ViewPatch;
use github::fake_github::{FakeAnswer, start_fake_service};
use patronus::fake_patronus::robot_json;
use pretty_assertions::assert_eq;
use remote_facts::scheduler::FactsPort;
use repository_model::facts::{FactResult, RobotsRead};
use repository_model::session::{SessionKey, Subscriber, SubscriberGone};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use sync_core::github_repository::GitHubRepository;
use sync_utils::timestamp::{format_utc_timestamp, unix_now};

/// Unix seconds of the request of the starts of the tests
const SENT: u32 = 1_800_000_000;
const TIP: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// A start of `alpha` sent at `SENT` on `TIP` through `route`
fn pending(route: StartRoute) -> PendingStart {
  let pr = match route {
    StartRoute::Comment { number } => Some(number),
    StartRoute::Api { .. } => None,
  };
  PendingStart {
    source_ref: ALPHA.to_string(),
    tip: TIP.to_string(),
    route,
    pr,
    comment_id: None,
    sent_at: SENT,
    url: None,
  }
}

fn api() -> PendingStart {
  pending(StartRoute::Api { profile: None })
}

/// A robot `id` of `alpha` in `mode` started at `start_time`, on `revision`
fn alpha_robot(id: &str, mode: PatronusPushMode, start_time: u32, revision: Option<&str>) -> PatronusRobot {
  PatronusRobot {
    push_mode: mode,
    source_revision: revision.map(ToString::to_string),
    ..robot(id, Some("alpha"), PatronusRobotStatus::Running, start_time)
  }
}

/// Through the API the robot of a start is one of its virtual branch in its mode, started since it was sent (minus the
/// slack of the clocks), on its commit when Patronus names one; of a whole stack the robot of its root; of a comment the
/// robot of its pull request; the newest of them
#[test]
fn the_robot_of_a_lost_start_is_of_its_ref_and_mode_since_it_was_sent() {
  let safe_push = PatronusPushMode::RebaseAutosquash;
  let robots = vec![
    alpha_robot("before", safe_push, SENT - 600, Some(TIP)),
    alpha_robot("dry-run", PatronusPushMode::DryRun, SENT + 1, Some(TIP)),
    alpha_robot("other-commit", safe_push, SENT + 2, Some("abc123")),
    alpha_robot("unnamed-commit", safe_push, SENT + 3, None),
    alpha_robot("skewed", safe_push, SENT - 60, Some(TIP)),
  ];
  let found = |start: &PendingStart, mode, robots: &[PatronusRobot]| robot_of(start, mode, "test", robots).map(|robot| robot.id);
  assert_eq!(found(&api(), safe_push, &robots), Some("unnamed-commit".to_string()));
  assert_eq!(found(&api(), safe_push, &robots[..3]), None);
  assert_eq!(found(&api(), PatronusPushMode::DryRun, &robots), Some("dry-run".to_string()));

  let stack = PendingStart {
    source_ref: "refs/heads/test/stack/alpha".to_string(),
    ..api()
  };
  let whole = PatronusRobot {
    branch_name: None,
    stack_root: Some("alpha".to_string()),
    ..alpha_robot("whole", safe_push, SENT + 1, None)
  };
  assert_eq!(found(&stack, safe_push, &[robots[3].clone(), whole.clone()]), Some("whole".to_string()));
  assert_eq!(found(&api(), safe_push, &[whole]), None);

  let comment = pending(StartRoute::Comment { number: 7 });
  let of_seven = PatronusRobot {
    pull_request: Some(PatronusPullRequestLink {
      number: 7,
      url: "https://github.com/acme/monorepo/pull/7".to_string(),
    }),
    ..alpha_robot("of-seven", safe_push, SENT + 5, None)
  };
  assert_eq!(found(&comment, safe_push, &[robots[3].clone(), of_seven]), Some("of-seven".to_string()));
}

/// Without its robot, a start through the API is proven absent only by a list of the whole window read once it settled
/// (the time a start waits for its answer): a list of a part of it, or one read before, tells nothing; a comment is
/// never proven absent by the robots (the bot may still answer it)
#[test]
fn a_list_of_the_whole_window_proves_a_lost_start_absent_once_it_settled() {
  let safe_push = PatronusPushMode::RebaseAutosquash;
  let settled = SENT + SETTLED_AFTER;
  assert_eq!(by_robots(&api(), safe_push, "test", &[], settled, true), Reconciled::Failed);
  assert_eq!(by_robots(&api(), safe_push, "test", &[], settled - 1, true), Reconciled::Unknown);
  assert_eq!(by_robots(&api(), safe_push, "test", &[], settled, false), Reconciled::Unknown);
  let comment = pending(StartRoute::Comment { number: 7 });
  assert_eq!(by_robots(&comment, safe_push, "test", &[], settled + 600, true), Reconciled::Unknown);
  let listed = alpha_robot("listed", safe_push, SENT + 1, Some(TIP));
  assert_eq!(
    by_robots(&api(), safe_push, "test", std::slice::from_ref(&listed), SENT, false),
    Reconciled::Started(Some(Box::new(listed)))
  );
}

/// The record of a start of `alpha` whose answer was lost, sent through `start`
fn lost_record(start: PendingStart) -> Record {
  Record {
    id: OperationId("lost".to_string()),
    tool: HolderTool::Bd,
    pid: 1,
    process_started_at: None,
    host: "elsewhere".to_string(),
    started_at: SENT,
    heartbeat_at: SENT,
    session: SessionKey {
      worktree: "/repository".to_string(),
      prefix: "test".to_string(),
    },
    intent: land_intent("alpha", PatronusPushMode::RebaseAutosquash),
    resources: Vec::new(),
    step: OperationStep::Starting,
    status: RecordStatus::Unknown,
    external: Some(crate::registry_file::External { pending_start: Some(start) }),
  }
}

const COMMENTS: &str = "/repos/acme/monorepo/issues/7/comments";

/// A list of the whole window without a robot, asked at `asked_at`
const fn listed_none(asked_at: u32) -> Listed<'static> {
  Listed {
    robots: &[],
    complete: true,
    asked_at,
  }
}

/// A comment whose post lost its answer is found by its text among the comments of its pull request since it was sent,
/// a user's (the bot's quote of it is no start), and settled by the reply of the bot to it: 👍 started, 👎 failed. None
/// found once it settled: the post never arrived; one found by its id is read at once
#[tokio::test]
async fn a_lost_comment_is_found_by_its_text_and_settled_by_the_reply_of_the_bot() {
  let (fake, github) = start_fake_service("t").await;
  let repository = GitHubRepository {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  };
  let services = Services {
    patronus: None,
    github: Some((Arc::new(github), repository)),
  };
  let key = SessionKey {
    worktree: "/repository".to_string(),
    prefix: "test".to_string(),
  };
  let sources = ReconcileSources::of(&services, &key, SENT + 5).await;
  let at = |now: u32| ReconcileSources {
    patronus: None,
    github: sources.github.clone(),
    key: key.clone(),
    now,
  };
  let record = lost_record(pending(StartRoute::Comment { number: 7 }));
  let user = json!({"login": "me", "type": "User"});
  let bot = json!({"login": "patronus[bot]", "type": "Bot"});
  let created = format_utc_timestamp(SENT + 1);
  fake.answer(
    "GET",
    COMMENTS,
    FakeAnswer::ok(json!([
      {"id": 4401, "body": "/patronus merge --keep-source-branch", "user": bot, "created_at": created},
      {"id": 4402, "body": "/patronus merge --keep-source-branch\n", "user": user, "created_at": created, "html_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402"},
    ])),
  );
  fake.answer(
    "GET",
    "/repos/acme/monorepo/issues/comments/4402",
    FakeAnswer::ok(json!({"id": 4402, "reactions": {"+1": 1, "-1": 0}})),
  );
  assert_eq!(reconcile(&record, &at(SENT + 5), Some(listed_none(SENT + 5))).await, Reconciled::Started(None));
  let since = fake.requests_to("GET", COMMENTS)[0].query.clone().unwrap_or_default();
  assert!(since.contains(&format_utc_timestamp(SENT - 120).replace(':', "%3A")), "{since}");

  fake.answer(
    "GET",
    "/repos/acme/monorepo/issues/comments/4402",
    FakeAnswer::ok(json!({"id": 4402, "reactions": {"+1": 0, "-1": 1}, "created_at": created})),
  );
  fake.answer(
    "GET",
    COMMENTS,
    FakeAnswer::ok(json!([{"id": 4402, "body": "/patronus merge --keep-source-branch", "user": user, "created_at": created}])),
  );
  assert_eq!(reconcile(&record, &at(SENT + 5), Some(listed_none(SENT + 5))).await, Reconciled::Failed);

  fake.answer("GET", COMMENTS, FakeAnswer::ok(json!([])));
  assert_eq!(reconcile(&record, &at(SENT + 5), Some(listed_none(SENT + 5))).await, Reconciled::Unknown);
  assert_eq!(
    reconcile(&record, &at(SENT + SETTLED_AFTER), Some(listed_none(SENT + SETTLED_AFTER))).await,
    Reconciled::Failed
  );

  // its id known: its reactions are read, no search
  let searched = fake.requests_to("GET", COMMENTS).len();
  let mut known = pending(StartRoute::Comment { number: 7 });
  known.comment_id = Some(4402);
  fake.answer(
    "GET",
    "/repos/acme/monorepo/issues/comments/4402",
    FakeAnswer::ok(json!({"id": 4402, "reactions": {"+1": 1, "-1": 0}})),
  );
  assert_eq!(reconcile(&lost_record(known), &at(SENT + 5), Some(listed_none(SENT + 5))).await, Reconciled::Started(None));
  assert_eq!(fake.requests_to("GET", COMMENTS).len(), searched);
}

/// The admission of `fixture`'s Safe Push of alpha into `files`, as the app's (`HolderTool::App`), over the Patronus of the
/// fixture at `now`
async fn admit_again(fixture: &StartFixture, files: &ProcessRegistry, key: &SessionKey, now: u32) -> crate::reconcile::SettledAdmission {
  let planned = fixture.planned(start_effects());
  let services = Services {
    patronus: fixture.services.patronus.clone(),
    github: None,
  };
  admit_settled(
    files,
    &OperationId("again".to_string()),
    HolderTool::App,
    key,
    &request_of("test", &planned),
    Some(SettleWith::Host(&services, now)),
  )
  .await
}

/// The push of alpha and the start of its Safe Push through the API
fn start_effects() -> Vec<PlannedEffect> {
  vec![
    PlannedEffect::Push {
      branches: vec!["alpha".to_string()],
      force: false,
    },
    PlannedEffect::StartRobot {
      route: StartRoute::Api { profile: None },
    },
  ]
}

/// The connection of the start breaks off after Patronus took it: the start is unknown, sent once, and its record stays
/// in the registry as a start whose answer was lost (with what it sent: the pushed commit), which holds the Safe Push of
/// alpha for every process, its own too. A list without its robot settles nothing before the start settled; once Patronus
/// lists its robot, an admission settles it as started (the robot for the facts) and refuses itself, its plan made
/// without that robot; then the record is gone
#[tokio::test(flavor = "multi_thread")]
async fn a_start_whose_answer_was_lost_is_unknown_once_and_holds_until_patronus_lists_its_robot() {
  let fixture = start_fixture().await;
  fixture.fake.lose_start_answers.store(true, Ordering::SeqCst);
  *fixture.fake.robots.lock().unwrap() = Some(Vec::new());
  let host = StartFixture::host(fixture.planned(start_effects()));
  let files = ProcessRegistry::of(&fixture.services.git_common_dir);
  let key = SessionKey {
    worktree: fixture.repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  let entry = files
    .admit(&OperationId("op".to_string()), HolderTool::Bd, &key, &request_of("test", &host.planned))
    .unwrap();
  let steps = Steps::none().with_record(Some(entry.handle()));
  let accepted = Accepted {
    fingerprint: Fingerprint("planned".to_string()),
    confirmations: Vec::new(),
  };
  let land = match &host.planned.plan.intent {
    Intent::Land(land) => land.clone(),
    _ => unreachable!(),
  };
  let started = execute(&host, &fixture.services, &steps, OperationId("op".to_string()), &land, &accepted, false)
    .await
    .unwrap();
  assert_eq!(statuses(&started), [&EffectStatus::Done, &EffectStatus::Unknown]);
  let OperationResult::Land(result) = &started.outcome.result else {
    panic!("{:?}", started.outcome.result)
  };
  assert_eq!(result.lost.as_ref().map(|lost| lost.code), Some(ErrorCode::PatronusError));
  assert_eq!(fixture.fake.starts.lock().unwrap().len(), 1);
  drop(entry);

  let records = files.records().unwrap().unwrap();
  let pushed = fixture.remote_alpha().unwrap();
  let record = &records[0];
  let start = record.pending_start().unwrap();
  assert_eq!(
    (records.len(), record.status, start.source_ref.as_str(), start.tip.as_str()),
    (1, RecordStatus::Unknown, ALPHA, pushed.as_str())
  );
  assert_eq!(start.url.as_deref(), Some(fixture.services.patronus.as_ref().unwrap().web_url().as_str()));

  // not settled yet: busy, named as a start whose answer was lost, for the process that sent it too
  let refused = admit_again(&fixture, &files, &key, start.sent_at + 1).await.entry.unwrap_err();
  let Some(ErrorDetails::Busy { holder: Some(holder) }) = &refused.details else {
    panic!("{refused:?}")
  };
  assert_eq!((refused.code, holder.op.as_str(), holder.unknown), (ErrorCode::Busy, "op", true));
  assert!(refused.message.contains("`bd operation forget op`"), "{}", refused.message);

  let mut listed = robot_json("started-1", "RUNNING", ALPHA, &format_utc_timestamp(start.sent_at));
  listed["sourceRevision"] = json!(pushed);
  *fixture.fake.robots.lock().unwrap() = Some(vec![listed]);
  let admitted = admit_again(&fixture, &files, &key, unix_now()).await;
  let refused = admitted.entry.unwrap_err();
  assert_eq!(
    (refused.code, admitted.started.map(|robot| robot.id)),
    (ErrorCode::PlanChanged, Some("started-1".to_string()))
  );
  assert_eq!(files.records().unwrap(), Some(Vec::new()));
  assert_eq!(fixture.fake.starts.lock().unwrap().len(), 1);
}

/// A start whose answer was lost and whose robot Patronus doesn't list once it settled started nothing: the admission
/// takes its record out and goes in
#[tokio::test(flavor = "multi_thread")]
async fn a_lost_start_patronus_does_not_list_once_it_settled_leaves_and_the_admission_goes_in() {
  let fixture = start_fixture().await;
  *fixture.fake.robots.lock().unwrap() = Some(Vec::new());
  let files = ProcessRegistry::of(&fixture.services.git_common_dir);
  let key = SessionKey {
    worktree: fixture.repo.path_str().to_string(),
    prefix: "test".to_string(),
  };
  let mut record = lost_record(api());
  record.session = key.clone();
  record.resources = request_of("test", &fixture.planned(start_effects())).resources;
  files.insert_of_another_process(record).unwrap();
  let admitted = admit_again(&fixture, &files, &key, SENT + SETTLED_AFTER).await;
  assert_eq!((admitted.entry.as_ref().map(|entry| entry.id().0.as_str()).ok(), admitted.started), (Some("again"), None));
  let records = files.records().unwrap().unwrap();
  assert_eq!(records.iter().map(|record| record.id.0.as_str()).collect::<Vec<_>>(), ["again"]);
}

/// The robots of a list of `alpha`'s session, of the whole window when `complete`, asked at `asked_at`
fn listed(robots: Vec<PatronusRobot>, complete: bool, asked_at: u32) -> ListedRobots {
  ListedRobots { robots, complete, asked_at }
}

/// The registry of the sessions of the tests over the registry in `directory`, settling with no service, its reconcile
/// at the Unix seconds `now` holds; the key of `alpha`'s session
fn settling_registry(directory: &std::path::Path, now: Arc<AtomicU32>) -> (Registry, SessionKey) {
  let dir = directory.to_path_buf();
  let services: Arc<dyn remote_facts::service_sources::ServiceHost> = Arc::new(Services { patronus: None, github: None });
  let registry = Registry::new(
    crate::registry::registry_test::sessions(),
    RegistryFiles::new(HolderTool::App, move |_key| Some(dir.clone()))
      .settled_with(services)
      .at(move |_key| now.load(Ordering::SeqCst)),
  );
  let key = SessionKey {
    worktree: "/repository".to_string(),
    prefix: "test".to_string(),
  };
  (registry, key)
}

/// After a list of the robots of a session the registry settles the starts whose answer was lost of its prefix: one
/// whose robot the list has is started, one a list of the whole window lacks once it settled started none; both leave.
/// A list of a part of the window settles nothing
#[tokio::test(flavor = "multi_thread")]
async fn a_list_of_the_robots_of_a_session_settles_its_lost_starts() {
  let directory = tempfile::tempdir().unwrap();
  let files = ProcessRegistry::of(directory.path());
  let (registry, key) = settling_registry(directory.path(), Arc::new(AtomicU32::new(SENT + SETTLED_AFTER)));
  files.insert_of_another_process(lost_record(api())).unwrap();
  let robot = alpha_robot("listed", PatronusPushMode::RebaseAutosquash, SENT + 1, Some(TIP));
  registry.reconcile_after_robots(&key, &listed(Vec::new(), false, SENT + SETTLED_AFTER)).await;
  assert_eq!(files.records().unwrap().map(|records| records.len()), Some(1));
  registry.reconcile_after_robots(&key, &listed(vec![robot], false, SENT + SETTLED_AFTER)).await;
  assert_eq!(files.records().unwrap(), Some(Vec::new()));

  files.insert_of_another_process(lost_record(api())).unwrap();
  registry.reconcile_after_robots(&key, &listed(Vec::new(), true, SENT + SETTLED_AFTER)).await;
  assert_eq!(files.records().unwrap(), Some(Vec::new()));
}

/// A window that hears no patch: it keeps the session of a test open
struct Unwatched;

impl Subscriber for Unwatched {
  fn send(&self, _patch: &ViewPatch) -> Result<(), SubscriberGone> {
    Ok(())
  }
}

/// A list of the whole window settles a start by the time its read began, as the port of the facts took it
/// (`FactsPort::begin`), never by when its answer came: one asked before the start settled and answered after it proves
/// nothing, as Patronus may have created the robot since; one asked once it settled proves it absent. A read that is no
/// list settles nothing
#[tokio::test(flavor = "multi_thread")]
async fn a_list_asked_before_a_lost_start_settled_proves_nothing_however_late_it_came() {
  let directory = tempfile::tempdir().unwrap();
  let files = ProcessRegistry::of(directory.path());
  let now = Arc::new(AtomicU32::new(SENT + 16));
  let (registry, key) = settling_registry(directory.path(), Arc::clone(&now));
  registry.sessions().subscribe(&key, Box::new(Unwatched)).await.unwrap();
  files.insert_of_another_process(lost_record(api())).unwrap();
  let whole_window = |robots: Vec<PatronusRobot>| FactResult::Robots {
    read: RobotsRead::List { full: true },
    result: PatronusRobotsResult {
      availability: PatronusAvailability::Available,
      robots,
      github_repository: None,
    },
  };

  let early = FactsPort::begin(&registry, &key).await.unwrap();
  now.store(SENT + SETTLED_AFTER + 1, Ordering::SeqCst);
  let late_answer = registry.listed_robots(&key, early, &whole_window(Vec::new())).unwrap();
  assert_eq!((late_answer.complete, late_answer.asked_at), (true, SENT + 16));
  registry.reconcile_after_robots(&key, &late_answer).await;
  let records = files.records().unwrap().unwrap();
  assert_eq!((records.len(), records[0].status), (1, RecordStatus::Unknown));

  let poll = FactsPort::begin(&registry, &key).await.unwrap();
  let polled = FactResult::Robots {
    read: RobotsRead::Poll,
    result: PatronusRobotsResult::from(PatronusAvailability::Available),
  };
  assert!(registry.listed_robots(&key, poll, &polled).is_none());

  let settled = FactsPort::begin(&registry, &key).await.unwrap();
  let answer = registry.listed_robots(&key, settled, &whole_window(Vec::new())).unwrap();
  assert_eq!(answer.asked_at, SENT + SETTLED_AFTER + 1);
  registry.reconcile_after_robots(&key, &answer).await;
  assert_eq!(files.records().unwrap(), Some(Vec::new()));
}
