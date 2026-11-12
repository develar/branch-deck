//! The lines a change moved from one file to another (openspec/specs/commit/quiet-changes/spec.md, "Subject";
//! docs/design/text-diff.md, "Moves between files"): rows of deleted lines of one file that stand again, as added
//! lines, in another file of the same subject. Branch Deck's own.
//!
//! - **What a pair of blobs leaves** (`left_rows`): the lines of its blocks that are a change and that no mark took,
//!   neither a run nor a move within the file, as rows of lines that follow each other in one block. A line is kept
//!   as the key of a move within a file (`moves.rs`: its text without the spaces and tabs at its ends), hashed, with
//!   the number of its characters that are no whitespace: the texts are not read again when the files of a subject
//!   are put together. A row that cannot hold a move between files by itself is not kept.
//! - **Where the indentation has a meaning** (Python, YAML, Starlark) the indent of a line is part of its key: a line
//!   at another depth is another line, so a block that another file holds with one line nested otherwise, or whole at
//!   another depth, is no move between files. A move within a file says by how much its lines were shifted
//!   (`LineMove::indent`) and which of them did not follow; a label on two files says "the same lines", and there
//!   they are not.
//! - **A move between files** (`find_moves_across`) is a row of lines of the same keys, line by line, on the old side
//!   of one file and on the new side of another: exact lines only, since a label on two files must not guess. Blank
//!   lines inside it are lines of it; it starts and ends with a line that is not blank. It counts when it holds
//!   `MIN_ACROSS_LINES` lines that are not blank and `MIN_ACROSS_CHARS` characters that are no whitespace: more than
//!   a move within a file needs, because the same boilerplate stands in many files of a change.
//! - **One to one, the longest first**: of the rows found the longest is taken, then the one with more characters,
//!   then the first by its new and by its old lines, the files in their order; a row that shares a line with a taken
//!   one is cut to the parts of it that are still free, which are asked again. So lines that match two places are a
//!   move to the first of them, and the other place is plain added lines. The answer is a function of the rows.
//! - **Limits by work, never by time**: the files are taken in their order, and a file whose rows do not fit into
//!   `MAX_ACROSS_LINES` lines beside those of the files taken before it takes no part; a key that more than
//!   `MAX_KEY_CANDIDATES` new lines have starts no row; the search reads `MAX_ACROSS_WORK` pairs of lines.
//!   `AcrossWork` says what was left out.
//!
//! A move between files is a label: it is never quiet, takes no line out of a view and changes no stop.

use crate::model::{DiffBlock, LineMove, LineRun, to_u32};
use crate::moves::{MAX_KEY_CANDIDATES, TakenLines, WHITESPACE, free_spans, line_starts, line_text, without_empty_last_line};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::ops::Range;

/// A move between files holds at least this many characters that are no whitespace: twice what a move within a file
/// needs; to be tuned on real commits
pub const MIN_ACROSS_CHARS: u32 = 48;
/// A move between files holds at least this many lines that are not blank
pub const MIN_ACROSS_LINES: u32 = 2;
/// The lines of the rows of the files that take part in the search, both sides together: a file whose rows do not
/// fit beside those of the files before it takes no part
pub const MAX_ACROSS_LINES: usize = 1 << 18;
/// The pairs of lines the search reads
pub const MAX_ACROSS_WORK: u64 = 1 << 22;

/// The key of a blank line
const BLANK: u64 = 0;

/// A row of changed lines of one side of a pair that no mark took: lines that follow each other in one block
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeftRow {
  /// Its first line, from 0
  pub start: u32,
  /// For each line of it, the hash of its text without the spaces and tabs at its ends, or without those at its end
  /// alone where the indentation has a meaning (`left_rows`); 0 for a blank line
  pub keys: Vec<u64>,
  /// For each line of it, its characters that are no whitespace
  pub solid: Vec<u32>,
}

impl LeftRow {
  fn lines(&self) -> Range<u32> {
    self.start..self.start + to_u32(self.keys.len())
  }

