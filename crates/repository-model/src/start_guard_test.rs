use crate::branch_table::BranchTable;
use crate::facts::TableFacts;
use crate::patronus_view::patronus_view_test::{chain, merged, robot, running, stack_robot};
use crate::push_guard::push_guard_test::stacked_chain;
use crate::push_guard::{GuardFacts, guard_facts};
use crate::start_guard::{CancelCheck, check_cancel, check_restart};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;

/// The facts of a start from `table` with `robots`, as the app's command reads them: the table of the session's last
/// sync, the robots matched as the rows match them; git is never asked (no repository: the hashes tell)
fn facts(table: BranchTable, robots: &[PatronusRobot]) -> GuardFacts {
  guard_facts(&GitCommandExecutor::new(), "/nonexistent", &TableFacts::new(table), robots.to_vec())
}

fn refused(text: &str) -> Result<(), String> {
  Err(text.to_string())
}

/// A restart is a start of the robot's subject that the robot itself does not refuse: its own robot on the last commit
/// restarts, a robot that lands an ancestor or a merged Safe Push that landed it all refuses it. A robot the user's list
/// lacks and one of a temporary ref are refused; a branch without a row restarts on its remote branch
#[test]
fn a_restart_is_a_start_its_own_robot_does_not_refuse_never_of_a_temporary_ref() {
  assert_eq!(check_restart(&facts(chain(), &[running("r", "auth", "va1")]), "r"), Ok(()));
  assert_eq!(check_restart(&facts(stacked_chain(), &[stack_robot("w", "auth", "stack-tip")]), "w"), Ok(()));

  let failed_oauth = robot("f", "oauth", PatronusRobotStatus::Failure, "vo0");
  assert_eq!(
    check_restart(&facts(chain(), &[failed_oauth, running("l", "login", "vl1")]), "f"),
    refused("Nothing was restarted: A Safe Push of login is running: wait until it merges")
  );
  let failed_auth = robot("f", "auth", PatronusRobotStatus::Failure, "va0");
  assert_eq!(
    check_restart(&facts(chain(), &[failed_auth, merged("m", "auth", "va1", 200)]), "f"),
    refused("Nothing was restarted: auth merged into master with its Safe Push (https://patronus/robot/m): nothing of it is left to land")
  );

  assert_eq!(
    check_restart(&facts(chain(), &[]), "x"),
    refused("Nothing was restarted: Patronus lists no robot x of the user")
  );
  let temporary = PatronusRobot {
    branch_name: None,
    ..robot("t", "auth", PatronusRobotStatus::Failure, "va1")
  };
  assert_eq!(
    check_restart(&facts(chain(), &[temporary]), "t"),
    refused("Nothing was restarted: the Safe Push is of a temporary ref (the IDE, the CLI): a restart would test its old commit (https://patronus/robot/t)")
  );
  assert_eq!(check_restart(&facts(chain(), &[robot("g", "gone", PatronusRobotStatus::Failure, "vg1")]), "g"), Ok(()));
}

/// A cancel needs a running robot of the user's list; a canceling one is canceled again, a finished one is refused
#[test]
fn a_cancel_of_a_robot_not_in_the_list_or_finished_is_refused() {
  assert_eq!(check_cancel(&[running("r", "auth", "va1")], "r", false), Ok(CancelCheck::Cancel));
  assert_eq!(
    check_cancel(&[robot("c", "auth", PatronusRobotStatus::Canceling, "va1")], "c", false),
    Ok(CancelCheck::Cancel)
  );
  assert_eq!(
    check_cancel(&[merged("m", "auth", "va1", 200)], "m", false),
    Err("Nothing was canceled: the Safe Push is not running (https://patronus/robot/m)".to_string())
  );
  assert_eq!(
    check_cancel(&[], "x", false),
    Err("Nothing was canceled: Patronus lists no robot x of the user".to_string())
  );
}

/// The cancel of the robot a start or a push replaces: one that finished meanwhile (failed, merged) is answered as it is,
/// already finished, so the start goes on without asking Patronus; a running one is canceled, and a robot the list
/// lacks is still refused
#[test]
fn the_cancel_of_a_replaced_robot_that_finished_meanwhile_is_already_finished() {
  let failed = robot("f", "auth", PatronusRobotStatus::Failure, "va1");
  assert_eq!(check_cancel(std::slice::from_ref(&failed), "f", true), Ok(CancelCheck::AlreadyFinished(Box::new(failed))));
  let landed = merged("m", "auth", "va1", 200);
  assert_eq!(check_cancel(std::slice::from_ref(&landed), "m", true), Ok(CancelCheck::AlreadyFinished(Box::new(landed))));
  assert_eq!(check_cancel(&[running("r", "auth", "va1")], "r", true), Ok(CancelCheck::Cancel));
  assert_eq!(check_cancel(&[], "x", true), Err("Nothing was canceled: Patronus lists no robot x of the user".to_string()));
}
