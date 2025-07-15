use crate::git::git_command::GitCommandExecutor;
use std::sync::Mutex;
use tracing::instrument;

pub const PREFIX: &str = "v-commit-v1:";

/// Information needed to write a git note after successful branch sync
#[derive(Debug)]
pub struct CommitNoteInfo {
  pub original_oid: String,
  pub new_oid: String,
  pub author: String,
  pub author_email: String,
}

/// Write git notes for all commits in a batch using git CLI for better performance
/// Uses git update-ref --stdin for atomic batch updates
#[instrument(skip(git_executor, git_notes_mutex), fields(notes_count = notes.len()))]
pub fn write_commit_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes: Vec<CommitNoteInfo>, git_notes_mutex: &Mutex<()>) -> Result<(), anyhow::Error> {
  if notes.is_empty() {
    return Ok(());
  }

  // Lock mutex for the entire batch operation
  let _lock = git_notes_mutex.lock().unwrap();

  // Optimize: Create all blobs in a single batch using --batch mode
  // Format: each line is the content followed by a blank line
  let mut batch_input = String::new();
  for note_info in &notes {
    let note_content = format!("{}{}", PREFIX, note_info.new_oid);
    batch_input.push_str(&note_content);
    batch_input.push('\n');
  }

  // Create all blobs at once using --batch
  let blob_args = vec!["hash-object", "-w", "--stdin"];

  let blob_oids_output = git_executor.execute_command_with_input(&blob_args, repo_path, &batch_input)?;
  let blob_oids: Vec<&str> = blob_oids_output.lines().collect();

  // Verify we got the expected number of blob OIDs
  if blob_oids.len() != notes.len() {
    return Err(anyhow::anyhow!("Expected {} blob OIDs but got {}", notes.len(), blob_oids.len()));
  }

  // Build batch update commands for git update-ref
  // Using the --stdin format for atomic updates
  let mut ref_updates = String::new();
  for (i, note_info) in notes.iter().enumerate() {
    let blob_oid = blob_oids[i].trim();
    // Format: update refs/notes/commits/<commit> <blob_oid>
    ref_updates.push_str(&format!("update refs/notes/commits/{} {}\n", note_info.original_oid, blob_oid));
  }

  // Execute all ref updates in a single atomic transaction
  let update_args = vec!["update-ref", "--stdin"];

  git_executor.execute_command_with_input(&update_args, repo_path, &ref_updates)?;

  Ok(())
}
