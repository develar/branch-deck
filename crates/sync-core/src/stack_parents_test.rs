use crate::commit_grouper::{CommitGrouper, GroupingInputs};
use crate::grouping::group_commits_ahead_of_baseline;
use crate::regroup::regroup_test::{commits, entries, groups, issue_group_plan, other_worktree, path, plan, plant_store, regroup, repo, sync};
use crate::regroup::{RegroupError, RegroupOptions, RegroupOutcome, RegroupPlan, UndoToken, regroup_commits_core, rename_branch_core, restack_branch_core, undo_regroup_core};
use crate::repository_lock::DEFAULT_LOCK_TIMEOUT;
use crate::snapshot::read_grouping_inputs;
use crate::stack::{StackForest, describe_problem, place_branches};
use crate::stack_parents::{STACK_PARENTS, STACK_PARENTS_REF, drop_integrated_stack_parents, integrated_children};
use crate::store_lines::{StoreLines, format_store_lines, read_store_head};
use crate::test_commit::commit;
use git_ops::revs::short_id;
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashMap};
use sync_types::stack::{DeclaredParent, ParentSource, ParentState, StackProblem};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn stack_parents(repo: &TestRepo) -> StoreLines {
  read_grouping_inputs(repo.git_executor(), path(repo)).unwrap().stack_parents
}

/// The commits of the store: one per write
fn history(repo: &TestRepo) -> usize {
  repo.log(&["--format=%H", STACK_PARENTS_REF]).unwrap_or_default().lines().count()
}

/// Write the store directly, as another tool would
fn plant(repo: &TestRepo, text: &str) {
  plant_store(repo, &STACK_PARENTS, text);
}

/// Add `lines` to the store, as a write of another worktree would
fn plant_after(repo: &TestRepo, lines: &str) {
  let mut text = format_store_lines(&stack_parents(repo).entries, &[]);
  text.push_str(lines);
  plant(repo, &text);
}

/// The forest of `subjects` grouped with the stack parents `stored`, and the idle children
fn forest(subjects: &[&str], stored: &[(&str, &str)]) -> (StackForest, Vec<String>) {
  let inputs = GroupingInputs {
    stacks: true,
    stack_parents: StoreLines {
      entries: entries(stored),
      ..StoreLines::default()
    },
    ..GroupingInputs::default()
  };
  let mut grouper = CommitGrouper::new_with(&inputs);
  for (index, subject) in subjects.iter().enumerate() {
    grouper.add_commit(commit(&format!("{index:040}"), subject));
  }
  let (grouped, forest) = grouper.finish_with_stacks();
  (forest.unwrap(), grouped.idle_stack_parents)
}

fn restack(repo: &TestRepo, leaf: &str, parent: Option<&str>) -> Result<RegroupOutcome, RegroupError> {
  restack_branch_core(repo.git_executor(), path(repo), leaf, parent, &RegroupOptions::default())
}

fn unstack_plan(branches: &[&str]) -> RegroupPlan {
  RegroupPlan {
    unstack: branches.iter().map(|branch| (*branch).to_string()).collect(),
    ..RegroupPlan::default()
  }
}

/// Subjects ahead of the baseline, oldest first
fn subjects(repo: &TestRepo, count: usize) -> Vec<String> {
  let mut messages = repo.get_commit_messages(count);
  messages.reverse();
  messages
}

#[test]
fn reads_nothing_without_the_file_and_the_written_commit_after_the_transaction() {
  let repo = repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  assert_eq!(stack_parents(&repo), StoreLines::default());

  let outcome = regroup(&repo, &plan(&[("auth/login", &[&ids[1]])], &[])).unwrap();
  let written = outcome.stack_parents.unwrap().new_commit.unwrap();
  let read = stack_parents(&repo);
  assert_eq!((read.commit.as_deref(), read.entries), (Some(written.as_str()), entries(&[("login", "auth")])));
  // no temporary ref is left
  assert_eq!(repo.git_executor().execute_command(&["for-each-ref", "refs/branch-deck/tmp/"], path(&repo)).unwrap(), "");

  // a commit of the ref without the file is an empty map
  let git = repo.git_executor();
  let tree = git.cmd(&["mktree"], path(&repo)).stdin(b"").text().unwrap();
  let empty = git.execute_command(&["commit-tree", tree.trim(), "-m", "empty"], path(&repo)).unwrap();
  git.execute_command(&["update-ref", STACK_PARENTS_REF, empty.trim()], path(&repo)).unwrap();
  assert_eq!(stack_parents(&repo).entries, BTreeMap::new());
}

