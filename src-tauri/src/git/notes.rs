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
#[instrument(skip(git_executor, git_notes_mutex), fields(notes_count = notes.len()))]
pub fn write_commit_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes: Vec<CommitNoteInfo>, git_notes_mutex: &Mutex<()>) -> Result<(), anyhow::Error> {
  if notes.is_empty() {
    return Ok(());
  }

  // Lock mutex for the entire batch operation
  let _lock = git_notes_mutex.lock().unwrap();

  // Write each note using git notes add
  for note_info in notes {
    let note_content = format!("{}{}", PREFIX, note_info.new_oid);

    // Use git notes add to properly create/update the note
    // -f flag forces overwrite if note already exists
    let args = vec!["notes", "add", "-f", "-m", &note_content, &note_info.original_oid];

    git_executor.execute_command(&args, repo_path)?;
  }

  Ok(())
}
