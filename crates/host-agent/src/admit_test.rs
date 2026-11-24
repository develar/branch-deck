use super::*;
use pretty_assertions::assert_eq;

/// A run directory of the test
fn run_dir() -> (tempfile::TempDir, RunDir) {
  let dir = tempfile::tempdir().expect("a directory");
  let run = RunDir::new(dir.path().to_path_buf());
  (dir, run)
}

/// The start of a run of `admit_now` answers what its supervisor decided: `busy` for a run that it refused, the start
/// for an admitted run, for a run that ended another way (its follow tells how) and for a supervisor that did not
/// decide in time
#[test]
fn a_start_that_admits_at_once_answers_the_decision_of_its_supervisor() {
  let (_dir, admitted) = run_dir();
  mark_admitted(&admitted).expect("marked");
  assert_eq!(wait_for_decision(&admitted, DECIDE_WAIT), Ok(()));

  let (_dir, refused) = run_dir();
  refused.write_exit(&ExitRecord::refused(&busy("its heavy turn has no room"))).expect("the end");
  let refusal = wait_for_decision(&refused, DECIDE_WAIT).expect_err("busy");
  assert_eq!(refusal.code, Code::Busy);
  assert!(
    refusal
      .message
      .starts_with("<host> takes runs only at once (`idle_only` of the set), and its heavy turn has no room"),
    "{}",
    refusal.message
  );

  let (_dir, failed) = run_dir();
  failed
    .write_exit(&ExitRecord::refused(&Refusal::new(Code::AgentError, "the supervisor failed")))
    .expect("the end");
  assert_eq!(wait_for_decision(&failed, DECIDE_WAIT), Ok(()));

  let (_dir, undecided) = run_dir();
  assert_eq!(wait_for_decision(&undecided, Duration::from_millis(50)), Ok(()));
}
