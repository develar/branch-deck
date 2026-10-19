// Ported and modified from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/ComparisonUtilAutoTest.kt`
// and `doAutoTest`, `generateText` and the char sequence assertions of `DiffTestCase.kt` of IntelliJ Platform (Apache
// License 2.0, Copyright 2000-2015 JetBrains s.r.o. and contributors), as property tests: proptest generates the
// texts with a fixed seed instead of the current time, and shrinks a failure to a minimal pair of texts.
// The explicit blocks come from `compare_lines` under the default policy instead of `createRanges` of the VCS line
// status. The three-way tests compare three generated texts: the source passes the base as the right text too
// (`test(text1, text2, text2, …)`). They also take the type of each block (`MergeRangeUtil`), which the source does
// not check, so that its assertions run on random blocks.

use crate::by_word;
use crate::cancellation::Deadline;
use crate::character_utils::{char_count, code_point_at};
use crate::comparison_util::{equals_ignore_whitespaces, unimportant_line_char_count};
use crate::comparison_util_test_base::check_line_offsets;
use crate::fragments::{DiffFragment, LineFragment};
use crate::manager::{
  compare_chars, compare_lines, compare_lines_inner_with, compare_lines_three, compare_lines_word_first, compare_words, is_equal, merge_lines, process_blocks, squash,
};
use crate::merge_range::{MergeRange, ThreeSide};
use crate::merge_range_util::{get_line_merge_type, get_line_three_way_diff_type};
use crate::policy::{ComparisonPolicy, HighlightPolicy, InnerFragmentsPolicy, WhitespacePolicy};
use crate::provider::compare_line_ranges;
use crate::range::Range;
use crate::test_support::{CH_FACE, CH_GUN, CH_MAN, CH_SMILE, get_line_count, insert_range, proptest_config};
use crate::text::Utf16Text;
use crate::trim_util::is_space_enter_or_tab;
use fixedbitset::FixedBitSet;
use proptest::collection::vec;
use proptest::prelude::{Strategy, any};
use proptest::proptest;
use proptest::sample::select;
use std::fmt::Debug;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// 30 runs of the source, each with and without high surrogates
const CASES: u32 = 60;
const MAX_LENGTH: usize = 300;

const COMPARISON_POLICIES: [ComparisonPolicy; 3] = [ComparisonPolicy::Default, ComparisonPolicy::TrimWhitespaces, ComparisonPolicy::IgnoreWhitespaces];

// jscpd:ignore-start: the tests repeat the structure of the Kotlin tests, the oracle of the port

