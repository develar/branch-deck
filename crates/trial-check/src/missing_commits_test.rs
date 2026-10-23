use domain_types::{BranchError, SyncEvent};
use pretty_assertions::assert_eq;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

/// The missing commits of the conflict of `branch` in a sync with the `test` prefix: `(subject, branch)`
async fn missing_commits_of(test_repo: &TestRepo, branch: &str) -> Vec<(String, Option<String>)> {
  let progress = TestReporter::new();
  sync_branches(
    test_repo.git_executor(),
    test_repo.path().to_str().unwrap(),
    "test",
    progress.clone(),
    SyncOptions::default(),
  )
  .await
  .unwrap();
  progress
    .get_events()
    .into_iter()
    .find_map(|event| match event {
      SyncEvent::CommitError {
        branch_name,
        error: BranchError::MergeConflict(info),
        ..
      } if branch_name == branch => Some(info.conflict_analysis.missing_commits),
      _ => None,
    })
    .expect("the branch conflicts")
    .into_iter()
    .map(|commit| (commit.subject, commit.branch))
    .collect()
}

#[tokio::test]
async fn the_missing_commits_leave_out_the_branches_it_is_built_on_and_name_their_branch() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.set_config("branchdeck.stacks", "true").unwrap();
  // login is built on auth, which has the first version of the file, but not on other, which changed it next
  test_repo.create_commit("(auth) Add config", "config.txt", "a");
  test_repo.create_commit("(other) Change config", "config.txt", "b");
  test_repo.create_commit("Tune config", "config.txt", "c");
  test_repo.create_commit("(auth/login) Change config again", "config.txt", "d");

  assert_eq!(
    missing_commits_of(&test_repo, "login").await,
    [("Tune config".to_string(), None), ("(other) Change config".to_string(), Some("other".to_string()))]
  );
}
