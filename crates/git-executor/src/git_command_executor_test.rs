use crate::git_command_executor::{GitCommandExecutor, ProgressStop, StoppedCommand, read_progress_lines};
use std::cell::Cell;
use std::path::Path;
use std::time::{Duration, Instant};

fn read(input: &str) -> (Vec<String>, String) {
  let mut lines = Vec::new();
  let messages = read_progress_lines(input.as_bytes(), &mut |line| lines.push(line.to_string()));
  (lines, messages)
}

#[test]
fn progress_updates_are_lines_but_not_messages() {
  let (lines, messages) = read("remote: Counting objects:  50% (1/2)   \rremote: Counting objects: 100% (2/2), done.   \nReceiving objects:  50% (1/2)\rfatal: early EOF\n");
  assert_eq!(
    lines,
    [
      "remote: Counting objects:  50% (1/2)",
      "remote: Counting objects: 100% (2/2), done.",
      "Receiving objects:  50% (1/2)",
      "fatal: early EOF"
    ]
  );
  assert_eq!(messages, "remote: Counting objects: 100% (2/2), done.\nfatal: early EOF\n");
}

#[test]
fn crlf_lines_are_messages() {
  let (lines, messages) = read("error: one\r\nfatal: two");
  assert_eq!(lines, ["error: one", "fatal: two"]);
  assert_eq!(messages, "error: one\nfatal: two");
}

#[test]
fn failed_command_reports_messages_without_progress_updates() {
  let dir = std::env::temp_dir();
  let mut lines = Vec::new();
  let error = GitCommandExecutor::new()
    .execute_command_with_progress(
      &["-c", "alias.fail=!printf 'step 1\\rstep 2\\rfatal: broken\\n' >&2; exit 3", "fail"],
      dir.to_str().unwrap(),
      &[],
      &[],
      &mut |line| lines.push(line.to_string()),
    )
    .unwrap_err();
  assert_eq!(lines, ["step 1", "step 2", "fatal: broken"]);
  let error = format!("{error:#}");
  assert!(error.ends_with("Error: fatal: broken"), "{error}");
}

/// The idle clock of a test: it stands still until the test moves it on an event of git, so the idle timeout follows
/// the order of the events, not how fast a loaded machine starts git and the shell
struct TestClock {
  start: Instant,
  elapsed: Cell<Duration>,
}

impl TestClock {
  fn new() -> Self {
    Self {
      start: Instant::now(),
      elapsed: Cell::new(Duration::ZERO),
    }
  }

  fn now(&self) -> Instant {
    self.start + self.elapsed.get()
  }

  fn advance(&self, by: Duration) {
    self.elapsed.set(self.elapsed.get() + by);
  }
}

/// Appended to a script, it keeps the shell, and so the pipes of git, alive after git is killed: while `hold` exists
/// in its directory (at most about 30 s), then it writes `exited`
const HOLD_STDERR: &str = "i=0; while [ -e hold ] && [ $i -lt 300 ]; do sleep 0.1; i=$((i+1)); done; touch exited 2>/dev/null";

/// A directory with `hold` for a script that ends with `HOLD_STDERR`
fn holding_dir() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("hold"), "").unwrap();
  dir
}

/// The shell of `HOLD_STDERR` still holds the pipes: the result did not wait for it. Then it is let go.
fn assert_returned_while_stderr_held(dir: &tempfile::TempDir) {
  assert!(!dir.path().join("exited").exists(), "the result waited for the shell that holds stderr open");
  std::fs::remove_file(dir.path().join("hold")).unwrap();
}

/// Runs `script` as a git alias (a shell that git waits for) in `dir` until `stop`, the idle timeout measured on `now`
fn run_until(script: &str, dir: &Path, on_line: &mut dyn FnMut(&str), stop: &ProgressStop<'_>, now: &dyn Fn() -> Instant) -> anyhow::Result<String> {
  let alias = format!("alias.run=!{script}");
  let args = ["-c", alias.as_str(), "run"];
  let git = GitCommandExecutor::new();
  let command = git.command(&args, dir.to_str().unwrap(), &[]).unwrap();
  git.run_with_progress(command, &args, &[], on_line, stop, now)
}

fn stopped(result: &anyhow::Result<String>) -> Option<StoppedCommand> {
  result.as_ref().err().and_then(|e| e.downcast_ref::<StoppedCommand>()).copied()
}

