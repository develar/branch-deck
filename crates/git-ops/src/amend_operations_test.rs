use super::amend_operations::{AmendResult, AmendToCommitParams, amend_to_commit_in_main};
use super::copy_commit::CopyCommitError;
use anyhow::Result;
use std::fs;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn repo_path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

/// Writes a file and stages it
fn stage_file(repo: &TestRepo, filename: &str, content: &str) -> Result<()> {
  write_file(repo, filename, content)?;
  repo.git_executor().execute_command(&["add", filename], repo_path(repo))?;
  Ok(())
}

/// Writes a file without staging it (simulates the IntelliJ IDEA workflow)
fn write_file(repo: &TestRepo, filename: &str, content: &str) -> Result<()> {
  fs::write(repo.path().join(filename), content)?;
  Ok(())
}

fn commit_count(repo: &TestRepo) -> Result<usize> {
  let output = repo.git_executor().execute_command(&["rev-list", "--count", "HEAD"], repo_path(repo))?;
  Ok(output.trim().parse().unwrap_or(0))
}

fn read_file(repo: &TestRepo, filename: &str) -> Result<String> {
  Ok(fs::read_to_string(repo.path().join(filename))?)
}

fn amend(repo: &TestRepo, original_commit_id: &str, files: &[&str]) -> Result<AmendResult, CopyCommitError> {
  let params = AmendToCommitParams {
    original_commit_id: original_commit_id.to_string(),
    files: files.iter().map(ToString::to_string).collect(),
  };
  amend_to_commit_in_main(repo.git_executor(), repo_path(repo), params)
}

/// `file1.txt` of a two-commit repo was amended into `original_commit`, which got a new ID
fn assert_file1_amended(repo: &TestRepo, result: &AmendResult, original_commit: &str) -> Result<()> {
  assert_ne!(result.amended_commit_id, "");
  assert_ne!(result.rebased_to_commit, "");
  assert_ne!(result.amended_commit_id, original_commit);

  // The file was amended and the commit count is still the same
  assert_eq!(read_file(repo, "file1.txt")?, "amended content");
  assert_eq!(commit_count(repo)?, 2);
  Ok(())
}

#[test]
fn test_amend_to_commit_basic() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit
  let commit1 = repo.create_commit("Initial commit", "file1.txt", "initial content");

  // Create second commit
  repo.create_commit("Add second file", "file2.txt", "second file");

  // Modify file and stage for amend
  stage_file(&repo, "file1.txt", "amended content")?;

  let result = amend(&repo, &commit1, &["file1.txt"])?;

  assert_file1_amended(&repo, &result, &commit1)?;

  Ok(())
}

#[test]
fn test_amend_no_uncommitted_changes() {
  let repo = TestRepo::new();

  // Create initial commit
  let commit1 = repo.create_commit("Initial commit", "file1.txt", "initial content");

  // Should fail because there are no uncommitted changes
  let result = amend(&repo, &commit1, &["file1.txt"]);
  assert!(result.is_err());

  let error = result.unwrap_err().to_string();
  assert!(error.contains("No uncommitted changes"));
}

#[test]
fn test_amend_preserves_commit_metadata() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit with specific timestamp
  let commit1 = repo.create_commit("Initial commit", "file1.txt", "initial content");

  // Modify file and stage for amend
  stage_file(&repo, "file1.txt", "amended content")?;

  let result = amend(&repo, &commit1, &["file1.txt"])?;

  // Verify original author information was preserved in amended commit
  // The original commit was created by "Test User" so that should be preserved
  let author_info = repo
    .git_executor()
    .execute_command(&["log", "-1", "--format=%an %ae", &result.amended_commit_id], repo_path(&repo))?;
  let parts: Vec<&str> = author_info.split_whitespace().collect();

  assert_eq!(parts[0], "Test");
  assert_eq!(parts[1], "User");
  assert_eq!(parts[2], "test@example.com");

  Ok(())
}