#[test]
fn the_stack_setting_and_both_maps_cost_two_git_processes() {
  let repo = repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "ABC-1 Add login", "XYZ-1 Add other"]);
  regroup(&repo, &issue_group_plan("other", &["XYZ-1"])).unwrap();
  regroup(&repo, &plan(&[("auth/ABC-1", &[&ids[1]])], &[])).unwrap();
  let trace_dir = tempfile::tempdir().unwrap();
  let trace = trace_dir.path().join("trace.json");
  // the trace of this test's git only: the process environment is shared with the tests that run beside it
  let git = repo.git_executor().with_env("GIT_TRACE2_EVENT", trace.to_str().unwrap());
  let inputs = read_grouping_inputs(&git, path(&repo)).unwrap();
  let started: Vec<String> = std::fs::read_to_string(&trace)
    .unwrap()
    .lines()
    .filter(|line| line.contains("\"event\":\"start\""))
    .map(str::to_string)
    .collect();
  assert_eq!(started.len(), 2, "{started:#?}");
  assert!(started.iter().any(|line| line.contains("\"cat-file\",\"--batch\"")), "{started:#?}");
  assert_eq!(inputs.issue_groups.entries, entries(&[("XYZ-1", "other")]));
  assert_eq!(inputs.stack_parents.entries, entries(&[("ABC-1", "auth")]));
}

#[test]
fn bad_lines_are_problems_and_never_used() {
  let text = "login\tauth\nself\tself\nslash/ed\tauth\noauth\tlog in\nnot-a-line\ntwice\ta\ntwice\tb\n\nui\tlogin\n";
  let (valid, problems) = STACK_PARENTS.parse(text);
  assert_eq!(valid, entries(&[("login", "auth"), ("ui", "login")]));
  let lines: Vec<&str> = problems.iter().map(|problem| problem.line.as_str()).collect();
  assert_eq!(lines, ["self\tself", "slash/ed\tauth", "oauth\tlog in", "not-a-line", "twice\ta", "twice\tb"]);
  assert!(problems.iter().all(|problem| !problem.message.is_empty()));

  // a write keeps the unused lines, except those of a child it sets or removes
  let repo = repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "(login) Add login", "(twice) Two"]);
  plant(&repo, text);
  regroup(&repo, &plan(&[("auth/twice", &[&ids[2]])], &[])).unwrap();
  let after = stack_parents(&repo);
  assert_eq!(after.entries, entries(&[("login", "auth"), ("twice", "auth"), ("ui", "login")]));
  let kept: Vec<&str> = after.problems.iter().map(|problem| problem.line.as_str()).collect();
  assert_eq!(kept, ["not-a-line", "oauth\tlog in", "self\tself", "slash/ed\tauth"]);
}

#[test]
fn a_write_on_one_worktree_keeps_the_parents_of_another() {
  let repo = repo(true);
  let base = repo.head();
  commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  let (_other_dir, other_path) = other_worktree(&repo, &base, &["(core) Add core", "(ui) Add ui"]);
  let other_path = other_path.as_str();
  let git = repo.git_executor();
  restack_branch_core(git, other_path, "ui", Some("core"), &RegroupOptions::default()).unwrap();

  let outcome = restack(&repo, "login", Some("auth")).unwrap();
  assert_eq!(outcome.stack_parents.unwrap().entries, entries(&[("login", "auth"), ("ui", "core")]));
  let here = group_commits_ahead_of_baseline(git, path(&repo), "test-prefix").unwrap();
  assert_eq!(here.idle_stack_parents, ["ui"]);
  let there = group_commits_ahead_of_baseline(git, other_path, "test-prefix").unwrap();
  assert_eq!(there.idle_stack_parents, ["login"]);
  assert_eq!(there.forest.unwrap().declared_parent("ui"), Some("core"));

  // a rename renames a parent only for the children with commits on its HEAD
  rename_branch_core(git, other_path, "core", "base", &RegroupOptions::default()).unwrap();
  rename_branch_core(git, path(&repo), "auth", "identity", &RegroupOptions::default()).unwrap();
  assert_eq!(stack_parents(&repo).entries, entries(&[("login", "identity"), ("ui", "base")]));
}

