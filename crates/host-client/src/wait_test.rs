use super::{WaitRequest, Waited};
use crate::record::{RunEnd, RunFiles, RunRecord};
use crate::testkit::{ALIAS, FakeProcesses, Said, Setup, calm};
use git_ops::lease_slot::STATE_DIR;
use host_link::channel::Channel;
use host_link::proto::{FileKind, Phase, Piece, Reply, Request, RunInfo, RunRefusal, Stream};
use host_link::refusal::{Code, Refusal};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use test_utils::time_shape::is_shape;

const RUN: &str = "1-2";

/// The state of the run on the agent for each `Runs` in their order, the last one for good, with the heavy turn of the
/// host
type States = Vec<(RunInfo, Option<Value>)>;

fn info(phases: &[&str], exit: Option<i32>) -> RunInfo {
  RunInfo {
    run: RUN.to_owned(),
    affinity: "checkout-0123456789ab".to_owned(),
    commit: "c".repeat(40),
    command: vec!["cargo".to_owned(), "test".to_owned()],
    started_ms: 0,
    ended_ms: exit.map(|_| 1),
    exit,
    phases: phases.iter().map(|name| Phase { name: (*name).to_owned(), ms: 1 }).collect(),
    fetch: vec!["out/bt-runs".to_owned()],
    forgotten: false,
    refused: None,
    cores: None,
  }
}

fn step(name: &str, ms: u64) -> Phase {
  Phase { name: name.to_owned(), ms }
}

/// What the agent of a test counts, and what it holds back
#[derive(Default)]
struct Agent {
  /// The `Runs` that came so far
  polls: AtomicUsize,
  /// The `Fetch` that came so far
  fetches: AtomicUsize,
  /// `Runs` answers the first state, as for a run that goes on
  held: AtomicBool,
  /// A fetch refuses
  fetch_fails: AtomicBool,
  /// The kinds of the requests in their order
  kinds: Mutex<Vec<&'static str>>,
}

impl Agent {
  fn kinds(&self) -> Vec<&'static str> {
    self.kinds.lock().unwrap_or_else(PoisonError::into_inner).clone()
  }
}

