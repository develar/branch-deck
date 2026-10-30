use crate::bazel_servers::{find_program, servers_by_output_base, shutdown};
#[cfg(windows)]
use crate::lease::LeaseRefusal;
use crate::lease::{slot_servers, stop_servers};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};
use sync_utils::paths::display_path;
use sync_utils::process_scan::{ProcessInfo, ProcessScan, is_running, process_started};
#[cfg(windows)]
use test_utils::env_vars::EnvVars;
use test_utils::links::directory_link;

/// A fake Bazel server: a slot whose workspace (`workspace` of it, the slot itself when empty) has the link `link` into
/// an output base, whose `server/server.pid.txt` names a child that sleeps. Bazel's own links are junctions on Windows
/// and symlinks elsewhere, as `directory_link` makes them
struct FakeServer {
  _dir: tempfile::TempDir,
  slot: PathBuf,
  workspace: PathBuf,
  output_base: PathBuf,
  pid_file: PathBuf,
  child: Child,
}

impl FakeServer {
  fn new(workspace: &str, workspace_file: &str, link: &str, mut server: Command) -> Self {
    let dir = tempfile::tempdir().unwrap();
    let slot = dir.path().join("slot");
    let workspace = if workspace.is_empty() { slot.clone() } else { slot.join(workspace) };
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(workspace.join(workspace_file), "").unwrap();
    let output_base = dir.path().join("output_base");
    let bazel_out = output_base.join("execroot").join("_main").join("bazel-out");
    std::fs::create_dir_all(&bazel_out).unwrap();
    std::fs::create_dir_all(output_base.join("server")).unwrap();
    let link = workspace.join(link);
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    directory_link(&bazel_out, &link);
    let child = server.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    // as Bazel does: the server writes its pid once it runs
    let pid_file = output_base.join("server").join("server.pid.txt");
    std::fs::write(&pid_file, child.id().to_string()).unwrap();
    Self {
      _dir: dir,
      slot,
      workspace: std::fs::canonicalize(workspace).unwrap(),
      output_base: std::fs::canonicalize(output_base).unwrap(),
      pid_file,
      child,
    }
  }

  fn pid(&self) -> u32 {
    self.child.id()
  }
}

