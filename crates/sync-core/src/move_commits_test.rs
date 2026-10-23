use crate::regroup_test::{commits, path, regroup};
use crate::stack_operations::{UndoRegroupParams, undo_regroup};
use domain_types::{BranchOrigin, GroupedBranchInfo, SyncEvent};
use history_rewrite::issue_groups::ISSUE_GROUPS;
use history_rewrite::move_commits::{MoveChange, MoveCommitsParams, MoveCommitsResult, move_commits_to_branch_core, validate_branch_name, with_branch_prefix};
use history_rewrite::regroup::RegroupError;
use history_rewrite::test_commit::issue_group_plan;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

fn params(test_repo: &TestRepo, commit_ids: &[&str], target_branch: Option<&str>) -> MoveCommitsParams {
  MoveCommitsParams::new(path(test_repo), commit_ids.iter().map(|id| (*id).to_string()).collect(), target_branch)
}

fn move_to(test_repo: &TestRepo, commit_ids: &[&str], target_branch: Option<&str>) -> anyhow::Result<MoveCommitsResult> {
  move_commits_to_branch_core(test_repo.git_executor(), &params(test_repo, commit_ids, target_branch))
}

/// `move_to` with the ids of `commits`
fn move_ids(test_repo: &TestRepo, commits: &[&String], target_branch: Option<&str>) -> MoveCommitsResult {
  let ids: Vec<&str> = commits.iter().map(|id| id.as_str()).collect();
  move_to(test_repo, &ids, target_branch).unwrap()
}

fn changes(result: &MoveCommitsResult) -> Vec<MoveChange> {
  result.commits.iter().map(|commit| commit.change).collect()
}

/// Issue key -> branch name of the store
fn issue_groups(test_repo: &TestRepo) -> BTreeMap<String, String> {
  ISSUE_GROUPS.read(test_repo.git_executor(), path(test_repo)).unwrap().entries
}

fn pairs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
  pairs.iter().map(|(key, name)| ((*key).to_string(), (*name).to_string())).collect()
}

fn undo(test_repo: &TestRepo, result: &MoveCommitsResult) {
  let params = UndoRegroupParams {
    repository_path: path(test_repo).to_string(),
    token: result.undo_token.clone().unwrap(),
  };
  undo_regroup(test_repo.git_executor(), &params).unwrap();
}

/// Oldest first
fn subjects(test_repo: &TestRepo, count: usize) -> Vec<String> {
  let mut messages = test_repo.get_commit_messages(count);
  messages.reverse();
  messages
}

/// Branches of the `BranchesGrouped` event of a sync against `origin/master`
fn grouped_branches(test_repo: &TestRepo) -> Vec<GroupedBranchInfo> {
  use crate::sync::{SyncOptions, sync_branches};
  use test_utils::sync_fixtures::TestReporter;

  let reporter = TestReporter::new();
  let repo_path = test_repo.path().to_str().unwrap();
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(sync_branches(test_repo.git_executor(), repo_path, "test-prefix", reporter.clone(), SyncOptions::default()))
    .unwrap();
  reporter
    .get_events()
    .into_iter()
    .find_map(|event| match event {
      SyncEvent::BranchesGrouped { branches, .. } => Some(branches),
      _ => None,
    })
    .unwrap()
}

#[test]
fn sets_replaces_and_removes_the_prefix() {
  assert_eq!(with_branch_prefix("Fix login", Some("auth")), "(auth) Fix login");
  assert_eq!(with_branch_prefix("ABC-1: Fix login", Some("auth")), "(auth) ABC-1: Fix login");
  assert_eq!(with_branch_prefix("(api) Fix login", Some("auth")), "(auth) Fix login");
  assert_eq!(with_branch_prefix("(api)   Fix login", None), "Fix login");
  assert_eq!(with_branch_prefix("(api) ABC-1 Fix login", None), "ABC-1 Fix login");
  // the body is not a subject
  assert_eq!(with_branch_prefix("Fix login\n\n(api) body", Some("auth")), "(auth) Fix login\n\n(api) body");
  // `()` is not a prefix
  assert_eq!(with_branch_prefix("() Fix", Some("auth")), "(auth) () Fix");
}

