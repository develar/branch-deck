//! The lines of the log: the stderr of `serve`, which launchd writes to `~/Library/Logs/brd-host.log`, and the stderr of a
//! supervisor, which `serve` points at `supervisor.log` of its run. `serve` gives a supervisor its own stderr as the
//! stdout, so the lines of a supervisor after the end of its run reach the log of the agent too: a client forgets the
//! run once it ended, and its directory goes with `supervisor.log`

use std::io::Write as _;

/// A line of the log of `serve`
pub(crate) fn log(message: &str) {
  let _ = writeln!(std::io::stderr(), "brd-host: {message}");
}

/// A line of the log of a supervisor
pub(crate) fn supervisor(message: &str) {
  let _ = writeln!(std::io::stderr(), "brd-host supervise: {message}");
}

/// A line of the supervisor of `run` after the end of the run: into `supervisor.log`, and with the id of the run into
/// the log of the agent, where it stays after the client forgot the run
pub(crate) fn supervisor_after_end(run: &str, message: &str) {
  supervisor(message);
  let _ = writeln!(std::io::stdout(), "brd-host supervise {run}: {message}");
}
