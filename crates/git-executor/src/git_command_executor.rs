use crate::git_info::GitInfo;
use crate::interactive_command::InteractiveCommand;
use crate::process_stats::ProcessStats;
use crate::process_tree::ProcessTree;
use anyhow::{Result, anyhow};
use std::any::Any;
use std::io::{Read, Write};
use std::ops::ControlFlow;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{RecvTimeoutError, SyncSender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tracing::instrument;

/// Environment set for every git invocation: never prompt (there is no terminal to answer), never take optional locks
/// (read-only commands like `status` must not contend with the user's git), and show the notes of `%N` from the default
/// notes ref only (`core.notesRef`, the mapping notes) or from an explicit `--notes=<ref>`. An empty
/// `GIT_NOTES_DISPLAY_REF` replaces an inherited one and turns off `notes.displayRef`, which would add every note of
/// `refs/notes/*` to `%N`.
const BASE_ENV: [(&str, &str); 3] = [("GIT_TERMINAL_PROMPT", "0"), ("GIT_OPTIONAL_LOCKS", "0"), ("GIT_NOTES_DISPLAY_REF", "")];

/// Inherited variables that would redirect git away from `repository_path` or its real index and notes
/// (the app may be started from a git hook or a shell inside another repository).
/// A temporary index is passed explicitly through the per-call `env`, which is applied after this removal.
const REMOVED_ENV: [&str; 4] = ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_NOTES_REF"];

/// Config overrides for every invocation, so output that we parse does not depend on the user's config:
/// unquoted UTF-8 paths, no colour codes, no signature verification text in `log`/`show` output, and messages in
/// UTF-8 (`i18n.logOutputEncoding`, which defaults to `i18n.commitEncoding`; a message git cannot re-encode keeps its
/// bytes). Callers don't need `--no-pager` either: git never starts a pager when stdout is not a terminal.
const BASE_CONFIG: [&str; 8] = [
  "-c",
  "core.quotePath=false",
  "-c",
  "color.ui=never",
  "-c",
  "log.showSignature=false",
  "-c",
  "i18n.logOutputEncoding=UTF-8",
];

/// The git `GitInfo::discover` found, shared by every executor of the process that has no git of its own (`with_info`):
/// each `new()` executor would otherwise run `git version` again on its first command
static DISCOVERED: OnceLock<GitInfo> = OnceLock::new();

#[derive(Clone, Debug, Default)]
pub struct GitCommandExecutor {
  info: Arc<OnceLock<GitInfo>>,
  /// Counts the processes of one operation (`with_process_stats`)
  stats: Option<Arc<ProcessStats>>,
  /// Variables every process of this executor gets (`with_env`)
  env: Arc<[(String, String)]>,
  /// What the operation of this executor read once at its entry, for every step it passes the executor to
  /// (`with_fact`)
  facts: Facts,
}

/// The facts of `GitCommandExecutor::with_fact`, one per type. The executor never reads or interprets them: a caller
/// (`git_ops::revs::with_repository_dirs`) puts a fact there and the helpers of the same crate take it back
#[derive(Clone, Default)]
struct Facts(Arc<[Arc<dyn Any + Send + Sync>]>);

impl std::fmt::Debug for Facts {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "Facts({})", self.0.len())
  }
}

impl GitCommandExecutor {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// An executor that runs an already discovered git instead of discovering it on first use
  #[must_use]
  pub fn with_info(info: GitInfo) -> Self {
    Self {
      info: Arc::new(OnceLock::from(info)),
      ..Self::default()
    }
  }

  /// A clone that carries `fact`, in place of a fact of the same type, to every step its operation passes it to: an
  /// operation reads a repository fact once (docs/design/git-operations.md, "Batch")
  #[must_use]
  pub fn with_fact<T: Any + Send + Sync>(&self, fact: T) -> Self {
    let mut facts: Vec<Arc<dyn Any + Send + Sync>> = self.facts.0.iter().filter(|known| !(***known).is::<T>()).cloned().collect();
    facts.push(Arc::new(fact));
    Self {
      facts: Facts(facts.into()),
      ..self.clone()
    }
  }

  /// The fact of type `T` that `with_fact` put here
  pub fn fact<T: Any>(&self) -> Option<&T> {
    self.facts.0.iter().find_map(|fact| (**fact).downcast_ref::<T>())
  }

