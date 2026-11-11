//! The lines a change moved within a file (openspec/specs/commit/quiet-changes/spec.md, "Moves";
//! docs/design/text-diff.md, "Moves"): rows of deleted lines of one block that stand again, as added lines, in another
//! block. Branch Deck's own. A move changes no block, and it is never quiet: where lines stand is behaviour.
//!
//! - **The lines a move may take** are the lines of the blocks that are a change (`BlockKind::Changed`) and in no
//!   run: a run already says what became of its lines, at their place, and a pale block is equal to itself under its
//!   policy. So a line is in a move or in a run, never in both.
//! - **The key of a line** is its text without the spaces and tabs at its ends, the whitespace of the comparison;
//!   the whitespace inside it counts (`a b` is not `ab`). A line without a key is blank and pairs with any blank one.
//! - **A pair** of an old and a new line is exact (the same key), blank, edited, or apart. Edited: the two keys are a
//!   similar pair by word (`inline::is_similar_pair`, the rule of the rows of the Inline view). Where the indentation
//!   has a meaning and the whitespace at the ends of a line counts, a line whose indent did not move as the indents
//!   of its move (`Shift`) is edited too: its nesting changed.
//! - **The shift of a move** is what happened to the indent of its lines: the same whitespace added to each, or
//!   taken from each, at one end of its indent (at its start, or else before the text of the line). A tab that became
//!   a space is no shift, and two lines that gained two columns, one of spaces and one of tabs, did not move alike.
//!   Where the indentation has a meaning the shift is that of the pair a chain grew from; elsewhere it is the shift
//!   most of the exact lines of the move agree on. `LineMove::indent` is its columns.
//! - **The whitespace at the ends of a line** counts under the policy that counts every character
//!   (`WhitespacePolicy::Default`): there a line of a move whose indent is another text and did not move by the shift,
//!   or whose whitespace after its text is another, is an edited line with that whitespace as a range, and a view
//!   that shows the old lines of a move once hides no change. Such a line is still an exact pair of the search
//!   unless the indentation has a meaning. Under a policy that leaves that whitespace out, it is no change of a
//!   move either.
//! - **A chain** grows from a pair of the same key in two blocks, line by line both ways along the two blocks, for as
//!   long as the pairs are not apart. Two lines of other keys are compared by word only where it matters: a chain
//!   grows through a pair that may be similar, and the pair is read when a row may be a move with it edited. Most
//!   such pairs (the lines of two functions that are alike) stand in rows that are no move whatever they are.
//! - **A move** is the longest row of a chain that starts and ends with an exact pair and holds at most a quarter of
//!   edited lines among its lines that are not blank (`MAX_EDITED_LINES`), with the blank pairs beside it; then the
//!   longest such rows of what is left of the chain. It counts when its exact lines hold `MIN_MOVE_CHARS` characters
//!   that are no whitespace. What is left of a row is read again for its longest row, and that is work: a row whose
//!   longest move holds at least half of it leaves half of it at most, and a longer row of shorter moves (a list
//!   whose every item has an edited line, moved) is read once for all of them (`Finder::many_moves`), each exact
//!   pair asked how far a move that starts with it reaches, the longest taken first.
//! - **The lines of a text** are those the comparison counts: the empty line after its last line end, and the one
//!   line of an empty text, is no line of a move and no blank line beside one (docs/design/text-diff.md, Key
//!   Decision 8: the comparison pairs it with no line but its like, and no count takes it).
//! - **The blank lines that moved with a move.** The comparison deletes a function with the blank line before it and
//!   adds it with the blank line after it as readily as the other way round: the two blank lines are no pair of the
//!   row. Blank lines beside the old lines of a move, in their block, and as many blank lines beside its new lines
//!   at its other end, in their block, left with it and stand at its other end now: the move counts them
//!   (`LineMove::blanks_before`, `blanks_after`). Without them the block the lines left would keep a deleted blank
//!   line to read. Where the whitespace at the ends of a line counts, they are blank lines of one text.
//! - **One to one, the longest first**: of the moves found, the longest is taken (then the one with fewer edited
//!   lines, then the first by its new and its old lines); a move that shares a line with a taken one is cut to the rows
//!   of it that are still free, which are asked again. The answer is a function of the two texts, the blocks, the
//!   runs, the policy and whether the indentation has a meaning.
//! - **Limits by work, never by time**: a side with more than `MAX_MOVE_LINES` lines a move may take has no moves;
//!   a key that more than `MAX_KEY_CANDIDATES` such new lines have starts no chain (a `}` is found as a line of a
//!   chain that another line started); the pairs read are counted, a pair of two lines read for their likeness by
//!   its length too and a pair of a row read again for what is left of the row, and the search ends at
//!   `MAX_MOVE_WORK` of them, with the moves found until then;
//!   `MAX_COMPARED_PAIRS` pairs of at most `MAX_PAIR_UNITS` a line are compared by word, and the chains grow through
//!   `MAX_UNREAD_PAIRS` pairs that are not compared: past either number a pair of other keys is apart.

use crate::by_word;
use crate::cancellation::Deadline;
use crate::fragments::DiffFragment;
use crate::inline::{LineRanges, SIMILAR_PAIR_MAX_CHANGED, is_similar_pair};
use crate::model::{BlockKind, DiffBlock, LineMove, LineRun, RangeKind, to_u32};
use crate::policy::{ComparisonPolicy, WhitespacePolicy};
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};
use std::ops::Range;

