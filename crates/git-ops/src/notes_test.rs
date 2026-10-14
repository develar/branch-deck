use crate::notes::{PREFIX, write_commit_notes};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

#[test]
fn test_write_commit_notes_empty() {
  let git_executor = GitCommandExecutor::new();
  let result = write_commit_notes(&git_executor, "/fake/path", &[]);
  assert!(result.is_ok());
}

#[test]
fn test_write_single_commit_note() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create test commits
  let commit1_hash = test_repo.create_commit("Initial commit", "test.txt", "initial content");
  let commit2_hash = test_repo.create_commit("Second commit", "test.txt", "updated content");

  // Write a note mapping commit1 to commit2
  let notes = vec![(commit1_hash.clone(), commit2_hash.clone())];

  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), &notes);
  assert!(result.is_ok());

  // Verify the note was written
  let note_content = test_repo.show_note(&commit1_hash).unwrap_or_else(|e| panic!("Failed to read note: {e}"));
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit2_hash}"));
}

#[test]
fn test_write_multiple_commit_notes() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create multiple test commits
  let mut original_commits = Vec::new();
  let mut new_commits = Vec::new();
  let mut notes = Vec::new();

  for i in 0..7 {
    let original = test_repo.create_commit(&format!("Original commit {i}"), &format!("file{i}.txt"), &format!("content {i}"));
    original_commits.push(original.clone());

    let new = test_repo.create_commit(&format!("New commit {i}"), &format!("file{i}.txt"), &format!("new content {i}"));
    new_commits.push(new.clone());

    notes.push((original, new));
  }

  // Write all notes at once
  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), &notes);
  assert!(result.is_ok());

  // Verify all notes were written correctly
  for i in 0..7 {
    let note_content = test_repo
      .show_note(&original_commits[i])
      .unwrap_or_else(|e| panic!("Failed to read note for commit {i}: {e}"));
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

  // Create commits
  let commit1 = test_repo.create_commit("Commit with special chars: 'quotes' and \"double quotes\"", "test.txt", "content");
  let commit2 = test_repo.create_commit("Another commit", "test.txt", "new content");

  // Write note with author containing special characters
  let notes = vec![(commit1.clone(), commit2.clone())];

  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), &notes);
  assert!(result.is_ok());

  // Verify the note
  let note_content = test_repo.show_note(&commit1).unwrap();
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit2}"));
}

#[test]
fn test_writes_many_notes_in_one_call() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create a base commit
  test_repo.create_commit("Base commit", "test.txt", "base");

  // Create multiple note sets
  let mut all_notes = Vec::new();
  let mut expected_mappings = Vec::new();

  for i in 0..3 {
    // Create commits for this batch
    let original = test_repo.create_commit(&format!("Original {i}"), &format!("file{i}.txt"), &format!("content {i}"));
    let new = test_repo.create_commit(&format!("New {i}"), &format!("file{i}.txt"), &format!("new content {i}"));

    all_notes.push((original.clone(), new.clone()));

    expected_mappings.push((original, new));
  }

  // Write all notes in one notes commit
  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), &all_notes);
  assert!(result.is_ok());

  // Verify all notes were written correctly
  for (i, (original, new)) in expected_mappings.iter().enumerate() {
    let note_content = test_repo.show_note(original).unwrap();
    assert_eq!(note_content.trim(), format!("{PREFIX}{new}"), "Batch {} should have written its note correctly", i);
  }
}

#[test]
fn test_write_commit_notes_overwrites_existing() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create commits
  let commit1 = test_repo.create_commit("First", "test.txt", "v1");
  let commit2 = test_repo.create_commit("Second", "test.txt", "v2");
  let commit3 = test_repo.create_commit("Third", "test.txt", "v3");

  // Write initial note
  let notes = vec![(commit1.clone(), commit2.clone())];

  write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), &notes).unwrap();

  // Verify initial note
  let note_content = test_repo.show_note(&commit1).unwrap();
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit2}"));

  // Overwrite with new note
  let notes = vec![(commit1.clone(), commit3.clone())];

  write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), &notes).unwrap();

  // Verify note was overwritten
  let note_content = test_repo.show_note(&commit1).unwrap();
  assert_eq!(note_content.trim(), format!("{PREFIX}{commit3}"));
}

#[test]
fn test_write_notes_batch_mode_edge_cases() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Test with empty lines and newlines in batch
  let mut notes = Vec::new();
  let mut commits = Vec::new();

  // Create commits with various edge cases
  for i in 0..3 {
    let original = test_repo.create_commit(&format!("Commit {i}"), &format!("test{i}.txt"), &format!("content\nwith\nnewlines\n{i}"));
    let new = test_repo.create_commit(&format!("New {i}"), &format!("test{i}.txt"), "updated");

    commits.push((original.clone(), new.clone()));
    notes.push((original, new));
  }

  // Write notes
  let result = write_commit_notes(&git_executor, test_repo.path().to_str().unwrap(), &notes);
  assert!(result.is_ok());

  // Verify all notes
  for (original, new) in commits {
    let note_content = test_repo.show_note(&original).unwrap();
    assert_eq!(note_content.trim(), format!("{PREFIX}{new}"));
  }
}