#[test]
fn keeps_the_autosquash_marker() {
  assert_eq!(with_branch_prefix("fixup! (api) Fix login", Some("auth")), "fixup! (auth) Fix login");
  assert_eq!(with_branch_prefix("squash! Fix login", Some("auth")), "squash! (auth) Fix login");
  assert_eq!(with_branch_prefix("amend! (api) Fix login", None), "amend! Fix login");
}

#[test]
fn validates_the_branch_name() {
  validate_branch_name("jsoup-1.21.2-update").unwrap();
  validate_branch_name("feature/sub").unwrap();
  for (name, error) in [
    ("", "cannot be empty"),
    ("feat 1", "can only contain"),
    ("a:b", "can only contain"),
    (".hidden", "cannot start with a dot"),
    ("feature/", "not a valid branch name"),
    ("a//b", "not a valid branch name"),
    ("a..b", "not a valid branch name"),
    ("-a", "not a valid branch name"),
  ] {
    let message = validate_branch_name(name).unwrap_err().to_string();
    assert!(message.contains(error), "{name}: {message}");
  }
}

#[test]
fn moves_unassigned_commits_to_a_new_branch() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let first = test_repo.create_commit("First feature commit", "file1.txt", "content1");
  let second = test_repo.create_commit("Second feature commit", "file2.txt", "content2");

  let result = move_to(&test_repo, &[&first, &second], Some("feature-123")).unwrap();

  assert_eq!(
    test_repo.get_commit_messages(2),
    ["(feature-123) Second feature commit", "(feature-123) First feature commit"]
  );
  assert_eq!(changes(&result), [MoveChange::Prefixed, MoveChange::Prefixed]);
  assert_eq!(result.commits[0].old_id, first);
  assert_eq!(result.commits[0].new_id, test_repo.rev_parse("HEAD~1").unwrap());
  assert_eq!(result.commits[1].new_id, test_repo.head());
  assert_eq!((result.target.as_deref(), result.head.as_str()), (Some("feature-123"), test_repo.head().as_str()));
  assert_eq!(result.grouped_issues, Vec::<String>::new());

  undo(&test_repo, &result);
  assert_eq!(test_repo.get_commit_messages(2), ["Second feature commit", "First feature commit"]);
}

#[test]
fn splits_commits_out_of_an_issue_branch() {
  let test_repo = TestRepo::new();
  let initial = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial).unwrap();
  test_repo.create_commit("ABC-123: Add parser", "parser.rs", "// parser");
  let lexer = test_repo.create_commit("ABC-123: Add lexer", "lexer.rs", "// lexer");
  let tokens = test_repo.create_commit("ABC-123: Add tokens", "tokens.rs", "// tokens");

  move_to(&test_repo, &[&lexer, &tokens], Some("lexer")).unwrap();

  // the issue key stays in the message that is pushed
  assert_eq!(
    test_repo.get_commit_messages(3),
    ["(lexer) ABC-123: Add tokens", "(lexer) ABC-123: Add lexer", "ABC-123: Add parser"]
  );
  let branches = grouped_branches(&test_repo);
  let lexer_branch = branches.iter().find(|b| b.name == "lexer").unwrap();
  assert_eq!(lexer_branch.origin, BranchOrigin::Prefix);
  assert_eq!(lexer_branch.issue_key.as_deref(), Some("ABC-123"));
  assert_eq!(lexer_branch.summary, "Add tokens");
  let issue_branch = branches.iter().find(|b| b.name == "ABC-123").unwrap();
  assert_eq!(issue_branch.origin, BranchOrigin::Issue);
  assert_eq!(issue_branch.issue_key.as_deref(), Some("ABC-123"));
  assert_eq!(issue_branch.commits.len(), 1);
}

#[test]
fn moves_to_another_branch_and_back_to_the_issue() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let commit = test_repo.create_commit("(api) ABC-1 Add endpoint", "api.rs", "// api");

  // the key carries no commit without a prefix: not a whole issue, so the prefix stays the way into auth
  let moved = move_to(&test_repo, &[&commit], Some("auth")).unwrap().commits;
  assert_eq!(test_repo.get_commit_messages(1), ["(auth) ABC-1 Add endpoint"]);

  move_to(&test_repo, &[&moved[0].new_id], None).unwrap();
  assert_eq!(test_repo.get_commit_messages(1), ["ABC-1 Add endpoint"]);
}