/// The exact lines of a move hold at least this many characters that are no whitespace; to be tuned on real commits
pub const MIN_MOVE_CHARS: usize = 24;
/// At most this share of the lines of a move that are not blank are edited, as `(numerator, denominator)`; to be tuned
/// on real commits
pub const MAX_EDITED_LINES: (i64, i64) = (1, 4);
/// A key that more new lines have than this starts no chain
pub const MAX_KEY_CANDIDATES: usize = 16;
/// A side with more lines that a move may take than this has no moves
pub const MAX_MOVE_LINES: usize = 1 << 16;
/// The pairs of lines the search reads, and as many lines of moves again the choice among the moves found: some
/// tenth of a second of one core at the worst (docs/design/diff-view.md, "Marks")
pub const MAX_MOVE_WORK: u64 = 1 << 18;
/// The pairs of lines compared by word
pub const MAX_COMPARED_PAIRS: u32 = 2_048;
/// The pairs of lines of other keys that the chains grow through unread: lines that are all alike (a list that was
/// sorted) make every row of them one long chain, and past this number such a pair ends its chain
pub const MAX_UNREAD_PAIRS: u32 = 16_384;
/// Two lines of other keys that are read for whether they may be similar cost the search a pair more for each so
/// many UTF-16 code units of them: the read of long lines is what a search of lines that are all alike costs
const SCANNED_UNITS_A_PAIR: usize = 64;
/// A line longer than this many UTF-16 code units, without the whitespace at its ends, is not compared by word
pub const MAX_PAIR_UNITS: usize = 512;
/// A row of more pairs than this whose longest move holds less than half of them is read once for all its moves
/// (`Finder::many_moves`), not again for each: what is left of a shorter row costs less than the heap of its starts
const SHORT_ROW: usize = 32;

/// The number of the key of a blank line
const BLANK: u32 = 0;

/// The limits of a search: the constants, but in a test of what a limit does
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
  pub lines: usize,
  pub key_candidates: usize,
  /// The pairs of lines the search reads
  pub search: u64,
  /// The lines of the moves the choice reads
  pub choice: u64,
  pub compared: u32,
  /// The pairs of lines the chains grow through unread
  pub unread: u32,
  /// The pairs of a row that is read again for each of its moves, whatever they hold of it
  pub short_row: usize,
}

pub(crate) const LIMITS: Limits = Limits {
  lines: MAX_MOVE_LINES,
  key_candidates: MAX_KEY_CANDIDATES,
  search: MAX_MOVE_WORK,
  choice: MAX_MOVE_WORK,
  compared: MAX_COMPARED_PAIRS,
  unread: MAX_UNREAD_PAIRS,
  short_row: SHORT_ROW,
};

/// What a search for moves cost
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MoveWork {
  /// The pairs of lines read by the search: a pair of other keys read for its likeness counted by its length too
  /// (one more for each 64 code units of its lines), and a pair of a row once more each time what is left of the
  /// row after a move is read again. At most `MAX_MOVE_WORK` and the pairs of the last row
  pub search: u64,
  /// The lines of the moves the choice read, those of what is left of a move read again counted as the search
  /// counts them: at most `MAX_MOVE_WORK` and the lines of the last one
  pub choice: u64,
  /// The pairs of lines compared by word, at most `MAX_COMPARED_PAIRS`
  pub compared: u32,
  /// The pairs of lines of other keys the chains grew through without comparing them, at most `MAX_UNREAD_PAIRS`
  pub unread: u32,
}

/// The moves of the diff `blocks` of `old` and `new`, by `new_start`: each line of a side in one move at most, the
/// old lines of a move inside one block and its new lines inside another, the blank lines that moved with it
/// counted beside them. `runs` are the runs of the diff (by `old_start`, each inside one block): their lines are in
/// no move. `policy` is the whitespace policy of the diff: under `Default` the whitespace at the ends of a line
/// counts, so a line of a move whose indent or whose whitespace after its text changed otherwise than the move says
/// is an edited line. `indentation_matters`: the indent of a line of the file has a meaning (Python, YAML,
/// Starlark), so such a line is one of the edited lines a move holds a quarter of at most
pub fn find_moves(old: &str, new: &str, blocks: &[DiffBlock], runs: &[LineRun], policy: WhitespacePolicy, indentation_matters: bool) -> Vec<LineMove> {
  find_moves_counted(old, new, blocks, runs, policy, indentation_matters).0
}

/// `find_moves` with what the search cost: for a benchmark and the tests of the limits
pub fn find_moves_counted(old: &str, new: &str, blocks: &[DiffBlock], runs: &[LineRun], policy: WhitespacePolicy, indentation_matters: bool) -> (Vec<LineMove>, MoveWork) {
  find_moves_within(old, new, blocks, runs, policy, indentation_matters, &LIMITS)
}

