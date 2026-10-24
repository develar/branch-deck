//! When a session syncs (docs/design/rust-domain-authority.md, "Поток sync"): one run at a time, the requests made
//! while one runs served by one quiet follow-up after it, a focus while one runs checked once after the chain, and one
//! silent retry of a run whose report asks for it (`SyncReport::retry`). The scheduler only decides; the actor of the
//! session (`runtime`) reads the fingerprint of the sync inputs, runs the sync and tells the scheduler what came back.
//!
//! A request carries its waiter (`W`), which the scheduler hands back with the outcome when the run that serves it
//! ends, never the chain after it: an update waits for its own sync, not for the follow-up a ⌘R during it asked for.
//! - `Manual` and `AfterRewrite` always run: at once, or as the follow-up of the run in flight.
//! - `Focus` and `Open` run only when what a sync reads changed since the last successful sync (`sync_inputs_fingerprint`,
//!   one cheap read); a session that never synced runs at once. A focus while a run is in flight checks once after the
//!   chain: a commit in the IDE during a long sync is not lost, and a sync that saw it costs no second one. A window that
//!   opens while a run is in flight is served by that run: its end is the table the window shows.
//! - A focus of a session without subscribers is skipped: no window shows it, so nothing runs.

use domain_types::error::ApiError;
use domain_types::session::{SyncOutcome, SyncReason};

/// The number of a sync run within a session; the events and the end of an older run are dropped by it
pub(crate) type RunId = u64;

/// What a request or a run ended with
pub(crate) type Settled = Result<SyncOutcome, ApiError>;

/// What the actor does next
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
  Nothing,
  /// Read the fingerprint of the sync inputs, then `inputs_read`
  CheckInputs,
  /// Start the run `run`; `inputs` is the fingerprint read just before, `None` makes the run read it first. `retry`: the
  /// silent repeat of a run whose report asked for it, which keeps the rows of that run `Syncing`
  Run {
    run: RunId,
    inputs: Option<String>,
    retry: bool,
  },
}

/// The waiters to resolve now, after the patch of what ended, and the next step
#[derive(Debug)]
pub(crate) struct Decision<W> {
  pub(crate) resolve: Vec<(W, Settled)>,
  pub(crate) next: Step,
}

impl<W> Decision<W> {
  const fn next(next: Step) -> Self {
    Self { resolve: Vec::new(), next }
  }
}

/// What a run ended with, as the scheduler reads it
#[derive(Debug)]
pub(crate) enum RunEnd {
  /// The sync succeeded; `retry`: its report asks for one more run (`SyncReport::retry`)
  Succeeded {
    retry: bool,
  },
  Failed(ApiError),
}

#[derive(Debug, PartialEq, Eq)]
enum Phase {
  Idle,
  /// A fingerprint is being read for `Focus` or `Open`
  Checking,
  /// A run is in flight; `retried`: it is the silent retry already
  Running {
    run: RunId,
    retried: bool,
  },
}

/// The sync schedule of one session
#[derive(Debug)]
pub(crate) struct SyncScheduler<W> {
  phase: Phase,
  /// The requests the check or the run in flight serves
  serving: Vec<W>,
  /// The requests a run made while one was in flight: one quiet follow-up serves them all
  follow_up: Vec<W>,
  /// The focuses while a run was in flight: one check after the chain serves them all
  recheck: Vec<W>,
  /// The fingerprint of the inputs the last successful sync read; `None` before one, or when git could not tell
  synced_inputs: Option<String>,
  last_run: RunId,
}

impl<W> SyncScheduler<W> {
  pub(crate) const fn new() -> Self {
    Self {
      phase: Phase::Idle,
      serving: Vec::new(),
      follow_up: Vec::new(),
      recheck: Vec::new(),
      synced_inputs: None,
      last_run: 0,
    }
  }

  /// The run in flight
  pub(crate) const fn running(&self) -> Option<RunId> {
    match self.phase {
      Phase::Running { run, .. } => Some(run),
      _ => None,
    }
  }

  /// Whether nothing is in flight and nothing waits
  #[cfg(test)]
  pub(crate) const fn is_idle(&self) -> bool {
    matches!(self.phase, Phase::Idle)
  }