#[test]
fn skips_commits_already_in_the_target_branch() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let in_target = test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  let other = test_repo.create_commit("Add logout", "logout.rs", "// logout");

  let result = move_to(&test_repo, &[&in_target, &other], Some("auth")).unwrap();

  assert_eq!(changes(&result), [MoveChange::Unchanged, MoveChange::Prefixed]);
  assert_eq!(result.commits[1].old_id, other);
  // older than the moved commit, so not recreated
  assert_eq!(test_repo.rev_parse("HEAD~1").unwrap(), in_target);
}

#[test]
fn keeps_the_body_and_the_metadata() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.set_config("user.name", "Test Author").unwrap();
  test_repo.set_config("user.email", "test@example.com").unwrap();
  let commit = test_repo.create_commit("Fix bug\n\nThe details.", "fix.txt", "fix");
  let metadata = |revision: &str| test_repo.log(&["-1", "--pretty=format:%an <%ae> %at %T", revision]).unwrap();
  let before = metadata(&commit);

  let result = move_to(&test_repo, &[&commit], Some("bugfix-456")).unwrap();

  assert_eq!(test_repo.log(&["-1", "--pretty=format:%B"]).unwrap().trim_end(), "(bugfix-456) Fix bug\n\nThe details.");
  assert_eq!(changes(&result), [MoveChange::Prefixed]);
  assert_eq!(metadata("HEAD"), before);
}

#[test]
fn keeps_the_checked_out_branch() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch("test-branch").unwrap();
  test_repo.checkout("test-branch").unwrap();
  let commit = test_repo.create_commit("Branch commit", "file1.txt", "content1");

  move_to(&test_repo, &[&commit], Some("updated")).unwrap();

  assert_eq!(test_repo.current_branch().unwrap(), "test-branch");
  assert_eq!(test_repo.get_commit_messages(1), ["(updated) Branch commit"]);
}

#[test]
fn refuses_an_invalid_target_or_an_unknown_commit() {
  let test_repo = TestRepo::new();
  let commit = test_repo.create_commit("Initial commit", "README.md", "# Test");

  let error = move_to(&test_repo, &[&commit], Some("feat 1")).unwrap_err();
  assert!(error.to_string().contains("can only contain"), "{error}");

  let error = move_to(&test_repo, &["abc"], Some("feature")).unwrap_err();
  assert!(format!("{error:#}").contains("abc"), "{error:#}");

  let error = move_to(&test_repo, &[], Some("feature")).unwrap_err();
  assert!(error.to_string().contains("No commits"), "{error}");
  assert_eq!(test_repo.head(), commit);
}

#[test]
fn moves_many_commits_in_one_rewrite() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let commit_ids: Vec<String> = (1..=10)
    .map(|i| test_repo.create_commit(&format!("Commit number {i}"), &format!("file{i}.txt"), "content"))
    .collect();
  let ids: Vec<&str> = commit_ids.iter().map(String::as_str).collect();

  let start = std::time::Instant::now();
  let result = move_to(&test_repo, &ids, Some("batch-test")).unwrap();

  assert_eq!(result.commits.len(), 10);
  assert!(start.elapsed().as_secs() < 6, "Batch move took too long: {:?}", start.elapsed());
  assert_eq!(test_repo.get_commit_messages(1), ["(batch-test) Commit number 10"]);
}

#[test]
fn moves_the_fixups_with_their_target() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let target = test_repo.create_commit("(api) Fix login", "login.rs", "fix");
  test_repo.create_commit("Unrelated", "other.rs", "other");
  test_repo.create_commit("fixup! (api) Fix login", "login.rs", "fix again");

  let result = move_to(&test_repo, &[&target], Some("auth")).unwrap();

  assert_eq!(test_repo.get_commit_messages(3), ["fixup! (auth) Fix login", "Unrelated", "(auth) Fix login"]);
  // the requested commit first, then the fixup that followed it
  assert_eq!(changes(&result), [MoveChange::Prefixed, MoveChange::Prefixed]);
  assert_eq!(result.commits[0].old_id, target);
  assert_eq!(result.commits[1].new_id, test_repo.head());
}

