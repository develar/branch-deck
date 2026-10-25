use crate::branch_table::BranchTable;
use crate::facts::TableFacts;
use crate::patronus_view::patronus_view_test::{chain, merged, robot, running, stack_robot};
use crate::push_guard::push_guard_test::stacked_chain;
use crate::push_guard::{GuardFacts, guard_facts};
use crate::start_guard::{CancelCheck, check_cancel, check_restart, check_start};
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::stack::{BlockReason, StackBranchConflict, StackBranchState, StackMemberBlock};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;

const SAFE_PUSH: PatronusPushMode = PatronusPushMode::RebaseAutosquash;
const DRY_RUN: PatronusPushMode = PatronusPushMode::DryRun;

/// The facts of a start from `table` with `robots`, as the app's command reads them: the table of the session's last
/// sync, the robots matched as the rows match them; git is never asked (no repository: the hashes tell)
fn facts(table: BranchTable, robots: &[PatronusRobot]) -> GuardFacts {
  guard_facts(&GitCommandExecutor::new(), "/nonexistent", &TableFacts::new(table), robots.to_vec())
}

fn refused(text: &str) -> Result<(), String> {
  Err(text.to_string())
}

/// `stacked_chain` with login parked
fn parked_login() -> BranchTable {
  let mut table = stacked_chain();
  table.parked.insert("login".to_string(), "IJPL-1".to_string());
  table
}

/// A parked branch stays local, and so does a branch built on one and the whole stack with a parked member: neither a
/// Safe Push nor a Dry Run starts; the branch below starts
#[test]
fn a_parked_branch_a_branch_built_on_parked_and_a_whole_stack_with_a_parked_member_are_refused() {
  let facts = facts(parked_login(), &[]);
  for mode in [SAFE_PUSH, DRY_RUN] {
    assert_eq!(
      check_start(&facts, &Subject::branch("login"), mode),
      refused("Nothing was started: login is parked: IJPL-1; it stays local until the user unparks it")
    );
  }
  assert_eq!(
    check_start(&facts, &Subject::branch("oauth"), SAFE_PUSH),
    refused("Nothing was started: oauth is built on parked login: IJPL-1; it stays local until the user unparks it")
  );
  assert_eq!(
    check_start(&facts, &Subject::whole_stack("auth"), SAFE_PUSH),
    refused("Nothing was started: the whole auth stack holds what is parked: login is parked: IJPL-1; land the stack in pieces")
  );
  assert_eq!(check_start(&facts, &Subject::branch("auth"), SAFE_PUSH), Ok(()));
}

/// A merged Safe Push that landed all of a branch left nothing of it to land, for a Dry Run too; once every branch of
/// the whole stack landed, the whole stack has nothing left either
#[test]
fn a_branch_and_a_whole_stack_merged_safe_pushes_landed_whole_are_refused() {
  let landed = facts(chain(), &[merged("m", "auth", "va1", 200)]);
  for mode in [SAFE_PUSH, DRY_RUN] {
    assert_eq!(
      check_start(&landed, &Subject::branch("auth"), mode),
      refused("Nothing was started: auth merged into master with its Safe Push (https://patronus/robot/m): nothing of it is left to land")
    );
  }
  assert_eq!(check_start(&landed, &Subject::branch("login"), SAFE_PUSH), Ok(()));
  assert_eq!(
    check_start(&facts(stacked_chain(), &[merged("o", "oauth", "vo1", 300)]), &Subject::whole_stack("auth"), SAFE_PUSH),
    refused("Nothing was started: every branch of the whole auth stack landed in master with the Safe Push of oauth: nothing of it is left to land")
  );
}

/// A branch the last sync did not build, a stack branch that is not the build of its commits and one whose commits
/// conflict as one branch are no build of what lands
#[test]
fn a_branch_and_a_stack_branch_the_sync_did_not_build_are_refused() {
  let mut table = stacked_chain();
  table.not_built.insert("login".to_string(), BlockReason::Conflict);
  assert_eq!(
    check_start(&facts(table, &[]), &Subject::branch("login"), SAFE_PUSH),
    refused("Nothing was started: login is not synced (a merge conflict)")
  );

  let mut blocked = stacked_chain();
  blocked.stack_branches[0].state = StackBranchState::Blocked;
  blocked.stack_branches[0].blocked_by = Some(StackMemberBlock {
    member: "login".to_string(),
    reason: BlockReason::Conflict,
  });
  assert_eq!(
    check_start(&facts(blocked, &[]), &Subject::whole_stack("auth"), DRY_RUN),
    refused("Nothing was started: the stack branch of auth is not the build of its commits: login conflicts, so the stack branch was not rebuilt; fix it, then sync")
  );

  let mut conflict = stacked_chain();
  conflict.stack_branches[0].state = StackBranchState::Conflict;
  conflict.stack_branches[0].conflict = Some(StackBranchConflict {
    commit: "o1".to_string(),
    files: vec!["a.rs".to_string()],
  });
  assert_eq!(
    check_start(&facts(conflict, &[]), &Subject::whole_stack("auth"), SAFE_PUSH),
    refused(
      "Nothing was started: the whole auth stack doesn't build as one branch: o1 (o1, of oauth) conflicts in a.rs with the commits of the stack before it, though each branch builds. Land it in pieces: the Safe Push of a branch lands it with what it is built on; or regroup so that the commits it needs come before it"
    )
  );
}

/// A robot on the commit the branch is at refuses a Safe Push and a Dry Run; one of a branch built on it that lands it
/// with more, and one that lands an ancestor, refuse only a Safe Push. The whole stack waits for its own robot on its
/// stack branch and for the robot of a member
#[test]
fn a_robot_on_the_last_commit_or_one_that_lands_a_part_of_the_subject_is_refused() {
  let own = facts(chain(), &[running("r", "auth", "va1")]);
  for mode in [SAFE_PUSH, DRY_RUN] {
    assert_eq!(
      check_start(&own, &Subject::branch("auth"), mode),
      refused("Nothing was started: A Safe Push of the last commit is running")
    );
  }

  let login = facts(stacked_chain(), &[running("l", "login", "vl1")]);
  assert_eq!(
    check_start(&login, &Subject::branch("auth"), SAFE_PUSH),
    refused("Nothing was started: A Safe Push of login is running: it lands 1 commit of auth with it (a1)")
  );
  assert_eq!(
    check_start(&login, &Subject::branch("oauth"), SAFE_PUSH),
    refused("Nothing was started: A Safe Push of login is running: wait until it merges")
  );
  assert_eq!(check_start(&login, &Subject::branch("oauth"), DRY_RUN), Ok(()));
  assert_eq!(
    check_start(&login, &Subject::whole_stack("auth"), SAFE_PUSH),
    refused("Nothing was started: A Safe Push of login is running: wait until it merges")
  );

  let stack = facts(stacked_chain(), &[stack_robot("w", "auth", "stack-tip")]);
  assert_eq!(
    check_start(&stack, &Subject::whole_stack("auth"), DRY_RUN),
    refused("Nothing was started: A Safe Push of the last commit is running")
  );
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
