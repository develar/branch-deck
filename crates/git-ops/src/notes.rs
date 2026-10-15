use crate::commit_writer::write_notes_to_refs;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

pub const PREFIX: &str = "v-commit-v1:";

/// The virtual branch commit a note (`{PREFIX}{new}`) maps its commit to; `None` for any other note. Only the first line
/// counts: `%N` may hold the notes of other refs after it
pub fn mapped_commit_id(note: &str) -> Option<&str> {
  note.trim_start().lines().next()?.strip_prefix(PREFIX).map(str::trim)
}

/// The notes ref the mapping notes live on (`core.notesRef`, normally `refs/notes/commits`)
pub fn notes_ref(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String, anyhow::Error> {
  git_executor.execute_command(&["notes", "get-ref"], repo_path)
}

/// Write `original -> new` commit mapping notes (`{PREFIX}{new}` attached to `original`) after a sync, all in one notes
/// commit on the default notes ref (`core.notesRef`, normally `refs/notes/commits`)
pub fn write_commit_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes: &[(String, String)]) -> Result<(), anyhow::Error> {
  write_commit_notes_with(git_executor, repo_path, notes, None)
}

/// `write_commit_notes` plus the notes of another ref (`(notes ref, (target, content))`) in the same `fast-import` run
#[instrument(skip(git_executor, notes, other), fields(notes_count = notes.len()))]
pub fn write_commit_notes_with(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  notes: &[(String, String)],
  other: Option<(&str, &[(String, String)])>,
) -> Result<(), anyhow::Error> {
  let mut refs = Vec::with_capacity(2);
  let mapping_ref;
  let mapping_notes: Vec<(String, String)>;
  if !notes.is_empty() {
    mapping_ref = notes_ref(git_executor, repo_path)?;
    mapping_notes = notes.iter().map(|(original_oid, new_oid)| (original_oid.clone(), format!("{PREFIX}{new_oid}"))).collect();
    refs.push((mapping_ref.as_str(), mapping_notes.as_slice()));
  }
  refs.extend(other);
  write_notes_to_refs(git_executor, repo_path, &refs)
}