  /// The part of the row at `lines` (lines of the text inside it), without the blank lines at its ends; `None` when
  /// it cannot hold a move between files
  fn part(&self, lines: Range<u32>) -> Option<Self> {
    let at = |line: u32| (line - self.start) as usize;
    let (mut from, mut to) = (at(lines.start), at(lines.end));
    while from < to && self.keys[from] == BLANK {
      from += 1;
    }
    while from < to && self.keys[to - 1] == BLANK {
      to -= 1;
    }
    let part = Self {
      start: self.start + to_u32(from),
      keys: self.keys[from..to].to_vec(),
      solid: self.solid[from..to].to_vec(),
    };
    counts(&part.keys, &part.solid).then_some(part)
  }

  /// The parts of the row outside `taken` (ranges of lines in their order, without overlap) that can still hold a
  /// move between files
  pub fn without(&self, taken: &[Range<u32>]) -> Vec<Self> {
    let lines = self.lines();
    let mut parts = Vec::new();
    let mut from = lines.start;
    for range in &taken[taken.partition_point(|range| range.end <= lines.start)..] {
      if range.start >= lines.end {
        break;
      }
      if from < range.start {
        parts.extend(self.part(from..range.start));
      }
      from = from.max(range.end);
    }
    if from < lines.end {
      parts.extend(self.part(from..lines.end));
    }
    parts
  }
}

/// Whether lines of these keys and characters are enough for a move between files
fn counts(keys: &[u64], solid: &[u32]) -> bool {
  let lines = keys.iter().filter(|key| **key != BLANK).count();
  lines >= MIN_ACROSS_LINES as usize && solid.iter().map(|count| u64::from(*count)).sum::<u64>() >= u64::from(MIN_ACROSS_CHARS)
}

/// The hash of the key of a line: FNV-1a over its bytes, then the finalizer of `SplitMix64`. Written out here, not a
/// hasher of a crate or of the standard library: the hashes of two pairs are compared whenever they were made, by
/// whichever build, so the function must never change without the version of the diffs
/// (`file_diffs::service::MODEL_VERSION`). Never 0, the key of a blank line
fn key_of(text: &str) -> u64 {
  let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
  for byte in text.bytes() {
    hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
  }
  hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
  hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
  hash ^= hash >> 31;
  hash.max(1)
}

/// What the diff `blocks` of `old` and `new` leaves to the other files of its subject, the old side first: the lines
/// of its changes that are in none of `runs` (by `old_start`, each inside one block) and in none of `moves`, as rows
/// that can each hold a move between files, in the order of the lines. The empty line after the last line end of a
/// text is no line of it, as for a move within a file. `indented`: the indentation of the file has a meaning, so
/// the indent of a line is part of its key and a line pairs only with one at its depth
pub fn left_rows(old: &str, new: &str, blocks: &[DiffBlock], runs: &[LineRun], moves: &[LineMove], indented: bool) -> [Vec<LeftRow>; 2] {
  let side = |text: &str, new_side: bool| -> Vec<LeftRow> {
    let mut spans = free_spans(blocks, runs, new_side);
    if spans.is_empty() {
      return Vec::new();
    }
    let starts = line_starts(text);
    without_empty_last_line(&mut spans, text, &starts);
    let mut moved: Vec<Range<u32>> = moves.iter().map(|it| if new_side { it.new_lines() } else { it.old_lines() }).collect();
    moved.sort_unstable_by_key(|lines| lines.start);
    let mut rows = Vec::new();
    for span in spans {
      let (mut keys, mut solid) = (Vec::with_capacity(span.lines.len()), Vec::with_capacity(span.lines.len()));
      for line in span.lines.clone() {
        let ended = line_text(text, &starts, line).trim_end_matches(['\n', '\r']).trim_end_matches(WHITESPACE);
        let trimmed = ended.trim_start_matches(WHITESPACE);
        // a blank line has no depth: it pairs with any blank line
        keys.push(if trimmed.is_empty() { BLANK } else { key_of(if indented { ended } else { trimmed }) });
        solid.push(to_u32(trimmed.chars().filter(|character| !WHITESPACE.contains(character)).count()));
      }
      let row = LeftRow {
        start: span.lines.start,
        keys,
        solid,
      };
      rows.extend(row.without(&moved));
    }
    rows
  };
  [side(old, false), side(new, true)]
}

