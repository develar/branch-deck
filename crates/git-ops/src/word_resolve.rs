//! The resolve by words of a conflicting cherry-pick (docs/design/git-operations.md, "Resolve by words"): the sync
//! commits a conflict of git whose every conflicting file the engine merges without a conflict that its words leave,
//! as IntelliJ's merge viewer resolves the simple conflicts. All or nothing: one file that does not resolve, or one
//! conflict that is not of the contents of a text, and the cherry-pick stays the conflict of git.
//!
//! - **Which conflicts.** A path with the stages 1, 2 and 3, regular files whose modes merge, only `CONFLICT (contents)`
//!   messages, the `merge` attribute unspecified, set or `text`, and UTF-8 text without NUL on every side. An add/add,
//!   a modify/delete, a rename/rename, a file/directory and a binary file stay conflicts.
//! - **The result.** `merge_texts` of the stages (left = 2, ours, the target; base = 1; right = 3, theirs, the commit)
//!   under `Default` without a deadline, so that every machine resolves alike; its size limits still hold. Every block
//!   must resolve, and `compose_merge` must keep every line end (`exact`).
//! - **The tree.** The tree of the merge with each resolved file in place of the file with the markers, its mode
//!   merged as git merges it. Its id is computed here, from the trees on the paths; the commit writes the blobs and the
//!   trees (`CommitChain::write_commit`).
//! - **Processes.** One `check-attr` and one `cat-file --batch` for the stages, the files with the markers and the
//!   trees; none when a path fails before.

use crate::attributes::read_attributes;
use crate::cat_file_batch::{BatchEntry, for_each_object};
use crate::commit_files::MAX_BLOB_BYTES;
use crate::commit_utils::{blob_id, object_id_of};
use crate::merge_conflict::count_conflicts;
use crate::merge_tree_output::MergeTreeOutput;
use crate::object_texts::is_binary;
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeMap, HashMap};
use text_diff::apply::compose_merge;
use text_diff::merge_model::merge_texts;
use text_diff::model::DiffBudget;
use text_diff::policy::WhitespacePolicy;
use tracing::{debug, instrument};

/// The version of the resolve by words: bump it when the merge, the resolve or the composition can give another result,
/// so that no remembered resolution of the old way is taken
pub const RESOLVE_VERSION: u32 = 1;

/// The attribute that names the merge driver of a path (`gitattributes`, "Performing a three-way merge")
const MERGE_ATTRIBUTE: &str = "merge";

/// Whether the copies of a sync resolve the conflicts of git by words (`branchdeck.resolveByWords`, on by default)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ResolveMode {
  Off,
  #[default]
  On,
}

impl ResolveMode {
  /// What a remembered conflict view or chain of copies keeps in its key: 0 off, else `RESOLVE_VERSION`, so that none
  /// remembered under another mode is taken
  pub const fn key(self) -> u32 {
    match self {
      Self::Off => 0,
      Self::On => RESOLVE_VERSION,
    }
  }
}

/// A cherry-pick that its words resolve
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordResolution {
  /// The tree of the merge with the resolved files; its trees and blobs exist only once the commit is written
  pub tree: String,
  /// The resolved files, in git's order
  pub files: Vec<ResolvedFile>,
}

impl WordResolution {
  /// The conflicts of git that the words resolved, in all files
  pub fn conflicts(&self) -> u32 {
    self.files.iter().map(|file| file.conflicts).sum()
  }
}

/// A resolved file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
  pub path: String,
  /// `100644` or `100755`, merged as git merges the modes
  pub mode: String,
  /// The conflicts of git in the file: the `<<<<<<<` lines of the file with the markers
  pub conflicts: u32,
  /// The id of `content`
  pub blob: String,
  pub content: Vec<u8>,
}

/// Why a conflicting cherry-pick does not resolve by words
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotResolved {
  /// A conflict of git that is not of the contents of a text: an add/add, a modify/delete, a rename, a binary file
  NotContent,
  /// Both sides changed the mode, each otherwise
  ModeConflict,
  /// The `merge` attribute names a driver or `binary`: the conflict is the driver's
  MergeAttribute,
  /// A side that is not UTF-8 text, has a NUL, or is over the size limits of the engine
  NotText,
  /// A conflict that the words do not resolve
  Unresolved,
  /// A line end that the merge takes as equal would be lost
  LineEnds,
}

/// What the resolve by words of a conflicting cherry-pick gives
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordResolveOutcome {
  Resolved(WordResolution),
  /// The first path that does not resolve, and why
  NotResolved {
    path: String,
    reason: NotResolved,
  },
}

/// The (mode, id) of the stages 1, 2 and 3 of a path, `None` for a stage git did not write
type Stages<'a> = [Option<(&'a str, &'a str)>; 3];

/// The stages of one conflicting path that qualifies: the ids of the base, ours and theirs, and the merged mode
struct Candidate {
  path: String,
  stages: [String; 3],
  mode: String,
}