/// `find_moves_counted` within `limits`
pub(crate) fn find_moves_within(
  old: &str,
  new: &str,
  blocks: &[DiffBlock],
  runs: &[LineRun],
  policy: WhitespacePolicy,
  indentation_matters: bool,
  limits: &Limits,
) -> (Vec<LineMove>, MoveWork) {
  let none = || (Vec::new(), MoveWork::default());
  let (mut old_spans, mut new_spans) = (free_spans(blocks, runs, false), free_spans(blocks, runs, true));
  // most diffs: lines added alone, or deleted alone. Neither text is read
  if old_spans.is_empty() || new_spans.is_empty() {
    return none();
  }
  let (old_starts, new_starts) = (line_starts(old), line_starts(new));
  without_empty_last_line(&mut old_spans, old, &old_starts);
  without_empty_last_line(&mut new_spans, new, &new_starts);
  let count = |spans: &[Span]| spans.iter().map(|span| span.lines.len()).sum::<usize>();
  let (old_count, new_count) = (count(&old_spans), count(&new_spans));
  if old_count == 0 || new_count == 0 || old_count > limits.lines || new_count > limits.lines {
    return none();
  }
  // a move has its two ends in two blocks
  let one_block = |spans: &[Span]| spans.first().map(|span| span.block).filter(|block| spans.iter().all(|span| span.block == *block));
  if one_block(&old_spans).is_some_and(|block| one_block(&new_spans) == Some(block)) {
    return none();
  }

  let mut keys: FxHashMap<&str, u32> = FxHashMap::default();
  let old_lines = side_lines(old, &old_starts, &old_spans, &mut keys);
  let new_lines = side_lines(new, &new_starts, &new_spans, &mut keys);
  // the new lines of each key, in their order
  let mut by_key: Vec<Vec<u32>> = vec![Vec::new(); keys.len() + 1];
  for (index, line) in new_lines.iter().enumerate() {
    if line.key != BLANK {
      by_key[line.key as usize].push(to_u32(index));
    }
  }

  let ends_count = policy == WhitespacePolicy::Default;
  let mut finder = Finder {
    old: old_lines,
    new: new_lines,
    ends_count,
    nesting: ends_count && indentation_matters,
    limits: *limits,
    similar: FxHashMap::default(),
    work: MoveWork::default(),
    found: BinaryHeap::new(),
    choosing: false,
  };
  // the exact pairs of the chains read: a pair is in one chain
  let mut seen: FxHashSet<(u32, u32)> = FxHashSet::default();
  // the pairs of the chain being read
  let mut pairs: Vec<Pair> = Vec::new();
  'search: for old_index in 0..finder.old.len() {
    let line = &finder.old[old_index];
    let (key, block) = (line.key, line.block);
    let candidates = &by_key[key as usize];
    if key == BLANK || candidates.len() > limits.key_candidates {
      continue;
    }
    for &new_index in candidates {
      if finder.work.search >= limits.search {
        break 'search;
      }
      finder.work.search += 1;
      if finder.new[new_index as usize].block == block || seen.contains(&(to_u32(old_index), new_index)) {
        continue;
      }
      // a pair whose indents are no shift of each other starts no chain: it is an edited line of another one
      let Some(shift) = finder.shift_of_pair(old_index, new_index as usize) else { continue };
      let (old_from, new_from) = finder.chain(old_index, new_index as usize, shift, &mut pairs);
      for (offset, pair) in pairs.iter().enumerate() {
        if *pair == Pair::Exact {
          seen.insert((to_u32(old_from + offset), to_u32(new_from + offset)));
        }
      }
      finder.moves_of(old_from, new_from, &mut pairs, shift);
    }
  }

  // the longest first, each line once
  finder.choosing = true;
  let mut moves = Vec::new();
  // the lines of each move taken, by their places among the lines of their sides: its first old one, its first new
  // one and their number
  let mut taken: Vec<(usize, usize, usize)> = Vec::new();
  let mut lines = TakenLines::new(finder.old.len(), finder.new.len());
  while let Some(Reverse(candidate)) = finder.found.pop() {
    if finder.work.choice >= limits.choice {
      break;
    }
    let (old_at, new_at, len) = (candidate.old as usize, candidate.new as usize, candidate.len.0 as usize);
    finder.work.choice += len as u64;
    let rows = match lines.take(old_at, new_at, len) {
      Ok(()) => {
        moves.push(finder.line_move(&candidate));
        taken.push((old_at, new_at, len));
        continue;
      }
      // the rows of it that are still free are asked again
      Err(rows) => rows,
    };
    for row in rows {
      pairs.clear();
      for offset in row.clone() {
        let pair = finder.read_pair(old_at + offset, new_at + offset, candidate.shift);
        pairs.push(pair);
      }
      finder.moves_of(old_at + row.start, new_at + row.start, &mut pairs, candidate.shift);
    }
  }
  // the blank lines that moved with each, from the longest on
  for (line_move, (old_at, new_at, len)) in moves.iter_mut().zip(taken) {
    // deleted before it and added after it
    let before = finder.blanks_beside(&lines.old, &lines.new, old_at, new_at + len, false);
    lines.old[old_at - before..old_at].fill(true);
    lines.new[new_at + len..new_at + len + before].fill(true);
    // deleted after it and added before it
    let after = finder.blanks_beside(&lines.old, &lines.new, old_at + len, new_at, true);
    lines.old[old_at + len..old_at + len + after].fill(true);
    lines.new[new_at - after..new_at].fill(true);
    (line_move.blanks_before, line_move.blanks_after) = (to_u32(before), to_u32(after));
  }
  moves.sort_by_key(|line_move| line_move.new_start);
  (moves, finder.work)
}

/// The lines of the two sides that the moves chosen took, by their places among the lines of their sides: a line of
/// a side is in one move
pub(crate) struct TakenLines {
  pub(crate) old: Vec<bool>,
  pub(crate) new: Vec<bool>,
}

impl TakenLines {
  pub(crate) fn new(old: usize, new: usize) -> Self {
    Self {
      old: vec![false; old],
      new: vec![false; new],
    }
  }

  /// Takes the row of `len` pairs from the old line at `old_at` and the new line at `new_at` when every line of it
  /// is free; else takes nothing and answers the rows of it that are still free, by their places in it
  pub(crate) fn take(&mut self, old_at: usize, new_at: usize, len: usize) -> Result<(), Vec<Range<usize>>> {
    let free = |offset: usize| !self.old[old_at + offset] && !self.new[new_at + offset];
    if (0..len).all(free) {
      self.old[old_at..old_at + len].fill(true);
      self.new[new_at..new_at + len].fill(true);
      return Ok(());
    }
    let mut rows: Vec<Range<usize>> = Vec::new();
    let mut offset = 0;
    while offset < len {
      if !free(offset) {
        offset += 1;
        continue;
      }
      let from = offset;
      while offset < len && free(offset) {
        offset += 1;
      }
      rows.push(from..offset);
    }
    Err(rows)
  }
}

/// Lines of one block that a move may take, on one side
pub(crate) struct Span {
  pub(crate) block: u32,
  pub(crate) lines: Range<u32>,
}

/// The lines of `blocks` a move may take on a side, in order: those of the blocks that are a change, without the
/// lines of `runs` (by `old_start`, in the order of both sides, each inside one block)
pub(crate) fn free_spans(blocks: &[DiffBlock], runs: &[LineRun], new_side: bool) -> Vec<Span> {
  let mut spans = Vec::new();
  let mut runs = runs.iter().peekable();
  for (index, block) in blocks.iter().enumerate() {
    let lines = if new_side { block.new_start..block.new_end } else { block.old_start..block.old_end };
    let mut free = |from: u32, to: u32| {
      if block.kind == BlockKind::Changed && from < to {
        spans.push(Span {
          block: to_u32(index),
          lines: from..to,
        });
      }
    };
    let mut from = lines.start;
    while let Some(run) = runs.next_if(|run| run.old_start < block.old_end || run.new_start < block.new_end) {
      let run = if new_side { run.new_start..run.new_end } else { run.old_start..run.old_end };
      free(from, run.start.min(lines.end));
      from = from.max(run.end);
    }
    free(from, lines.end);
  }
  spans
}

