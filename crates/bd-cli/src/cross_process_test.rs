//! Two processes of one repository admitted through its operations registry (docs/design/git-operations.md, "Operations
//! registry"): `bd` of a child process (this test binary again, running `bd_of_another_process` with what it needs in
//! `CHILD_ENV`), beside `bd` of the test process or beside the app, whose harness here runs what the app runs (`App`:
//! the sessions over `GitSessionHost`, the scheduler of the facts, `operations::Registry` recording its operations as the
//! app's, `engine::prepare` and `engine::execute`). The fake Patronus holds a start at its request, so each side asks
//! while the other one runs. The child is the test binary, not the `bd` binary: `CARGO_BIN_EXE_bd` is set for
//! integration tests only, and only `Environment` points `bd` at the fakes (GitHub has no variable)

use crate::fixture_test::{PREFIX, TestChild, assert_error, environment, print_child_answer, run_in, run_json};
use crate::remote_fixture_test::Space;
use bd_core::context::{Environment, RepoContext, RepoOptions};
use bd_core::services::{BdServices, github_service, patronus_service, session_key};
use domain_types::error::{ApiError, ErrorCode, ErrorDetails, HolderTool, OperationKind};
use domain_types::landing::{Land, ProfileChoice, Subject};
use domain_types::operation::{Intent, Origin, Push, PushScope, RunningOperation};
use domain_types::patronus::{LandingAllowances, PatronusPushMode};
use domain_types::session::{Demand, FactFamily, FactRefreshReason, SyncReason, ViewPatch};
use operations::events::NoEvents;
use operations::land_execute::Accepted;
use operations::reconcile::SETTLED_AFTER;
use operations::registry::{Answer, Registry, RegistryFiles};
use operations::registry_file::{External, PendingStart, ProcessRegistry, Record, RecordStatus};
use remote_facts::scheduler::{FactsPort, Scheduler};
use remote_facts::service_sources::ServiceFactSources;
use repository_model::facts::Needs;
use repository_model::runtime::{GitSessionHost, RunSetup, SessionEnvironment};
use repository_model::session::{OneClock, SessionClocks, SessionKey, Sessions, Subscriber, SubscriberGone};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sync_core::patronus::SpaceRepository;
use sync_core::sync::SyncOptions;
use sync_utils::clock::SystemClock;
use sync_utils::timestamp::unix_now;
use tokio::sync::mpsc;

/// The environment variable that makes `bd_of_another_process` run: the JSON of `ChildSpec`
const CHILD_ENV: &str = "BD_CROSS_PROCESS_CHILD";

/// What the child runs: `bd <args>` in `repo` with the Space token of `home` and the fake Patronus at `patronus_url`
#[derive(serde::Serialize, serde::Deserialize)]
struct ChildSpec {
  repo: PathBuf,
  home: PathBuf,
  patronus_url: String,
  args: Vec<String>,
}

/// The child of the tests: `bd` of another process, whose exit code and output it prints on a line of its own; does
/// nothing in a normal run
#[test_log::test]
fn bd_of_another_process() {
  let Ok(spec) = std::env::var(CHILD_ENV) else { return };
  let spec: ChildSpec = serde_json::from_str(&spec).unwrap();
  let environment = Environment {
    home: Some(spec.home),
    patronus_url: spec.patronus_url,
    ..environment(&spec.repo)
  };
  let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
  let (exit_code, output) = run_json(&environment, &args);
  print_child_answer(exit_code, &output);
}

/// `bd` of another process
struct OtherBd(TestChild);

impl OtherBd {
  /// `bd --prefix PREFIX <args>` in the repository of `space`, in a child process
  fn start(space: &Space, args: &[&str]) -> Self {
    let spec = ChildSpec {
      repo: space.repo.path().to_path_buf(),
      home: space.home.path().to_path_buf(),
      patronus_url: space.patronus_url.clone(),
      args: ["--prefix", PREFIX].iter().chain(args).map(ToString::to_string).collect(),
    };
    Self(TestChild::spawn(
      "cross_process_test::bd_of_another_process",
      CHILD_ENV,
      &serde_json::to_string(&spec).unwrap(),
    ))
  }

