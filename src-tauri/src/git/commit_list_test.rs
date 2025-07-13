use crate::git::commit_list::*;
use crate::test_utils::git_test_utils::TestRepo;

#[test]
fn test_get_commit_list_with_no_commits_ahead() {
  let test_repo = TestRepo::new();

  // Create only the initial commit
  test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Open repository to get git2::Repository
  let repo = git2::Repository::open(test_repo.path()).unwrap();

  // Since there's no remote branch and no commits with prefixes,
  // we should get an empty list
  let commits = get_commit_list(&repo, "master").unwrap();
  assert_eq!(commits.len(), 0, "Should return 0 commits when no prefixed commits exist");
}

#[test]
fn test_get_commit_list_head_equals_local_branch_no_upstream() {
  let test_repo = TestRepo::new();

  // Create multiple commits to simulate a real repository
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "login fix");
  test_repo.create_commit("Regular commit", "regular.txt", "regular content");
  test_repo.create_commit("(ui-components) Add button", "button.js", "button code");

  // In this scenario:
  // - No origin/master exists (no upstream)
  // - HEAD and master point to the same commit (last commit)
  // - This mimics a local repository without upstream
  let repo = git2::Repository::open(test_repo.path()).unwrap();
  let commits = get_commit_list(&repo, "master").unwrap();

  // Should return commits with branch prefixes when no upstream is configured
  assert_eq!(commits.len(), 3, "Should return commits with branch prefixes when no upstream");

  // Verify the commits are the ones with prefixes (in chronological order)
  assert_eq!(commits[0].message().unwrap().trim(), "(feature-auth) Add authentication");
  assert_eq!(commits[1].message().unwrap().trim(), "(bugfix-login) Fix login issue");
  assert_eq!(commits[2].message().unwrap().trim(), "(ui-components) Add button");

  // Verify that HEAD and master indeed point to the same commit
  let head_hash = test_repo.head();
  let master_hash = test_repo.rev_parse("master").unwrap();
  assert_eq!(head_hash, master_hash, "HEAD and master should point to the same commit");
}

#[test]
fn test_get_commit_list_with_commits_ahead() {
  let test_repo = TestRepo::new();

  // Create an initial commit
  let id1 = test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Create a baseline branch reference
  test_repo.create_branch_at("baseline", &id1).unwrap();

  // Create additional commits ahead of baseline
  let id2 = test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  let id3 = test_repo.create_commit("(feature-auth) Improve auth", "auth.js", "better auth code");

  // Get commits ahead of baseline
  let repo = git2::Repository::open(test_repo.path()).unwrap();
  let commits = get_commit_list(&repo, "baseline").unwrap();

  // Should return commits in chronological order (oldest first)
  assert_eq!(commits.len(), 2);
  assert_eq!(commits[0].id().to_string(), id2);
  assert_eq!(commits[1].id().to_string(), id3);
  assert_eq!(commits[0].message().unwrap().trim(), "(feature-auth) Add authentication");
  assert_eq!(commits[1].message().unwrap().trim(), "(feature-auth) Improve auth");
}

#[test]
fn test_get_commit_list_with_remote_branch() {
  let test_repo = TestRepo::new();

  // Create an initial commit
  let initial_commit_id = test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Create a remote branch reference to simulate origin/master
  test_repo.create_branch_at("origin/master", &initial_commit_id).unwrap();

  // Create commits ahead of origin/master
  let id2 = test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "fixed login");
  let id3 = test_repo.create_commit("(ui-components) Add button", "button.vue", "<button></button>");

  // Get commits ahead of origin/master
  let repo = git2::Repository::open(test_repo.path()).unwrap();
  let commits = get_commit_list(&repo, "master").unwrap();

  assert_eq!(commits.len(), 2);
  assert_eq!(commits[0].id().to_string(), id2);
  assert_eq!(commits[1].id().to_string(), id3);
}

#[test]
fn test_get_commit_list_handles_missing_branch() {
  let test_repo = TestRepo::new();

  // Create some commits
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(feature-test) Test feature", "test.js", "test code");

  // Try to get commits against a non-existent branch
  let repo = git2::Repository::open(test_repo.path()).unwrap();
  let result = get_commit_list(&repo, "nonexistent-branch");

  // Should succeed and return all commits from HEAD since neither remote nor local branch exists
  // This behavior allows the function to work even when the specified baseline branch doesn't exist
  assert!(result.is_ok());
  let commits = result.unwrap();
  assert_eq!(commits.len(), 2, "Should return all commits when baseline branch doesn't exist");
}

#[test]
fn test_get_commit_list_preserves_commit_order() {
  let test_repo = TestRepo::new();

  // Create an initial commit and branch it
  let initial_id = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("baseline", &initial_id).unwrap();

  // Create multiple commits in sequence
  let messages = [
    "(feature-auth) First auth commit",
    "(feature-auth) Second auth commit",
    "(bugfix-login) Login fix",
    "(feature-auth) Third auth commit",
    "(ui-components) UI commit",
  ];

  let mut commit_ids = Vec::new();
  for (i, message) in messages.iter().enumerate() {
    let id = test_repo.create_commit(message, &format!("file{i}.txt"), &format!("content {i}"));
    commit_ids.push(id);
  }

  // Get commits ahead of baseline
  let repo = git2::Repository::open(test_repo.path()).unwrap();
  let commits = get_commit_list(&repo, "baseline").unwrap();

  // Verify all commits are present and in chronological order (oldest first)
  assert_eq!(commits.len(), 5);
  for (i, commit) in commits.iter().enumerate() {
    assert_eq!(commit.id().to_string(), commit_ids[i]);
    assert_eq!(commit.message().unwrap().trim(), messages[i]);
  }
}
