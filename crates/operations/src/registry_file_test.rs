use crate::registry::OperationRequest;
use crate::registry_file::{External, GONE_AFTER, PendingStart, ProcessIdentity, ProcessRegistry, Record, RecordStatus, RegistryFile, SCHEMA, admit_into, beat, is_gone, others};
use domain_types::error::{ErrorCode, ErrorDetails, HolderTool, OperationHolder, OperationKind};
use domain_types::fixtures::{land_intent, push_intent};
use domain_types::landing::StartRoute;
use domain_types::operation::{JournalTransition, OperationId, OperationStep, Resource};
use domain_types::patronus::PatronusPushMode;
use pretty_assertions::assert_eq;
use repository_model::session::SessionKey;
use std::collections::HashSet;
use sync_utils::process_scan::{is_running, process_started};
use sync_utils::timestamp::unix_now;

const NOW: u32 = 1_800_000_000;

pub(crate) fn id(text: &str) -> OperationId {
  OperationId(text.to_string())
}

pub(crate) fn session(prefix: &str) -> SessionKey {
  SessionKey {
    worktree: "/repo".to_string(),
    prefix: prefix.to_string(),
  }
}

fn lands(branch: &str) -> Resource {
  Resource::Lands { branch: branch.to_string() }
}

/// The Safe Push of `branch`, as `engine::request_of` asks for it: what it lands
pub(crate) fn safe_push(branch: &str) -> OperationRequest {
  OperationRequest {
    intent: land_intent(branch, PatronusPushMode::RebaseAutosquash),
    resources: vec![lands(branch)],
    step: OperationStep::Checking,
    moves: Vec::new(),
  }
}

/// A process of this host: `pid` with the start time `started`
pub(crate) fn process(pid: u32, started: Option<u64>) -> ProcessIdentity {
  ProcessIdentity {
    pid,
    started,
    host: "this-host".to_string(),
  }
}

/// The record of `request` as the operation `op` of `of`, admitted and last beaten at `at`
pub(crate) fn record(op: &str, of: &ProcessIdentity, request: &OperationRequest, at: u32) -> Record {
  Record {
    id: id(op),
    tool: HolderTool::Bd,
    pid: of.pid,
    process_started_at: of.started,
    host: of.host.clone(),
    started_at: at,
    heartbeat_at: at,
    session: session("user"),
    intent: request.intent.clone(),
    resources: request.resources.clone(),
    step: request.step.clone(),
    status: RecordStatus::Running,
    external: None,
  }
}

fn file(records: Vec<Record>) -> RegistryFile {
  RegistryFile {
    schema: SCHEMA,
    operations: records,
  }
}

fn ids(file: &RegistryFile) -> Vec<&str> {
  file.operations.iter().map(|record| record.id.0.as_str()).collect()
}

fn alive(_pid: u32, _started: Option<u64>) -> bool {
  true
}

fn dead(_pid: u32, _started: Option<u64>) -> bool {
  false
}

/// A record of another process holds what the same subject or a resource of it needs while the process runs; its own
/// process admits beside it, and an operation of another subject too
#[test]
fn a_record_of_another_process_holds_what_it_needs_and_the_records_of_this_process_hold_nothing() {
  let (this, bd) = (process(100, Some(1)), process(200, Some(2)));
  let mut registry = file(vec![record("bd", &bd, &safe_push("auth"), NOW)]);
  let holder = admit_into(&mut registry, record("app", &this, &safe_push("auth"), NOW), &this, NOW, &alive).unwrap_err();
  assert_eq!(holder.id, id("bd"));
  assert_eq!(
    holder.holder(NOW + 12),
    OperationHolder {
      op: "bd".to_string(),
      tool: HolderTool::Bd,
      pid: 200,
      kind: OperationKind::SafePush,
      subject: Some("auth".to_string()),
      held_for: 12,
      unknown: false,
    }
  );
  assert_eq!(ids(&registry), ["bd"]);

  // another subject runs beside it
  admit_into(&mut registry, record("docs", &this, &safe_push("docs"), NOW), &this, NOW, &alive).unwrap();
  // what this process runs never holds its own: its session admits those over each other
  admit_into(&mut registry, record("again", &this, &safe_push("docs"), NOW), &this, NOW, &alive).unwrap();
  assert_eq!(ids(&registry), ["bd", "docs", "again"]);
  // the other process is the one that is busy now
  assert_eq!(
    admit_into(&mut registry, record("bd2", &bd, &safe_push("docs"), NOW), &bd, NOW, &alive).unwrap_err().id,
    id("docs")
  );
}

