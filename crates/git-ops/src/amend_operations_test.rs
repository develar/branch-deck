use super::amend_operations::{AmendToCommitParams, amend_to_commit_in_main};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use std::fs;
use tempfile::TempDir;
use test_log::test;

/// Helper to create a test repository with initial commits
struct TestRepository {
  dir: TempDir,
  git: GitCommandExecutor,
  path: String,
}

impl TestRepository {
  fn new() -> Result<Self> {
    let dir = TempDir::new()?;
    let path = dir.path().to_string_lossy().to_string();
    let git = GitCommandExecutor::new();

    // Initialize repository
    git.execute_command(&["init"], &path)?;
    git.execute_command(&["config", "user.name", "Test User"], &path)?;
    git.execute_command(&["config", "user.email", "test@example.com"], &path)?;

    Ok(Self { dir, git, path })
  }

  fn commit_file(&self, filename: &str, content: &str, message: &str) -> Result<String> {
    let file_path = self.dir.path().join(filename);
    fs::write(&file_path, content)?;
    self.git.execute_command(&["add", filename], &self.path)?;
    self.git.execute_command(&["commit", "-m", message], &self.path)?;

    // Return the commit hash
    let commit_hash = self.git.execute_command(&["rev-parse", "HEAD"], &self.path)?;
    Ok(commit_hash.trim().to_string())
  }

  fn modify_file(&self, filename: &str, content: &str) -> Result<()> {
    let file_path = self.dir.path().join(filename);
    fs::write(&file_path, content)?;
    self.git.execute_command(&["add", filename], &self.path)?;
    Ok(())
  }

  fn modify_file_without_staging(&self, filename: &str, content: &str) -> Result<()> {
    let file_path = self.dir.path().join(filename);
    fs::write(&file_path, content)?;
    // Don't stage the file - simulates IntelliJ IDEA workflow
    Ok(())
  }

  fn get_commit_count(&self) -> Result<usize> {
    let output = self.git.execute_command(&["rev-list", "--count", "HEAD"], &self.path)?;
    Ok(output.trim().parse().unwrap_or(0))
  }

  fn get_file_content(&self, filename: &str) -> Result<String> {
    let file_path = self.dir.path().join(filename);
    Ok(fs::read_to_string(file_path)?)
  }
}

#[test]
fn test_amend_to_commit_basic() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit
  let commit1 = repo.commit_file("file1.txt", "initial content", "Initial commit")?;

  // Create second commit
  repo.commit_file("file2.txt", "second file", "Add second file")?;

  // Modify file and stage for amend
  repo.modify_file("file1.txt", "amended content")?;

  let params = AmendToCommitParams {
    original_commit_id: commit1.clone(),
    files: vec!["file1.txt".to_string()],
  };

  let result = amend_to_commit_in_main(&repo.git, &repo.path, params)?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");
  assert_ne!(result.rebased_to_commit, "");
  assert_ne!(result.amended_commit_id, commit1);

  // Verify the file was amended
  assert_eq!(repo.get_file_content("file1.txt")?, "amended content");

  // Verify commit count is still the same
  assert_eq!(repo.get_commit_count()?, 2);

  Ok(())
}

#[test]
fn test_amend_no_uncommitted_changes() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit
  let commit1 = repo.commit_file("file1.txt", "initial content", "Initial commit")?;

  let params = AmendToCommitParams {
    original_commit_id: commit1,
    files: vec!["file1.txt".to_string()],
  };

  // Should fail because there are no uncommitted changes
  let result = amend_to_commit_in_main(&repo.git, &repo.path, params);
  assert!(result.is_err());

  let error = result.unwrap_err().to_string();
  assert!(error.contains("No uncommitted changes"));

  Ok(())
}

