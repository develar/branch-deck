//! The process tests of the command line of brd-host (openspec/specs/cli/host/spec.md): a real `brd-host serve` on a loopback port
//! with a temporary home, a `TestRepo` whose host slots the keeper fills in the pool of that home, and the client
//! commands of the same binary in that home. The repository is the checkout of the client and the repository of the
//! host at once, so the host has every commit and no pack travels
#![cfg(unix)]
#![expect(clippy::tests_outside_test_module, reason = "a Cargo integration test has no #[cfg(test)] module")]

use host_agent::serve::{PROCESS_TEST_ENV, spawn_on_loopback};
use host_agent::teardown::settle_runs;
use host_link::allow::AllowList;
use host_link::identity::Identity;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use test_utils::git_test_utils::{TestRepo, append_config};

const BD_HOST: &str = env!("CARGO_BIN_EXE_brd-host");

/// The alias of the host file of the tests
const ALIAS: &str = "local";

/// A temporary home with the identity of a client in the allow list of the agent, and a repository with one host slot
struct World {
  home: tempfile::TempDir,
  repo: TestRepo,
}

impl Drop for World {
  /// Before the home and the repository go: the supervisor of a run gives its slot back after `exit.json`, in a session
  /// of its own that the kill of `serve` does not end
  fn drop(&mut self) {
    settle_runs(self.home.path(), &[self.repo.path()]);
  }
}

impl World {
  fn new() -> Self {
    let home = tempfile::tempdir().expect("a temporary home");
    let repo = TestRepo::new();
    repo.create_commit_with_files("first", &[(".gitignore", "out/\n")]);
    append_config(repo.path(), &[("branchdeck.hostSlots", "1")]);
    let client = Identity::load_or_create(&home.path().join(".config/brd-host")).expect("the client identity");
    let mut allow = AllowList::default();
    allow.add(client.fingerprint(), "test").expect("a name");
    allow.write(&home.path().join(".config/brd-host/agent/clients")).expect("the allow list");
    Self { home, repo }
  }

  /// `brd-host` with the environment of the tests, in the repository
  fn command(&self) -> Command {
    self.command_of(BD_HOST)
  }

  /// `program` with the environment of the tests, in the repository
  fn command_of(&self, program: &str) -> Command {
    let mut command = Command::new(program);
    command
      .current_dir(self.repo.path())
      .env("HOME", self.home.path())
      .envs(PROCESS_TEST_ENV)
      .stdin(Stdio::null());
    command
  }

  /// `brd-host <args>` to its end
  fn bd_host(&self, args: &[&str]) -> Output {
    self.command().args(args).output().expect("brd-host ran")
  }

  /// Starts `serve` on a loopback port, and writes the host file of `ALIAS` for it
  fn serve(&self) -> Agent {
    let log = fs::File::create(self.home.path().join("serve.log")).expect("the serve log");
    let (child, listening) = spawn_on_loopback(self.command(), 0, &[], log).expect("serve listens");
    let host_file = json!({
      "alias": ALIAS,
      "fingerprint": listening.fingerprint.to_string(),
      "port": listening.listening.port(),
      "hostname": "127.0.0.1",
      "addresses": [listening.listening.to_string()],
      "repo": self.repo.path_str(),
    });
    let hosts = self.home.path().join(".config/brd-host/hosts");
    fs::create_dir_all(&hosts).expect("the hosts directory");
    fs::write(hosts.join(format!("{ALIAS}.json")), host_file.to_string()).expect("the host file");
    Agent { child }
  }

  fn serve_log(&self) -> String {
    fs::read_to_string(self.home.path().join("serve.log")).unwrap_or_default()
  }
}

/// A running `serve`
struct Agent {
  child: Child,
}

