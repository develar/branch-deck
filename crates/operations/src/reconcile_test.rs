use crate::engine::request_of;
use crate::land_execute::land_execute_test::{ALPHA, Fixture as StartFixture, fixture as start_fixture, statuses};
use crate::land_execute::{Accepted, Steps, execute};
use crate::push::push_test::Services;
use crate::reconcile::{ReconcileSources, Reconciled, SETTLED_AFTER, SettleWith, admit_settled, by_robots, reconcile, robot_of};
use crate::registry::{Registry, RegistryFiles};
use crate::registry_file::{PendingStart, ProcessRegistry, Record, RecordStatus};
use domain_policy::refresh::PATRONUS_COMMENT_CLOCK_SLACK;
use domain_types::error::{ErrorCode, ErrorDetails, HolderTool};
use domain_types::fixtures::{land_intent, robot};
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::operation::{EffectStatus, Fingerprint, Intent, JournalTransition, OperationId, OperationResult, OperationStep};
use domain_types::patronus::{PatronusAvailability, PatronusPullRequestLink, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use github::fake_github::{FakeAnswer, start_fake_service};
use patronus::fake_patronus::{FakePatronus, robot_json, start_fake};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use pretty_assertions::assert_eq;
use repository_model::session::SessionKey;
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

/// Without its robot, a start through the API is proven absent only by a list of the whole window since it was sent
/// asked once it settled (the time a start waits for its answer, and as long again): one asked before tells nothing; a
/// comment is never proven absent by the robots (the bot may still answer it)
#[test]
fn a_list_of_the_whole_window_proves_a_lost_start_absent_once_it_settled() {
  let safe_push = PatronusPushMode::RebaseAutosquash;
  let settled = SENT + SETTLED_AFTER;
  assert_eq!(by_robots(&api(), safe_push, "test", &[], settled), Reconciled::Failed);
  assert_eq!(by_robots(&api(), safe_push, "test", &[], settled - 1), Reconciled::Unknown);
  let comment = pending(StartRoute::Comment { number: 7 });
  assert_eq!(by_robots(&comment, safe_push, "test", &[], settled + 600), Reconciled::Unknown);
  let listed = alpha_robot("listed", safe_push, SENT + 1, Some(TIP));
  assert_eq!(
    by_robots(&api(), safe_push, "test", std::slice::from_ref(&listed), SENT),
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
  assert_eq!(reconcile(&record, &at(SENT + 5), Some(&[])).await, Reconciled::Started(None));
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
  assert_eq!(reconcile(&record, &at(SENT + 5), Some(&[])).await, Reconciled::Failed);

  fake.answer("GET", COMMENTS, FakeAnswer::ok(json!([])));
  assert_eq!(reconcile(&record, &at(SENT + 5), Some(&[])).await, Reconciled::Unknown);
  assert_eq!(reconcile(&record, &at(SENT + SETTLED_AFTER), Some(&[])).await, Reconciled::Failed);

  // its id known: its reactions are read, no search
  let searched = fake.requests_to("GET", COMMENTS).len();
  let mut known = pending(StartRoute::Comment { number: 7 });
  known.comment_id = Some(4402);
  fake.answer(
    "GET",
    "/repos/acme/monorepo/issues/comments/4402",
    FakeAnswer::ok(json!({"id": 4402, "reactions": {"+1": 1, "-1": 0}})),
  );
  assert_eq!(reconcile(&lost_record(known), &at(SENT + 5), Some(&[])).await, Reconciled::Started(None));
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
    (refused.code, admitted.started.map(|started| (started.session == key, started.robot.id))),
    (ErrorCode::PlanChanged, Some((true, "started-1".to_string())))
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

/// A fake Patronus that lists no robot yet, and the service of the app over it, which keeps a full list for a while
/// (`FULL_LIST_REUSE`), as for the windows of a repository
async fn patronus_of_the_app() -> (Arc<FakePatronus>, Arc<PatronusService>) {
  let (fake, client) = start_fake().await;
  *fake.robots.lock().unwrap() = Some(Vec::new());
  let service = PatronusService::new(client, |_| {
    TokenLookup::Found(Token {
      value: "t".to_string(),
      stamp: TokenStamp::Env,
    })
  });
  (fake, Arc::new(service))
}

/// The robot `id` of the start of `alpha` as Patronus lists it, created at `created` on `TIP`
fn listed_robot(id: &str, created: u32) -> serde_json::Value {
  let mut robot = robot_json(id, "RUNNING", ALPHA, &format_utc_timestamp(created));
  robot["sourceRevision"] = json!(TIP);
  robot
}

/// The registry of the sessions of the tests over the registry in `directory`, settling over `patronus` (none: no
/// service), its reconcile at the Unix seconds `now` holds; the key of `alpha`'s session
fn settling_registry(directory: &std::path::Path, now: Arc<AtomicU32>, patronus: Option<Arc<PatronusService>>) -> (Registry, SessionKey) {
  let dir = directory.to_path_buf();
  let services: Arc<dyn remote_facts::service_sources::ServiceHost> = Arc::new(Services { patronus, github: None });
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

/// What the journal in `directory` tells of each settled start: the robot it started (none: the bot accepted its
/// comment), or that none started
fn settled_as(directory: &std::path::Path) -> Vec<Option<String>> {
  crate::journal::read(directory)
    .unwrap()
    .into_iter()
    .filter_map(|entry| match entry.transition? {
      JournalTransition::Started { robot } => Some(Some(robot.map_or_else(String::new, |robot| robot.id))),
      JournalTransition::NotStarted => Some(None),
      JournalTransition::Forgotten => None,
    })
    .collect()
}

/// After a list of the robots of a session the registry settles the starts whose answer was lost of its prefix: one
/// whose robot the list has started, with no request of its own. One the list lacks proves nothing by it: once the start
/// settled, the reconcile asks the robots since it was sent itself, and a list of its own without the robot proves that
/// none started; before, nothing is asked and it stays. Settled, it leaves
#[tokio::test(flavor = "multi_thread")]
async fn a_list_of_the_robots_of_a_session_settles_its_lost_starts() {
  let directory = tempfile::tempdir().unwrap();
  let files = ProcessRegistry::of(directory.path());
  let (fake, patronus) = patronus_of_the_app().await;
  let now = Arc::new(AtomicU32::new(SENT + 16));
  let (registry, key) = settling_registry(directory.path(), Arc::clone(&now), Some(patronus));
  files.insert_of_another_process(lost_record(api())).unwrap();
  registry.reconcile_after_robots(&key, &[]).await;
  assert_eq!(files.records().unwrap().map(|records| records.len()), Some(1));
  let robot = alpha_robot("listed", PatronusPushMode::RebaseAutosquash, SENT + 1, Some(TIP));
  registry.reconcile_after_robots(&key, &[robot]).await;
  assert_eq!(files.records().unwrap(), Some(Vec::new()));
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 0);

  files.insert_of_another_process(lost_record(api())).unwrap();
  now.store(SENT + SETTLED_AFTER, Ordering::SeqCst);
  registry.reconcile_after_robots(&key, &[]).await;
  assert_eq!(files.records().unwrap(), Some(Vec::new()));
  assert_eq!(settled_as(directory.path()), [Some("listed".to_string()), None]);
  let queries = fake.list_queries.lock().unwrap().clone();
  assert_eq!(queries.len(), 1);
  assert_eq!(queries[0].get("start"), Some(&format_utc_timestamp(SENT - PATRONUS_COMMENT_CLOCK_SLACK)));
}

/// Whether the open session of `key` has the robot `id` among its robots
async fn session_has_robot(registry: &Registry, key: &SessionKey, id: &'static str) -> Option<bool> {
  registry.sessions().read(key, move |_state, facts| facts.robots.view().robots.contains_key(id)).await
}

/// A full list of the robots that the service kept for the windows (`FULL_LIST_REUSE`) answers a session after the start
/// settled, but was asked before Patronus created the robot of a start whose answer was lost: it never proves the start
/// absent. The reconcile asks the robots since the start was sent itself, finds the robot and settles it as started;
/// the robot goes into the session that settled it and into the session of the record (another worktree with the same
/// prefix), and the service drops its kept list, so the next full list of a window asks Patronus and lists it: no row
/// offers to start it once more
#[tokio::test(flavor = "multi_thread")]
async fn a_list_the_service_kept_from_before_a_lost_start_settled_never_proves_it_absent() {
  let directory = tempfile::tempdir().unwrap();
  let files = ProcessRegistry::of(directory.path());
  let (fake, patronus) = patronus_of_the_app().await;
  let now = Arc::new(AtomicU32::new(SENT + 55));
  let (registry, key) = settling_registry(directory.path(), Arc::clone(&now), Some(Arc::clone(&patronus)));
  let mut record = lost_record(api());
  record.session.worktree = "/elsewhere".to_string();
  let elsewhere = record.session.clone();
  let _open = (registry.sessions().hold(&key), registry.sessions().hold(&elsewhere));
  files.insert_of_another_process(record).unwrap();
  // the full list of a window (Refresh), asked before the start settled: kept by the service
  let first = patronus.robots("ultimate", "test", None, false, false).await;
  assert_eq!((first.availability.clone(), first.robots.len()), (PatronusAvailability::Available, 0));
  registry.reconcile_after_robots(&key, &first.robots).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 1);

  // Patronus creates the robot of the start late; the next full list, once the start settled, is the kept one
  *fake.robots.lock().unwrap() = Some(vec![listed_robot("late", SENT + 59)]);
  now.store(SENT + SETTLED_AFTER + 1, Ordering::SeqCst);
  let kept = patronus.robots("ultimate", "test", None, false, false).await;
  assert_eq!((kept.robots.len(), fake.list_calls.load(Ordering::SeqCst)), (0, 1));
  assert_eq!(session_has_robot(&registry, &key, "late").await, Some(false));
  registry.reconcile_after_robots(&key, &kept.robots).await;

  assert_eq!(files.records().unwrap(), Some(Vec::new()));
  assert_eq!(settled_as(directory.path()), [Some("late".to_string())]);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);
  assert_eq!(
    (session_has_robot(&registry, &key, "late").await, session_has_robot(&registry, &elsewhere, "late").await),
    (Some(true), Some(true))
  );
  let next = patronus.robots("ultimate", "test", None, false, false).await;
  assert_eq!(
    (
      next.robots.iter().map(|robot| robot.id.as_str()).collect::<Vec<_>>(),
      fake.list_calls.load(Ordering::SeqCst)
    ),
    (vec!["late"], 3)
  );
}

/// The lost starts of a prefix that a list of the session lacks are settled by one list of the robots since the oldest
/// of them, and a list of another window that comes meanwhile joins that reconcile, which runs it once it ended, so it
/// asks nothing for what that one settled: one request for two starts and two lists
#[tokio::test(flavor = "multi_thread")]
async fn the_lost_starts_of_a_prefix_settle_by_one_list_since_the_oldest() {
  let directory = tempfile::tempdir().unwrap();
  let files = ProcessRegistry::of(directory.path());
  let (fake, patronus) = patronus_of_the_app().await;
  let now = Arc::new(AtomicU32::new(SENT + 5 + SETTLED_AFTER));
  let (registry, key) = settling_registry(directory.path(), now, Some(patronus));
  files.insert_of_another_process(lost_record(api())).unwrap();
  let mut beta = lost_record(PendingStart {
    source_ref: "refs/heads/test/virtual/beta".to_string(),
    sent_at: SENT + 5,
    ..api()
  });
  beta.id = OperationId("lost-beta".to_string());
  beta.intent = land_intent("beta", PatronusPushMode::RebaseAutosquash);
  files.insert_of_another_process(beta).unwrap();
  *fake.robots.lock().unwrap() = Some(vec![listed_robot("late", SENT + 59)]);

  tokio::join!(registry.reconcile_after_robots(&key, &[]), registry.reconcile_after_robots(&key, &[]));

  assert_eq!(files.records().unwrap(), Some(Vec::new()));
  let mut settled = settled_as(directory.path());
  settled.sort();
  assert_eq!(settled, [None, Some("late".to_string())]);
  let queries = fake.list_queries.lock().unwrap().clone();
  assert_eq!(
    queries.iter().map(|query| query.get("start").cloned()).collect::<Vec<_>>(),
    [Some(format_utc_timestamp(SENT - PATRONUS_COMMENT_CLOCK_SLACK))]
  );
}