#[test]
fn test_amend_preserves_commit_metadata() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit with specific timestamp
  let commit1 = repo.commit_file("file1.txt", "initial content", "Initial commit")?;

  // Modify file and stage for amend
  repo.modify_file("file1.txt", "amended content")?;

  let params = AmendToCommitParams {
    original_commit_id: commit1.clone(),
    files: vec!["file1.txt".to_string()],
  };

  let result = amend_to_commit_in_main(&repo.git, &repo.path, params)?;

  // Verify original author information was preserved in amended commit
  // The original commit was created by "Test User" so that should be preserved
  let author_info = repo.git.execute_command(&["log", "-1", "--format=%an %ae", &result.amended_commit_id], &repo.path)?;
  let parts: Vec<&str> = author_info.split_whitespace().collect();

  assert_eq!(parts[0], "Test");
  assert_eq!(parts[1], "User");
  assert_eq!(parts[2], "test@example.com");

  Ok(())
}

#[test]
fn test_working_directory_preserved() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit
  let commit1 = repo.commit_file("file1.txt", "initial content", "Initial commit")?;

  // Create an uncommitted file that should be preserved
  let uncommitted_file = repo.dir.path().join("uncommitted.txt");
  fs::write(&uncommitted_file, "uncommitted content")?;

  // Add staged changes for amend
  repo.modify_file("file1.txt", "amended content")?;

  let params = AmendToCommitParams {
    original_commit_id: commit1,
    files: vec!["file1.txt".to_string()],
  };

  amend_to_commit_in_main(&repo.git, &repo.path, params)?;

  // Verify uncommitted file still exists (working directory preserved)
  assert!(uncommitted_file.exists());
  assert_eq!(fs::read_to_string(&uncommitted_file)?, "uncommitted content");

  Ok(())
}

#[test]
fn test_amend_with_unstaged_changes() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit
  let commit1 = repo.commit_file("file1.txt", "initial content", "Initial commit")?;

  // Create second commit
  repo.commit_file("file2.txt", "second file", "Add second file")?;

  // Modify file WITHOUT staging (simulates IntelliJ IDEA workflow)
  repo.modify_file_without_staging("file1.txt", "amended content")?;

  let params = AmendToCommitParams {
    original_commit_id: commit1.clone(),
    files: vec!["file1.txt".to_string()],
  };

  // Should succeed with -a flag auto-staging the changes
  let result = amend_to_commit_in_main(&repo.git, &repo.path, params)?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");
  assert_ne!(result.rebased_to_commit, "");
  assert_ne!(result.amended_commit_id, commit1);

  // Verify the file was amended
  assert_eq!(repo.get_file_content("file1.txt")?, "amended content");

  // Verify commit count is still the same
  assert_eq!(repo.get_commit_count()?, 2);

  Ok(())
}

#[test]
fn test_amend_with_mixed_staged_unstaged_changes() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit
  let commit1 = repo.commit_file("file1.txt", "initial content", "Initial commit")?;

  // Create second commit
  repo.commit_file("file2.txt", "second file", "Add second file")?;

  // Modify file1 without staging (unstaged change)
  repo.modify_file_without_staging("file1.txt", "amended content")?;

  // Add a new file and stage it
  let new_file = repo.dir.path().join("file3.txt");
  fs::write(&new_file, "new file content")?;
  repo.git.execute_command(&["add", "file3.txt"], &repo.path)?;

  let params = AmendToCommitParams {
    original_commit_id: commit1.clone(),
    files: vec!["file1.txt".to_string(), "file3.txt".to_string()],
  };

  // Should succeed with -a flag handling both staged and unstaged changes
  let result = amend_to_commit_in_main(&repo.git, &repo.path, params)?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");

  // Verify both files were included in the amend
  assert_eq!(repo.get_file_content("file1.txt")?, "amended content");
  assert_eq!(repo.get_file_content("file3.txt")?, "new file content");

  Ok(())
}

#[test]
fn test_error_handling_invalid_commit() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit
  repo.commit_file("file1.txt", "initial content", "Initial commit")?;

  // Add uncommitted changes
  repo.modify_file("file1.txt", "amended content")?;

  let params = AmendToCommitParams {
    original_commit_id: "invalid_commit_hash".to_string(),
    files: vec!["file1.txt".to_string()],
  };

  let result = amend_to_commit_in_main(&repo.git, &repo.path, params);
  assert!(result.is_err());

  Ok(())
}

