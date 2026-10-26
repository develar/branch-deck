use crate::branch_table::BranchTable;
use crate::facts::TableFacts;
use crate::patronus_view::patronus_view_test::{chain, merged, robot, running, stack_robot};
use crate::push_guard::{check_push, guard_facts};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::stack::{BlockReason, StackBranchConflict, StackBranchState, StackBranchUpdate, StackMemberBlock};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use sync_core::push::StackPushBranch;

fn branch(name: &str) -> StackPushBranch {
  StackPushBranch {
    branch_name: name.to_string(),
    parent_branch: None,
    expected_remote: None,
    tip: None,
    whole_stack: false,
    unapplied: false,
  }
}

fn whole_stack(root: &str) -> StackPushBranch {
  StackPushBranch {
    whole_stack: true,
    ..branch(root)
  }
}

/// What the checks answer for a push of `branches` from `table` with `robots`, as the app's push reads them: the table
/// of the session's last sync, the robots matched as the rows match them; git is never asked (no repository: the hashes
/// tell)
fn check(table: BranchTable, robots: &[PatronusRobot], branches: &[StackPushBranch]) -> Result<(), String> {
  let facts = guard_facts(&GitCommandExecutor::new(), "/nonexistent", &TableFacts::new(table), robots.to_vec());
  check_push(&facts, branches)
}

/// `chain` (auth, login built on it, oauth on login) whose whole auth stack has a built stack branch at `stack-tip`
pub(crate) fn stacked_chain() -> BranchTable {
  let mut table = chain();
  table.stack_branches.push(StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "test/stack/auth".to_string(),
    members: vec!["auth".to_string(), "login".to_string(), "oauth".to_string()],
    commit_count: 3,
    state: StackBranchState::Built,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  });
  table
}

/// A parked branch stays local, and so does a branch built on one, which carries its commits; the branch below it
/// pushes. The stack branch of a whole stack with a parked member stays local too
#[test]
fn a_parked_branch_and_a_branch_built_on_parked_are_refused() {
  let parked = || {
    let mut table = stacked_chain();
    table.parked.insert("login".to_string(), "IJPL-1".to_string());
    table
  };
  assert_eq!(
    check(parked(), &[], &[branch("login")]),
    Err("Nothing was pushed: login is parked: IJPL-1; it stays local until the user unparks it".to_string())
  );
  assert_eq!(
    check(parked(), &[], &[branch("auth"), branch("oauth")]),
    Err("Nothing was pushed: oauth is built on parked login: IJPL-1; it stays local until the user unparks it".to_string())
  );
  assert_eq!(check(parked(), &[], &[branch("auth")]), Ok(()));
  assert_eq!(
    check(parked(), &[], &[whole_stack("auth")]),
    Err("Nothing was pushed: test/stack/auth holds parked login; land the stack in pieces".to_string())
  );
}

/// A merged Safe Push that took the branch as it is landed all of it: nothing of it is left to push, the branch built
/// on it still pushes. Once every branch of the whole stack landed, its stack branch has nothing left either
#[test]
fn a_branch_a_merged_safe_push_landed_whole_is_refused() {
  let auth = merged("m", "auth", "va1", 200);
  assert_eq!(
    check(chain(), std::slice::from_ref(&auth), &[branch("auth")]),
    Err("Nothing was pushed: auth merged into master with its Safe Push (https://patronus/robot/m): nothing of it is left to push".to_string())
  );
  assert_eq!(check(chain(), std::slice::from_ref(&auth), &[branch("login")]), Ok(()));

  let oauth = merged("o", "oauth", "vo1", 300);
  assert_eq!(
    check(stacked_chain(), &[oauth], &[whole_stack("auth")]),
    Err("Nothing was pushed: test/stack/auth: every branch of the whole auth stack landed in master with the Safe Push of oauth: nothing of it is left to push".to_string())
  );
}

/// The robot that runs on the branch itself holds it: a push moves the branch it took its commit from and fails it at
/// the end. Canceling (the page canceled it before Push and Restart) or a dry run, it holds nothing
#[test]
fn a_branch_whose_own_safe_push_runs_is_refused_until_it_is_canceled() {
  assert_eq!(
    check(chain(), &[running("r", "auth", "va1")], &[branch("auth")]),
    Err(
      "Nothing was pushed: A Safe Push of auth is running on test/virtual/auth: a push moves the branch it took its commit from, so it won't land and fails the Safe Push at the end. Wait until it merges, or restart it on the current branch (https://patronus/robot/r)"
        .to_string()
    )
  );
  assert_eq!(check(chain(), &[robot("r", "auth", PatronusRobotStatus::Canceling, "va1")], &[branch("auth")]), Ok(()));
}

