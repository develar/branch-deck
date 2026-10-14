use crate::move_commits::{MoveCommitsParams, MoveCommitsResult, move_commits_to_branch_core, validate_branch_name, with_branch_prefix};
use pretty_assertions::assert_eq;
use sync_types::{BranchOrigin, GroupedBranchInfo, SyncEvent};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn move_to(test_repo: &TestRepo, commit_ids: &[&str], target_branch: Option<&str>) -> anyhow::Result<MoveCommitsResult> {
  let params = MoveCommitsParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    commit_ids: commit_ids.iter().map(|id| (*id).to_string()).collect(),
    target_branch: target_branch.map(str::to_string),
  };
  move_commits_to_branch_core(test_repo.git_executor(), &params)
}

/// Branches of the `BranchesGrouped` event of a sync against `origin/master`
fn grouped_branches(test_repo: &TestRepo) -> Vec<GroupedBranchInfo> {
  use crate::sync::{SyncOptions, sync_branches};
  use sync_test_utils::TestReporter;

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
  assert_eq!(result.moved.len(), 2);
  assert_eq!(result.moved[0].old_id, first);
  assert_eq!(result.moved[0].new_id, test_repo.rev_parse("HEAD~1").unwrap());
  assert_eq!(result.moved[0].previous_message, "First feature commit");
  assert_eq!(result.moved[1].new_id, test_repo.head());
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

  let moved = move_to(&test_repo, &[&commit], Some("auth")).unwrap().moved;
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

  assert_eq!(result.moved.len(), 1);
  assert_eq!(result.moved[0].old_id, other);
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
  assert_eq!(result.moved[0].previous_message, "Fix bug\n\nThe details.");
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

  assert_eq!(result.moved.len(), 10);
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
  // the requested commit first, then the fixup that followed it; both can be undone
  assert_eq!(result.moved.len(), 2);
  assert_eq!(result.moved[0].old_id, target);
  assert_eq!(result.moved[1].previous_message, "fixup! (api) Fix login");
  assert_eq!(result.moved[1].new_id, test_repo.head());
}
