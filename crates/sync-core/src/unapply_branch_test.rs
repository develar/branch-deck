use crate::unapply_branch::{UnapplyBranchParams, UnapplyBranchResult, unapply_branch_core};
use anyhow::Result;
use git_ops::model::to_final_branch_name;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// Create a test repository with commits in HEAD and a virtual branch
fn create_test_repo_with_virtual_branch() -> Result<(TestRepo, Vec<String>)> {
  let test_repo = TestRepo::new();

  // Create initial commit
  let _initial_commit_id = test_repo.create_commit("Initial commit", "initial.txt", "initial content");

  // Create some commits in HEAD that represent the commits to be unapplied
  let commit1_id = test_repo.create_commit("(feature) Add file1", "file1.txt", "content 1");
  let commit2_id = test_repo.create_commit("(feature) Add file2", "file2.txt", "content 2");

  // Create a virtual branch to simulate the existing virtual branch structure
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;
  test_repo.create_branch(&virtual_branch_name).map_err(|e| anyhow::anyhow!(e))?;

  // Stay on master (the commits we created are in HEAD and can be unapplied)
  Ok((test_repo, vec![commit1_id, commit2_id]))
}

/// Unapplies `original_commit_ids` of `branch_name` (branch prefix `test-user`)
fn unapply(test_repo: &TestRepo, branch_name: &str, original_commit_ids: Vec<String>) -> Result<UnapplyBranchResult> {
  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: branch_name.to_string(),
    branch_prefix: "test-user".to_string(),
    original_commit_ids,
  };
  unapply_branch_core(test_repo.git_executor(), params)
}

fn assert_unapply_fails(test_repo: &TestRepo, branch_name: &str, original_commit_ids: Vec<String>, expected_error: &str) {
  let error = unapply(test_repo, branch_name, original_commit_ids).expect_err("unapply should fail").to_string();
  assert!(error.contains(expected_error), "error should contain {expected_error:?}: {error}");
}

/// The virtual branch is gone and the unapplied branch took its place
fn assert_branch_moved(test_repo: &TestRepo, virtual_branch_name: &str, result: &UnapplyBranchResult) {
  assert!(!test_repo.branch_exists(virtual_branch_name), "Virtual branch should have been moved");
  assert!(test_repo.branch_exists(&result.unapplied_branch_name), "Unapplied branch should exist");
}

#[test]
fn test_unapply_single_commit() -> Result<()> {
  let (test_repo, original_commits) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  // Use the first original commit
  let commit_to_unapply = &original_commits[0];

  let result = unapply(&test_repo, &virtual_branch_name, vec![commit_to_unapply.clone()])?;

  // Verify the result
  assert_eq!(result.commits_removed.len(), 1);
  assert_eq!(result.commits_removed[0], *commit_to_unapply);
  assert!(result.unapplied_branch_name.starts_with("test-user/unapplied/feature"));

  assert_branch_moved(&test_repo, &virtual_branch_name, &result);

  // Verify HEAD was reset (the commit should no longer be in HEAD)
  let head_commits = test_repo
    .git_executor()
    .execute_command(&["log", "--format=%H", "-n", "5"], test_repo.path().to_str().unwrap())?;
  assert!(!head_commits.contains(commit_to_unapply), "Unapplied commit should not be in HEAD");

  Ok(())
}

#[test]
fn test_unapply_multiple_commits() -> Result<()> {
  let (test_repo, original_commits) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  let result = unapply(&test_repo, &virtual_branch_name, original_commits)?;

  // Verify the result
  assert_eq!(result.commits_removed.len(), 2);
  assert!(result.unapplied_branch_name.starts_with("test-user/unapplied/feature"));

  // Verify HEAD was reset to initial commit
  let head_log = test_repo.git_executor().execute_command(&["log", "--oneline"], test_repo.path().to_str().unwrap())?;
  assert!(head_log.contains("Initial commit"));
  assert!(!head_log.contains("Add file1"));
  assert!(!head_log.contains("Add file2"));

  Ok(())
}

#[test]
fn test_unapply_non_virtual_branch_fails() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;

  // Try to unapply a non-virtual branch (master)
  assert_unapply_fails(&test_repo, "master", vec!["dummy".to_string()], "Can only unapply virtual branches");

  Ok(())
}

#[test]
fn test_unapply_nonexistent_branch_fails() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;

  assert_unapply_fails(&test_repo, "test-user/virtual/nonexistent", vec!["dummy".to_string()], "does not exist");

  Ok(())
}

#[test]
fn test_unapply_empty_commit_list_fails() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  assert_unapply_fails(&test_repo, &virtual_branch_name, vec![], "No commits provided to unapply");

  Ok(())
}

