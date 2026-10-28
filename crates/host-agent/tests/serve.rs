//! The process tests of the agent (spec/cli/host.spec.md, "serve"): a real `serve` of `host-agent-test` on a loopback
//! port with a temporary home, a `TestRepo` whose host slots the keeper fills in the pool of that home, and commands of
//! `/bin/sh`. The client is the `Channel` of host-link. No fake `bd` exists: the lend table, the refs and the git of the
//! host run in process
#![cfg(unix)]
#![expect(clippy::tests_outside_test_module, reason = "a Cargo integration test has no #[cfg(test)] module")]

// The modules of `serve/` are part of this test binary, so they share its fixture and its build
#[path = "serve/signals.rs"]
mod signals;

#[path = "serve/pack.rs"]
mod pack;

#[path = "serve/system.rs"]
mod system;

use host_agent::serve::spawn_on_loopback;
use host_link::allow::AllowList;
use host_link::channel::{Channel, ClientTls};
use host_link::identity::{Fingerprint, Identity};
use host_link::proto::{FileKind, PackTarget, Phase, Reply, Request, RunSpec, Stream};
use pretty_assertions::assert_eq;
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use test_utils::git_test_utils::{TestRepo, git_command};

/// The deadline of each connect and read of the test client
const DEADLINE: Duration = Duration::from_secs(60);

const AFFINITY: &str = "checkout-0123456789ab";

/// The phases of a run that ran its command, in their order
const ALL_PHASES: [&str; 6] = ["slot", "pack_wait", "checkout", "prepare", "command", "stage"];

/// The start of the line that the agent writes right before the command
const START_LINE: &str = "bd-host: running in ";

/// A temporary home, a repository of the host with `slots` host slots, and the identity of a client in the allow list
struct World {
  home: tempfile::TempDir,
  repo: TestRepo,
  client: Identity,
}

impl World {
  fn new(slots: u32, files: &[(&str, &str)]) -> Self {
    let home = tempfile::tempdir().expect("a temporary home");
    let repo = TestRepo::new();
    let mut all = vec![(".gitignore", "out/\n")];
    all.extend_from_slice(files);
    repo.create_commit_with_files("first", &all);
    repo.git(&["config", "branchdeck.hostSlots", &slots.to_string()]);
    let client = Identity::generate().expect("the client identity");
    let mut allow = AllowList::default();
    allow.add(client.fingerprint(), "test").expect("a name");
    allow.write(&home.path().join(".config/bd-host/agent/clients")).expect("the allow list");
    Self { home, repo, client }
  }

  fn spec(&self, run: &str, script: &str) -> RunSpec {
    RunSpec {
      run: run.to_owned(),
      repo: self.repo.path_str().to_owned(),
      affinity: AFFINITY.to_owned(),
      commit: self.repo.head(),
      command: vec!["/bin/sh".to_owned(), "-c".to_owned(), script.to_owned()],
      fetch: vec!["out/bt-runs".to_owned()],
      ..RunSpec::default()
    }
  }

  fn run_dir(&self, run: &str) -> PathBuf {
    self.home.path().join(".cache/bd-host/runs").join(run)
  }

  /// Starts `serve` on a loopback port
  fn serve(&self) -> Agent<'_> {
    self.serve_with(&[])
  }

  /// Starts `serve` on a loopback port with more environment, such as a duration of `tuning.rs` or a fake of
  /// `PROCESS_TEST_ENV`
  fn serve_with(&self, env: &[(&str, &str)]) -> Agent<'_> {
    let log = fs::File::create(self.home.path().join("serve.log")).expect("the serve log");
    let mut command = Command::new(env!("CARGO_BIN_EXE_host-agent-test"));
    command.env("HOME", self.home.path());
    command.envs(env.iter().copied());
    let (child, listening) = spawn_on_loopback(command, log).expect("serve listens");
    Agent {
      child,
      address: listening.listening,
      fingerprint: listening.fingerprint,
      world: self,
    }
  }
}

/// A running `serve` of a world
struct Agent<'w> {
  child: Child,
  address: SocketAddr,
  fingerprint: Fingerprint,
  world: &'w World,
}

