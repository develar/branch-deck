//! The process tests of the command line of bd-host (spec/cli/host.spec.md): a real `bd-host serve` on a loopback port
//! with a temporary home, a `TestRepo` whose host slots the keeper fills in the pool of that home, and the client
//! commands of the same binary in that home. The repository is the checkout of the client and the repository of the
//! host at once, so the host has every commit and no pack travels
#![cfg(unix)]
#![expect(clippy::tests_outside_test_module, reason = "a Cargo integration test has no #[cfg(test)] module")]

use host_agent::serve::{PROCESS_TEST_ENV, spawn_on_loopback};
use host_link::allow::AllowList;
use host_link::identity::Identity;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use test_utils::git_test_utils::TestRepo;

const BD_HOST: &str = env!("CARGO_BIN_EXE_bd-host");

/// The alias of the host file of the tests
const ALIAS: &str = "local";

/// A temporary home with the identity of a client in the allow list of the agent, and a repository with one host slot
struct World {
  home: tempfile::TempDir,
  repo: TestRepo,
}

impl World {
  fn new() -> Self {
    let home = tempfile::tempdir().expect("a temporary home");
    let repo = TestRepo::new();
    repo.create_commit_with_files("first", &[(".gitignore", "out/\n")]);
    repo.git(&["config", "branchdeck.hostSlots", "1"]);
    let client = Identity::load_or_create(&home.path().join(".config/bd-host")).expect("the client identity");
    let mut allow = AllowList::default();
    allow.add(client.fingerprint(), "test").expect("a name");
    allow.write(&home.path().join(".config/bd-host/agent/clients")).expect("the allow list");
    Self { home, repo }
  }

  /// `bd-host` with the environment of the tests, in the repository
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