  /// A clone that counts its git processes, and those of its clones, for `operation`; the summary is logged when the
  /// last of them is dropped (`process_stats.rs`). An executor that counts already keeps its stats: an operation inside
  /// another (the sync of `bd sync`) counts into the outer one
  #[must_use]
  pub fn with_process_stats(&self, operation: &str) -> Self {
    if self.stats.is_some() {
      return self.clone();
    }
    Self {
      stats: Some(Arc::new(ProcessStats::new(operation))),
      ..self.clone()
    }
  }

  /// A clone whose git processes, and those of its clones, also get `name=value`, after the base environment and before
  /// the variables of a call. A test traces its own processes this way (`GIT_TRACE2_EVENT`), never through the process
  /// environment, which the tests of one process share
  #[must_use]
  pub fn with_env(&self, name: &str, value: &str) -> Self {
    let mut env = self.env.to_vec();
    env.push((name.to_string(), value.to_string()));
    Self { env: env.into(), ..self.clone() }
  }

  /// The stats of `with_process_stats`
  pub fn process_stats(&self) -> Option<&ProcessStats> {
    self.stats.as_deref()
  }

  fn record_process(&self, args: &[&str], started: Instant) {
    let elapsed = started.elapsed();
    tracing::Span::current().record("elapsed_ms", u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
    if let Some(stats) = &self.stats {
      stats.record(args, elapsed);
    }
  }

  /// Git executable path and version, discovered on first use, once per process (`DISCOVERED`). A failed discovery is
  /// not cached.
  #[instrument(skip(self))]
  pub fn get_info(&self) -> Result<&GitInfo> {
    if let Some(info) = self.info.get() {
      return Ok(info);
    }
    let info = if let Some(info) = DISCOVERED.get() {
      info.clone()
    } else {
      let info = GitInfo::discover().map_err(|e| anyhow!(e))?;
      tracing::info!(git_version = %info.version, git_path = %info.path, "discovered git info");
      // Another thread may have won the race; both discovered the same executable
      DISCOVERED.get_or_init(|| info).clone()
    };
    Ok(self.info.get_or_init(|| info))
  }

  /// The git command with the base config and environment. stdin is null unless a caller pipes it: git never inherits
  /// ours. A spawned child inherits stdin by default, and on Windows git queries its stdin handle at startup
  /// (`winansi_init`), which waits while a read of the same pipe is pending in this process (`bd check` reads a plan
  /// from stdin), so every git started meanwhile would hang
  fn command(&self, args: &[&str], repository_path: &str, env: &[(&str, &str)]) -> Result<Command> {
    if repository_path.is_empty() {
      return Err(anyhow!("repository path cannot be blank"));
    }
    let git_info = self.get_info()?;
    let mut cmd = Command::new(&git_info.path);
    for name in REMOVED_ENV {
      cmd.env_remove(name);
    }
    cmd
      .args(BASE_CONFIG)
      .args(args)
      .current_dir(repository_path)
      .envs(BASE_ENV)
      .envs(self.env.iter().map(|(name, value)| (name.as_str(), value.as_str())))
      .envs(env.iter().copied())
      .stdin(Stdio::null());
    Ok(cmd)
  }

  /// Build the error for a failed command. Logged at error level: callers that expect failures use `status`.
  fn command_error(&self, stderr: &[u8], args: &[&str]) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    let git_command = args.join(" ");
    tracing::error!(stderr = %stderr, git_command = %git_command, "git command failed");
    let git_path = self.info.get().map_or("git", |info| info.path.as_str());
    anyhow!("git command failed: {git_path} {git_command}\nError: {stderr}")
  }

  /// One git invocation, configured by the methods of `GitCommand` and run by one of its finishers:
  /// `git.cmd(&["merge-tree", "--write-tree", a, b], repo).accept(&[1]).output()?`. Every finisher runs on the same
  /// spawn core: the base config and environment of `command`, stdin written from its own thread, both pipes read, the
  /// timeout, the process counted in `with_process_stats` and one tracing span.
  pub fn cmd<'a>(&'a self, args: &'a [&'a str], repository_path: &'a str) -> GitCommand<'a> {
    GitCommand {
      git: self,
      args,
      repository_path,
      env: &[],
      stdin: None,
      accept: &[],
      timeout: None,
      now: &Instant::now,
    }
  }

  /// `cmd(args, repository_path).text()`: trimmed stdout, an error with git's stderr on a non-zero exit code
  pub fn execute_command(&self, args: &[&str], repository_path: &str) -> Result<String> {
    self.cmd(args, repository_path).text()
  }

  /// `cmd(args, repository_path).lines()`: the non-empty lines of stdout, trimmed
  pub fn execute_command_lines(&self, args: &[&str], repository_path: &str) -> Result<Vec<String>> {
    self.cmd(args, repository_path).lines()
  }

  /// `cmd(args, repository_path).status()` as text: trimmed stdout on success, trimmed stderr otherwise, and the exit
  /// code. A non-zero exit code is not an error and is not logged as one, so this is the shorthand for expected failures
  pub fn execute_command_with_status(&self, args: &[&str], repository_path: &str) -> Result<(String, i32)> {
    let output = self.cmd(args, repository_path).status()?;
    Ok(if output.code == 0 { (output.into_text(), 0) } else { (output.stderr, output.code) })
  }
}

