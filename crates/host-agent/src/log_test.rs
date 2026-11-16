use super::panics_reach;
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
  let line = &written[1];
  assert!(
    line.starts_with("agent log: brd-host supervise 1-1: panicked: panicked at crates/host-agent/src/log_test.rs:") && line.ends_with(" the give-back broke\n"),
    "{line:?}"
  );
}