impl Drop for Agent<'_> {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

/// The replies of one follow
#[derive(Debug)]
struct Followed {
  lines: Vec<(Stream, String)>,
  phases: Vec<Phase>,
  end: Reply,
}

impl Followed {
  fn phase_names(&self) -> Vec<&str> {
    self.phases.iter().map(|phase| phase.name.as_str()).collect()
  }

  /// The lines of one stream, less the lines of the agent
  fn of(&self, stream: Stream) -> Vec<&str> {
    self
      .lines
      .iter()
      .filter(|(of, text)| *of == stream && !text.starts_with("bd-host: "))
      .map(|(_, text)| text.as_str())
      .collect()
  }

  /// Whether a line of the run is `text`, a line of the agent too
  fn has_line(&self, text: &str) -> bool {
    self.lines.iter().any(|(_, line)| line == text)
  }

  /// The line that the agent writes right before the command
  fn start_line(&self) -> &str {
    self
      .lines
      .iter()
      .map(|(_, text)| text.as_str())
      .find(|text| text.starts_with(START_LINE))
      .unwrap_or_else(|| panic!("no start line: {self:?}"))
  }
}

impl Agent<'_> {
  /// A connection after its hello, with `request` sent
  fn ask(&self, request: &Request) -> Channel {
    let tls = ClientTls::new(self.fingerprint, &self.world.client).expect("the TLS setup");
    let mut channel = Channel::connect(self.address, &tls, DEADLINE).expect("connected");
    channel.send(&Request::hello("test")).expect("the hello");
    let hello: Reply = channel.recv().expect("the hello reply");
    assert!(matches!(hello, Reply::Hello { .. }), "{hello:?}");
    channel.send(request).expect("the request");
    channel
  }

  /// The last reply of `request`; the heartbeats before it are skipped
  fn reply(&self, request: &Request) -> Reply {
    let mut channel = self.ask(request);
    loop {
      match channel.recv().expect("the reply") {
        Reply::Heartbeat { .. } => {}
        reply => return reply,
      }
    }
  }

  fn start(&self, spec: RunSpec) {
    let run = spec.run.clone();
    assert_eq!(self.reply(&Request::Start(spec)), Reply::Started { run });
  }

  /// Follows a run from its start to its end
  fn follow(&self, run: &str) -> Followed {
    let mut channel = self.ask(&Request::Follow { run: run.to_owned(), offset: 0 });
    let mut lines = Vec::new();
    let mut phases = Vec::new();
    loop {
      match channel.recv::<Reply>().unwrap_or_else(|error| panic!("{error}; {}", self.dump(run))) {
        Reply::Output { stream, text, .. } => lines.push((stream, text)),
        Reply::Heartbeat { .. } => {}
        Reply::Phases { phases: got } => phases = got,
        end => return Followed { lines, phases, end },
      }
    }
  }

  /// Follows a run from its start until a line of its command is `text`, then drops the connection
  fn follow_until(&self, run: &str, text: &str) {
    let mut channel = self.ask(&Request::Follow { run: run.to_owned(), offset: 0 });
    loop {
      match channel.recv::<Reply>().unwrap_or_else(|error| panic!("{error}; {}", self.dump(run))) {
        Reply::Output { text: line, .. } if line == text => return,
        Reply::Output { .. } | Reply::Heartbeat { .. } | Reply::Phases { .. } => {}
        end => panic!("the run ended before {text:?}: {end:?}; {}", self.dump(run)),
      }
    }
  }

  /// The pid of the supervisor of a run, as its `pid` file has it
  fn supervisor_pid(&self, run: &str) -> String {
    let path = self.world.run_dir(run).join("pid");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display())).trim().to_owned()
  }

  /// The files of a fetch: the path, the kind and the text of each
  fn fetch(&self, run: &str) -> Vec<(String, FileKind, String)> {
    let mut channel = self.ask(&Request::Fetch { run: run.to_owned() });
    let mut files = Vec::new();
    loop {
      match channel.recv::<Reply>().expect("a reply") {
        Reply::File { path, kind, .. } => {
          let mut bytes = Vec::new();
          channel.recv_stream(&mut bytes).expect("the bytes");
          files.push((path, kind, String::from_utf8(bytes).expect("UTF-8")));
        }
        Reply::Fetched { files: count, .. } => {
          assert_eq!(usize::try_from(count).ok(), Some(files.len()));
          return files;
        }
        other => panic!("{other:?}"),
      }
    }
  }

  /// The files of a run and the log of `serve`, for a failure message
  fn dump(&self, run: &str) -> String {
    let read = |path: PathBuf| fs::read_to_string(&path).unwrap_or_else(|error| format!("<{}: {error}>", path.display()));
    let dir = self.world.run_dir(run);
    format!(
      "events: {}\nsupervisor: {}\nserve: {}",
      read(dir.join("events.log")),
      read(dir.join("supervisor.log")),
      read(self.world.home.path().join("serve.log"))
    )
  }

  /// The lend table of the repository, as `Request::Slots` answers it
  fn slots(&self) -> serde_json::Value {
    match self.reply(&Request::Slots) {
      Reply::Slots { repositories } => {
        assert_eq!(repositories.len(), 1, "{repositories:?}");
        repositories.into_iter().next().map_or_else(serde_json::Value::default, |repository| repository.table)
      }
      other => panic!("{other:?}"),
    }
  }
}