/// `spans`, lines of `text` whose lines start at `starts`, without what is no line of the text to a move: the empty
/// line after its last line end, and the one line of an empty text. The comparison pairs that line with no line but
/// its like and no count takes it (docs/design/text-diff.md, Key Decision 8), so it left with no move, as a pair of
/// a blank line or as a blank line beside one
pub(crate) fn without_empty_last_line(spans: &mut Vec<Span>, text: &str, starts: &[usize]) {
  let lines = to_u32(starts.len() - usize::from(starts.last() == Some(&text.len())));
  spans.retain_mut(|span| {
    span.lines.end = span.lines.end.min(lines);
    !span.lines.is_empty()
  });
}

/// A line that a move may take
struct Line<'t> {
  /// Its number in its text, from 0
  number: u32,
  /// The block it is in
  block: u32,
  /// Its text without the whitespace at its ends
  text: &'t str,
  /// The number of its key: the same for the same `text`, `BLANK` for none
  key: u32,
  /// The whitespace before its text, spaces and tabs: a byte and a UTF-16 code unit each. All of a blank line
  indent: &'t str,
  /// The whitespace after its text
  trail: &'t str,
  /// The UTF-16 code units of its text
  units: u32,
  /// The characters of its text that are no whitespace
  solid: u32,
}

/// The whitespace of the comparison inside a line (`trim_util::is_space_enter_or_tab`)
pub(crate) const WHITESPACE: [char; 2] = [' ', '\t'];

/// The byte each line of `text` starts at, as the comparison counts lines: `\n`, `\r\n` and a lone `\r` end one
pub(crate) fn line_starts(text: &str) -> Vec<usize> {
  let mut starts: Vec<usize> = vec![0];
  if text.contains('\r') {
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
      if *byte == b'\n' || (*byte == b'\r' && bytes.get(index + 1) != Some(&b'\n')) {
        starts.push(index + 1);
      }
    }
  } else {
    // nearly every text: the search for one character is the fast one
    starts.extend(text.match_indices('\n').map(|(index, _)| index + 1));
  }
  starts
}

/// The lines of `spans` of `text`, whose lines start at `starts`
fn side_lines<'t>(text: &'t str, starts: &[usize], spans: &[Span], keys: &mut FxHashMap<&'t str, u32>) -> Vec<Line<'t>> {
  let mut lines = Vec::new();
  for span in spans {
    for line in span.lines.clone() {
      let start = starts.get(line as usize).copied().unwrap_or(text.len());
      let end = starts.get(line as usize + 1).copied().unwrap_or(text.len());
      let whole = text.get(start..end).unwrap_or_default().trim_end_matches(['\n', '\r']);
      let indented = whole.trim_start_matches(WHITESPACE);
      let trimmed = indented.trim_end_matches(WHITESPACE);
      let key = if trimmed.is_empty() {
        BLANK
      } else {
        let next = to_u32(keys.len() + 1);
        *keys.entry(trimmed).or_insert(next)
      };
      let (mut units, mut solid) = (0, 0);
      for character in trimmed.chars() {
        units += character.len_utf16();
        solid += usize::from(!WHITESPACE.contains(&character));
      }
      lines.push(Line {
        number: line,
        block: span.block,
        text: trimmed,
        key,
        indent: &whole[..whole.len() - indented.len()],
        trail: &indented[trimmed.len()..],
        units: to_u32(units),
        solid: to_u32(solid),
      });
    }
  }
  lines
}

/// What an old line and a new line are to each other in a move
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pair {
  /// The same key
  Exact,
  /// Both blank
  Blank,
  /// Similar by word, or the same key at an indent that did not move as its move where the indentation has a meaning
  Edited,
  /// Other keys that may be similar by word, not compared yet: edited or apart. A chain grows through such a pair,
  /// and only a row that may be a move with it edited has it compared (`Finder::moves_of`)
  Unread,
  /// No pair of a move
  Apart,
}

/// What a move did to the indent of its lines: the whitespace `text` added to each at one end of its indent, or
/// taken from each there. No whitespace is no shift at all
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Shift<'t> {
  /// The columns gained, the length of `text`; negative for lost ones
  delta: i64,
  text: &'t str,
  /// At the end of the indent, before the text of the line; at its start else
  inner: bool,
}

impl<'t> Shift<'t> {
  const NONE: Self = Self { delta: 0, text: "", inner: false };

  /// The shifts that make the indent `new` of `old`, at most two: at the start of the indent, then at its end.
  /// None when neither indent is the other with whitespace added at one end
  fn between(old: &'t str, new: &'t str) -> [Option<Self>; 2] {
    let delta = i64::try_from(new.len()).unwrap_or(i64::MAX) - i64::try_from(old.len()).unwrap_or(i64::MAX);
    let (short, long) = if delta >= 0 { (old, new) } else { (new, old) };
    let shift = |text: &'t str, inner: bool| Self { delta, text, inner };
    [
      long.strip_suffix(short).map(|text| shift(text, false)),
      // of two indents of one length the first says it all
      long.strip_prefix(short).filter(|_| delta != 0).map(|text| shift(text, true)),
    ]
  }

  /// Whether the indent `new` is `old` moved by this shift
  fn holds(&self, old: &str, new: &str) -> bool {
    let (short, long) = if self.delta >= 0 { (old, new) } else { (new, old) };
    let added = if self.inner { long.strip_prefix(short) } else { long.strip_suffix(short) };
    added == Some(self.text)
  }

  fn columns(&self) -> i32 {
    i32::try_from(self.delta).unwrap_or(if self.delta < 0 { i32::MIN } else { i32::MAX })
  }
}

/// A move that may be taken, in the order they are taken in: the longest, then the fewest edited lines, then the
/// first by its new and by its old lines
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Candidate<'t> {
  len: Reverse<u32>,
  edited: u32,
  /// The place of its first new line among the lines of the new side, and of its first old line
  new: u32,
  old: u32,
  /// The shift of its exact lines, where the indentation has a meaning
  shift: Shift<'t>,
}

struct Finder<'t> {
  old: Vec<Line<'t>>,
  new: Vec<Line<'t>>,
  /// The whitespace at the ends of a line counts: the policy of the diff counts every character
  ends_count: bool,
  /// It counts and the indentation has a meaning: a line whose indent did not move as its move is an edited line
  nesting: bool,
  limits: Limits,
  /// Whether an old and a new line with other keys are a similar pair, by their places
  similar: FxHashMap<(u32, u32), bool>,
  work: MoveWork,
  /// The moves that may be taken, the first to take first
  found: BinaryHeap<Reverse<Candidate<'t>>>,
  /// The search is over and the choice among the moves found reads the rows: their work is the choice's
  choosing: bool,
}