#[test]
fn without_stacks_the_map_is_not_used_and_not_written() {
  let repo = repo(false);
  let ids = commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  plant(&repo, "login\tauth\n");
  let grouping = group_commits_ahead_of_baseline(repo.git_executor(), path(&repo), "test-prefix").unwrap();
  assert!(grouping.forest.is_none());
  assert_eq!(grouping.idle_stack_parents, Vec::<String>::new());
  assert_eq!(grouping.grouping.stack_parents.entries, entries(&[("login", "auth")]));

  // a slash is part of a flat name, so the path writes no parent
  let outcome = regroup(&repo, &plan(&[("auth/login", &[&ids[1]])], &[])).unwrap();
  assert_eq!((outcome.stack_parents.is_none(), groups(&outcome)[1]), (true, Some("auth/login")));
  for error in [restack(&repo, "auth", Some("login")).unwrap_err(), regroup(&repo, &unstack_plan(&["login"])).unwrap_err()] {
    assert!(
      matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("branchdeck.stacks")),
      "{error}"
    );
  }
  assert_eq!(history(&repo), 1);
}

#[test]
fn a_stored_parent_stacks_a_prefix_branch_an_issue_branch_and_an_issue_group() {
  let (forest, idle) = forest(
    &["(auth) Add auth", "(login) Add login", "ABC-1 Fix login", "(auth/oauth) Add oauth"],
    &[("login", "auth"), ("ABC-1", "login"), ("oauth", "auth"), ("ghost", "auth")],
  );
  let parent = |name: &str| (forest.declared_parent(name), forest.nodes[name].parent_source);
  assert_eq!(parent("auth"), (None, None));
  assert_eq!(parent("login"), (Some("auth"), Some(ParentSource::Store)));
  assert_eq!(parent("ABC-1"), (Some("login"), Some(ParentSource::Store)));
  // the prefix and the store agree: the prefix declares it
  assert_eq!(parent("oauth"), (Some("auth"), Some(ParentSource::Prefix)));
  assert_eq!(forest.order, ["auth", "login", "ABC-1", "oauth"]);
  assert_eq!(idle, ["ghost"]);
  assert!(!forest.nodes.contains_key("ghost"));

  // an issue group is stacked by its name
  let repo = repo(true);
  commits(&repo, &["(auth) Add auth", "ABC-1 One", "ABC-2 Two"]);
  regroup(&repo, &issue_group_plan("parser", &["ABC-1", "ABC-2"])).unwrap();
  let outcome = restack(&repo, "parser", Some("auth")).unwrap();
  assert_eq!(outcome.changed_count(), 0);
  let grouping = group_commits_ahead_of_baseline(repo.git_executor(), path(&repo), "test-prefix").unwrap();
  let placements = grouping.placements.unwrap();
  assert_eq!(placements["parser"].effective_base.as_deref(), Some("auth"));
  assert_eq!(placements["parser"].parent_source, Some(ParentSource::Store));
}

#[test]
fn a_stored_parent_that_differs_from_a_prefix_is_a_conflict() {
  let (forest, _) = forest(&["(auth) Add auth", "(docs) Add docs", "(auth/login) Add login"], &[("login", "docs")]);
  let login = &forest.nodes["login"];
  assert_eq!(login.declared_parent, None);
  assert_eq!(
    login.problems,
    [StackProblem::ConflictingParents(vec![
      DeclaredParent {
        parent: "auth".to_string(),
        commits: vec![format!("{:040}", 2)],
      },
      DeclaredParent {
        parent: "docs".to_string(),
        commits: Vec::new(),
      },
    ])]
  );
  let message = describe_problem("login", &login.problems[0]);
  assert_eq!(
    message,
    format!(
      "Commits declare different parents: auth ({}), docs (the stack parent in refs/branch-deck/stack-parents); keep one: `bd restack login auth` or \
       `bd restack login docs`",
      short_id(&format!("{:040}", 2))
    )
  );
}

#[test]
fn a_cycle_through_the_map_marks_each_of_its_branches() {
  let (cycle, _) = forest(&["(a) A", "(b) B", "(a/c) C"], &[("a", "b"), ("b", "c")]);
  for name in ["a", "b", "c"] {
    assert!(cycle.nodes[name].problems.iter().any(|problem| matches!(problem, StackProblem::Cycle(_))), "{name}");
  }
  // also through an ancestor without commits
  let (cycle, _) = forest(&["(a) A"], &[("a", "ghost"), ("ghost", "a")]);
  assert!(cycle.nodes["a"].problems.iter().any(|problem| matches!(problem, StackProblem::Cycle(_))));

  let repo = repo(true);
  commits(&repo, &["(a) A", "(b) B"]);
  restack(&repo, "a", Some("b")).unwrap();
  let error = restack(&repo, "b", Some("a")).unwrap_err();
  assert!(matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("cycle")), "{error}");
}