proptest! {
  #![proptest_config(proptest_config(CASES))]

  #[test]
  fn char((text1, text2) in texts()) {
    for_each_policy(&text1, &text2, |text1, text2, policy| {
      let fragments = compare_chars(text1.units(), text2.units(), policy, &Deadline::NONE).expect("no limits");
      // `checkResultChar` of the source checks the same as `checkResultWord`
      check_result_word(text1.units(), text2.units(), &fragments, policy);
    });
  }

  #[test]
  fn word((text1, text2) in texts()) {
    for_each_policy(&text1, &text2, |text1, text2, policy| {
      let fragments = compare_words(text1.units(), text2.units(), policy, &Deadline::NONE).expect("no limits");
      check_result_word(text1.units(), text2.units(), &fragments, policy);
    });
  }

  #[test]
  fn line((text1, text2) in texts()) {
    for_each_inner_policy(&text1, &text2, &[InnerFragmentsPolicy::Words, InnerFragmentsPolicy::Chars], |text1, text2, policy, fragments_policy| {
      let fragments = compare_lines_inner_with(text1, text2, policy, fragments_policy, &Deadline::NONE).expect("no limits");
      check_result_line(text1, text2, &fragments, policy, true, false);
    });
  }

  #[test]
  fn line_squashed((text1, text2) in texts()) {
    for_each_inner_policy(&text1, &text2, &[InnerFragmentsPolicy::Words, InnerFragmentsPolicy::Chars], |text1, text2, policy, fragments_policy| {
      let fragments = compare_lines_inner_with(text1, text2, policy, fragments_policy, &Deadline::NONE).expect("no limits");
      let squashed_fragments = squash(fragments);
      check_result_line(text1, text2, &squashed_fragments, policy, false, false);
    });
  }

  #[test]
  fn line_trim_squashed((text1, text2) in texts()) {
    for_each_inner_policy(&text1, &text2, &[InnerFragmentsPolicy::Words, InnerFragmentsPolicy::Chars], |text1, text2, policy, fragments_policy| {
      let fragments = compare_lines_inner_with(text1, text2, policy, fragments_policy, &Deadline::NONE).expect("no limits");
      let processed = process_blocks(fragments, text1.units(), text2.units(), policy, true, true);
      check_result_line(text1, text2, &processed, policy, false, false);
    });
  }

  #[test]
  fn words_first((text1, text2) in texts()) {
    for_each_policy(&text1, &text2, |text1, text2, policy| {
      let fragments = compare_lines_word_first(text1, text2, policy, &Deadline::NONE).expect("no limits");
      let processed = process_blocks(fragments, text1.units(), text2.units(), policy, true, true);
      // we do not consider 'x x' and 'xx' identical lines
      check_result_line(text1, text2, &processed, policy, true, policy == ComparisonPolicy::IgnoreWhitespaces);
    });
  }

  #[test]
  fn explicit_blocks((text1, text2) in texts()) {
    let (text1, text2) = (Utf16Text::new(&text1), Utf16Text::new(&text2));
    let ranges: Vec<_> = compare_lines(&text1, &text2, ComparisonPolicy::Default, &Deadline::NONE)
      .expect("no limits")
      .iter()
      .map(LineFragment::lines)
      .collect();
    for highlight_policy in [HighlightPolicy::ByLine, HighlightPolicy::ByWord, HighlightPolicy::ByWordSplit] {
      for ignore_policy in [WhitespacePolicy::Default, WhitespacePolicy::Trim, WhitespacePolicy::Ignore, WhitespacePolicy::IgnoreBlankLines] {
        in_context(&(highlight_policy, ignore_policy, &ranges), || {
          let fragments: Vec<LineFragment> = compare_line_ranges(&text1, &text2, &ranges, ignore_policy, highlight_policy, &Deadline::NONE)
            .expect("no limits")
            .into_iter()
            .flatten()
            .collect();
          check_result_line(&text1, &text2, &fragments, ignore_policy.comparison_policy(), !highlight_policy.should_squash(), false);
        });
      }
    }
  }
}

proptest! {
  #![proptest_config(proptest_config(CASES))]

  #[test]
  fn three_way_diff((text1, text2, text3) in three_texts()) {
    for_each_policy_three(&text1, &text2, &text3, |texts, policy| {
      let fragments = compare_lines_three(texts[0], texts[1], texts[2], policy, &Deadline::NONE).expect("no limits");
      for fragment in &fragments {
        get_line_three_way_diff_type(fragment, texts, policy);
      }
      check_result_merge(texts, &fragments, policy, false);
    });
  }

  #[test]
  fn merge((text1, text2, text3) in three_texts()) {
    for_each_policy_three(&text1, &text2, &text3, |texts, policy| {
      let fragments = merge_lines(texts[0], texts[1], texts[2], policy, &Deadline::NONE).expect("no limits");
      for fragment in &fragments {
        get_line_merge_type(fragment, texts, policy);
      }
      check_result_merge(texts, &fragments, policy, policy != ComparisonPolicy::Default);
    });
  }
}

// jscpd:ignore-end

fn for_each_policy_three(text1: &str, text2: &str, text3: &str, test: impl Fn([&Utf16Text; 3], ComparisonPolicy)) {
  let (text1, text2, text3) = (Utf16Text::new(text1), Utf16Text::new(text2), Utf16Text::new(text3));
  for policy in COMPARISON_POLICIES {
    in_context(&policy, || test([&text1, &text2, &text3], policy));
  }
}

fn for_each_inner_policy(text1: &str, text2: &str, fragments_policies: &[InnerFragmentsPolicy], test: impl Fn(&Utf16Text, &Utf16Text, ComparisonPolicy, InnerFragmentsPolicy)) {
  for_each_policy(text1, text2, |text1, text2, policy| {
    for &fragments_policy in fragments_policies {
      in_context(&fragments_policy, || test(text1, text2, policy, fragments_policy));
    }
  });
}