/// The rows of one file of a subject: what its pair left on each side
#[derive(Clone, Copy, Debug, Default)]
pub struct FileRows<'a> {
  pub old: &'a [LeftRow],
  pub new: &'a [LeftRow],
}

/// Lines that moved from one file of a subject to another without a change: `len` old lines from `old_start` of the
/// file `old_file` are the new lines from `new_start` of the file `new_file` (the files by their places among the
/// rows given, the lines from 0)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveAcross {
  pub old_file: u32,
  pub old_start: u32,
  pub new_file: u32,
  pub new_start: u32,
  pub len: u32,
}

/// What a search for moves between files cost, and what it left out
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcrossWork {
  /// The lines of the rows that took part
  pub lines: usize,
  /// The files that took no part, their rows not fitting into `MAX_ACROSS_LINES`, and the lines of those rows
  pub skipped_files: usize,
  pub skipped_lines: usize,
  /// The pairs of lines read, at most `MAX_ACROSS_WORK` and those of the last row
  pub pairs: u64,
  /// The search ended at `MAX_ACROSS_WORK` with the moves found until then
  pub cut: bool,
}

/// The limits of a search: the constants, but in a test of what a limit does
#[derive(Clone, Copy, Debug)]
pub(crate) struct AcrossLimits {
  pub lines: usize,
  pub work: u64,
}

