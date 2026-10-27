use crate::fixture_test::{PREFIX, environment, repo_with_baseline, run_operation};
use crate::remote_fixture_test::Space;
use crate::{Out, operation_name, parse};
use clap::CommandFactory;
use domain_types::error::HolderTool;
use domain_types::landing::PlannedEffect;
use domain_types::operation::EffectStatus;
use test_utils::captured_log::CapturedLog;

/// A sync builds its branches on the blocking pool and detects the integration on another thread: the lines of every
/// thread name the operation, whose id the JSON carries. The subscriber is global, so it sees every thread (nextest runs
/// a process per test).
#[test]
fn every_log_line_of_a_command_names_its_operation_whose_id_its_json_carries() {
  let repo = repo_with_baseline(&[("a.txt", "a")]);
  repo.create_commit_with_files("(auth) Add login", &[("login.txt", "login")]);
  repo.create_commit_with_files("(ui) Add button", &[("button.txt", "button")]);
  let captured = CapturedLog::install(false);
  let (exit_code, output, operation_id) = run_operation(&environment(repo.path()), &["--prefix", PREFIX, "sync"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["branches"].as_array().map(Vec::len), Some(2), "{output}");

  let logs = captured.text();
  // every line, whatever its level: a warning of a thread outside the span would pass a filter on DEBUG and INFO
  let lines: Vec<&str> = logs.lines().filter(|line| !line.trim().is_empty()).collect();
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
  // the id of the root span is the one of the JSON
  assert_eq!(operation_id, id);
}

/// The operation of the remote a command runs is recorded and journaled under the id of its JSON: the Safe Push of `bd`
/// is in the journal of its repository once, as `bd`'s, with what it did and what it cost from its admission on (its
/// push, its requests to Patronus)
#[test]
fn a_safe_push_is_journaled_under_the_operation_id_of_its_json() {
  let space = Space::new(&["(auth) Add auth"]);
  let (exit_code, output, operation_id) = run_operation(&space.environment(), &["--prefix", PREFIX, "safe-push", "auth"]);
  assert_eq!(exit_code, 0, "{output}");

  let journal = operations::journal::read(&space.repo.path().join(".git")).unwrap();
  let [entry] = journal.as_slice() else { panic!("{journal:#?}") };
  assert_eq!(entry.id.0, operation_id);
  assert_eq!((entry.tool, entry.subject.as_deref(), entry.error.as_ref()), (HolderTool::Bd, Some("auth"), None));
  let started = entry
    .effects
    .iter()
    .any(|record| matches!(record.effect, PlannedEffect::StartRobot { .. }) && record.status == EffectStatus::Done);
  assert!(started, "{entry:#?}");
  assert!(entry.stats.git_processes.get("push") == Some(&1) && entry.stats.http_requests > 0, "{entry:#?}");
}

#[test]
fn the_operation_of_a_nested_command_names_both_words() {
  let matches = crate::Cli::command().try_get_matches_from(["bd", "lease", "status"]).unwrap();
  assert_eq!(operation_name(&matches), "lease status");

  let mut printed = Vec::new();
  let invocation = parse(["version".into()], &environment(std::path::Path::new(".")), &mut Out::of(&mut printed)).unwrap_or_else(|code| panic!("exit {code}"));
  assert_eq!(invocation.op, "version");
}