/// Whether the line at `index` follows the line before it in its block
const fn follows(lines: &[Line<'_>], index: usize) -> bool {
  index > 0 && index < lines.len() && lines[index - 1].block == lines[index].block && lines[index - 1].number + 1 == lines[index].number
}

/// The whitespace of the blank line `count` lines away from a move, in the block of the line of the move it is
/// beside and taken by no move: before the line at `at` (`backwards`), or from the line at `at` on
fn blank_beside<'t>(lines: &[Line<'t>], taken: &[bool], at: usize, backwards: bool, count: usize) -> Option<&'t str> {
  let index = if backwards { at.checked_sub(count + 1)? } else { at + count };
  let joined = follows(lines, if backwards { index + 1 } else { index });
  (joined && lines[index].key == BLANK && !taken[index]).then(|| lines[index].indent)
}

impl<'t> Finder<'t> {
  /// The shift a chain that grows from the old line at `old` and the new line at `new`, of one key, reads its pairs
  /// by. Where the indentation has a meaning: what made the one indent of the other, at the start of the indent
  /// before its end; `None` when nothing did (a tab that became a space). Elsewhere a pair is exact at any indent
  fn shift_of_pair(&self, old: usize, new: usize) -> Option<Shift<'t>> {
    if !self.nesting {
      return Some(Shift::NONE);
    }
    Shift::between(self.old[old].indent, self.new[new].indent).into_iter().flatten().next()
  }

  /// What the old line at `old` and the new line at `new` are to each other in a move of `shift`. Two lines of
  /// other keys that may be similar are `Unread` until they are compared by word (`read`): most such pairs stand in
  /// rows that are no move whatever they are, and the comparison is what a pair costs
  fn pair(&mut self, old: usize, new: usize, shift: Shift<'t>) -> Pair {
    let (old_line, new_line) = (&self.old[old], &self.new[new]);
    if old_line.key == new_line.key {
      return if old_line.key == BLANK {
        Pair::Blank
      } else if self.nesting && !shift.holds(old_line.indent, new_line.indent) {
        Pair::Edited
      } else {
        Pair::Exact
      };
    }
    if old_line.key == BLANK || new_line.key == BLANK {
      return Pair::Apart;
    }
    let key = (to_u32(old), to_u32(new));
    if let Some(similar) = self.similar.get(&key) {
      return if *similar { Pair::Edited } else { Pair::Apart };
    }
    let (old_units, new_units) = (old_line.units as usize, new_line.units as usize);
    if old_units > MAX_PAIR_UNITS || new_units > MAX_PAIR_UNITS {
      self.similar.insert(key, false);
      return Pair::Apart;
    }
    // the two lines are read for their likeness: work by their length
    self.work.search += ((old_units + new_units) / SCANNED_UNITS_A_PAIR) as u64;
    // past the pairs a search leaves unread, a pair is apart: a chain ends at it, as it does past the pairs compared
    if !may_be_similar(old_line.text, old_units, new_line.text, new_units) || self.work.unread >= self.limits.unread {
      self.similar.insert(key, false);
      return Pair::Apart;
    }
    self.work.unread += 1;
    Pair::Unread
  }

  /// `pair`, compared by word where it is unread: edited or apart
  fn read_pair(&mut self, old: usize, new: usize, shift: Shift<'t>) -> Pair {
    match self.pair(old, new, shift) {
      Pair::Unread => self.read(old, new),
      pair => pair,
    }
  }

  /// An unread pair compared by word: edited when the two lines are a similar pair, else apart; apart past the
  /// number of pairs a search compares
  fn read(&mut self, old: usize, new: usize) -> Pair {
    let similar = self.work.compared < self.limits.compared && self.compare(old, new);
    self.similar.insert((to_u32(old), to_u32(new)), similar);
    if similar { Pair::Edited } else { Pair::Apart }
  }

  /// Whether two lines with other keys that may be similar are a similar pair by word
  fn compare(&mut self, old: usize, new: usize) -> bool {
    let (old_line, new_line) = (&self.old[old], &self.new[new]);
    self.work.compared += 1;
    let (old_units, new_units): (Vec<u16>, Vec<u16>) = (old_line.text.encode_utf16().collect(), new_line.text.encode_utf16().collect());
    let Some(fragments) = fragments_of(&old_units, &new_units) else { return false };
    let ranges = |length: usize, side: fn(&DiffFragment) -> (usize, usize)| LineRanges {
      length,
      ranges: fragments.iter().map(side).filter(|(start, end)| start < end).collect(),
    };
    is_similar_pair(
      &ranges(old_units.len(), |fragment| (fragment.start_offset1, fragment.end_offset1)),
      &ranges(new_units.len(), |fragment| (fragment.start_offset2, fragment.end_offset2)),
    )
  }

  /// The chain of the pair of the old line at `old` and the new line at `new`, which have the same key: its pairs
  /// into `pairs`, and the places of its first old and its first new line
  fn chain(&mut self, old: usize, new: usize, shift: Shift<'t>, pairs: &mut Vec<Pair>) -> (usize, usize) {
    pairs.clear();
    let (mut old_from, mut new_from) = (old, new);
    while follows(&self.old, old_from) && follows(&self.new, new_from) && self.work.search < self.limits.search {
      self.work.search += 1;
      let pair = self.pair(old_from - 1, new_from - 1, shift);
      if pair == Pair::Apart {
        break;
      }
      pairs.push(pair);
      (old_from, new_from) = (old_from - 1, new_from - 1);
    }
    pairs.reverse();
    pairs.push(Pair::Exact);
    let (mut old_to, mut new_to) = (old + 1, new + 1);
    while follows(&self.old, old_to) && follows(&self.new, new_to) && self.work.search < self.limits.search {
      self.work.search += 1;
      let pair = self.pair(old_to, new_to, shift);
      if pair == Pair::Apart {
        break;
      }
      pairs.push(pair);
      (old_to, new_to) = (old_to + 1, new_to + 1);
    }
    (old_from, new_from)
  }

  /// The moves of a row of pairs that starts at the old line at `old` and the new line at `new`, added to `found`:
  /// its longest row that may be a move (`longest`) with the blank pairs beside it, when its exact lines are long
  /// enough, then the same of what is left before it and after it. A pair that is apart ends a row. The unread
  /// pairs of a row that may be a move with them edited are compared by word, and the row is asked again when one
  /// of them is apart: the answer is that of rows whose every pair was read, at the cost of the pairs that matter.
  ///
  /// What is left of a row is read again, and counts as work (`read_again`); at the limit of that work the rest of
  /// the row is not asked. A move of at least half of its row leaves at most half of it to read, so such rows cost
  /// twice their pairs at most; a longer row of shorter moves is read once for all of them (`many_moves`)
  fn moves_of(&mut self, old: usize, new: usize, pairs: &mut [Pair], shift: Shift<'t>) {
    // one pair, as of most chains of a file whose lines repeat: nothing to choose
    if let [pair] = pairs {
      if *pair == Pair::Exact {
        self.found_row(old, new, pairs, 0..1, shift);
      }
      return;
    }
    let mut rows: Vec<Range<usize>> = Vec::new();
    cut_rows(pairs, 0..pairs.len(), &mut rows);
    while let Some(row) = rows.pop() {
      let Some((first, last)) = longest(&pairs[row.clone()]) else { continue };
      if row.len() > self.limits.short_row && (last + 1 - first) * 2 < row.len() {
        if !self.many_moves(old, new, pairs, row, shift) {
          return;
        }
        continue;
      }
      let (mut first, mut last) = (row.start + first, row.start + last);
      let mut apart = false;
      for (index, pair) in pairs.iter_mut().enumerate().take(last + 1).skip(first) {
        if *pair == Pair::Unread {
          *pair = self.read(old + index, new + index);
          apart |= *pair == Pair::Apart;
        }
      }
      if apart {
        if !self.read_again(row.len()) {
          return;
        }
        cut_rows(pairs, row, &mut rows);
        continue;
      }
      while first > row.start && pairs[first - 1] == Pair::Blank {
        first -= 1;
      }
      while last + 1 < row.end && pairs[last + 1] == Pair::Blank {
        last += 1;
      }
      self.found_row(old, new, pairs, first..last + 1, shift);
      let left = (first - row.start) + (row.end - last - 1);
      if left > 0 {
        if !self.read_again(left) {
          return;
        }
        rows.push(row.start..first);
        rows.push(last + 1..row.end);
      }
    }
  }

  /// The moves of `row` of `pairs`, a row without a pair that is apart, as `moves_of` finds them, where the row
  /// holds many moves that are short: read again for each of them, a list of n lines whose every fifth is exact
  /// costs n² / 10 pairs. Here each exact pair is asked once how far a move that starts with it reaches (`Reach`),
  /// and the longest is taken first, of as long ones the first: the longest row of what is left then, since a move
  /// taken and a pair found apart only cut the reach of the starts before them, which are asked again as they come
  /// up. Each start asked is a pair of work; false at the limit of that work
  fn many_moves(&mut self, old: usize, new: usize, pairs: &mut [Pair], row: Range<usize>, shift: Shift<'t>) -> bool {
    if !self.read_again(row.len()) {
      return false;
    }
    let length = row.len();
    let reach = Reach::new(&pairs[row.clone()]);
    // the pairs of the moves taken, and where a row ends: at the first pair of a move taken and at a pair found
    // apart, by their places in `row`
    let mut taken = vec![false; length];
    let mut ends: BTreeSet<usize> = BTreeSet::new();
    // each start by the pairs of its longest row when last asked, never fewer than now; the first of as long ones
    let mut starts: BinaryHeap<(usize, Reverse<usize>)> = (0..length)
      .filter(|start| pairs[row.start + start] == Pair::Exact)
      .map(|start| (reach.of(start, length) + 1 - start, Reverse(start)))
      .collect();
    while let Some((len, Reverse(start))) = starts.pop() {
      if !self.read_again(1) {
        return false;
      }
      if taken[start] {
        continue;
      }
      let end = ends.range(start..).next().copied().unwrap_or(length);
      let mut last = reach.of(start, end);
      if last + 1 - start < len {
        starts.push((last + 1 - start, Reverse(start)));
        continue;
      }
      let mut apart = false;
      for index in start..=last {
        let place = row.start + index;
        if pairs[place] == Pair::Unread {
          pairs[place] = self.read(old + place, new + place);
          if pairs[place] == Pair::Apart {
            ends.insert(index);
            apart = true;
          }
        }
      }
      if apart {
        starts.push((len, Reverse(start)));
        continue;
      }
      let mut first = start;
      while first > 0 && !taken[first - 1] && pairs[row.start + first - 1] == Pair::Blank {
        first -= 1;
      }
      while last + 1 < length && !taken[last + 1] && pairs[row.start + last + 1] == Pair::Blank {
        last += 1;
      }
      self.found_row(old, new, pairs, row.start + first..row.start + last + 1, shift);
      taken[first..=last].fill(true);
      ends.insert(first);
    }
    true
  }

  /// The row `row` of `pairs`, which start at the old line at `old` and the new line at `new`, as a move that may
  /// be taken, when its exact lines are long enough
  fn found_row(&mut self, old: usize, new: usize, pairs: &[Pair], row: Range<usize>, shift: Shift<'t>) {
    let of = |kind: Pair| row.clone().filter(move |index| pairs[*index] == kind);
    let solid: usize = of(Pair::Exact).map(|index| self.old[old + index].solid as usize).sum();
    if solid >= MIN_MOVE_CHARS {
      let edited = to_u32(of(Pair::Edited).count());
      self.found.push(Reverse(Candidate {
        len: Reverse(to_u32(row.len())),
        edited,
        new: to_u32(new + row.start),
        old: to_u32(old + row.start),
        shift,
      }));
    }
  }

  /// Counts `pairs` pairs of a row that are read again as work, of the search or of the choice among its moves;
  /// false at the limit of that work, where no more is read
  const fn read_again(&mut self, pairs: usize) -> bool {
    let (work, limit) = if self.choosing {
      (&mut self.work.choice, self.limits.choice)
    } else {
      (&mut self.work.search, self.limits.search)
    };
    if *work >= limit {
      return false;
    }
    *work += pairs as u64;
    true
  }

  /// The blank lines that moved with a move and are no pair of its lines: those right before the old line at `old`
  /// that stand from the new line at `new` on (`after` false), or those from the old line at `old` on that stand
  /// right before the new line at `new`; in the blocks of the move, taken by no move, as many as both sides have.
  /// Where the whitespace at the ends of a line counts, blank lines of one text
  fn blanks_beside(&self, old_taken: &[bool], new_taken: &[bool], old: usize, new: usize, after: bool) -> usize {
    let mut count = 0;
    let mut text = None;
    while let (Some(old_blank), Some(new_blank)) = (
      blank_beside(&self.old, old_taken, old, !after, count),
      blank_beside(&self.new, new_taken, new, after, count),
    ) {
      if self.ends_count && (old_blank != new_blank || *text.get_or_insert(old_blank) != old_blank) {
        break;
      }
      count += 1;
    }
    count
  }

  /// The shift of the lines of a move where the indentation has no meaning: the one most of its exact lines agree
  /// on, the first of them among as many; none without such a line
  fn shift_of_most(&self, old: usize, new: usize, pairs: &[Pair]) -> Shift<'t> {
    let mut shifts: Vec<(Shift<'t>, usize)> = Vec::new();
    for offset in (0..pairs.len()).filter(|offset| pairs[*offset] == Pair::Exact) {
      for shift in Shift::between(self.old[old + offset].indent, self.new[new + offset].indent).into_iter().flatten() {
        match shifts.iter_mut().find(|(known, _)| *known == shift) {
          Some((_, count)) => *count += 1,
          None => shifts.push((shift, 1)),
        }
      }
    }
    shifts.iter().rev().max_by_key(|(_, count)| *count).map_or(Shift::NONE, |(shift, _)| *shift)
  }

  /// The move of a candidate that was taken, without the blank lines beside it
  fn line_move(&mut self, candidate: &Candidate<'t>) -> LineMove {
    let (old_at, new_at, len) = (candidate.old as usize, candidate.new as usize, candidate.len.0 as usize);
    let pairs: Vec<Pair> = (0..len).map(|offset| self.read_pair(old_at + offset, new_at + offset, candidate.shift)).collect();
    let shift = if self.nesting { candidate.shift } else { self.shift_of_most(old_at, new_at, &pairs) };
    let mut edited = Vec::new();
    let (mut old_ranges, mut new_ranges) = (Vec::with_capacity(len), Vec::with_capacity(len));
    for (offset, pair) in pairs.iter().enumerate() {
      let (old_line, new_line) = self.ranges_of(old_at + offset, new_at + offset, shift);
      // edited in its text or its nesting, or in the whitespace at its ends
      if *pair == Pair::Edited || !old_line.is_empty() {
        edited.push(to_u32(offset));
      }
      old_ranges.push(old_line);
      new_ranges.push(new_line);
    }
    if old_ranges.iter().all(Vec::is_empty) {
      (old_ranges, new_ranges) = (Vec::new(), Vec::new());
    }
    LineMove {
      old_start: self.old[old_at].number,
      new_start: self.new[new_at].number,
      len: to_u32(len),
      indent: shift.columns(),
      blanks_before: 0,
      blanks_after: 0,
      edited,
      old_ranges,
      new_ranges,
    }
  }

  /// The ranges of a pair of a move of `shift`, `[start, end, kind, ...]` of each of its two lines as the ranges of
  /// a block, each a text that is another on the other side: where the whitespace at the ends of a line counts, its
  /// indent when that is another text and did not move by the shift, and its whitespace after its text when that
  /// is another; between them, what changed in its text by word. None for a line that is the same on both sides but
  /// for the shift
  fn ranges_of(&self, old: usize, new: usize, shift: Shift<'t>) -> (Vec<u32>, Vec<u32>) {
    let (old_line, new_line) = (&self.old[old], &self.new[new]);
    let (mut old_ranges, mut new_ranges) = (Vec::new(), Vec::new());
    let (old_indent, new_indent) = (old_line.indent.len(), new_line.indent.len());
    if self.ends_count && old_line.indent != new_line.indent && !shift.holds(old_line.indent, new_line.indent) {
      whitespace_range(&mut old_ranges, &mut new_ranges, 0..old_indent, 0..new_indent);
    }
    let (old_end, new_end) = (old_indent + old_line.units as usize, new_indent + new_line.units as usize);
    let trail = self.ends_count && old_line.trail != new_line.trail;
    if old_line.key != new_line.key {
      let (old_units, new_units): (Vec<u16>, Vec<u16>) = (old_line.text.encode_utf16().collect(), new_line.text.encode_utf16().collect());
      for fragment in fragments_of(&old_units, &new_units).unwrap_or_default() {
        let kind = match (fragment.start_offset1 == fragment.end_offset1, fragment.start_offset2 == fragment.end_offset2) {
          (true, _) => RangeKind::Inserted,
          (_, true) => RangeKind::Deleted,
          _ => RangeKind::Modified,
        };
        old_ranges.extend([to_u32(old_indent + fragment.start_offset1), to_u32(old_indent + fragment.end_offset1), kind as u32]);
        new_ranges.extend([to_u32(new_indent + fragment.start_offset2), to_u32(new_indent + fragment.end_offset2), kind as u32]);
      }
    }
    if trail {
      whitespace_range(
        &mut old_ranges,
        &mut new_ranges,
        old_end..old_end + old_line.trail.len(),
        new_end..new_end + new_line.trail.len(),
      );
    }
    (old_ranges, new_ranges)
  }
}

