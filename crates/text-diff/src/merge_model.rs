//! The merge of three texts for the views of the app: the blocks of the merge with their kind, the sides that changed
//! them, their resolution and the changed ranges inside their lines, built straight from the ported merge. Branch
//! Deck's own: the wire model of a conflict, as `model.rs` is the one of a two-way diff.
//!
//! The sides are IntelliJ's: left is ours (the target as built), base is the merge base (stage 1), right is theirs (the
//! commit being picked). Lines are IntelliJ's, as in `model.rs`.

use crate::by_word;
use crate::cancellation::Deadline;
use crate::comparison_util::is_equal_texts;
use crate::manager::merge_lines;
use crate::merge_conflict_type::{MergeConflictKind, MergeConflictType, ResolutionStrategy};
use crate::merge_range::{MergeRange, ThreeSide};
use crate::merge_range_util::{compare_line_merge_contents, get_merge_type, lines_content};
use crate::merge_resolve_util::try_resolve;
use crate::model::{DiffBudget, MAX_INNER_BYTES, MAX_INNER_LINES, Scope, flat_ranges, has_empty_last_line, line_ranges, to_u32};
use crate::policy::{ComparisonPolicy, WhitespacePolicy};
use crate::text::{LineEndKind, SourceText, Utf16Text};
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// The merge of three texts
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TextMerge {
  pub left_line_count: u32,
  pub base_line_count: u32,
  pub right_line_count: u32,
  /// The line ends of the sides (`None` for a side without one) when they differ; `None` when the sides with a line
  /// end agree
  pub line_ends: Option<Sides<Option<LineEndKind>>>,
  /// Whether each text ends with a line end, when they disagree; `None` when they agree or a text is empty
  pub final_newline: Option<Sides<bool>>,
  /// Whether the last line of each text is the empty line that does not count (`TextDiff::empty_last_line`)
  pub empty_last_line: Sides<bool>,
  /// The inner ranges and the resolve by words were not computed (a side over a limit, or out of time): the blocks are
  /// still exact, and no conflict resolves by words
  pub line_level_only: bool,
  /// In order and without overlap; the lines between two blocks are equal on the three sides under the policy
  pub blocks: Vec<MergeBlock>,
  /// The blocks that both sides changed differently
  pub conflicts: u32,
  /// The conflicts that no resolution resolves; `conflicts > 0` with `unresolved == 0` is a conflict of git that its
  /// words resolve
  pub unresolved: u32,
  pub left_scopes: Vec<Scope>,
  pub base_scopes: Vec<Scope>,
  pub right_scopes: Vec<Scope>,
}

/// A block of a merge
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MergeBlock {
  /// 0-based line indices of each side, the end exclusive
  pub left_start: u32,
  pub left_end: u32,
  pub base_start: u32,
  pub base_end: u32,
  pub right_start: u32,
  pub right_end: u32,
  pub kind: MergeBlockKind,
  pub left_changed: bool,
  pub right_changed: bool,
  /// Under `Ignore` (and `IgnoreBlankLines`, whose comparison it is), the three sides are equal ignoring whitespace,
  /// a side that did not change as empty: a blank line added or removed. Drawn pale without word ranges, as IntelliJ
  /// paints it, and still a change that a side applies and the navigation stops at; IntelliJ's merge drops no block
  pub ignored: bool,
  /// How the block resolves: `Default` takes the side that changed, `Text` takes `resolved_text`; `None` for a conflict
  /// that the user resolves
  pub resolution: Option<MergeResolution>,
  /// The text of the block resolved by its words, without the end of its last line; only for the `Text` resolution
  pub resolved_text: Option<String>,
  /// For each line of a side, `[start, end, ...]`: UTF-16 offsets inside the line, never the line end. Empty for a
  /// block without inner ranges: inserted, deleted, ignored, or `line_level_only`
  pub left_ranges: Vec<Vec<u32>>,
  pub base_ranges: Vec<Vec<u32>>,
  pub right_ranges: Vec<Vec<u32>>,
}

/// The kind of a merge block (`MergeConflictType.Type`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum MergeBlockKind {
  Inserted,
  Deleted,
  Modified,
  Conflict,
}

