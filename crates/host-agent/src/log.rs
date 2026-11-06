//! The lines of the log: the stderr of `serve`, which launchd writes to `~/Library/Logs/brd-host.log`, and the stderr of a
//! supervisor, which `serve` points at `supervisor.log` of its run

use std::io::Write as _;

/// A line of the log of `serve`
pub(crate) fn log(message: &str) {
  let _ = writeln!(std::io::stderr(), "brd-host: {message}");
}

/// A line of the log of a supervisor
pub(crate) fn supervisor(message: &str) {
  let _ = writeln!(std::io::stderr(), "brd-host supervise: {message}");
}