/// Adds the range of a text of whitespace that is another on the other side, `old` of the old line and `new` of the
/// new one: inserted where the old line has none, deleted where the new one has none
fn whitespace_range(old_ranges: &mut Vec<u32>, new_ranges: &mut Vec<u32>, old: Range<usize>, new: Range<usize>) {
  let kind = match (old.is_empty(), new.is_empty()) {
    (true, _) => RangeKind::Inserted,
    (_, true) => RangeKind::Deleted,
    _ => RangeKind::Modified,
  };
  old_ranges.extend([to_u32(old.start), to_u32(old.end), kind as u32]);
  new_ranges.extend([to_u32(new.start), to_u32(new.end), kind as u32]);
}

/// The changes of two lines by word, every character counted; `None` when they differ in too many places
fn fragments_of(old: &[u16], new: &[u16]) -> Option<Vec<DiffFragment>> {
  by_word::compare(old, new, ComparisonPolicy::Default, &Deadline::NONE).ok()
}

/// Whether two lines of `old_units` and `new_units` UTF-16 code units may be a similar pair
/// (`inline::is_similar_pair`: less than a third of their code units changed), before they are compared by word: a
/// side changes at least what it is longer by, and keeps at most the code units the other side has too
fn may_be_similar(old: &str, old_units: usize, new: &str, new_units: usize) -> bool {
  let total = old_units + new_units;
  let similar = |changed: usize| changed * SIMILAR_PAIR_MAX_CHANGED.1 < total * SIMILAR_PAIR_MAX_CHANGED.0;
  if !similar(old_units.abs_diff(new_units)) {
    return false;
  }
  // the code units both sides have, counted by their low byte: no fewer than they share
  let mut counts = [0i32; 256];
  for unit in old.encode_utf16() {
    counts[usize::from(unit & 0xff)] += 1;
  }
  let mut shared = 0;
  for unit in new.encode_utf16() {
    let count = &mut counts[usize::from(unit & 0xff)];
    if *count > 0 {
      *count -= 1;
      shared += 1;
    }
  }
  similar(total - 2 * shared)
}

