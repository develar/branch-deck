use crate::stack_branch::{BuildResult, StackBranchPlan, StackBranchSync, StackBuild, StackFinish, build_stack_branch, finish_stack_branches, plan_stack_branches};
use domain_types::commit::{Commit, SubjectParts};
use domain_types::stack::{BlockReason, StackBranchConflict, StackBranchState, StackMemberBlock};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::get_commit_list;
use git_ops::tree_id_cache::TreeIdCache;
use git_ops::word_resolve::ResolveMode;
use history_rewrite::commit_grouper::CommitGrouper;
use history_rewrite::stack::place_branches;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_utils::git_test_utils::TestRepo;

/// A commit of `subject` by `email`, without git
fn commit(index: usize, subject: &str, email: &str) -> Commit {
  Commit {
    id: format!("c{index}"),
    subject: subject.to_string(),
    stripped_subject: subject.to_string(),
    message: subject.to_string(),
    author_name: "Test Author".to_string(),
    author_email: email.to_string(),
    author_timestamp: 1_700_000_000,
    committer_timestamp: 1_700_000_000,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    parent_id: None,
    tree_id: String::new(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
    subject_parts: SubjectParts::default(),
  }
}

/// The plans of `commits` grouped with stacked branches on, in this order of the history
fn plans_of(commits: &[Commit]) -> Vec<StackBranchPlan> {
  let mut grouper = CommitGrouper::new_with_stacks(true);
  for commit in commits {
    grouper.add_commit(commit.clone());
  }
  let (grouped, forest) = grouper.finish_with_stacks();
  let forest = forest.expect("stacked branches are on");
  let placements = place_branches(&forest, &HashMap::new());
  let by_branch: HashMap<&str, &[Commit]> = grouped.branches.iter().map(|(name, commits)| (name.as_str(), commits.as_slice())).collect();
  let positions: HashMap<&str, usize> = commits.iter().enumerate().map(|(index, commit)| (commit.id.as_str(), index)).collect();
  plan_stack_branches("test", &forest.order, &placements, &by_branch, &positions)
}

fn plans(subjects: &[&str]) -> Vec<StackBranchPlan> {
  let commits: Vec<Commit> = subjects.iter().enumerate().map(|(index, subject)| commit(index, subject, "jane@example.com")).collect();
  plans_of(&commits)
}

fn ids(plan: &StackBranchPlan) -> Vec<&str> {
  plan.commits.iter().map(|commit| commit.id.as_str()).collect()
}

fn roots(plans: &[StackBranchPlan]) -> Vec<(&str, Vec<&str>)> {
  plans.iter().map(|plan| (plan.root.as_str(), plan.members.iter().map(String::as_str).collect())).collect()
}

#[test]
fn a_stack_forks_where_a_branch_has_two_children_and_takes_its_commits_in_history_order() {
  let plans = plans(&[
    "(auth) Add auth",
    "(auth/login) Add form",
    "(docs) Document",
    "(auth/session) Add session",
    "(login/oauth) Add OAuth",
    "(auth) Add logout",
  ]);

  assert_eq!(roots(&plans), [("auth", vec!["auth", "login", "session", "oauth"])]);
  let plan = &plans[0];
  assert_eq!(plan.full_name, "test/stack/auth");
  assert_eq!(ids(plan), ["c0", "c1", "c3", "c4", "c5"]);
  assert_eq!(plan.my_email.as_deref(), Some("jane@example.com"));
  assert_eq!(plan.blocked_by, None);
}

/// The fork may be anywhere below the root, and each stack of the forest gets its own stack branch
#[test]
fn a_fork_below_the_root_and_two_stacks() {
  let plans = plans(&[
    "(auth) Add auth",
    "(auth/login) Add form",
    "(login/oauth) Add OAuth",
    "(login/sso) Add SSO",
    "(ui) Add UI",
    "(ui/dark) Dark mode",
    "(ui/light) Light mode",
  ]);

  assert_eq!(roots(&plans), [("auth", vec!["auth", "login", "oauth", "sso"]), ("ui", vec!["ui", "dark", "light"])]);
}

#[test]
fn a_straight_chain_and_a_lone_branch_have_none() {
  assert_eq!(
    roots(&plans(&["(auth) Add auth", "(auth/login) Add form", "(login/oauth) Add OAuth", "(docs) Document"])),
    []
  );
}

/// A member with a stack problem keeps the stack branch from being built, and the branches it blocks are in the stack
/// too; a cycle belongs to no stack
#[test]
fn a_member_with_a_problem_blocks_the_stack_and_a_cycle_is_in_none() {
  let plans = plans(&[
    "(auth) Add auth",
    "(auth/login) Add form",
    "(auth/session) Add session",
    // a branch that is its own parent: a problem of `x`, which still declares `session` as its parent
    "(session/x) Add x",
    "(x/x) Loop",
    "(x/y) Below the broken one",
    "(p/q) One way",
    "(q/p) And back",
  ]);

  assert_eq!(roots(&plans), [("auth", vec!["auth", "login", "session", "x", "y"])]);
  assert_eq!(
    plans[0].blocked_by,
    Some(StackMemberBlock {
      member: "x".to_string(),
      reason: BlockReason::Problem,
    })
  );
}

/// The author of the most commits, of a tie the one seen first
#[test]
fn the_email_of_a_stack_branch_is_its_most_frequent_author() {
  let commits = [
    commit(0, "(auth) Add auth", "bob@example.com"),
    commit(1, "(auth/login) Add form", "jane@example.com"),
    commit(2, "(auth/session) Add session", "jane@example.com"),
  ];
  assert_eq!(plans_of(&commits)[0].my_email.as_deref(), Some("jane@example.com"));
}

/// A commit whose copy conflicts with the commits of the stack before it stops the build, named with its files, and the
/// finish keeps the ref where it was
#[test]
fn a_conflicting_build_names_the_commit_and_keeps_the_ref() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Initial commit", "README.md", "# Test");
  let path = repo.path_str().to_string();
  repo.create_commit("Add config", "config.txt", "a\n");
  let changed = repo.create_commit("(auth) Change config", "config.txt", "b\n");
  let git = GitCommandExecutor::new();
  let commits: Vec<Commit> = get_commit_list(&git, &path, &base).unwrap().into_iter().filter(|commit| commit.id == changed).collect();

  let build = build_stack_branch(&git, &path, "test/stack/auth", &TreeIdCache::new(), &base, &commits, ResolveMode::Off).unwrap();

  let conflict = StackBuild::Conflict {
    commit: changed.clone(),
    files: vec!["config.txt".to_string()],
  };
  assert_eq!(build, conflict);
  repo.create_branch_at("test/stack/auth", &base).unwrap();
  let plan = StackBranchPlan {
    root: "auth".to_string(),
    full_name: "test/stack/auth".to_string(),
    members: vec!["auth".to_string()],
    commits,
    my_email: None,
    blocked_by: None,
  };
  let sync = StackBranchSync {
    plans: vec![plan],
    tips: HashMap::from([("test/stack/auth".to_string(), base.clone())]),
    ..StackBranchSync::default()
  };
  let finish = StackFinish {
    git_executor: &git,
    repository_path: &path,
    remote_statuses: None,
    failed: &HashMap::new(),
  };
  let finished = finish_stack_branches(&finish, &sync, vec![Some(BuildResult::Built(build))]);
  let updates = finished.updates;

  assert!(!finished.moved_elsewhere, "a conflict is no move of another process");

  assert_eq!(updates.len(), 1);
  assert_eq!(updates[0].state, StackBranchState::Conflict);
  assert_eq!(
    updates[0].conflict,
    Some(StackBranchConflict {
      commit: changed,
      files: vec!["config.txt".to_string()],
    })
  );
  assert_eq!(updates[0].tip.as_deref(), Some(base.as_str()));
  assert_eq!(repo.rev_parse("test/stack/auth").unwrap(), base);
}

