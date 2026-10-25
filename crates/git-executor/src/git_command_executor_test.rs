use crate::git_command_executor::{GitCommandExecutor, ProgressStop, StoppedCommand, read_progress_lines};
use anyhow::Context;
use std::cell::Cell;
use std::ops::ControlFlow;
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

/// An empty repository and its path
fn repo() -> (tempfile::TempDir, String) {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().to_str().unwrap().to_string();
  GitCommandExecutor::new().cmd(&["init", "-q", "-b", "master"], &path).text().unwrap();
  (dir, path)
}

/// Writes `content` as a blob and returns its id
fn blob(git: &GitCommandExecutor, repo: &str, content: &[u8]) -> String {
  git.cmd(&["hash-object", "-w", "--stdin"], repo).stdin(content).text().unwrap()
}

#[test]
fn text_is_trimmed_bytes_are_untouched_and_lines_skip_empty_ones() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new();
  let id = blob(&git, &repo, b"\n  first \n\nsecond\n\n");
  assert_eq!(git.cmd(&["cat-file", "blob", &id], &repo).text().unwrap(), "first \n\nsecond");
  assert_eq!(git.cmd(&["cat-file", "blob", &id], &repo).bytes().unwrap(), b"\n  first \n\nsecond\n\n");
  assert_eq!(git.cmd(&["cat-file", "blob", &id], &repo).lines().unwrap(), ["first", "second"]);
}

#[test]
fn text_that_is_not_utf8_is_decoded_lossily() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new();
  let id = blob(&git, &repo, b"caf\xe9 \xc3\xa9\n");
  assert_eq!(git.cmd(&["cat-file", "blob", &id], &repo).text().unwrap(), "caf\u{fffd} \u{e9}");
}

#[test]
fn an_accepted_exit_code_is_success_and_is_reported() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new();
  // `--quiet` exits with 1 for a revision that does not resolve, without it git exits with 128
  let missing = ["rev-parse", "--verify", "--quiet", "missing"];
  let error = git.cmd(&missing, &repo).text().unwrap_err();
  assert!(format!("{error:#}").contains("rev-parse --verify --quiet missing"), "{error:#}");
  assert_eq!(git.cmd(&missing, &repo).accept(&[1]).text().unwrap(), "");
  let output = git.cmd(&missing, &repo).accept(&[1]).output().unwrap();
  assert_eq!((output.stdout, output.code), (Vec::new(), 1));

  let error = git.cmd(&["rev-parse", "--verify", "missing"], &repo).accept(&[1]).output().unwrap_err();
  assert!(format!("{error:#}").ends_with("Error: fatal: Needed a single revision"), "{error:#}");
}

#[test]
fn a_status_reports_a_failure_with_its_stderr_and_exit_code() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new();
  let output = git.cmd(&["rev-parse", "--verify", "missing"], &repo).status().unwrap();
  assert_eq!((output.stdout, output.stderr.as_str(), output.code), (Vec::new(), "fatal: Needed a single revision", 128));

  let output = git.cmd(&["rev-parse", "--show-toplevel"], &repo).status().unwrap();
  assert_eq!(output.code, 0);
  assert!(Path::new(&output.into_text()).ends_with(Path::new(&repo).file_name().unwrap()));
}

#[test]
fn the_environment_reaches_every_finisher() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new();
  let env = [("GIT_COMMITTER_NAME", "Builder Env"), ("GIT_COMMITTER_EMAIL", "env@example.com")];
  let ident = |text: &str| text.starts_with("Builder Env <env@example.com> ");
  let args = ["var", "GIT_COMMITTER_IDENT"];
  assert!(ident(&git.cmd(&args, &repo).env(&env).text().unwrap()));
  assert!(ident(&git.cmd(&args, &repo).env(&env).status().unwrap().into_text()));

  let mut streamed = Vec::new();
  git
    .cmd(&args, &repo)
    .env(&env)
    .stream(|chunk| {
      streamed.extend_from_slice(chunk);
      Ok(ControlFlow::Continue(()))
    })
    .unwrap();
  assert!(ident(&String::from_utf8(streamed).unwrap()));

  // a git with piped stdin that answers once: `var` does not read it
  let mut interactive = git.cmd(&args, &repo).env(&env).interactive(b'\n').unwrap();
  assert!(ident(&interactive.read_line(Duration::from_secs(30)).unwrap()));
  interactive.finish().unwrap();
}