#[test]
fn a_stored_parent_without_commits_is_passed() {
  // login <- auth <- core, and auth has no commits: login is built on core
  let (chain, idle) = forest(&["(core) Add core", "(login) Add login"], &[("login", "auth"), ("auth", "core")]);
  assert_eq!(idle, Vec::<String>::new());
  assert!(!chain.nodes["auth"].has_commits);
  let placed = place_branches(&chain, &HashMap::new());
  assert_eq!(placed["login"].declared_parent.as_deref(), Some("auth"));
  assert_eq!(placed["login"].parent_state, Some(ParentState::Missing));
  assert_eq!((placed["login"].effective_base.as_deref(), placed["login"].pr_base_changed), (Some("core"), true));

  // a prefix ancestor without commits takes its stack parent too
  let (chain, _) = forest(&["(core) Add core", "(auth/login) Add login"], &[("auth", "core")]);
  let placed = place_branches(&chain, &HashMap::new());
  assert_eq!(placed["login"].effective_base.as_deref(), Some("core"));
}

#[test]
fn a_sync_reads_the_map_and_never_writes_a_line_it_can_not_prove_dead() {
  let repo = repo(true);
  commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  restack(&repo, "login", Some("auth")).unwrap();
  plant_after(&repo, "ghost\tauth\n");
  let before = (stack_parents(&repo), history(&repo));
  sync(&repo);
  assert!(repo.branch_exists("test-prefix/virtual/login"));
  // a child without an integrated archive stays: it may stack a branch of another worktree
  assert_eq!((stack_parents(&repo), history(&repo)), before);
}

#[test]
fn the_newest_archive_of_a_child_decides_whether_it_is_integrated() {
  let children = ["login".to_string(), "ui".to_string(), "ghost".to_string()];
  let archived = [
    ("p/archived/2026-10-01/login".to_string(), true),
    // a newer archive that is not integrated, e.g. the branch of another worktree that this sync archived
    ("p/archived/2026-10-02/ui".to_string(), true),
    ("p/archived/2026-10-02/ui-1".to_string(), false),
    ("p/archived/2026-10-02/login-ish".to_string(), false),
  ];
  assert_eq!(integrated_children(&children, &archived, "p"), ["login"]);
  // without an archive `ghost` that day, `ghost-2` is another branch, not a second archive of `ghost`
  let other = [("p/archived/2026-10-03/ghost-2".to_string(), true)];
  assert_eq!(integrated_children(&children, &other, "p"), Vec::<&str>::new());
  let second = [("p/archived/2026-10-03/ghost".to_string(), false), ("p/archived/2026-10-03/ghost-2".to_string(), true)];
  assert_eq!(integrated_children(&children, &second, "p"), ["ghost"]);

  // a drop writes only when a line goes, and keeps every other line
  let repo = repo(true);
  plant(&repo, "login\tauth\nui\tlogin\nbad line\n");
  drop_integrated_stack_parents(repo.git_executor(), path(&repo), &["ghost"]);
  assert_eq!(history(&repo), 1);
  drop_integrated_stack_parents(repo.git_executor(), path(&repo), &["login"]);
  let after = stack_parents(&repo);
  assert_eq!((after.entries, after.problems.len(), history(&repo)), (entries(&[("ui", "login")]), 1, 2));
}

#[test]
fn a_plan_path_writes_the_parent_and_a_bare_leaf_prefix() {
  let repo = repo(true);
  let ids = commits(&repo, &["(a) A", "Add b", "Add c"]);
  let outcome = regroup(&repo, &plan(&[("a/b/c", &[&ids[2]]), ("a/b", &[&ids[1]])], &[])).unwrap();
  assert_eq!(subjects(&repo, 3), ["(a) A", "(b) Add b", "(c) Add c"]);
  assert_eq!(outcome.stack_parents.unwrap().entries, entries(&[("b", "a"), ("c", "b")]));
  let forest = outcome.forest.unwrap();
  assert_eq!((forest.declared_parent("b"), forest.declared_parent("c")), (Some("a"), Some("b")));
}