/// A git invocation of `GitCommandExecutor::cmd`: options first, then one finisher that runs it
#[must_use = "git runs only in a finisher: text, bytes, lines, output, status, stream, progress or interactive"]
pub struct GitCommand<'a> {
  git: &'a GitCommandExecutor,
  args: &'a [&'a str],
  repository_path: &'a str,
  env: &'a [(&'a str, &'a str)],
  stdin: Option<&'a [u8]>,
  accept: &'a [i32],
  timeout: Option<Duration>,
  /// The clock of the timeout and of `ProgressStop::idle_timeout`: a test moves it on the events of git, so how fast
  /// the machine starts git does not count
  now: &'a dyn Fn() -> Instant,
}

/// What git printed and how it exited (`GitCommand::output`, `GitCommand::status`, `GitCommand::progress`)
#[derive(Debug)]
pub struct GitOutput {
  /// stdout untouched
  pub stdout: Vec<u8>,
  /// stderr as text, trimmed: git's messages
  pub stderr: String,
  /// The exit code; -1 when a signal ended git
  pub code: i32,
}

impl GitOutput {
  /// stdout as text, trimmed
  pub fn into_text(self) -> String {
    trim_text(decode(self.stdout))
  }
}

impl<'a> GitCommand<'a> {
  /// Extra environment variables, applied after the base ones (`GIT_INDEX_FILE` of a temporary index)
  pub const fn env(mut self, env: &'a [(&'a str, &'a str)]) -> Self {
    self.env = env;
    self
  }

  /// `input` on stdin, written from its own thread so that a large input cannot deadlock against a full stdout pipe;
  /// without it stdin is null
  pub const fn stdin(mut self, input: &'a [u8]) -> Self {
    self.stdin = Some(input);
    self
  }

  /// Non-zero exit codes that are still success (`merge-tree` exits with 1 on conflicts)
  pub const fn accept(mut self, codes: &'a [i32]) -> Self {
    self.accept = codes;
    self
  }

  /// Kill git when it has not finished within `timeout`; the error is then a `StoppedCommand::TimedOut`
  pub const fn timeout(mut self, timeout: Duration) -> Self {
    self.timeout = Some(timeout);
    self
  }

  /// The clock of the timeouts (`now`)
  #[cfg(test)]
  pub(crate) fn clock(mut self, now: &'a dyn Fn() -> Instant) -> Self {
    self.now = now;
    self
  }

  /// stdout as text, trimmed; an exit code that is neither 0 nor accepted is an error with git's stderr
  pub fn text(self) -> Result<String> {
    Ok(trim_text(decode(self.bytes()?)))
  }

  /// stdout untouched, for `-z` and size-framed output (`cat-file --batch`); fails as `text`
  pub fn bytes(self) -> Result<Vec<u8>> {
    Ok(self.output()?.stdout)
  }

  /// The lines of stdout, trimmed, without the empty ones; fails as `text`
  pub fn lines(self) -> Result<Vec<String>> {
    Ok(non_empty_lines(&self.bytes()?))
  }

  /// stdout, stderr and the exit code, when the exit code carries the answer (an accepted 1 of `merge-tree`); fails as
  /// `text`
  pub fn output(self) -> Result<GitOutput> {
    let finished = self.run(None, None, &ProgressStop::NEVER)?;
    self.check(&finished)?;
    Ok(finished.into_output())
  }

