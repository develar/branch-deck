use crate::commit_grouper::CommitGrouper;
use crate::stack::{PendingDetection, StackForest, describe_block, describe_problem, empty_parent_states, name_suffix, place_branches, problem_views, read_only_detection};
use crate::test_commit::commit;
use branch_integration::archive::ARCHIVE_RETENTION_DAYS;
use branch_integration::detection_notes::{CacheOps, CachedDetection, NOTES_REF, Recheck, deserialize_from_cache};
use branch_integration::strategy::DetectionStrategy;
use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use sync_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus, IntegrationConfidence};
use sync_types::stack::{BlockReason, BranchStack, ParentState, StackBlock, StackFix};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

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
fn a_parent_without_commits_moves_the_child_to_the_nearest_ancestor_with_commits() {
  let placed = place(
    &["(auth/login) Add form", "(login/oauth) Add OAuth", "(api/client) Add client"],
    &[("api", ParentState::Unapplied)],
  );

  assert_eq!(placed["login"].parent_state, Some(ParentState::Missing));
  assert_eq!(placed["login"].effective_base, None);
  assert!(placed["login"].pr_base_changed);
  assert_eq!(placed["login"].blocked_by, None);
  assert_eq!(placed["oauth"].effective_base.as_deref(), Some("login"));
  assert_eq!(placed["oauth"].blocked_by, None);
  assert!(!placed["oauth"].pr_base_changed);
  assert_eq!(placed["client"].parent_state, Some(ParentState::Unapplied));
  assert_eq!(placed["client"].effective_base, None);
  assert!(placed["client"].pr_base_changed);
  assert_eq!(placed["client"].blocked_by, None);
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
  assert_eq!(placed["b"].blocked_by, Some(blocked("a", BlockReason::Problem)));
  // conflicting parents: no declared parent, a problem, and the child is blocked
  assert_eq!(placed["login"].declared_parent, None);
  assert_eq!(placed["login"].problems.len(), 1);
  assert_eq!(placed["form"].blocked_by, Some(blocked("login", BlockReason::Problem)));
  // the child names the problem of its parent
  assert_eq!(
    describe_block(&blocked("login", BlockReason::Problem), placed["login"].problems.first()),
    "Blocked: parent login has a stack problem (conflicting parents)"
  );
}

/// (label, fixes, commits to reword) of the first problem of `branch`
fn first_problem(forest: &StackForest, branch: &str) -> (String, Vec<StackFix>, Vec<String>) {
  let view = problem_views(forest, branch).remove(0);
  (view.label, view.fixes, view.commits)
}

#[test]
fn each_problem_offers_the_fixes_that_remove_it() {
  let stack_on = |parent: &str| StackFix::StackOn(parent.to_string());

  // conflicting parents: either parent or none, except a parent stacked on the branch, which would make a cycle
  let conflict = forest(&["(x/login) X", "(y/login) Y", "(login/y) Back"]);
  assert_eq!(
    first_problem(&conflict, "login"),
    ("conflicting parents".to_string(), vec![stack_on("x"), StackFix::Unstack], Vec::new())
  );
  let view = problem_views(&conflict, "login").remove(0);
  assert_eq!(view.reason, "login has conflicting parents");
  assert_eq!(view.message, "Commits declare different parents: x (c0), y (c1)");
  assert_eq!(
    describe_problem("login", &view),
    "Commits declare different parents: x (c0), y (c1); fix: `bd restack login x` or `bd unstack login`"
  );

  // a cycle: the branch leaves it
  let cycle = forest(&["(b/a) A", "(a/b) B"]);
  assert_eq!(first_problem(&cycle, "a"), ("parent cycle".to_string(), vec![StackFix::Unstack], Vec::new()));

  // `(a/a)` alone: an unstack cuts it to `(a)`; beside a declared parent, an unstack would drop that parent too
  assert_eq!(
    first_problem(&forest(&["(a/a) Loop"]), "a"),
    ("its own parent".to_string(), vec![StackFix::Unstack], Vec::new())
  );
  let with_parent = forest(&["(session/x) X", "(x/x) Loop"]);
  assert_eq!(
    first_problem(&with_parent, "x"),
    ("its own parent".to_string(), vec![StackFix::Reword], vec!["c1".to_string()])
  );
  assert_eq!(
    describe_problem("x", &problem_views(&with_parent, "x").remove(0)),
    "A prefix makes the branch its own parent: c1; fix: `bd reword c1 --subject <subject>`"
  );

  let collision = forest(&["(Login) A", "(login) B"]);
  assert_eq!(first_problem(&collision, "login"), ("case clash".to_string(), vec![StackFix::Rename], Vec::new()));

  let syntax = forest(&["(a//b) X"]);
  assert_eq!(
    first_problem(&syntax, "b"),
    ("empty prefix level".to_string(), vec![StackFix::Reword], vec!["c0".to_string()])
  );
}

/// A repository with its first commit, and that commit
fn repo_with_base() -> (TestRepo, String) {
  let test_repo = TestRepo::new();
  let base = test_repo.create_commit("Initial commit", "README.md", "# Test");
  (test_repo, base)
}