/// The rows of `pairs` in `row` between its pairs that are apart, added to `rows`
fn cut_rows(pairs: &[Pair], row: Range<usize>, rows: &mut Vec<Range<usize>>) {
  let mut from = row.start;
  for (index, pair) in pairs.iter().enumerate().take(row.end).skip(row.start) {
    if *pair == Pair::Apart {
      rows.push(from..index);
      from = index + 1;
    }
  }
  rows.push(from..row.end);
}

/// What a pair counts in a row that may be a move, one whose count is not negative: an exact pair the numerator of
/// `MAX_EDITED_LINES`, an edited one the numerator less the denominator, an unread one as an edited one (what it is
/// at best), a blank one nothing
const fn count_of(pair: Pair) -> i64 {
  match pair {
    Pair::Exact => MAX_EDITED_LINES.0,
    Pair::Edited | Pair::Unread => MAX_EDITED_LINES.0 - MAX_EDITED_LINES.1,
    Pair::Blank | Pair::Apart => 0,
  }
}

/// The count of the pairs before each of `pairs`, and of all of them last
fn counts_before(pairs: &[Pair]) -> Vec<i64> {
  let mut before: Vec<i64> = Vec::with_capacity(pairs.len() + 1);
  let mut count = 0;
  before.push(count);
  for pair in pairs {
    count += count_of(*pair);
    before.push(count);
  }
  before
}