#[test]
fn stacking_whole_issue_branches_rewrites_nothing() {
  let repo = repo(true);
  let ids = commits(&repo, &["IJAI-3992 Add the model", "IJAI-4047 Use the model", "IJAI-4047 Tune it"]);
  let head = repo.head();
  let reflog = || repo.log(&["-g", "--format=%gs", "master"]).unwrap().lines().count();
  let entries_before = reflog();
  let options = RegroupOptions {
    expect_head: Some(head[..12].to_string()),
    ..RegroupOptions::default()
  };
  let stack = plan(&[("IJAI-3992/IJAI-4047", &[&ids[1], &ids[2]])], &[]);
  let outcome = regroup_commits_core(repo.git_executor(), path(&repo), &stack, &options).unwrap();
  assert_eq!((outcome.changed_count(), repo.head()), (0, head));
  assert_eq!(reflog(), entries_before, "HEAD did not move");
  assert_eq!(outcome.forest.as_ref().unwrap().declared_parent("IJAI-4047"), Some("IJAI-3992"));
  let token = UndoToken::of(&outcome).unwrap();
  assert_eq!((token.head.is_none(), token.issue_groups.is_none(), token.stack_parents.is_some()), (true, true, true));

  // an expected HEAD that moved refuses a write of the store alone too
  repo.create_commit("Later", "later.txt", "later");
  let error = regroup_commits_core(repo.git_executor(), path(&repo), &unstack_plan(&["IJAI-4047"]), &options).unwrap_err();
  assert!(matches!(error, RegroupError::HeadMoved(_)), "{error}");
}

#[test]
fn a_named_commit_that_gets_the_bare_leaf_keeps_the_levels_above_as_stack_parents() {
  let repo = repo(true);
  let ids = commits(&repo, &["(core) Add core", "(core/x/login) Add z"]);
  regroup(&repo, &plan(&[("auth/login", &[&ids[1]])], &[])).unwrap();
  assert_eq!(subjects(&repo, 1), ["(login) Add z"]);
  // x stays on core: the plan restacks only login
  assert_eq!(stack_parents(&repo).entries, entries(&[("login", "auth"), ("x", "core")]));
}

#[test]
fn renaming_a_level_without_commits_renames_its_stack_parent() {
  let repo = repo(true);
  commits(&repo, &["(core) Add core", "(auth/login) Add form"]);
  plant(&repo, "auth\tcore\n");
  rename_branch_core(repo.git_executor(), path(&repo), "auth", "identity", &RegroupOptions::default()).unwrap();
  assert_eq!(subjects(&repo, 1), ["(identity/login) Add form"]);
  assert_eq!(stack_parents(&repo).entries, entries(&[("identity", "core")]));
}

#[test]
fn a_differing_parent_level_becomes_a_bare_leaf() {
  let repo = repo(true);
  commits(
    &repo,
    &[
      "(auth) Add auth",
      "(docs) Add docs",
      "(docs/login) Add form",
      "(auth/login) Add button",
      "(core/x/login) Add z",
    ],
  );
  regroup(&repo, &plan(&[("auth/login", &[])], &[])).unwrap();
  // the same parent stays; another one loses its levels, and the levels above it become stack parents
  assert_eq!(
    subjects(&repo, 5)[2..],
    ["(login) Add form", "(auth/login) Add button", "(login) Add z"].map(str::to_string)
  );
  assert_eq!(stack_parents(&repo).entries, entries(&[("login", "auth"), ("x", "core")]));

  // a deeper path of another branch that declares another parent for the child loses the levels above the child
  let deeper = crate::regroup::regroup_test::repo(true);
  commits(&deeper, &["(auth) Add auth", "(x/login/oauth) Add oauth", "(login) Add login"]);
  regroup(&deeper, &plan(&[("auth/login", &[])], &[])).unwrap();
  assert_eq!(subjects(&deeper, 3)[1..], ["(login/oauth) Add oauth", "(login) Add login"].map(str::to_string));
}