impl Drop for Agent {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

fn text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

/// `brd-host <args>` in `dir` with the descriptor 3 open on `report`: the shell opens it, as bt does
fn run_with_report(world: &World, dir: &Path, report: &Path, args: &[&str]) -> Output {
  world
    .command_of("/bin/sh")
    .current_dir(dir)
    .arg("-c")
    .arg(r#"report="$1"; shift; exec "$0" "$@" 3>"$report""#)
    .arg(BD_HOST)
    .arg(report)
    .args(args)
    .output()
    .expect("brd-host ran")
}

fn report_of(path: &Path) -> Value {
  let text = fs::read_to_string(path).unwrap_or_else(|error| panic!("the report {}: {error}", path.display()));
  assert!(text.ends_with('\n') && text.lines().count() == 1, "one object and a line end: {text:?}");
  serde_json::from_str(&text).expect("the report is JSON")
}

/// Asks `done` every 100 ms for up to 60 s, and fails the test with `what` when it never answers `true`
fn wait_until(what: &str, done: impl Fn() -> bool) {
  let deadline = Instant::now() + Duration::from_mins(1);
  while !done() {
    assert!(Instant::now() < deadline, "{what} did not happen in 60 s");
    thread::sleep(Duration::from_millis(100));
  }
}

#[test]
fn stdout_holds_only_the_command_and_the_lines_of_brd_host_go_to_stderr() {
  let world = World::new();
  let _agent = world.serve();
  let report = world.home.path().join("report.json");
  let ran = run_with_report(
    &world,
    world.repo.path(),
    &report,
    &[
      "run",
      ALIAS,
      "--fetch",
      "out/bt-runs",
      "--report-fd",
      "3",
      "--",
      "/bin/sh",
      "-c",
      "echo out; echo err >&2; mkdir -p out/bt-runs/a && echo r > out/bt-runs/a/summary.json; exit 3",
    ],
  );
  let (stdout, stderr) = (text(&ran.stdout), text(&ran.stderr));
  assert_eq!(ran.status.code(), Some(3), "{stderr}\nserve: {}", world.serve_log());
  assert_eq!(stdout, "out\n", "stdout holds only the stdout of the command; stderr: {stderr}");
  assert!(stderr.lines().any(|line| line == "err"), "{stderr}");
  assert!(stderr.lines().any(|line| line.starts_with("brd-host: run ")), "{stderr}");
  assert!(stderr.lines().any(|line| line.starts_with("brd-host: running in ")), "{stderr}");
  assert!(stderr.lines().any(|line| line.starts_with("brd-host: phases: ")), "{stderr}");

  let report = report_of(&report);
  assert_eq!((report["exit"].as_u64(), report["command_exit"].as_i64()), (Some(3), Some(3)), "{report}");
  assert_eq!(report["refusal"], Value::Null);
  assert_eq!(report["working_tree"], Value::Bool(true));
  assert_eq!(report["fetched"], json!(["out/bt-runs/a/summary.json"]));
  assert_eq!(report["commit"].as_str(), Some(world.repo.head().as_str()), "a clean tree ships HEAD");
  let phases: Vec<&str> = report["phases"].as_array().expect("phases").iter().filter_map(|phase| phase["name"].as_str()).collect();
  assert_eq!(phases.first(), Some(&"slot"), "no pack travels: {phases:?}");
  assert_eq!(phases.last(), Some(&"total"), "{phases:?}");
  assert_eq!(fs::read_to_string(world.repo.path().join("out/bt-runs/a/summary.json")).expect("the fetched file"), "r\n");
}

/// The bound of one event of the log of the agent: a longer line of the command is several events there
const EVENT_BOUND: usize = 64 * 1024;

/// `brd-host run` of the shell line `script` in the repository, whose working tree has the file `printed` of `bytes`:
/// the run ships it, so the command prints it on the host
fn run_printing(world: &World, bytes: &[u8], script: &str) -> Output {
  fs::write(world.repo.path().join("printed"), bytes).expect("the file to print");
  world.bd_host(&["run", ALIAS, "--", "/bin/sh", "-c", script])
}

/// Fails at the first byte where `got` and `want` differ, with the bytes around it instead of all of them
fn assert_bytes(got: &[u8], want: &[u8], what: &str) {
  if got == want {
    return;
  }
  let first = got.iter().zip(want).position(|(got, want)| got != want).unwrap_or_else(|| got.len().min(want.len()));
  let around = |bytes: &[u8]| String::from_utf8_lossy(&bytes[first.saturating_sub(8).min(bytes.len())..(first + 8).min(bytes.len())]).into_owned();
  panic!(
    "{what}: {} bytes where {} are expected, the first difference at byte {first}: {:?} where {:?} is expected",
    got.len(),
    want.len(),
    around(got),
    around(want)
  );
}

/// The offset of `part` in `bytes`
fn find(bytes: &[u8], part: &[u8]) -> Option<usize> {
  bytes.windows(part.len()).position(|window| window == part)
}

/// One stdout line of 200 KiB without a line end is four events of the log of the agent, and stdout gets its bytes
/// without a line end between them or after them
#[test]
fn a_line_of_200_kib_without_a_line_end_reaches_stdout_byte_for_byte() {
  let world = World::new();
  let _agent = world.serve();
  let line: Vec<u8> = b"abcdefghijklmnopqrstuvwxyz".iter().copied().cycle().take(200 * 1024).collect();
  let ran = run_printing(&world, &line, "cat printed");
  assert_eq!(ran.status.code(), Some(0), "{}\nserve: {}", text(&ran.stderr), world.serve_log());
  assert_bytes(&ran.stdout, &line, "stdout");
}

/// A character of 4 bytes across the bound of an event, 1, 2 and 3 bytes before it, keeps its bytes on stdout and on
/// stderr
#[test]
fn a_character_across_the_bound_of_an_event_reaches_stdout_and_stderr_unchanged() {
  let world = World::new();
  let _agent = world.serve();
  let mut printed = Vec::new();
  for before in 1..=3 {
    printed.extend(std::iter::repeat_n(b'x', EVENT_BOUND - before));
    printed.extend("😀".as_bytes());
    printed.push(b'\n');
  }
  let ran = run_printing(&world, &printed, "cat printed; cat printed >&2");
  assert_eq!(ran.status.code(), Some(0), "{}\nserve: {}", text(&ran.stderr), world.serve_log());
  assert_bytes(&ran.stdout, &printed, "stdout");
  assert!(find(&ran.stderr, &printed).is_some(), "stderr has the bytes of the command: {}", text(&ran.stderr));
}

/// Short lines, an empty line, a line of CR LF, one line of 150 KiB, bytes that are not UTF-8 and a last line without
/// a line end keep every byte and every line end. On stderr the line of the phases starts a line of its own after the
/// last line of the command
#[test]
fn short_lines_and_a_long_line_keep_every_line_end() {
  let world = World::new();
  let _agent = world.serve();
  let mut printed = b"one\ntwo\r\n\nthree\n".to_vec();
  printed.extend(std::iter::repeat_n(b'y', 150 * 1024));
  printed.extend(b"\nfour\n\xff\xfe not UTF-8\nlast");
  let ran = run_printing(&world, &printed, "cat printed; cat printed >&2");
  assert_eq!(ran.status.code(), Some(0), "{}\nserve: {}", text(&ran.stderr), world.serve_log());
  assert_bytes(&ran.stdout, &printed, "stdout");
  let at = find(&ran.stderr, &printed).unwrap_or_else(|| panic!("stderr has the bytes of the command: {}", text(&ran.stderr)));
  let after = text(&ran.stderr[at + printed.len()..]);
  assert!(
    after.starts_with("\nbrd-host: the command used ") && after.contains(" of the host\nbrd-host: phases: "),
    "the lines of the agent start lines of their own: {after:?}"
  );
}

/// The client of a run and the hold file of its command, which waits while the file exists. The drop kills the client
/// and removes the file, and so does the removal of the temporary home after a failed test. So no process stays: the
/// supervisor has a session of its own, and the drop of `serve` does not stop it
struct Held {
  client: Child,
  hold: PathBuf,
}

impl Drop for Held {
  fn drop(&mut self) {
    let _ = self.client.kill();
    let _ = self.client.wait();
    let _ = fs::remove_file(&self.hold);
  }
}

/// Starts `brd-host run <args> -- /bin/sh -c '<before>; …; <after>'`, whose command holds while the hold file of the home
/// exists. A test that the runner kills leaves the command for at most 60 s
fn start_a_held_run(world: &World, args: &[&str], before: &str, after: &str) -> Held {
  let hold = world.home.path().join("hold");
  fs::write(&hold, "").expect("the hold file");
  let script = format!(
    "{before}; i=0; while [ -e '{}' ] && [ $i -lt 1200 ]; do sleep 0.05; i=$((i+1)); done; {after}",
    hold.display()
  );
  let client = world
    .command()
    .args(["run", ALIAS])
    .args(args)
    .args(["--", "/bin/sh", "-c", &script])
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()
    .expect("a run");
  Held { client, hold }
}

/// `brd-host follow` of a run that ended after its client went gives the bytes of its stdout. They are a carriage
/// return, a character across the bound of an event, and a last line without a line end. That line came after the
/// client went
#[test]
fn follow_of_an_ended_run_gives_the_bytes_of_its_stdout() {
  let world = World::new();
  let _agent = world.serve();
  let mut printed = b"short\r\n".to_vec();
  printed.extend(std::iter::repeat_n(b'z', EVENT_BOUND - 2));
  printed.extend("😀\n".as_bytes());
  fs::write(world.repo.path().join("printed"), &printed).expect("the file to print");
  // the last line has no line end, so it comes only at the end of the stream, after the hold
  let mut held = start_a_held_run(&world, &[], "cat printed", "printf last");
  let mut got = vec![0; printed.len()];
  let read = held.client.stdout.take().expect("the stdout").read_exact(&mut got);
  // the client goes, and the run goes on: nothing forgets it on the host
  drop(held);
  read.unwrap_or_else(|error| panic!("the stdout of the run: {error}\nserve: {}", world.serve_log()));
  assert_bytes(&got, &printed, "the stdout of run");

  // the id of the run once it ended with 0
  let ended_run = || {
    let listed: Value = serde_json::from_slice(&world.bd_host(&["runs", ALIAS, "--json"]).stdout).ok()?;
    let run = &listed["runs"][0];
    if run["exit"] == json!(0) { run["run"].as_str().map(str::to_owned) } else { None }
  };
  wait_until("the end of the run", || ended_run().is_some());
  let id = ended_run().expect("the ended run");
  let followed = world.bd_host(&["follow", ALIAS, &id]);
  assert_eq!(followed.status.code(), Some(0), "{}\nserve: {}", text(&followed.stderr), world.serve_log());
  printed.extend(b"last");
  assert_bytes(&followed.stdout, &printed, "the stdout of follow");
  assert!(text(&followed.stderr).ends_with('\n'), "{}", text(&followed.stderr));
}

#[test]
fn the_report_goes_to_its_descriptor_on_every_way_out() {
  let world = World::new();
  // no descriptor 9: a usage refusal before anything runs, and no report
  let ran = world.bd_host(&["run", ALIAS, "--report-fd", "9", "--", "true"]);
  assert_eq!(ran.status.code(), Some(2), "{}", text(&ran.stderr));
  assert!(text(&ran.stderr).contains("brd-host: usage: --report-fd 9"), "{}", text(&ran.stderr));

  // no host file: a refusal before the start, reported
  let report = world.home.path().join("report.json");
  let ran = run_with_report(&world, world.repo.path(), &report, &["run", ALIAS, "--head", "--report-fd", "3", "--", "true"]);
  assert_eq!(ran.status.code(), Some(75), "{}", text(&ran.stderr));
  assert_eq!(text(&ran.stdout), "");
  let report = report_of(&report);
  assert_eq!(report["refusal"]["code"].as_str(), Some("no_host"), "{report}");
  assert_eq!((report["exit"].as_u64(), report["command_exit"].clone()), (Some(75), Value::Null));
  assert_eq!(report["working_tree"], Value::Bool(false));

  // a working directory in no checkout: `usage`, reported
  let outside = tempfile::tempdir().expect("a directory");
  let path = outside.path().join("report.json");
  let ran = run_with_report(&world, outside.path(), &path, &["run", ALIAS, "--report-fd", "3", "--", "true"]);
  assert_eq!(ran.status.code(), Some(2), "{}", text(&ran.stderr));
  assert_eq!(report_of(&path)["refusal"]["code"].as_str(), Some("usage"));
}

/// Starts `brd-host run` of a command that waits for SIGINT up to 60 s, and answers once its first line came. One `sleep`
/// until the cancel, where a loop of short ones started a process every 50 ms: SIGINT reaches the group, so `sleep` dies
/// of it and sh runs the trap. The trap makes the end of the command certain: without it, sh goes on when SIGINT comes
/// while it reaps a child that has exited 0, as it takes the signal as handled by its child. A failed test leaves the
/// command for at most 60 s
fn start_a_run_that_waits(world: &World, stderr: Stdio) -> Child {
  start_a_run_that_waits_in(world, world.repo.path(), stderr)
}

/// `start_a_run_that_waits` of the checkout `dir`
fn start_a_run_that_waits_in(world: &World, dir: &Path, stderr: Stdio) -> Child {
  let mut run = world
    .command()
    .current_dir(dir)
    .args(["run", ALIAS, "--head", "--", "/bin/sh", "-c", "trap 'exit 130' INT; echo started; sleep 60"])
    .stdout(Stdio::piped())
    .stderr(stderr)
    .spawn()
    .expect("a run");
  let mut first = String::new();
  BufReader::new(run.stdout.take().expect("the stdout")).read_line(&mut first).expect("the first line");
  assert_eq!(first, "started\n", "serve: {}", world.serve_log());
  run
}

#[test]
fn follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent() {
  let world = World::new();
  let _agent = world.serve();
  let run = start_a_run_that_waits(&world, Stdio::piped());

  let runs = world.bd_host(&["runs", ALIAS, "--json"]);
  assert_eq!(runs.status.code(), Some(0), "{}", text(&runs.stderr));
  let listed: Value = serde_json::from_slice(&runs.stdout).expect("JSON");
  let id = listed["runs"][0]["run"].as_str().expect("the run that goes on").to_owned();
  assert_eq!(listed["runs"][0]["exit"], Value::Null, "{listed}");
  assert_eq!(
    (&listed["heavy"]["holders"], &listed["heavy"]["queue"]),
    (&json!([]), &json!([])),
    "a run that is not heavy takes no turn: {listed}"
  );
  let text_runs = world.bd_host(&["runs", ALIAS]);
  assert!(text(&text_runs.stdout).starts_with(&format!("{id}  running, started ")), "{}", text(&text_runs.stdout));
  // a run that is not heavy is in the turn for the time of its command, from its start
  let turn = text(&text_runs.stdout);
  assert!(turn.contains("\nheavy turn: free"), "{turn}");
  assert!(turn.contains(&format!("\n  run {id} of ")) && turn.contains(", not heavy, uses 0.0: /bin/sh -c "), "{turn}");

  let fetch = world.bd_host(&["fetch", ALIAS, &id, "--fetch", "out"]);
  assert_eq!(fetch.status.code(), Some(255), "a fetch of a run that goes on");
  assert!(text(&fetch.stderr).contains("brd-host: bad_request: "), "{}", text(&fetch.stderr));

  let cancel = world.bd_host(&["cancel", ALIAS, &id]);
  assert_eq!(cancel.status.code(), Some(0), "{}", text(&cancel.stderr));
  let ended = run.wait_with_output().expect("the run ended");
  let run_dir = world.home.path().join(".cache/brd-host/runs").join(&id);
  assert_eq!(
    ended.status.code(),
    Some(130),
    "SIGINT ends the command; {}\nsupervisor: {}\nevents: {}\nfiles: {:?}",
    text(&ended.stderr),
    fs::read_to_string(run_dir.join("supervisor.log")).unwrap_or_default(),
    fs::read_to_string(run_dir.join("events.log")).unwrap_or_default(),
    fs::read_dir(&run_dir).map_or_default(|entries| entries.flatten().map(|entry| entry.file_name()).collect::<Vec<_>>())
  );

  // the run saw its end and forgot itself on the host, which keeps its end: a follow answers it, a forget says so, and
  // the runs list it as finished
  let follow = world.bd_host(&["follow", ALIAS, &id]);
  assert_eq!(follow.status.code(), Some(130), "{}", text(&follow.stderr));
  assert_eq!(text(&follow.stdout), "started\n", "the output of the forgotten run");
  let forget = world.bd_host(&["forget", ALIAS, &id]);
  assert_eq!(forget.status.code(), Some(0), "{}", text(&forget.stderr));
  assert_eq!(text(&forget.stdout), format!("the run {id} was forgotten already\n"));
  let finished = text(&world.bd_host(&["runs", ALIAS]).stdout);
  assert!(finished.starts_with(&format!("{id}  finished, exit 130, started ")), "{finished}");
  assert!(finished.contains(", took ") && !finished.contains("its files kept"), "{finished}");
  let unknown = world.bd_host(&["follow", ALIAS, "1-1"]);
  assert_eq!(unknown.status.code(), Some(255));
  assert!(text(&unknown.stderr).contains("brd-host: unknown_run: "), "{}", text(&unknown.stderr));
  assert_eq!(world.bd_host(&["forget", ALIAS, "1-1"]).status.code(), Some(255));

  let hello = world.bd_host(&["hello", ALIAS, "--json"]);
  assert_eq!(hello.status.code(), Some(0), "{}", text(&hello.stderr));
  let hello: Value = serde_json::from_slice(&hello.stdout).expect("JSON");
  assert_eq!(hello["same_build"], Value::Bool(true), "{hello}");
  assert_eq!(hello["agent"]["features"], features_of_this_platform());
  let refused = world.bd_host(&["hello", "nowhere", "--json"]);
  assert_eq!(refused.status.code(), Some(255));
  let refused: Value = serde_json::from_slice(&refused.stdout).expect("JSON");
  assert_eq!(refused["refusal"]["code"].as_str(), Some("no_host"), "{refused}");
}

/// The end of a lease cancels the runs that the lease leaves (openspec/specs/cli/lease/spec.md, "Return: A return
/// cancels the runs of brd-host that the lease leaves"): a lease of the pool with a run that ended, a run that waits on
/// the agent with no client, and a run of a host that takes the connection and never answers is returned through the
/// pool with this `brd-host`. Only the waiting run is cancelled, and it ends on the host; the silent host keeps its run,
/// and the return ends within the bound of the cancel. The record of the ended run stays, that of the cancelled run goes
#[test]
fn a_return_of_a_lease_cancels_only_its_runs_without_an_end_and_a_silent_host_holds_it_within_the_bound() {
  use git_executor::git_command_executor::GitCommandExecutor;
  use git_ops::lease_slot::{git_dir_of, host_runs_dir};
  use git_ops::revs::repository_dirs;
  use lease_pool::get_queue::QueueTiming;
  use lease_pool::host_cancel::HostCancel;
  use lease_pool::lease::{LeaseGetParams, LeaseReturnParams, lease_get_core, lease_return_core};

  let world = World::new();
  let _agent = world.serve();
  // the `brd-host` that the pool starts has the environment of this process: the home and the timings of the tests
  let home = world.home.path().to_string_lossy().into_owned();
  let mut env: Vec<(&str, &str)> = PROCESS_TEST_ENV.to_vec();
  env.push(("HOME", &home));
  let _env = test_utils::env_vars::EnvVars::set(&env);
  let git = GitCommandExecutor::new();
  let source = repository_dirs(&git, world.repo.path_str()).unwrap().expect("the repository");
  let got = lease_get_core(
    &git,
    &LeaseGetParams {
      source: &source,
      home: Some(world.home.path()),
      holder: Some("agent".to_owned()),
      branch: None,
      min_free: Some(0),
      server_timeout: Duration::from_secs(5),
      bazel: None,
      host_cancel: &HostCancel::NONE,
      lock_timeout: Duration::from_secs(10),
      progress: None,
      space: None,
      cached_build: None,
      queue: QueueTiming::default(),
    },
  )
  .expect("a lease");
  let slot = got.slot.path.clone();
  let lease_id = got.slot.lease.as_ref().expect("the lease").id.clone();

  let ended = world.command().current_dir(&slot).args(["run", ALIAS, "--head", "--", "true"]).output().expect("a run");
  assert_eq!(ended.status.code(), Some(0), "{}", text(&ended.stderr));
  let mut waiting = start_a_run_that_waits_in(&world, Path::new(&slot), Stdio::null());
  // SIGKILL leaves the run going on the host, with no client and no end in the slot
  waiting.kill().expect("the kill of the client");
  let _ = waiting.wait();
  let listed: Value = serde_json::from_slice(&world.bd_host(&["runs", ALIAS, "--json"]).stdout).expect("JSON");
  let id = listed["runs"]
    .as_array()
    .and_then(|runs| runs.iter().find(|run| run["exit"].is_null()))
    .and_then(|run| run["run"].as_str())
    .unwrap_or_else(|| panic!("the run that waits: {listed}"))
    .to_owned();

  // a host that takes the connection and never answers
  let silent = std::net::TcpListener::bind("127.0.0.1:0").expect("a listener");
  let address = silent.local_addr().expect("its address");
  let hosts = world.home.path().join(".config/brd-host/hosts");
  let local: Value = serde_json::from_str(&fs::read_to_string(hosts.join(format!("{ALIAS}.json"))).expect("the host file")).expect("JSON");
  let silent_file = json!({
    "alias": "silent",
    "fingerprint": local["fingerprint"],
    "port": address.port(),
    "hostname": "127.0.0.1",
    "addresses": [address.to_string()],
    "repo": world.repo.path_str(),
  });
  fs::write(hosts.join("silent.json"), silent_file.to_string()).expect("the host file");
  let runs = host_runs_dir(&git_dir_of(Path::new(&slot)).expect("the git dir of the slot"));
  fs::write(runs.join("9-9.json"), "{}").expect("a record");
  fs::write(
    runs.join("9-9.host"),
    "silent
",
  )
  .expect("its host");
  let ended_runs: Vec<String> = fs::read_dir(&runs)
    .expect("the records")
    .filter_map(|entry| entry.ok()?.file_name().to_str()?.strip_suffix(".end.json").map(str::to_owned))
    .collect();
  assert_eq!(ended_runs.len(), 1, "one run ended in the slot: {ended_runs:?}");

  let workspace = repository_dirs(&git, &slot).unwrap().expect("the slot");
  let started = Instant::now();
  let returned = lease_return_core(
    &git,
    &LeaseReturnParams {
      workspace: &workspace,
      lease_id: Some(&lease_id),
      find_by_lease_id: false,
      confirm_preserved: true,
      stop_own: false,
      server_timeout: Duration::from_secs(5),
      bazel: None,
      host_cancel: &HostCancel {
        program: Some(PathBuf::from(BD_HOST)),
        max: Duration::from_secs(1),
      },
      clean: None,
      lock_timeout: Duration::from_secs(10),
      cargo_lock_wait: Duration::ZERO,
      picked: &[],
    },
  )
  .expect("the return");
  let took = started.elapsed();
  let cancelled: Vec<&str> = returned.host_runs.cancelled.iter().map(|run| run.run.as_str()).collect();
  assert_eq!(cancelled, [id.as_str()], "only the run without an end on the agent: {:#?}", returned.host_runs);
  let kept: Vec<(&str, &str, &str)> = returned
    .host_runs
    .not_cancelled
    .iter()
    .map(|run| (run.run.as_str(), run.host.as_str(), run.code.as_str()))
    .collect();
  assert_eq!(kept, [("9-9", "silent", "timed_out")], "{:#?}", returned.host_runs);
  assert!(took < Duration::from_secs(20), "the silent host held the return past the bound of the cancel: {took:?}");
  assert!(!runs.join(format!("{id}.host")).exists(), "the record of the cancelled run goes");
  assert!(runs.join("9-9.host").exists() && runs.join(format!("{}.end.json", ended_runs[0])).exists());

  wait_until("the end of the cancelled run on the host", || {
    let listed: Value = serde_json::from_slice(&world.bd_host(&["runs", ALIAS, "--json"]).stdout).unwrap_or_default();
    listed["runs"]
      .as_array()
      .is_some_and(|runs| runs.iter().any(|run| run["run"] == json!(id) && run["exit"] == json!(130)))
  });
  drop(silent);
}

/// SIGTERM to a client counts as Ctrl+C: a tool that stops the client after its timeout cancels the run on the host,
/// and the client follows the run to its end and leaves by the exit code of its command, rather than leave the run going
/// for nobody. The command ends on the SIGINT of the cancel; the trap of the shell gives it the code 130, and so does the
/// shell itself, where a SIGINT around the start of `sleep` makes bash 3.2 of macOS exit 130 without its trap
#[test]
fn sigterm_to_the_client_cancels_the_run_on_the_host() {
  let world = World::new();
  let _agent = world.serve();
  let run = start_a_run_that_waits(&world, Stdio::piped());
  assert!(Command::new("kill").args(["-TERM", &run.id().to_string()]).status().expect("kill").success());
  let ended = run.wait_with_output().expect("the client ended");
  let stderr = text(&ended.stderr);
  assert_eq!(ended.status.code(), Some(130), "the command ended on the SIGINT of the cancel: {stderr}");
  assert!(stderr.contains("brd-host: Ctrl+C, SIGTERM or SIGHUP: cancelling the run "), "{stderr}");
  assert!(stderr.contains("\nbrd-host: phases: "), "the client followed the run to its end: {stderr}");
}

/// SIGTERM to `brd-host follow` leaves the follow at once with 130, as Ctrl+C does, and the run goes on on the host: a
/// follow that an agent started in the background and stopped never hangs
#[test]
fn sigterm_to_a_follow_leaves_it_and_the_run_goes_on() {
  let world = World::new();
  let _agent = world.serve();
  let mut run = start_a_run_that_waits(&world, Stdio::null());
  let runs: Value = serde_json::from_slice(&world.bd_host(&["runs", ALIAS, "--json"]).stdout).expect("JSON");
  let id = runs["runs"][0]["run"].as_str().expect("the run").to_owned();
  let mut follow = world
    .command()
    .args(["follow", ALIAS, &id])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("a follow");
  let mut stdout = BufReader::new(follow.stdout.take().expect("the stdout of the follow"));
  let mut line = String::new();
  stdout.read_line(&mut line).expect("the first line of the follow");
  assert_eq!(line, "started\n");
  assert!(Command::new("kill").args(["-TERM", &follow.id().to_string()]).status().expect("kill").success());
  let left = follow.wait_with_output().expect("the follow ended");
  let stderr = text(&left.stderr);
  assert_eq!(left.status.code(), Some(130), "{stderr}");
  assert!(
    stderr.contains(&format!(
      "brd-host: interrupted: Ctrl+C, SIGTERM or SIGHUP left the follow of the run {id}, which goes on on {ALIAS}"
    )),
    "{stderr}"
  );
  let listed: Value = serde_json::from_slice(&world.bd_host(&["runs", ALIAS, "--json"]).stdout).expect("JSON");
  assert_eq!(listed["runs"][0]["exit"], Value::Null, "the run goes on: {listed}");
  assert_eq!(world.bd_host(&["cancel", ALIAS, &id]).status.code(), Some(0));
  assert_eq!(run.wait().expect("the run ended").code(), Some(130));
}

/// SIGTERM to a command whose request goes on on the host without it (`trim`, `upgrade`) ends it at once with 130 and
/// says what goes on there, while its connection waits for a host that never answers
#[test]
fn sigterm_ends_a_trim_or_an_upgrade_at_once_and_says_what_goes_on_on_the_host() {
  let world = World::new();
  // a host that accepts the connection and never answers the handshake
  let silent = std::net::TcpListener::bind("127.0.0.1:0").expect("a listener");
  let address = silent.local_addr().expect("its address");
  let host_file = json!({
    "alias": "silent",
    "fingerprint": Identity::generate().expect("an identity").fingerprint().to_string(),
    "port": address.port(),
    "hostname": "127.0.0.1",
    "addresses": [address.to_string()],
    "repo": world.repo.path_str(),
  });
  let hosts = world.home.path().join(".config/brd-host/hosts");
  fs::create_dir_all(&hosts).expect("the hosts directory");
  fs::write(hosts.join("silent.json"), host_file.to_string()).expect("the host file");
  for (command, said) in [
    ("trim", "brd-host: Ctrl+C, SIGTERM or SIGHUP ends this process; the trim goes on on the host"),
    (
      "upgrade",
      "brd-host: Ctrl+C, SIGTERM or SIGHUP ends this process; the host goes on with an upgrade whose binary came whole",
    ),
  ] {
    let child = world
      .command()
      .args([command, "silent"])
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()
      .expect(command);
    let _connection = silent.accept().expect("the command connects");
    assert!(Command::new("kill").args(["-TERM", &child.id().to_string()]).status().expect("kill").success());
    let ended = child.wait_with_output().expect("the command ended");
    let stderr = text(&ended.stderr);
    assert_eq!(ended.status.code(), Some(130), "{command}: {stderr}");
    assert!(stderr.contains(said), "{command}: {stderr}");
  }
}

#[test]
fn runs_shows_who_holds_the_heavy_turn_and_who_waits_for_how_long() {
  let world = World::new();
  let _agent = world.serve();
  let script = "trap 'exit 130' INT; echo started; sleep 60";
  let mut holder = world
    .command()
    .args(["run", ALIAS, "--head", "--heavy", "--", "/bin/sh", "-c", script])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("a heavy run");
  let mut first = String::new();
  BufReader::new(holder.stdout.take().expect("the stdout")).read_line(&mut first).expect("the first line");
  assert_eq!(first, "started\n", "serve: {}", world.serve_log());
  let waiting = world
    .command()
    .args(["run", ALIAS, "--head", "--heavy", "--", "true"])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("a second heavy run");
  let heavy = || -> Value {
    let runs = world.bd_host(&["runs", ALIAS, "--json"]);
    serde_json::from_slice::<Value>(&runs.stdout).expect("JSON")["heavy"].clone()
  };
  wait_until("the second heavy run waits for the turn", || {
    heavy()["queue"].as_array().is_some_and(|queue| queue.len() == 1)
  });
  let turn = heavy();
  let (held_by, waits) = (
    turn["holder"]["run"].as_str().expect("the holder"),
    turn["queue"][0]["run"].as_str().expect("the waiting run"),
  );
  let repo = world.repo.path().file_name().expect("a name").to_string_lossy().into_owned();
  let listed = text(&world.bd_host(&["runs", ALIAS]).stdout);
  let lines: Vec<&str> = listed
    .lines()
    .filter(|line| line.contains(" heavy turn: ") || line.starts_with("heavy turn: ") || line.starts_with("  run "))
    .collect();
  assert_eq!(lines.len(), 3, "{listed}");
  assert!(lines[0].starts_with("heavy turn: ") && lines[0].ends_with(" cores held"), "{listed}");
  assert!(
    lines[1].starts_with(&format!("  run {held_by} of {repo} holds the whole host for ")) && lines[1].ends_with(&format!(" s (no measure): /bin/sh -c {script}")),
    "{listed}"
  );
  assert!(
    lines[2].starts_with(&format!("  run {waits} of {repo} waits "))
      && lines[2].ends_with(" s for the whole host (no measure; a command whose text changes every run never gets one: `--measure-as <class>` gives it one): true"),
    "{listed}"
  );

  let cancel = world.bd_host(&["cancel", ALIAS, held_by]);
  assert_eq!(cancel.status.code(), Some(0), "{}", text(&cancel.stderr));
  assert_eq!(holder.wait().expect("the holder ended").code(), Some(130));
  let waited = waiting.wait_with_output().expect("the second heavy run ended");
  let stderr = text(&waited.stderr);
  assert_eq!(waited.status.code(), Some(0), "{stderr}");
  assert!(
    stderr.contains(
      "brd-host: a heavy run by --heavy: on local it waits until the cores that its command used before are free beside the other heavy runs, or the whole host without a measure\n"
    ),
    "{stderr}"
  );
  assert!(
    stderr.contains(&format!("brd-host: run {waits} waits 0 s for the heavy turn of the host, all its "))
      && stderr.contains(&format!(
        " cores, as the command has no measure yet (a command whose text changes every run never gets one: `--measure-as <class>` gives it one), 120 min left: run {held_by} of {repo} holds the whole host for "
      )),
    "{stderr}"
  );
  assert!(stderr.contains("brd-host: phases: heavy "), "{stderr}");
  wait_until("the turn is free", || {
    let turn = heavy();
    turn["holders"] == json!([]) && turn["queue"] == json!([])
  });
}

#[test]
fn slots_shows_the_lend_table_and_shrinks_it_without_the_agent() {
  let world = World::new();
  let agent = world.serve();
  // the agent serves a repository from its first run on
  let ran = world.bd_host(&["run", ALIAS, "--head", "--", "true"]);
  assert_eq!(ran.status.code(), Some(0), "{}", text(&ran.stderr));
  wait_until("the give-back of the slot", || {
    let slots = world.bd_host(&["slots", ALIAS, "--json"]);
    serde_json::from_slice::<Value>(&slots.stdout).is_ok_and(|slots| slots["repositories"][0]["table"]["slots"][0]["state"]["kind"] == "idle")
  });
  let shown = world.bd_host(&["slots", ALIAS]);
  let shown = text(&shown.stdout);
  assert!(shown.lines().next().is_some_and(|line| line.ends_with(": 1 slots")), "{shown}");
  assert!(shown.lines().nth(1).is_some_and(|line| line.starts_with("  slot 1: idle")), "{shown}");
  drop(agent);

  let repo = world.repo.path_str();
  let local = world.bd_host(&["slots", "--local", "-C", repo, "--sizes"]);
  assert_eq!(local.status.code(), Some(0), "{}", text(&local.stderr));
  assert!(text(&local.stdout).contains("  slot 1: idle"), "{}", text(&local.stdout));

  let shrunk = world.bd_host(&["slots", "--local", "-C", repo, "--shrink", "0", "--json"]);
  assert_eq!(shrunk.status.code(), Some(0), "{}", text(&shrunk.stderr));
  let shrunk: Value = serde_json::from_slice(&shrunk.stdout).expect("JSON");
  assert_eq!(shrunk["shrunk"][0]["outcome"].as_str(), Some("returned"), "{shrunk}");
  assert_eq!(shrunk["table"]["slots"], json!([]));
  assert_eq!(shrunk["table"]["limit"], json!(0));
}

/// `brd-host trim` deletes the free slots of the pools of the host past `branchdeck.leaseMaxSlots` there, without a `brd`
/// on the host; `--plan` only names them, and a host slot, a lease, stays
#[test]
fn trim_plans_and_deletes_the_free_slots_of_the_host() {
  let world = World::new();
  world.repo.git(&["config", "branchdeck.hostSlots", "3"]);
  let agent = world.serve();
  let ran = world.bd_host(&["run", ALIAS, "--head", "--", "true"]);
  assert_eq!(ran.status.code(), Some(0), "{}", text(&ran.stderr));
  let slot_kinds = || {
    let slots = world.bd_host(&["slots", ALIAS, "--json"]);
    serde_json::from_slice::<Value>(&slots.stdout).map_or_else(
      |_| Vec::new(),
      |slots| {
        slots["repositories"][0]["table"]["slots"]
          .as_array()
          .map_or_else(Vec::new, |slots| slots.iter().map(|slot| slot["state"]["kind"].clone()).collect())
      },
    )
  };
  wait_until("three idle host slots", || slot_kinds() == [json!("idle"), json!("idle"), json!("idle")]);
  // two of them go back to the pool as free slots, and a pool of three slots keeps two
  let repo = world.repo.path_str();
  let shrunk = world.bd_host(&["slots", "--local", "-C", repo, "--shrink", "1"]);
  assert_eq!(shrunk.status.code(), Some(0), "{}", text(&shrunk.stderr));
  world.repo.git(&["config", "branchdeck.leaseMaxSlots", "2"]);

  let plan = world.bd_host(&["trim", ALIAS, "--plan", "--json"]);
  assert_eq!(plan.status.code(), Some(0), "{}", text(&plan.stderr));
  let plan: Value = serde_json::from_slice(&plan.stdout).expect("JSON");
  let report = &plan["hosts"][0]["report"];
  assert_eq!(
    (&report["plan"], &report["pools"][0]["slots"], &report["pools"][0]["max_slots"]),
    (&json!(true), &json!(3), &json!(2)),
    "{plan}"
  );
  let planned = report["pools"][0]["removed"][0]["path"].as_str().expect("a slot the trim would delete").to_owned();
  assert!(Path::new(&planned).is_dir(), "a plan deletes nothing");

  let trimmed = world.bd_host(&["trim", ALIAS]);
  assert_eq!(trimmed.status.code(), Some(0), "{}", text(&trimmed.stderr));
  let trimmed = text(&trimmed.stdout);
  assert!(trimmed.starts_with(&format!("{ALIAS}: deleted 1 free slot(s) past the limit")), "{trimmed}");
  assert!(trimmed.contains(&format!("    deleted {planned}, ")), "{trimmed}");
  assert!(!Path::new(&planned).exists(), "the trim deletes the slot it planned");
  assert_eq!(slot_kinds(), [json!("idle")], "the host slot stays");
  drop(agent);
}

/// `brd-host disk`: the agent answers what the pools of its host hold, as `brd lease disk --all` writes it; without an
/// alias every host of the client
#[test]
fn disk_reports_the_pools_of_the_host() {
  let world = World::new();
  let agent = world.serve();
  // the agent serves a repository from its first run on: its host slot is a slot of the pool
  let ran = world.bd_host(&["run", ALIAS, "--head", "--", "true"]);
  assert_eq!(ran.status.code(), Some(0), "{}", text(&ran.stderr));
  let disk = world.bd_host(&["disk", ALIAS, "--fresh", "--json"]);
  assert_eq!(disk.status.code(), Some(0), "{}", text(&disk.stderr));
  let disk: Value = serde_json::from_slice(&disk.stdout).expect("JSON");
  let report = &disk["hosts"][0]["report"];
  assert_eq!(disk["hosts"][0]["alias"], ALIAS, "{disk}");
  assert_eq!(report["pools"][0]["slots"][0]["holder"], "brd-host/1", "{disk}");
  assert_eq!(report["pools"][0]["max_slots"], 10, "{disk}");
  let every = world.bd_host(&["disk"]);
  assert_eq!(every.status.code(), Some(0), "{}", text(&every.stderr));
  let every = text(&every.stdout);
  assert!(every.lines().next().is_some_and(|line| line.starts_with(&format!("{ALIAS}: "))), "{every}");
  assert!(every.contains("in 1 slot(s), limit 10"), "{every}");
  drop(agent);
}

/// `brd-host stats`: the agent sums the runs that ended on its host, the ones that it still keeps too, and answers the
/// summary; without an alias every host of the client, and a window past 90 days is a usage error
#[test]
fn stats_summarizes_the_runs_that_ended_on_the_host() {
  let world = World::new();
  let agent = world.serve();
  let ran = world.bd_host(&["run", ALIAS, "--head", "--", "echo", "hello"]);
  assert_eq!(ran.status.code(), Some(0), "{}", text(&ran.stderr));
  let stats = world.bd_host(&["stats", ALIAS, "--days", "1", "--json"]);
  assert_eq!(stats.status.code(), Some(0), "{}", text(&stats.stderr));
  let stats: Value = serde_json::from_slice(&stats.stdout).expect("JSON");
  let report = &stats["hosts"][0]["report"];
  assert_eq!(
    (&stats["hosts"][0]["alias"], &report["runs"], &report["days"]),
    (&json!(ALIAS), &json!(1), &json!(1)),
    "{stats}"
  );
  assert_eq!(report["outcomes"], json!([{"exit": 0, "runs": 1}]), "{stats}");
  assert_eq!(report["commands"][0]["command"], "echo hello", "{stats}");
  assert!(report["bytes"]["output"].as_u64().is_some_and(|bytes| bytes > 0), "{stats}");
  let every = world.bd_host(&["stats"]);
  assert_eq!(every.status.code(), Some(0), "{}", text(&every.stderr));
  let every = text(&every.stdout);
  assert!(
    every.starts_with(&format!("{ALIAS}: 1 run(s) in the last 7 day(s), 0 heavy, 0 exclusive: waited ")),
    "{every}"
  );
  assert!(every.contains(": echo hello\n"), "{every}");
  assert_eq!(world.bd_host(&["stats", ALIAS, "--days", "91"]).status.code(), Some(2));
  drop(agent);
}

/// `setup --local` makes the temporary home its own host: the client is in the allow list of the agent of this home, the
/// host file names the loopback address and the fingerprint of that agent, the `LaunchAgent` runs a copy of the binary
/// outside the `PATH` and binds the loopback address on macOS, the link of the client stays, and a run then goes over
/// the loopback. The fake launchctl starts nothing, so the agent that launchd would
/// start is a `serve` on the loopback that the test starts first, on the port of the setup
#[test]
fn setup_local_makes_this_machine_its_own_host_on_the_loopback_and_a_run_goes_there() {
  let world = World::new();
  let log = fs::File::create(world.home.path().join("serve.log")).expect("the serve log");
  let (child, listening) = spawn_on_loopback(world.command(), 0, &[], log).expect("serve listens");
  let _agent = Agent { child };
  let port = listening.listening.port().to_string();
  let launchctl = world.home.path().join("launchctl");
  fs::write(&launchctl, "#!/bin/sh\n[ \"$1\" != print ]\n").expect("the fake launchctl");
  fs::set_permissions(&launchctl, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("executable");
  // the client of this machine, which `brd install` links to the bundle of the app
  let client_link = world.home.path().join(".local/bin/brd-host");
  fs::create_dir_all(client_link.parent().expect("a parent")).expect("the bin directory");
  std::os::unix::fs::symlink(BD_HOST, &client_link).expect("the link of brd install");

  let ran = world.bd_host(&[
    "setup",
    "--local",
    "--repo",
    world.repo.path_str(),
    "--port",
    &port,
    "--name",
    "this Mac",
    "--launchctl",
    launchctl.to_str().expect("UTF-8"),
  ]);
  let (stdout, stderr) = (text(&ran.stdout), text(&ran.stderr));
  assert_eq!(ran.status.code(), Some(0), "{stdout}{stderr}\nserve: {}", world.serve_log());
  let binary = world.home.path().join(".local/share/brd-host/agent/brd-host");
  assert!(stdout.starts_with(&format!("copied {BD_HOST} to {}\n", binary.display())), "{stdout}");
  assert!(stdout.contains(&format!("answers on 127.0.0.1:{port} (")), "{stdout}");
  assert!(
    !fs::symlink_metadata(&binary).expect("the binary").file_type().is_symlink(),
    "launchd runs a copy of its own"
  );
  assert_eq!(fs::read_link(&client_link).ok(), Some(PathBuf::from(BD_HOST)), "the client of the PATH stays the link");

  let host_file: Value = serde_json::from_str(&fs::read_to_string(world.home.path().join(".config/brd-host/hosts/local.json")).expect("the host file")).expect("JSON");
  assert_eq!(
    (&host_file["fingerprint"], &host_file["hostname"], &host_file["addresses"]),
    (&json!(listening.fingerprint.to_string()), &json!("127.0.0.1"), &json!([format!("127.0.0.1:{port}")])),
    "{host_file}"
  );
  let client = Identity::load(&world.home.path().join(".config/brd-host")).expect("the client identity").fingerprint();
  let allowed = AllowList::read(&world.home.path().join(".config/brd-host/agent/clients")).expect("the allow list");
  assert_eq!(allowed.name(&client), Some("this Mac"));
  let plist = fs::read_to_string(world.home.path().join("Library/LaunchAgents/com.jetbrains.brd-host.plist")).unwrap_or_default();
  if cfg!(target_os = "macos") {
    assert!(plist.contains(&format!("<string>{}</string>", binary.display())), "{plist}");
    assert!(plist.contains("<string>--bind</string>\n    <string>127.0.0.1</string>"), "{plist}");
  } else {
    assert_eq!(plist, "", "no launchd");
  }

  let ran = world.bd_host(&["run", ALIAS, "--", "/bin/sh", "-c", "echo here"]);
  assert_eq!(ran.status.code(), Some(0), "{}\nserve: {}", text(&ran.stderr), world.serve_log());
  assert_eq!(text(&ran.stdout), "here\n");
}

#[test]
fn schema_and_version_answer_one_json_object() {
  let world = World::new();
  let schema = world.bd_host(&["schema"]);
  assert_eq!(schema.status.code(), Some(0));
  let schema: Value = serde_json::from_slice(&schema.stdout).expect("JSON");
  assert_eq!(schema["protocol"]["port"], json!(7742));
  let version = world.bd_host(&["version", "--json"]);
  let version: Value = serde_json::from_slice(&version.stdout).expect("JSON");
  assert_eq!((version["schema"].clone(), version["features"].clone()), (json!(1), features_of_this_platform()));
  assert_eq!(world.bd_host(&["run", ALIAS]).status.code(), Some(2), "a run without a command");
}

/// The features an agent of this build answers: the upgrade only on macOS, where launchd starts the new binary, and the
/// disk report, the trim, the heavy turn and the estimate everywhere
fn features_of_this_platform() -> Value {
  if cfg!(target_os = "macos") {
    json!(["upgrade", "disk", "trim", "heavy", "exclusive", "stats", "estimate", "measure_as"])
  } else {
    json!(["disk", "trim", "heavy", "exclusive", "stats", "estimate", "measure_as"])
  }
}

/// `brd-host wait` of a run whose client went: past its `--max` it exits 124 and the run goes on; the next wait ends it
/// with the exit code of the command, says its summary and leaves its output in its log; a third answers from its end
#[test]
fn wait_waits_for_a_run_past_its_max_and_the_next_one_ends_it() {
  let world = World::new();
  let _agent = world.serve();
  let mut held = start_a_held_run(&world, &["--head"], "echo started", "echo done; exit 3");
  let hold = held.hold.clone();
  let mut first = String::new();
  BufReader::new(held.client.stdout.take().expect("the stdout"))
    .read_line(&mut first)
    .expect("the first line");
  assert_eq!(first, "started\n", "serve: {}", world.serve_log());
  // SIGKILL leaves the run going on the host, as a tool that kills its call
  let _ = held.client.kill();
  let _ = held.client.wait();
  let listed: Value = serde_json::from_slice(&world.bd_host(&["runs", ALIAS, "--json"]).stdout).expect("JSON");
  let id = listed["runs"][0]["run"].as_str().expect("the run").to_owned();

  let waited = world.bd_host(&["wait", ALIAS, &id, "--max", "1s"]);
  let said = text(&waited.stderr);
  assert_eq!(waited.status.code(), Some(124), "{said}\nserve: {}", world.serve_log());
  assert!(said.contains(&format!(" run {id} runs its command on {ALIAS}\n")), "{said}");
  assert!(
    said.contains(&format!("brd-host: still_running: the run {id} goes on on {ALIAS} after a wait of ")),
    "{said}"
  );
  assert_eq!(text(&waited.stdout), "");

  fs::remove_file(&hold).expect("the hold goes");
  let waited = world.bd_host(&["wait", ALIAS, &id]);
  let said = text(&waited.stderr);
  assert_eq!(waited.status.code(), Some(3), "{said}\nserve: {}", world.serve_log());
  assert!(said.contains(&format!("brd-host: run {id} on {ALIAS} exited with 3 after ")), "{said}");
  assert_eq!(text(&waited.stdout), "", "the output goes to the log");
  let log = said
    .lines()
    .find_map(|line| line.strip_prefix("brd-host: the output of the run: "))
    .and_then(|rest| rest.rsplit_once(" ("))
    .map_or_else(|| panic!("the line of the log: {said}"), |(path, _)| PathBuf::from(path));
  let output = fs::read_to_string(&log).expect("the log");
  assert!(output.contains("started\n") && output.contains("done\n"), "{output}");
  assert!(said.contains("\nstarted\ndone\n"), "the last lines of the output end the summary: {said}");

  let again = world.bd_host(&["wait", ALIAS, &id]);
  assert_eq!(again.status.code(), Some(3), "{}", text(&again.stderr));
  assert!(text(&again.stderr).contains("exited with 3 after "), "{}", text(&again.stderr));
}

/// `brd-host run --max` of a run that goes on past it exits 124 by itself with an empty stdout, and the run goes on: a
/// `wait` ends it with the exit code of the command
#[test]
fn run_max_leaves_a_run_going_and_a_wait_ends_it() {
  let world = World::new();
  let _agent = world.serve();
  let mut held = start_a_held_run(&world, &["--head", "--max", "1s"], "echo started", "exit 4");
  let status = held.client.wait().expect("the run of the client");
  assert_eq!(status.code(), Some(124), "serve: {}", world.serve_log());
  let mut stdout = String::new();
  held.client.stdout.take().expect("the stdout").read_to_string(&mut stdout).expect("the stdout");
  assert_eq!(stdout, "", "the output goes to the log");
  let listed: Value = serde_json::from_slice(&world.bd_host(&["runs", ALIAS, "--json"]).stdout).expect("JSON");
  let id = listed["runs"][0]["run"].as_str().expect("the run").to_owned();
  assert_eq!(listed["runs"][0]["exit"], Value::Null, "the run goes on: {listed}");

  fs::remove_file(&held.hold).expect("the hold goes");
  let waited = world.bd_host(&["wait", ALIAS, &id]);
  assert_eq!(waited.status.code(), Some(4), "{}\nserve: {}", text(&waited.stderr), world.serve_log());
  assert!(text(&waited.stderr).contains("\nstarted\n"), "{}", text(&waited.stderr));
}

/// A set of hosts: `set` writes it, prints it, and refuses the alias of a host file as its name; a run to its name says
/// its plan, goes to the member that ranks first and names it in its report; a wait to the name of the set goes to the
/// member that took the run by `<run>.host` of the checkout; `--exclusive` names one host and refuses with a set
#[test]
fn a_run_to_a_set_of_hosts_says_its_plan_and_a_wait_to_the_set_goes_to_its_member() {
  let world = World::new();
  let _agent = world.serve();
  let named_as_host = world.bd_host(&["set", ALIAS, ALIAS]);
  assert_eq!(named_as_host.status.code(), Some(2), "{}", text(&named_as_host.stderr));
  assert!(
    text(&named_as_host.stderr).contains("brd-host: usage: local is the alias of a host file"),
    "{}",
    text(&named_as_host.stderr)
  );
  let set = world.bd_host(&["set", "any", ALIAS]);
  assert_eq!(set.status.code(), Some(0), "{}", text(&set.stderr));
  let path = world.home.path().join(".config/brd-host/sets/any.json");
  assert_eq!(text(&set.stdout), format!("set any: local; {}\n", path.display()));
  let printed = world.bd_host(&["set", "any", "--json"]);
  assert_eq!(
    serde_json::from_slice::<Value>(&printed.stdout).expect("JSON"),
    json!({"name": "any", "members": [{"alias": "local"}]})
  );

  let report = world.home.path().join("report.json");
  let ran = run_with_report(
    &world,
    world.repo.path(),
    &report,
    &["run", "any", "--head", "--report-fd", "3", "--", "/bin/sh", "-c", "echo done"],
  );
  assert_eq!(ran.status.code(), Some(0), "{}\nserve: {}", text(&ran.stderr), world.serve_log());
  assert_eq!(text(&ran.stdout), "done\n");
  assert!(
    text(&ran.stderr).starts_with("brd-host: local: starts now, no measure of this command; chose local\n"),
    "{}",
    text(&ran.stderr)
  );
  let report = report_of(&report);
  assert_eq!(report["alias"].as_str(), Some(ALIAS), "{report}");
  let run = report["run"].as_str().expect("the run").to_owned();
  let wait = world.bd_host(&["wait", "any", &run]);
  assert_eq!(wait.status.code(), Some(0), "{}", text(&wait.stderr));
  assert!(
    text(&wait.stderr).contains(&format!("brd-host: run {run} on local exited with 0 ")),
    "{}",
    text(&wait.stderr)
  );
  let unknown = world.bd_host(&["wait", "any", "1-1"]);
  assert_eq!(unknown.status.code(), Some(2), "{}", text(&unknown.stderr));

  let exclusive = world.bd_host(&["run", "any", "--exclusive", "--", "true"]);
  assert_eq!(exclusive.status.code(), Some(2), "{}", text(&exclusive.stderr));
  assert!(
    text(&exclusive.stderr).contains("brd-host: usage: --exclusive names one host, not the set any"),
    "{}",
    text(&exclusive.stderr)
  );
}
