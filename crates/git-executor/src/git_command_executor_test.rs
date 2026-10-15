use crate::git_command_executor::{GitCommandExecutor, ProgressStop, StoppedCommand, read_progress_lines};
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Runs `script` as a git alias (a shell that git waits for) until `stop`
fn run_until(script: &str, stop: &ProgressStop<'_>) -> (anyhow::Result<String>, Vec<String>) {
  let dir = std::env::temp_dir();
  let alias = format!("alias.run=!{script}");
  let mut lines = Vec::new();
  let result =
    GitCommandExecutor::new().execute_command_with_progress_until(&["-c", &alias, "run"], dir.to_str().unwrap(), &[], &[], &mut |line| lines.push(line.to_string()), stop);
  (result, lines)
}

fn stopped(result: &anyhow::Result<String>) -> Option<StoppedCommand> {
  result.as_ref().err().and_then(|e| e.downcast_ref::<StoppedCommand>()).copied()
}

#[test]
fn a_git_that_reports_nothing_for_the_idle_timeout_is_killed() {
  let started = Instant::now();
  // the sleeping shell keeps stderr open after git is killed: the result must not wait for it
  let (result, lines) = run_until(
    "printf 'step 1\\n' >&2; sleep 30",
    &ProgressStop {
      idle_timeout: Some(Duration::from_millis(300)),
      is_cancelled: &|| false,
    },
  );
  assert_eq!(stopped(&result), Some(StoppedCommand::Stalled(Duration::from_millis(300))));
  assert_eq!(lines, ["step 1"]);
  assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
}

#[test]
fn progress_resets_the_idle_timeout() {
  let (result, lines) = run_until(
    "for i in 1 2 3 4 5; do printf \"$i\\r\" >&2; sleep 0.1; done; echo out",
    &ProgressStop {
      idle_timeout: Some(Duration::from_millis(400)),
      is_cancelled: &|| false,
    },
  );
  assert_eq!(result.unwrap(), "out");
  assert_eq!(lines, ["1", "2", "3", "4", "5"]);
}

#[test]
fn a_cancelled_git_is_killed() {
  let cancelled = AtomicBool::new(false);
  let started = Instant::now();
  let watcher = std::thread::scope(|scope| {
    scope.spawn(|| {
      std::thread::sleep(Duration::from_millis(200));
      cancelled.store(true, Ordering::SeqCst);
    });
    run_until(
      "sleep 30",
      &ProgressStop {
        idle_timeout: None,
        is_cancelled: &|| cancelled.load(Ordering::SeqCst),
      },
    )
  });
  assert_eq!(stopped(&watcher.0), Some(StoppedCommand::Cancelled));
  assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
}
