//! What a run gets from the system of the host: the power assertions of `serve` and of the supervisor, the login
//! environment, the open-files limit, the niceness, and the private modes of the files of the agent

use super::{START_LINE, World, eventually, process_alive, wait_for, write_script};
use host_link::proto::{Reply, Request, RunSpec, Stream};
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

/// The host stays awake while a client talks to the agent and while a run goes on: `serve` and the supervisor of the
/// run each start the program of the power assertions, which watches their process. The one of `serve` lives while a
/// connection is open, and the one of the supervisor ends with the supervisor
#[test]
fn serve_and_the_supervisor_keep_the_host_awake() {
  let world = World::new(1, &[]);
  let log = world.home.path().join("caffeinate.log");
  let fake = world.home.path().join("caffeinate");
  write_script(
    &fake,
    &format!("#!/bin/sh\necho \"$$ $*\" >> '{}'\nwhile kill -0 \"$4\" 2>/dev/null; do sleep 0.05; done\n", log.display()),
  );
  let agent = world.serve_with(&[("BD_HOST_TEST_CAFFEINATE", fake.to_str().expect("a text path"))]);
  let gate = world.home.path().join("gate");
  agent.start(world.spec("1-1", &format!("echo ran; {}", wait_for(&gate))));
  let supervisor = agent.supervisor_pid("1-1");
  let serve = agent.child.id().to_string();
  // each line: the pid of the program, then `-i -s -w <watched pid>`
  let started = || -> Vec<(String, String)> {
    fs::read_to_string(&log)
      .unwrap_or_default()
      .lines()
      .map(|line| {
        let (pid, args) = line.split_once(' ').unwrap_or_else(|| panic!("a pid and the arguments: {line}"));
        let watched = args.strip_prefix("-i -s -w ").unwrap_or_else(|| panic!("the arguments: {line}"));
        (pid.to_owned(), watched.to_owned())
      })
      .collect()
  };
  // the open follow holds the host awake for `serve`, and the run for its supervisor
  let mut follow = agent.ask(&Request::Follow { run: "1-1".to_owned(), offset: 0 });
  eventually("a program for serve and one for the supervisor", || {
    let started = started();
    let alive = |watched: &str| started.iter().any(|(pid, of)| of == watched && process_alive(pid));
    alive(&serve) && alive(&supervisor)
  });
  let watched: BTreeSet<String> = started().into_iter().map(|(_, watched)| watched).collect();
  assert_eq!(watched, BTreeSet::from([serve, supervisor]), "only serve and the supervisor are watched");
  fs::write(&gate, "").expect("the gate opens");
  let end = loop {
    match follow.recv::<Reply>().expect("a reply") {
      Reply::Output { .. } | Reply::Heartbeat { .. } | Reply::Phases { .. } => {}
      end => break end,
    }
  };
  assert_eq!(end, Reply::Exit { code: 0 }, "{}", agent.dump("1-1"));
  drop(follow);
  // `serve` stops its program after the last connection, and the program of the supervisor ends with it
  for (pid, watched) in started() {
    eventually(&format!("the end of the program that watches {watched}"), || !process_alive(&pid));
  }
}

/// A login shell for the tests: it sets `FROM_PROFILE` and `BD_HOST_RUN`, and puts `/profile/bin` first on `PATH`,
/// then runs the script of `-c`, its last argument, as a real login shell does after its profile
const LOGIN_SHELL: &str = r#"#!/bin/sh
for script; do :; done
FROM_PROFILE=yes BD_HOST_RUN=from-the-profile PATH="/profile/bin:$PATH"
export FROM_PROFILE BD_HOST_RUN PATH
exec /bin/sh -c "$script"
"#;

/// The niceness of this test process, as `ps` shows it
fn own_niceness() -> u8 {
  let output = Command::new("ps").args(["-o", "nice=", "-p", &std::process::id().to_string()]).output().expect("ps");
  String::from_utf8_lossy(&output.stdout).trim().parse().expect("a niceness of 0 to 19")
}

