//! The diff of two texts: the changed blocks of lines with their kind and the changed ranges inside their lines,
//! built straight from the comparison (`diff_texts`), with the types of the marks a reader reads beside them that the
//! comparison and the moves know: the runs of lines that changed in one way alone (`LineRun`) and the lines that moved
//! (`LineMove`, `moves`). Where the reading of each block starts follows from them (`starts_of`).
//!
//! Lines are IntelliJ's (`SourceText`): `\n`, `\r\n` and a lone `\r` end a line, and a text has one line more than it
//! has line ends. Line ends compare as equal; `LineDiff::line_ends` names them when the files differ in them.
//!
//! Values of the comparison: a consumer that sends them to another program or keeps them describes them by its own
//! types, as it likes them there.

use crate::cancellation::{Deadline, DiffTooBig};
use crate::comparison_util::is_equal_texts;
use crate::inline::{LineRanges, inline_pairs, ranges_dropped};
use crate::policy::{ComparisonPolicy, HighlightPolicy, WhitespacePolicy};
use crate::provider::{ViewFragment, compare_for_view};
use crate::text::{LineEndKind, SourceText};
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// The inner ranges are compared only when each side has at most this many lines
pub const MAX_INNER_LINES: usize = 20_000;
/// The inner ranges are compared only when each side has at most this many UTF-8 bytes
pub const MAX_INNER_BYTES: usize = 1 << 20;

/// The diff of two texts
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineDiff {
  pub old_line_count: u32,
  pub new_line_count: u32,
  /// The changed lines of the new text under the policy: an `Ignored` block, and the empty line after a final line
  /// end, do not count
  pub additions: u32,
  /// The same for the old text
  pub deletions: u32,
  /// The line ends of the two files when they differ; `None` when they are the same, or a file has no line end
  pub line_ends: Option<LineEnds>,
  /// Whether each text ends with a line end, when only one does ("Newline at end of file added"); `None` when they
  /// agree, or a text is empty (an added or a deleted file)
  pub final_newline: Option<FinalNewline>,
  /// Whether the last line of each text is the empty line that does not count: the line after a final line end, or
  /// the one line of an empty text
  pub empty_last_line: EmptyLastLine,
  /// The inner ranges were not compared (a side over a limit, or out of time): the blocks are still exact
  pub line_level_only: bool,
  /// In order and without overlap; the lines between two blocks are equal under the policy
  pub blocks: Vec<DiffBlock>,
}

/// A run of lines inside one block that changed in one way alone: the old lines `[old_start, old_end)` became the new
/// lines `[new_start, new_end)` (0-based, the ends exclusive).
/// Lines on both sides, but for the part of a run read across the blocks the comparison cut out of one change
/// (`diff_marks::marks_joined`) in a block with lines on one side alone
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineRun {
  pub old_start: u32,
  pub old_end: u32,
  pub new_start: u32,
  pub new_end: u32,
  pub kind: RunKind,
  /// The run provably changes no behaviour: a reader passes it, and a stop of the reading is never in it. A run that
  /// is not quiet is a label
  pub quiet: bool,
}

/// What alone changed in a run
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunKind {
  /// The same code and the same comments: line breaks, indents, spaces between tokens
  Formatting,
  /// The same code, other comments
  Comments,
  /// The same code but for a renamed name
  Renamed,
}

/// `len` lines that moved from `old_start` of the old text to `new_start` of the new one (0-based): old line
/// `old_start + i` is new line `new_start + i`. Never quiet: the new lines are read where they stand now. The blank
/// lines that left with it and stand at its other end now are counted beside its lines (`blanks_before`,
/// `blanks_after`): they are lines of the move on both sides, and no pair of its lines
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineMove {
  pub old_start: u32,
  pub new_start: u32,
  pub len: u32,
  /// The columns of whitespace its lines gained at their start, negative for lost ones: "re-indented". Of every line
  /// that is the same on both sides where the indentation of the file has a meaning, of most of them elsewhere
  pub indent: i32,
  /// The blank lines right before its old lines that stand right after its new lines: the old lines
  /// `[old_start - blanks_before, old_start)`, the new lines `[new_start + len, new_start + len + blanks_before)`
  pub blanks_before: u32,
  /// The blank lines right after its old lines that stand right before its new lines: the old lines
  /// `[old_start + len, old_start + len + blanks_after)`, the new lines `[new_start - blanks_after, new_start)`
  pub blanks_after: u32,
  /// The lines of the move that were edited on the way, by their place in it from 0, in order: a line whose text
  /// changed, a line whose indent or whose whitespace after its text changed otherwise than the move says where the
  /// policy of the diff counts that whitespace, and a line whose indent did not move as the indents of the move
  /// where the indentation of the file has a meaning (its nesting changed, also when no character of it did)
  pub edited: Vec<u32>,
  /// For each line of the move, `[start, end, kind, ...]` as the ranges of a `DiffBlock`, the two lines of a pair
  /// with as many ranges of the same kinds, each a text that is another on the other side: what an edited line
  /// changed in. A line without a range is the same on both sides but for the indent of the move, and for the
  /// whitespace at its ends where the policy of the diff leaves that out. Empty lists for a move none of whose
  /// lines has a range
  pub old_ranges: Vec<Vec<u32>>,
  pub new_ranges: Vec<Vec<u32>>,
}

