// Ported and modified from `community/platform/diff-impl/src/com/intellij/diff/comparison/ComparisonManagerImpl.java`
// of IntelliJ Platform (Apache License 2.0, Copyright 2000-2024 JetBrains s.r.o. and contributors). Left out: the
// registry switch `diff.by.word.deprioritize.line.differences` (off by default), which would take
// `compare_lines_word_first` for the words; that function is ported.

//! The comparison of two texts into changed blocks of lines, the changes inside them, and the post-processing of the
//! blocks.

use crate::by_char;
use crate::by_line;
use crate::by_word;
use crate::by_word::{InlineChunk, LineBlock};
use crate::cancellation::{Deadline, DiffTooBig};
use crate::comparison_util::{is_equal_texts, is_equals};
use crate::fragments::{DiffFragment, LineFragment};
use crate::iterables::diff_iterable::DiffIterable;
use crate::iterables::diff_iterable_util::{ChangeBuilder, create, fair};
use crate::lcs::diff::MAX_BAD_LINES;
use crate::line_offsets::LineOffsets;
use crate::merge_range::MergeRange;
use crate::policy::{ComparisonPolicy, InnerFragmentsPolicy};
use crate::range::Range;
use crate::text::Utf16Text;
use crate::trim_util::{trim_expand, trim_with};
use fixedbitset::FixedBitSet;

