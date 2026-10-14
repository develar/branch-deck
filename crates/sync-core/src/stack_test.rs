use crate::commit_grouper::CommitGrouper;
use crate::stack::{StackForest, empty_parent_states, place_branches};
use branch_integration::cache::CacheOps;
use git_ops::commit_list::Commit;
use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use sync_types::stack::{BlockReason, BranchStack, ParentState, StackBlock};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn commit(id: &str, subject: &str) -> Commit {
  Commit {
    id: id.to_string(),
    stripped_subject: subject.to_string(),
    message: subject.to_string(),
    author_name: "Test Author".to_string(),
    author_email: "test@example.com".to_string(),
    author_timestamp: 1_234_567_890,
    committer_timestamp: 1_234_567_890,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject: subject.to_string(),
    parent_id: None,
    tree_id: "tree".to_string(),
    note: None,
    mapped_commit_id: None,
  }
}

fn forest(subjects: &[&str]) -> StackForest {
  let mut grouper = CommitGrouper::new_with_stacks(true);
  for (index, subject) in subjects.iter().enumerate() {
    grouper.add_commit(commit(&format!("c{index}"), subject));
  }
  grouper.finish_with_stacks().1.expect("stacks are on")
}

fn place(subjects: &[&str], empty_parents: &[(&str, ParentState)]) -> IndexMap<String, BranchStack> {
  let states: HashMap<String, ParentState> = empty_parents.iter().map(|(name, state)| ((*name).to_string(), *state)).collect();
  place_branches(&forest(subjects), &states)
}

fn blocked(parent: &str, reason: BlockReason) -> StackBlock {
  StackBlock {
    parent: parent.to_string(),
    reason,
  }
}

#[test]
fn a_child_is_built_on_its_active_parent() {
  let placed = place(&["(auth) Add auth", "(auth/login) Add form", "(login/oauth) Add OAuth", "(docs) Document"], &[]);

  assert_eq!(placed.keys().collect::<Vec<_>>(), ["auth", "login", "oauth", "docs"]);
  assert_eq!(placed["auth"], BranchStack::default());
  assert_eq!(placed["login"].effective_base.as_deref(), Some("auth"));
  assert_eq!(placed["login"].parent_state, Some(ParentState::Active));
  assert_eq!(placed["oauth"].effective_base.as_deref(), Some("login"));
  assert_eq!(placed["oauth"].blocked_by, None);
  assert_eq!(placed["docs"].effective_base, None);
}

#[test]
fn a_missing_or_unapplied_parent_blocks_the_child_and_its_descendants() {
  let placed = place(
    &["(auth/login) Add form", "(login/oauth) Add OAuth", "(api/client) Add client"],
    &[("api", ParentState::Unapplied)],
  );

  assert_eq!(placed["login"].parent_state, Some(ParentState::Missing));
  assert_eq!(placed["login"].blocked_by, Some(blocked("auth", BlockReason::Missing)));
  assert_eq!(placed["login"].effective_base, None);
  assert_eq!(placed["oauth"].blocked_by, Some(blocked("login", BlockReason::Blocked)));
  assert_eq!(placed["client"].parent_state, Some(ParentState::Unapplied));
  assert_eq!(placed["client"].blocked_by, Some(blocked("api", BlockReason::Unapplied)));
}

#[test]
fn an_integrated_parent_moves_the_child_to_the_nearest_active_ancestor() {
  let placed = place(
    &["(root) Add root", "(root/auth/login) Add form", "(auth/session) Add session"],
    &[("auth", ParentState::Integrated)],
  );

  assert_eq!(placed["login"].parent_state, Some(ParentState::Integrated));
  assert_eq!(placed["login"].effective_base.as_deref(), Some("root"));
  assert!(placed["login"].pr_base_changed);
  assert_eq!(placed["login"].blocked_by, None);

  // without an active ancestor, onto the baseline
  let placed = place(&["(auth/login) Add form"], &[("auth", ParentState::Integrated)]);
  assert_eq!(placed["login"].effective_base, None);
  assert!(placed["login"].pr_base_changed);
  assert_eq!(placed["login"].blocked_by, None);
}

#[test]
fn a_parent_with_a_problem_blocks_its_children() {
  let placed = place(&["(a/a) Loop", "(a/b) B", "(x/login) X", "(y/login) Y", "(login/form) Form"], &[]);

  assert_eq!(placed["a"].problems.len(), 1);
  assert_eq!(placed["a"].effective_base, None);
  assert_eq!(placed["b"].blocked_by, Some(blocked("a", BlockReason::Error)));
  // conflicting parents: no declared parent, a problem, and the child is blocked
  assert_eq!(placed["login"].declared_parent, None);
  assert_eq!(placed["login"].problems.len(), 1);
  assert_eq!(placed["form"].blocked_by, Some(blocked("login", BlockReason::Error)));
}

#[test]
fn reads_the_state_of_parents_without_commits_from_their_branches() {
  let test_repo = TestRepo::new();
  let repo_path = test_repo.path().to_str().unwrap();
  let git_executor = test_repo.git_executor();
  let base = test_repo.create_commit("Initial commit", "README.md", "# Test");
  let cache_tip = test_repo.create_commit("Add cache", "cache.rs", "// cache");
  let docs_tip = test_repo.create_commit("Add docs", "docs.md", "docs");
  test_repo.create_branch_at("test/unapplied/auth-1", &base).unwrap();
  test_repo.create_branch_at("test/archived/2026-01-01/cache", &base).unwrap();
  test_repo.create_branch_at("test/archived/2026-01-02/cache", &cache_tip).unwrap();
  test_repo.create_branch_at("test/archived/2026-01-02/docs", &docs_tip).unwrap();
  let status = |integrated| {
    if integrated {
      BranchIntegrationStatus::Integrated {
        integrated_at: Some(1_700_000_000),
        confidence: IntegrationConfidence::Exact,
        commit_count: 1,
      }
    } else {
      BranchIntegrationStatus::NotIntegrated {
        total_commit_count: 1,
        integrated_count: 0,
        orphaned_count: 1,
        integrated_at: None,
      }
    }
  };
  let info = |name: &str, integrated| BranchIntegrationInfo {
    name: name.to_string(),
    summary: String::new(),
    status: status(integrated),
  };
  let cache = CacheOps::new(git_executor, repo_path);
  // only the newest archive of a name counts
  cache.write(&base, &info("cache", false)).unwrap();
  cache.write(&cache_tip, &info("cache", true)).unwrap();
  cache.write(&docs_tip, &info("docs", false)).unwrap();

  let lines = git_executor
    .execute_command_lines(&["for-each-ref", "--format=%(refname:lstrip=2) %(objectname)", "refs/heads/test/"], repo_path)
    .unwrap();
  let branch_refs: HashMap<String, String> = lines
    .iter()
    .filter_map(|line| line.split_once(' '))
    .map(|(name, tip)| (name.to_string(), tip.to_string()))
    .collect();
  let forest = forest(&["(auth/a) A", "(cache/b) B", "(docs/c) C", "(gone/d) D"]);

  let states = empty_parent_states(git_executor, repo_path, "test", &branch_refs, &forest).unwrap();

  assert_eq!(states.get("auth"), Some(&ParentState::Unapplied));
  assert_eq!(states.get("cache"), Some(&ParentState::Integrated));
  assert_eq!(states.get("docs"), None);
  assert_eq!(states.get("gone"), None);
}