  fn pid(&self) -> u32 {
    self.0.pid()
  }

  /// Its exit code and its output, once it ended
  fn answer(self) -> (i32, Value) {
    self.0.answer()
  }
}

/// The holder a `busy` of `bd` names: its operation, tool, pid, kind and subject
fn named_holder(output: &Value) -> (Value, Value, Value, Value, Value) {
  let holder = &output["error"]["holder"];
  (
    holder["op"].clone(),
    holder["tool"].clone(),
    holder["pid"].clone(),
    holder["kind"].clone(),
    holder["subject"].clone(),
  )
}

// jscpd:ignore-start: the test host of the app's sessions and its window, as the ones of `operations` (`push_test.rs`,
// `registry_watch_test.rs`): a test module of another crate shares no code
/// The syncs of the app's session: the options of a sync of the app, under the prefix of its key
struct AppEnvironment;

impl SessionEnvironment for AppEnvironment {
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

/// A window of the app: each patch wakes the test
struct Window(mpsc::UnboundedSender<()>);

impl Subscriber for Window {
  fn send(&self, _patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.0.send(()).map_err(|_gone| SubscriberGone)
  }
}
/// The rewrites of the app's session of the test, which runs none: a failure in the words of its chain
struct Rewrites;

impl operations::rewrite::RewriteHost for Rewrites {
  fn rewrite_error(&self, error: anyhow::Error) -> ApiError {
    ApiError::internal(format!("{error:#}"))
  }

  fn undo_error(&self, error: anyhow::Error) -> ApiError {
    ApiError::internal(format!("{error:#}")).in_undo()
  }
}
// jscpd:ignore-end

/// The services of the app's session in the repository of `space`: its fake Patronus with the Space token of its home,
/// no GitHub repository, the target `master`
fn services(space: &Space, context: &RepoContext) -> BdServices {
  let environment = space.environment();
  let mut services = BdServices::new(context, Arc::new(github_service(context, &environment)));
  services.patronus = Some(Arc::new(patronus_service(&environment).unwrap()));
  services.space = Some(SpaceRepository {
    project_key: "ij".to_string(),
    name: "ultimate".to_string(),
  });
  services.target = Some("master".to_string());
  services
}

/// The app with the repository of a `Space` open in a window: its session synced, its robots and profiles read, its
/// operations recorded in the operations registry of the repository as the app's
struct App {
  registry: Registry,
  services: Arc<BdServices>,
  key: SessionKey,
  patched: mpsc::UnboundedReceiver<()>,
}

impl App {
  async fn open(space: &Space) -> Self {
    let options = RepoOptions {
      repo: None,
      prefix: Some(PREFIX),
      full_ids: false,
      no_cache: true,
    };
    let context = RepoContext::resolve(&space.environment(), &options).unwrap();
    let key = session_key(&context);
    let clocks: Arc<dyn SessionClocks> = Arc::new(OneClock(Arc::new(SystemClock::new())));
    let runtime = tokio::runtime::Handle::current();
    let sessions = Arc::new(Sessions::new(
      runtime.clone(),
      Arc::new(GitSessionHost::new(context.git.clone(), AppEnvironment)),
      Arc::clone(&clocks),
    ));
    let events = sessions.events().unwrap();
    let common_dir = context.git_common_dir.clone();
    // a start whose answer was lost is settled over the services of the app, as late as a read once it settled
    let files = RegistryFiles::new(HolderTool::App, move |_key| Some(common_dir.clone()))
      .settled_with(Arc::new(services(space, &context)))
      .at(|_key| unix_now() + SETTLED_AFTER);
    let registry = Registry::new(Arc::clone(&sessions), files);
    let scheduler = Scheduler {
      port: Arc::new(registry.clone()) as Arc<dyn FactsPort>,
      sources: Arc::new(ServiceFactSources::new(services(space, &context))),
      clocks,
    };
    scheduler.start(&runtime, events);
    let (patches, patched) = mpsc::unbounded_channel();
    let (subscription, _view) = registry.subscribe(&key, Box::new(Window(patches))).await.unwrap();
    // the slot of each row shows its step: the session asks what a start of the rows needs
    let demand = Demand {
      focused: true,
      online: true,
      needed: vec!["auth".to_string(), "docs".to_string()],
      ..Demand::default()
    };
    sessions.set_demand(subscription, &demand);
    sessions.sync(&key, SyncReason::Manual).wait().await.unwrap();
    for family in [FactFamily::Robots, FactFamily::Profiles] {
      sessions.refresh(&key, family, FactRefreshReason::Manual, None).await;
    }
    Self {
      registry,
      services: Arc::new(services(space, &context)),
      key,
      patched,
    }
  }

