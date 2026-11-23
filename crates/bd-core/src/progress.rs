//! The steps of a long command on stderr, each as it starts with the time since the command began, and in its answer
//! (`steps`, each with `at_ms`): `brd pick` and `brd lease pick` (openspec/specs/cli/pick/spec.md, "Steps"), whose minutes an
//! agent sees only through these lines. A write to stderr is best-effort: an agent pipes it into `head -c`, and a closed
//! pipe must never stop a command halfway

use schemars::JsonSchema;
use serde::Serialize;
use std::fmt::Display;
use std::io::Write as _;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// A step of a command in its answer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct StepOutput {
  /// The step, as stderr printed it after `<command>: [<at>] `
  pub step: String,
  /// When it started, in milliseconds since the command began
  pub at_ms: u64,
}

/// The steps of one command: told to stderr as each starts, kept for the answer
pub struct CommandSteps {
  /// `brd pick`, the prefix of each line
  command: &'static str,
  started: Instant,
  steps: Mutex<Vec<StepOutput>>,
}

impl CommandSteps {
  /// The steps of `command`, on stderr
  pub fn new(command: &'static str) -> Self {
    Self {
      command,
      started: Instant::now(),
      steps: Mutex::default(),
    }
  }

  /// Tells `step` as it starts: its line to stderr (a failed write ignored), and keeps it for the answer
  pub fn tell(&self, step: &dyn Display) {
    let at = self.started.elapsed();
    let step = step.to_string();
    let _ = writeln!(std::io::stderr().lock(), "{}", progress_line(self.command, at, &step));
    lock(&self.steps).push(StepOutput {
      step,
      at_ms: u64::try_from(at.as_millis()).unwrap_or(u64::MAX),
    });
  }

  /// The steps told so far, in their order
  pub fn take(&self) -> Vec<StepOutput> {
    std::mem::take(&mut *lock(&self.steps))
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