fn for_each_policy(text1: &str, text2: &str, test: impl Fn(&Utf16Text, &Utf16Text, ComparisonPolicy)) {
  let (text1, text2) = (Utf16Text::new(text1), Utf16Text::new(text2));
  for policy in COMPARISON_POLICIES {
    in_context(&policy, || test(&text1, &text2, policy));
  }
}

/// Runs `check` and names `context` in its panic, as the debug data of the source
fn in_context(context: &dyn Debug, check: impl FnOnce()) {
  if let Err(panic) = catch_unwind(AssertUnwindSafe(check)) {
    let message = panic
      .downcast_ref::<String>()
      .map(String::as_str)
      .or_else(|| panic.downcast_ref::<&str>().copied())
      .unwrap_or("a panic");
    panic!("{context:?}: {message}");
  }
}

//
// DiffTestCase
//

/// Two random texts of up to `MAX_LENGTH` chars, with or without high surrogates (`generateText`): `\n`, `\t`, space,
/// `.`, `<`, `!` and the letters `i`-`l`, or, with high surrogates, the same with four emoji and `m`-`p` for the letters
fn texts() -> impl Strategy<Value = (String, String)> {
  any::<bool>().prop_flat_map(|use_high_surrogates| (text(use_high_surrogates), text(use_high_surrogates)))
}

/// Three random texts, as `texts`
fn three_texts() -> impl Strategy<Value = (String, String, String)> {
  any::<bool>().prop_flat_map(|use_high_surrogates| (text(use_high_surrogates), text(use_high_surrogates), text(use_high_surrogates)))
}

fn text(use_high_surrogates: bool) -> impl Strategy<Value = String> {
  let mut table = vec!["\n", "\n", "\t", " ", " ", ".", "<", "!"];
  if use_high_surrogates {
    table.extend([CH_SMILE, CH_FACE, CH_GUN, CH_MAN, "m", "n", "o", "p"]);
  } else {
    table.extend(["i", "j", "k", "l"]);
  }
  vec(select(table), 0..=MAX_LENGTH).prop_map(|chars| chars.concat())
}

fn assert_equals_char_sequences(chunk1: &[u16], chunk2: &[u16], ignore_spaces: bool, skip_last_newline: bool) {
  let equal = if skip_last_newline && !ignore_spaces {
    chunk1 == chunk2 || strip_newline(chunk1) == Some(chunk2) || Some(chunk1) == strip_newline(chunk2)
  } else {
    is_equals_char_sequences(chunk1, chunk2, ignore_spaces)
  };
  assert!(equal, "Text1: '{}', Text2: '{}'", String::from_utf16_lossy(chunk1), String::from_utf16_lossy(chunk2));
}

fn assert_not_equals_char_sequences(chunk1: &[u16], chunk2: &[u16], ignore_spaces: bool, skip_last_newline: bool) {
  if skip_last_newline && !ignore_spaces {
    assert!(chunk1 != chunk2 || strip_newline(chunk1) != Some(chunk2) || Some(chunk1) != strip_newline(chunk2));
  } else {
    assert!(!is_equals_char_sequences(chunk1, chunk2, ignore_spaces));
  }
}

fn is_equals_char_sequences(chunk1: &[u16], chunk2: &[u16], ignore_spaces: bool) -> bool {
  if ignore_spaces { equals_ignore_whitespaces(chunk1, chunk2) } else { chunk1 == chunk2 }
}

fn strip_newline(text: &[u16]) -> Option<&[u16]> {
  text.strip_suffix(&[u16::from(b'\n')])
}

//
// Checks
//

