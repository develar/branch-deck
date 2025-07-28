use crate::git_command::GitCommandExecutor;
use crate::notes::{write_commit_notes, CommitNoteInfo, PREFIX};
use pretty_assertions::assert_eq;
use std::process::Command;
use std::sync::Mutex;
use test_utils::git_test_utils::TestRepo;

#[test]
fn test_write_commit_notes_empty() {
  let git_executor = GitCommandExecutor::new();
  let mutex = Mutex::new(());
  let result = write_commit_notes(&git_executor, "/fake/path", vec![], &mutex);
  assert!(result.is_ok());
}

#[test]
fn test_write_single_commit_note() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();
  let mutex = Mutex::new(());

  // Create test commits
  let commit1_hash = test_repo.create_commit("Initial commit", "test.txt", "initial content");
  let commit2_hash = test_repo.create_commit("Second commit", "test.txt", "updated content");

  // Write a note mapping commit1 to commit2
  let notes = vec![CommitNoteInfo {
    original_oid: commit1_hash.clone(),
    new_oid: commit2_hash.clone(),
    author: "Test Author".to_string(),
    author_email: "test@example.com".to_string(),
  }];

  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), notes, &mutex);
  assert!(result.is_ok());

  // Verify the note was written
  let output = Command::new("git")
    .args(["--no-pager", "notes", "show", &commit1_hash])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  assert!(output.status.success(), "Failed to read note: {}", String::from_utf8_lossy(&output.stderr));
  let note_content = String::from_utf8_lossy(&output.stdout);
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit2_hash}"));
}

#[test]
fn test_write_multiple_commit_notes() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();
  let mutex = Mutex::new(());

  // Create multiple test commits
  let mut original_commits = Vec::new();
  let mut new_commits = Vec::new();
  let mut notes = Vec::new();

  for i in 0..7 {
    let original = test_repo.create_commit(&format!("Original commit {i}"), &format!("file{i}.txt"), &format!("content {i}"));
    original_commits.push(original.clone());

    let new = test_repo.create_commit(&format!("New commit {i}"), &format!("file{i}.txt"), &format!("new content {i}"));
    new_commits.push(new.clone());

    notes.push(CommitNoteInfo {
      original_oid: original,
      new_oid: new,
      author: format!("Author {i}"),
      author_email: format!("author{i}@example.com"),
    });
  }

  // Write all notes at once
  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), notes, &mutex);
  assert!(result.is_ok());

  // Verify all notes were written correctly
  for i in 0..7 {
    let output = Command::new("git")
      .args(["--no-pager", "notes", "show", &original_commits[i]])
      .current_dir(test_repo.path())
      .output()
      .unwrap();

    assert!(output.status.success(), "Failed to read note for commit {}: {}", i, String::from_utf8_lossy(&output.stderr));
    let note_content = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
      note_content.trim(),
      format!("{}{}", PREFIX, new_commits[i]),
      "Note for commit {} should map to correct new commit",
      i
    );
  }
}

#[test]
fn test_write_commit_notes_with_special_characters() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();
  let mutex = Mutex::new(());

  // Create commits
  let commit1 = test_repo.create_commit("Commit with special chars: 'quotes' and \"double quotes\"", "test.txt", "content");
  let commit2 = test_repo.create_commit("Another commit", "test.txt", "new content");

  // Write note with author containing special characters
  let notes = vec![CommitNoteInfo {
    original_oid: commit1.clone(),
    new_oid: commit2.clone(),
    author: "Author with 'quotes' and spaces".to_string(),
    author_email: "special.chars+test@example.com".to_string(),
  }];

  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), notes, &mutex);
  assert!(result.is_ok());

  // Verify the note
  let output = Command::new("git")
    .args(["--no-pager", "notes", "show", &commit1])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  assert!(output.status.success());
  let note_content = String::from_utf8_lossy(&output.stdout);
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit2}"));
}