  /// A request; `watched`: a window subscribes to the session
  pub(crate) fn request(&mut self, reason: SyncReason, waiter: W, watched: bool) -> Decision<W> {
    let checks_inputs = matches!(reason, SyncReason::Focus | SyncReason::Open);
    if reason == SyncReason::Focus && !watched {
      return Decision {
        resolve: vec![(waiter, Ok(SyncOutcome::Skipped))],
        next: Step::Nothing,
      };
    }
    match self.phase {
      Phase::Idle => {
        self.serving.push(waiter);
        if checks_inputs && self.synced_inputs.is_some() {
          self.phase = Phase::Checking;
          Decision::next(Step::CheckInputs)
        } else {
          Decision::next(self.start(None, false))
        }
      }
      // a focus or an open joins the check; a run waits for its answer
      Phase::Checking if checks_inputs => {
        self.serving.push(waiter);
        Decision::next(Step::Nothing)
      }
      Phase::Running { .. } if reason == SyncReason::Open => {
        self.serving.push(waiter);
        Decision::next(Step::Nothing)
      }
      Phase::Running { .. } if reason == SyncReason::Focus => {
        self.recheck.push(waiter);
        Decision::next(Step::Nothing)
      }
      Phase::Checking | Phase::Running { .. } => {
        self.follow_up.push(waiter);
        Decision::next(Step::Nothing)
      }
    }
  }

  /// The fingerprint of the check came: the same as the last successful sync read ends the check; any other, or none
  /// (git could not tell), runs, and that run serves the requests that waited for the check too
  pub(crate) fn inputs_read(&mut self, inputs: Option<String>, watched: bool) -> Decision<W> {
    if self.phase != Phase::Checking {
      return Decision::next(Step::Nothing);
    }
    if inputs.is_some() && inputs == self.synced_inputs {
      let mut resolve = self.serving.drain(..).map(|waiter| (waiter, Ok(SyncOutcome::Unchanged))).collect();
      self.phase = Phase::Idle;
      let next = self.after_chain(watched, &mut resolve);
      return Decision { resolve, next };
    }
    let follow_up = std::mem::take(&mut self.follow_up);
    self.serving.extend(follow_up);
    Decision::next(self.start(inputs, false))
  }

  /// The run `run` ended, having read `inputs` before it; the end of another run than the one in flight is dropped
  pub(crate) fn run_finished(&mut self, run: RunId, inputs: Option<String>, end: RunEnd, watched: bool) -> Decision<W> {
    let Phase::Running { run: current, retried } = self.phase else {
      return Decision::next(Step::Nothing);
    };
    if run != current {
      return Decision::next(Step::Nothing);
    }
    let settled = match end {
      // one silent retry, for the same requests: its tip was not this run's build
      RunEnd::Succeeded { retry: true } if !retried => return Decision::next(self.start(None, true)),
      RunEnd::Succeeded { .. } => {
        self.synced_inputs = inputs;
        Ok(SyncOutcome::Synced)
      }
      RunEnd::Failed(error) => Err(error),
    };
    let mut resolve = self.serving.drain(..).map(|waiter| (waiter, settled.clone())).collect();
    self.phase = Phase::Idle;
    let next = self.after_chain(watched, &mut resolve);
    Decision { resolve, next }
  }

  /// What follows the end of a check or a run: the quiet follow-up of the requests made meanwhile, else the check of the
  /// focuses made meanwhile (skipped into `resolve` when no window shows the session any more), else nothing
  fn after_chain(&mut self, watched: bool, resolve: &mut Vec<(W, Settled)>) -> Step {
    if !self.follow_up.is_empty() {
      self.serving = std::mem::take(&mut self.follow_up);
      return self.start(None, false);
    }
    let recheck = std::mem::take(&mut self.recheck);
    if recheck.is_empty() {
      return Step::Nothing;
    }
    if !watched {
      resolve.extend(recheck.into_iter().map(|waiter| (waiter, Ok(SyncOutcome::Skipped))));
      return Step::Nothing;
    }
    self.serving = recheck;
    if self.synced_inputs.is_some() {
      self.phase = Phase::Checking;
      Step::CheckInputs
    } else {
      self.start(None, false)
    }
  }

  const fn start(&mut self, inputs: Option<String>, retry: bool) -> Step {
    self.last_run += 1;
    let run = self.last_run;
    self.phase = Phase::Running { run, retried: retry };
    Step::Run { run, inputs, retry }
  }
}

#[cfg(test)]
#[path = "sync_scheduler_test.rs"]
mod sync_scheduler_test;