  /// stdout, stderr and the exit code, whatever the exit code: a failure is not an error and is not logged as one, so
  /// this is the finisher for expected failures
  pub fn status(self) -> Result<GitOutput> {
    let finished = self.run(None, None, &ProgressStop::NEVER)?;
    self.check_input(&finished)?;
    let output = finished.into_output();
    if output.code != 0 {
      tracing::debug!(stderr = %output.stderr, exit_code = output.code, git_command = %self.args.join(" "), "git command failed with status");
    }
    Ok(output)
  }

  /// Pass stdout to `handler` in chunks as they arrive, on the calling thread. `ControlFlow::Break` stops git: it is
  /// killed and the stream ends with `Ok` without a look at the exit code. A handler error kills git too and is the
  /// result. Otherwise the result fails as `text` does
  pub fn stream(self, mut handler: impl FnMut(&[u8]) -> Result<ControlFlow<()>>) -> Result<()> {
    let finished = self.run(Some(&mut handler), None, &ProgressStop::NEVER)?;
    if finished.broken {
      return Ok(());
    }
    self.check(&finished)
  }

  /// For a git that reports progress on stderr (`fetch --progress`): `on_line` gets every stderr line as it arrives,
  /// on the calling thread (`read_progress_lines`), and git is killed when `stop` says so (the error is then a
  /// `StoppedCommand`). Returns stdout, the messages of stderr (its lines that end with `\n`, not the `\r` progress
  /// updates) and the exit code, so an accepted exit code keeps what git said (`push` with a refused ref: the lines of
  /// the remote); fails as `text`, its error holding the same messages
  pub fn progress(self, on_line: &mut dyn FnMut(&str), stop: &ProgressStop<'_>) -> Result<GitOutput> {
    let finished = self.run(None, Some(on_line), stop)?;
    self.check(&finished)?;
    Ok(finished.into_output())
  }

  /// git with piped stdin, for a line protocol that waits for a reply before the next command
  /// (`RefTransaction::commit_while`, the chains of `commit_chain.rs`); its replies end with `delimiter` (`b'\n'`, or
  /// `b'\0'` for `-z` output). `stdin`, `accept` and `timeout` do not apply: the protocol writes stdin, and every reply
  /// has its own timeout (`InteractiveCommand::read_record`)
  pub fn interactive(self, delimiter: u8) -> Result<InteractiveCommand> {
    let spawned = self.spawn(true)?;
    let mut interactive = InteractiveCommand::start(spawned, format!("git {}", self.args.join(" ")), delimiter);
    if let Some(stats) = &self.git.stats {
      interactive.count_into(Arc::clone(stats), self.args);
    }
    Ok(interactive)
  }