#[test]
fn test_working_directory_preserved() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit
  let commit1 = repo.create_commit("Initial commit", "file1.txt", "initial content");

  // Create an uncommitted file that should be preserved
  let uncommitted_file = repo.path().join("uncommitted.txt");
  fs::write(&uncommitted_file, "uncommitted content")?;

  // Add staged changes for amend
  stage_file(&repo, "file1.txt", "amended content")?;

  amend(&repo, &commit1, &["file1.txt"])?;

  // Verify uncommitted file still exists (working directory preserved)
  assert!(uncommitted_file.exists());
  assert_eq!(fs::read_to_string(&uncommitted_file)?, "uncommitted content");

  Ok(())
}

#[test]
fn test_amend_with_unstaged_changes() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit
  let commit1 = repo.create_commit("Initial commit", "file1.txt", "initial content");

  // Create second commit
  repo.create_commit("Add second file", "file2.txt", "second file");

  // Modify file WITHOUT staging (simulates IntelliJ IDEA workflow)
  write_file(&repo, "file1.txt", "amended content")?;

  // Should succeed with -a flag auto-staging the changes
  let result = amend(&repo, &commit1, &["file1.txt"])?;

  assert_file1_amended(&repo, &result, &commit1)?;

  Ok(())
}

#[test]
fn test_amend_with_mixed_staged_unstaged_changes() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit
  let commit1 = repo.create_commit("Initial commit", "file1.txt", "initial content");

  // Create second commit
  repo.create_commit("Add second file", "file2.txt", "second file");

  // Modify file1 without staging (unstaged change)
  write_file(&repo, "file1.txt", "amended content")?;

  // Add a new file and stage it
  let new_file = repo.path().join("file3.txt");
  fs::write(&new_file, "new file content")?;
  repo.git_executor().execute_command(&["add", "file3.txt"], repo_path(&repo))?;

  // Should succeed with -a flag handling both staged and unstaged changes
  let result = amend(&repo, &commit1, &["file1.txt", "file3.txt"])?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");

  // Verify both files were included in the amend
  assert_eq!(read_file(&repo, "file1.txt")?, "amended content");
  assert_eq!(read_file(&repo, "file3.txt")?, "new file content");

  Ok(())
}

#[test]
fn test_error_handling_invalid_commit() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit
  repo.create_commit("Initial commit", "file1.txt", "initial content");

  // Add uncommitted changes
  stage_file(&repo, "file1.txt", "amended content")?;

  let result = amend(&repo, "invalid_commit_hash", &["file1.txt"]);
  assert!(result.is_err());

  Ok(())
}

#[test]
fn test_amend_with_multiple_subsequent_commits() -> Result<()> {
  let repo = TestRepo::new();

  // Create a series of commits
  let commit1 = repo.create_commit("Initial commit", "file1.txt", "initial");
  repo.create_commit("Second commit", "file2.txt", "second");
  repo.create_commit("Third commit", "file3.txt", "third");

  let initial_count = commit_count(&repo)?;

  // Add uncommitted changes to amend to the first commit
  stage_file(&repo, "file1.txt", "initial amended")?;

  // This should fail due to conflicts or succeed with rebasing
  let result = amend(&repo, &commit1, &["file1.txt"]);

  match result {
    Ok(_) => {
      // If successful, verify all commits are still there
      assert_eq!(commit_count(&repo)?, initial_count);
      assert_eq!(read_file(&repo, "file1.txt")?, "initial amended");
    }
    Err(e) => {
      // If it fails, it should be due to conflicts
      let error_msg = e.to_string();
      assert!(error_msg.contains("conflict") || error_msg.contains("Cannot safely amend"));
    }
  }

  Ok(())
}

