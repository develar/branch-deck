use crate::branch_table::BranchTable;
use crate::patronus_view::patronus_view_test::{chain, merged, robot, running, stack_robot, table};
use crate::push_guard::{check_push, push_guarded, push_guarded_within};
use crate::views::RepositoryViews;
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::stack::{BlockReason, StackBranchConflict, StackBranchState, StackBranchUpdate, StackMemberBlock};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{RepositoryLock, lock_repository_in};
use pretty_assertions::assert_eq;
use std::sync::mpsc;
use std::time::Duration;
use sync_core::push::{NOT_SYNCED_YET, PushBranchesOutcome, PushBranchesParams, StackPushBranch};
use sync_core::transfer_progress::TransferProgress;
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

/// `beta` of `stack_ahead_of_bare_origin`, built on `alpha`
fn beta() -> StackPushBranch {
  StackPushBranch {
    parent_branch: Some("alpha".to_string()),
    ..branch("beta")
  }
}

/// The push of `branches` of `repo`
fn push_of(repo: &TestRepo, branches: Vec<StackPushBranch>) -> PushBranchesParams {
  PushBranchesParams {
    repository_path: repo.path_str().to_string(),
    branch_prefix: "test".to_string(),
    my_email: None,
    baseline_branch: "origin/master".to_string(),
    branches,
  }
}

/// `stack_ahead_of_bare_origin` and the push of `alpha` and `beta` built on it, parents first
fn pushable() -> (TestRepo, PushBranchesParams) {
  let repo = stack_ahead_of_bare_origin();
  let params = push_of(&repo, vec![branch("alpha"), beta()]);
  (repo, params)
}

/// The branches of the bare origin of `stack_ahead_of_bare_origin` under the prefix
fn remote_branches(repo: &TestRepo) -> String {
  repo.git(&["--git-dir", ".git/origin.git", "for-each-ref", "--format=%(refname)", "refs/heads/test/"])
}