  /// Starts git: the one place that spawns a process. stdout and stderr are piped, stdin only for `pipe_stdin` (else
  /// null, `GitCommandExecutor::command`)
  fn spawn(&self, pipe_stdin: bool) -> Result<Spawned> {
    let mut command = self.git.command(self.args, self.repository_path, self.env)?;
    if pipe_stdin {
      command.stdin(Stdio::piped());
    }
    let started = Instant::now();
    let mut child = command
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(|e| anyhow!("Failed to spawn git command: {e}"))?;
    let tree = ProcessTree::of(&child);
    let stdin = if pipe_stdin {
      Some(child.stdin.take().ok_or_else(|| anyhow!("git stdin is not piped"))?)
    } else {
      None
    };
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("git stdout is not piped"))?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow!("git stderr is not piped"))?;
    Ok(Spawned {
      child,
      tree,
      stdin,
      stdout,
      stderr,
      started,
    })
  }

  /// Runs git to its end: stdin written, stdout and stderr read by threads that send their events to this one. stdout
  /// goes to `on_chunk` (`stream`) or is collected, stderr goes line by line to `on_line` (`progress`) or is collected.
  /// A timeout or `stop` makes it poll: git is killed when one says so, and the rest of the pipes is awaited for
  /// `EXIT_GRACE` after git exits, because a helper of git (ssh, a credential helper) may hold them open. The reading
  /// threads are not joined then, so they may outlive the run
  #[instrument(
    name = "git",
    skip_all,
    fields(
      git_command = self.args.join(" "),
      repository_path = self.repository_path,
      input_length = self.stdin.map(<[u8]>::len),
      exit_code = tracing::field::Empty,
      elapsed_ms = tracing::field::Empty,
    )
  )]
  fn run(&self, mut on_chunk: Option<&mut ChunkHandler<'_>>, mut on_line: Option<&mut dyn FnMut(&str)>, stop: &ProgressStop<'_>) -> Result<Finished> {
    let Spawned {
      child,
      tree,
      stdin,
      stdout,
      stderr,
      started,
    } = self.spawn(self.stdin.is_some())?;
    let mut running = Running {
      child,
      tree,
      git: self.git,
      args: self.args,
      started,
    };
    // a git that exits before reading all input closes the pipe: the write error then tells nothing
    let writer = stdin.map(|mut pipe| {
      let input = self.stdin.unwrap_or_default().to_vec();
      std::thread::spawn(move || pipe.write_all(&input))
    });
    let (sender, events) = std::sync::mpsc::sync_channel(EVENT_CAPACITY);
    let chunks = on_chunk.is_some();
    let stdout_sender = sender.clone();
    std::thread::spawn(move || read_stdout(stdout, chunks, &stdout_sender));
    let progress = on_line.is_some();
    std::thread::spawn(move || read_stderr(stderr, progress, &sender));

    let polls = progress || self.timeout.is_some();
    let now = self.now;
    let deadline = self.timeout.map(|timeout| now() + timeout);
    let mut last_output = now();
    let mut stdout = Vec::new();
    let mut stdout_done = false;
    let mut stderr: Option<Vec<u8>> = None;
    let mut exited: Option<(ExitStatus, Instant)> = None;
    let mut broken = false;
    let status = loop {
      if !stdout_done || stderr.is_none() {
        let event = if polls {
          events.recv_timeout(POLL_INTERVAL).map_err(|e| e == RecvTimeoutError::Disconnected)
        } else {
          events.recv().map_err(|_disconnected| true)
        };
        match event {
          Ok(Event::Stdout(chunk)) => match on_chunk.as_mut() {
            Some(handler) => {
              if handler(&chunk)?.is_break() {
                broken = true;
                running.kill();
                break running.wait()?;
              }
            }
            None if stdout.is_empty() => stdout = chunk,
            None => stdout.extend_from_slice(&chunk),
          },
          Ok(Event::StdoutEnd) => stdout_done = true,
          Ok(Event::StdoutFailed(e)) => return Err(anyhow!("Failed to read stdout: {e}")),
          Ok(Event::StderrLine(line)) => {
            if let Some(on_line) = on_line.as_mut() {
              on_line(&line);
            }
            last_output = now();
          }
          Ok(Event::StderrEnd(bytes)) => stderr = Some(bytes),
          // both readers ended (they send their end first, unless one panicked)
          Err(true) => {
            stdout_done = true;
            stderr.get_or_insert_default();
          }
          Err(false) => {}
        }
      } else if polls {
        std::thread::sleep(POLL_INTERVAL);
      }
      let pipes_done = stdout_done && stderr.is_some();
      if !polls {
        if pipes_done {
          break running.wait()?;
        }
        continue;
      }
      if exited.is_none() {
        exited = running
          .child
          .try_wait()
          .map_err(|e| anyhow!("Failed to wait for git command: {e}"))?
          .map(|status| (status, Instant::now()));
      }
      match exited {
        // the rest of the pipes, unless a helper of git keeps them open
        Some((status, at)) if pipes_done || at.elapsed() >= EXIT_GRACE => break status,
        Some(_) => {}
        None => {
          let stopped = if (stop.is_cancelled)() {
            Some(StoppedCommand::Cancelled)
          } else if deadline.is_some_and(|deadline| now() >= deadline) {
            self.timeout.map(StoppedCommand::TimedOut)
          } else {
            stop
              .idle_timeout
              .filter(|timeout| now().saturating_duration_since(last_output) >= *timeout)
              .map(StoppedCommand::Stalled)
          };
          if let Some(stopped) = stopped {
            running.kill();
            tracing::warn!(git_command = %self.args.join(" "), reason = %stopped, "git command stopped");
            return Err(anyhow::Error::new(stopped));
          }
        }
      }
    };
    tracing::Span::current().record("exit_code", status.code().unwrap_or(-1));

    // a polled run does not wait for a writer that a helper of git may block, nor does a stopped stream
    let write_error = match writer {
      Some(writer) if !polls && !broken => match writer.join() {
        Ok(result) => result.err(),
        Err(_) => return Err(anyhow!("stdin writer thread panicked")),
      },
      _ => None,
    };
    Ok(Finished {
      stdout,
      stderr: stderr.unwrap_or_default(),
      status,
      broken,
      write_error,
    })
  }

  /// Fails on an exit code that is neither 0 nor accepted, and as `check_input`
  fn check(&self, finished: &Finished) -> Result<()> {
    self.check_input(finished)?;
    let accepted = finished.status.success() || finished.status.code().is_some_and(|code| self.accept.contains(&code));
    if accepted { Ok(()) } else { Err(self.git.command_error(&finished.stderr, self.args)) }
  }

  /// git may legitimately exit before reading all input; only a successful run with a failed write is an error
  fn check_input(&self, finished: &Finished) -> Result<()> {
    match &finished.write_error {
      Some(e) if finished.status.success() => Err(anyhow!("Failed to write to stdin: {e}")),
      Some(e) => {
        tracing::debug!(error = %e, git_command = %self.args.join(" "), "git exited before consuming stdin");
        Ok(())
      }
      None => Ok(()),
    }
  }
}