#[test]
fn test_unapply_current_branch_fails() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  // Switch to the virtual branch
  test_repo.checkout(&virtual_branch_name).map_err(|e| anyhow::anyhow!(e))?;

  assert_unapply_fails(
    &test_repo,
    &virtual_branch_name,
    vec!["dummy".to_string()],
    "Cannot unapply the currently checked out branch",
  );

  Ok(())
}

#[test]
fn test_unapply_branch_naming_collision() -> Result<()> {
  let (test_repo, original_commits) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  // Create a branch that would collide with the unapplied name
  let head_commit = test_repo.head();
  test_repo.create_branch_at("test-user/unapplied/feature", &head_commit).map_err(|e| anyhow::anyhow!(e))?;

  let result = unapply(&test_repo, &virtual_branch_name, vec![original_commits[0].clone()])?;

  // Should have created a suffixed name to avoid collision
  assert!(result.unapplied_branch_name.ends_with("-1") || result.unapplied_branch_name.ends_with("-2"));
  assert_ne!(result.unapplied_branch_name, "test-user/unapplied/feature");

  Ok(())
}

#[test]
fn test_unapply_invalid_commit_ids() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  // Use commit IDs that don't exist in the virtual branch
  assert_unapply_fails(
    &test_repo,
    &virtual_branch_name,
    vec!["nonexistent1".to_string(), "nonexistent2".to_string()],
    "are not found in HEAD",
  );

  Ok(())
}

#[test]
fn test_unapply_non_consecutive_commits() -> Result<()> {
  let test_repo = TestRepo::new();

  // Create initial commit
  let _initial_commit_id = test_repo.create_commit("Initial commit", "initial.txt", "initial content");

  // Create a sequence of commits where we want to drop non-consecutive ones
  let commit1_id = test_repo.create_commit("(feature) Add file1", "file1.txt", "content 1");
  let _commit2_id = test_repo.create_commit("(other) Keep this commit", "other.txt", "keep this");
  let commit3_id = test_repo.create_commit("(feature) Add file3", "file3.txt", "content 3");
  let _commit4_id = test_repo.create_commit("(different) Another keeper", "diff.txt", "another keeper");

  // Create virtual branch
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;
  test_repo.create_branch(&virtual_branch_name).map_err(|e| anyhow::anyhow!(e))?;

  // Try to drop the first and third commits (non-consecutive)
  let result = unapply(&test_repo, &virtual_branch_name, vec![commit1_id.clone(), commit3_id.clone()])?;

  // Verify the result
  assert_eq!(result.commits_removed.len(), 2);
  assert!(result.commits_removed.contains(&commit1_id));
  assert!(result.commits_removed.contains(&commit3_id));

  assert_branch_moved(&test_repo, &virtual_branch_name, &result);

  // Verify that HEAD now only contains the commits we didn't drop
  let head_log = test_repo
    .git_executor()
    .execute_command(&["log", "--oneline", "--format=%s"], test_repo.path().to_str().unwrap())?;
  assert!(head_log.contains("Initial commit"));
  assert!(head_log.contains("Keep this commit"));
  assert!(head_log.contains("Another keeper"));
  assert!(!head_log.contains("Add file1"), "Dropped commit should not be in HEAD");
  assert!(!head_log.contains("Add file3"), "Dropped commit should not be in HEAD");

  Ok(())
}

#[test]
fn test_unapply_drops_the_commits_from_the_current_branch_of_a_clone() -> Result<()> {
  // with a remote, the baseline is `origin/master`: the commits must still leave the current branch, `master`
  let upstream = TestRepo::new();
  upstream.create_commit("Initial commit", "initial.txt", "initial content");
  let test_repo = TestRepo::new_empty();
  test_repo.clone_from(upstream.path()).map_err(|e| anyhow::anyhow!(e))?;
  let kept = test_repo.create_commit("(other) Add other", "other.txt", "other");
  let dropped = test_repo.create_commit("(feature) Add file1", "file1.txt", "content 1");
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;
  test_repo.create_branch(&virtual_branch_name).map_err(|e| anyhow::anyhow!(e))?;

  unapply(&test_repo, &virtual_branch_name, vec![dropped.clone()])?;

  let head_commits = test_repo.log(&["--format=%H", "master"]).map_err(|e| anyhow::anyhow!(e))?;
  assert!(!head_commits.contains(&dropped), "the unapplied commit must leave master");
  assert!(head_commits.contains(&kept), "the other commits must stay");
  assert!(!test_repo.branch_exists("origin/master"), "no local branch named like the baseline");
  Ok(())
}