/// A record of a process of this host that runs no more, or whose pid runs another process now (another start time),
/// leaves at the next admission at once, whatever its heartbeat says; one of another host leaves only once its heartbeat
/// is older than five minutes, as its pid tells nothing here
#[test]
fn a_gone_process_of_this_host_leaves_at_once_and_one_of_another_host_after_five_minutes() {
  let this = process(100, Some(1));
  let gone = record("gone", &process(200, Some(2)), &safe_push("auth"), NOW);
  assert!(is_gone(&gone, &this, NOW, &dead));
  assert!(!is_gone(&gone, &this, NOW, &alive));
  let mut registry = file(vec![gone]);
  admit_into(&mut registry, record("app", &this, &safe_push("auth"), NOW), &this, NOW, &dead).unwrap();
  assert_eq!(ids(&registry), ["app"]);

  // a pid the system gave to another process: the start time tells it apart
  let own = std::process::id();
  let started = process_started(own);
  let reused = record("reused", &process(own, started.map(|started| started + 1)), &safe_push("auth"), NOW);
  assert!(is_gone(&reused, &this, NOW, &is_running));
  if started.is_some() {
    assert!(!is_gone(&record("own", &process(own, started), &safe_push("auth"), NOW), &this, NOW, &is_running));
  }

  let remote = ProcessIdentity {
    host: "another-host".to_string(),
    ..process(200, Some(2))
  };
  let fresh = record("remote", &remote, &safe_push("auth"), NOW - GONE_AFTER);
  // its pid is never asked: a pid of another host means nothing here
  let never = |_pid: u32, _started: Option<u64>| -> bool { unreachable!("the pid of another host is asked") };
  assert!(!is_gone(&fresh, &this, NOW, &never));
  assert!(is_gone(&fresh, &this, NOW + 1, &never));
  let mut registry = file(vec![fresh]);
  assert_eq!(
    admit_into(&mut registry, record("app", &this, &safe_push("auth"), NOW), &this, NOW, &never).unwrap_err().id,
    id("remote")
  );
  admit_into(&mut registry, record("app", &this, &safe_push("auth"), NOW + 1), &this, NOW + 1, &never).unwrap();
  assert_eq!(ids(&registry), ["app"]);
}

/// A record of a process that is gone with a start in flight stays, as `Unknown`, and holds what it held: what the start
/// did is not known yet
#[test]
fn a_gone_record_with_a_start_in_flight_stays_unknown_and_holds() {
  let this = process(100, Some(1));
  let in_flight = Record {
    external: Some(External {
      pending_start: Some(PendingStart {
        source_ref: "refs/heads/user/virtual/auth".to_string(),
        tip: "1a2b3c".to_string(),
        route: StartRoute::Api { profile: None },
        pr: None,
        comment_id: None,
        sent_at: NOW,
        url: None,
      }),
    }),
    ..record("sent", &process(200, Some(2)), &safe_push("auth"), NOW)
  };
  let mut registry = file(vec![in_flight]);
  assert_eq!(
    admit_into(&mut registry, record("app", &this, &safe_push("auth"), NOW), &this, NOW, &dead).unwrap_err().id,
    id("sent")
  );
  assert_eq!((ids(&registry), registry.operations[0].status), (vec!["sent"], RecordStatus::Unknown));
}

/// Under another branch prefix a branch of the same name is another branch: a Safe Push of it runs beside
#[test]
fn a_record_of_another_prefix_holds_only_what_the_worktrees_share() {
  let (this, bd) = (process(100, Some(1)), process(200, Some(2)));
  let mut registry = file(vec![record("bd", &bd, &safe_push("auth"), NOW)]);
  let other = Record {
    session: session("other"),
    ..record("app", &this, &safe_push("auth"), NOW)
  };
  admit_into(&mut registry, other, &this, NOW, &alive).unwrap();
  let push = OperationRequest {
    intent: push_intent("auth"),
    resources: vec![Resource::Ref {
      name: "user/virtual/auth".to_string(),
    }],
    step: OperationStep::Checking,
    moves: Vec::new(),
  };
  let mut registry = file(vec![record("bd", &bd, &push, NOW)]);
  let pushes_the_same_ref = Record {
    session: session("other"),
    ..record("app", &this, &push, NOW)
  };
  assert_eq!(admit_into(&mut registry, pushes_the_same_ref, &this, NOW, &alive).unwrap_err().id, id("bd"));
}

/// The heartbeat gives the live records of this process the time, takes out the ones of it that are not live (their
/// removal failed), and leaves the records of other processes as they are
#[test]
fn a_heartbeat_beats_the_live_records_of_this_process_and_takes_out_its_others() {
  let (this, bd) = (process(100, Some(1)), process(200, Some(2)));
  let mut registry = file(vec![
    record("live", &this, &safe_push("auth"), NOW),
    record("left", &this, &safe_push("docs"), NOW),
    record("bd", &bd, &safe_push("api"), NOW),
  ]);
  beat(&mut registry, &this, &HashSet::from([id("live")]), NOW + 30);
  assert_eq!(ids(&registry), ["live", "bd"]);
  assert_eq!(registry.operations.iter().map(|record| record.heartbeat_at).collect::<Vec<_>>(), [NOW + 30, NOW]);
}