/// The handler of `GitCommand::stream`
type ChunkHandler<'h> = dyn FnMut(&[u8]) -> Result<ControlFlow<()>> + 'h;

/// A started git with its pipes taken (`GitCommand::spawn`)
pub(crate) struct Spawned {
  pub(crate) child: Child,
  /// What a kill of git ends: git and the processes it started (`process_tree.rs`)
  pub(crate) tree: ProcessTree,
  pub(crate) stdin: Option<ChildStdin>,
  pub(crate) stdout: ChildStdout,
  pub(crate) stderr: ChildStderr,
  pub(crate) started: Instant,
}

/// The git of `GitCommand::run`; however the run ends, dropping it kills a git that still runs, waits for it and
/// counts the process (`with_process_stats`)
struct Running<'a> {
  child: Child,
  tree: ProcessTree,
  git: &'a GitCommandExecutor,
  args: &'a [&'a str],
  started: Instant,
}

impl Running<'_> {
  fn kill(&mut self) {
    if matches!(self.child.try_wait(), Ok(None)) {
      self.tree.kill(&mut self.child);
    }
  }

  fn wait(&mut self) -> Result<ExitStatus> {
    self.child.wait().map_err(|e| anyhow!("Failed to wait for git command: {e}"))
  }
}

impl Drop for Running<'_> {
  fn drop(&mut self) {
    self.kill();
    let _ = self.child.wait();
    self.git.record_process(self.args, self.started);
  }
}

/// How `GitCommand::run` ended
struct Finished {
  stdout: Vec<u8>,
  /// All of stderr, or the messages of `read_progress_lines` for `progress`
  stderr: Vec<u8>,
  status: ExitStatus,
  /// The handler of `stream` stopped git
  broken: bool,
  /// Why stdin could not be written to its end
  write_error: Option<std::io::Error>,
}

impl Finished {
  fn into_output(self) -> GitOutput {
    GitOutput {
      code: self.status.code().unwrap_or(-1),
      stdout: self.stdout,
      stderr: trim_text(decode(self.stderr)),
    }
  }
}

/// What the reading threads of `GitCommand::run` send
enum Event {
  /// A chunk of stdout for `stream`, else all of it
  Stdout(Vec<u8>),
  StdoutEnd,
  StdoutFailed(std::io::Error),
  /// A line of stderr for `progress`
  StderrLine(String),
  /// The end of stderr: all of it, or the messages of `read_progress_lines`
  StderrEnd(Vec<u8>),
}

/// Events that wait for `GitCommand::run`, so that a slow `stream` handler holds git back instead of the memory
/// filling with chunks
const EVENT_CAPACITY: usize = 16;

/// The most `stream` reads at once
const STREAM_CHUNK: usize = 64 * 1024;

fn read_stdout(mut stdout: ChildStdout, chunks: bool, sender: &SyncSender<Event>) {
  if chunks {
    let mut buffer = vec![0; STREAM_CHUNK];
    loop {
      match stdout.read(&mut buffer) {
        Ok(0) => break,
        Ok(n) => {
          if sender.send(Event::Stdout(buffer[..n].to_vec())).is_err() {
            return;
          }
        }
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
        Err(e) => {
          let _ = sender.send(Event::StdoutFailed(e));
          return;
        }
      }
    }
  } else {
    let mut all = Vec::new();
    if let Err(e) = stdout.read_to_end(&mut all) {
      let _ = sender.send(Event::StdoutFailed(e));
      return;
    }
    let _ = sender.send(Event::Stdout(all));
  }
  let _ = sender.send(Event::StdoutEnd);
}

