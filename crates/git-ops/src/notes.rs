use crate::commit_writer::{NewCommit, NotesOfWritten, StoreCommit, WrittenCommits, write_commits_with_notes, write_notes_to_refs};
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::instrument;

pub const PREFIX: &str = "v-commit-v1:";
/// The line of a mapping note that names what the virtual commit was copied from: `<parent tree> <tree>` of the
/// original. A note copied onto a commit with other trees (`git notes copy`, `notes.rewriteRef`) no longer matches it.
pub const SOURCE_PREFIX: &str = "v-commit-source-v1:";

/// The trees a virtual commit was copied from: the original's parent tree and its tree
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSource {
  pub parent_tree: String,
  pub tree: String,
}

/// A mapping note to write: `original` is copied as `mapped` from `source`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingNote {
  pub original: String,
  pub mapped: String,
  pub source: NoteSource,
}

impl MappingNote {
  /// The note text: the mapping line, then the source line (old binaries read only the first)
  pub fn content(&self) -> String {
    format!("{PREFIX}{}\n{SOURCE_PREFIX}{} {}", self.mapped, self.source.parent_tree, self.source.tree)
  }
}

/// The virtual branch commit a note maps its commit to (`{PREFIX}{new}`); `None` for any other note. The line is found
/// by its prefix: `notes.rewriteMode=cat_sort_uniq` sorts the lines, and `%N` may hold the notes of other refs
pub fn mapped_commit_id(note: &str) -> Option<&str> {
  note.lines().find_map(|line| line.trim().strip_prefix(PREFIX)).map(str::trim)
}

/// The source a mapping note records (`{SOURCE_PREFIX}<parent tree> <tree>`); `None` for a note written before it
pub fn mapped_source(note: &str) -> Option<NoteSource> {
  let (parent_tree, tree) = note.lines().find_map(|line| line.trim().strip_prefix(SOURCE_PREFIX))?.trim().split_once(' ')?;
  Some(NoteSource {
    parent_tree: parent_tree.to_string(),
    tree: tree.trim().to_string(),
  })
}

/// The mapping lines of a note, to copy them onto a rewritten commit without the lines of other notes
pub fn mapping_lines(note: &str) -> Option<String> {
  let lines: Vec<&str> = note
    .lines()
    .map(str::trim)
    .filter(|line| line.starts_with(PREFIX) || line.starts_with(SOURCE_PREFIX))
    .collect();
  lines.iter().any(|line| line.starts_with(PREFIX)).then(|| lines.join("\n"))
}

/// The notes ref the mapping notes live on (`core.notesRef`, normally `refs/notes/commits`)
pub fn notes_ref(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String, anyhow::Error> {
  git_executor.execute_command(&["notes", "get-ref"], repo_path)
}

/// `write_commits_with_notes` with `notes` (`(index of the commit in the batch, note content)`) on the mapping notes
/// ref, which is looked up only when there are notes
pub fn write_commits_with_mapping_notes(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commits: &[NewCommit<'_>],
  notes: Vec<(usize, String)>,
  stores: &[StoreCommit<'_>],
) -> Result<WrittenCommits, anyhow::Error> {
  let notes_ref = if notes.is_empty() { None } else { Some(notes_ref(git_executor, repo_path)?) };
  let notes = notes_ref.as_deref().map(|notes_ref| NotesOfWritten { notes_ref, notes });
  write_commits_with_notes(git_executor, repo_path, commits, notes.as_ref(), stores)
}

/// Write mapping notes after a sync, all in one notes commit on the default notes ref (`core.notesRef`, normally
/// `refs/notes/commits`)
pub fn write_commit_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes: &[MappingNote]) -> Result<(), anyhow::Error> {
  write_commit_notes_with(git_executor, repo_path, notes, None)
}

/// `write_commit_notes` plus the notes of another ref (`(notes ref, (target, content))`) in the same `fast-import` run
#[instrument(skip(git_executor, notes, other), fields(notes_count = notes.len()))]
pub fn write_commit_notes_with(git_executor: &GitCommandExecutor, repo_path: &str, notes: &[MappingNote], other: Option<(&str, &[(String, String)])>) -> Result<(), anyhow::Error> {
  let mut refs = Vec::with_capacity(2);
  let mapping_ref;
  let mapping_notes: Vec<(String, String)>;
  if !notes.is_empty() {
    mapping_ref = notes_ref(git_executor, repo_path)?;
    mapping_notes = notes.iter().map(|note| (note.original.clone(), note.content())).collect();
    refs.push((mapping_ref.as_str(), mapping_notes.as_slice()));
  }
  refs.extend(other);
  write_notes_to_refs(git_executor, repo_path, &refs)
}