  /// The operations of the view once `done` holds of them, read again after each patch
  async fn operations_once(&mut self, done: impl Fn(&[RunningOperation]) -> bool) -> Vec<RunningOperation> {
    loop {
      let view = self.registry.sessions().get_snapshot(&self.key).await.unwrap();
      let operations: Vec<RunningOperation> = view.operations.into_values().collect();
      if done(&operations) {
        return operations;
      }
      self.patched.recv().await.unwrap();
    }
  }

  /// `intent` run as a window starts it, on a task of its own: its plan once the facts it waits for came, every
  /// confirmation of it accepted
  async fn spawn_execute(&mut self, intent: Intent) -> tokio::task::JoinHandle<Answer> {
    let plan = loop {
      let plan = operations::engine::prepare(self.registry.sessions(), &self.key, intent.clone()).await.unwrap();
      if let Some(plan) = plan.filter(|plan| plan.waits.is_empty()) {
        break plan;
      }
      self.patched.recv().await.unwrap();
    };
    let accepted = Accepted {
      fingerprint: plan.fingerprint,
      confirmations: plan.confirmations,
    };
    let (registry, key, services) = (self.registry.clone(), self.key.clone(), Arc::clone(&self.services));
    tokio::spawn(async move {
      let hosts = operations::engine::Hosts {
        services: &*services,
        rewrites: &Rewrites,
      };
      operations::engine::execute(&registry, &key, intent, accepted, hosts, Arc::new(NoEvents)).await
    })
  }
}

/// The Safe Push of `branch` as the slot of its row starts it
fn safe_push_of(branch: &str) -> Intent {
  Intent::Land(Land {
    subject: Subject::branch(branch),
    mode: PatronusPushMode::RebaseAutosquash,
    profile: ProfileChoice::Default,
    without_approvals: false,
    create_pull_request: false,
    allow: LandingAllowances::default(),
  })
}

/// The push of `branch` alone, as its push button pushes it
fn push_of(branch: &str) -> Intent {
  Intent::Push(Push {
    subject: Subject::branch(branch),
    scope: PushScope::Alone,
    expect_remote: None,
    force: false,
    restart_running: false,
  })
}

/// `bd safe-push <branch>` of another process, held by the fake Patronus at its start: the gate that holds it, the
/// process, and its record once its request reached Patronus
fn bd_held_at_its_start(space: &Space, branch: &str) -> (Arc<test_utils::fake_server::FakeGate>, OtherBd, Record) {
  let start = space.fake.hold("POST", "/app/rest/v1/robots");
  let bd = OtherBd::start(space, &["safe-push", branch]);
  space.runtime.block_on(start.arrived());
  (start, bd, the_record(space))
}

/// The one record of the registry of the repository of `space`
fn the_record(space: &Space) -> Record {
  let records = ProcessRegistry::of(&space.repo.path().join(".git")).records().unwrap().unwrap();
  let [record] = records.as_slice() else {
    panic!("one record: {records:?}");
  };
  record.clone()
}

/// The stage's exit (docs/design/rust-domain-authority.md, s10-registry-journal): while the app's Safe Push of auth
/// waits for Patronus to answer its start, `bd safe-push auth` of another process is `busy` (exit 3), naming the app's
/// operation, its pid and what it runs, and starts nothing; `bd push docs` runs beside it. Once Patronus answered, the
/// app's start ends and leaves the registry empty; Patronus got one start
#[test_log::test]
fn bd_beside_the_apps_safe_push_of_the_same_branch_is_busy_naming_the_app() {
  let space = Space::new(&["(auth) Add auth", "(docs) Add docs"]);
  space.set_robots(Vec::new());
  let start = space.fake.hold("POST", "/app/rest/v1/robots");
  space.runtime.block_on(async {
    let mut app = App::open(&space).await;
    let running = app.spawn_execute(safe_push_of("auth")).await;
    start.arrived().await;
    let held = the_record(&space);
    assert_eq!((held.tool, held.pid), (HolderTool::App, std::process::id()));

    let bd = OtherBd::start(&space, &["safe-push", "auth"]);
    let (exit_code, output) = tokio::task::spawn_blocking(move || bd.answer()).await.unwrap();
    assert_error(&output, exit_code, (3, "busy"));
    assert_eq!(
      named_holder(&output),
      (json!(held.id.0), json!("app"), json!(std::process::id()), json!("safe_push"), json!("auth")),
      "{output}"
    );
    let bd = OtherBd::start(&space, &["push", "docs"]);
    let (exit_code, output) = tokio::task::spawn_blocking(move || bd.answer()).await.unwrap();
    assert_eq!(exit_code, 0, "{output}");

    start.open();
    let outcome = running.await.unwrap().unwrap();
    assert!(
      outcome.effects.iter().all(|effect| effect.status == domain_types::operation::EffectStatus::Done),
      "{outcome:?}"
    );
  });
  assert_eq!(ProcessRegistry::of(&space.repo.path().join(".git")).records().unwrap(), Some(Vec::new()));
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1);
}

