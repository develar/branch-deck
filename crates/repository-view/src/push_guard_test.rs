use crate::branch_table::BranchTable;
use crate::patronus_view::patronus_view_test::{chain, merged, robot, running, stack_robot};
use crate::push_guard::{check_push, push_guarded, push_guarded_within};
use crate::views::RepositoryViews;
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::lock_repository_in;
use pretty_assertions::assert_eq;
use std::time::Duration;
use sync_core::push::{NOT_SYNCED_YET, PushBranchesOutcome, PushBranchesParams, StackPushBranch};
use test_utils::git_test_utils::{TestRepo, stack_ahead_of_bare_origin};

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
/// kept by the views, the robots matched as the rows match them; git is never asked (no repository: the hashes tell)
fn check(table: BranchTable, robots: &[PatronusRobot], branches: &[StackPushBranch]) -> Result<(), String> {
  let views = RepositoryViews::default();
  views.keep("/nonexistent", "test", table);
  let facts = views.guard_facts(&GitCommandExecutor::new(), "/nonexistent", "test", robots.to_vec()).unwrap();
  check_push(&facts, branches)
}

/// `chain` (auth, login built on it, oauth on login) whose whole auth stack has a built stack branch at `stack-tip`
fn stacked_chain() -> BranchTable {
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

/// `stack_ahead_of_bare_origin` and the push of `alpha` and `beta` built on it, parents first
fn pushable() -> (TestRepo, PushBranchesParams) {
  let repo = stack_ahead_of_bare_origin();
  let params = PushBranchesParams {
    repository_path: repo.path_str().to_string(),
    branch_prefix: "test".to_string(),
    my_email: None,
    baseline_branch: "origin/master".to_string(),
    branches: vec![
      branch("alpha"),
      StackPushBranch {
        parent_branch: Some("alpha".to_string()),
        ..branch("beta")
      },
    ],
  };
  (repo, params)
}

/// The guarded push of a stack runs the git processes of the core push (`an_atomic_push_of_a_stack`) and nothing more:
/// the lock is a file of the known common dir, the checks read the kept table; before the first sync it is refused
#[test]
fn a_guarded_push_runs_the_processes_of_the_core_push() {
  let (repo, params) = pushable();
  let common_dir = repo.path().join(".git");
  let views = RepositoryViews::default();
  let git = GitCommandExecutor::new().with_process_stats("test");
  let refused = push_guarded(&git, &common_dir, &views, Vec::new(), &params, &mut |_| {}).unwrap_err();
  assert_eq!(refused.to_string(), NOT_SYNCED_YET);

  views.keep(repo.path_str(), "test", BranchTable::default());
  let outcome = push_guarded(&git, &common_dir, &views, Vec::new(), &params, &mut |_| {}).unwrap();
  assert!(
    matches!(outcome, PushBranchesOutcome::Pushed { ref remote_statuses } if remote_statuses.len() == 2),
    "{outcome:?}"
  );
  assert_eq!(
    git.process_stats().unwrap().counts(),
    [("for-each-ref".to_string(), 2), ("push".to_string(), 1), ("reflog".to_string(), 1)]
  );
}

/// Another holder of the repository lock (a rewrite, `bd sync`) refuses the push after the wait, and nothing is pushed
#[test]
fn a_push_while_another_operation_holds_the_lock_is_refused() {
  let (repo, params) = pushable();
  let common_dir = repo.path().join(".git");
  let views = RepositoryViews::default();
  views.keep(repo.path_str(), "test", BranchTable::default());
  let held = lock_repository_in(&common_dir, Duration::ZERO, &|| {}).unwrap();
  let git = GitCommandExecutor::new();
  let refused = push_guarded_within(Duration::from_millis(100), &git, &common_dir, &views, Vec::new(), &params, &mut |_| {}).unwrap_err();
  assert!(refused.to_string().starts_with("Nothing was pushed: "), "{refused}");
  assert!(refused.to_string().ends_with(" holds the repository; try again when it is done"), "{refused}");
  drop(held);
  assert_eq!(repo.git(&["--git-dir", ".git/origin.git", "for-each-ref", "--format=%(refname)", "refs/heads/test/"]), "");
}