/// Waits up to 60 s until `done` answers `true`
fn eventually(what: &str, done: impl Fn() -> bool) {
  let started = Instant::now();
  while !done() {
    assert!(started.elapsed() < Duration::from_mins(1), "{what} did not happen in 60 s");
    thread::sleep(Duration::from_millis(50));
  }
}

/// A shell loop that waits until the file `gate` exists, up to 60 s, so a failed test leaves no command behind
fn wait_for(gate: &Path) -> String {
  format!("i=0; while [ ! -e '{}' ] && [ $i -lt 1200 ]; do sleep 0.05; i=$((i + 1)); done", gate.display())
}

/// The text of `events.log` of a run, or nothing
fn events(world: &World, run: &str) -> String {
  fs::read_to_string(world.run_dir(run).join("events.log")).unwrap_or_default()
}

/// Whether the process `pid` exists, as `kill -0` answers
fn process_alive(pid: &str) -> bool {
  Command::new("kill").args(["-0", pid]).stderr(Stdio::null()).status().is_ok_and(|status| status.success())
}

/// Writes an executable script
fn write_script(path: &Path, text: &str) {
  fs::write(path, text).expect("a script");
  fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// A clone of the repository of the host with one more commit, and the thin pack of that commit against the head of
/// the host
fn client_commit(world: &World, file: &str, text: &str) -> (tempfile::TempDir, String, Vec<u8>) {
  let dir = tempfile::tempdir().expect("a temporary directory");
  let client = dir.path().join("client");
  let cloned = git_command().args(["clone", "-q", world.repo.path_str()]).arg(&client).status().expect("git clone");
  assert!(cloned.success());
  fs::write(client.join(file), text).expect("a file");
  let git = |args: &[&str]| {
    let output = git_command()
      .current_dir(&client)
      .args(["-c", "user.name=t", "-c", "user.email=t@t"])
      .args(args)
      .output()
      .expect("git");
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    output.stdout
  };
  git(&["add", file]);
  git(&["commit", "-q", "-m", "second"]);
  let commit = String::from_utf8(git(&["rev-parse", "HEAD"])).expect("UTF-8").trim().to_owned();
  let mut pack = git_command()
    .current_dir(&client)
    .args(["pack-objects", "--stdout", "--thin", "--revs", "-q"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .expect("git pack-objects");
  pack
    .stdin
    .take()
    .expect("the stdin")
    .write_all(format!("{commit}\n^{}\n", world.repo.head()).as_bytes())
    .expect("the revisions");
  let output = pack.wait_with_output().expect("the pack");
  assert!(output.status.success());
  (dir, commit, output.stdout)
}

/// Sends `bytes` as the pack of `target` at `commit`, and answers the reply
fn send_pack(agent: &Agent<'_>, commit: &str, target: PackTarget, bytes: &[u8]) -> Reply {
  let mut channel = agent.ask(&Request::Pack {
    repo: agent.world.repo.path_str().to_owned(),
    commit: commit.to_owned(),
    target,
  });
  channel.send_stream(&mut &bytes[..], &mut |_| {}).expect("the pack");
  loop {
    match channel.recv().expect("the reply") {
      Reply::Heartbeat { .. } => {}
      reply => return reply,
    }
  }
}

#[test]
fn serve_answers_a_run_on_the_loopback_with_a_real_pool() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  let head = world.repo.head();
  assert_eq!(
    agent.reply(&Request::Have {
      repo: world.repo.path_str().to_owned(),
      commits: vec![head.clone(), "0".repeat(40)],
    }),
    Reply::Have { commits: vec![head] }
  );
  assert!(
    matches!(agent.reply(&Request::Have { repo: "~/missing".to_owned(), commits: Vec::new() }), Reply::Refused { code, .. } if code == "repo_missing"),
    "a path that is no repository"
  );

  agent.start(world.spec("1-1", "echo out; echo err >&2; mkdir -p out/bt-runs/a && echo r > out/bt-runs/a/report.txt; exit 3"));
  let followed = agent.follow("1-1");
  assert_eq!(followed.end, Reply::Exit { code: 3 }, "{}", agent.dump("1-1"));
  assert_eq!(followed.of(Stream::Stdout), ["out"]);
  assert_eq!(followed.of(Stream::Stderr), ["err"]);
  assert!(followed.lines.iter().any(|(_, text)| text.starts_with(START_LINE)), "{followed:?}");
  assert_eq!(followed.phase_names(), ALL_PHASES);
  assert_eq!(agent.fetch("1-1"), [("out/bt-runs/a/report.txt".to_owned(), FileKind::Staged, "r\n".to_owned())]);

  let table = agent.slots();
  assert_eq!(table["slots"].as_array().map(Vec::len), Some(1), "{table}");
  // the give-back follows `exit.json` in the supervisor
  eventually("the slot is idle again", || agent.slots()["slots"][0]["state"]["kind"] == "idle");
  match agent.reply(&Request::Runs) {
    Reply::Runs { runs } => {
      assert_eq!(runs.len(), 1);
      assert_eq!((runs[0].run.as_str(), runs[0].affinity.as_str(), runs[0].exit), ("1-1", AFFINITY, Some(3)));
    }
    other => panic!("{other:?}"),
  }
  assert!(matches!(agent.reply(&Request::Forget { run: "1-1".to_owned() }), Reply::Done { .. }));
  assert!(!world.run_dir("1-1").exists());
}

#[test]
fn a_run_waits_for_its_pack_checks_out_by_the_diff_and_gives_its_slot_back() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  let (_client, commit, pack) = client_commit(&world, "b.txt", "from the client\n");
  agent.start(RunSpec {
    commit: commit.clone(),
    ..world.spec("2-1", "cat b.txt")
  });
  eventually("the run waits for its pack", || {
    fs::read_to_string(world.run_dir("2-1").join("events.log")).is_ok_and(|events| events.contains("waiting for the pack"))
  });
  assert_eq!(send_pack(&agent, &commit, PackTarget::Run("2-1".to_owned()), &pack), Reply::Packed);
  let followed = agent.follow("2-1");
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{}", agent.dump("2-1"));
  assert_eq!(followed.of(Stream::Stdout), ["from the client"]);
  assert_eq!(followed.phase_names(), ALL_PHASES);
  // the give-back follows `exit.json` in the supervisor: it deletes the ref of the run
  eventually("the give-back deletes the ref of the run", || {
    world.repo.git(&["for-each-ref", "--format=%(refname)", "refs/branch-deck/host/"]).trim().is_empty()
  });
  assert_eq!(
    send_pack(&agent, &commit, PackTarget::Base("../x".to_owned()), &pack),
    Reply::Refused {
      code: "bad_request".to_owned(),
      message: "\"../x\" cannot name a run or a base".to_owned(),
    }
  );
  assert_eq!(send_pack(&agent, &commit, PackTarget::Base(AFFINITY.to_owned()), &pack), Reply::Packed);
  assert_eq!(
    world.repo.git(&["rev-parse", &format!("refs/branch-deck/host/base/{AFFINITY}")]).trim(),
    commit,
    "a base pack keeps its ref"
  );
}

#[test]
fn the_changes_of_a_command_come_back_as_a_patch_without_the_prepare_steps() {
  let config = "[host]\noutputs = [\"out/result.txt\"]\n\n[host.env]\nFROM_CONFIG = \"yes\"\n\n[[host.prepare]]\nname = \"made\"\nrun = [\"/bin/sh\", \"-c\", \"echo prepared > made.txt\"]\ncreates = [\"made.txt\"]\n";
  let world = World::new(1, &[(".branchdeck.toml", config), ("a.txt", "a\n")]);
  let agent = world.serve();
  agent.start(RunSpec {
    changes: true,
    outputs: true,
    ..world.spec("3-1", "echo \"$FROM_CONFIG $BD_HOST_RUN\"; echo changed > a.txt; mkdir -p out && echo o > out/result.txt")
  });
  let followed = agent.follow("3-1");
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{}", agent.dump("3-1"));
  assert_eq!(followed.of(Stream::Stdout), ["yes 3-1"]);
  assert!(
    followed.lines.iter().any(|(_, text)| text == "bd-host: prepare: made (made.txt is missing)"),
    "{followed:?}"
  );
  let files = agent.fetch("3-1");
  let kinds: Vec<(&str, &FileKind)> = files.iter().map(|(path, kind, _)| (path.as_str(), kind)).collect();
  assert_eq!(
    kinds,
    [
      (
        "out/result.txt",
        &FileKind::Output {
          root: "out/result.txt".to_owned()
        }
      ),
      ("changes.patch", &FileKind::Changes),
    ]
  );
  let patch = &files[1].2;
  assert!(patch.contains("+changed") && patch.contains("a.txt"), "{patch}");
  assert!(!patch.contains("made.txt"), "the file of a prepare step is no change of the command: {patch}");
}

#[test]
fn two_runs_of_one_affinity_run_in_parallel_and_a_third_waits_for_a_slot() {
  let world = World::new(2, &[]);
  let agent = world.serve();
  let gate = world.home.path().join("gate");
  let marks = world.home.path().join("marks");
  fs::create_dir_all(&marks).expect("the marks");
  for run in ["4-1", "4-2", "4-3"] {
    let mark = marks.join(run);
    agent.start(world.spec(run, &format!(": > '{}'; {}", mark.display(), wait_for(&gate))));
  }
  // the supervisors lend in the order that they start, which is not the order of the starts
  let running = || fs::read_dir(&marks).map_or(0, Iterator::count);
  eventually("two runs run at once", || running() == 2);
  thread::sleep(Duration::from_millis(500));
  assert_eq!(running(), 2, "the third run waits for a slot");
  fs::write(&gate, "").expect("the gate opens");
  for run in ["4-1", "4-2", "4-3"] {
    assert_eq!(agent.follow(run).end, Reply::Exit { code: 0 }, "{}", agent.dump(run));
  }
}

#[test]
fn a_cancel_in_the_queue_of_the_host_slots_ends_the_run_before_its_command() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  let gate = world.home.path().join("gate");
  agent.start(world.spec("5-1", &wait_for(&gate)));
  eventually("the first run holds the slot", || {
    fs::read_to_string(world.run_dir("5-1").join("events.log")).is_ok_and(|events| events.contains(START_LINE))
  });
  let ran = world.home.path().join("ran");
  agent.start(world.spec("5-2", &format!(": > '{}'", ran.display())));
  assert!(matches!(agent.reply(&Request::Cancel { run: "5-2".to_owned() }), Reply::Done { .. }));
  let followed = agent.follow("5-2");
  assert_eq!(followed.end, Reply::Exit { code: 130 }, "{}", agent.dump("5-2"));
  assert!(!ran.exists(), "the command never ran");
  fs::write(&gate, "").expect("the gate opens");
  assert_eq!(agent.follow("5-1").end, Reply::Exit { code: 0 });
  let table = agent.slots();
  assert_eq!(table["queue"].as_array().map(Vec::len), Some(0), "the cancel left the queue: {table}");
}