#[test]
fn a_stream_gets_stdin() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new();
  let id = blob(&git, &repo, b"streamed");
  let mut streamed = Vec::new();
  git
    .cmd(&["cat-file", "--batch"], &repo)
    .stdin(format!("{id}\n").as_bytes())
    .stream(|chunk| {
      streamed.extend_from_slice(chunk);
      Ok(ControlFlow::Continue(()))
    })
    .unwrap();
  assert_eq!(String::from_utf8(streamed).unwrap(), format!("{id} blob 8\nstreamed\n"));
}

/// Runs `stream` on its own thread: a git that the stream does not stop would keep it waiting, so the test fails
/// after a minute instead of hanging
fn within_a_minute<T: Send + 'static>(stream: impl FnOnce() -> T + Send + 'static) -> T {
  let (sender, receiver) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    let _ = sender.send(stream());
  });
  receiver.recv_timeout(Duration::from_mins(1)).expect("the stream did not stop git")
}

#[test]
fn a_stream_that_breaks_kills_git_and_counts_it() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new().with_process_stats("test");
  // far more than the pipe and the queue of chunks hold: a git that is not killed waits for a reader forever
  let id = blob(&git, &repo, &vec![b'x'; 16 * 1024 * 1024]);
  let (chunks, result) = within_a_minute({
    let git = git.clone();
    move || {
      let mut chunks = 0;
      let result = git.cmd(&["cat-file", "blob", &id], &repo).stream(|_| {
        chunks += 1;
        Ok(ControlFlow::Break(()))
      });
      (chunks, result)
    }
  });
  result.unwrap();
  assert_eq!(chunks, 1, "no chunk after the break");
  let by_command = git.process_stats().unwrap().by_command();
  assert_eq!(by_command.get("cat-file").map(|stats| stats.count), Some(1), "{by_command:#?}");
}

#[test]
fn a_stream_handler_error_kills_git_and_is_the_result() {
  let (_dir, repo) = repo();
  let git = GitCommandExecutor::new();
  let id = blob(&git, &repo, &vec![b'x'; 16 * 1024 * 1024]);
  let result = within_a_minute(move || {
    git
      .cmd(&["cat-file", "blob", &id], &repo)
      .stream(|_| Err(anyhow::anyhow!("enough")))
      .map_err(|e| e.to_string())
  });
  assert_eq!(result, Err("enough".to_string()));
}

#[test]
fn a_failed_stream_is_an_error_with_stderr() {
  let (_dir, repo) = repo();
  let error = GitCommandExecutor::new()
    .cmd(&["cat-file", "blob", "HEAD"], &repo)
    .stream(|_| Ok(ControlFlow::Continue(())))
    .unwrap_err();
  assert!(format!("{error:#}").contains("Error: fatal: "), "{error:#}");
}

/// The stderr of an interactive git is kept when it is not UTF-8: a failed reply still says why
#[test]
fn an_interactive_failure_keeps_a_message_that_is_not_utf8() {
  let dir = tempfile::tempdir().unwrap();
  let git = GitCommandExecutor::new();
  let args = ["-c", "alias.fail=!printf 'fatal: caf\\351 broken\\n' >&2; exit 3", "fail"];
  let mut interactive = git.cmd(&args, dir.path().to_str().unwrap()).interactive(b'\n').unwrap();
  let error = interactive.read_line(Duration::from_secs(30)).unwrap_err();
  assert!(error.to_string().ends_with("failed: fatal: caf\u{fffd} broken"), "{error}");
}