fn check_result_line(text1: &Utf16Text, text2: &Utf16Text, fragments: &[LineFragment], policy: ComparisonPolicy, allow_non_squashed: bool, allow_non_trimmed: bool) {
  check_line_consistency(text1, text2, fragments, allow_non_squashed);

  for fragment in fragments {
    if let Some(inner_fragments) = &fragment.inner_fragments {
      let sequence1 = &text1.units()[fragment.start_offset1..fragment.end_offset1];
      let sequence2 = &text2.units()[fragment.start_offset2..fragment.end_offset2];

      check_result_word(sequence1, sequence2, inner_fragments, policy);
    }
  }

  // a block can have empty offsets on both sides, which no `DiffFragment` has
  let offsets: Vec<Range> = fragments.iter().map(LineFragment::offsets).collect();
  check_valid_ranges(text1.units(), text2.units(), &offsets, policy, true);
  if !allow_non_trimmed {
    check_cant_trim_lines(text1, text2, fragments, policy, allow_non_squashed);
  }
}

fn check_result_word(text1: &[u16], text2: &[u16], fragments: &[DiffFragment], policy: ComparisonPolicy) {
  check_diff_consistency(fragments);
  check_valid_ranges(text1, text2, &to_ranges(fragments), policy, false);
}

fn to_ranges(fragments: &[DiffFragment]) -> Vec<Range> {
  fragments
    .iter()
    .map(|it| Range::new(it.start_offset1, it.end_offset1, it.start_offset2, it.end_offset2))
    .collect()
}

fn check_line_consistency(text1: &Utf16Text, text2: &Utf16Text, fragments: &[LineFragment], allow_non_squashed: bool) {
  let mut last: Option<(usize, usize)> = None;

  for fragment in fragments {
    let (start1, start2, end1, end2) = (fragment.start_line1, fragment.start_line2, fragment.end_line1, fragment.end_line2);

    assert!(fragment.end_offset1 <= text1.len());
    assert!(fragment.end_offset2 <= text2.len());

    assert!(end1 <= get_line_count(text1));
    assert!(end2 <= get_line_count(text2));

    assert!(fragment.start_offset1 <= fragment.end_offset1);
    assert!(fragment.start_offset2 <= fragment.end_offset2);

    assert!(start1 <= end1);
    assert!(start2 <= end2);
    assert!(start1 != end1 || start2 != end2);

    assert!(allow_non_squashed || last != Some((start1, start2)));

    check_line_offsets(text1, start1, end1, fragment.start_offset1, fragment.end_offset1);
    check_line_offsets(text2, start2, end2, fragment.start_offset2, fragment.end_offset2);

    last = Some((end1, end2));
  }
}

fn check_diff_consistency(fragments: &[DiffFragment]) {
  let mut last: Option<(usize, usize)> = None;

  for fragment in fragments {
    let (start1, start2, end1, end2) = (fragment.start_offset1, fragment.start_offset2, fragment.end_offset1, fragment.end_offset2);

    assert!(start1 <= end1);
    assert!(start2 <= end2);
    assert!(start1 != end1 || start2 != end2);

    assert_ne!(last, Some((start1, start2)));

    last = Some((end1, end2));
  }
}

fn check_valid_ranges(text1: &[u16], text2: &[u16], fragments: &[Range], policy: ComparisonPolicy, skip_newline: bool) {
  // TODO: better check for Trim spaces case ?
  let ignore_spaces_unchanged = policy != ComparisonPolicy::Default;
  let ignore_spaces_changed = policy == ComparisonPolicy::IgnoreWhitespaces;

  let mut changes_set1 = FixedBitSet::new();
  let mut changes_set2 = FixedBitSet::new();

  let mut last1 = 0;
  let mut last2 = 0;
  for fragment in fragments {
    let Range { start1, end1, start2, end2 } = *fragment;

    assert_equals_char_sequences(&text1[last1..start1], &text2[last2..start2], ignore_spaces_unchanged, skip_newline);

    if !skip_newline {
      assert_not_equals_char_sequences(&text1[start1..end1], &text2[start2..end2], ignore_spaces_changed, skip_newline);
    }

    insert_range(&mut changes_set1, start1, end1);
    insert_range(&mut changes_set2, start2, end2);

    last1 = end1;
    last2 = end2;
  }
  assert_equals_char_sequences(&text1[last1..], &text2[last2..], ignore_spaces_unchanged, skip_newline);

  check_code_points(text1, &changes_set1);
  check_code_points(text2, &changes_set2);
}