/// The moves between the files of a subject, from the rows each of them left (`left_rows`), in the order of the
/// files: by the file and the line they stand at now. A line of a side is in one move at most, and a move has its
/// two ends in two files: lines that moved within a file are the moves of its pair
pub fn find_moves_across(files: &[FileRows<'_>]) -> (Vec<MoveAcross>, AcrossWork) {
  find_moves_across_within(
    files,
    &AcrossLimits {
      lines: MAX_ACROSS_LINES,
      work: MAX_ACROSS_WORK,
    },
  )
}

/// A line of a row among the lines of its side
#[derive(Clone, Copy)]
struct Line {
  file: u32,
  /// Its number in its text
  number: u32,
  key: u64,
  solid: u32,
  /// It follows the line before it in its row
  joined: bool,
}

/// A row that may be taken, in the order they are taken in: the longest, then the one with more characters, then the
/// first by its new and by its old lines
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate {
  len: Reverse<u32>,
  solid: Reverse<u64>,
  /// The place of its first new line among the lines of the new sides, and of its first old line
  new: u32,
  old: u32,
}

/// `find_moves_across` within `limits`
pub(crate) fn find_moves_across_within(files: &[FileRows<'_>], limits: &AcrossLimits) -> (Vec<MoveAcross>, AcrossWork) {
  let mut work = AcrossWork::default();
  let (mut old, mut new): (Vec<Line>, Vec<Line>) = (Vec::new(), Vec::new());
  let flatten = |rows: &[LeftRow], file: usize, lines: &mut Vec<Line>| {
    for row in rows {
      for (offset, (key, solid)) in row.keys.iter().zip(&row.solid).enumerate() {
        lines.push(Line {
          file: to_u32(file),
          number: row.start + to_u32(offset),
          key: *key,
          solid: *solid,
          joined: offset > 0,
        });
      }
    }
  };
  for (file, rows) in files.iter().enumerate() {
    let count: usize = rows.old.iter().chain(rows.new).map(|row| row.keys.len()).sum();
    if work.lines + count > limits.lines {
      // the files before it stay, and a smaller one after it may still fit: the answer is of the rows in their order
      work.skipped_files += 1;
      work.skipped_lines += count;
      continue;
    }
    work.lines += count;
    flatten(rows.old, file, &mut old);
    flatten(rows.new, file, &mut new);
  }
  if old.is_empty() || new.is_empty() {
    return (Vec::new(), work);
  }

  // the new lines of each key, in their order
  let mut by_key: FxHashMap<u64, Vec<u32>> = FxHashMap::default();
  for (index, line) in new.iter().enumerate() {
    if line.key != BLANK {
      by_key.entry(line.key).or_default().push(to_u32(index));
    }
  }
  let mut finder = Finder {
    old: &old,
    found: BinaryHeap::new(),
  };
  // the pairs of the rows read: a pair is in one row
  let mut seen: FxHashSet<(u32, u32)> = FxHashSet::default();
  'search: for (old_index, line) in old.iter().enumerate() {
    let Some(candidates) = by_key.get(&line.key).filter(|candidates| candidates.len() <= MAX_KEY_CANDIDATES) else {
      continue;
    };
    for &new_index in candidates {
      if work.pairs >= limits.work {
        work.cut = true;
        break 'search;
      }
      work.pairs += 1;
      if new[new_index as usize].file == line.file || seen.contains(&(to_u32(old_index), new_index)) {
        continue;
      }
      // the row of the same keys through this pair, both ways
      let (mut old_from, mut new_from) = (old_index, new_index as usize);
      while old[old_from].joined && new[new_from].joined && old[old_from - 1].key == new[new_from - 1].key {
        (old_from, new_from) = (old_from - 1, new_from - 1);
      }
      let mut len = old_index - old_from + 1;
      while old.get(old_from + len).is_some_and(|line| line.joined) && new.get(new_from + len).is_some_and(|line| line.joined) && old[old_from + len].key == new[new_from + len].key
      {
        len += 1;
      }
      work.pairs += len as u64;
      for offset in 0..len {
        if old[old_from + offset].key != BLANK {
          seen.insert((to_u32(old_from + offset), to_u32(new_from + offset)));
        }
      }
      finder.offer(old_from, new_from, len);
    }
  }

  // the longest first, each line once
  let mut moves = Vec::new();
  let mut taken = TakenLines::new(old.len(), new.len());
  while let Some(Reverse(candidate)) = finder.found.pop() {
    let (old_at, new_at) = (candidate.old as usize, candidate.new as usize);
    match taken.take(old_at, new_at, candidate.len.0 as usize) {
      Ok(()) => moves.push(MoveAcross {
        old_file: old[old_at].file,
        old_start: old[old_at].number,
        new_file: new[new_at].file,
        new_start: new[new_at].number,
        len: candidate.len.0,
      }),
      // the parts of it that are still free are asked again: each is shorter, so the choice ends
      Err(rows) => {
        for row in rows {
          finder.offer(old_at + row.start, new_at + row.start, row.len());
        }
      }
    }
  }
  moves.sort_unstable_by_key(|found| (found.new_file, found.new_start));
  (moves, work)
}

struct Finder<'a> {
  /// The lines of the old sides: a row has the same keys and characters on its new side
  old: &'a [Line],
  /// The rows that may be taken, the first to take first
  found: BinaryHeap<Reverse<Candidate>>,
}

impl Finder<'_> {
  /// Keeps the row of `len` pairs from the old line at `old` and the new line at `new`, without the blank lines at
  /// its ends, when it is enough for a move between files
  fn offer(&mut self, mut old: usize, mut new: usize, mut len: usize) {
    while len > 0 && self.old[old].key == BLANK {
      (old, new, len) = (old + 1, new + 1, len - 1);
    }
    while len > 0 && self.old[old + len - 1].key == BLANK {
      len -= 1;
    }
    let lines = &self.old[old..old + len];
    let solid: u64 = lines.iter().map(|line| u64::from(line.solid)).sum();
    let filled = lines.iter().filter(|line| line.key != BLANK).count();
    if filled >= MIN_ACROSS_LINES as usize && solid >= u64::from(MIN_ACROSS_CHARS) {
      self.found.push(Reverse(Candidate {
        len: Reverse(to_u32(len)),
        solid: Reverse(solid),
        new: to_u32(new),
        old: to_u32(old),
      }));
    }
  }
}