/// Resolve the conflicting cherry-pick `merge` (`merge-tree --write-tree -z` with its messages) by words
#[instrument(skip_all, fields(paths = merge.conflicted_paths().len()))]
pub fn resolve_by_words(git: &GitCommandExecutor, repository_path: &str, merge: &MergeTreeOutput) -> Result<WordResolveOutcome> {
  let candidates = match candidates(merge) {
    Ok(candidates) => candidates,
    Err((path, reason)) => return Ok(WordResolveOutcome::NotResolved { path, reason }),
  };
  if candidates.is_empty() {
    bail!("a merge without conflicting paths has nothing to resolve");
  }
  for attribute in read_attributes(git, repository_path, None, &[MERGE_ATTRIBUTE], candidates.iter().map(|candidate| candidate.path.as_str()))? {
    if !matches!(attribute.value.as_str(), "unspecified" | "set" | "text") {
      debug!(path = %attribute.path, value = %attribute.value, "the merge attribute names a driver");
      return Ok(not_resolved(&attribute.path, NotResolved::MergeAttribute));
    }
  }

  // the stages and the file with the markers of each path, then the trees on the paths
  let mut names: Vec<String> = Vec::with_capacity(candidates.len() * 4);
  for candidate in &candidates {
    names.extend(candidate.stages.iter().cloned());
    names.push(format!("{}:{}", merge.tree, candidate.path));
  }
  let directories = directories(&candidates);
  let tree_names_start = names.len();
  names.extend(directories.iter().map(|directory| tree_name(&merge.tree, directory)));
  let mut objects: Vec<Option<Vec<u8>>> = vec![None; names.len()];
  for_each_object(git, repository_path, &names, MAX_BLOB_BYTES, |index, entry| {
    if let BatchEntry::Object { content, .. } = entry {
      objects[index] = Some(content);
    }
    Ok(())
  })?;

  let mut files = Vec::with_capacity(candidates.len());
  for (index, candidate) in candidates.iter().enumerate() {
    let sides = &objects[index * 4..index * 4 + 3];
    let texts: Vec<Option<&str>> = sides
      .iter()
      .map(|side| side.as_deref().filter(|bytes| !is_binary(bytes)).and_then(|bytes| std::str::from_utf8(bytes).ok()))
      .collect();
    let (Some(base), Some(left), Some(right)) = (texts[0], texts[1], texts[2]) else {
      return Ok(not_resolved(&candidate.path, NotResolved::NotText));
    };
    let text_merge = merge_texts(left, base, right, WhitespacePolicy::Default, &DiffBudget::NONE);
    if text_merge.line_level_only {
      return Ok(not_resolved(&candidate.path, NotResolved::NotText));
    }
    if text_merge.unresolved > 0 {
      return Ok(not_resolved(&candidate.path, NotResolved::Unresolved));
    }
    let composed = compose_merge(left.as_bytes(), base.as_bytes(), right.as_bytes(), &text_merge);
    if !composed.exact {
      return Ok(not_resolved(&candidate.path, NotResolved::LineEnds));
    }
    let marked = objects[index * 4 + 3].as_deref().ok_or_else(|| anyhow!("the merge tree has no {}", candidate.path))?;
    files.push(ResolvedFile {
      path: candidate.path.clone(),
      mode: candidate.mode.clone(),
      conflicts: count_conflicts(&String::from_utf8_lossy(marked)),
      blob: blob_id(&merge.tree, &composed.bytes),
      content: composed.bytes,
    });
  }

  let trees: HashMap<&str, &[u8]> = directories
    .iter()
    .enumerate()
    .map(|(offset, directory)| {
      objects[tree_names_start + offset]
        .as_deref()
        .map(|tree| (directory.as_str(), tree))
        .ok_or_else(|| anyhow!("the merge tree has no directory {directory:?}"))
    })
    .collect::<Result<_>>()?;
  let tree = tree_with(&merge.tree, &trees, &files)?;
  Ok(WordResolveOutcome::Resolved(WordResolution { tree, files }))
}

fn not_resolved(path: &str, reason: NotResolved) -> WordResolveOutcome {
  debug!(path, ?reason, "the cherry-pick does not resolve by words");
  WordResolveOutcome::NotResolved { path: path.to_string(), reason }
}

/// The conflicting paths of `merge`, each qualified by its stages, its modes and its messages; the first one that does
/// not qualify, and why
fn candidates(merge: &MergeTreeOutput) -> Result<Vec<Candidate>, (String, NotResolved)> {
  let mut by_path: BTreeMap<&[u8], Stages<'_>> = BTreeMap::new();
  for entry in &merge.entries {
    let stages = by_path.entry(&entry.path).or_default();
    if let Some(slot) = stages.get_mut(usize::from(entry.stage).wrapping_sub(1)) {
      *slot = Some((entry.mode.as_str(), entry.id.as_str()));
    }
  }
  let mut candidates = Vec::with_capacity(by_path.len());
  // git's order of the entries: by path
  for path in merge.conflicted_paths() {
    let name = String::from_utf8_lossy(path).into_owned();
    let stages = by_path[path];
    let contents_only = merge
      .messages
      .iter()
      .filter(|message| message.paths.iter().any(|message_path| message_path.as_slice() == path))
      .all(|message| message.kind == "CONFLICT (contents)" || message.kind == "Auto-merging");
    let [Some(base), Some(left), Some(right)] = stages else {
      return Err((name, NotResolved::NotContent));
    };
    // a name that git cannot take in a `<tree>:<path>` of cat-file stays a conflict too
    if !contents_only || std::str::from_utf8(path).is_err() || path.contains(&0) {
      return Err((name, NotResolved::NotContent));
    }
    if ![base.0, left.0, right.0].iter().all(|mode| matches!(*mode, "100644" | "100755")) {
      return Err((name, NotResolved::NotContent));
    }
    let mode = if left.0 == base.0 {
      right.0
    } else if right.0 == base.0 || right.0 == left.0 {
      left.0
    } else {
      return Err((name, NotResolved::ModeConflict));
    };
    candidates.push(Candidate {
      path: name,
      stages: [base.1.to_string(), left.1.to_string(), right.1.to_string()],
      mode: mode.to_string(),
    });
  }
  Ok(candidates)
}