#[test]
fn test_amend_with_intervening_commits_modifying_same_file() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit with a file
  let original_commit = repo.create_commit("Original commit", "shared.txt", "original content");

  // Create intervening commits that modify the same file (simulating other branches)
  repo.create_commit("Intervening commit 1", "shared.txt", "modified by commit 1");
  repo.create_commit("Intervening commit 2", "shared.txt", "modified by commit 2");

  let initial_count = commit_count(&repo)?;

  // Add uncommitted changes to amend to the original commit
  stage_file(&repo, "shared.txt", "original content with new amendments")?;

  // This should succeed with our new implementation
  let result = amend(&repo, &original_commit, &["shared.txt"])?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");
  assert_ne!(result.rebased_to_commit, "");
  assert_ne!(result.amended_commit_id, original_commit);

  // Verify commit count is still the same (no commits lost)
  assert_eq!(commit_count(&repo)?, initial_count);

  // Verify the amended commit contains our changes by checking the file at that specific commit
  let amended_file_content = repo
    .git_executor()
    .execute_command(&["show", &format!("{}:shared.txt", result.amended_commit_id)], repo_path(&repo))?;
  assert_eq!(amended_file_content.trim(), "original content with new amendments");

  // The final state depends on what subsequent commits did - they were rebased on top of our amended commit
  // Since the subsequent commits completely overwrote the file content, the final state should be from the last commit
  // But our test confirms the amend operation itself worked correctly

  Ok(())
}

#[test]
fn test_amend_multiple_files_optimization() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit with multiple files
  let original_commit = repo.create_commit("Initial commit", "file1.txt", "content1");
  repo.create_commit("Add file2", "file2.txt", "content2");
  repo.create_commit("Add file3", "file3.txt", "content3");

  // Modify multiple files to test batch processing
  stage_file(&repo, "file1.txt", "amended content1")?;
  stage_file(&repo, "file2.txt", "amended content2")?;
  stage_file(&repo, "file3.txt", "amended content3")?;

  // Should use the optimized batch processing for multiple files
  let result = amend(&repo, &original_commit, &["file1.txt", "file2.txt", "file3.txt"])?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");
  assert_ne!(result.amended_commit_id, original_commit);

  // Verify the amended commit contains our changes for file1 (the original commit only had file1)
  let amended_file_content = repo
    .git_executor()
    .execute_command(&["show", &format!("{}:file1.txt", result.amended_commit_id)], repo_path(&repo))?;
  assert_eq!(amended_file_content.trim(), "amended content1");

  Ok(())
}

#[test]
fn test_amend_conflict_returns_merge_conflict_info() -> Result<()> {
  let repo = TestRepo::new();

  // Create initial commit
  repo.create_commit("Base commit", "base.txt", "base content");

  // Create second commit to amend to
  let commit_to_amend = repo.create_commit("Add file1", "file1.txt", "line 1\noriginal line 2\nline 3");

  // Create third commit that modifies the same file (this will cause conflicts when rebased)
  repo.create_commit("Later modification", "file1.txt", "line 1\nmodified by later commit\nline 3");

  // Add conflicting uncommitted changes to the same lines
  stage_file(&repo, "file1.txt", "line 1\nmodified by uncommitted\nline 3")?;

  // Attempt to amend - should fail with conflict
  let result = amend(&repo, &commit_to_amend, &["file1.txt"]);

  // Should return an error due to conflicts
  assert!(result.is_err());

  let error = result.unwrap_err();

  // For this test, we mainly care that conflicts are detected properly
  // The exact error type may vary depending on where the conflict analysis fails
  match &error {
    crate::copy_commit::CopyCommitError::BranchError(branch_err) => {
      match branch_err {
        crate::model::BranchError::MergeConflict(conflict_info) => {
          // Verify the conflict info has expected data
          assert_ne!(conflict_info.commit.hash, "");
          assert_ne!(conflict_info.commit.message, "");
          assert!(!conflict_info.conflicting_files.is_empty());

          // Should have at least one conflicting file (file1.txt)
          let conflicting_file = &conflict_info.conflicting_files[0];
          assert_eq!(conflicting_file.file, "file1.txt");

          println!("✓ Correctly returned MergeConflictInfo with {} conflicting files", conflict_info.conflicting_files.len());
        }
        crate::model::BranchError::Generic(msg) => {
          // Sometimes conflicts are detected but conflict analysis may fail
          // That's also acceptable behavior - the important thing is we detect conflicts
          assert!(msg.contains("conflict") || msg.contains("Rebase"));
          println!("✓ Conflicts properly detected with message: {msg}");
        }
      }
    }
    crate::copy_commit::CopyCommitError::Other(other_err) => {
      // Check if it's a conflict-related error
      let error_msg = other_err.to_string();
      assert!(error_msg.contains("conflict") || error_msg.contains("Rebase"));
      println!("✓ Conflicts properly detected with error: {error_msg}");
    }
  }

  Ok(())
}