/// The other way round: while `bd safe-push auth` of another process waits for Patronus to answer its start, the app's
/// window shows it on its row (an operation of another process of its session), and the app's Safe Push of auth is
/// `busy` naming the operation of `bd`, its pid and what it runs, before anything of it runs; the app pushes docs beside
/// it. Once Patronus answered, `bd` ends, the registry is empty and the window shows nothing of it
#[test_log::test]
fn the_apps_safe_push_beside_bd_of_the_same_branch_is_busy_naming_bd() {
  let space = Space::new(&["(auth) Add auth", "(docs) Add docs"]);
  space.set_robots(Vec::new());
  let start = space.fake.hold("POST", "/app/rest/v1/robots");
  let bd = OtherBd::start(&space, &["safe-push", "auth"]);
  let pid = bd.pid();
  space.runtime.block_on(async {
    start.arrived().await;
    let held = the_record(&space);
    assert_eq!((held.tool, held.pid), (HolderTool::Bd, pid));
    let mut app = App::open(&space).await;
    let shown = app.operations_once(|operations| !operations.is_empty()).await;
    assert_eq!(
      shown.iter().map(|operation| (operation.id.clone(), operation.origin.clone())).collect::<Vec<_>>(),
      [(held.id.clone(), Origin::OtherProcess { tool: HolderTool::Bd, pid })]
    );

    let error = app.spawn_execute(safe_push_of("auth")).await.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::Busy, "{error:?}");
    let Some(ErrorDetails::Busy { holder: Some(holder) }) = &error.details else {
      panic!("no holder: {error:?}");
    };
    assert_eq!(
      (holder.op.as_str(), holder.tool, holder.pid, holder.kind, holder.subject.as_deref()),
      (held.id.0.as_str(), HolderTool::Bd, pid, OperationKind::SafePush, Some("auth"))
    );
    app.spawn_execute(push_of("docs")).await.await.unwrap().unwrap();

    start.open();
    let (exit_code, output) = tokio::task::spawn_blocking(move || bd.answer()).await.unwrap();
    assert_eq!(exit_code, 0, "{output}");
    app.operations_once(<[RunningOperation]>::is_empty).await;
  });
  assert_eq!(ProcessRegistry::of(&space.repo.path().join(".git")).records().unwrap(), Some(Vec::new()));
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1, "the start of bd alone reached Patronus");
}

