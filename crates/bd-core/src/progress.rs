//! The steps of a long command on stderr, each as it starts with the time since the command began, and in its answer
//! (`steps`, each with `at_ms`): `brd pick` and `brd lease pick` (openspec/specs/cli/pick/spec.md, "Steps"), whose minutes an
//! agent sees only through these lines. Their wait for the repository lock goes on while its holder lives, up to
//! `--lock-wait` (`CommandSteps` is its `LockWaiter`), and names the holder at once and every 10 s. A write to stderr is
//! best-effort: an agent pipes it into `head -c`, and a closed pipe must never stop a command halfway

use crate::error::CliError;
use domain_types::error::ErrorCode;
use history_rewrite::repository_lock::{LockHolder, LockWaiter};
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt::Display;
use std::io::Write as _;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// How long `brd pick` and `brd lease pick` wait for the repository lock in all without `--lock-wait`: below the 600 s
/// limit of a Bash call of an agent, so the answer arrives before the call ends
pub const DEFAULT_LOCK_WAIT: Duration = Duration::from_secs(540);

/// The bound of `--lock-wait <secs>`, `DEFAULT_LOCK_WAIT` without it
#[must_use]
pub fn lock_wait_of(secs: Option<u64>) -> Duration {
  secs.map_or(DEFAULT_LOCK_WAIT, Duration::from_secs)
}

/// How often a wait for a live holder names it again on stderr
const HOLDER_TICK: Duration = Duration::from_secs(10);

/// A step of a command in its answer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct StepOutput {
  /// The step, as stderr printed it after `<command>: [<at>] `
  pub step: String,
  /// When it started, in milliseconds since the command began
  pub at_ms: u64,
}

/// The steps of one command and its waits for the repository lock (`LockWaiter`): told to stderr as each starts (or to
/// the sink of a test), kept for the answer
pub struct CommandSteps<'a> {
  /// `brd pick`, the prefix of each line
  command: &'static str,
  started: Instant,
  steps: Mutex<Vec<StepOutput>>,
  /// Where a line goes instead of stderr: a test acts on a step
  sink: Option<&'a (dyn Fn(&str) + Sync)>,
  /// How long the command waits for the repository lock in all, while its holder lives
  lock_wait: Duration,
  /// How long a wait goes on while no record names a live holder (`LockWaiter::unknown_timeout`)
  unknown_timeout: Duration,
  wait: Mutex<Wait>,
}

/// The waits of a command for the repository lock
#[derive(Default)]
struct Wait {
  /// The time of the waits that ended
  waited: Duration,
  /// The wait that goes on: when it started, and the holder named last with when
  current: Option<(Instant, Option<(u32, Instant)>)>,
  /// A wait ran past `lock_wait`
  ran_out: bool,
}

impl<'a> CommandSteps<'a> {
  /// The steps of `command`, on stderr; its waits for the repository lock end after `lock_wait` in all, a wait whose
  /// holder writes no record after `unknown_timeout`
  pub fn new(command: &'static str, lock_wait: Duration, unknown_timeout: Duration) -> Self {
    Self {
      command,
      started: Instant::now(),
      steps: Mutex::default(),
      sink: None,
      lock_wait,
      unknown_timeout,
      wait: Mutex::default(),
    }
  }

  /// `new`, each line told to `sink` instead of stderr
  #[must_use]
  pub fn with_sink(mut self, sink: &'a (dyn Fn(&str) + Sync)) -> Self {
    self.sink = Some(sink);
    self
  }

  /// Tells `step` as it starts: its line to stderr (a failed write ignored), and keeps it for the answer
  pub fn tell(&self, step: &dyn Display) {
    let at = self.started.elapsed();
    let step = step.to_string();
    let line = progress_line(self.command, at, &step);
    match self.sink {
      Some(sink) => sink(&line),
      None => {
        let _ = writeln!(std::io::stderr().lock(), "{line}");
      }
    }
    lock(&self.steps).push(StepOutput {
      step,
      at_ms: u64::try_from(at.as_millis()).unwrap_or(u64::MAX),
    });
  }

  /// The steps told so far, in their order
  pub fn take(&self) -> Vec<StepOutput> {
    std::mem::take(&mut *lock(&self.steps))
  }

  /// `error` as the command answers it: a wait that ran past `--lock-wait` ended with `LockWaitCanceled`, which is
  /// `locked` (retryable), named with the bound; any other error as it is
  #[must_use]
  pub fn bounded(&self, error: CliError) -> CliError {
    if !lock(&self.wait).ran_out || error.code != ErrorCode::Locked {
      return error;
    }
    CliError::new(
      ErrorCode::Locked,
      format!(
        "{}: another process held the repository lock for the whole wait of {} (--lock-wait); nothing changed. The steps on stderr name it; run the \
         same command again once it is done",
        self.command,
        elapsed_text(self.lock_wait),
      ),
    )
  }
}

impl LockWaiter for CommandSteps<'_> {
  /// The holder at once and every 10 s, or when another one takes its place; the end of a wait adds its time
  fn holder(&self, holder: Option<&LockHolder>) {
    let mut wait = lock(&self.wait);
    let Some(holder) = holder else {
      if let Some((started, _)) = wait.current.take() {
        wait.waited += started.elapsed();
      }
      return;
    };
    let (_, told) = wait.current.get_or_insert_with(|| (Instant::now(), None));
    if told.is_some_and(|(pid, at)| pid == holder.pid && at.elapsed() < HOLDER_TICK) {
      return;
    }
    *told = Some((holder.pid, Instant::now()));
    drop(wait);
    self.tell(&format_args!("the repository lock is held by {holder}"));
  }

  /// Past `--lock-wait` in all, once the holder was named: a holder the waiter never heard of ends the wait after
  /// `unknown_timeout` instead
  fn canceled(&self) -> bool {
    let mut wait = lock(&self.wait);
    let Some((started, Some(_))) = wait.current else {
      return false;
    };
    let ran_out = wait.waited + started.elapsed() >= self.lock_wait;
    wait.ran_out |= ran_out;
    ran_out
  }

  fn unknown_timeout(&self) -> Duration {
    self.unknown_timeout
  }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The stderr line of a step that started `at` after `command` began: `brd pick: [1 min 5 s] checking …`
#[must_use]
pub fn progress_line(command: &str, at: Duration, step: &str) -> String {
  format!("{command}: [{}] {step}", elapsed_text(at))
}

/// `42 s`, `3 min 12 s`: whole seconds, as the holder of a lock names its time
#[must_use]
pub fn elapsed_text(elapsed: Duration) -> String {
  let seconds = elapsed.as_secs();
  if seconds >= 60 {
    format!("{} min {} s", seconds / 60, seconds % 60)
  } else {
    format!("{seconds} s")
  }
}