/// A change never splits a surrogate pair
fn check_code_points(text: &[u16], changes_set: &FixedBitSet) {
  let mut offset = 0;
  while offset < text.len() {
    let ch = code_point_at(text, offset);
    let char_count = char_count(ch);

    if char_count == 2 {
      assert_eq!(changes_set.contains(offset), changes_set.contains(offset + 1));
    }

    offset += char_count;
  }
}

fn check_cant_trim_lines(text1: &Utf16Text, text2: &Utf16Text, fragments: &[LineFragment], policy: ComparisonPolicy, allow_non_squashed: bool) {
  for fragment in fragments {
    let (Some(sequence1), Some(sequence2)) = (
      get_first_last_lines(text1, fragment.start_line1, fragment.end_line1),
      get_first_last_lines(text2, fragment.start_line2, fragment.end_line2),
    ) else {
      continue;
    };

    check_non_equals_if_long_enough(sequence1.0, sequence2.0, policy, allow_non_squashed);
    check_non_equals_if_long_enough(sequence1.1, sequence2.1, policy, allow_non_squashed);
  }
}

fn check_non_equals_if_long_enough(line1: &[u16], line2: &[u16], policy: ComparisonPolicy, allow_non_squashed: bool) {
  // in non-squashed blocks non-trimmed elements are possible
  if allow_non_squashed {
    if policy != ComparisonPolicy::IgnoreWhitespaces {
      return;
    }
    if count_non_whitespace_characters(line1) <= unimportant_line_char_count() {
      return;
    }
    if count_non_whitespace_characters(line2) <= unimportant_line_char_count() {
      return;
    }
  }

  assert!(
    !is_equal(line1, line2, policy),
    "\"{}\" vs \"{}\"",
    String::from_utf16_lossy(line1),
    String::from_utf16_lossy(line2)
  );
}

fn count_non_whitespace_characters(line: &[u16]) -> usize {
  line.iter().filter(|&&c| !is_space_enter_or_tab(c)).count()
}

fn get_first_last_lines(text: &Utf16Text, start: usize, end: usize) -> Option<(&[u16], &[u16])> {
  if start == end {
    return None;
  }

  let line_offsets = text.line_offsets();
  let (first_start, first_end) = line_offsets.lines_range(start, start + 1, false);
  let (last_start, last_end) = line_offsets.lines_range(end - 1, end, false);

  Some((&text.units()[first_start..first_end], &text.units()[last_start..last_end]))
}

//
// Merge checks
//

/// The lines `[start, end)` of a text without the last `\n` (`DiffUtil.getLinesContent`)
fn lines_content(text: &Utf16Text, start: usize, end: usize) -> &[u16] {
  let (start_offset, end_offset) = text.line_offsets().lines_range(start, end, false);
  &text.units()[start_offset..end_offset]
}

fn check_result_merge(texts: [&Utf16Text; 3], fragments: &[MergeRange], policy: ComparisonPolicy, allow_ignored_blocks: bool) {
  check_line_consistency3(texts, fragments, allow_ignored_blocks);

  check_valid_line_ranges3(texts, fragments, policy);
  if !allow_ignored_blocks {
    check_cant_trim_lines3(texts, fragments, policy);
  }

  for fragment in fragments {
    let chunks = ThreeSide::ALL.map(|side| lines_content(texts[side.index()], fragment.start(side), fragment.end(side)));
    let inner_fragments = by_word::compare_three(chunks[0], chunks[1], chunks[2], policy, &Deadline::NONE).expect("no limits");
    check_diff_consistency3(&inner_fragments);
    check_valid_word_ranges3(chunks, &inner_fragments, policy);
  }
}

fn check_line_consistency3(texts: [&Utf16Text; 3], fragments: &[MergeRange], allow_non_squashed: bool) {
  let mut last: Option<[usize; 3]> = None;
  for fragment in fragments {
    for side in ThreeSide::ALL {
      assert!(fragment.end(side) <= get_line_count(texts[side.index()]));
      assert!(fragment.start(side) <= fragment.end(side));
    }
    assert!(!fragment.is_empty());
    let starts = ThreeSide::ALL.map(|side| fragment.start(side));
    assert!(allow_non_squashed || last != Some(starts));
    last = Some(ThreeSide::ALL.map(|side| fragment.end(side)));
  }
}