#[test]
fn whole_issues_join_a_branch_without_a_rewrite_and_their_next_commits_follow() {
  let test_repo = baseline_repo(false);
  let ids = commits(&test_repo, &["IJPL-1 Parse lists", "IJPL-1 Parse maps", "IJPL-2 Print maps"]);
  let head = test_repo.head();

  let result = move_ids(&test_repo, &[&ids[0], &ids[1], &ids[2]], Some("maps"));

  assert_eq!(test_repo.head(), head);
  assert_eq!(subjects(&test_repo, 3), ["IJPL-1 Parse lists", "IJPL-1 Parse maps", "IJPL-2 Print maps"]);
  assert_eq!(changes(&result), [MoveChange::Regrouped; 3]);
  assert_eq!(result.grouped_issues, ["IJPL-1", "IJPL-2"]);
  assert_eq!(issue_groups(&test_repo), pairs(&[("IJPL-1", "maps"), ("IJPL-2", "maps")]));
  assert!(result.undo_token.as_deref().unwrap().starts_with("groups:"), "{:?}", result.undo_token);

  // the issue, not the commits, is in the branch: the next commit of IJPL-1 goes there too
  test_repo.create_commit("IJPL-1 Parse sets", "sets.rs", "// sets");
  let maps = grouped_branches(&test_repo).into_iter().find(|branch| branch.name == "maps").unwrap();
  assert_eq!((maps.origin, maps.commits.len()), (BranchOrigin::IssueGroup, 4));
}

#[test]
fn a_part_of_an_issue_and_a_commit_without_a_key_get_the_prefix_in_the_same_transaction() {
  let test_repo = baseline_repo(false);
  let ids = commits(&test_repo, &["IJPL-1 Parse lists", "IJPL-1 Parse maps", "IJPL-2 Print maps", "Fix docs"]);

  let result = move_ids(&test_repo, &[&ids[1], &ids[2], &ids[3]], Some("maps"));

  assert_eq!(
    subjects(&test_repo, 4),
    ["IJPL-1 Parse lists", "(maps) IJPL-1 Parse maps", "IJPL-2 Print maps", "(maps) Fix docs"]
  );
  assert_eq!(changes(&result), [MoveChange::Prefixed, MoveChange::Regrouped, MoveChange::Prefixed]);
  assert_eq!(issue_groups(&test_repo), pairs(&[("IJPL-2", "maps")]));
  let token = result.undo_token.clone().unwrap();
  assert!(token.contains("+groups:"), "{token}");

  // one undo moves HEAD and the issue groups back
  undo(&test_repo, &result);
  assert_eq!(test_repo.head(), ids[3]);
  assert_eq!(issue_groups(&test_repo), BTreeMap::new());
}

#[test]
fn a_prefixed_commit_that_its_key_puts_into_the_target_loses_the_prefix() {
  let test_repo = baseline_repo(false);
  commits(&test_repo, &["ABC-1 Add parser", "(lexer) ABC-1 Add lexer", "ABC-2 Add maps", "(other) ABC-2 Add sets"]);
  regroup(&test_repo, &issue_group_plan("parser", &["ABC-2"])).unwrap();
  let lexer = test_repo.rev_parse("HEAD~2").unwrap();

  // back into its issue branch, not `(ABC-1) ABC-1 Add lexer`
  let result = move_ids(&test_repo, &[&lexer], Some("ABC-1"));
  assert_eq!(changes(&result), [MoveChange::PrefixRemoved]);
  assert_eq!(subjects(&test_repo, 4)[1], "ABC-1 Add lexer");

  // into the issue group of its key, which stays an issue group
  let sets = test_repo.head();
  let result = move_ids(&test_repo, &[&sets], Some("parser"));
  assert_eq!(changes(&result), [MoveChange::PrefixRemoved]);
  assert_eq!(subjects(&test_repo, 4)[3], "ABC-2 Add sets");
  assert_eq!(issue_groups(&test_repo), pairs(&[("ABC-2", "parser")]));
}

