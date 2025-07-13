use git2::{Oid, Repository, Signature};
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::{debug, instrument};

pub const PREFIX: &str = "v-commit-v1:";

/// Information needed to write a git note after successful branch sync
#[derive(Debug)]
pub struct CommitNoteInfo {
  pub original_oid: Oid,
  pub new_oid: Oid,
  pub author: String,
  pub author_email: String,
}

/// Write git notes for all commits in a batch
/// This is called after all commits in a branch are successfully processed
#[instrument(skip(repo, git_notes_mutex), fields(notes_count = notes.len()))]
pub fn write_commit_notes(repo: &Repository, notes: Vec<CommitNoteInfo>, git_notes_mutex: &Mutex<()>) -> Result<(), anyhow::Error> {
  if notes.is_empty() {
    return Ok(());
  }

  // Lock mutex for the entire batch operation
  let _lock = git_notes_mutex.lock().unwrap();

  // Create committer signature once
  let committer = Signature::now("branch-deck", "branch-deck@example.com")?;

  // Group notes by author email to reuse signatures
  let mut authors_cache: HashMap<String, Signature> = HashMap::new();

  for note_info in notes {
    // Get or create author signature
    let author = authors_cache
      .entry(note_info.author_email.clone())
      .or_insert_with(|| Signature::new(&note_info.author, &note_info.author_email, &git2::Time::new(0, 0)).expect("Failed to create author signature"));

    repo.note(author, &committer, None, note_info.original_oid, &format!("{}{}", PREFIX, note_info.new_oid), true)?;
  }

  Ok(())
}

/// Find an existing commit by checking git notes
/// Returns the mapped commit hash if found
#[instrument(skip(repo), fields(commit_id = %commit_id))]
pub fn find_existing_commit(repo: &Repository, commit_id: Oid) -> Option<String> {
  // No mutex needed for read-only operations

  if let Ok(note) = repo.find_note(None, commit_id) {
    if let Some(message) = note.message() {
      if let Some(hash) = message.strip_prefix(PREFIX) {
        if !hash.is_empty() {
          debug!(original_id = %commit_id, mapped_id = %hash, "found existing commit mapping");
          return Some(hash.to_string());
        }
      }
    }
  }

  None
}