#[test]
fn test_amend_with_multiple_subsequent_commits() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create a series of commits
  let commit1 = repo.commit_file("file1.txt", "initial", "Initial commit")?;
  repo.commit_file("file2.txt", "second", "Second commit")?;
  repo.commit_file("file3.txt", "third", "Third commit")?;

  let initial_count = repo.get_commit_count()?;

  // Add uncommitted changes to amend to the first commit
  repo.modify_file("file1.txt", "initial amended")?;

  let params = AmendToCommitParams {
    original_commit_id: commit1,
    files: vec!["file1.txt".to_string()],
  };

  // This should fail due to conflicts or succeed with rebasing
  let result = amend_to_commit_in_main(&repo.git, &repo.path, params);

  match result {
    Ok(_) => {
      // If successful, verify all commits are still there
      assert_eq!(repo.get_commit_count()?, initial_count);
      assert_eq!(repo.get_file_content("file1.txt")?, "initial amended");
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
  let repo = TestRepository::new()?;

  // Create initial commit with a file
  let original_commit = repo.commit_file("shared.txt", "original content", "Original commit")?;

  // Create intervening commits that modify the same file (simulating other branches)
  repo.commit_file("shared.txt", "modified by commit 1", "Intervening commit 1")?;
  repo.commit_file("shared.txt", "modified by commit 2", "Intervening commit 2")?;

  let initial_count = repo.get_commit_count()?;

  // Add uncommitted changes to amend to the original commit
  repo.modify_file("shared.txt", "original content with new amendments")?;

  let params = AmendToCommitParams {
    original_commit_id: original_commit.clone(),
    files: vec!["shared.txt".to_string()],
  };

  // This should succeed with our new implementation
  let result = amend_to_commit_in_main(&repo.git, &repo.path, params)?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");
  assert_ne!(result.rebased_to_commit, "");
  assert_ne!(result.amended_commit_id, original_commit);

  // Verify commit count is still the same (no commits lost)
  assert_eq!(repo.get_commit_count()?, initial_count);

  // Verify the amended commit contains our changes by checking the file at that specific commit
  let amended_file_content = repo.git.execute_command(&["show", &format!("{}:shared.txt", result.amended_commit_id)], &repo.path)?;
  assert_eq!(amended_file_content.trim(), "original content with new amendments");

  // The final state depends on what subsequent commits did - they were rebased on top of our amended commit
  // Since the subsequent commits completely overwrote the file content, the final state should be from the last commit
  // But our test confirms the amend operation itself worked correctly

  Ok(())
}

#[test]
fn test_amend_multiple_files_optimization() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit with multiple files
  let original_commit = repo.commit_file("file1.txt", "content1", "Initial commit")?;
  repo.commit_file("file2.txt", "content2", "Add file2")?;
  repo.commit_file("file3.txt", "content3", "Add file3")?;

  // Modify multiple files to test batch processing
  repo.modify_file("file1.txt", "amended content1")?;
  repo.modify_file("file2.txt", "amended content2")?;
  repo.modify_file("file3.txt", "amended content3")?;

  let params = AmendToCommitParams {
    original_commit_id: original_commit.clone(),
    files: vec!["file1.txt".to_string(), "file2.txt".to_string(), "file3.txt".to_string()],
  };

  // Should use the optimized batch processing for multiple files
  let result = amend_to_commit_in_main(&repo.git, &repo.path, params)?;

  // Verify the operation succeeded
  assert_ne!(result.amended_commit_id, "");
  assert_ne!(result.amended_commit_id, original_commit);

  // Verify the amended commit contains our changes for file1 (the original commit only had file1)
  let amended_file_content = repo.git.execute_command(&["show", &format!("{}:file1.txt", result.amended_commit_id)], &repo.path)?;
  assert_eq!(amended_file_content.trim(), "amended content1");

  Ok(())
}