fn check_diff_consistency3(fragments: &[MergeRange]) {
  let mut last: Option<[usize; 3]> = None;
  for fragment in fragments {
    for side in ThreeSide::ALL {
      assert!(fragment.start(side) <= fragment.end(side));
    }
    assert!(!fragment.is_empty());
    let starts = ThreeSide::ALL.map(|side| fragment.start(side));
    assert_ne!(last, Some(starts));
    last = Some(ThreeSide::ALL.map(|side| fragment.end(side)));
  }
}

/// The lines between the blocks are equal on all three sides
fn check_valid_line_ranges3(texts: [&Utf16Text; 3], fragments: &[MergeRange], policy: ComparisonPolicy) {
  let ignore_spaces = policy != ComparisonPolicy::Default;
  let check = |last: [usize; 3], starts: [usize; 3]| {
    let contents = ThreeSide::ALL.map(|side| lines_content(texts[side.index()], last[side.index()], starts[side.index()]));
    assert_equals_char_sequences(contents[1], contents[0], ignore_spaces, false);
    assert_equals_char_sequences(contents[1], contents[2], ignore_spaces, false);
  };
  let mut last = [0; 3];
  for fragment in fragments {
    check(last, ThreeSide::ALL.map(|side| fragment.start(side)));
    last = ThreeSide::ALL.map(|side| fragment.end(side));
  }
  check(last, texts.map(get_line_count));
}

/// The chars between the word blocks are equal on all three sides, a block is no equal one, and no block splits a
/// surrogate pair
fn check_valid_word_ranges3(texts: [&[u16]; 3], fragments: &[MergeRange], policy: ComparisonPolicy) {
  let ignore_spaces_unchanged = policy != ComparisonPolicy::Default;
  let ignore_spaces_changed = policy == ComparisonPolicy::IgnoreWhitespaces;
  let mut changes_sets = [FixedBitSet::new(), FixedBitSet::new(), FixedBitSet::new()];
  let check_unchanged = |last: [usize; 3], starts: [usize; 3]| {
    let contents = ThreeSide::ALL.map(|side| &texts[side.index()][last[side.index()]..starts[side.index()]]);
    assert_equals_char_sequences(contents[1], contents[0], ignore_spaces_unchanged, false);
    assert_equals_char_sequences(contents[1], contents[2], ignore_spaces_unchanged, false);
  };

  let mut last = [0; 3];
  for fragment in fragments {
    check_unchanged(last, ThreeSide::ALL.map(|side| fragment.start(side)));
    let chunks = ThreeSide::ALL.map(|side| &texts[side.index()][fragment.start(side)..fragment.end(side)]);
    assert!(!(is_equals_char_sequences(chunks[1], chunks[0], ignore_spaces_changed) && is_equals_char_sequences(chunks[1], chunks[2], ignore_spaces_changed)));
    for side in ThreeSide::ALL {
      insert_range(&mut changes_sets[side.index()], fragment.start(side), fragment.end(side));
    }
    last = ThreeSide::ALL.map(|side| fragment.end(side));
  }
  check_unchanged(last, texts.map(<[u16]>::len));

  for (text, changes_set) in texts.iter().zip(&changes_sets) {
    check_code_points(text, changes_set);
  }
}

fn check_cant_trim_lines3(texts: [&Utf16Text; 3], fragments: &[MergeRange], policy: ComparisonPolicy) {
  for fragment in fragments {
    let [Some(sequence1), Some(sequence2), Some(sequence3)] = ThreeSide::ALL.map(|side| get_first_last_lines(texts[side.index()], fragment.start(side), fragment.end(side))) else {
      continue;
    };
    assert!(!(is_equal(sequence2.0, sequence1.0, policy) && is_equal(sequence2.0, sequence3.0, policy)));
    assert!(!(is_equal(sequence2.1, sequence1.1, policy) && is_equal(sequence2.1, sequence3.1, policy)));
  }
}
