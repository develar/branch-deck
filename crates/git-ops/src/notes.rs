//! The mapping notes of a sync: a note on each original commit names its virtual copy and the trees it was copied from
//! (`MappingNote`); `read_notes` reads the notes of any ref for many commits with one `log --no-walk --stdin`, and
//! notes are written with one `fast-import` (`commit_writer.rs`).

use crate::commit_writer::{NewCommit, NotesOfWritten, StoreCommit, WrittenCommits, write_commits_with_notes, write_notes_to_refs};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;
use tracing::instrument;

pub const PREFIX: &str = "v-commit-v1:";
/// The line of a mapping note that names what the virtual commit was copied from: `<parent tree> <tree>` of the
/// original. A note copied onto a commit with other trees (`git notes copy`, `notes.rewriteRef`) no longer matches it.
pub const SOURCE_PREFIX: &str = "v-commit-source-v1:";
/// The line of a mapping note whose virtual commit resolved a conflict of git by words (`word_resolve`):
/// `<resolve version> <conflicts> <files>`
pub const RESOLVED_PREFIX: &str = "v-commit-resolved-v1:";

/// The trees a virtual commit was copied from: the original's parent tree and its tree
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteSource {
  pub parent_tree: String,
  pub tree: String,
}

/// The resolve by words that a virtual commit took (`RESOLVED_PREFIX`): the version of the resolve, the conflicts of
/// git it resolved and in how many files
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteResolution {
  pub version: u32,
  pub conflicts: u32,
  pub files: u32,
}

/// A mapping note to write: `original` is copied as `mapped` from `source`, with the resolve by words it took
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingNote {
  pub original: String,
  pub mapped: String,
  pub source: NoteSource,
  pub resolved: Option<NoteResolution>,
}

impl MappingNote {
  /// The note text: the mapping line, then the source line (old binaries read only the first), then the resolve line
  pub fn content(&self) -> String {
    let mut content = format!("{PREFIX}{}\n{SOURCE_PREFIX}{} {}", self.mapped, self.source.parent_tree, self.source.tree);
    if let Some(resolved) = self.resolved {
      let _ = write!(content, "\n{RESOLVED_PREFIX}{} {} {}", resolved.version, resolved.conflicts, resolved.files);
    }
    content
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

/// The resolve by words a mapping note records (`{RESOLVED_PREFIX}<version> <conflicts> <files>`); `None` for a copy
/// that resolved nothing, or a line this binary cannot read
pub fn mapped_resolution(note: &str) -> Option<NoteResolution> {
  let line = note.lines().find_map(|line| line.trim().strip_prefix(RESOLVED_PREFIX))?;
  let mut numbers = line.split_whitespace().map(str::parse::<u32>);
  match (numbers.next(), numbers.next(), numbers.next()) {
    (Some(Ok(version)), Some(Ok(conflicts)), Some(Ok(files))) => Some(NoteResolution { version, conflicts, files }),
    _ => None,
  }
}

/// The mapping lines of a note, to copy them onto a rewritten commit without the lines of other notes
pub fn mapping_lines(note: &str) -> Option<String> {
  let lines: Vec<&str> = note
    .lines()
    .map(str::trim)
    .filter(|line| line.starts_with(PREFIX) || line.starts_with(SOURCE_PREFIX) || line.starts_with(RESOLVED_PREFIX))
    .collect();
  lines.iter().any(|line| line.starts_with(PREFIX)).then(|| lines.join("\n"))
}

/// The notes ref the mapping notes live on (`core.notesRef`, normally `refs/notes/commits`)
pub fn notes_ref(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String, anyhow::Error> {
  git_executor.execute_command(&["notes", "get-ref"], repo_path)
}

/// The notes of `notes_ref` on `commits` (full commit ids, all existing), commit -> note without the surrounding
/// whitespace; a commit without a note, or with an empty one, is left out. One `log --no-walk --stdin`, the commits on
/// stdin (no argv limit); `%N` shows `notes_ref` only, whatever `notes.displayRef` says.
#[instrument(skip(git_executor, commits), fields(commits = commits.len()))]
pub fn read_notes(git_executor: &GitCommandExecutor, repo_path: &str, notes_ref: &str, commits: &[&str]) -> Result<HashMap<String, String>, anyhow::Error> {
  if commits.is_empty() {
    return Ok(HashMap::new());
  }
  let mut input = String::with_capacity(commits.len() * 41);
  for commit in commits {
    input.push_str(commit);
    input.push('\n');
  }
  let notes_arg = format!("--notes={notes_ref}");
  // `-z` ends each record with NUL, which a text note never holds
  let output = git_executor
    .cmd(&["log", "--no-walk", "--stdin", "-z", "--format=%H%x1f%N", &notes_arg], repo_path)
    .stdin(input.as_bytes())
    .text()?;
  Ok(
    output
      .split('\0')
      .filter_map(|record| {
        let (commit, note) = record.split_once('\u{1f}')?;
        let note = note.trim();
        (!note.is_empty()).then(|| (commit.trim().to_string(), note.to_string()))
      })
      .collect(),
  )
}

/// `write_commits_with_notes` with `notes` (`(index of the commit in the batch, note content)`) on the mapping notes
/// ref, which is looked up only when there are notes
pub fn write_commits_with_mapping_notes(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  commits: &[NewCommit<'_>],
  notes: Vec<(usize, String)>,
  stores: &[StoreCommit<'_>],
  committer: Option<&str>,
) -> Result<WrittenCommits, anyhow::Error> {
  let notes_ref = if notes.is_empty() { None } else { Some(notes_ref(git_executor, repo_path)?) };
  let notes = notes_ref.as_deref().map(|notes_ref| NotesOfWritten { notes_ref, notes });
  write_commits_with_notes(git_executor, repo_path, commits, notes.as_ref(), stores, committer)
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
