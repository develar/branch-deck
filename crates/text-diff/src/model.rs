//! The diff of two texts for the views of the app: the changed blocks of lines with their kind and the changed ranges
//! inside their lines, built straight from the comparison. Branch Deck's own: the wire model that the backend sends as
//! JSON. The views take the texts by blob id and build the context, the hunks and the expansion from the blocks.
//!
//! Lines are IntelliJ's (`SourceText`): `\n`, `\r\n` and a lone `\r` end a line, and a text has one line more than it
//! has line ends. Line ends compare as equal; `TextDiff::line_ends` names them when the files differ in them.

use crate::cancellation::Deadline;
use crate::inline::{LineRanges, inline_pairs, ranges_dropped};
use crate::policy::{HighlightPolicy, WhitespacePolicy};
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TextDiff {
  pub old_line_count: u32,
  pub new_line_count: u32,
  /// The changed lines of the new text under the policy: an `Ignored` block, and the empty line after a final line
  /// end, do not count
  pub additions: u32,
  /// The same for the old text
  pub deletions: u32,
  /// The line ends of the two files when they differ; `None` when they are the same, or a file has no line end
  pub line_ends: Option<LineEnds>,
  /// Whether each text ends with a line end, when only one does ("Newline at end of file added"); `None` when they agree,
  /// or a text is empty (an added or a deleted file)
  pub final_newline: Option<FinalNewline>,
  /// Whether the last line of each text is the empty line that does not count: the line after a final line end, or
  /// the one line of an empty text. A view hides it when no block touches it, so a file shows no empty last row
  pub empty_last_line: EmptyLastLine,
  /// The inner ranges were not compared (a side over a limit, or out of time): the blocks are still exact
  pub line_level_only: bool,
  /// In order and without overlap; the lines between two blocks are equal under the policy
  pub blocks: Vec<DiffBlock>,
  /// The declarations of the old text, for the labels of the hunks; empty when not known
  pub old_scopes: Vec<Scope>,
  pub new_scopes: Vec<Scope>,
}

/// A block of changed lines
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffBlock {
  /// 0-based line indices, the end exclusive; `start == end` is a side without lines
  pub old_start: u32,
  pub old_end: u32,
  pub new_start: u32,
  pub new_end: u32,
  pub kind: BlockKind,
  /// The sides have the same number of lines and each pair is similar: the Inline view may draw pair i as one row
  pub inline_pairs: bool,
  /// For each line of the old side, `[start, end, kind, start, end, kind, ...]`: UTF-16 offsets inside the line, the
  /// end exclusive, never the line end, and the `RangeKind` of the inner change, as IntelliJ types a fragment.
  /// `start == end` marks the empty side of an inserted or a deleted change, which a view draws as a thin bar. Empty
  /// when the block has no inner ranges: `Ignored`, `WhitespaceOnly`, ranges dropped, or `line_level_only`.
  pub old_ranges: Vec<Vec<u32>>,
  pub new_ranges: Vec<Vec<u32>>,
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

/// How a view draws a block
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LineEnds {
  pub old: LineEndKind,
  pub new: LineEndKind,
  /// The files differ in their line ends only
  pub only: bool,
}

/// Whether each text ends with a line end
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FinalNewline {
  pub old: bool,
  pub new: bool,
}

/// Whether the last line of each text is empty and does not count
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EmptyLastLine {
  pub old: bool,
  pub new: bool,
}

/// A declaration of a text: its lines and its label, `Outer › inner`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Scope {
  /// 0-based, the end exclusive
  pub start_line: u32,
  pub end_line: u32,
  pub label: String,
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

/// Compares two texts under `policy`, by word inside the changed lines
pub fn diff_texts(old: &str, new: &str, policy: WhitespacePolicy, budget: &DiffBudget) -> TextDiff {
  let (old_source, new_source) = (SourceText::new(old), SourceText::new(new));
  let (old_text, new_text) = (old_source.text(), new_source.text());
  let fits = |text: &str, source: &SourceText| text.len() <= MAX_INNER_BYTES && source.line_count() <= MAX_INNER_LINES;

  let inner = budget.inner.filter(|_| fits(old, &old_source) && fits(new, &new_source));
  let mut line_level_only = inner.is_none();
  let mut fragments = inner.and_then(|deadline| compare_for_view(old_text, new_text, policy, HighlightPolicy::ByWord, &deadline).ok());
  if fragments.is_none() {
    line_level_only = true;
    fragments = compare_for_view(old_text, new_text, policy, HighlightPolicy::ByLine, &budget.lines).ok();
  }
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
  TextDiff {
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
    old_scopes: Vec::new(),
    new_scopes: Vec::new(),
  }
}

fn final_newline(old: &SourceText, new: &SourceText) -> Option<FinalNewline> {
  let (old_end, new_end) = (old.ends_with_line_end(), new.ends_with_line_end());
  (old_end != new_end && !old.text().is_empty() && !new.text().is_empty()).then_some(FinalNewline { old: old_end, new: new_end })
}

/// The last line of the text is the empty one that does not count
pub(crate) fn has_empty_last_line(source: &SourceText) -> bool {
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
      block.inline_pairs = fragment.end_line1 - fragment.start_line1 == fragment.end_line2 - fragment.start_line2;
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
        block.inline_pairs = inline_pairs(&old_lines, &new_lines);
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

/// The ranges inside each line of `lines`, from ranges of text offsets; a range is cut at the line ends
pub(crate) fn line_ranges(source: &SourceText, lines: Range<usize>, ranges: &[Range<usize>]) -> Vec<LineRanges> {
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

pub(crate) fn flat_ranges(line: &LineRanges) -> Vec<u32> {
  line.ranges.iter().flat_map(|&(start, end)| [to_u32(start), to_u32(end)]).collect()
}

/// A line index or an offset; the texts are far below 4 GiB
pub(crate) fn to_u32(value: usize) -> u32 {
  u32::try_from(value).expect("a text below 4 GiB")
}
