use crate::commit_writer::write_notes;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Mutex;
use tracing::instrument;

pub const PREFIX: &str = "v-commit-v1:";

/// Write `original -> new` commit mapping notes (`{PREFIX}{new}` attached to `original`) after a successful branch sync,
/// all in one notes commit on the default notes ref (`core.notesRef`, normally `refs/notes/commits`)
#[instrument(skip(git_executor, notes, git_notes_mutex), fields(notes_count = notes.len()))]
pub fn write_commit_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes: &[(String, String)], git_notes_mutex: &Mutex<()>) -> Result<(), anyhow::Error> {
  if notes.is_empty() {
    return Ok(());
  }

  // the branches of one sync write their notes one after another
  let _lock = git_notes_mutex.lock().unwrap();

  let notes_ref = git_executor.execute_command(&["notes", "get-ref"], repo_path)?;
  let notes: Vec<(String, String)> = notes.iter().map(|(original_oid, new_oid)| (original_oid.clone(), format!("{PREFIX}{new_oid}"))).collect();
  write_notes(git_executor, repo_path, &notes_ref, &notes)
}