/// A session shows the records of the other processes that hold what they hold: of a process that runs, and of one that
/// is gone with a start in flight; never one of this process, and never one whose process is gone with nothing in flight
#[test]
fn a_session_shows_the_records_of_other_processes_that_hold_what_they_hold() {
  let (this, bd, gone, elsewhere) = (
    process(100, Some(1)),
    process(200, Some(2)),
    process(300, Some(3)),
    ProcessIdentity {
      host: "another-host".to_string(),
      ..process(400, Some(4))
    },
  );
  let mut in_flight = record("in-flight", &gone, &safe_push("api"), NOW);
  in_flight.external = Some(External { pending_start: None });
  let records = vec![
    record("own", &this, &safe_push("auth"), NOW),
    record("bd", &bd, &safe_push("docs"), NOW),
    record("gone", &gone, &safe_push("login"), NOW),
    in_flight,
    record("elsewhere", &elsewhere, &safe_push("ui"), NOW - GONE_AFTER - 1),
  ];
  let runs = |pid: u32, _started: Option<u64>| pid == 200;
  let shown = others(records, &this, NOW, &runs);
  assert_eq!(shown.iter().map(|record| record.id.0.as_str()).collect::<Vec<_>>(), ["bd", "in-flight"]);
}

/// A child that runs until it is killed: another process of this host
pub(crate) fn sleeping_child() -> std::process::Child {
  let mut command = if cfg!(windows) {
    let mut ping = sync_utils::child_process::windowless_command("ping");
    ping.args(["-n", "120", "127.0.0.1"]);
    ping
  } else {
    let mut sleep = sync_utils::child_process::windowless_command("sleep");
    sleep.arg("120");
    sleep
  };
  command.stdout(std::process::Stdio::null()).spawn().unwrap()
}

/// The file says busy while another process of this host that runs the same subject lives, naming it; once that process
/// is gone the admission takes its record out and writes its own, which leaves the file with its entry
#[test]
fn the_file_is_busy_while_the_holder_lives_and_an_entry_takes_its_record_out() {
  let directory = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(directory.path());
  let mut child = sleeping_child();
  let holder = ProcessIdentity {
    pid: child.id(),
    started: process_started(child.id()),
    host: sync_utils::process_scan::host_name().to_string(),
  };
  let held = record("bd", &holder, &safe_push("auth"), unix_now());
  registry.change(|file| file.operations.push(held)).unwrap().unwrap();

  let busy = registry.admit(&id("app"), HolderTool::App, &session("user"), &safe_push("auth")).unwrap_err();
  assert_eq!(busy.code, ErrorCode::Busy, "{busy:?}");
  let Some(ErrorDetails::Busy { holder: Some(named) }) = &busy.details else {
    panic!("no holder: {busy:?}");
  };
  assert_eq!((named.op.as_str(), named.pid, named.kind), ("bd", child.id(), OperationKind::SafePush));
  assert!(
    busy.message.contains(&format!("`bd` (pid {}) has run the Safe Push of auth", child.id())),
    "{}",
    busy.message
  );
  // another subject is admitted beside it
  let docs = registry.admit(&id("docs"), HolderTool::App, &session("user"), &safe_push("docs")).unwrap();

  child.kill().unwrap();
  child.wait().unwrap();
  let auth = registry.admit(&id("app"), HolderTool::App, &session("user"), &safe_push("auth")).unwrap();
  let listed = |registry: &ProcessRegistry| registry.records().unwrap().unwrap().into_iter().map(|record| record.id.0).collect::<Vec<_>>();
  assert_eq!(listed(&registry), ["docs", "app"]);
  assert_eq!(auth.id(), &id("app"));
  drop((auth, docs));
  assert_eq!(listed(&registry), Vec::<String>::new());
}

/// A registry of another schema, or one that does not parse, is of another Branch Deck: every admission is busy, with no
/// holder it can name, and the file stays as it was
#[test]
fn a_registry_of_another_schema_is_busy_and_never_written() {
  let directory = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(directory.path());
  std::fs::create_dir_all(registry.path().parent().unwrap()).unwrap();
  for content in [
    r#"{"schema": 2, "operations": [{"id": "new"}]}"#,
    r#"{"schema": 1, "operations": [{"what": 1}]}"#,
    "not json",
  ] {
    std::fs::write(registry.path(), content).unwrap();
    let busy = registry.admit(&id("app"), HolderTool::App, &session("user"), &safe_push("auth")).unwrap_err();
    assert_eq!((busy.code, busy.details), (ErrorCode::Busy, Some(ErrorDetails::Busy { holder: None })), "{content}");
    assert_eq!(std::fs::read_to_string(registry.path()).unwrap(), content);
    assert_eq!(registry.records().unwrap(), None);
  }
}