/// A block of changed lines
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffBlock {
  /// 0-based line indices, the end exclusive; `start == end` is a side without lines
  pub old_start: u32,
  pub old_end: u32,
  pub new_start: u32,
  pub new_end: u32,
  pub kind: BlockKind,
  /// The Inline view may draw pair i as one row (`pairs_align`): the sides have the same number of lines, each pair is
  /// similar, each inner change lies in one pair, and outside its changes a pair is the same text on both sides but
  /// for whitespace. So the ranges of line i of both sides align by index, and the row of a pair shows all of both
  /// its lines.
  pub inline_pairs: bool,
  /// For each line of the old side, `[start, end, kind, start, end, kind, ...]`: UTF-16 offsets inside the line, the
  /// end exclusive, never the line end, and the `RangeKind` of the inner change, as IntelliJ types a fragment.
  /// `start == end` marks the empty side of an inserted or a deleted change, which a view draws as a thin bar. Empty
  /// when the block has no inner ranges: `Ignored`, `WhitespaceOnly`, ranges dropped, or `line_level_only`.
  pub old_ranges: Vec<Vec<u32>>,
  pub new_ranges: Vec<Vec<u32>>,
}

/// How a view draws a block
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockKind {
  /// A change
  Changed,
  /// The lines are equal under the policy, an empty list of inner changes (whitespace only, or a blank line under
  /// `Ignore`): drawn pale, still a change that counts and that the navigation stops at, as in IntelliJ
  WhitespaceOnly,
  /// `IgnoreBlankLines` cut the block off the ends of a run of changes, where IntelliJ drops it: drawn as unchanged,
  /// not counted, and the change navigation skips it. A view needs it to align the two sides.
  Ignored,
}

/// The line ends of two files that differ in them
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineEnds {
  pub old: LineEndKind,
  pub new: LineEndKind,
  /// The files differ in their line ends only
  pub only: bool,
}

/// Whether each text ends with a line end
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalNewline {
  pub old: bool,
  pub new: bool,
}

/// Whether the last line of each text is empty and does not count
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyLastLine {
  pub old: bool,
  pub new: bool,
}

impl LineMove {
  /// Its old lines with the blank lines that left with it
  pub const fn old_lines(&self) -> Range<u32> {
    self.old_start - self.blanks_before..self.old_start + self.len + self.blanks_after
  }

  /// Its new lines with the blank lines that stand beside them now
  pub const fn new_lines(&self) -> Range<u32> {
    self.new_start - self.blanks_after..self.new_start + self.len + self.blanks_before
  }
}

/// The kind of an inner change of a block by its sides, as IntelliJ's `DiffUtil.getDiffType` types a fragment; its
/// number is the third value of each range of `DiffBlock`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeKind {
  /// The old side is empty
  Inserted = 0,
  /// The new side is empty
  Deleted = 1,
  Modified = 2,
}

/// The time of one `diff_texts`
#[derive(Clone, Copy, Debug)]
pub struct DiffBudget {
  /// The deadline of the comparison with inner ranges; `None` compares the lines only
  pub inner: Option<Deadline>,
  /// The deadline of the line comparison, when the inner one was skipped or ran out of time. When it runs out too,
  /// the diff is one block of both whole texts.
  pub lines: Deadline,
}

impl DiffBudget {
  /// No time limit
  pub const NONE: Self = Self {
    inner: Some(Deadline::NONE),
    lines: Deadline::NONE,
  };
}

/// Compares two texts under `policy`, by word inside the changed lines (`diff_texts_in`)
pub fn diff_texts(old: &str, new: &str, policy: WhitespacePolicy, budget: &DiffBudget) -> LineDiff {
  diff_texts_in(old, new, policy, budget).diff
}