/// The move is a compare-and-swap on the tip the sync listed: a ref another process moved meanwhile keeps that tip,
/// and the stack branch is stale, saying so
#[test]
fn another_process_moving_the_stack_branch_keeps_its_tip_stale() {
  let repo = TestRepo::new();
  let listed = repo.create_commit("Initial commit", "README.md", "# Test");
  let moved = repo.create_commit("Moved by another process", "other.txt", "other");
  let built = repo.create_commit("Built by this sync", "built.txt", "built");
  repo.create_branch_at("test/stack/auth", &moved).unwrap();
  let git = GitCommandExecutor::new();
  let sync = StackBranchSync {
    plans: vec![StackBranchPlan {
      root: "auth".to_string(),
      full_name: "test/stack/auth".to_string(),
      members: vec!["auth".to_string()],
      commits: Vec::new(),
      my_email: None,
      blocked_by: None,
    }],
    tips: HashMap::from([("test/stack/auth".to_string(), listed)]),
    ..StackBranchSync::default()
  };
  let finish = StackFinish {
    git_executor: &git,
    repository_path: repo.path_str(),
    remote_statuses: None,
    failed: &HashMap::new(),
  };

  let finished = finish_stack_branches(&finish, &sync, vec![Some(BuildResult::Built(StackBuild::Built { tip: built }))]);
  let updates = finished.updates;

  assert!(finished.moved_elsewhere, "the sync asks for one more run (`SyncReport::retry`)");

  assert_eq!((updates[0].state, updates[0].tip.as_deref()), (StackBranchState::Stale, Some(moved.as_str())));
  let error = updates[0].error.clone().unwrap();
  assert!(error.starts_with("Not updated: another process moved test/stack/auth to "), "{error}");
  assert_eq!(repo.rev_parse("test/stack/auth").unwrap(), moved);
}
