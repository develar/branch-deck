use super::amend_operations::{AmendResult, AmendToCommitParams, amend_to_commit_in_main};
use super::copy_commit::CopyCommitError;
use super::rewrite_test_utils::{assert_no_temp_refs, commit_count, log_format, read_file, repo_path, show, stage_file, status, write_file};
use anyhow::Result;
use std::fs;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

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
fn test_amend_is_refused_when_later_commits_changed_the_same_lines() -> Result<()> {
  let repo = TestRepo::new();
  let original_commit = repo.create_commit("Original commit", "shared.txt", "original content");
  repo.create_commit("Intervening commit 1", "shared.txt", "modified by commit 1");
  repo.create_commit("Intervening commit 2", "shared.txt", "modified by commit 2");
  let head_before = repo.head();

  // the change is made on top of HEAD, so it conflicts with the commits after the target
  stage_file(&repo, "shared.txt", "original content with new amendments")?;

  let error = amend(&repo, &original_commit, &["shared.txt"]).expect_err("amend should be refused");

  assert!(error.to_string().contains("conflict"), "{error}");
  assert_eq!(repo.head(), head_before);
  assert_eq!(read_file(&repo, "shared.txt")?, "original content with new amendments");
  assert_eq!(status(&repo)?, "M  shared.txt");
  Ok(())
}

#[test]
fn test_amend_multiple_files() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Add file1", "file1.txt", "content1");
  let original_commit = repo.create_commit("Add file2", "file2.txt", "content2");
  repo.create_commit("Add file3", "file3.txt", "content3");

  stage_file(&repo, "file1.txt", "amended content1")?;
  write_file(&repo, "file2.txt", "amended content2")?;

  let result = amend(&repo, &original_commit, &["file1.txt", "file2.txt"])?;

  assert_eq!(show(&repo, &result.amended_commit_id, "file1.txt")?, "amended content1");
  assert_eq!(show(&repo, &result.amended_commit_id, "file2.txt")?, "amended content2");
  assert_eq!(show(&repo, "HEAD", "file3.txt")?, "content3");
  assert_eq!(status(&repo)?, "");
  Ok(())
}

#[test]
fn test_amend_conflict_changes_nothing() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Base commit", "base.txt", "base content");
  let commit_to_amend = repo.create_commit("Add file1", "file1.txt", "line 1\noriginal line 2\nline 3");
  repo.create_commit("Later modification", "file1.txt", "line 1\nmodified by later commit\nline 3");
  let head_before = repo.head();
  write_file(&repo, "file1.txt", "line 1\nmodified by uncommitted\nline 3")?;

  let error = amend(&repo, &commit_to_amend, &["file1.txt"]).expect_err("amend should conflict");

  let CopyCommitError::BranchError(crate::model::BranchError::Generic(message)) = &error else {
    panic!("expected a conflict error, got {error:?}");
  };
  assert!(message.contains("conflict"), "{message}");
  assert_eq!(repo.head(), head_before);
  assert_eq!(read_file(&repo, "file1.txt")?, "line 1\nmodified by uncommitted\nline 3");
  assert_eq!(status(&repo)?, " M file1.txt");
  assert_no_temp_refs(&repo)?;
  Ok(())
}

#[test]
fn amend_refuses_range_with_merges() -> Result<()> {
  let repo = TestRepo::new();

  repo.create_commit("Base commit", "base.txt", "base content");
  let branch = repo.git_executor().execute_command(&["symbolic-ref", "--short", "HEAD"], repo_path(&repo))?;
  let commit_to_amend = repo.create_commit("Add file1", "file1.txt", "line 1\noriginal line 2\nline 3\n");

  // a merge commit after the target: Branch Deck rewrites only linear history
  repo.git_executor().execute_command(&["checkout", "-q", "-b", "side"], repo_path(&repo))?;
  repo.create_commit("Side commit", "side.txt", "side content");
  repo.git_executor().execute_command(&["checkout", "-q", &branch], repo_path(&repo))?;
  repo.create_commit("Later modification", "file2.txt", "other");
  repo
    .git_executor()
    .execute_command(&["merge", "-q", "--no-ff", "-m", "Merge side", "side"], repo_path(&repo))?;
  let head_before = repo.head();
  write_file(&repo, "file1.txt", "line 1\namended line 2\nline 3\n")?;

  let error = amend(&repo, &commit_to_amend, &["file1.txt"]).expect_err("amend should be refused");

  assert!(error.to_string().contains("merge commit"), "{error}");
  assert_eq!(repo.head(), head_before);
  assert_eq!(status(&repo)?, " M file1.txt");
  assert_eq!(read_file(&repo, "file1.txt")?, "line 1\namended line 2\nline 3\n");
  Ok(())
}