/// A diff, and whether the time of its budget cut it short
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetedDiff {
  pub diff: LineDiff,
  /// A deadline of the budget ran out: the inner ranges (the diff is by lines alone) or the lines (one block of both
  /// whole texts). A side over the size limits or a comparison of too many changes is a limit of the work, the same on
  /// any machine, and is not this
  pub out_of_time: bool,
}

/// Compares two texts under `policy`, by word inside the changed lines, within `budget`; says whether its time ran out
pub fn diff_texts_in(old: &str, new: &str, policy: WhitespacePolicy, budget: &DiffBudget) -> BudgetedDiff {
  let (old_source, new_source) = (SourceText::new(old), SourceText::new(new));
  let (old_text, new_text) = (old_source.text(), new_source.text());
  let fits = |text: &str, source: &SourceText| text.len() <= MAX_INNER_BYTES && source.line_count() <= MAX_INNER_LINES;

  let inner = budget.inner.filter(|_| fits(old, &old_source) && fits(new, &new_source));
  let mut line_level_only = inner.is_none();
  let mut out_of_time = false;
  let mut compare = |highlight: HighlightPolicy, deadline: &Deadline| match compare_for_view(old_text, new_text, policy, highlight, deadline) {
    Ok(fragments) => Some(fragments),
    Err(error) => {
      out_of_time |= error == DiffTooBig::DeadlineExceeded;
      None
    }
  };
  let mut fragments = inner.and_then(|deadline| compare(HighlightPolicy::ByWord, &deadline));
  if fragments.is_none() {
    line_level_only = true;
    fragments = compare(HighlightPolicy::ByLine, &budget.lines);
  }
  drop(compare);
  let blocks = match fragments {
    Some(fragments) => fragments.iter().map(|fragment| block_of(fragment, &old_source, &new_source)).collect(),
    // out of time: the whole texts, unless they are equal
    None if old_text == new_text => Vec::new(),
    None => vec![DiffBlock {
      old_start: 0,
      old_end: to_u32(old_source.line_count()),
      new_start: 0,
      new_end: to_u32(new_source.line_count()),
      kind: BlockKind::Changed,
      inline_pairs: false,
      old_ranges: Vec::new(),
      new_ranges: Vec::new(),
    }],
  };

  let counted = |source: &SourceText, lines: Vec<Range<u32>>| -> u32 {
    let limit = to_u32(source.counted_lines());
    lines.into_iter().map(|range| range.end.min(limit).saturating_sub(range.start)).sum()
  };
  let changed = || blocks.iter().filter(|block| block.kind != BlockKind::Ignored);
  let diff = LineDiff {
    old_line_count: to_u32(old_source.line_count()),
    new_line_count: to_u32(new_source.line_count()),
    additions: counted(&new_source, changed().map(|block| block.new_start..block.new_end).collect()),
    deletions: counted(&old_source, changed().map(|block| block.old_start..block.old_end).collect()),
    line_ends: line_ends(&old_source, &new_source),
    final_newline: final_newline(&old_source, &new_source),
    empty_last_line: EmptyLastLine {
      old: has_empty_last_line(&old_source),
      new: has_empty_last_line(&new_source),
    },
    line_level_only,
    blocks,
  };
  BudgetedDiff { diff, out_of_time }
}

/// Where the reading of each block starts, from the quiet runs among `runs` (by `old_start`, each inside one block,
/// without overlap) and from `moves`: flat `[block, side, line]` for each block with something to read, in the order
/// of the blocks, the index of the block, the side of the line (0 the old text, 1 the new one) and the 0-based line
/// the reading starts at. The quiet runs of a block cut it into parts in the order of both sides; the reading starts
/// at the first part with a line to read: at its first new line, at its first old line that is in no move when it
/// has no new one. The old lines of a move, with the blank lines that left with it (`LineMove::old_lines`), are read
/// where they stand now, so they are nothing to read where they left; its new lines are in no quiet run and always
/// read. An `Ignored` block and a block without a line to read have no entry. The one place that reads
/// `LineRun::quiet` and the moves for the stops
pub fn starts_of(blocks: &[DiffBlock], runs: &[LineRun], moves: &[LineMove]) -> Vec<u32> {
  let mut starts = Vec::new();
  let mut quiet = runs.iter().filter(|run| run.quiet).peekable();
  // the old lines of the moves, in their order
  let mut left: Vec<Range<u32>> = moves.iter().map(LineMove::old_lines).collect();
  left.sort_unstable_by_key(|lines| lines.start);
  // the first of `lines` that no move took
  let first_kept = |lines: Range<u32>| {
    let mut line = lines.start;
    for moved in &left[left.partition_point(|moved| moved.end <= line)..] {
      if moved.start > line {
        break;
      }
      line = moved.end;
    }
    (line < lines.end).then_some(line)
  };
  for (index, block) in blocks.iter().enumerate() {
    let (mut old, mut new) = (block.old_start, block.new_start);
    let mut start = None;
    // the first part with a line to read between `old`, `new` and the next quiet run, or the end of the block
    let mut part = |old: u32, new: u32, old_end: u32, new_end: u32| {
      if start.is_none() {
        start = if new < new_end {
          Some((1, new))
        } else {
          first_kept(old..old_end).map(|line| (0, line))
        };
      }
    };
    while let Some(run) = quiet.next_if(|run| run.old_start < block.old_end || run.new_start < block.new_end) {
      part(old, new, run.old_start, run.new_start);
      (old, new) = (run.old_end.max(old), run.new_end.max(new));
    }
    part(old, new, block.old_end, block.new_end);
    if block.kind != BlockKind::Ignored
      && let Some((side, line)) = start
    {
      starts.extend([to_u32(index), side, line]);
    }
  }
  starts
}