#[test]
fn failed_command_reports_messages_without_progress_updates() {
  let dir = std::env::temp_dir();
  let mut lines = Vec::new();
  let error = GitCommandExecutor::new()
    .cmd(
      &["-c", "alias.fail=!printf 'step 1\\rstep 2\\rfatal: broken\\n' >&2; exit 3", "fail"],
      dir.to_str().unwrap(),
    )
    .progress(&mut |line| lines.push(line.to_string()), &ProgressStop::NEVER)
    .unwrap_err();
  assert_eq!(lines, ["step 1", "step 2", "fatal: broken"]);
  let error = format!("{error:#}");
  assert!(error.ends_with("Error: fatal: broken"), "{error}");
}

/// An accepted exit code keeps the messages of stderr, without the progress updates: a `push` whose ref the remote
/// refused exits with 1, and what the remote said is in them
#[test]
fn an_accepted_failure_keeps_the_messages_of_stderr() {
  let dir = std::env::temp_dir();
  let output = GitCommandExecutor::new()
    .cmd(
      &["-c", "alias.refuse=!printf 'Writing 1\\rWriting 2\\nremote: refused\\n' >&2; echo report; exit 1", "refuse"],
      dir.to_str().unwrap(),
    )
    .accept(&[1])
    .progress(&mut |_| {}, &ProgressStop::NEVER)
    .unwrap();
  assert_eq!((output.code, output.stderr.as_str()), (1, "Writing 2\nremote: refused"));
  assert_eq!(output.into_text(), "report");
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
  Ok(git.cmd(&args, dir.to_str().context("a UTF-8 path")?).clock(now).progress(on_line, stop)?.into_text())
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

/// Runs `script` as a git alias in `dir`, canceled once git reported something, so it surely runs
fn run_until_reported(script: &str, dir: &Path) -> anyhow::Result<String> {
  let reported = Cell::new(false);
  let stop = ProgressStop {
    idle_timeout: None,
    is_cancelled: &|| reported.get(),
  };
  run_until(script, dir, &mut |_| reported.set(true), &stop, &Instant::now)
}

#[test]
fn a_cancelled_git_is_killed() {
  let dir = holding_dir();
  let result = run_until_reported(&format!("printf 'step 1\\n' >&2; {HOLD_STDERR}"), dir.path());
  assert_eq!(stopped(&result), Some(StoppedCommand::Cancelled), "{result:?}");
  assert_returned_while_stderr_held(&dir);
}

/// On Windows `git` is mostly the launcher of Git for Windows (`Git\cmd\git.exe`), which starts the real git as its child:
/// a stop ends the launcher, the git and the shell of the alias (`process_tree.rs`), not the launcher alone
#[cfg(windows)]
#[test]
fn a_stopped_git_ends_every_process_it_started() {
  let dir = tempfile::tempdir().unwrap();
  // the shell adds a line to `beats` every 50 ms, for 30 s at most; it is canceled at its first line
  let script = "i=0; while [ $i -lt 600 ]; do echo $i >> beats; printf 'beat\\n' >&2; sleep 0.05; i=$((i+1)); done";
  let result = run_until_reported(script, dir.path());
  assert_eq!(stopped(&result), Some(StoppedCommand::Cancelled), "{result:?}");
  let beats = || std::fs::read_to_string(dir.path().join("beats")).map_or(0, |text| text.lines().count());
  // a beat may be on its way while the processes end
  std::thread::sleep(Duration::from_millis(200));
  let at_the_stop = beats();
  assert!(at_the_stop > 0, "the shell never ran");
  // a shell that still ran would add about ten
  std::thread::sleep(Duration::from_millis(500));
  assert_eq!(beats(), at_the_stop, "a process of the stopped git still runs");
}

/// Runs `script` as a git alias in `dir` with `input` on stdin, killed after `timeout` measured on `now`
fn run_with_input(script: &str, dir: &Path, input: &str, timeout: Duration, now: &dyn Fn() -> Instant) -> anyhow::Result<(Vec<u8>, i32)> {
  let alias = format!("alias.run=!{script}");
  let args = ["-c", alias.as_str(), "run"];
  let output = GitCommandExecutor::new()
    .cmd(&args, dir.to_str().context("a UTF-8 path")?)
    .env(&[("BD_TEST", "set")])
    .stdin(input.as_bytes())
    .timeout(timeout)
    .clock(now)
    .status()?;
  Ok((output.stdout, output.code))
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
