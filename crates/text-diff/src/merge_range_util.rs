// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/util/MergeRangeUtil.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2024 JetBrains s.r.o. and contributors). Three texts with their line
// offsets (`Utf16Text`) replace the lists of sequences and of line offsets.

//! The type of a block of a three-way merge, from the contents of its three sides.

use crate::comparison_merge_util::try_resolve_conflict;
use crate::comparison_util::is_equal_texts;
use crate::merge_conflict_type::{MergeConflictKind, MergeConflictType, ResolutionStrategy};
use crate::merge_range::{MergeRange, ThreeSide};
use crate::policy::ComparisonPolicy;
use crate::text::Utf16Text;

/// The type of a block from which sides are empty, which sides are equal under the policy (`equality`) and, for a
/// block that differs only in what the policy ignores, under the default policy (`true_equality`). A conflict of two
/// changed sides is resolvable when `conflict_resolver` says so.
pub fn get_merge_type(
  emptiness: impl Fn(ThreeSide) -> bool,
  equality: impl Fn(ThreeSide, ThreeSide) -> bool,
  true_equality: Option<&dyn Fn(ThreeSide, ThreeSide) -> bool>,
  conflict_resolver: impl FnOnce() -> bool,
) -> MergeConflictType {
  use MergeConflictKind::{Conflict, Deleted, Inserted, Modified};

  let is_left_empty = emptiness(ThreeSide::Left);
  let is_base_empty = emptiness(ThreeSide::Base);
  let is_right_empty = emptiness(ThreeSide::Right);
  assert!(!is_left_empty || !is_base_empty || !is_right_empty);

  if is_base_empty {
    if is_left_empty {
      // --=
      return MergeConflictType::new(Inserted, false, true);
    }
    if is_right_empty {
      // =--
      return MergeConflictType::new(Inserted, true, false);
    }
    // =-=
    return if equality(ThreeSide::Left, ThreeSide::Right) {
      MergeConflictType::new(Inserted, true, true)
    } else {
      MergeConflictType::with_resolution(Conflict, true, true, None)
    };
  }

  if is_left_empty && is_right_empty {
    // -=-
    return MergeConflictType::new(Deleted, true, true);
  }

  // -==, ==-, ===
  let unchanged_left = equality(ThreeSide::Base, ThreeSide::Left);
  let unchanged_right = equality(ThreeSide::Base, ThreeSide::Right);

  if unchanged_left && unchanged_right {
    let true_equality = true_equality.expect("a true equality for a block that the policy ignores");
    let true_unchanged_left = true_equality(ThreeSide::Base, ThreeSide::Left);
    let true_unchanged_right = true_equality(ThreeSide::Base, ThreeSide::Right);
    assert!(!true_unchanged_left || !true_unchanged_right);
    return MergeConflictType::new(Modified, !true_unchanged_left, !true_unchanged_right);
  }

  if unchanged_left {
    return MergeConflictType::new(if is_right_empty { Deleted } else { Modified }, false, true);
  }
  if unchanged_right {
    return MergeConflictType::new(if is_left_empty { Deleted } else { Modified }, true, false);
  }

  if equality(ThreeSide::Left, ThreeSide::Right) {
    MergeConflictType::new(Modified, true, true)
  } else {
    let can_be_resolved = !is_left_empty && !is_right_empty && conflict_resolver();
    MergeConflictType::with_resolution(Conflict, true, true, can_be_resolved.then_some(ResolutionStrategy::Text))
  }
}

/// The type of a block of lines of a three-way comparison (`compare_lines_three`): a block that differs only in what
/// the policy ignores is not expected
pub fn get_line_three_way_diff_type(fragment: &MergeRange, texts: [&Utf16Text; 3], policy: ComparisonPolicy) -> MergeConflictType {
  get_merge_type(
    |side| is_line_merge_interval_empty(fragment, side),
    |side1, side2| compare_line_merge_contents(fragment, texts, policy, side1, side2),
    None,
    || can_resolve_line_conflict(fragment, texts),
  )
}

/// The type of a block of lines of a merge (`merge_lines`): a block that differs only in what the policy ignores is a
/// modification of the sides that differ from the base under the default policy
pub fn get_line_merge_type(fragment: &MergeRange, texts: [&Utf16Text; 3], policy: ComparisonPolicy) -> MergeConflictType {
  get_merge_type(
    |side| is_line_merge_interval_empty(fragment, side),
    |side1, side2| compare_line_merge_contents(fragment, texts, policy, side1, side2),
    Some(&|side1, side2| compare_line_merge_contents(fragment, texts, ComparisonPolicy::Default, side1, side2)),
    || can_resolve_line_conflict(fragment, texts),
  )
}