/// How a merge block resolves (`MergeConflictResolutionStrategy` without the language-specific one)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum MergeResolution {
  /// Take the side that changed
  Default,
  /// The changes of the two sides do not overlap at the word level
  Text,
}

/// A value of each of the three sides of a merge
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Sides<T> {
  pub left: T,
  pub base: T,
  pub right: T,
}

/// Merges three texts under `policy` by line, with the words inside the blocks and the resolve of the conflicts by
/// words; `budget.inner` bounds the words and the resolves of the whole file, `budget.lines` the merge of the lines
pub fn merge_texts(left: &str, base: &str, right: &str, policy: WhitespacePolicy, budget: &DiffBudget) -> TextMerge {
  let sources = [SourceText::new(left), SourceText::new(base), SourceText::new(right)];
  let texts = [sources[0].text(), sources[1].text(), sources[2].text()];
  let comparison_policy = policy.comparison_policy();
  let fits = [left, base, right]
    .iter()
    .zip(&sources)
    .all(|(text, source)| text.len() <= MAX_INNER_BYTES && source.line_count() <= MAX_INNER_LINES);
  let mut inner = budget.inner.filter(|_| fits);

  let ranges = merge_lines(texts[0], texts[1], texts[2], comparison_policy, &budget.lines).unwrap_or_else(|_| {
    inner = None;
    whole_texts(&sources)
  });

  let mut blocks = Vec::with_capacity(ranges.len());
  for range in &ranges {
    let (block, completed) = block_of(range, texts, &sources, comparison_policy, inner.as_ref());
    if !completed {
      inner = None;
    }
    blocks.push(block);
  }

  let conflicts = blocks.iter().filter(|block| block.kind == MergeBlockKind::Conflict).count();
  let unresolved = blocks.iter().filter(|block| block.kind == MergeBlockKind::Conflict && block.resolution.is_none()).count();
  TextMerge {
    left_line_count: to_u32(sources[0].line_count()),
    base_line_count: to_u32(sources[1].line_count()),
    right_line_count: to_u32(sources[2].line_count()),
    line_ends: line_ends(&sources),
    final_newline: final_newline(&sources),
    empty_last_line: Sides {
      left: has_empty_last_line(&sources[0]),
      base: has_empty_last_line(&sources[1]),
      right: has_empty_last_line(&sources[2]),
    },
    line_level_only: inner.is_none(),
    blocks,
    conflicts: to_u32(conflicts),
    unresolved: to_u32(unresolved),
    left_scopes: Vec::new(),
    base_scopes: Vec::new(),
    right_scopes: Vec::new(),
  }
}

/// One block of the whole texts, when the merge of the lines runs out of time
fn whole_texts(sources: &[SourceText; 3]) -> Vec<MergeRange> {
  let range = MergeRange::new(0, sources[0].line_count(), 0, sources[1].line_count(), 0, sources[2].line_count());
  if sources.iter().all(|source| source.text() == sources[1].text()) {
    Vec::new()
  } else {
    vec![range]
  }
}

