use crate::sync::detect_baseline_branch;
use crate::unapply_branch::{UnapplyBranchParams, unapply_branch_core};
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

#[test]
fn test_unapply_single_commit() -> Result<()> {
  let (test_repo, original_commits) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  // Use the first original commit
  let commit_to_unapply = &original_commits[0];

  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: virtual_branch_name.clone(),
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec![commit_to_unapply.clone()],
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch)?;

  // Verify the result
  assert_eq!(result.commits_removed.len(), 1);
  assert_eq!(result.commits_removed[0], *commit_to_unapply);
  assert!(result.unapplied_branch_name.starts_with("test-user/unapplied/feature"));

  // Verify virtual branch was moved
  assert!(!test_repo.branch_exists(&virtual_branch_name), "Virtual branch should have been moved");

  // Verify unapplied branch exists
  assert!(test_repo.branch_exists(&result.unapplied_branch_name), "Unapplied branch should exist");

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

  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: virtual_branch_name.clone(),
    branch_prefix: "test-user".to_string(),
    original_commit_ids: original_commits.clone(),
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch)?;

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
  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "master".to_string(),
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec!["dummy".to_string()],
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch);

  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("Can only unapply virtual branches"));

  Ok(())
}

#[test]
fn test_unapply_nonexistent_branch_fails() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;

  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "test-user/virtual/nonexistent".to_string(),
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec!["dummy".to_string()],
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch);

  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("does not exist"));

  Ok(())
}

#[test]
fn test_unapply_empty_commit_list_fails() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: virtual_branch_name,
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec![], // Empty list
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch);

  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("No commits provided to unapply"));

  Ok(())
}

#[test]
fn test_unapply_current_branch_fails() -> Result<()> {
  let (test_repo, _) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  // Switch to the virtual branch
  test_repo.checkout(&virtual_branch_name).map_err(|e| anyhow::anyhow!(e))?;

  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: virtual_branch_name,
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec!["dummy".to_string()],
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch);

  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("Cannot unapply the currently checked out branch"));

  Ok(())
}

#[test]
fn test_unapply_branch_naming_collision() -> Result<()> {
  let (test_repo, original_commits) = create_test_repo_with_virtual_branch()?;
  let virtual_branch_name = to_final_branch_name("test-user", "feature")?;

  // Create a branch that would collide with the unapplied name
  let head_commit = test_repo.head();
  test_repo.create_branch_at("test-user/unapplied/feature", &head_commit).map_err(|e| anyhow::anyhow!(e))?;

  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: virtual_branch_name,
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec![original_commits[0].clone()],
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch)?;

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
  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: virtual_branch_name,
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec!["nonexistent1".to_string(), "nonexistent2".to_string()],
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch);

  assert!(result.is_err());
  assert!(result.unwrap_err().to_string().contains("are not found in HEAD"));

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
  let params = UnapplyBranchParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: virtual_branch_name.clone(),
    branch_prefix: "test-user".to_string(),
    original_commit_ids: vec![commit1_id.clone(), commit3_id.clone()],
  };

  let baseline_branch = detect_baseline_branch(test_repo.git_executor(), test_repo.path().to_str().unwrap(), "master")?;
  let result = unapply_branch_core(test_repo.git_executor(), params, &baseline_branch)?;

  // Verify the result
  assert_eq!(result.commits_removed.len(), 2);
  assert!(result.commits_removed.contains(&commit1_id));
  assert!(result.commits_removed.contains(&commit3_id));

  // Verify virtual branch was moved
  assert!(!test_repo.branch_exists(&virtual_branch_name), "Virtual branch should have been moved");

  // Verify unapplied branch exists
  assert!(test_repo.branch_exists(&result.unapplied_branch_name), "Unapplied branch should exist");

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