/// Compares two texts by line
pub fn compare_lines(text1: &Utf16Text, text2: &Utf16Text, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<LineFragment>, DiffTooBig> {
  let boundary_range = full_range(text1, text2);
  compare_lines_in_range(boundary_range, text1, text2, policy, deadline)
}

/// Compares three texts by line, `text2` the base: a change wherever a side differs from the base
pub fn compare_lines_three(text1: &Utf16Text, text2: &Utf16Text, text3: &Utf16Text, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<MergeRange>, DiffTooBig> {
  let (line_texts1, line_texts2, line_texts3) = (all_line_contents(text1), all_line_contents(text2), all_line_contents(text3));
  by_line::compare_three(&line_texts1, &line_texts2, &line_texts3, policy, deadline)
}

/// Merges three texts by line, `text2` the base: as `compare_lines_three`, but a change the policy ignores stays a
/// change of its own, outside the conflicts
pub fn merge_lines(text1: &Utf16Text, text2: &Utf16Text, text3: &Utf16Text, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<MergeRange>, DiffTooBig> {
  let (line_texts1, line_texts2, line_texts3) = (all_line_contents(text1), all_line_contents(text2), all_line_contents(text3));
  by_line::merge(&line_texts1, &line_texts2, &line_texts3, policy, deadline)
}

/// `merge_lines` of the lines of `boundary_range` only, in the lines of the whole texts
pub fn merge_lines_within_range(
  text1: &Utf16Text,
  text2: &Utf16Text,
  text3: &Utf16Text,
  policy: ComparisonPolicy,
  boundary_range: MergeRange,
  deadline: &Deadline,
) -> Result<Vec<MergeRange>, DiffTooBig> {
  let line_texts1 = get_line_contents(boundary_range.start1, boundary_range.end1, text1);
  let line_texts2 = get_line_contents(boundary_range.start2, boundary_range.end2, text2);
  let line_texts3 = get_line_contents(boundary_range.start3, boundary_range.end3, text3);
  let ranges = by_line::merge(&line_texts1, &line_texts2, &line_texts3, policy, deadline)?;
  Ok(by_line::convert_into_merge_line_fragments(&ranges, boundary_range))
}

/// The lines that `text1` and `text3` share, joined by `\n`: the base of a merge of two texts that only add lines
pub fn merge_lines_additions(text1: &Utf16Text, text3: &Utf16Text, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<u16>, DiffTooBig> {
  let (line_texts1, line_texts3) = (all_line_contents(text1), all_line_contents(text3));
  let diff = by_line::compare(&line_texts1, &line_texts3, policy, deadline)?;

  let mut base: Vec<u16> = Vec::new();
  for range in diff.unchanged() {
    for line in &line_texts1[range.start1..range.end1] {
      if !base.is_empty() {
        base.push(u16::from(b'\n'));
      }
      base.extend_from_slice(line);
    }
  }
  Ok(base)
}

/// Compares the lines `[start1, end1)` of `text1` with the lines `[start2, end2)` of `text2`
pub fn compare_lines_in_range(boundary_range: Range, text1: &Utf16Text, text2: &Utf16Text, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<LineFragment>, DiffTooBig> {
  let line_texts1 = get_line_contents(boundary_range.start1, boundary_range.end1, text1);
  let line_texts2 = get_line_contents(boundary_range.start2, boundary_range.end2, text2);

  let iterable = by_line::compare(&line_texts1, &line_texts2, policy, deadline)?;
  Ok(convert_into_line_fragments(boundary_range, text1.line_offsets(), text2.line_offsets(), &iterable))
}

/// Compares two texts by line, then the changed blocks by word
pub fn compare_lines_inner(text1: &Utf16Text, text2: &Utf16Text, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<LineFragment>, DiffTooBig> {
  let line_fragments = compare_lines(text1, text2, policy, deadline)?;
  create_inner_fragments(&line_fragments, text1.units(), text2.units(), policy, InnerFragmentsPolicy::Words, deadline)
}

/// Compares two texts by line, then the changed blocks as `fragments_policy` asks
pub fn compare_lines_inner_with(
  text1: &Utf16Text,
  text2: &Utf16Text,
  policy: ComparisonPolicy,
  fragments_policy: InnerFragmentsPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  compare_lines_inner_in_range(full_range(text1, text2), text1, text2, policy, fragments_policy, deadline)
}

/// Compares the lines of `boundary_range`, then the changed blocks as `fragments_policy` asks
pub fn compare_lines_inner_in_range(
  boundary_range: Range,
  text1: &Utf16Text,
  text2: &Utf16Text,
  policy: ComparisonPolicy,
  fragments_policy: InnerFragmentsPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  let line_fragments = compare_lines_in_range(boundary_range, text1, text2, policy, deadline)?;
  if fragments_policy == InnerFragmentsPolicy::None {
    Ok(line_fragments)
  } else {
    create_inner_fragments(&line_fragments, text1.units(), text2.units(), policy, fragments_policy, deadline)
  }
}

/// The changes inside each block; a block whose words differ in too many places keeps no inner changes, and after
/// `MAX_BAD_LINES` such blocks the rest is not compared by word at all
fn create_inner_fragments(
  line_fragments: &[LineFragment],
  text1: &[u16],
  text2: &[u16],
  policy: ComparisonPolicy,
  fragments_policy: InnerFragmentsPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  let mut result = Vec::with_capacity(line_fragments.len());

  let mut too_big_chunks_count = 0;
  for fragment in line_fragments {
    debug_assert!(fragment.inner_fragments.is_none());

    // Do not try to build fine blocks after few fails
    let try_compute_differences = too_big_chunks_count < MAX_BAD_LINES;
    match create_inner_fragments_of(fragment, text1, text2, policy, fragments_policy, deadline, try_compute_differences) {
      Ok(fragments) => result.extend(fragments),
      Err(DiffTooBig::TooManyChanges) => {
        result.push(fragment.clone());
        too_big_chunks_count += 1;
      }
      Err(error @ DiffTooBig::DeadlineExceeded) => return Err(error),
    }
  }

  Ok(result)
}

fn create_inner_fragments_of(
  fragment: &LineFragment,
  text1: &[u16],
  text2: &[u16],
  policy: ComparisonPolicy,
  fragments_policy: InnerFragmentsPolicy,
  deadline: &Deadline,
  try_compute_differences: bool,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  if fragments_policy == InnerFragmentsPolicy::None {
    return Ok(vec![fragment.clone()]);
  }

  let sub_sequence1 = &text1[fragment.start_offset1..fragment.end_offset1];
  let sub_sequence2 = &text2[fragment.start_offset2..fragment.end_offset2];

  if fragment.start_line1 == fragment.end_line1 || fragment.start_line2 == fragment.end_line2 {
    // Insertion / Deletion
    if is_equal_texts(sub_sequence1, sub_sequence2, policy) {
      return Ok(vec![fragment.with_inner(Some(Vec::new()))]);
    }
    return Ok(vec![fragment.clone()]);
  }

  if !try_compute_differences {
    return Ok(vec![fragment.clone()]);
  }

  match fragments_policy {
    InnerFragmentsPolicy::Words => create_inner_word_fragments(fragment, sub_sequence1, sub_sequence2, policy, deadline),
    InnerFragmentsPolicy::Chars => create_inner_char_fragments(fragment, sub_sequence1, sub_sequence2, policy, deadline),
    InnerFragmentsPolicy::None => unreachable!("handled above"),
  }
}

fn create_inner_word_fragments(
  fragment: &LineFragment,
  sub_sequence1: &[u16],
  sub_sequence2: &[u16],
  policy: ComparisonPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  let line_blocks: Vec<LineBlock> = by_word::compare_and_split(sub_sequence1, sub_sequence2, policy, deadline)?;

  let start_offset1 = fragment.start_offset1;
  let start_offset2 = fragment.start_offset2;

  let mut current_start_line1 = fragment.start_line1;
  let mut current_start_line2 = fragment.start_line2;

  let mut chunks = Vec::with_capacity(line_blocks.len());
  let last_index = line_blocks.len() - 1;
  for (i, block) in line_blocks.into_iter().enumerate() {
    let offsets = block.offsets;

    // special case for last line to void problem with empty last line
    let current_end_line1 = if i == last_index { fragment.end_line1 } else { current_start_line1 + block.newlines1 };
    let current_end_line2 = if i == last_index { fragment.end_line2 } else { current_start_line2 + block.newlines2 };

    chunks.push(LineFragment::new(
      Range::new(current_start_line1, current_end_line1, current_start_line2, current_end_line2),
      Range::new(
        offsets.start1 + start_offset1,
        offsets.end1 + start_offset1,
        offsets.start2 + start_offset2,
        offsets.end2 + start_offset2,
      ),
      Some(block.fragments),
    ));

    current_start_line1 = current_end_line1;
    current_start_line2 = current_end_line2;
  }
  Ok(chunks)
}

fn create_inner_char_fragments(
  fragment: &LineFragment,
  sub_sequence1: &[u16],
  sub_sequence2: &[u16],
  policy: ComparisonPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  let inner_changes = compare_chars(sub_sequence1, sub_sequence2, policy, deadline)?;
  Ok(vec![fragment.with_inner(Some(inner_changes))])
}

/// Compares two texts by code point
pub fn compare_chars(text1: &[u16], text2: &[u16], policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<DiffFragment>, DiffTooBig> {
  let iterable = match policy {
    ComparisonPolicy::Default => by_char::compare_two_step(text1, text2, deadline)?,
    ComparisonPolicy::TrimWhitespaces => by_char::compare_trim_whitespaces(text1, text2, deadline)?,
    ComparisonPolicy::IgnoreWhitespaces => by_char::compare_ignore_whitespaces(text1, text2, deadline)?,
  };

  Ok(by_word::convert_into_diff_fragments(&iterable))
}

/// Compares two texts by word first and lifts the changed words to blocks of lines (experimental in the source)
pub fn compare_lines_word_first(text1: &Utf16Text, text2: &Utf16Text, policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<LineFragment>, DiffTooBig> {
  let (line_offsets1, line_offsets2) = (text1.line_offsets(), text2.line_offsets());
  let line_blocks = by_word::compare_words_first(text1.units(), text2.units(), line_offsets1, line_offsets2, policy, deadline)?;
  Ok(
    line_blocks
      .into_iter()
      .map(|block| create_line_fragment(block.line_range, line_offsets1, line_offsets2).with_inner(Some(block.fragments)))
      .collect(),
  )
}

/// Compares two texts by word
pub fn compare_words(text1: &[u16], text2: &[u16], policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<DiffFragment>, DiffTooBig> {
  by_word::compare(text1, text2, policy, deadline)
}

pub fn is_equal(text1: &[u16], text2: &[u16], policy: ComparisonPolicy) -> bool {
  is_equal_texts(text1, text2, policy)
}

//
// Fragments
//

/// The changes of a comparison of lines, which starts at the lines `boundary_range.start1` and `start2`, as blocks
pub fn convert_into_line_fragments(boundary_range: Range, line_offsets1: &LineOffsets, line_offsets2: &LineOffsets, changes: &DiffIterable) -> Vec<LineFragment> {
  changes
    .changes()
    .iter()
    .map(|range| {
      let lines = Range::new(
        range.start1 + boundary_range.start1,
        range.end1 + boundary_range.start1,
        range.start2 + boundary_range.start2,
        range.end2 + boundary_range.start2,
      );
      create_line_fragment(lines, line_offsets1, line_offsets2)
    })
    .collect()
}

/// A block of lines with the offsets of the lines
pub fn create_line_fragment(lines: Range, line_offsets1: &LineOffsets, line_offsets2: &LineOffsets) -> LineFragment {
  let (start_offset1, end_offset1) = get_offsets(line_offsets1, lines.start1, lines.end1);
  let (start_offset2, end_offset2) = get_offsets(line_offsets2, lines.start2, lines.end2);
  LineFragment::new(lines, Range::new(start_offset1, end_offset1, start_offset2, end_offset2), None)
}

fn get_offsets(line_offsets: &LineOffsets, start_index: usize, end_index: usize) -> (usize, usize) {
  if start_index == end_index {
    let offset = if start_index < line_offsets.line_count() {
      line_offsets.line_start(start_index)
    } else {
      line_offsets.line_end_with(line_offsets.line_count() - 1, true)
    };
    (offset, offset)
  } else {
    let offset1 = line_offsets.line_start(start_index);
    let offset2 = line_offsets.line_end_with(end_index - 1, true);
    (offset1, offset2)
  }
}

//
// Post process line fragments
//

/// Merges each run of adjoining blocks into one block
pub fn squash(old_fragments: Vec<LineFragment>) -> Vec<LineFragment> {
  if old_fragments.is_empty() {
    return old_fragments;
  }

  let mut new_fragments = Vec::new();
  process_adjoining(&old_fragments, |fragments| new_fragments.push(do_squash(fragments)));
  new_fragments
}

/// Merges each run of adjoining blocks when `squash` is set, and cuts the blocks at the ends of a run that changed only
/// in whitespace when `trim` is set under `IgnoreWhitespaces`
pub fn process_blocks(old_fragments: Vec<LineFragment>, text1: &[u16], text2: &[u16], policy: ComparisonPolicy, squash: bool, trim: bool) -> Vec<LineFragment> {
  if !squash && !trim {
    return old_fragments;
  }
  if old_fragments.is_empty() {
    return old_fragments;
  }

  let mut new_fragments = Vec::new();
  process_adjoining(&old_fragments, |fragments| {
    new_fragments.extend(process_adjoining_blocks(fragments, text1, text2, policy, squash, trim));
  });
  new_fragments
}

/// Calls `consumer` with each run of adjoining blocks
pub(crate) fn process_adjoining(old_fragments: &[LineFragment], mut consumer: impl FnMut(&[LineFragment])) {
  let mut start_index = 0;
  for i in 1..old_fragments.len() {
    if !is_adjoining(&old_fragments[i - 1], &old_fragments[i]) {
      consumer(&old_fragments[start_index..i]);
      start_index = i;
    }
  }
  if start_index < old_fragments.len() {
    consumer(&old_fragments[start_index..]);
  }
}

fn process_adjoining_blocks(fragments: &[LineFragment], text1: &[u16], text2: &[u16], policy: ComparisonPolicy, squash: bool, trim: bool) -> Vec<LineFragment> {
  let (start, end) = trimmed_run(fragments, text1, text2, policy, trim);

  if start == end {
    return Vec::new();
  }
  if squash {
    return vec![do_squash(&fragments[start..end])];
  }
  fragments[start..end].to_vec()
}

/// The part `[start, end)` of a run of adjoining blocks that trimming keeps
pub(crate) fn trimmed_run(fragments: &[LineFragment], text1: &[u16], text2: &[u16], policy: ComparisonPolicy, trim: bool) -> (usize, usize) {
  let mut start = 0;
  let mut end = fragments.len();

  // TODO: trim empty leading/trailing lines
  if trim && policy == ComparisonPolicy::IgnoreWhitespaces {
    // a block whose inner changes are empty, or whose texts differ only in whitespace
    let is_whitespace_only = |fragment: &LineFragment| {
      let sequence1 = &text1[fragment.start_offset1..fragment.end_offset1];
      let sequence2 = &text2[fragment.start_offset2..fragment.end_offset2];
      fragment.inner_fragments.as_ref().is_some_and(Vec::is_empty) || is_equals(sequence1, sequence2, ComparisonPolicy::IgnoreWhitespaces)
    };
    while start < end && is_whitespace_only(&fragments[start]) {
      start += 1;
    }
    while start < end && is_whitespace_only(&fragments[end - 1]) {
      end -= 1;
    }
  }
  (start, end)
}

pub(crate) fn do_squash(old_fragments: &[LineFragment]) -> LineFragment {
  if let [fragment] = old_fragments {
    return fragment.clone();
  }

  let first_fragment = &old_fragments[0];
  let last_fragment = &old_fragments[old_fragments.len() - 1];

  let mut new_inner_fragments: Vec<DiffFragment> = Vec::new();
  for fragment in old_fragments {
    for inner_fragment in extract_inner_fragments(fragment) {
      let shift1 = fragment.start_offset1 - first_fragment.start_offset1;
      let shift2 = fragment.start_offset2 - first_fragment.start_offset2;

      match new_inner_fragments.last_mut() {
        Some(previous_fragment) if is_adjoining_inner(previous_fragment, &inner_fragment, shift1, shift2) => {
          *previous_fragment = DiffFragment::new(
            previous_fragment.start_offset1,
            inner_fragment.end_offset1 + shift1,
            previous_fragment.start_offset2,
            inner_fragment.end_offset2 + shift2,
          );
        }
        _ => new_inner_fragments.push(DiffFragment::new(
          inner_fragment.start_offset1 + shift1,
          inner_fragment.end_offset1 + shift1,
          inner_fragment.start_offset2 + shift2,
          inner_fragment.end_offset2 + shift2,
        )),
      }
    }
  }

  LineFragment::new(
    Range::new(first_fragment.start_line1, last_fragment.end_line1, first_fragment.start_line2, last_fragment.end_line2),
    Range::new(
      first_fragment.start_offset1,
      last_fragment.end_offset1,
      first_fragment.start_offset2,
      last_fragment.end_offset2,
    ),
    Some(new_inner_fragments),
  )
}

const fn is_adjoining(before_fragment: &LineFragment, after_fragment: &LineFragment) -> bool {
  before_fragment.end_line1 == after_fragment.start_line1
    && before_fragment.end_line2 == after_fragment.start_line2
    && before_fragment.end_offset1 == after_fragment.start_offset1
    && before_fragment.end_offset2 == after_fragment.start_offset2
}

const fn is_adjoining_inner(before_fragment: &DiffFragment, after_fragment: &DiffFragment, shift1: usize, shift2: usize) -> bool {
  before_fragment.end_offset1 == after_fragment.start_offset1 + shift1 && before_fragment.end_offset2 == after_fragment.start_offset2 + shift2
}

fn extract_inner_fragments(line_fragment: &LineFragment) -> Vec<DiffFragment> {
  if let Some(inner_fragments) = &line_fragment.inner_fragments {
    return inner_fragments.clone();
  }

  let length1 = line_fragment.end_offset1 - line_fragment.start_offset1;
  let length2 = line_fragment.end_offset2 - line_fragment.start_offset2;
  vec![DiffFragment::new(0, length1, 0, length2)]
}

const fn full_range(text1: &Utf16Text, text2: &Utf16Text) -> Range {
  Range::new(0, text1.line_offsets().line_count(), 0, text2.line_offsets().line_count())
}

fn all_line_contents(text: &Utf16Text) -> Vec<&[u16]> {
  get_line_contents(0, text.line_offsets().line_count(), text)
}

fn get_line_contents(start: usize, end: usize, text: &Utf16Text) -> Vec<&[u16]> {
  let line_offsets = text.line_offsets();
  (start..end).map(|line| &text.units()[line_offsets.line_start(line)..line_offsets.line_end(line)]).collect()
}

fn get_not_ignored_line_contents(start: usize, end: usize, text: &Utf16Text, ignored: &FixedBitSet) -> Vec<Vec<u16>> {
  let line_offsets = text.line_offsets();
  (start..end)
    .map(|line| {
      (line_offsets.line_start(line)..line_offsets.line_end(line))
        .filter(|&offset| !ignored.contains(offset))
        .map(|offset| text.units()[offset])
        .collect()
    })
    .collect()
}

//
// Ignored ranges
//

/// Compares two texts by line while the chars of `ignored1` and `ignored2` do not count, then the changed blocks as
/// `fragments_policy` asks
pub fn compare_lines_with_ignored_ranges(
  text1: &Utf16Text,
  text2: &Utf16Text,
  ignored1: &FixedBitSet,
  ignored2: &FixedBitSet,
  fragments_policy: InnerFragmentsPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  compare_lines_with_ignored_ranges_in_range(full_range(text1, text2), text1, text2, ignored1, ignored2, fragments_policy, deadline)
}

/// Compares the lines of `boundary_range` by line while the ignored chars do not count, then the changed blocks
pub fn compare_lines_with_ignored_ranges_in_range(
  boundary_range: Range,
  text1: &Utf16Text,
  text2: &Utf16Text,
  ignored1: &FixedBitSet,
  ignored2: &FixedBitSet,
  fragments_policy: InnerFragmentsPolicy,
  deadline: &Deadline,
) -> Result<Vec<LineFragment>, DiffTooBig> {
  let line_texts1 = get_not_ignored_line_contents(boundary_range.start1, boundary_range.end1, text1, ignored1);
  let line_texts2 = get_not_ignored_line_contents(boundary_range.start2, boundary_range.end2, text2, ignored2);
  let line_texts1: Vec<&[u16]> = line_texts1.iter().map(Vec::as_slice).collect();
  let line_texts2: Vec<&[u16]> = line_texts2.iter().map(Vec::as_slice).collect();

  let iterable = by_line::compare(&line_texts1, &line_texts2, ComparisonPolicy::Default, deadline)?;

  let ignored = IgnoredLines {
    boundary_range,
    text1,
    text2,
    ignored1,
    ignored2,
  };
  let corrected_iterable = ignored.correct_ignored_ranges_second_step(&iterable);

  let trimmed_iterable = ignored.trim_ignored_lines(&corrected_iterable);

  let mut line_fragments = convert_into_line_fragments(boundary_range, text1.line_offsets(), text2.line_offsets(), &trimmed_iterable);

  if fragments_policy != InnerFragmentsPolicy::None {
    line_fragments = create_inner_fragments(&line_fragments, text1.units(), text2.units(), ComparisonPolicy::Default, fragments_policy, deadline)?;
  }

  Ok(
    line_fragments
      .into_iter()
      .filter_map(|fragment| trim_ignored_inner_fragments(fragment, ignored1, ignored2))
      .collect(),
  )
}

/// The ignored chars of the given ranges
pub fn collect_ignored_ranges(ignored_ranges: &[(usize, usize)]) -> FixedBitSet {
  let mut set = FixedBitSet::default();
  for &(start, end) in ignored_ranges {
    set.grow(end);
    set.insert_range(start..end);
  }
  set
}

/// The texts and the ignored chars of `compareLinesWithIgnoredRanges`
struct IgnoredLines<'a> {
  boundary_range: Range,
  text1: &'a Utf16Text,
  text2: &'a Utf16Text,
  ignored1: &'a FixedBitSet,
  ignored2: &'a FixedBitSet,
}

impl IgnoredLines<'_> {
  fn correct_ignored_ranges_second_step(&self, iterable: &DiffIterable) -> DiffIterable {
    let mut builder = ChangeBuilder::new(iterable.length1(), iterable.length2());
    for ch in iterable.unchanged() {
      let count = ch.end1 - ch.start1;
      for i in 0..count {
        let index1 = ch.start1 + i;
        let index2 = ch.start2 + i;
        if self.are_ignored_equal_lines(self.boundary_range.start1 + index1, self.boundary_range.start2 + index2) {
          builder.mark_equal(index1, index2);
        }
      }
    }
    fair(builder.finish())
  }

  fn trim_ignored_lines(&self, iterable: &DiffIterable) -> DiffIterable {
    let mut changed_ranges = Vec::new();

    for ch in iterable.changes() {
      let trimmed_range = trim_expand(
        ch.start1,
        ch.start2,
        ch.end1,
        ch.end2,
        |index1, index2| self.are_ignored_equal_lines(self.boundary_range.start1 + index1, self.boundary_range.start2 + index2),
        |index| is_ignored_line(self.boundary_range.start1 + index, self.text1.line_offsets(), self.ignored1),
        |index| is_ignored_line(self.boundary_range.start2 + index, self.text2.line_offsets(), self.ignored2),
      );

      if !trimmed_range.is_empty() {
        changed_ranges.push(trimmed_range);
      }
    }

    create(changed_ranges, iterable.length1(), iterable.length2())
  }

  fn are_ignored_equal_lines(&self, index1: usize, index2: usize) -> bool {
    let (text1, text2) = (self.text1.units(), self.text2.units());
    let (line_offsets1, line_offsets2) = (self.text1.line_offsets(), self.text2.line_offsets());
    let start1 = line_offsets1.line_start(index1);
    let end1 = line_offsets1.line_end_with(index1, true);
    let start2 = line_offsets2.line_start(index2);
    let end2 = line_offsets2.line_end_with(index2, true);
    let range = trim_expand(
      start1,
      start2,
      end1,
      end2,
      |i1, i2| text1[i1] == text2[i2],
      |i| self.ignored1.contains(i),
      |i| self.ignored2.contains(i),
    );
    if !range.is_empty() {
      return false;
    }

    let words1 = get_non_ignored_words(index1, self.text1, self.ignored1);
    let words2 = get_non_ignored_words(index2, self.text2, self.ignored2);
    if words1.len() != words2.len() {
      return false;
    }

    words1.iter().zip(&words2).all(|(word1, word2)| {
      let word1 = get_word_content(index1, self.text1, word1);
      let word2 = get_word_content(index2, self.text2, word2);
      is_equals(word1, word2, ComparisonPolicy::Default)
    })
  }
}

fn trim_ignored_inner_fragments(fragment: LineFragment, ignored1: &FixedBitSet, ignored2: &FixedBitSet) -> Option<LineFragment> {
  let Some(inner_fragments) = &fragment.inner_fragments else {
    return Some(fragment);
  };

  let start_offset1 = fragment.start_offset1;
  let start_offset2 = fragment.start_offset2;

  let new_inner: Vec<DiffFragment> = inner_fragments
    .iter()
    .filter_map(|it| {
      let (start1, end1) = trim_ignored_range(it.start_offset1, it.end_offset1, ignored1, start_offset1);
      let (start2, end2) = trim_ignored_range(it.start_offset2, it.end_offset2, ignored2, start_offset2);

      (start1 != end1 || start2 != end2).then(|| DiffFragment::new(start1, end1, start2, end2))
    })
    .collect();

  if new_inner.is_empty() {
    return None;
  }
  Some(fragment.with_inner(Some(new_inner)))
}

fn is_ignored_line(index: usize, line_offsets: &LineOffsets, ignored: &FixedBitSet) -> bool {
  is_ignored_range(ignored, line_offsets.line_start(index), line_offsets.line_end_with(index, true))
}

fn get_non_ignored_words<'a>(index: usize, text: &'a Utf16Text, ignored: &FixedBitSet) -> Vec<InlineChunk<'a>> {
  let offset = text.line_offsets().line_start(index);
  by_word::get_inline_chunks(get_line_content(index, text))
    .into_iter()
    .filter(|it| by_word::is_word_chunk(it) && !is_ignored_range(ignored, offset + it.offset1(), offset + it.offset2()))
    .collect()
}

fn get_word_content<'a>(index: usize, text: &'a Utf16Text, word: &InlineChunk<'_>) -> &'a [u16] {
  &get_line_content(index, text)[word.offset1()..word.offset2()]
}

fn trim_ignored_range(start: usize, end: usize, ignored: &FixedBitSet, offset: usize) -> (usize, usize) {
  let (start, end) = trim_with(offset + start, offset + end, |index| ignored.contains(index));
  (start - offset, end - offset)
}

/// Whether every char of `[start, end)` is ignored; a bit past the end of the set is not
fn is_ignored_range(ignored: &FixedBitSet, start: usize, end: usize) -> bool {
  start >= end || end <= ignored.len() && ignored.contains_all_in_range(start..end)
}

fn get_line_content(index: usize, text: &Utf16Text) -> &[u16] {
  let line_offsets = text.line_offsets();
  &text.units()[line_offsets.line_start(index)..line_offsets.line_end_with(index, true)]
}