/// The table a sync of `stack_ahead_of_bare_origin` leaves: `alpha` and `beta` built on it, as the checks need a row
fn synced_alpha_beta() -> BranchTable {
  table(&[("beta", Some("alpha"), &[("b1", "vb1")]), ("alpha", None, &[("a1", "va1")])])
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

  views.keep(repo.path_str(), "test", synced_alpha_beta());
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

/// `pushable` synced, with its common dir, its views and the repository lock, which the test holds as another holder
fn pushable_while_locked() -> (TestRepo, PushBranchesParams, std::path::PathBuf, RepositoryViews, RepositoryLock) {
  let (repo, params) = pushable();
  let common_dir = repo.path().join(".git");
  let views = RepositoryViews::default();
  views.keep(repo.path_str(), "test", synced_alpha_beta());
  let held = lock_repository_in(&common_dir, Duration::ZERO, &|| {}).unwrap();
  (repo, params, common_dir, views, held)
}

/// Another holder of the repository lock (`bd pick --follow` of an agent, a rewrite of the app) keeps the push waiting,
/// without a bound, as long as it lives: the progress of the push names it ("Waiting for `…` (pid …, for 0 s)"), and
/// once it releases the lock, the push goes. Here the holder is the test itself, alive; the bound for a holder no record
/// names is zero, so a wait that counted it would refuse at once
#[test]
fn a_push_waits_while_the_holder_of_the_lock_lives_and_names_it() {
  let (repo, params, common_dir, views, held) = pushable_while_locked();
  let git = GitCommandExecutor::new();
  let (progress, told) = mpsc::channel();
  std::thread::scope(|scope| {
    let push = scope.spawn(|| {
      let mut on_progress = |tick: TransferProgress| progress.send(tick).unwrap();
      push_guarded_within(Duration::ZERO, &|| {}, &git, &common_dir, &views, Vec::new(), &params, &mut on_progress)
    });
    let waiting = told.recv_timeout(Duration::from_mins(1)).unwrap();
    assert!(waiting.waiting, "{waiting:?}");
    assert!(waiting.line.starts_with("Waiting for `"), "{waiting:?}");
    assert!(waiting.line.contains(&format!("(pid {}, for ", std::process::id())), "{waiting:?}");
    assert_eq!(remote_branches(&repo), "");
    drop(held);
    assert!(matches!(push.join().unwrap(), Ok(PushBranchesOutcome::Pushed { .. })));
  });
  assert_eq!(remote_branches(&repo), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// A lock no record names a live holder of (a holder that writes none, an older Branch Deck) refuses the push after the
/// bounded wait, and nothing is pushed
#[test]
fn a_push_while_a_holder_no_record_names_holds_the_lock_is_refused() {
  let (repo, params, common_dir, views, held) = pushable_while_locked();
  std::fs::remove_file(common_dir.join("branch-deck.lock.holder")).unwrap();
  let git = GitCommandExecutor::new();
  let refused = push_guarded_within(Duration::from_millis(100), &|| {}, &git, &common_dir, &views, Vec::new(), &params, &mut |_| {}).unwrap_err();
  assert_eq!(
    refused.to_string(),
    "Nothing was pushed: another Branch Deck operation holds the repository; try again when it is done"
  );
  drop(held);
  assert_eq!(remote_branches(&repo), "");
}

/// The push of `branch` from a thread of `scope`, held in its first progress (the network push, after the lock) until
/// `go` sends or drops: it says `pushes` on `events` then
fn held_push<'scope>(
  scope: &'scope std::thread::Scope<'scope, '_>,
  fixture: &'scope (TestRepo, std::path::PathBuf, RepositoryViews, GitCommandExecutor),
  branch: StackPushBranch,
  events: mpsc::Sender<&'static str>,
  go: mpsc::Receiver<()>,
) -> std::thread::ScopedJoinHandle<'scope, anyhow::Result<PushBranchesOutcome>> {
  let (repo, common_dir, views, git) = fixture;
  let params = push_of(repo, vec![branch]);
  scope.spawn(move || {
    let mut held = Some(go);
    let mut on_progress = |_| {
      if let Some(go) = held.take() {
        events.send("pushes").unwrap();
        // an error is a dropped sender (a failed step of the test), which ends the hold as well
        let _released = go.recv_timeout(Duration::from_mins(1));
      }
    };
    push_guarded_within(Duration::from_mins(1), &|| {}, git, common_dir, views, Vec::new(), &params, &mut on_progress)
  })
}

/// `stack_ahead_of_bare_origin` synced, with its common dir, its views and an executor
fn synced_fixture() -> (TestRepo, std::path::PathBuf, RepositoryViews, GitCommandExecutor) {
  let repo = stack_ahead_of_bare_origin();
  let common_dir = repo.path().join(".git");
  let views = RepositoryViews::default();
  views.keep(repo.path_str(), "test", synced_alpha_beta());
  (repo, common_dir, views, GitCommandExecutor::new())
}

fn assert_pushed(pushed: &anyhow::Result<PushBranchesOutcome>) {
  assert!(
    matches!(pushed, Ok(PushBranchesOutcome::Pushed { remote_statuses }) if remote_statuses.len() == 1),
    "{pushed:?}"
  );
}

/// Two Safe Pushes of the app of different branches run side by side: the lock is held only while a push is decided,
/// so while the first one pushes (held in its progress) the lock is free, and the second one, which waits for no holder
/// at all (`Duration::ZERO`), decides, pushes and ends before the first does. A bound of a minute only ends a broken run
#[test]
fn the_pushes_of_two_branches_run_side_by_side() {
  let fixture = synced_fixture();
  let (repo, common_dir, views, git) = &fixture;
  let (events, event) = mpsc::channel();
  std::thread::scope(|scope| {
    let (go, gone) = mpsc::channel::<()>();
    let first = held_push(scope, &fixture, branch("alpha"), events.clone(), gone);
    assert_eq!(event.recv_timeout(Duration::from_mins(1)).unwrap(), "pushes");
    drop(lock_repository_in(common_dir, Duration::ZERO, &|| {}).expect("the lock is free while the first one pushes"));

    let params = push_of(repo, vec![beta()]);
    let second = push_guarded_within(
      Duration::ZERO,
      &|| panic!("beta waits for nobody"),
      git,
      common_dir,
      views,
      Vec::new(),
      &params,
      &mut |_| {},
    );
    assert_pushed(&second);
    assert!(!first.is_finished());
    go.send(()).unwrap();
    assert_pushed(&first.join().unwrap());
  });
  assert_eq!(remote_branches(repo), "refs/heads/test/virtual/alpha\nrefs/heads/test/virtual/beta");
}

/// Two pushes of one branch (a second Safe Push of it, two whole stacks of one stack branch) don't run side by side:
/// both lease on the head the user saw, so the second waits for the first, without a bound, and then finds the remote
/// at its commit instead of a lease the first broke. Both push
#[test]
fn the_pushes_of_one_ref_wait_for_each_other_and_both_push() {
  let fixture = synced_fixture();
  let (repo, common_dir, views, git) = &fixture;
  let (events, event) = mpsc::channel();
  let next_event = || event.recv_timeout(Duration::from_mins(1)).unwrap();
  std::thread::scope(|scope| {
    let (go, gone) = mpsc::channel::<()>();
    let first = held_push(scope, &fixture, branch("alpha"), events.clone(), gone);
    assert_eq!(next_event(), "pushes");

    let params = push_of(repo, vec![branch("alpha")]);
    let second = scope.spawn(move || {
      let on_wait = || events.send("waits").unwrap();
      push_guarded_within(Duration::ZERO, &on_wait, git, common_dir, views, Vec::new(), &params, &mut |_| {})
    });
    assert_eq!(next_event(), "waits");
    go.send(()).unwrap();
    assert_pushed(&first.join().unwrap());
    assert_pushed(&second.join().unwrap());
  });
  assert_eq!(remote_branches(repo), "refs/heads/test/virtual/alpha");
}
