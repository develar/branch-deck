//! The lines of the log: the stderr of `serve`, which launchd writes to `~/Library/Logs/brd-host.log`, and the stderr of a
//! supervisor, which `serve` points at `supervisor.log` of its run. `serve` gives a supervisor its own stderr as the
//! stdout, so the lines of a supervisor after the end of its run reach the log of the agent too: a client forgets the
//! run once it ended and never reads `supervisor.log`, and the run goes from the host a day later. So does a panic
//! after the end
//! (`panics_reach_the_agent_log`). A `serve` by hand whose stderr is a pipe that nothing reads holds such a line at a
//! full pipe, as it holds its own lines; under launchd the log is a file.
//!
//! Each line starts with the local time of the host to the millisecond (`2026-11-19 07:10:02.123`), so a stall of a
//! run can be set against the other lines and the system log; each line is one write, so the lines of several processes
//! in one file never mix

use std::io::Write;
use std::panic;

/// A line of the log of `serve`
pub(crate) fn log(message: &str) {
  write_line(&mut std::io::stderr(), &format!("brd-host: {message}"));
}

/// A line of the log of a supervisor
pub(crate) fn supervisor(message: &str) {
  write_line(&mut std::io::stderr(), &format!("brd-host supervise: {message}"));
}

/// A line of the supervisor of `run` after the end of the run: into `supervisor.log`, and with the id of the run into
/// the log of the agent, where it stays after the client forgot the run
pub(crate) fn supervisor_after_end(run: &str, message: &str) {
  supervisor(message);
  agent_log_line(&mut std::io::stdout(), run, message);
}

/// The line of the supervisor of `run` in the log of the agent, `out`
fn agent_log_line(out: &mut impl Write, run: &str, message: &str) {
  write_line(out, &format!("brd-host supervise {run}: {message}"));
}

/// `text` as one line of `out`, after the local time of now, in one write
fn write_line(out: &mut impl Write, text: &str) {
  let _ = out.write_all(stamped(&jiff::Zoned::now(), text).as_bytes());
}

/// The line of `text` at `now`: `2026-11-19 07:10:02.123 <text>` and its line end
fn stamped(now: &jiff::Zoned, text: &str) -> String {
  format!("{} {text}\n", now.strftime("%Y-%m-%d %H:%M:%S%.3f"))
}

/// From now on a panic of this supervisor goes to the log of the agent too, as the line of `supervisor_after_end` with
/// the id of `run`, after the hook it had wrote it to `supervisor.log` with its backtrace: the give-back after
/// `exit.json` runs the git of the pool, and a client may have forgotten the run already
pub(crate) fn panics_reach_the_agent_log(run: &str) {
  panics_reach(run, std::io::stdout);
}

/// `panics_reach_the_agent_log` with the log of the agent that `agent_log` opens
fn panics_reach<W: Write + 'static>(run: &str, agent_log: fn() -> W) {
  let previous = panic::take_hook();
  let run = run.to_owned();
  panic::set_hook(Box::new(move |info| {
    previous(info);
    agent_log_line(&mut agent_log(), &run, &format!("panicked: {}", info.to_string().replace('\n', " ")));
  }));
}

#[cfg(test)]
#[path = "log_test.rs"]
mod log_test;