fn final_newline(old: &SourceText, new: &SourceText) -> Option<FinalNewline> {
  let (old_end, new_end) = (old.ends_with_line_end(), new.ends_with_line_end());
  (old_end != new_end && !old.text().is_empty() && !new.text().is_empty()).then_some(FinalNewline { old: old_end, new: new_end })
}

/// The last line of the text is the empty one that does not count
pub fn has_empty_last_line(source: &SourceText) -> bool {
  source.counted_lines() < source.line_count()
}

fn line_ends(old: &SourceText, new: &SourceText) -> Option<LineEnds> {
  let (old_kind, new_kind) = (old.separator()?, new.separator()?);
  (old_kind != new_kind).then(|| LineEnds {
    old: old_kind,
    new: new_kind,
    only: old.text() == new.text(),
  })
}

fn block_of(view: &ViewFragment, old: &SourceText, new: &SourceText) -> DiffBlock {
  let fragment = &view.fragment;
  let mut block = DiffBlock {
    old_start: to_u32(fragment.start_line1),
    old_end: to_u32(fragment.end_line1),
    new_start: to_u32(fragment.start_line2),
    new_end: to_u32(fragment.end_line2),
    kind: BlockKind::Changed,
    inline_pairs: false,
    old_ranges: Vec::new(),
    new_ranges: Vec::new(),
  };
  if view.ignored {
    block.kind = BlockKind::Ignored;
    return block;
  }
  match &fragment.inner_fragments {
    // no similar parts, or not compared
    None => {}
    Some(inner) if inner.is_empty() => {
      block.kind = BlockKind::WhitespaceOnly;
      block.inline_pairs = pairs_align(old, new, fragment.start_line1..fragment.end_line1, fragment.start_line2..fragment.end_line2, &[], &[]);
    }
    Some(inner) => {
      let ranges1: Vec<Range<usize>> = inner
        .iter()
        .map(|it| fragment.start_offset1 + it.start_offset1..fragment.start_offset1 + it.end_offset1)
        .collect();
      let ranges2: Vec<Range<usize>> = inner
        .iter()
        .map(|it| fragment.start_offset2 + it.start_offset2..fragment.start_offset2 + it.end_offset2)
        .collect();
      let old_lines = line_ranges(old, fragment.start_line1..fragment.end_line1, &ranges1);
      let new_lines = line_ranges(new, fragment.start_line2..fragment.end_line2, &ranges2);
      if !ranges_dropped(&old_lines, &new_lines) {
        block.inline_pairs = inline_pairs(&old_lines, &new_lines)
          && pairs_align(
            old,
            new,
            fragment.start_line1..fragment.end_line1,
            fragment.start_line2..fragment.end_line2,
            &ranges1,
            &ranges2,
          );
        let kinds: Vec<RangeKind> = inner
          .iter()
          .map(|it| match (it.start_offset1 == it.end_offset1, it.start_offset2 == it.end_offset2) {
            (true, _) => RangeKind::Inserted,
            (_, true) => RangeKind::Deleted,
            _ => RangeKind::Modified,
          })
          .collect();
        block.old_ranges = typed_line_ranges(old, fragment.start_line1..fragment.end_line1, &ranges1, &kinds);
        block.new_ranges = typed_line_ranges(new, fragment.start_line2..fragment.end_line2, &ranges2, &kinds);
      }
    }
  }
  block
}