/// While the `bd safe-push auth` of another process waits for Patronus to answer its start, a `bd safe-push auth` here is
/// `busy` (exit 3, retryable), naming the operation of the other process, its pid and what it runs, and starts nothing; a
/// push of another branch runs beside it. Once Patronus answered, the other one ends and leaves the registry empty
#[test_log::test]
fn a_safe_push_beside_the_safe_push_of_another_process_is_busy_with_its_holder() {
  let space = Space::new(&["(auth) Add auth", "(docs) Add docs"]);
  let (start, bd, held) = bd_held_at_its_start(&space, "auth");
  assert_eq!(held.pid, bd.pid());

  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (3, "busy"));
  assert_eq!(
    named_holder(&output),
    (json!(held.id.0), json!("bd"), json!(bd.pid()), json!("safe_push"), json!("auth")),
    "{output}"
  );
  assert_eq!(output["error"]["retryable"], json!(true));
  assert!(output["error"]["message"].as_str().unwrap().contains("has run the Safe Push of auth"), "{output}");
  // another branch runs beside it
  let (exit_code, output) = run_json(&space.environment(), &["--prefix", PREFIX, "push", "docs"]);
  assert_eq!(exit_code, 0, "{output}");

  start.open();
  let (exit_code, output) = bd.answer();
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(ProcessRegistry::of(&space.repo.path().join(".git")).records().unwrap(), Some(Vec::new()));
  // the start of the other process alone reached Patronus
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1);
}

/// `bd` touches the stamp of its repository (`$GIT_COMMON_DIR/branch-deck/stamp`) after a mutation that succeeded, so
/// the app's windows of the repository sync; a read, a dry run and a failed command leave it as it was
#[test_log::test]
fn bd_moves_the_stamp_of_its_repository_after_a_mutation_only() {
  let space = Space::new(&["(auth) Add auth"]);
  let stamp = space.repo.path().join(".git/branch-deck/stamp");
  let read = |path: &Path| std::fs::read_to_string(path).ok();
  for args in [&["status"][..], &["fold", "--dry-run"], &["describe", "nothing", "--title", "T"]] {
    let _ = run_in(&space.repo, args);
    assert_eq!(read(&stamp), None, "{args:?}");
  }
  let (exit_code, output) = run_in(&space.repo, &["describe", "auth", "--title", "Auth"]);
  assert_eq!(exit_code, 0, "{output}");
  let touched = read(&stamp).unwrap();
  assert!(touched.ends_with(&format!(" {}\n", std::process::id())), "{touched}");
}