#[test]
fn moving_an_issue_group_reassigns_its_keys_and_moving_it_back_ungroups_them() {
  let test_repo = baseline_repo(false);
  let ids = commits(&test_repo, &["ABC-1 One", "ABC-1 Uno", "ABC-2 Two", "ABC-3 Three"]);
  regroup(&test_repo, &issue_group_plan("parser", &["ABC-1", "ABC-2"])).unwrap();

  let result = move_ids(&test_repo, &[&ids[0], &ids[1], &ids[2]], Some("grammar"));
  assert_eq!(result.grouped_issues, ["ABC-1", "ABC-2"]);
  assert_eq!(issue_groups(&test_repo), pairs(&[("ABC-1", "grammar"), ("ABC-2", "grammar")]));

  // a whole issue goes back to its issue branch by the issue groups, a part of one by a prefix
  let result = move_ids(&test_repo, &[&ids[2]], None);
  assert_eq!(
    (changes(&result), result.ungrouped_issues.clone()),
    (vec![MoveChange::Regrouped], vec!["ABC-2".to_string()])
  );
  let result = move_ids(&test_repo, &[&ids[1]], None);
  assert_eq!(changes(&result), [MoveChange::Prefixed]);
  assert_eq!(subjects(&test_repo, 4)[1], "(ABC-1) ABC-1 Uno");
  assert_eq!(issue_groups(&test_repo), pairs(&[("ABC-1", "grammar")]));

  // moving a whole issue onto its own key ungroups it too
  let three = test_repo.head();
  move_ids(&test_repo, &[&three], Some("parser"));
  let result = move_ids(&test_repo, &[&three], Some("ABC-3"));
  assert_eq!(result.ungrouped_issues, ["ABC-3"]);
}

#[test]
fn a_whole_issue_joins_a_stacked_target_without_a_rewrite() {
  let test_repo = baseline_repo(true);
  let ids = commits(&test_repo, &["(auth) Add auth", "(auth/login) Add form", "ABC-1 Fix login", "Tidy login"]);
  let (fix, tidy) = (&ids[2], &ids[3]);

  // a whole issue goes by the issue groups, also into a stacked branch
  let result = move_ids(&test_repo, &[fix], Some("login"));
  assert_eq!(subjects(&test_repo, 4)[2], "ABC-1 Fix login");
  assert_eq!((changes(&result), result.target.as_deref()), (vec![MoveChange::Regrouped], Some("login")));
  assert_eq!(issue_groups(&test_repo), pairs(&[("ABC-1", "login")]));

  // a commit without a key gets the bare leaf, and a path stacks the target in the stack parents: a prefix that
  // declares another parent loses its parent level
  let result = move_ids(&test_repo, &[tidy], Some("docs/login"));
  assert_eq!(
    subjects(&test_repo, 4)[1..],
    ["(login) Add form", "ABC-1 Fix login", "(login) Tidy login"].map(str::to_string)
  );
  assert_eq!(changes(&result), [MoveChange::Prefixed, MoveChange::Prefixed]);
}

#[test]
fn the_fixups_of_a_whole_issue_move_with_it() {
  let test_repo = baseline_repo(false);
  let ids = commits(&test_repo, &["ABC-1 Fix login", "Unrelated", "fixup! ABC-1 Fix login"]);

  let result = move_ids(&test_repo, &[&ids[0]], Some("auth"));

  assert_eq!(changes(&result), [MoveChange::Regrouped, MoveChange::Regrouped]);
  assert_eq!(result.commits[1].old_id, ids[2]);
  assert_eq!(issue_groups(&test_repo), pairs(&[("ABC-1", "auth")]));
}

#[test]
fn a_dry_run_writes_nothing_and_expect_head_refuses_a_moved_head() {
  let test_repo = baseline_repo(false);
  let ids = commits(&test_repo, &["IJPL-1 Parse lists", "Fix docs"]);
  let mut dry_run = params(&test_repo, &[&ids[0], &ids[1]], Some("maps"));
  dry_run.dry_run = true;

  let result = move_commits_to_branch_core(test_repo.git_executor(), &dry_run).unwrap();
  assert_eq!(changes(&result), [MoveChange::Regrouped, MoveChange::Prefixed]);
  assert_eq!(result.grouped_issues, ["IJPL-1"]);
  assert_eq!((result.undo_token, result.head), (None, ids[1].clone()));
  assert_eq!((test_repo.head(), issue_groups(&test_repo)), (ids[1].clone(), BTreeMap::new()));

  let mut stale = params(&test_repo, &[&ids[1]], Some("maps"));
  stale.expect_head = Some(ids[0].clone());
  let error = move_commits_to_branch_core(test_repo.git_executor(), &stale).unwrap_err();
  assert!(matches!(error.downcast_ref::<RegroupError>(), Some(RegroupError::HeadMoved(_))), "{error:#}");
}