#[test]
fn amend_only_selected_files() -> Result<()> {
  let repo = TestRepo::new();
  let target = repo.create_commit("Add files", "file1.txt", "one");
  repo.create_commit("Add file2", "file2.txt", "two");
  write_file(&repo, "file1.txt", "one amended")?;
  write_file(&repo, "file2.txt", "two modified")?;

  let result = amend(&repo, &target, &["file1.txt"])?;

  assert_eq!(show(&repo, &result.amended_commit_id, "file1.txt")?, "one amended");
  assert_eq!(show(&repo, "HEAD", "file2.txt")?, "two");
  assert_eq!(status(&repo)?, " M file2.txt");
  assert_eq!(read_file(&repo, "file2.txt")?, "two modified");
  Ok(())
}

#[test]
fn amend_head_only_selected_files() -> Result<()> {
  let repo = TestRepo::new();
  repo.create_commit("Add file1", "file1.txt", "one");
  let head = repo.create_commit("Add file2", "file2.txt", "two");
  write_file(&repo, "file1.txt", "one modified")?;
  write_file(&repo, "file2.txt", "two amended")?;

  let result = amend(&repo, &head, &["file2.txt"])?;

  assert_eq!(result.amended_commit_id, result.rebased_to_commit);
  assert_eq!(show(&repo, "HEAD", "file2.txt")?, "two amended");
  assert_eq!(show(&repo, "HEAD", "file1.txt")?, "one");
  assert_eq!(status(&repo)?, " M file1.txt");
  Ok(())
}

#[test]
fn amend_non_head_applies_only_delta() -> Result<()> {
  let repo = TestRepo::new();
  let target = repo.create_commit("Add config", "config.txt", "a\nb\nc\nd\ne\nf\ng\n");
  // a later commit changes another part of the same file
  repo.create_commit("Change the end", "config.txt", "a\nb\nc\nd\ne\nf\nG\n");
  write_file(&repo, "config.txt", "A\nb\nc\nd\ne\nf\nG\n")?;

  let result = amend(&repo, &target, &["config.txt"])?;

  // the target gets only the working-tree delta, not the later commit's change
  assert_eq!(show(&repo, &result.amended_commit_id, "config.txt")?, "A\nb\nc\nd\ne\nf\ng");
  assert_eq!(show(&repo, "HEAD", "config.txt")?, "A\nb\nc\nd\ne\nf\nG");
  assert_eq!(status(&repo)?, "");
  assert_eq!(commit_count(&repo)?, 2);
  Ok(())
}

#[test]
fn amend_keeps_unrelated_staged_changes() -> Result<()> {
  let repo = TestRepo::new();
  let target = repo.create_commit("Add file1", "file1.txt", "one");
  repo.create_commit("Add file2", "file2.txt", "two");
  write_file(&repo, "file1.txt", "one amended")?;
  stage_file(&repo, "file2.txt", "two staged")?;
  write_file(&repo, "file2.txt", "two staged and edited")?;

  amend(&repo, &target, &["file1.txt"])?;

  // file2 is still staged as before, with its unstaged edit on top
  assert_eq!(status(&repo)?, "MM file2.txt");
  assert_eq!(repo.git_executor().execute_command(&["show", ":file2.txt"], repo_path(&repo))?, "two staged");
  assert_eq!(read_file(&repo, "file2.txt")?, "two staged and edited");
  Ok(())
}

#[test]
fn amend_keeps_the_author_and_message_and_records_the_reflog() -> Result<()> {
  let repo = TestRepo::new();
  let target = repo.create_commit("Add file1\n\nWith a body", "file1.txt", "one");
  repo.create_commit("Add file2", "file2.txt", "two");
  let author_before = log_format(&repo, &target, "%an <%ae> %ad")?;
  write_file(&repo, "file1.txt", "one amended")?;

  let result = amend(&repo, &target, &["file1.txt"])?;

  assert_eq!(log_format(&repo, &result.amended_commit_id, "%an <%ae> %ad")?, author_before);
  assert_eq!(log_format(&repo, &result.amended_commit_id, "%B")?, "Add file1\n\nWith a body");
  let reflog = repo.git_executor().execute_command(&["reflog", "-1", "--format=%gs"], repo_path(&repo))?;
  assert!(reflog.starts_with("branch-deck: amend"), "{reflog}");
  assert_no_temp_refs(&repo)?;
  Ok(())
}