#[test]
fn a_git_that_reports_nothing_for_the_idle_timeout_is_killed() {
  let dir = holding_dir();
  // silence before the first line is not counted: however long git and the shell take to start, `step 1` comes first;
  // after it, every look at the clock is 50 ms of silence
  let clock = TestClock::new();
  let reported = Cell::new(false);
  let now = || {
    if reported.get() {
      clock.advance(Duration::from_millis(50));
    }
    clock.now()
  };
  let mut lines = Vec::new();
  let result = run_until(
    &format!("printf 'step 1\\n' >&2; {HOLD_STDERR}"),
    dir.path(),
    &mut |line| {
      lines.push(line.to_string());
      reported.set(true);
    },
    &ProgressStop {
      idle_timeout: Some(Duration::from_millis(300)),
      is_cancelled: &|| false,
    },
    &now,
  );
  assert_eq!(stopped(&result), Some(StoppedCommand::Stalled(Duration::from_millis(300))), "{result:?}");
  assert_eq!(lines, ["step 1"]);
  assert_returned_while_stderr_held(&dir);
}

#[test]
fn progress_resets_the_idle_timeout() {
  let dir = tempfile::tempdir().unwrap();
  // 300 ms pass before each line: 1.5 s in all, beyond the 400 ms idle timeout, which only a reset on every line passes
  let clock = TestClock::new();
  let mut lines = Vec::new();
  let result = run_until(
    "for i in 1 2 3 4 5; do printf \"$i\\r\" >&2; done; echo out",
    dir.path(),
    &mut |line| {
      clock.advance(Duration::from_millis(300));
      lines.push(line.to_string());
    },
    &ProgressStop {
      idle_timeout: Some(Duration::from_millis(400)),
      is_cancelled: &|| false,
    },
    &|| clock.now(),
  );
  assert_eq!(result.unwrap(), "out");
  assert_eq!(lines, ["1", "2", "3", "4", "5"]);
}

#[test]
fn a_cancelled_git_is_killed() {
  let dir = holding_dir();
  // canceled once git reported something, so it surely runs
  let reported = Cell::new(false);
  let result = run_until(
    &format!("printf 'step 1\\n' >&2; {HOLD_STDERR}"),
    dir.path(),
    &mut |_| reported.set(true),
    &ProgressStop {
      idle_timeout: None,
      is_cancelled: &|| reported.get(),
    },
    &Instant::now,
  );
  assert_eq!(stopped(&result), Some(StoppedCommand::Cancelled), "{result:?}");
  assert_returned_while_stderr_held(&dir);
}

/// Runs `script` as a git alias in `dir` with `input` on stdin, killed after `timeout` measured on `now`
fn run_with_input(script: &str, dir: &Path, input: &str, timeout: Duration, now: &dyn Fn() -> Instant) -> anyhow::Result<(Vec<u8>, i32)> {
  let alias = format!("alias.run=!{script}");
  let args = ["-c", alias.as_str(), "run"];
  let command = GitCommandExecutor::new().command(&args, dir.to_str().unwrap(), &[("BD_TEST", "set")]).unwrap();
  GitCommandExecutor::run_with_input(command, &args, input.as_bytes(), timeout, now)
}

#[test]
fn a_timed_command_reads_stdin_and_reports_a_failure_as_its_exit_code() {
  let dir = tempfile::tempdir().unwrap();
  // the clock stands still: however slow the machine, the timeout never comes
  let clock = TestClock::new();
  let (stdout, code) = run_with_input(
    "read line; printf \"$line $BD_TEST\"; exit 3",
    dir.path(),
    "host=github.com\n",
    Duration::from_secs(10),
    &|| clock.now(),
  )
  .unwrap();
  assert_eq!(String::from_utf8(stdout).unwrap(), "host=github.com set");
  assert_eq!(code, 3);
}

#[test]
fn a_timed_command_is_killed_after_its_timeout() {
  let dir = holding_dir();
  // the clock stands still until the shell runs, however long git takes to start it; then every look is 100 ms
  let clock = TestClock::new();
  let now = || {
    if dir.path().join("started").exists() {
      clock.advance(Duration::from_millis(100));
    }
    clock.now()
  };
  let result = run_with_input(&format!("touch started; {HOLD_STDERR}"), dir.path(), "", Duration::from_millis(300), &now);
  assert_eq!(stopped(&result.map(|_| String::new())), Some(StoppedCommand::TimedOut(Duration::from_millis(300))));
  assert_returned_while_stderr_held(&dir);
}