/// Whether line i of the old side and line i of the new side of a block are a pair that one row can show whole. The
/// inner changes are of the text of the whole block, not of a pair, so a change may take a line end with it or have
/// its sides in lines apart (a call wrapped again), and under `Ignore` a line end may move without a change. Such a
/// block has no pairs: a row of line i of both sides would lose the deleted text whose inserted side stands in
/// another row. The sides align when they have as many lines, each change lies within line i of both sides for one i
/// (an empty side in the first line that holds its offset, where `typed_line_ranges` puts its marker), and what a pair
/// holds between its changes is the same text on both sides but for whitespace.
fn pairs_align(old: &SourceText, new: &SourceText, old_lines: Range<usize>, new_lines: Range<usize>, old_ranges: &[Range<usize>], new_ranges: &[Range<usize>]) -> bool {
  if old_lines.len() != new_lines.len() {
    return false;
  }
  let (old_text, new_text) = (old.text(), new.text());
  let (old_offsets, new_offsets) = (old_text.line_offsets(), new_text.line_offsets());
  let same = |old_part: Range<usize>, new_part: Range<usize>| {
    let (old_part, new_part) = (&old_text.units()[old_part], &new_text.units()[new_part]);
    old_part == new_part || is_equal_texts(old_part, new_part, ComparisonPolicy::IgnoreWhitespaces)
  };
  let mut changes = old_ranges.iter().zip(new_ranges).peekable();
  for (old_line, new_line) in old_lines.zip(new_lines) {
    let (mut old_at, mut new_at) = (old_offsets.line_start(old_line), new_offsets.line_start(new_line));
    let (old_end, new_end) = (old_offsets.line_end(old_line), new_offsets.line_end(new_line));
    // the changes of this pair: the ones whose old side starts in its old line
    while let Some((old_range, new_range)) = changes.next_if(|(old_range, _)| old_range.start <= old_end) {
      let inside = old_range.start >= old_at && old_range.end <= old_end && new_range.start >= new_at && new_range.end <= new_end;
      if !inside || !same(old_at..old_range.start, new_at..new_range.start) {
        return false;
      }
      (old_at, new_at) = (old_range.end, new_range.end);
    }
    if !same(old_at..old_end, new_at..new_end) {
      return false;
    }
  }
  changes.next().is_none()
}

/// The ranges inside each line of `lines`, from ranges of text offsets; a range is cut at the line ends
pub fn line_ranges(source: &SourceText, lines: Range<usize>, ranges: &[Range<usize>]) -> Vec<LineRanges> {
  let offsets = source.text().line_offsets();
  lines
    .map(|line| {
      let (line_start, line_end) = (offsets.line_start(line), offsets.line_end(line));
      let ranges = ranges
        .iter()
        .filter_map(|range| {
          let (start, end) = (range.start.max(line_start), range.end.min(line_end));
          (start < end).then(|| (start - line_start, end - line_start))
        })
        .collect();
      LineRanges {
        length: line_end - line_start,
        ranges,
      }
    })
    .collect()
}

/// The typed ranges of each line of `lines`, `[start, end, kind, ...]`: a range is cut at the line ends; an empty range
/// (the empty side of an inserted or a deleted change) is a marker in the first line that holds its offset
fn typed_line_ranges(source: &SourceText, lines: Range<usize>, ranges: &[Range<usize>], kinds: &[RangeKind]) -> Vec<Vec<u32>> {
  let offsets = source.text().line_offsets();
  let mut placed = vec![false; ranges.len()];
  lines
    .map(|line| {
      let (line_start, line_end) = (offsets.line_start(line), offsets.line_end(line));
      let mut typed = Vec::new();
      for (index, (range, kind)) in ranges.iter().zip(kinds).enumerate() {
        let (start, end) = if range.is_empty() {
          if placed[index] || range.start < line_start || range.start > line_end {
            continue;
          }
          placed[index] = true;
          (range.start, range.start)
        } else {
          let (start, end) = (range.start.max(line_start), range.end.min(line_end));
          if start >= end {
            continue;
          }
          (start, end)
        };
        typed.extend([to_u32(start - line_start), to_u32(end - line_start), *kind as u32]);
      }
      typed
    })
    .collect()
}

pub fn flat_ranges(line: &LineRanges) -> Vec<u32> {
  line.ranges.iter().flat_map(|&(start, end)| [to_u32(start), to_u32(end)]).collect()
}

/// A line index or an offset; the texts are far below 4 GiB
pub fn to_u32(value: usize) -> u32 {
  u32::try_from(value).expect("a text below 4 GiB")
}
