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

/// A sync builds its branches on the blocking pool and detects the integration on another thread: the lines of every
/// thread name the operation. The subscriber is global, so it sees every thread (nextest runs a process per test).
#[test]
fn every_log_line_of_a_command_names_its_operation() {
  let repo = repo_with_baseline(&[("a.txt", "a")]);
  repo.create_commit_with_files("(auth) Add login", &[("login.txt", "login")]);
  repo.create_commit_with_files("(ui) Add button", &[("button.txt", "button")]);
  let captured = Captured::default();
  let writer = captured.clone();
  let subscriber = tracing_subscriber::fmt()
    .with_writer(move || writer.clone())
    .with_ansi(false)
    .with_max_level(tracing::Level::DEBUG)
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("the first subscriber of this process");
  let (exit_code, output) = run_in(&repo, &["sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"].as_array().map(Vec::len), Some(2), "{output}");

  let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
  let lines: Vec<&str> = logs.lines().filter(|line| line.contains(" DEBUG ") || line.contains(" INFO ")).collect();
  assert!(lines.len() > 20, "too few log lines for a sync of two branches: {logs}");
  let id = lines[0]
    .split("operation_id=")
    .nth(1)
    .and_then(|rest| rest.get(..26))
    .unwrap_or_else(|| panic!("no operation_id: {}", lines[0]));
  let prefix = format!("operation{{op=sync operation_id={id}}}");
  let outside: Vec<&&str> = lines.iter().filter(|line| !line.contains(&prefix)).collect();
  assert!(outside.is_empty(), "lines outside the operation: {outside:#?}");
  // the lines of the branch builds, which run on the blocking pool
  assert!(lines.iter().any(|line| line.contains("merge-tree")), "no merge-tree line: {logs}");
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
