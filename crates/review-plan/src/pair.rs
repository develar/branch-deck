//! The lines the reviewed commits changed at the head (spec/branch/review-plan.spec.md, "Fan-in"): per file that exists
//! after them, the lines of its blocks from its blob before the first commit that changes it to its blob after the
//! last one, under `Default`: the blocks the diffs of `base..head` show. A use on one of them was written or rewritten
//! by this change (`fan_in`, "inside"); a use on another line of a changed file was left as it was.
//!
//! - A file one commit changes has these blocks as the hunks of that commit already; a file that several commits change
//!   is compared once from its first blob to its last (`FileDiffs::blocks_of`: the memory of the diffs, the `file_diff`
//!   store, else one `cat-file --batch` for the blobs of all such files).
//! - A file the commits add, and one whose type changes (a symlink that became a file), is changed as a whole. A file
//!   without text (binary, over 8 MiB) has no changed line: no use can be read there.

use crate::files::ChangedFile;
use anyhow::Result;
use file_diffs::blocks::{BlobPair, PairDiff};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use std::collections::HashMap;
use std::ops::Range;

/// The changed lines of one file at the head
#[derive(Debug, Clone, PartialEq, Eq)]
enum FileLines {
  /// Every line: the commits add the file
  Whole,
  /// The lines from 1, each range's end excluded, in order
  Ranges(Vec<Range<u32>>),
}

/// The lines the reviewed commits changed, per path after them
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangedLines {
  files: HashMap<String, FileLines>,
}

impl ChangedLines {
  /// The commits changed line `line` (from 1) of `path` at the head
  pub fn contains(&self, path: &str, line: u32) -> bool {
    match self.files.get(path) {
      None => false,
      Some(FileLines::Whole) => true,
      Some(FileLines::Ranges(ranges)) => ranges.iter().any(|range| range.contains(&line)),
    }
  }

  /// Every line of `path` changed
  pub fn add_whole(&mut self, path: &str) {
    self.files.insert(path.to_string(), FileLines::Whole);
  }

  /// The lines `ranges` (from 1, the end excluded) of `path` changed
  pub fn add_ranges(&mut self, path: &str, ranges: Vec<Range<u32>>) {
    self.files.insert(path.to_string(), FileLines::Ranges(ranges));
  }
}

/// The changed lines of `files` (`files::changed_files`) at the head: the hunks of a file one commit changes, the blocks
/// of its first blob against its last for one several commits change
pub(crate) fn changed_lines(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, file_diffs: &FileDiffs, files: &[ChangedFile<'_>]) -> Result<ChangedLines> {
  let mut lines = ChangedLines::default();
  let mut compared: Vec<(&str, BlobPair<'_>)> = Vec::new();
  for file in files {
    let Some(after) = file.after else { continue };
    let Some(before) = file.before else {
      lines.add_whole(file.path);
      continue;
    };
    match file.changes.as_slice() {
      [change] if change.status == 'T' => lines.add_whole(file.path),
      [change] if change.binary => lines.add_ranges(file.path, Vec::new()),
      [change] => {
        let ranges = change
          .hunks
          .iter()
          .filter(|hunk| hunk.new_lines > 0)
          .map(|hunk| hunk.new_start..hunk.new_start + hunk.new_lines)
          .collect();
        lines.add_ranges(file.path, ranges);
      }
      _ => compared.push((
        file.path,
        BlobPair {
          path: file.path,
          old_path: Some(file.first_path).filter(|first_path| *first_path != file.path),
          old_id: Some(before),
          new_id: Some(after),
        },
      )),
    }
  }
  if compared.is_empty() {
    return Ok(lines);
  }
  let pairs: Vec<BlobPair<'_>> = compared.iter().map(|(_, pair)| *pair).collect();
  for ((path, _), diff) in compared.iter().zip(file_diffs.blocks_of(git, repository_path, cache, &pairs)?) {
    match diff {
      // only the line ends changed: every line is, as git sees it
      PairDiff::Text(diff) if diff.blocks.is_empty() && diff.line_ends.is_some() => lines.add_whole(path),
      PairDiff::Text(diff) => {
        // a block's lines are from 0, its end excluded
        let ranges = diff.blocks.iter().filter(|block| block.new_end > block.new_start).map(|block| block.new_start + 1..block.new_end + 1).collect();
        lines.add_ranges(path, ranges);
      }
      PairDiff::NoText => lines.add_ranges(path, Vec::new()),
    }
  }
  Ok(lines)
}