/// How far a move that starts with an exact pair of a row reaches, asked of any start and any end of the row
/// (`longest` answers for the whole row alone): the last exact pair before the end whose row from the start has a
/// count that is not negative
struct Reach {
  /// The count of the pairs before each pair, and of all of them last
  before: Vec<i64>,
  /// The last exact pair at or before each pair
  exact: Vec<usize>,
  /// The highest count after a pair of each stretch of pairs: a tree of stretches by halves, the two halves of the
  /// stretch at `node` at `2 * node` and `2 * node + 1`, the stretches of one pair from `leaves` on
  highest: Vec<i64>,
  leaves: usize,
}

impl Reach {
  fn new(pairs: &[Pair]) -> Self {
    let before = counts_before(pairs);
    let mut exact = Vec::with_capacity(pairs.len());
    let mut last = 0;
    for (index, pair) in pairs.iter().enumerate() {
      if *pair == Pair::Exact {
        last = index;
      }
      exact.push(last);
    }
    // more leaves than pairs: the place after the last pair, where a row may end, is one of them
    let leaves = (pairs.len() + 1).next_power_of_two();
    let mut highest = vec![i64::MIN; 2 * leaves];
    highest[leaves..leaves + pairs.len()].copy_from_slice(&before[1..]);
    for node in (1..leaves).rev() {
      highest[node] = highest[2 * node].max(highest[2 * node + 1]);
    }
    Self { before, exact, highest, leaves }
  }

  /// The last pair of the longest row that starts with the exact pair `start`, ends before the pair `end` and may
  /// be a move: an exact pair, `start` itself at the least
  fn of(&self, start: usize, end: usize) -> usize {
    let count = self.before[start];
    // the last pair before `end` with a count after it that is not below the one before `start`, which the pair
    // `start` is: the stretches before `end` from the last to the first, each the left half of its parent
    let mut node = self.leaves + end;
    while node > 1 {
      if node % 2 == 1 && self.highest[node - 1] >= count {
        node -= 1;
        while node < self.leaves {
          node = if self.highest[2 * node + 1] >= count { 2 * node + 1 } else { 2 * node };
        }
        // the pairs after an exact one up to there are edited or blank: they count nothing or less
        return self.exact[node - self.leaves];
      }
      node /= 2;
    }
    start
  }
}

/// The longest row of `pairs`, none of them apart, that starts and ends with an exact pair and whose edited pairs
/// are at most `MAX_EDITED_LINES` of its pairs that are not blank, as its first and its last place; the first of the
/// longest. `None` without an exact pair. An unread pair counts as an edited one: what it is at best.
///
/// A row may be a move when the count of its pairs is not negative (`count_of`). A row may start only at an exact
/// pair with a lower count before it than every such pair before; from the last pair back, each such start is
/// paired with the last end it reaches
fn longest(pairs: &[Pair]) -> Option<(usize, usize)> {
  let before = counts_before(pairs);
  let mut starts: Vec<usize> = Vec::new();
  for (index, pair) in pairs.iter().enumerate() {
    if *pair == Pair::Exact && starts.last().is_none_or(|last| before[index] < before[*last]) {
      starts.push(index);
    }
  }
  let mut best: Option<(usize, usize)> = None;
  for end in (0..pairs.len()).rev().filter(|end| pairs[*end] == Pair::Exact) {
    while let Some(&start) = starts.last() {
      if start <= end {
        if before[end + 1] < before[start] {
          break;
        }
        if best.is_none_or(|(first, last)| end - start > last - first || (end - start == last - first && start < first)) {
          best = Some((start, end));
        }
      }
      starts.pop();
    }
  }
  best
}