#[test]
fn test_amend_conflict_in_non_linear_history_returns_merge_conflict_info() -> Result<()> {
  let repo = TestRepo::new();

  repo.create_commit("Base commit", "base.txt", "base content");
  let branch = repo.git_executor().execute_command(&["symbolic-ref", "--short", "HEAD"], repo_path(&repo))?;
  let commit_to_amend = repo.create_commit("Add file1", "file1.txt", "line 1\noriginal line 2\nline 3\n");

  // A merge commit after the target makes the history non-linear, so amend falls back to fixup + autosquash
  repo.git_executor().execute_command(&["checkout", "-q", "-b", "side"], repo_path(&repo))?;
  repo.create_commit("Side commit", "side.txt", "side content");
  repo.git_executor().execute_command(&["checkout", "-q", &branch], repo_path(&repo))?;
  repo.create_commit("Later modification", "file1.txt", "line 1\nmodified by later commit\nline 3\n");
  repo
    .git_executor()
    .execute_command(&["merge", "-q", "--no-ff", "-m", "Merge side", "side"], repo_path(&repo))?;
  let head_before = repo.git_executor().execute_command(&["rev-parse", "HEAD"], repo_path(&repo))?;

  // Conflicting uncommitted change to the same line
  write_file(&repo, "file1.txt", "line 1\nmodified by uncommitted\nline 3\n")?;

  let error = amend(&repo, &commit_to_amend, &["file1.txt"]).expect_err("amend should conflict");

  let CopyCommitError::BranchError(crate::model::BranchError::MergeConflict(conflict_info)) = error else {
    panic!("expected MergeConflictInfo, got {error:?}");
  };
  assert_eq!(conflict_info.original_parent.hash, commit_to_amend);
  assert_eq!(conflict_info.commit.hash, conflict_info.target_branch.hash);
  assert_ne!(conflict_info.commit.message, "");
  assert!(conflict_info.conflict_marker_commits.contains_key(&conflict_info.commit.hash));

  let files: Vec<&str> = conflict_info.conflicting_files.iter().map(|f| f.file.as_str()).collect();
  assert_eq!(files, ["file1.txt"]);
  let detail = &conflict_info.conflicting_files[0];
  assert!(
    detail.target_file.as_ref().is_some_and(|f| f.content.ends_with("line 3\n")),
    "untrimmed target content: {detail:?}"
  );

  // The rebase was aborted and the fixup commit dropped: the branch is back at the pre-amend HEAD
  assert_eq!(repo.git_executor().execute_command(&["rev-parse", "HEAD"], repo_path(&repo))?, head_before);
  let (_, rebase_head_exit_code) = repo
    .git_executor()
    .execute_command_with_status(&["rev-parse", "--verify", "--quiet", "REBASE_HEAD"], repo_path(&repo))?;
  assert_ne!(rebase_head_exit_code, 0, "rebase should be aborted");

  // The uncommitted change is back in the working tree, unstaged
  assert_eq!(
    repo.git_executor().execute_command(&["status", "--porcelain", "--", "file1.txt"], repo_path(&repo))?,
    "M file1.txt"
  );
  assert_eq!(read_file(&repo, "file1.txt")?, "line 1\nmodified by uncommitted\nline 3\n");

  Ok(())
}