#[test]
fn a_mixed_plan_moves_head_and_the_store_in_one_transaction() {
  let repo = repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "Add login", "ABC-1 One"]);
  let head = repo.head();
  let mixed = RegroupPlan {
    issue_groups: BTreeMap::from([("parser".to_string(), vec!["ABC-1".to_string()])]),
    ..plan(&[("auth/login", &[&ids[1]]), ("login/parser", &[])], &[])
  };
  let outcome = regroup(&repo, &mixed).unwrap();
  let token = UndoToken::of(&outcome).unwrap();
  let text = token.to_string();
  assert!(text.contains("+groups:") && text.contains("+stacks:"), "{text}");
  assert_eq!(UndoToken::parse(&text), Some(token.clone()));
  assert_eq!(stack_parents(&repo).entries, entries(&[("login", "auth"), ("parser", "login")]));
  // one ref transaction for the branch and both stores
  let reflog: Vec<String> = repo.log(&["-g", "--format=%gs", "master"]).unwrap().lines().map(str::to_string).collect();
  assert!(reflog[0].starts_with("branch-deck: reword"), "{reflog:?}");

  undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap();
  assert_eq!(repo.head(), head);
  assert_eq!(read_store_head(repo.git_executor(), path(&repo), STACK_PARENTS_REF).unwrap(), None);
  assert_eq!(read_grouping_inputs(repo.git_executor(), path(&repo)).unwrap().issue_groups.commit, None);
}

#[test]
fn invalid_stack_parent_plans_are_refused() {
  let repo = repo(true);
  let ids = commits(&repo, &["(a) A", "(b) B", "(c) C"]);
  let invalid = |plan: &RegroupPlan, text: &str| {
    let error = regroup(&repo, plan).unwrap_err();
    assert!(matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains(text)), "{text}: {error}");
  };
  invalid(&plan(&[("b/b", &[&ids[1]])], &[]), "own parent");
  invalid(&plan(&[("a/c", &[&ids[2]]), ("b/c", &[])], &[]), "two parents");
  invalid(&plan(&[("b/a", &[]), ("a/b", &[])], &[]), "cycle");
  invalid(
    &RegroupPlan {
      unstack: vec!["c".to_string()],
      ..plan(&[("a/c", &[])], &[])
    },
    "both stacked and unstacked",
  );
  invalid(&unstack_plan(&["c"]), "not stacked");
  invalid(&unstack_plan(&["a/c"]), "without `/`");
  assert_eq!(history(&repo), 0);
}

#[test]
fn an_issue_group_is_restacked_and_unstacked_without_a_rewrite() {
  let repo = repo(true);
  commits(&repo, &["(auth) Add auth", "ABC-1 One", "ABC-2 Two", "XYZ-1 Three"]);
  regroup(&repo, &issue_group_plan("parser", &["ABC-1", "ABC-2"])).unwrap();
  let head = repo.head();
  for parent in [Some("auth"), None] {
    let outcome = restack(&repo, "parser", parent).unwrap();
    assert_eq!((outcome.changed_count(), repo.head()), (0, head.clone()));
    assert_eq!(outcome.forest.unwrap().declared_parent("parser"), parent);
  }
  // and so is an issue branch
  restack(&repo, "XYZ-1", Some("parser")).unwrap();
  assert_eq!(repo.head(), head);

  // an idle line is unstacked although no commit is in the branch here
  plant_after(&repo, "ghost\tauth\n");
  regroup(&repo, &unstack_plan(&["ghost"])).unwrap();
  assert_eq!(stack_parents(&repo).entries, entries(&[("XYZ-1", "parser")]));
}

#[test]
fn an_undo_passes_over_the_drops_of_a_sync() {
  let repo = repo(true);
  commits(&repo, &["(auth) Add auth", "(login) Add login", "(docs) Write"]);
  plant(&repo, "ghost\tauth\n");
  let token = UndoToken::of(&restack(&repo, "login", Some("auth")).unwrap()).unwrap();
  // a sync drops the line of an integrated branch meanwhile: the token's commit is no longer the store
  drop_integrated_stack_parents(repo.git_executor(), path(&repo), &["ghost"]);
  assert_eq!(stack_parents(&repo).entries, entries(&[("login", "auth")]));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap();
  // the restack is undone, the drop stays
  assert_eq!(stack_parents(&repo).entries, BTreeMap::new());
  assert_eq!(history(&repo), 4);

  // a write of a regroup after the token is no drop: the undo refuses
  let token = UndoToken::of(&restack(&repo, "login", Some("auth")).unwrap()).unwrap();
  restack(&repo, "docs", Some("auth")).unwrap();
  let error = undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap_err();
  assert!(matches!(error, RegroupError::StacksMoved(_)), "{error}");
}
