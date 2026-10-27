use crate::journal::journal_test::AppRun;
use crate::journal::read;
use domain_types::error::{ApiError, ErrorCode};
use test_utils::captured_log::CapturedLog;

/// The root span of an operation names it in every line of its run, on the threads of its git processes too, by the id
/// of its record and its entry in the journal; the error it ends with carries that id, which leads from the error to
/// its lines. The subscriber is global, so it sees every thread (nextest runs a process per test)
#[tokio::test(flavor = "multi_thread")]
async fn every_log_line_of_an_operation_names_it_by_the_id_of_its_error_and_its_entry_in_the_journal() {
  // a line as each git process ends, from its span
  let captured = CapturedLog::install(true);
  let app = AppRun::new();
  let answer = app.safe_push(|_op| Err(ApiError::new(ErrorCode::GitError, "the push failed"))).await;

  let id = answer.unwrap_err().operation_id.expect("the error names its operation");
  let [entry] = read(app.common.path()).unwrap().try_into().unwrap();
  assert_eq!(entry.id.0, id);
  let logs = captured.text();
  let span = format!("operation{{op=safe_push operation_id={id} tool=app subject=auth}}");
  let of_the_run: Vec<&str> = logs
    .lines()
    .filter(|line| line.contains("rev-parse") || line.contains("the body of the operation ran"))
    .collect();
  assert!(of_the_run.len() >= 2, "no line of the git process or of the body: {logs}");
  let outside: Vec<&&str> = of_the_run.iter().filter(|line| !line.contains(&span)).collect();
  assert!(outside.is_empty(), "lines of the run outside {span}: {outside:#?}");
}
