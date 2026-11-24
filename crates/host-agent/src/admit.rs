//! The start of a run on a member of a set of hosts that takes runs only at once (`RunSpec::admit_now`, `idle_only` of
//! the set; openspec/specs/cli/host/spec.md, "serve: A start that admits at once or refuses with busy";
//! docs/design/host-runs.md, "Sets of hosts").
//!
//! The admission decides, not the estimate, so two clients never meet on one free host: the supervisor of such a run
//! takes the heavy turn in one look under the lock of the table (`turn::take` with no wait), as the admission of every
//! heavy run does, or a run that is not heavy passes no measurement. Admitted, it writes `admitted` in the run
//! directory and goes on as any run; else its run ends with `busy` before its lend, its ticket out of the queue at once.
//! The start request waits for that decision ([`wait_for_decision`]) and answers `busy` itself, so the client goes to
//! the next member before it sends a pack

use crate::files::write_private;
use crate::run::{ExitRecord, RunDir};
use host_link::refusal::{Code, HOST, Refusal};
use std::io;
use std::thread;
use std::time::{Duration, Instant};

/// The file of an admitted run of `RunSpec::admit_now`
const ADMITTED: &str = "admitted";

/// How long a start of `RunSpec::admit_now` waits for its supervisor to decide: the start of the process and one look
/// at the turn, whose lock waits 2 s. A supervisor that has not decided by then decides later, and the follow of the
/// client sees its end
pub(crate) const DECIDE_WAIT: Duration = Duration::from_secs(30);

/// How often the start looks at the run directory for the decision
const DECIDE_POLL: Duration = Duration::from_millis(20);

/// Notes in the run directory that the run of `RunSpec::admit_now` is admitted
pub(crate) fn mark_admitted(run: &RunDir) -> io::Result<()> {
  write_private(&run.path().join(ADMITTED), b"")
}

/// The refusal of a run of `RunSpec::admit_now` that the host has no room for now, by `why`
pub(crate) fn busy(why: &str) -> Refusal {
  Refusal::new(
    Code::Busy,
    format!("{HOST} takes runs only at once (`idle_only` of the set), and {why}: nothing ran there, and the client goes to the next member"),
  )
}

/// Waits up to `wait` for the supervisor of the run of `RunSpec::admit_now` to decide: `Ok` once it is admitted, once it
/// ended another way (its follow tells how), or once the time is up; the refusal `busy` of a run that it refused
pub(crate) fn wait_for_decision(run: &RunDir, wait: Duration) -> Result<(), Refusal> {
  let started = Instant::now();
  while started.elapsed() < wait {
    if run.path().join(ADMITTED).exists() {
      return Ok(());
    }
    match run.exit() {
      Ok(Some(ExitRecord::Refused { refused })) if refused.code == Code::Busy.name() => return Err(Refusal::new(Code::Busy, refused.message)),
      Ok(Some(_)) => return Ok(()),
      _ => thread::sleep(DECIDE_POLL),
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "admit_test.rs"]
mod admit_test;