/// The block of a merge range, and whether its words were compared within the deadline
fn block_of(range: &MergeRange, texts: [&Utf16Text; 3], sources: &[SourceText; 3], policy: ComparisonPolicy, inner: Option<&Deadline>) -> (MergeBlock, bool) {
  let mut resolved: Option<Vec<u16>> = None;
  let mut completed = true;
  let conflict_type: MergeConflictType = get_merge_type(
    |side| range.start(side) == range.end(side),
    |side1, side2| compare_line_merge_contents(range, texts, policy, side1, side2),
    Some(&|side1, side2| compare_line_merge_contents(range, texts, ComparisonPolicy::Default, side1, side2)),
    || {
      let Some(deadline) = inner else {
        return false;
      };
      let [left, base, right] = ThreeSide::ALL.map(|side| lines_content(texts[side.index()], range.start(side), range.end(side)));
      resolved = try_resolve(left, base, right, deadline);
      completed = deadline.check_canceled().is_ok();
      resolved.is_some()
    },
  );
  // IntelliJ (`DiffUtil.compareThreesideInner`): under `Ignore`, a block whose changed sides are equal ignoring
  // whitespace, an unchanged or an empty side as "", has empty inner lists and is painted pale without words
  let chunk = |side: ThreeSide| -> &[u16] {
    if conflict_type.is_change_of(side) {
      lines_content(texts[side.index()], range.start(side), range.end(side))
    } else {
      &[]
    }
  };
  let ignored = policy == ComparisonPolicy::IgnoreWhitespaces
    && is_equal_texts(chunk(ThreeSide::Left), chunk(ThreeSide::Base), policy)
    && is_equal_texts(chunk(ThreeSide::Left), chunk(ThreeSide::Right), policy);
  let kind = match conflict_type.kind {
    MergeConflictKind::Inserted => MergeBlockKind::Inserted,
    MergeConflictKind::Deleted => MergeBlockKind::Deleted,
    MergeConflictKind::Modified => MergeBlockKind::Modified,
    MergeConflictKind::Conflict => MergeBlockKind::Conflict,
  };
  let resolution = match conflict_type.resolution {
    Some(ResolutionStrategy::Text) => Some(MergeResolution::Text),
    Some(ResolutionStrategy::Default | ResolutionStrategy::Semantic) => Some(MergeResolution::Default),
    None => None,
  };

  let mut block = MergeBlock {
    left_start: to_u32(range.start1),
    left_end: to_u32(range.end1),
    base_start: to_u32(range.start2),
    base_end: to_u32(range.end2),
    right_start: to_u32(range.start3),
    right_end: to_u32(range.end3),
    kind,
    left_changed: conflict_type.is_change_of(ThreeSide::Left),
    right_changed: conflict_type.is_change_of(ThreeSide::Right),
    ignored,
    resolution,
    resolved_text: resolved.filter(|_| resolution == Some(MergeResolution::Text)).map(|text| String::from_utf16_lossy(&text)),
    left_ranges: Vec::new(),
    base_ranges: Vec::new(),
    right_ranges: Vec::new(),
  };
  if let Some(deadline) = inner.filter(|_| completed && !ignored && matches!(kind, MergeBlockKind::Modified | MergeBlockKind::Conflict)) {
    completed = add_inner_ranges(&mut block, range, texts, sources, policy, deadline);
  }
  (block, completed)
}

/// The changed words of each line of the sides of a block; false when the deadline ran out
fn add_inner_ranges(block: &mut MergeBlock, range: &MergeRange, texts: [&Utf16Text; 3], sources: &[SourceText; 3], policy: ComparisonPolicy, deadline: &Deadline) -> bool {
  let chunk_starts = ThreeSide::ALL.map(|side| texts[side.index()].line_offsets().lines_range(range.start(side), range.end(side), false).0);
  let [left, base, right] = ThreeSide::ALL.map(|side| lines_content(texts[side.index()], range.start(side), range.end(side)));
  let Ok(words) = by_word::compare_three(left, base, right, policy, deadline) else {
    return false;
  };
  let side_ranges = |side: ThreeSide| -> Vec<Vec<u32>> {
    let start = chunk_starts[side.index()];
    let offsets: Vec<Range<usize>> = words.iter().map(|word| start + word.start(side)..start + word.end(side)).collect();
    line_ranges(&sources[side.index()], range.start(side)..range.end(side), &offsets)
      .iter()
      .map(flat_ranges)
      .collect()
  };
  block.left_ranges = side_ranges(ThreeSide::Left);
  block.base_ranges = side_ranges(ThreeSide::Base);
  block.right_ranges = side_ranges(ThreeSide::Right);
  true
}

fn line_ends(sources: &[SourceText; 3]) -> Option<Sides<Option<LineEndKind>>> {
  let [left, base, right] = sources.each_ref().map(SourceText::separator);
  let mut known = [left, base, right].into_iter().flatten();
  let first = known.next()?;
  known.any(|kind| kind != first).then_some(Sides { left, base, right })
}

fn final_newline(sources: &[SourceText; 3]) -> Option<Sides<bool>> {
  if sources.iter().any(|source| source.text().is_empty()) {
    return None;
  }
  let [left, base, right] = sources.each_ref().map(SourceText::ends_with_line_end);
  (left != base || base != right).then_some(Sides { left, base, right })
}