/// A Safe Push of a branch built on it holds a branch while the robot's commit has commits of it, and the robot of the
/// whole stack holds the stack branch it runs on
#[test]
fn a_branch_a_running_safe_push_lands_and_a_stack_branch_its_robot_runs_on_are_refused() {
  assert_eq!(
    check(chain(), &[running("l", "login", "vl1")], &[branch("auth")]),
    Err("Nothing was pushed: A Safe Push of login is running: it lands 1 commit of auth with it (a1), a push won't land (https://patronus/robot/l)".to_string())
  );
  assert_eq!(
    check(stacked_chain(), &[stack_robot("w", "auth", "stack-tip")], &[whole_stack("auth")]),
    Err(
      "Nothing was pushed: test/stack/auth: a Safe Push of the whole auth stack is running on it: a push moves the branch it took its commit from and fails the Safe Push at the end; wait until it merges (https://patronus/robot/w)"
        .to_string()
    )
  );
}

/// The stack branch of a whole stack is pushed only `built`, as `bd push` pushes it and as the start of the whole stack
/// refuses it: a conflicted one names the commit, a stale or blocked one names what kept it from being rebuilt. The page
/// decided to push it while it was built; a sync since (which takes no lock) may have left it otherwise
#[test]
fn a_stack_branch_that_is_not_built_is_refused() {
  let stacked = |state: StackBranchState, tip: Option<&str>| {
    let mut table = stacked_chain();
    let stack = &mut table.stack_branches[0];
    stack.state = state;
    stack.tip = tip.map(str::to_string);
    table
  };
  let mut conflict = stacked(StackBranchState::Conflict, Some("stack-tip"));
  conflict.stack_branches[0].conflict = Some(StackBranchConflict {
    commit: "l1".to_string(),
    files: vec!["a.rs".to_string()],
  });
  assert_eq!(
    check(conflict, &[], &[whole_stack("auth")]),
    Err("Nothing was pushed: test/stack/auth: the whole auth stack doesn't build as one branch: l1 (l1, of login) conflicts in a.rs with the commits of the stack before it, though each branch builds".to_string())
  );
  let mut stale = stacked(StackBranchState::Stale, Some("stack-tip"));
  stale.stack_branches[0].blocked_by = Some(StackMemberBlock {
    member: "login".to_string(),
    reason: BlockReason::Conflict,
  });
  assert_eq!(
    check(stale, &[], &[whole_stack("auth")]),
    Err("Nothing was pushed: test/stack/auth: the stack branch of auth is not the build of its commits: login conflicts, so the stack branch was not rebuilt".to_string())
  );
  assert_eq!(
    check(stacked(StackBranchState::Built, None), &[], &[whole_stack("auth")]),
    Err("Nothing was pushed: test/stack/auth: the stack branch of auth is not the build of its commits: the stack branch was not rebuilt".to_string())
  );
  assert_eq!(check(stacked(StackBranchState::Built, Some("stack-tip")), &[], &[whole_stack("auth")]), Ok(()));
}

/// A branch the table of the last sync has no row of is no branch to push (archived, renamed, a name by IPC), as the
/// start refuses it and `bd push` answers `unknown_branch`; a whole stack without a stack branch likewise
#[test]
fn a_branch_the_last_sync_has_no_row_of_is_refused() {
  assert_eq!(
    check(chain(), &[], &[branch("gone")]),
    Err("Nothing was pushed: gone is no branch of the last sync".to_string())
  );
  assert_eq!(
    check(chain(), &[], &[whole_stack("auth")]),
    Err("Nothing was pushed: the whole auth stack has no stack branch of the last sync".to_string())
  );
}

/// An unapplied branch is no branch of the table: its push takes only the lock, also under the name of a parked one
#[test]
fn an_unapplied_branch_is_checked_by_nothing_but_the_lock() {
  let mut table = chain();
  table.parked.insert("auth".to_string(), String::new());
  let unapplied = StackPushBranch {
    unapplied: true,
    ..branch("auth")
  };
  assert_eq!(check(table, &[running("r", "auth", "va1")], &[unapplied]), Ok(()));
}