  /// `bd-host <args>` to its end
  fn bd_host(&self, args: &[&str]) -> Output {
    self.command().args(args).output().expect("bd-host ran")
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
    let hosts = self.home.path().join(".config/bd-host/hosts");
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

/// `bd-host <args>` in `dir` with the descriptor 3 open on `report`: the shell opens it, as bt does
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
    .expect("bd-host ran")
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
fn stdout_holds_only_the_command_and_the_lines_of_bd_host_go_to_stderr() {
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
  assert!(stderr.lines().any(|line| line.starts_with("bd-host: run ")), "{stderr}");
  assert!(stderr.lines().any(|line| line.starts_with("bd-host: running in ")), "{stderr}");
  assert!(stderr.lines().any(|line| line.starts_with("bd-host: phases: ")), "{stderr}");

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

#[test]
fn the_report_goes_to_its_descriptor_on_every_way_out() {
  let world = World::new();
  // no descriptor 9: a usage refusal before anything runs, and no report
  let ran = world.bd_host(&["run", ALIAS, "--report-fd", "9", "--", "true"]);
  assert_eq!(ran.status.code(), Some(2), "{}", text(&ran.stderr));
  assert!(text(&ran.stderr).contains("bd-host: usage: --report-fd 9"), "{}", text(&ran.stderr));

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

#[test]
fn follow_cancel_fetch_forget_and_runs_answer_the_runs_of_the_agent() {
  let world = World::new();
  let _agent = world.serve();
  let gate = world.home.path().join("never");
  // The trap makes the end of the command certain. Without it, sh goes on when SIGINT comes while it reaps a `sleep`
  // that has exited 0: sh then takes the signal as handled by its child. The loop then runs to its end, before SIGKILL
  let script = format!(
    "trap 'exit 130' INT; echo started; i=0; while [ ! -e '{}' ] && [ $i -lt 1200 ]; do sleep 0.05; i=$((i + 1)); done",
    gate.display()
  );
  let mut run = world
    .command()
    .args(["run", ALIAS, "--head", "--", "/bin/sh", "-c", &script])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("a run");
  let mut first = String::new();
  BufReader::new(run.stdout.take().expect("the stdout")).read_line(&mut first).expect("the first line");
  assert_eq!(first, "started\n", "serve: {}", world.serve_log());

  let runs = world.bd_host(&["runs", ALIAS, "--json"]);
  assert_eq!(runs.status.code(), Some(0), "{}", text(&runs.stderr));
  let listed: Value = serde_json::from_slice(&runs.stdout).expect("JSON");
  let id = listed["runs"][0]["run"].as_str().expect("the run that goes on").to_owned();
  assert_eq!(listed["runs"][0]["exit"], Value::Null, "{listed}");
  let text_runs = world.bd_host(&["runs", ALIAS]);
  assert!(text(&text_runs.stdout).starts_with(&format!("{id}  running  checkout-")), "{}", text(&text_runs.stdout));

  let fetch = world.bd_host(&["fetch", ALIAS, &id, "--fetch", "out"]);
  assert_eq!(fetch.status.code(), Some(255), "a fetch of a run that goes on");
  assert!(text(&fetch.stderr).contains("bd-host: bad_request: "), "{}", text(&fetch.stderr));

  let cancel = world.bd_host(&["cancel", ALIAS, &id]);
  assert_eq!(cancel.status.code(), Some(0), "{}", text(&cancel.stderr));
  let ended = run.wait_with_output().expect("the run ended");
  let run_dir = world.home.path().join(".cache/bd-host/runs").join(&id);
  assert_eq!(
    ended.status.code(),
    Some(130),
    "SIGINT ends the command; {}\nsupervisor: {}\nevents: {}\nfiles: {:?}",
    text(&ended.stderr),
    fs::read_to_string(run_dir.join("supervisor.log")).unwrap_or_default(),
    fs::read_to_string(run_dir.join("events.log")).unwrap_or_default(),
    fs::read_dir(&run_dir).map_or_default(|entries| entries.flatten().map(|entry| entry.file_name()).collect::<Vec<_>>())
  );

  // the run saw its end and forgot itself on the host
  let follow = world.bd_host(&["follow", ALIAS, &id]);
  assert_eq!(follow.status.code(), Some(255));
  assert!(text(&follow.stderr).contains("bd-host: unknown_run: "), "{}", text(&follow.stderr));
  let forget = world.bd_host(&["forget", ALIAS, &id]);
  assert_eq!(forget.status.code(), Some(255));
  assert!(text(&world.bd_host(&["runs", ALIAS]).stdout).starts_with("no runs on local"));

  let hello = world.bd_host(&["hello", ALIAS, "--json"]);
  assert_eq!(hello.status.code(), Some(0), "{}", text(&hello.stderr));
  let hello: Value = serde_json::from_slice(&hello.stdout).expect("JSON");
  assert_eq!(hello["same_build"], Value::Bool(true), "{hello}");
  assert_eq!(hello["agent"]["features"], json!(["upgrade"]));
  let refused = world.bd_host(&["hello", "nowhere", "--json"]);
  assert_eq!(refused.status.code(), Some(255));
  let refused: Value = serde_json::from_slice(&refused.stdout).expect("JSON");
  assert_eq!(refused["refusal"]["code"].as_str(), Some("no_host"), "{refused}");
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

#[test]
fn schema_and_version_answer_one_json_object() {
  let world = World::new();
  let schema = world.bd_host(&["schema"]);
  assert_eq!(schema.status.code(), Some(0));
  let schema: Value = serde_json::from_slice(&schema.stdout).expect("JSON");
  assert_eq!(schema["protocol"]["port"], json!(7742));
  let version = world.bd_host(&["version", "--json"]);
  let version: Value = serde_json::from_slice(&version.stdout).expect("JSON");
  assert_eq!((version["schema"].clone(), version["features"].clone()), (json!(1), json!(["upgrade"])));
  assert_eq!(world.bd_host(&["run", ALIAS]).status.code(), Some(2), "a run without a command");
}