#[test]
fn test_concurrent_note_writing() {
  use std::sync::Arc;

  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();
  let mutex = Arc::new(Mutex::new(()));

  // Create a base commit
  test_repo.create_commit("Base commit", "test.txt", "base");

  // Create multiple note sets and write them with shared mutex
  let mut all_notes = Vec::new();
  let mut expected_mappings = Vec::new();

  for i in 0..3 {
    // Create commits for this batch
    let original = test_repo.create_commit(&format!("Original {i}"), &format!("file{i}.txt"), &format!("content {i}"));
    let new = test_repo.create_commit(&format!("New {i}"), &format!("file{i}.txt"), &format!("new content {i}"));

    all_notes.push(CommitNoteInfo {
      original_oid: original.clone(),
      new_oid: new.clone(),
      author: format!("Batch {i}"),
      author_email: format!("batch{i}@example.com"),
    });

    expected_mappings.push((original, new));
  }

  // Write all notes - the mutex ensures they don't interfere
  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), all_notes, &mutex);
  assert!(result.is_ok());

  // Verify all notes were written correctly
  for (i, (original, new)) in expected_mappings.iter().enumerate() {
    let output = Command::new("git")
      .args(["--no-pager", "notes", "show", original])
      .current_dir(test_repo.path())
      .output()
      .unwrap();

    assert!(output.status.success());
    let note_content = String::from_utf8_lossy(&output.stdout);
    assert_eq!(note_content.trim(), format!("{PREFIX}{new}"), "Batch {} should have written its note correctly", i);
  }
}

#[test]
fn test_write_commit_notes_overwrites_existing() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();
  let mutex = Mutex::new(());

  // Create commits
  let commit1 = test_repo.create_commit("First", "test.txt", "v1");
  let commit2 = test_repo.create_commit("Second", "test.txt", "v2");
  let commit3 = test_repo.create_commit("Third", "test.txt", "v3");

  // Write initial note
  let notes = vec![CommitNoteInfo {
    original_oid: commit1.clone(),
    new_oid: commit2.clone(),
    author: "Author".to_string(),
    author_email: "test@example.com".to_string(),
  }];

  write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), notes, &mutex).unwrap();

  // Verify initial note
  let output = Command::new("git")
    .args(["--no-pager", "notes", "show", &commit1])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  let note_content = String::from_utf8_lossy(&output.stdout);
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit2}"));

  // Overwrite with new note
  let notes = vec![CommitNoteInfo {
    original_oid: commit1.clone(),
    new_oid: commit3.clone(),
    author: "Author".to_string(),
    author_email: "test@example.com".to_string(),
  }];

  write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), notes, &mutex).unwrap();

  // Verify note was overwritten
  let output = Command::new("git")
    .args(["--no-pager", "notes", "show", &commit1])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  let note_content = String::from_utf8_lossy(&output.stdout);
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit3}"));
}

#[test]
fn test_write_notes_batch_mode_edge_cases() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();
  let mutex = Mutex::new(());

  // Test with empty lines and newlines in batch
  let mut notes = Vec::new();
  let mut commits = Vec::new();

  // Create commits with various edge cases
  for i in 0..3 {
    let original = test_repo.create_commit(&format!("Commit {i}"), &format!("test{i}.txt"), &format!("content\nwith\nnewlines\n{i}"));
    let new = test_repo.create_commit(&format!("New {i}"), &format!("test{i}.txt"), "updated");

    commits.push((original.clone(), new.clone()));
    notes.push(CommitNoteInfo {
      original_oid: original,
      new_oid: new,
      author: "Test".to_string(),
      author_email: "test@example.com".to_string(),
    });
  }

  // Write notes
  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), notes, &mutex);
  assert!(result.is_ok());

  // Verify all notes
  for (original, new) in commits {
    let output = Command::new("git")
      .args(["--no-pager", "notes", "show", &original])
      .current_dir(test_repo.path())
      .output()
      .unwrap();

    assert!(output.status.success());
    let note_content = String::from_utf8_lossy(&output.stdout);
    assert_eq!(note_content.trim(), format!("{PREFIX}{new}"));
  }
}