/// The start of `branch` through the API, sent at `sent_at`
pub(crate) fn sent(branch: &str, sent_at: u32) -> PendingStart {
  PendingStart {
    source_ref: format!("refs/heads/user/virtual/{branch}"),
    tip: "1a2b3c".to_string(),
    route: StartRoute::Api { profile: None },
    pr: None,
    comment_id: None,
    sent_at,
    url: Some("https://patronus.example".to_string()),
  }
}

/// A start of this process whose answer was lost runs no more: its record stays after its entry, with what it sent, and
/// holds what it held for every admission, this process's own too (named so, with how to let it go); a session shows it
/// as such (`StartUnknown`), and the heartbeat leaves it as it is
#[test]
fn a_start_of_this_process_whose_answer_was_lost_holds_its_own_admissions_after_its_entry() {
  let directory = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(directory.path());
  let entry = registry.admit(&id("lost"), HolderTool::App, &session("user"), &safe_push("auth")).unwrap();
  let handle = entry.handle();
  handle.send_start(sent("auth", NOW)).unwrap();
  handle.unknown().unwrap();
  drop(entry);
  let records = registry.records().unwrap().unwrap();
  assert_eq!(
    (
      records.len(),
      records[0].status,
      records[0].step.clone(),
      records[0].pending_start().map(|start| start.sent_at)
    ),
    (1, RecordStatus::Unknown, OperationStep::Starting, Some(NOW))
  );

  let busy = registry.admit(&id("again"), HolderTool::App, &session("user"), &safe_push("auth")).unwrap_err();
  let Some(ErrorDetails::Busy { holder: Some(holder) }) = &busy.details else {
    panic!("{busy:?}")
  };
  assert_eq!((holder.op.as_str(), holder.unknown), ("lost", true));
  assert!(busy.message.starts_with("Cannot tell whether the Safe Push of auth that Branch Deck"), "{}", busy.message);
  assert!(busy.message.contains("`bd operation forget lost`"), "{}", busy.message);

  let shown = registry.of_other_processes().unwrap();
  assert_eq!(
    shown.iter().map(|record| record.running().step).collect::<Vec<_>>(),
    [OperationStep::StartUnknown {
      sent_at: NOW,
      url: Some("https://patronus.example".to_string()),
      pull_request: None,
    }]
  );
  let beaten = records[0].heartbeat_at;
  let mut kept = file(records);
  beat(&mut kept, ProcessIdentity::this(), &HashSet::new(), NOW + 30);
  assert_eq!((ids(&kept), kept.operations[0].heartbeat_at), (vec!["lost"], beaten));
}

/// Forget takes a start whose answer was lost out, of any process, and the admission it held back goes in; it refuses
/// an operation whose process still runs it and an id the registry lacks. A settle takes out only the start it read: one
/// sent again under the same id since stays
#[test]
fn a_forget_takes_out_a_start_whose_answer_was_lost_and_refuses_one_that_runs() {
  let directory = tempfile::tempdir().unwrap();
  let registry = ProcessRegistry::of(directory.path());
  let gone = process(4_000_000, Some(1));
  let mut lost = Record {
    status: RecordStatus::Unknown,
    external: Some(External {
      pending_start: Some(sent("auth", NOW)),
    }),
    ..record("lost", &gone, &safe_push("auth"), NOW)
  };
  registry.insert_of_another_process(lost.clone()).unwrap();
  let running = registry.admit(&id("docs"), HolderTool::App, &session("user"), &safe_push("docs")).unwrap();

  let refused = registry.forget(&id("docs")).unwrap_err();
  assert_eq!(refused.code, ErrorCode::InvalidArguments, "{refused:?}");
  assert_eq!(registry.forget(&id("none")).unwrap_err().code, ErrorCode::InvalidArguments);

  lost.external = Some(External {
    pending_start: Some(sent("auth", NOW + 1)),
  });
  assert!(!registry.settle(&lost, JournalTransition::NotStarted).unwrap());
  assert_eq!(registry.forget(&id("lost")).unwrap().id, id("lost"));
  let auth = registry.admit(&id("auth"), HolderTool::App, &session("user"), &safe_push("auth")).unwrap();
  drop((auth, running));
  assert_eq!(registry.records().unwrap().unwrap(), Vec::<Record>::new());
}