/// `bd safe-push auth` killed after its push, while Patronus holds its start: its record stays, with the start it sent
/// (`pending_start`, on the pushed commit), and the request of the killed process never reaches Patronus. The app's Safe
/// Push of auth settles it first (a read of Patronus once it settled lists no robot of it: none started), so it is
/// admitted, plans no push (the remote has the commit) and starts once
#[test_log::test]
fn a_bd_killed_between_its_push_and_its_start_leaves_a_lost_start_the_app_settles_and_starts_once() {
  use domain_types::landing::PlannedEffect;
  use domain_types::operation::EffectStatus;
  let space = Space::new(&["(auth) Add auth", "(docs) Add docs"]);
  space.set_robots(Vec::new());
  let (_start, bd, held) = bd_held_at_its_start(&space, "auth");
  let pending = held.pending_start().cloned().unwrap();
  let pushed = space.repo.git(&["rev-parse", &format!("refs/remotes/origin/{PREFIX}/virtual/auth")]);
  assert_eq!(
    (pending.source_ref.as_str(), pending.tip.as_str()),
    (format!("refs/heads/{PREFIX}/virtual/auth").as_str(), pushed.as_str())
  );
  bd.0.kill();
  space.fake.unhold("POST", "/app/rest/v1/robots");

  space.runtime.block_on(async {
    let mut app = App::open(&space).await;
    let outcome = app.spawn_execute(safe_push_of("auth")).await.await.unwrap().unwrap();
    let effects: Vec<(&PlannedEffect, &EffectStatus)> = outcome.effects.iter().map(|record| (&record.effect, &record.status)).collect();
    assert!(matches!(effects.as_slice(), [(PlannedEffect::StartRobot { .. }, EffectStatus::Done)]), "{effects:?}");
  });
  assert_eq!(ProcessRegistry::of(&space.repo.path().join(".git")).records().unwrap(), Some(Vec::new()));
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1, "the start of the app alone reached Patronus");
}

/// A start whose answer was lost, of a process that is gone, holds `bd safe-push auth` (`busy`, exit 3, the holder
/// `unknown`, the message naming `bd operation forget`) while Patronus can't tell yet; `bd operation forget <op>`
/// takes it out (and refuses an id the registry lacks), and the Safe Push starts
#[test_log::test]
fn a_lost_start_holds_bd_until_bd_operation_forget_lets_it_go() {
  let space = Space::new(&["(auth) Add auth"]);
  space.set_robots(Vec::new());
  let now = unix_now();
  let lost = Record {
    id: domain_types::operation::OperationId("01LOST".to_string()),
    tool: HolderTool::Bd,
    pid: 4_000_000,
    process_started_at: Some(1),
    host: sync_utils::process_scan::host_name().to_string(),
    started_at: now,
    heartbeat_at: now,
    session: SessionKey {
      worktree: space.repo.path_str().to_string(),
      prefix: PREFIX.to_string(),
    },
    intent: safe_push_of("auth"),
    resources: vec![domain_types::operation::Resource::Lands { branch: "auth".to_string() }],
    step: domain_types::operation::OperationStep::Starting,
    status: RecordStatus::Running,
    external: Some(External {
      pending_start: Some(PendingStart {
        source_ref: format!("refs/heads/{PREFIX}/virtual/auth"),
        tip: String::new(),
        route: domain_types::landing::StartRoute::Api { profile: None },
        pr: None,
        comment_id: None,
        sent_at: now,
        url: None,
      }),
    }),
  };
  let registry = ProcessRegistry::of(&space.repo.path().join(".git"));
  registry.insert_of_another_process(lost).unwrap();

  let (exit_code, output) = space.run(&["auth"]);
  assert_error(&output, exit_code, (3, "busy"));
  assert_eq!(
    (output["error"]["holder"]["op"].clone(), output["error"]["holder"]["unknown"].clone()),
    (json!("01LOST"), json!(true)),
    "{output}"
  );
  assert!(output["error"]["message"].as_str().unwrap().contains("`bd operation forget 01LOST`"), "{output}");

  let forget = |id: &str| run_json(&space.environment(), &["--prefix", PREFIX, "operation", "forget", id]);
  let (exit_code, output) = forget("01LOST");
  assert_eq!(
    (exit_code, output["forgotten"].clone(), output["subject"].clone()),
    (0, json!("01LOST"), json!("auth")),
    "{output}"
  );
  let (exit_code, output) = forget("01LOST");
  assert_error(&output, exit_code, (2, "invalid_arguments"));

  let (exit_code, output) = space.run(&["auth"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(space.fake.starts.lock().unwrap().len(), 1);
  assert_eq!(registry.records().unwrap(), Some(Vec::new()));
}
