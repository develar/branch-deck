use crate::fixture_test::{repo_with_baseline, run_in};
use crate::{operation_name, parse};
use bd_core::context::Environment;
use clap::CommandFactory;
use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

/// The lines a subscriber writes, kept for the assertions
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
  fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}

#[test]
fn every_log_line_of_a_command_names_its_operation() {
  let repo = repo_with_baseline(&[("a.txt", "a")]);
  repo.create_commit_with_files("(auth) Add login", &[("login.txt", "login")]);
  let captured = Captured::default();
  let writer = captured.clone();
  let subscriber = tracing_subscriber::fmt()
    .with_writer(move || writer.clone())
    .with_ansi(false)
    .with_max_level(tracing::Level::DEBUG)
    .finish();
  let (exit_code, output) = tracing::subscriber::with_default(subscriber, || run_in(&repo, &["status"]));
  assert_eq!(exit_code, 0, "{output}");

  let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
  let lines: Vec<&str> = logs.lines().filter(|line| line.contains(" DEBUG ") || line.contains(" INFO ")).collect();
  assert!(!lines.is_empty(), "no log lines: {logs}");
  let id = lines[0]
    .split("operation_id=")
    .nth(1)
    .and_then(|rest| rest.get(..26))
    .unwrap_or_else(|| panic!("no operation_id: {}", lines[0]));
  for line in &lines {
    assert!(line.contains(&format!("operation{{op=status operation_id={id}}}")), "{line}");
  }
  // the JSON output is unchanged
  assert!(output.get("operation_id").is_none(), "{output}");
}

#[test]
fn the_operation_of_a_nested_command_names_both_words() {
  let matches = crate::Cli::command().try_get_matches_from(["bd", "lease", "status"]).unwrap();
  assert_eq!(operation_name(&matches), "lease status");

  let invocation = parse(["version".into()], &Environment::from_process(), &mut Vec::new()).unwrap_or_else(|code| panic!("exit {code}"));
  assert_eq!(invocation.op, "version");
}