fn can_resolve_line_conflict(fragment: &MergeRange, texts: [&Utf16Text; 3]) -> bool {
  let [left, base, right] = ThreeSide::ALL.map(|side| lines_content(texts[side.index()], fragment.start(side), fragment.end(side)));
  try_resolve_conflict(left, base, right).is_some()
}

/// Whether two sides of a block of lines are equal under the policy, line by line
pub fn compare_line_merge_contents(fragment: &MergeRange, texts: [&Utf16Text; 3], policy: ComparisonPolicy, side1: ThreeSide, side2: ThreeSide) -> bool {
  let (start1, end1) = (fragment.start(side1), fragment.end(side1));
  let (start2, end2) = (fragment.start(side2), fragment.end(side2));
  if end2 - start2 != end1 - start1 {
    return false;
  }

  let (text1, text2) = (texts[side1.index()], texts[side2.index()]);
  (0..end1 - start1).all(|i| {
    let (line1, line2) = (start1 + i, start2 + i);
    is_equal_texts(lines_content(text1, line1, line1 + 1), lines_content(text2, line2, line2 + 1), policy)
  })
}

const fn is_line_merge_interval_empty(fragment: &MergeRange, side: ThreeSide) -> bool {
  fragment.start(side) == fragment.end(side)
}

/// The type of a block of words of a three-way comparison (`by_word::compare_three`), in offsets; a word conflict is
/// never resolvable
pub fn get_word_merge_type(fragment: &MergeRange, texts: [&[u16]; 3], policy: ComparisonPolicy) -> MergeConflictType {
  get_merge_type(
    |side| fragment.start(side) == fragment.end(side),
    |side1, side2| compare_word_merge_contents(fragment, texts, policy, side1, side2),
    None,
    || false,
  )
}

/// Whether two sides of a block of words are equal under the policy
pub fn compare_word_merge_contents(fragment: &MergeRange, texts: [&[u16]; 3], policy: ComparisonPolicy, side1: ThreeSide, side2: ThreeSide) -> bool {
  let content1 = &texts[side1.index()][fragment.start(side1)..fragment.end(side1)];
  let content2 = &texts[side2.index()][fragment.start(side2)..fragment.end(side2)];
  is_equal_texts(content1, content2, policy)
}

// jscpd:ignore-start: `getLeftToRightDiffType` of the source repeats the start of `getMergeType`, and the port keeps both
/// The type of a block of lines where the changes go from left to right through the base, as the "compare with the
/// base" view shows them: an insertion on the left is a deletion from left to right
pub fn get_line_left_to_right_three_side_diff_type(fragment: &MergeRange, texts: [&Utf16Text; 3], policy: ComparisonPolicy) -> MergeConflictType {
  use MergeConflictKind::{Deleted, Inserted, Modified};

  let emptiness = |side| is_line_merge_interval_empty(fragment, side);
  let equality = |side1, side2| compare_line_merge_contents(fragment, texts, policy, side1, side2);

  let is_left_empty = emptiness(ThreeSide::Left);
  let is_base_empty = emptiness(ThreeSide::Base);
  let is_right_empty = emptiness(ThreeSide::Right);
  assert!(!is_left_empty || !is_base_empty || !is_right_empty);

  if is_base_empty {
    if is_left_empty {
      // --=
      return MergeConflictType::new(Inserted, false, true);
    }
    if is_right_empty {
      // =--
      return MergeConflictType::new(Deleted, true, false);
    }
    // =-=
    return MergeConflictType::new(Modified, true, true);
  }

  if is_left_empty && is_right_empty {
    // -=-
    return MergeConflictType::new(Modified, true, true);
  }

  // -==, ==-, ===
  let unchanged_left = equality(ThreeSide::Base, ThreeSide::Left);
  let unchanged_right = equality(ThreeSide::Base, ThreeSide::Right);
  assert!(!unchanged_left || !unchanged_right);

  if unchanged_left {
    return MergeConflictType::new(if is_right_empty { Deleted } else { Modified }, false, true);
  }
  if unchanged_right {
    return MergeConflictType::new(if is_left_empty { Inserted } else { Modified }, true, false);
  }
  MergeConflictType::new(Modified, true, true)
}
// jscpd:ignore-end

/// The text of lines `[line1, line2)`, without the last `\n` (`DiffRangeUtil.getLinesContent`)
pub(crate) fn lines_content(text: &Utf16Text, line1: usize, line2: usize) -> &[u16] {
  let (start, end) = text.line_offsets().lines_range(line1, line2, false);
  &text.units()[start..end]
}