#[test]
fn reads_the_state_of_parents_without_commits_from_their_branches() {
  let (test_repo, base) = repo_with_base();
  let repo_path = test_repo.path().to_str().unwrap();
  let git_executor = test_repo.git_executor();
  let cache_tip = test_repo.create_commit("Add cache", "cache.rs", "// cache");
  let docs_tip = test_repo.create_commit("Add docs", "docs.md", "docs");
  test_repo.create_branch_at("test/unapplied/auth-1", &base).unwrap();
  test_repo.create_branch_at("test/archived/2026-01-01/cache", &base).unwrap();
  test_repo.create_branch_at("test/archived/2026-01-02/cache", &cache_tip).unwrap();
  test_repo.create_branch_at("test/archived/2026-01-02/docs", &docs_tip).unwrap();
  // an old integrated archive of `old`, whose newer branch, still virtual, is not integrated
  test_repo.create_branch_at("test/archived/2026-01-01/old", &cache_tip).unwrap();
  test_repo.create_branch_at("test/virtual/old", &docs_tip).unwrap();
  let status = |integrated| {
    if integrated {
      BranchIntegrationStatus::Integrated {
        integrated_at: Some(1_700_000_000),
        confidence: IntegrationConfidence::Exact,
        commit_count: 1,
        commit: None,
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
  // checked against the current baseline, else the virtual tip of `old` would be detected again
  let head = test_repo.head();
  let info = |tip: &str, name: &str, integrated: bool| CachedDetection {
    info: BranchIntegrationInfo {
      name: name.to_string(),
      summary: String::new(),
      status: status(integrated),
    },
    check: (!integrated).then(|| Recheck {
      baseline: head.clone(),
      orphaned: vec![tip.to_string()],
    }),
  };
  let cache = CacheOps::new(git_executor, repo_path);
  // only the newest archive of a name counts
  cache.write(&base, &info(&base, "cache", false)).unwrap();
  cache.write(&cache_tip, &info(&cache_tip, "cache", true)).unwrap();
  cache.write(&docs_tip, &info(&docs_tip, "docs", false)).unwrap();

  let lines = git_executor
    .execute_command_lines(&["for-each-ref", "--format=%(refname:lstrip=2) %(objectname)", "refs/heads/test/"], repo_path)
    .unwrap();
  let branch_refs: HashMap<String, String> = lines
    .iter()
    .filter_map(|line| line.split_once(' '))
    .map(|(name, tip)| (name.to_string(), tip.to_string()))
    .collect();
  let forest = forest(&["(auth/a) A", "(cache/b) B", "(docs/c) C", "(gone/d) D", "(old/e) E"]);

  let states = empty_parent_states(git_executor, repo_path, "test", &branch_refs, &forest, read_only_detection("HEAD")).unwrap();

  assert_eq!(states.get("auth"), Some(&ParentState::Unapplied));
  assert_eq!(states.get("cache"), Some(&ParentState::Integrated));
  assert_eq!(states.get("docs"), None);
  assert_eq!(states.get("gone"), None);
  assert_eq!(states.get("old"), None, "the virtual branch is newer than any archive");
}

#[test]
fn a_parent_whose_virtual_branch_lost_its_commits_is_detected_before_the_placement() {
  let (test_repo, base) = repo_with_base();
  let repo_path = test_repo.path().to_str().unwrap();
  let git_executor = test_repo.git_executor();
  // the baseline has the change of `auth` in another commit, not the change of `cache`
  let baseline = test_repo.create_commit("Add auth upstream", "auth.rs", "// auth");
  test_repo.create_branch_at("baseline", &baseline).unwrap();
  test_repo.reset_hard(&base).unwrap();
  let integrated = test_repo.create_commit("Add auth", "auth.rs", "// auth");
  test_repo.create_branch_at("test/virtual/auth", &integrated).unwrap();
  test_repo.reset_hard(&base).unwrap();
  let local = test_repo.create_commit("Add cache", "cache.rs", "// cache");
  test_repo.create_branch_at("test/virtual/cache", &local).unwrap();
  let branch_refs = HashMap::from([("test/virtual/auth".to_string(), integrated.clone()), ("test/virtual/cache".to_string(), local.clone())]);
  let forest = forest(&["(auth/a) A", "(cache/b) B"]);
  let notes = || test_repo.list_notes_with_ref(NOTES_REF).unwrap_or_default();

  let detect = |cache| PendingDetection {
    baseline_commit: None,
    baseline_branch: "baseline",
    strategy: DetectionStrategy::Rebase,
    cache,
    retention_days: ARCHIVE_RETENTION_DAYS,
  };
  let states = empty_parent_states(git_executor, repo_path, "test", &branch_refs, &forest, detect(false)).unwrap();
  assert_eq!((states.get("auth"), states.get("cache")), (Some(&ParentState::Integrated), None));
  assert!(notes().is_empty(), "a read-only command writes no note");

  let states = empty_parent_states(git_executor, repo_path, "test", &branch_refs, &forest, detect(true)).unwrap();
  assert_eq!((states.get("auth"), states.get("cache")), (Some(&ParentState::Integrated), None));
  let notes = notes();
  assert!(notes.iter().any(|line| line.ends_with(&integrated)), "{notes:?}");
  assert!(notes.iter().any(|line| line.ends_with(&local)), "a root branch caches any result");
  // a `NotIntegrated` note records the baseline commit it was checked against
  let note = git_executor.execute_command(&["notes", &format!("--ref={NOTES_REF}"), "show", &local], repo_path).unwrap();
  let cached = deserialize_from_cache(note.trim()).unwrap();
  assert_eq!(cached.check.map(|check| check.baseline), Some(baseline));
}

#[test]
fn a_name_suffix_orders_the_archives_of_one_day_numerically() {
  assert_eq!(name_suffix("auth", "auth"), Some(0));
  assert_eq!(name_suffix("auth-9", "auth"), Some(9));
  assert_eq!(name_suffix("auth-10", "auth"), Some(10));
  assert_eq!(name_suffix("auth-x", "auth"), None);
  assert_eq!(name_suffix("auth-", "auth"), None);
  assert_eq!(name_suffix("authz", "auth"), None);
}