fn read_stderr(mut stderr: ChildStderr, progress: bool, sender: &SyncSender<Event>) {
  let end = if progress {
    read_progress_lines(stderr, &mut |line| {
      let _ = sender.send(Event::StderrLine(line.to_string()));
    })
    .into_bytes()
  } else {
    let mut all = Vec::new();
    // a read error ends stderr early: the exit code tells the failure
    let _ = stderr.read_to_end(&mut all);
    all
  };
  let _ = sender.send(Event::StderrEnd(end));
}

/// `bytes` as text, copied only when they are not UTF-8 (lossily then)
pub(crate) fn decode(bytes: Vec<u8>) -> String {
  String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// The lines of `output`, trimmed, without the empty ones
fn non_empty_lines(output: &[u8]) -> Vec<String> {
  output
    .split(|&b| b == b'\n')
    .filter_map(|line| {
      let line = String::from_utf8_lossy(line);
      let trimmed = line.trim();
      if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
    })
    .collect()
}

/// `text` without its leading and trailing whitespace, in place
fn trim_text(mut text: String) -> String {
  text.truncate(text.trim_end().len());
  let leading = text.len() - text.trim_start().len();
  text.replace_range(..leading, "");
  text
}

/// When `GitCommand::progress` kills git
pub struct ProgressStop<'a> {
  /// Kill git after this long without a stderr line; None: never
  pub idle_timeout: Option<Duration>,
  /// Polled while git runs; true kills it
  pub is_cancelled: &'a dyn Fn() -> bool,
}

impl ProgressStop<'_> {
  pub const NEVER: ProgressStop<'static> = ProgressStop {
    idle_timeout: None,
    is_cancelled: &|| false,
  };
}

/// Why `GitCommand::progress` or `GitCommand::timeout` killed git
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoppedCommand {
  /// Nothing on stderr for this long
  Stalled(Duration),
  /// Not finished within this long (`GitCommand::timeout`)
  TimedOut(Duration),
  Cancelled,
}

impl std::fmt::Display for StoppedCommand {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Stalled(timeout) => write!(f, "git reported nothing for {} s", timeout.as_secs()),
      Self::TimedOut(timeout) => write!(f, "git did not finish within {} s", timeout.as_secs()),
      Self::Cancelled => f.write_str("canceled"),
    }
  }
}

impl std::error::Error for StoppedCommand {}

/// How often a run with a timeout or a `ProgressStop` checks whether git exited or must be stopped
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// How long the rest of stdout and stderr may take after git exited
const EXIT_GRACE: Duration = Duration::from_millis(500);

/// Read git's stderr and pass each line to `on_line` as it arrives, trimmed (the sideband pads `remote:` lines with
/// spaces); empty lines are skipped. A line ends at `\n` or at `\r`, with which git overwrites a progress line in
/// place. Returns the messages: the lines that ended with `\n` (a `\r\n` line counts), without the `\r` updates.
pub(crate) fn read_progress_lines(mut reader: impl Read, on_line: &mut dyn FnMut(&str)) -> String {
  let mut messages = String::new();
  let mut pending = Vec::new();
  // a line that ended with `\r`: an update, unless an empty line ending with `\n` follows (`\r\n`)
  let mut overwritten: Option<String> = None;
  let mut buffer = [0u8; 8192];
  loop {
    let n = match reader.read(&mut buffer) {
      Ok(0) => break,
      Ok(n) => n,
      Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
      Err(_) => break,
    };
    for &byte in &buffer[..n] {
      if byte != b'\r' && byte != b'\n' {
        pending.push(byte);
        continue;
      }
      let line = String::from_utf8_lossy(&pending).trim().to_string();
      pending.clear();
      if byte == b'\n' && line.is_empty() {
        if let Some(message) = overwritten.take() {
          messages.push_str(&message);
          messages.push('\n');
        }
        continue;
      }
      overwritten = None;
      if line.is_empty() {
        continue;
      }
      on_line(&line);
      if byte == b'\n' {
        messages.push_str(&line);
        messages.push('\n');
      } else {
        overwritten = Some(line);
      }
    }
  }
  let line = String::from_utf8_lossy(&pending).trim().to_string();
  if !line.is_empty() {
    on_line(&line);
    messages.push_str(&line);
  }
  messages
}

#[cfg(test)]
#[path = "git_command_executor_test.rs"]
mod git_command_executor_test;