impl Drop for FakeServer {
  fn drop(&mut self) {
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

/// A child that sleeps for two minutes
fn sleeper() -> Command {
  if cfg!(windows) {
    let mut command = Command::new("ping");
    command.args(["-n", "120", "127.0.0.1"]);
    command
  } else {
    let mut command = Command::new("sleep");
    command.arg("120");
    command
  }
}

/// The pid of a process that has exited
fn gone_pid() -> u32 {
  let mut command = if cfg!(windows) {
    let mut command = Command::new("cmd");
    command.args(["/C", "exit"]);
    command
  } else {
    Command::new("true")
  };
  let mut child = command.spawn().unwrap();
  child.wait().unwrap();
  child.id()
}

/// The server of a workspace is told by the pid file of the output base its `bazel-out` link leads to: of the slot
/// with `--symlink_prefix=out/bazel-` (the IntelliJ monorepo), of a workspace right in the slot (`community/`) with
/// the default link, with the workspace and the output base a scan would read from its argv; the return, the get and
/// the reclaim take it so where no process scan is
#[test]
fn a_server_is_told_by_the_pid_file_of_the_output_base_its_bazel_out_link_leads_to() {
  for (workspace, workspace_file, link) in [("", "MODULE.bazel", "out/bazel-out"), ("community", "WORKSPACE", "bazel-out")] {
    let server = FakeServer::new(workspace, workspace_file, link, sleeper());
    let found = servers_by_output_base(&server.slot);
    assert_eq!(found.iter().map(|found| found.pid).collect::<Vec<_>>(), [server.pid()], "{workspace:?}");
    assert_eq!(found[0].bazel_workspace().as_deref(), Some(server.workspace.as_path()), "{workspace:?}");
    assert_eq!(found[0].output_base(), Some(display_path(&server.output_base).as_str()), "{workspace:?}");
    assert_eq!(found[0].started, process_started(server.pid()), "{workspace:?}");

    let (servers, others) = slot_servers(&ProcessScan::Unavailable, &server.slot);
    assert_eq!(servers, found, "without a scan the slot's servers are told by their output base");
    assert_eq!(others, Vec::<&ProcessInfo>::new());
  }
}

/// No server is told where none can run: no pid file (a server that exited deletes it), a pid that is gone, a pid of a
/// process that started after the pid file was written (the system gave the pid of a killed server to it), a link that
/// leads to no output base, a directory without a workspace file
#[test]
fn no_server_is_told_without_a_running_pid_of_the_pid_file() {
  let server = FakeServer::new("", "MODULE.bazel", "bazel-out", sleeper());
  assert_eq!(servers_by_output_base(&server.slot).len(), 1);

  std::fs::write(&server.pid_file, gone_pid().to_string()).unwrap();
  assert_eq!(servers_by_output_base(&server.slot), Vec::new(), "a pid that is gone");

  std::fs::write(&server.pid_file, server.pid().to_string()).unwrap();
  let file = std::fs::File::options().write(true).open(&server.pid_file).unwrap();
  file.set_modified(SystemTime::now() - Duration::from_secs(3600)).unwrap();
  drop(file);
  assert_eq!(servers_by_output_base(&server.slot), Vec::new(), "a pid that started after the pid file");

  std::fs::write(&server.pid_file, "not a pid").unwrap();
  assert_eq!(servers_by_output_base(&server.slot), Vec::new(), "a pid file without a pid");

  std::fs::remove_file(&server.pid_file).unwrap();
  assert_eq!(servers_by_output_base(&server.slot), Vec::new(), "no pid file");

  std::fs::write(&server.pid_file, server.pid().to_string()).unwrap();
  std::fs::remove_file(server.workspace.join("MODULE.bazel")).unwrap();
  assert_eq!(servers_by_output_base(&server.slot), Vec::new(), "no workspace file");

  let other = FakeServer::new("", "MODULE.bazel", "out/bazel-out", sleeper());
  let elsewhere = other.output_base.join("execroot").join("_main").join("other");
  std::fs::create_dir_all(&elsewhere).unwrap();
  std::fs::remove_dir_all(other.workspace.join("out")).unwrap();
  std::fs::create_dir_all(other.workspace.join("out")).unwrap();
  directory_link(&elsewhere, &other.workspace.join("out").join("bazel-out"));
  assert_eq!(servers_by_output_base(&other.slot), Vec::new(), "a link that leads to no output base");
}

/// A program in place of `bazel`: on Windows a `.cmd` that writes its working directory and arguments into `args.txt`
/// beside it, then runs `then`
#[cfg(windows)]
fn bazel_script(dir: &Path, then: &str) -> PathBuf {
  let script = dir.join("bazel.cmd");
  std::fs::write(&script, format!("@echo %CD% %*> \"{}\"\r\n@{then}\r\n", dir.join("args.txt").display())).unwrap();
  script
}

/// A program in place of `bazel` that fails: it says why on stderr and exits with 3
fn failing_bazel(dir: &Path) -> PathBuf {
  #[cfg(windows)]
  let script = bazel_script(dir, "echo cannot reach the server 1>&2 & exit /b 3");
  #[cfg(unix)]
  let script = {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("bazel");
    std::fs::write(&script, "#!/bin/sh\necho cannot reach the server >&2\nexit 3\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
  };
  script
}

/// `error` is of the `failing_bazel` at `failing`: it names the program and what it said
fn assert_names_the_failing_bazel(error: &str, failing: &Path) {
  assert!(error.contains("cannot reach the server") && error.contains(&failing.display().to_string()), "{error}");
}

/// The stop asks the server to exit and names it in what it stopped, with its workspace and output base: on Windows by
/// `bazel --output_base=<its output base> shutdown` in its workspace, through the `bazel.cmd` of the `PATH` (as an npm
/// Bazelisk installs it, which `CreateProcess` alone would not find; here a script that ends the server), elsewhere by
/// SIGTERM
#[test]
fn the_stop_of_a_server_told_by_its_output_base_names_it_stopped() {
  let mut server = FakeServer::new("", "MODULE.bazel", "out/bazel-out", sleeper());
  let found = servers_by_output_base(&server.slot);
  let scripts = tempfile::tempdir().unwrap();
  #[cfg(windows)]
  let _path = {
    bazel_script(scripts.path(), &format!("taskkill /F /PID {} >NUL 2>&1", server.pid()));
    let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32");
    let path = std::env::join_paths([scripts.path(), system.as_path()]).unwrap();
    EnvVars::set(&[("PATH", path.as_os_str()), ("PATHEXT", OsStr::new(".COM;.EXE;.BAT;.CMD"))])
  };

  let stop = stop_servers(&found.iter().collect::<Vec<_>>(), None, Duration::from_secs(60));
  assert_eq!(stop.running, Vec::new());
  let stopped = stop.stopped;
  assert_eq!(stopped.iter().map(|stopped| stopped.pid).collect::<Vec<_>>(), [server.pid()]);
  assert_eq!(stopped[0].workspace, display_path(&server.workspace));
  assert_eq!(stopped[0].output_base.as_deref(), Some(display_path(&server.output_base).as_str()));
  // a wait, not `try_wait`: macOS tells an exiting process gone a moment before its parent can reap it
  assert!(!is_running(server.pid(), found[0].started), "the server exited");
  server.child.wait().unwrap();
  if cfg!(windows) {
    // without the quotes std puts around an argument with `=` for a batch file
    let args = std::fs::read_to_string(scripts.path().join("args.txt")).unwrap().replace('"', "");
    assert_eq!(
      args.trim(),
      format!("{} --output_base={} shutdown", display_path(&server.workspace), display_path(&server.output_base))
    );
  }
}

/// A server that does not exit when asked is not named stopped and still runs: the return refuses as
/// `ServersRunning`, the get and the reclaim skip the slot as busy. On Windows `bazel shutdown` that ends nothing (here
/// a script that does nothing); elsewhere a server that ignores SIGTERM
#[test]
fn a_server_that_does_not_exit_when_asked_is_not_stopped() {
  let scripts = tempfile::tempdir().unwrap();
  #[cfg(windows)]
  let (child, bazel) = (sleeper(), Some(bazel_script(scripts.path(), "exit /b 0")));
  #[cfg(not(windows))]
  let (child, bazel): (Command, Option<PathBuf>) = {
    let ready = scripts.path().join("ready");
    let mut command = Command::new("sh");
    command.args(["-c", &format!("trap '' TERM; touch '{}'; exec sleep 120", ready.display())]);
    (command, None)
  };
  let mut server = FakeServer::new("", "MODULE.bazel", "out/bazel-out", child);
  #[cfg(not(windows))]
  while !scripts.path().join("ready").exists() && server.child.try_wait().unwrap().is_none() {
    std::thread::sleep(Duration::from_millis(10));
  }
  let found = servers_by_output_base(&server.slot);
  assert_eq!(found.len(), 1);

  let stop = stop_servers(&found.iter().collect::<Vec<_>>(), bazel.as_deref().map(Path::as_os_str), Duration::from_millis(300));
  assert_eq!(stop.stopped, Vec::new());
  assert_eq!(
    stop.running.iter().map(|running| (running.pid, running.stop_error.clone())).collect::<Vec<_>>(),
    [(server.pid(), None)]
  );
  assert!(server.child.try_wait().unwrap().is_none(), "the server still runs");
  assert!(is_running(server.pid(), found[0].started));
}

/// A `bazel shutdown` that can't run (a program that is not there, a bare name on no directory of the `PATH`) or that
/// fails says why and names the program; the server is never signalled and runs on
#[test]
fn a_bazel_shutdown_that_cannot_run_or_fails_says_why_and_names_the_program() {
  let mut server = FakeServer::new("", "MODULE.bazel", "out/bazel-out", sleeper());
  let found = servers_by_output_base(&server.slot);
  assert_eq!(found.len(), 1);
  let scripts = tempfile::tempdir().unwrap();
  let missing = scripts.path().join("missing").join(if cfg!(windows) { "bazel.exe" } else { "bazel" });
  let failing = failing_bazel(scripts.path());

  let error = shutdown(Some(missing.as_os_str()), &found[0], Duration::from_secs(60)).unwrap_err();
  assert!(error.contains("cannot run") && error.contains(&missing.display().to_string()), "{error}");
  let error = shutdown(Some(OsStr::new("bd-test-no-such-bazel")), &found[0], Duration::from_secs(60)).unwrap_err();
  assert!(error.contains("no bd-test-no-such-bazel on the PATH"), "{error}");
  assert_names_the_failing_bazel(&shutdown(Some(failing.as_os_str()), &found[0], Duration::from_secs(60)).unwrap_err(), &failing);
  assert!(server.child.try_wait().unwrap().is_none(), "the server still runs");
}

/// A server whose `bazel shutdown` can't run (no `bazel` on the `PATH`) or fails is not waited for: it is not named
/// stopped, runs on, and is named still running with why, which the refusal of the return words
#[cfg(windows)]
#[test]
fn a_server_whose_bazel_shutdown_cannot_run_or_fails_is_named_running_with_why() {
  let mut server = FakeServer::new("", "MODULE.bazel", "out/bazel-out", sleeper());
  let servers = servers_by_output_base(&server.slot);
  let found: Vec<&ProcessInfo> = servers.iter().collect();
  let scripts = tempfile::tempdir().unwrap();
  let failing = failing_bazel(scripts.path());
  let no_bazel = EnvVars::set(&[("PATH", scripts.path().join("empty").as_os_str())]);

  let stop = stop_servers(&found, None, Duration::from_secs(60));
  drop(no_bazel);
  assert_eq!(stop.stopped, Vec::new());
  assert_eq!(stop.running.iter().map(|running| running.pid).collect::<Vec<_>>(), [server.pid()]);
  let error = stop.running[0].stop_error.clone().unwrap();
  assert!(error.contains("no bazel on the PATH"), "{error}");
  let refusal = LeaseRefusal::ServersRunning {
    path: display_path(&server.slot),
    processes: stop.running,
  };
  assert!(refusal.to_string().contains(&error), "{refusal}");

  let stop = stop_servers(&found, Some(failing.as_os_str()), Duration::from_secs(60));
  assert_eq!(stop.stopped, Vec::new());
  assert_names_the_failing_bazel(stop.running[0].stop_error.as_deref().unwrap(), &failing);
  assert!(server.child.try_wait().unwrap().is_none(), "the server still runs");
}

/// A bare program name is looked up as `cmd` looks it up: by the directories of the `PATH` in order, in each by the
/// extensions of `PATHEXT` in order (`bazel.bat` before `bazel.cmd`), the name alone where no extensions are (Unix)
#[test]
fn a_bare_program_is_found_by_the_path_and_the_extensions_of_pathext() {
  let dir = tempfile::tempdir().unwrap();
  let [empty, shims, binaries] = ["empty", "shims", "binaries"].map(|name| dir.path().join(name));
  for (dir, files) in [(&empty, &[][..]), (&shims, &["bazel.cmd", "bazel.bat"][..]), (&binaries, &["bazel.exe", "bazel"][..])] {
    std::fs::create_dir_all(dir).unwrap();
    for file in files {
      std::fs::write(dir.join(file), "").unwrap();
    }
  }
  let path = std::env::join_paths([&empty, &shims, &binaries]).unwrap();
  let bazel = OsStr::new("bazel");

  assert_eq!(find_program(bazel, &path, ".COM;.EXE;.BAT;.CMD"), Some(shims.join("bazel.bat")));
  std::fs::remove_file(shims.join("bazel.bat")).unwrap();
  assert_eq!(find_program(bazel, &path, ".COM;.EXE;.BAT;.CMD"), Some(shims.join("bazel.cmd")));
  assert_eq!(find_program(bazel, &path, ".COM;.EXE"), Some(binaries.join("bazel.exe")));
  assert_eq!(find_program(bazel, &path, ""), Some(binaries.join("bazel")));
  assert_eq!(find_program(OsStr::new("bazelisk"), &path, ".COM;.EXE;.BAT;.CMD"), None);
}