/// A run gets, in this order: the environment of the agent, the login environment, the git of the host, the variables
/// of the run, and `[host.env]`. Its command runs with the raised open-files limit and the niceness of the spec
#[test]
fn a_run_gets_the_login_environment_a_raised_open_files_limit_and_its_niceness() {
  let config = "[host.env]\nFROM_CONFIG = \"yes\"\nPATH = { prepend = [\"~/from-config\"] }\n";
  let world = World::new(1, &[(".branchdeck.toml", config)]);
  let shell = world.home.path().join("login-shell");
  write_script(&shell, LOGIN_SHELL);
  let agent = world.serve_with(&[("BD_HOST_TEST_SHELL", shell.to_str().expect("a text path")), ("FROM_PROFILE", "agent")]);
  // a process cannot lower its own niceness, and a test can start niced, such as under a build server at nice 10
  let nice = (own_niceness() + 3).min(19);
  let script = r#"echo "$FROM_PROFILE $FROM_CONFIG $BD_HOST_RUN"; echo "$PATH" | cut -d: -f1-2; ulimit -n; ps -o nice= -p $$; git --version"#;
  agent.start(RunSpec {
    nice: Some(nice),
    ..world.spec("2-1", script)
  });
  let followed = agent.follow("2-1");
  assert_eq!(followed.end, Reply::Exit { code: 0 }, "{}", agent.dump("2-1"));
  let stdout = followed.of(Stream::Stdout);
  assert_eq!(stdout.len(), 5, "{followed:?}");
  // the login environment over the agent, the run over the login environment, and `[host.env]` over both
  assert_eq!(stdout[0], "yes yes 2-1", "{followed:?}");
  assert_eq!(stdout[1], format!("{}:/profile/bin", world.home.path().join("from-config").display()));
  let start = followed.start_line();
  let limit: u64 = stdout[2].trim().parse().expect("a limit");
  assert!(limit >= 4096, "{limit}");
  assert!(start.contains(&format!(", {limit} open files, ")), "the command has the limit of the agent: {start}");
  assert_eq!(stdout[3].trim(), nice.to_string());
  assert!(start.contains(&format!(", nice {nice}, ")), "{start}");
  // the command finds the git of the host that the start line names
  let version = start.split(", ").find_map(|part| part.strip_prefix("git ")).unwrap_or_else(|| panic!("no git: {start}"));
  assert!(stdout[4].starts_with(&format!("git version {version}")), "{} is not git {version}", stdout[4]);
  assert!(start.contains(&format!("the login environment of {} -l -i from ", shell.display())), "{start}");
  let kept = fs::read_to_string(world.home.path().join(".cache/bd-host/login-env.json")).expect("the capture");
  assert!(kept.contains("FROM_PROFILE") && !kept.contains("\"PWD\""), "{kept}");
  assert!(START_LINE.starts_with("bd-host: ") && start.starts_with(START_LINE));
}

fn mode(path: &Path) -> u32 {
  fs::symlink_metadata(path)
    .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    .permissions()
    .mode()
    & 0o777
}

/// Each path under `dir`, `dir` too, less the entries under `skip`
fn tree(dir: &Path, skip: &Path, paths: &mut Vec<std::path::PathBuf>) {
  paths.push(dir.to_path_buf());
  if dir == skip || !dir.is_dir() {
    return;
  }
  for entry in fs::read_dir(dir).expect("a directory") {
    tree(&entry.expect("an entry").path(), skip, paths);
  }
}

/// Each directory of the agent has the mode 0700, and each file 0600, whatever the umask: the identity, the cache and
/// each file of a run. The staged files of a command keep their modes
#[test]
fn the_files_of_the_agent_are_private() {
  let world = World::new(1, &[]);
  let agent = world.serve();
  agent.start(world.spec("3-1", "mkdir -p out/bt-runs/r; echo x > out/bt-runs/r/f"));
  assert_eq!(agent.follow("3-1").end, Reply::Exit { code: 0 }, "{}", agent.dump("3-1"));
  let supervisor = agent.supervisor_pid("3-1");
  // the supervisor writes the fingerprints of the slot after `exit.json`
  eventually("the end of the supervisor", || !process_alive(&supervisor));
  let home = world.home.path();
  let run = world.run_dir("3-1");
  let mut paths = Vec::new();
  tree(&home.join(".cache/bd-host"), &run.join("stage"), &mut paths);
  for path in [".config/bd-host/agent", ".config/bd-host/agent/identity.key"] {
    paths.push(home.join(path));
  }
  for name in ["spec.json", "events.log", "exit.json", "phases.json", "pid", "lock", "supervisor.log", "stage"] {
    assert!(paths.contains(&run.join(name)), "{name}: {paths:?}");
  }
  assert!(
    fs::read_dir(home.join(".cache/bd-host/slots"))
      .is_ok_and(|mut entries| entries.any(|entry| entry.is_ok_and(|entry| entry.file_name().to_string_lossy().ends_with(".prepared")))),
    "the fingerprints of the slot: {paths:?}"
  );
  let wrong: Vec<String> = paths
    .iter()
    .filter(|path| mode(path) != if path.is_dir() { 0o700 } else { 0o600 })
    .map(|path| format!("{} {:o}", path.display(), mode(path)))
    .collect();
  assert!(wrong.is_empty(), "not private: {wrong:?}");
}