/// The end of a run that the follow sends after the steps of the host: its exit code or its refusal
#[derive(Clone, Copy)]
enum End {
  Exit(i32),
  Refused(Code, &'static str),
}

/// An agent whose run goes through `states`, and whose follow sends one line of each stream, the steps and `end`
fn agent(states: States, end: End) -> (Setup, Arc<Agent>) {
  let counts = Arc::new(Agent::default());
  let setup = Setup::new({
    let counts = Arc::clone(&counts);
    move |request, channel| answer(request, channel, &states, end, &counts)
  });
  (setup, counts)
}

fn answer(request: &Request, channel: &mut Channel, states: &States, end: End, counts: &Agent) {
  let kind = match request {
    Request::Runs => "runs",
    Request::Follow { .. } => "follow",
    Request::Fetch { .. } => "fetch",
    Request::Forget { .. } => "forget",
    Request::Cancel { .. } => "cancel",
    _ => "other",
  };
  counts.kinds.lock().unwrap_or_else(PoisonError::into_inner).push(kind);
  let _ = match request {
    Request::Runs => {
      let poll = counts.polls.fetch_add(1, Ordering::SeqCst);
      let index = if counts.held.load(Ordering::SeqCst) { 0 } else { poll.min(states.len() - 1) };
      let (info, heavy) = states[index].clone();
      channel.send(&Reply::Runs { runs: vec![info], heavy })
    }
    Request::Follow { .. } => {
      let _ = channel.send(&Reply::Output {
        stream: Stream::Stdout,
        piece: Piece::of(b"test a ... ok", false),
        end: 10,
      });
      let _ = channel.send(&Reply::Output {
        stream: Stream::Stderr,
        piece: Piece::of(b"test result: FAILED", false),
        end: 20,
      });
      let _ = channel.send(&Reply::Phases {
        phases: vec![step("heavy", 1000), step("slot", 100), step("command", 2000)],
      });
      match end {
        End::Exit(code) => channel.send(&Reply::Exit { code }),
        End::Refused(code, message) => channel.send(&Reply::refused(code, message)),
      }
    }
    Request::Fetch { .. } if counts.fetch_fails.load(Ordering::SeqCst) => {
      counts.fetches.fetch_add(1, Ordering::SeqCst);
      channel.send(&Reply::refused(Code::AgentError, "the stage is gone"))
    }
    Request::Fetch { .. } => {
      counts.fetches.fetch_add(1, Ordering::SeqCst);
      let bytes = b"hello";
      let _ = channel.send(&Reply::File {
        path: "out/bt-runs/1/test.log".to_owned(),
        size: bytes.len() as u64,
        executable: false,
        kind: FileKind::Staged,
      });
      let _ = channel.send_bytes(bytes);
      let _ = channel.send_end();
      channel.send(&Reply::Fetched { files: 1, bytes: 5 })
    }
    Request::Forget { .. } => channel.send(&Reply::Done { message: "forgot".to_owned() }),
    _ => channel.send(&Reply::refused(Code::BadRequest, "not in this script")),
  };
}

/// The run files of the checkout of `setup`
fn files(setup: &Setup) -> RunFiles {
  RunFiles::new(&setup.repo.path().join(".git").join(STATE_DIR).join("host/runs")).unwrap()
}

/// Writes the record of the run: its files of `out/bt-runs` come back
fn record(setup: &Setup) {
  let record = RunRecord {
    commit: "c".repeat(40),
    fetch: vec!["out/bt-runs".to_owned()],
    ..RunRecord::default()
  };
  files(setup).write(RUN, &record).unwrap();
}

/// A wait of the run in the checkout of `setup`, whose `--max` passes when `past_max` says so
fn wait(setup: &Setup, say: &(dyn Fn(&str) + Sync), presses: &(dyn Fn() -> u32 + Sync), past_max: &(dyn Fn() -> bool + Sync)) -> Waited {
  let processes = FakeProcesses::default();
  setup.client(&processes, say, presses).with_past_max(past_max).wait(&WaitRequest {
    checkout: setup.repo.path(),
    alias: ALIAS,
    run: RUN,
    max: Duration::from_mins(1),
  })
}

const fn never() -> bool {
  false
}

/// The words of each state line after its time: `run 1-2 runs its command on studio`
fn states_said(lines: &[String]) -> Vec<String> {
  lines
    .iter()
    .filter_map(|line| line.strip_prefix("brd-host: "))
    .filter(|line| line.as_bytes().get(2) == Some(&b':') && line.as_bytes().get(5) == Some(&b':'))
    .map(|line| line[9..].to_owned())
    .collect()
}

fn log_of(setup: &Setup) -> PathBuf {
  files(setup).log(RUN)
}

/// A wait says each state of the run once with its time, the first one at once; at the end of the run it takes the
/// output into the log of the run, copies the files that the record asks for, forgets the run, writes its end and says
/// the summary with the last lines of the output; it exits with the exit code of the command
#[test]
fn a_wait_says_each_state_of_the_run_then_takes_its_log_fetches_and_forgets_it_by_its_record() {
  let queued = json!({"holders": [{"run": "0-1", "pid": 1}], "queue": [{"run": "0-2", "pid": 2}, {"run": RUN, "pid": 3}]});
  let states = vec![
    (info(&[], None), Some(queued.clone())),
    (info(&[], None), Some(queued)),
    (info(&["heavy", "slot"], None), None),
    (info(&["heavy", "slot", "pack_wait", "checkout", "prepare"], None), None),
    (info(&["heavy", "slot", "pack_wait", "checkout", "prepare", "command", "measure", "stage"], Some(3)), None),
  ];
  let (setup, agent) = agent(states, End::Exit(3));
  record(&setup);
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let waited = wait(&setup, &say, &calm, &never);
  assert_eq!(waited, Waited { exit: 3, refusal: None });
  let lines = said.lines();
  assert_eq!(
    states_said(&lines),
    [
      "run 1-2 waits for the heavy turn of studio, 1 run ahead: `brd-host runs studio` names the runs that hold it",
      "run 1-2 checks out its commit and prepares on studio",
      "run 1-2 runs its command on studio",
      "run 1-2 ended on studio",
    ],
    "{lines:#?}"
  );
  let kinds = agent.kinds();
  assert_eq!(kinds[kinds.len() - 3..], ["follow", "fetch", "forget"]);
  assert_eq!(std::fs::read_to_string(log_of(&setup)).unwrap(), "test a ... ok\ntest result: FAILED\n");
  assert_eq!(std::fs::read_to_string(setup.repo.path().join("out/bt-runs/1/test.log")).unwrap(), "hello");
  let end = files(&setup).read_end(RUN).unwrap().expect("the end of the run");
  assert_eq!((end.exit, end.command_exit, end.forgotten), (3, Some(3), true));
  assert_eq!(end.fetched, ["out/bt-runs/1/test.log"]);
  let summary: Vec<&str> = lines.iter().skip_while(|line| !line.contains("exited with")).map(String::as_str).collect();
  let root = setup.repo.path().canonicalize().unwrap();
  let output_line = format!("brd-host: the output of the run: {} (2 lines)", root.join(".git/branch-deck/host/runs/1-2.log").display());
  let copied_line = format!("brd-host: copied 1 files of the run 1-2 into {}", root.display());
  // the fetch is a step of this client, measured: its time and the total are any duration
  assert!(is_shape(summary[0], "brd-host: run 1-2 on studio exited with 3 after <time> on the host"), "{}", summary[0]);
  assert!(
    is_shape(summary[1], "brd-host: phases: heavy 1.0s, slot 0.1s, command 2.0s, fetch <time>"),
    "{}",
    summary[1]
  );
  assert_eq!(
    summary[2..],
    [
      copied_line.as_str(),
      output_line.as_str(),
      "brd-host: its last 2 lines:",
      "test a ... ok",
      "test result: FAILED",
    ]
  );
}

/// Past its `--max` a wait leaves the run going with `still_running` (124) and the state it saw; it sends no cancel and
/// finishes nothing. The next wait says the state at once and ends the run
#[test]
fn a_wait_past_its_max_leaves_the_run_going_and_exits_124_and_the_next_one_ends_it() {
  let states = vec![(info(&["slot", "prepare"], None), None), (info(&["slot", "prepare", "command"], Some(0)), None)];
  let (setup, agent) = agent(states, End::Exit(0));
  record(&setup);
  agent.held.store(true, Ordering::SeqCst);
  let said = Said::default();
  let say = |line: &str| said.push(line);
  let past = || agent.polls.load(Ordering::SeqCst) >= 2;
  let waited = wait(&setup, &say, &calm, &past);
  let refusal = waited.refusal.expect("the run goes on");
  assert_eq!((waited.exit, refusal.code), (124, Code::StillRunning), "{}", refusal.message);
  assert!(
    refusal
      .message
      .contains("and it runs its command; nothing was cancelled: `brd-host wait studio 1-2` waits again"),
    "{}",
    refusal.message
  );
  assert_eq!(
    states_said(&said.lines()),
    ["run 1-2 runs its command on studio"],
    "one line for the state, whatever the polls"
  );
  assert!(agent.kinds().iter().all(|kind| *kind == "runs"), "no follow, fetch, forget or cancel: {:?}", agent.kinds());
  assert!(files(&setup).read_end(RUN).unwrap().is_none(), "nothing was finished");

  agent.held.store(false, Ordering::SeqCst);
  let said = Said::default();
  let say = |line: &str| said.push(line);
  assert_eq!(wait(&setup, &say, &calm, &never), Waited { exit: 0, refusal: None });
  assert_eq!(states_said(&said.lines()), ["run 1-2 ended on studio"]);
  assert_eq!(agent.kinds().last(), Some(&"forget"));
}

/// A host that never answers in the whole `--max` of a wait is `timed_out` (255), never `still_running`: a host that is
/// down must not keep an agent waiting call after call
#[test]
fn a_wait_whose_host_never_answers_exits_255_with_timed_out() {
  let (setup, _) = agent(vec![(info(&[], None), None)], End::Exit(0));
  setup.agent.stopper().stop();
  let processes = FakeProcesses::default();
  let waited = setup.client(&processes, &|_| {}, &calm).wait(&WaitRequest {
    checkout: setup.repo.path(),
    alias: ALIAS,
    run: RUN,
    max: Duration::from_millis(50),
  });
  let refusal = waited.refusal.expect("no answer");
  assert_eq!((waited.exit, refusal.code), (255, Code::TimedOut), "{}", refusal.message);
  assert!(refusal.message.contains("`brd-host wait studio 1-2` asks again"), "{}", refusal.message);
}

/// A wait after the end of the run answers from its end file and asks the host only to forget a run that an earlier call
/// could not forget; once forgotten, a wait asks the host nothing
#[test]
fn a_second_wait_after_the_end_answers_from_the_end_file_and_only_forgets() {
  let (setup, agent) = agent(vec![(info(&[], None), None)], End::Exit(0));
  let end = RunEnd {
    exit: 1,
    command_exit: Some(1),
    phases: vec![step("command", 1500)],
    fetched: vec!["out/bt-runs/1/test.log".to_owned()],
    ..RunEnd::default()
  };
  files(&setup).write_end(RUN, &end).unwrap();
  std::fs::write(log_of(&setup), "one\ntwo\n").unwrap();
  let said = Said::default();
  let say = |line: &str| said.push(line);
  assert_eq!(wait(&setup, &say, &calm, &never), Waited { exit: 1, refusal: None });
  assert_eq!(agent.kinds(), ["forget"]);
  assert!(files(&setup).read_end(RUN).unwrap().unwrap().forgotten);
  let lines = said.lines();
  assert!(lines.contains(&"brd-host: run 1-2 on studio exited with 1 after 1.5s on the host".to_owned()), "{lines:#?}");
  assert!(
    lines.ends_with(&["brd-host: its last 2 lines:".to_owned(), "one".to_owned(), "two".to_owned()]),
    "{lines:#?}"
  );

  assert_eq!(wait(&setup, &|_| {}, &calm, &never), Waited { exit: 1, refusal: None });
  assert_eq!(agent.kinds(), ["forget"], "the third call asks the host nothing");
}

/// While another process of the checkout holds the lock of the end of the run, such as the `brd-host run` of the run, a
/// wait finishes nothing and leaves with `still_running` at its `--max`
#[test]
fn a_wait_while_another_process_finishes_the_run_exits_124_and_asks_nothing() {
  let (setup, agent) = agent(vec![(info(&[], Some(0)), None)], End::Exit(0));
  record(&setup);
  let files = files(&setup);
  let _held = files.lock(RUN, None, Duration::from_millis(1), &|| false).unwrap().unwrap();
  let waited = wait(&setup, &|_| {}, &calm, &|| true);
  let refusal = waited.refusal.expect("held");
  assert_eq!((waited.exit, refusal.code), (124, Code::StillRunning));
  assert!(
    refusal.message.starts_with("another brd-host process of this checkout finishes the run 1-2"),
    "{}",
    refusal.message
  );
  assert!(agent.kinds().is_empty(), "{:?}", agent.kinds());
}

/// A fetch that fails leaves no end file and keeps the run on the host, so the next wait copies its files again and then
/// forgets it
#[test]
fn a_wait_whose_fetch_failed_copies_the_files_again_on_the_next_wait() {
  let (setup, agent) = agent(vec![(info(&["command"], Some(0)), None)], End::Exit(0));
  record(&setup);
  agent.fetch_fails.store(true, Ordering::SeqCst);
  let waited = wait(&setup, &|_| {}, &calm, &never);
  let refusal = waited.refusal.expect("the fetch failed");
  assert_eq!((waited.exit, refusal.code), (255, Code::AgentError), "{}", refusal.message);
  assert!(refusal.message.contains("`brd-host wait studio 1-2` copies them again"), "{}", refusal.message);
  assert!(files(&setup).read_end(RUN).unwrap().is_none());
  assert!(!agent.kinds().contains(&"forget"));

  agent.fetch_fails.store(false, Ordering::SeqCst);
  assert_eq!(wait(&setup, &|_| {}, &calm, &never), Waited { exit: 0, refusal: None });
  assert_eq!(agent.fetches.load(Ordering::SeqCst), 2);
  assert_eq!(agent.kinds().last(), Some(&"forget"));
  assert!(setup.repo.path().join("out/bt-runs/1/test.log").is_file());
}

/// A checkout without the record of the run says so, takes its output into the log but none of its files, and leaves
/// the run to its own checkout
#[test]
fn a_wait_in_another_checkout_says_so_and_fetches_nothing() {
  let (setup, agent) = agent(vec![(info(&["command"], Some(2)), None)], End::Exit(2));
  let said = Said::default();
  let say = |line: &str| said.push(line);
  assert_eq!(wait(&setup, &say, &calm, &never), Waited { exit: 2, refusal: None });
  assert!(said.lines().iter().any(|line| line.contains("has no record of the run 1-2")), "{:#?}", said.lines());
  assert_eq!(agent.kinds(), ["runs", "follow"]);
  assert!(log_of(&setup).is_file());
}

/// A run that a later run superseded exits 75 with the refusal of the agent, as its `brd-host run` would, brings no
/// files and is forgotten
#[test]
fn a_superseded_run_exits_75_with_the_refusal_of_the_agent() {
  let mut superseded = info(&["heavy"], Some(75));
  superseded.refused = Some(RunRefusal {
    code: "superseded".to_owned(),
    message: "run 1-2 was superseded by run 3-3".to_owned(),
  });
  let (setup, agent) = agent(vec![(superseded, None)], End::Refused(Code::Superseded, "run 1-2 was superseded by run 3-3"));
  record(&setup);
  let waited = wait(&setup, &|_| {}, &calm, &never);
  assert_eq!(waited.exit, 75);
  assert_eq!(waited.refusal.map(|refusal: Refusal| refusal.code), Some(Code::Superseded));
  assert_eq!(agent.kinds(), ["runs", "follow", "forget"]);
}

/// Ctrl+C, SIGTERM or SIGHUP leaves a wait with 130; it sends no cancel, and the run goes on
#[test]
fn ctrl_c_leaves_a_wait_and_the_run_goes_on() {
  let (setup, agent) = agent(vec![(info(&["slot", "prepare"], None), None)], End::Exit(0));
  record(&setup);
  let presses = || u32::from(agent.polls.load(Ordering::SeqCst) >= 1);
  let waited = wait(&setup, &|_| {}, &presses, &never);
  let refusal = waited.refusal.expect("left");
  assert_eq!((waited.exit, refusal.code), (130, Code::Interrupted));
  assert_eq!(
    refusal.message,
    "Ctrl+C, SIGTERM or SIGHUP left the wait of the run 1-2, which goes on on studio: `brd-host wait studio 1-2` waits again"
  );
  assert!(!agent.kinds().contains(&"cancel"));
}