/// The directories that hold the paths, the root (`""`) among them, each once
fn directories(candidates: &[Candidate]) -> Vec<String> {
  let mut directories = vec![String::new()];
  for candidate in candidates {
    let mut end = 0;
    while let Some(slash) = candidate.path[end..].find('/') {
      end += slash;
      let directory = &candidate.path[..end];
      if !directories.iter().any(|known| known == directory) {
        directories.push(directory.to_string());
      }
      end += 1;
    }
  }
  directories
}

fn tree_name(tree: &str, directory: &str) -> String {
  if directory.is_empty() { tree.to_string() } else { format!("{tree}:{directory}") }
}

/// The id of `tree` with each of `files` in place of its entry: the trees on the paths rewritten from the deepest one,
/// each entry where it was, so that the order of the tree stays git's
fn tree_with(tree: &str, trees: &HashMap<&str, &[u8]>, files: &[ResolvedFile]) -> Result<String> {
  let id_length = tree.len() / 2;
  // directory -> (entry name -> (mode, id)) of the entries to replace
  let mut changes: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
  for file in files {
    let (directory, name) = file.path.rsplit_once('/').unwrap_or(("", &file.path));
    changes
      .entry(directory.to_string())
      .or_default()
      .push((name.to_string(), file.mode.clone(), file.blob.clone()));
  }
  let mut ids: HashMap<String, String> = HashMap::new();
  let mut pending: Vec<String> = trees.keys().map(|directory| (*directory).to_string()).collect();
  // the deepest first: a directory is rewritten once each of its changed subdirectories has its id
  pending.sort_by_key(|directory| std::cmp::Reverse(if directory.is_empty() { 0 } else { directory.matches('/').count() + 1 }));
  for directory in pending {
    let mut replaced = changes.remove(&directory).unwrap_or_default();
    for (subdirectory, id) in &ids {
      let (parent, name) = subdirectory.rsplit_once('/').unwrap_or(("", subdirectory));
      if parent == directory && !subdirectory.is_empty() {
        replaced.push((name.to_string(), "40000".to_string(), id.clone()));
      }
    }
    let content = replace_entries(trees[directory.as_str()], &replaced, id_length)?;
    ids.insert(directory, object_id_of(tree, "tree", &content));
  }
  ids.remove("").ok_or_else(|| anyhow!("no root tree"))
}

/// A tree object with the entries named in `replaced` given another mode and id: `<mode> <name>\0<raw id>` each
fn replace_entries(tree: &[u8], replaced: &[(String, String, String)], id_length: usize) -> Result<Vec<u8>> {
  let mut content = Vec::with_capacity(tree.len() + 8 * replaced.len());
  let mut found = 0;
  let mut rest = tree;
  while !rest.is_empty() {
    let nul = rest.iter().position(|&b| b == 0).ok_or_else(|| anyhow!("a tree entry without a NUL"))?;
    let (header, after) = rest.split_at(nul);
    if after.len() < 1 + id_length {
      bail!("a tree entry without its id");
    }
    let space = header.iter().position(|&b| b == b' ').ok_or_else(|| anyhow!("a tree entry without a mode"))?;
    let name = &header[space + 1..];
    match replaced.iter().find(|(replaced_name, _, _)| replaced_name.as_bytes() == name) {
      Some((_, mode, id)) => {
        found += 1;
        content.extend_from_slice(mode.as_bytes());
        content.push(b' ');
        content.extend_from_slice(name);
        content.push(0);
        content.extend_from_slice(&decode_hex(id)?);
      }
      None => content.extend_from_slice(&rest[..nul + 1 + id_length]),
    }
    rest = &after[1 + id_length..];
  }
  if found != replaced.len() {
    bail!("a tree lacks {} of the entries to replace", replaced.len() - found);
  }
  Ok(content)
}

fn decode_hex(id: &str) -> Result<Vec<u8>> {
  (0..id.len())
    .step_by(2)
    .map(|index| u8::from_str_radix(id.get(index..index + 2).unwrap_or_default(), 16).map_err(|e| anyhow!("bad object id {id}: {e}")))
    .collect()
}

#[cfg(test)]
#[path = "word_resolve_test.rs"]
mod word_resolve_test;
