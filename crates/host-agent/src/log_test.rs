use super::{log_to, panics_reach, stamped, supervisor_after_end_to};
use pretty_assertions::assert_eq;
use std::io::{self, Write};
use std::sync::{Mutex, PoisonError};

/// What the hooks of the test got, in their order: the hook before, then the log of the agent
static WRITTEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn note(text: String) {
  WRITTEN.lock().unwrap_or_else(PoisonError::into_inner).push(text);
}

/// The log of the agent of the test
struct AgentLog;

impl Write for AgentLog {
  /// A line comes in pieces, as `writeln!` writes them: they join until its line end
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    let mut written = WRITTEN.lock().unwrap_or_else(PoisonError::into_inner);
    let piece = String::from_utf8_lossy(bytes);
    match written.last_mut() {
      Some(line) if line.starts_with("agent log: ") && !line.ends_with('\n') => line.push_str(&piece),
      _ => written.push(format!("agent log: {piece}")),
    }
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

/// A panic after the end of a run reaches the log of the agent as one line with the id of the run, its place and its
/// message, after the hook it had, which writes it to `supervisor.log`
#[test]
fn a_panic_after_the_end_of_a_run_reaches_the_log_of_the_agent_with_the_id_of_the_run() {
  std::panic::set_hook(Box::new(|_| note("the hook before".to_owned())));
  panics_reach("1-1", || AgentLog);
  let caught = std::panic::catch_unwind(|| panic!("the give-back broke"));
  drop(std::panic::take_hook());
  assert!(caught.is_err());
  let written = WRITTEN.lock().unwrap_or_else(PoisonError::into_inner).clone();
  assert_eq!(written.len(), 2, "{written:?}");
  assert_eq!(written[0], "the hook before");
  let line = written[1].strip_prefix("agent log: ").unwrap();
  let (stamp, line) = line.split_once(" brd-host").unwrap();
  assert!(jiff::civil::DateTime::strptime("%Y-%m-%d %H:%M:%S%.f", stamp).is_ok(), "{stamp:?}");
  assert!(
    line.starts_with(" supervise 1-1: panicked: panicked at crates/host-agent/src/log_test.rs:") && line.ends_with(" the give-back broke\n"),
    "{line:?}"
  );
}

/// A line of the log starts with the local time of the host to the millisecond, so a stall of a run can be set against
/// the other lines and the system log
#[test]
fn a_line_of_the_log_starts_with_the_local_time_to_the_millisecond() {
  let now = jiff::civil::date(2026, 11, 19)
    .at(7, 10, 2, 123_456_789)
    .to_zoned_datetime(jiff::tz::TimeZone::UTC)
    .unwrap();
  assert_eq!(stamped(&now, "brd-host: started the run 1-1"), "2026-11-19 07:10:02.123 brd-host: started the run 1-1\n");
}

/// A note of the files of the heavy queue reaches the log of the agent: a line of `serve` (an estimate) and a line of a
/// supervisor, into `supervisor.log` and with the id of the run into the log of the agent
#[test]
fn a_note_of_the_heavy_queue_reaches_the_log_of_the_agent() {
  let note = "/h/.cache/brd-host/heavy/groups.json does not parse (EOF while parsing a value at line 1 column 0): the heavy queue takes its groups from the runs anew, and every waiting run may be passed 8 times again";
  let lines = |bytes: Vec<u8>| {
    String::from_utf8(bytes)
      .unwrap()
      .lines()
      .map(|line| line.split_once(' ').unwrap().1.split_once(' ').unwrap().1.to_owned())
      .collect::<Vec<_>>()
  };
  let mut serve = Vec::new();
  log_to(&mut serve, note);
  assert_eq!(lines(serve), [format!("brd-host: {note}")]);
  let (mut supervisor, mut agent) = (Vec::new(), Vec::new());
  supervisor_after_end_to(&mut supervisor, &mut agent, "1-2", note);
  assert_eq!(lines(supervisor), [format!("brd-host supervise: {note}")]);
  assert_eq!(lines(agent), [format!("brd-host supervise 1-2: {note}")]);
}
