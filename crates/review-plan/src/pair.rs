//! The pair a review shows (spec/branch/review-units.spec.md, "Pair"): per file the reviewed commits change, its blocks
//! from its blob before the first commit that changes it to its blob after the last one, under `Default`: the blocks
//! the diffs of `base..head` show. The units of the review (`units`) cut these blocks, and a use on one of their lines
//! at the head was written or rewritten by this change (`fan_in`, "inside"); a use on another line of a changed file
//! was left as it was.
//!
//! - A file one commit changes has these blocks as the hunks of that commit already; a file that several commits change
//!   is compared once from its first blob to its last (`FileDiffs::blocks_of`: the memory of the diffs, the `file_diff`
//!   store, else one `cat-file --batch` for the blobs of all such files).
//! - A file the commits add is changed as a whole, and so is one whose type changes (a symlink that became a file). A
//!   file without text (binary, over 8 MiB) has no changed line: no use can be read there.

use crate::files::ChangedFile;
use anyhow::Result;
use file_diffs::blocks::{BlobPair, PairDiff};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use object_cache::commit_diff::Hunk;
use std::collections::HashMap;
use std::ops::Range;

/// A block of the pair: its lines on each side, from 0, the end excluded, as `text_diff::model::DiffBlock` has them
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineBlock {
  pub old: Range<u32>,
  pub new: Range<u32>,
}

/// What the pair of a file is
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairBlocks {
  /// The commits add it
  Added,
  /// The commits delete it
  Deleted,
  /// Its type changed (a symlink, a submodule): no lines to compare
  TypeChanged,
  /// Binary, over 8 MiB, or unreadable on a side
  NoText,
  /// Only its line ends changed: the diffs show no block, every line is changed as git sees it
  LineEnds,
  /// The blocks of the diffs, in order; the index of a block is its place here
  Blocks(Vec<LineBlock>),
}

/// The block of a `-U0` hunk: the inverse of `diff_scan::hunk_of`, whose empty side names the line before it
pub(crate) fn block_of(hunk: &Hunk) -> LineBlock {
  let start = |start: u32, lines: u32| if lines == 0 { start } else { start - 1 };
  let (old, new) = (start(hunk.old_start, hunk.old_lines), start(hunk.new_start, hunk.new_lines));
  LineBlock {
    old: old..old + hunk.old_lines,
    new: new..new + hunk.new_lines,
  }
}

/// Whether two texts differ only in their line ends
fn same_but_line_ends(a: &str, b: &str) -> bool {
  a != b && a.lines().map(|line| line.trim_end_matches('\r')).eq(b.lines().map(|line| line.trim_end_matches('\r')))
}

/// The pair of each of `files` (`files::changed_files`), by place: the hunks of a file one commit changes, the blocks of
/// its first blob against its last for one that several change. `texts` (the blobs read for the hunk text) tell a
/// change of the line ends alone, which a commit gives as one hunk of the whole file
pub(crate) fn pair_blocks(
  git: &GitCommandExecutor,
  repository_path: &str,
  cache: &ObjectCache,
  file_diffs: &FileDiffs,
  files: &[ChangedFile<'_>],
  texts: &HashMap<String, String>,
) -> Result<Vec<PairBlocks>> {
  let mut pairs: Vec<Option<PairBlocks>> = Vec::with_capacity(files.len());
  let mut compared: Vec<(usize, BlobPair<'_>)> = Vec::new();
  for (index, file) in files.iter().enumerate() {
    let (before, after) = match (file.before, file.after) {
      (None, _) => {
        pairs.push(Some(PairBlocks::Added));
        continue;
      }
      (Some(_), None) => {
        pairs.push(Some(PairBlocks::Deleted));
        continue;
      }
      (Some(before), Some(after)) => (before, after),
    };
    let pair = match file.changes.as_slice() {
      [change] if change.status == 'T' => Some(PairBlocks::TypeChanged),
      [change] if change.binary => Some(PairBlocks::NoText),
      [change] => {
        let line_ends = change.hunks.len() == 1 && matches!((texts.get(before), texts.get(after)), (Some(a), Some(b)) if same_but_line_ends(a, b));
        Some(if line_ends {
          PairBlocks::LineEnds
        } else {
          PairBlocks::Blocks(change.hunks.iter().map(block_of).collect())
        })
      }
      _ => {
        compared.push((
          index,
          BlobPair {
            path: file.path,
            old_path: Some(file.first_path).filter(|first_path| *first_path != file.path),
            old_id: Some(before),
            new_id: Some(after),
          },
        ));
        None
      }
    };
    pairs.push(pair);
  }
  if !compared.is_empty() {
    let blob_pairs: Vec<BlobPair<'_>> = compared.iter().map(|(_, pair)| *pair).collect();
    for ((index, _), diff) in compared.iter().zip(file_diffs.blocks_of(git, repository_path, cache, &blob_pairs)?) {
      pairs[*index] = Some(match diff {
        PairDiff::Text(diff) if diff.blocks.is_empty() && diff.line_ends.is_some() => PairBlocks::LineEnds,
        PairDiff::Text(diff) => PairBlocks::Blocks(
          diff
            .blocks
            .iter()
            .map(|block| LineBlock {
              old: block.old_start..block.old_end,
              new: block.new_start..block.new_end,
            })
            .collect(),
        ),
        PairDiff::NoText => PairBlocks::NoText,
      });
    }
  }
  Ok(pairs.into_iter().map(|pair| pair.unwrap_or(PairBlocks::NoText)).collect())
}

/// The changed lines of one file at the head
#[derive(Debug, Clone, PartialEq, Eq)]
enum FileLines {
  /// Every line
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
  /// The lines of `files` that their pairs (`pair_blocks`, by place) change at the head
  pub(crate) fn of(files: &[ChangedFile<'_>], pairs: &[PairBlocks]) -> Self {
    let mut lines = Self::default();
    for (file, pair) in files.iter().zip(pairs) {
      match pair {
        PairBlocks::Deleted => {}
        PairBlocks::Added | PairBlocks::TypeChanged | PairBlocks::LineEnds => lines.add_whole(file.path),
        PairBlocks::NoText => lines.add_ranges(file.path, Vec::new()),
        PairBlocks::Blocks(blocks) => {
          let ranges = blocks
            .iter()
            .filter(|block| !block.new.is_empty())
            .map(|block| block.new.start + 1..block.new.end + 1)
            .collect();
          lines.add_ranges(file.path, ranges);
        }
      }
    }
    lines
  }

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