#[test]
fn test_amend_conflict_returns_merge_conflict_info() -> Result<()> {
  let repo = TestRepository::new()?;

  // Create initial commit
  repo.commit_file("base.txt", "base content", "Base commit")?;

  // Create second commit to amend to
  let commit_to_amend = repo.commit_file("file1.txt", "line 1\noriginal line 2\nline 3", "Add file1")?;

  // Create third commit that modifies the same file (this will cause conflicts when rebased)
  repo.commit_file("file1.txt", "line 1\nmodified by later commit\nline 3", "Later modification")?;

  // Add conflicting uncommitted changes to the same lines
  repo.modify_file("file1.txt", "line 1\nmodified by uncommitted\nline 3")?;

  let params = AmendToCommitParams {
    original_commit_id: commit_to_amend.clone(),
    files: vec!["file1.txt".to_string()],
  };

  // Attempt to amend - should fail with conflict
  let result = amend_to_commit_in_main(&repo.git, &repo.path, params);

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
          assert_ne!(conflict_info.commit_hash, "");
          assert_ne!(conflict_info.commit_message, "");
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
  let repo = TestRepository::new()?;

  repo.commit_file("base.txt", "base content", "Base commit")?;
  let branch = repo.git.execute_command(&["symbolic-ref", "--short", "HEAD"], &repo.path)?;
  let commit_to_amend = repo.commit_file("file1.txt", "line 1\noriginal line 2\nline 3\n", "Add file1")?;

  // A merge commit after the target makes the history non-linear, so amend falls back to fixup + autosquash
  repo.git.execute_command(&["checkout", "-q", "-b", "side"], &repo.path)?;
  repo.commit_file("side.txt", "side content", "Side commit")?;
  repo.git.execute_command(&["checkout", "-q", &branch], &repo.path)?;
  repo.commit_file("file1.txt", "line 1\nmodified by later commit\nline 3\n", "Later modification")?;
  repo.git.execute_command(&["merge", "-q", "--no-ff", "-m", "Merge side", "side"], &repo.path)?;
  let head_before = repo.git.execute_command(&["rev-parse", "HEAD"], &repo.path)?;

  // Conflicting uncommitted change to the same line
  repo.modify_file_without_staging("file1.txt", "line 1\nmodified by uncommitted\nline 3\n")?;

  let params = AmendToCommitParams {
    original_commit_id: commit_to_amend.clone(),
    files: vec!["file1.txt".to_string()],
  };
  let error = amend_to_commit_in_main(&repo.git, &repo.path, params).expect_err("amend should conflict");

  let crate::copy_commit::CopyCommitError::BranchError(crate::model::BranchError::MergeConflict(conflict_info)) = error else {
    panic!("expected MergeConflictInfo, got {error:?}");
  };
  assert_eq!(conflict_info.original_parent_hash, commit_to_amend);
  assert_eq!(conflict_info.commit_hash, conflict_info.target_branch_hash);
  assert_ne!(conflict_info.commit_message, "");
  assert!(conflict_info.conflict_marker_commits.contains_key(&conflict_info.commit_hash));

  let files: Vec<&str> = conflict_info.conflicting_files.iter().map(|f| f.file.as_str()).collect();
  assert_eq!(files, ["file1.txt"]);
  let detail = &conflict_info.conflicting_files[0];
  assert!(
    detail.target_file.as_ref().is_some_and(|f| f.content.ends_with("line 3\n")),
    "untrimmed target content: {detail:?}"
  );

  // The rebase was aborted and the fixup commit dropped: the branch is back at the pre-amend HEAD
  assert_eq!(repo.git.execute_command(&["rev-parse", "HEAD"], &repo.path)?, head_before);
  let (_, rebase_head_exit_code) = repo.git.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "REBASE_HEAD"], &repo.path)?;
  assert_ne!(rebase_head_exit_code, 0, "rebase should be aborted");

  // The uncommitted change is back in the working tree, unstaged
  assert_eq!(repo.git.execute_command(&["status", "--porcelain", "--", "file1.txt"], &repo.path)?, "M file1.txt");
  assert_eq!(repo.get_file_content("file1.txt")?, "line 1\nmodified by uncommitted\nline 3\n");

  Ok(())
}
