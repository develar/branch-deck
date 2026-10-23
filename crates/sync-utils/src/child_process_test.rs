use crate::child_process::{ChildProcessError, ToolDir, ToolLookup, find_executable, find_tool_in, run_child};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use test_utils::fake_executable::fake_executable;

const TIMEOUT: Duration = Duration::from_secs(10);

/// A child that copies its stdin to stdout, or one that hangs: `cat` and `sleep` of the fake's sh, which Windows has
/// only in Git's `usr\bin`, not always on `PATH`
fn fake(dir: &tempfile::TempDir, body: &str) -> std::path::PathBuf {
  fake_executable(dir.path(), "child", body)
}

#[test]
fn writes_stdin_while_reading_stdout() {
  // more than a pipe buffer in both directions: writing all input before reading would deadlock
  let input = "0123456789abcdef\n".repeat(64 * 1024).into_bytes();
  let dir = tempfile::tempdir().unwrap();
  let output = run_child(&mut Command::new(fake(&dir, "exec cat")), Some(input.clone()), TIMEOUT, &|| false).unwrap();
  assert!(output.status.success());
  assert_eq!(output.stdout, input);
}

#[test]
fn reports_a_missing_binary() {
  let dir = tempfile::tempdir().unwrap();
  let result = run_child(&mut Command::new(dir.path().join("missing")), None, TIMEOUT, &|| false);
  assert!(matches!(result, Err(ChildProcessError::NotFound)), "{result:?}");
}

#[test]
fn kills_the_child_after_the_timeout() {
  let dir = tempfile::tempdir().unwrap();
  let mut hang = Command::new(fake(&dir, "exec sleep 30"));
  let started = Instant::now();
  let result = run_child(&mut hang, None, Duration::from_millis(300), &|| false);
  assert!(matches!(result, Err(ChildProcessError::TimedOut)), "{result:?}");
  assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn kills_the_child_when_superseded() {
  let dir = tempfile::tempdir().unwrap();
  let mut hang = Command::new(fake(&dir, "exec sleep 30"));
  let superseded = Arc::new(AtomicBool::new(false));
  let flag = Arc::clone(&superseded);
  std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(100));
    flag.store(true, Ordering::SeqCst);
  });
  let started = Instant::now();
  let result = run_child(&mut hang, None, TIMEOUT, &|| superseded.load(Ordering::SeqCst));
  assert!(matches!(result, Err(ChildProcessError::Superseded)), "{result:?}");
  assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn the_failure_message_is_the_first_stderr_line_or_the_exit_code() {
  let dir = tempfile::tempdir().unwrap();
  let tool = fake_executable(dir.path(), "tool", "echo >&2\necho '  boom  ' >&2\necho second >&2\nexit 3");
  let output = run_child(&mut Command::new(&tool), None, TIMEOUT, &|| false).unwrap();
  assert_eq!(output.failure_message("tool"), "boom");

  let tool = fake_executable(dir.path(), "tool", "exit 3");
  let output = run_child(&mut Command::new(&tool), None, TIMEOUT, &|| false).unwrap();
  assert_eq!(output.failure_message("tool"), "tool exited with code 3");
}

#[test]
fn finds_the_first_executable_in_path() {
  let first = tempfile::tempdir().unwrap();
  let second = tempfile::tempdir().unwrap();
  let expected = fake_executable(second.path(), "tool", "true");
  let path = std::env::join_paths([first.path(), second.path()]).unwrap();
  assert_eq!(find_executable("tool", &path), Some(expected));
  assert_eq!(find_executable("other", &path), None);
}

#[test]
fn finds_a_tool_before_path_then_on_path_then_after_it() {
  const LOOKUP: ToolLookup = ToolLookup {
    name: "tool",
    before_path: &[ToolDir::Home("first")],
    after_path: &[ToolDir::Home("last")],
    login_shell: false,
  };
  let home = tempfile::tempdir().unwrap();
  let bin = tempfile::tempdir().unwrap();
  let path = bin.path().as_os_str();
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), None);

  std::fs::create_dir_all(home.path().join("last")).unwrap();
  let last = fake_executable(&home.path().join("last"), "tool", "true");
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), Some(last));

  let on_path = fake_executable(bin.path(), "tool", "true");
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), Some(on_path.clone()));
  // without a home directory, the home dirs are skipped, not an error
  assert_eq!(find_tool_in(&LOOKUP, None, Some(path)), Some(on_path));

  std::fs::create_dir_all(home.path().join("first")).unwrap();
  let first = fake_executable(&home.path().join("first"), "tool", "true");
  assert_eq!(find_tool_in(&LOOKUP, Some(home.path()), Some(path)), Some(first));
}
