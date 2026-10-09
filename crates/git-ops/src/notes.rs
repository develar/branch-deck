use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Mutex;
use tracing::instrument;

pub const PREFIX: &str = "v-commit-v1:";

/// Write `original -> new` commit mapping notes (`{PREFIX}{new}` attached to `original`) after a successful branch sync
#[instrument(skip(git_executor, notes, git_notes_mutex), fields(notes_count = notes.len()))]
pub fn write_commit_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes: &[(String, String)], git_notes_mutex: &Mutex<()>) -> Result<(), anyhow::Error> {
  if notes.is_empty() {
    return Ok(());
  }

  // Lock mutex for the entire batch operation
  let _lock = git_notes_mutex.lock().unwrap();

  // Write each note using git notes add
  for (original_oid, new_oid) in notes {
    let note_content = format!("{PREFIX}{new_oid}");

    // Use git notes add to properly create/update the note
    // -f flag forces overwrite if note already exists
    let args = vec!["notes", "add", "-f", "-m", &note_content, original_oid];

    git_executor.execute_command(&args, repo_path)?;
  }

  Ok(())
}
