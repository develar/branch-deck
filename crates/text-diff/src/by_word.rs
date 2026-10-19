// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/ByWordRt.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). Left out: the three-side
// comparison of the merge and its correctors, and the conversion into merge fragments. A `Vec<u16>` of both texts
// replaces `MergingCharSequence`.

//! The comparison of two texts by word: words and newlines are matched first, then the punctuation and the
//! whitespace between them.

use crate::by_char;
use crate::cancellation::{Deadline, DiffTooBig};
use crate::character_utils::{char_count, code_point_at};
use crate::chunk_optimizer::optimize_word_chunks;
use crate::comparison_util::{is_equal_texts, is_equals as is_equals_policy};
use crate::fragments::DiffFragment;
use crate::iterables::diff_iterable::DiffIterable;
use crate::iterables::diff_iterable_util::{ChangeBuilder, create, create_unchanged, diff, fair};
use crate::line_fragment_splitter::{LineFragmentSplitter, WordBlock};
use crate::line_offsets::LineOffsets;
use crate::policy::ComparisonPolicy;
use crate::range::Range;
use crate::trim_util::{
  expand_whitespaces, expand_whitespaces_backward, expand_whitespaces_forward, is_alpha, is_continuous_script, is_equals, is_equals_ignore_whitespaces, is_space_enter_or_tab,
  trim_end, trim_range, trim_start,
};
use std::hash::{Hash, Hasher};

/// Compares two texts by word
pub fn compare(text1: &[u16], text2: &[u16], policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<DiffFragment>, DiffTooBig> {
  deadline.check_canceled()?;

  let words1 = get_inline_chunks(text1);
  let words2 = get_inline_chunks(text2);

  compare_chunks(text1, &words1, text2, &words2, policy, deadline)
}

/// Compares two texts by their words
pub fn compare_chunks(
  text1: &[u16],
  words1: &[InlineChunk<'_>],
  text2: &[u16],
  words2: &[InlineChunk<'_>],
  policy: ComparisonPolicy,
  deadline: &Deadline,
) -> Result<Vec<DiffFragment>, DiffTooBig> {
  let word_changes = diff(words1, words2, deadline)?;
  let word_changes = optimize_word_chunks(text1, text2, words1, words2, &word_changes);

  let delimiters_iterable = match_adjustment_delimiters(text1, text2, words1, words2, &word_changes, 0, 0, deadline)?;
  let iterable = match_adjustment_whitespaces(text1, text2, &delimiters_iterable, policy);

  Ok(convert_into_diff_fragments(&iterable))
}

/// Compares two texts by word and splits them into blocks of lines at the matched words
pub fn compare_and_split(text1: &[u16], text2: &[u16], policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<LineBlock>, DiffTooBig> {
  deadline.check_canceled()?;

  let words1 = get_inline_chunks(text1);
  let words2 = get_inline_chunks(text2);

  let word_changes = diff(&words1, &words2, deadline)?;
  let word_changes = optimize_word_chunks(text1, text2, &words1, &words2, &word_changes);

  let word_blocks = LineFragmentSplitter::new(text1, text2, &words1, &words2, &word_changes).run();

  let sub_iterables = collect_word_block_sub_iterables(&word_changes, &word_blocks);

  let mut line_blocks = Vec::with_capacity(word_blocks.len());
  for (block, sub_iterable) in word_blocks.iter().zip(&sub_iterables) {
    let offsets = block.offsets;
    let words = block.words;

    let subtext1 = &text1[offsets.start1..offsets.end1];
    let subtext2 = &text2[offsets.start2..offsets.end2];

    let subwords1 = &words1[words.start1..words.end1];
    let subwords2 = &words2[words.start2..words.end2];

    let delimiters_iterable = match_adjustment_delimiters(subtext1, subtext2, subwords1, subwords2, sub_iterable, offsets.start1, offsets.start2, deadline)?;
    let iterable = match_adjustment_whitespaces(subtext1, subtext2, &delimiters_iterable, policy);

    let fragments = convert_into_diff_fragments(&iterable);

    let newlines1 = count_newlines(subwords1);
    let newlines2 = count_newlines(subwords2);

    line_blocks.push(LineBlock {
      fragments,
      offsets,
      newlines1,
      newlines2,
    });
  }

  Ok(line_blocks)
}

fn collect_word_block_sub_iterables(word_changes: &DiffIterable, word_blocks: &[WordBlock]) -> Vec<DiffIterable> {
  let changed = word_changes.changes();

  let mut index = 0;

  let mut sub_iterables = Vec::with_capacity(word_blocks.len());
  for block in word_blocks {
    let words = block.words;

    while index < changed.len() {
      let range = changed[index];
      if range.end1 < words.start1 || range.end2 < words.start2 {
        index += 1;
        continue;
      }

      break;
    }

    sub_iterables.push(fair(DiffIterable::subiterable(changed, words, index)));
  }
  sub_iterables
}

/// Constructs 'lines + words' differences while giving priority to the 'words' delta.
///
/// This works better than `compare` for inputs that have no matching lines (and get dragged away by the empty lines
/// as a result). For example, 'Java -> Kotlin' conversions, as the latter has no trailing ';', or text in natural
/// language with manual line wrapping (that is inconsistent between compared texts).
pub fn compare_words_first(
  text1: &[u16],
  text2: &[u16],
  line_offsets1: &LineOffsets,
  line_offsets2: &LineOffsets,
  policy: ComparisonPolicy,
  deadline: &Deadline,
) -> Result<Vec<WordLineBlock>, DiffTooBig> {
  let words1: Vec<InlineChunk<'_>> = get_inline_chunks(text1).into_iter().filter(is_word_chunk).collect();
  let words2: Vec<InlineChunk<'_>> = get_inline_chunks(text2).into_iter().filter(is_word_chunk).collect();

  let word_changes = diff(&words1, &words2, deadline)?;
  let word_changes = optimize_word_chunks(text1, text2, &words1, &words2, &word_changes);

  let delimiters_iterable = match_adjustment_delimiters(text1, text2, &words1, &words2, &word_changes, 0, 0, deadline)?;

  let line_blocks = LineUplifter::new(text1, text2, line_offsets1, line_offsets2).build(&delimiters_iterable);

  let sub_iterables = collect_line_block_sub_iterables(&delimiters_iterable, &line_blocks, line_offsets1, line_offsets2);

  let mut result = Vec::new();
  for (line_block, sub_iterable) in line_blocks.iter().zip(&sub_iterables) {
    let (sub_start1, sub_end1) = line_offsets1.lines_range(line_block.start1, line_block.end1, true);
    let (sub_start2, sub_end2) = line_offsets2.lines_range(line_block.start2, line_block.end2, true);

    let subtext1 = &text1[sub_start1..sub_end1];
    let subtext2 = &text2[sub_start2..sub_end2];

    let iterable = match_adjustment_whitespaces(subtext1, subtext2, sub_iterable, policy);

    let inner_fragments = convert_into_diff_fragments(&iterable);
    if inner_fragments.is_empty() {
      continue;
    }

    // workaround 'include newline' desync for the last modification in delta
    if line_block.start1 != line_block.end1 && line_block.start2 != line_block.end2 {
      if line_block.end1 == line_offsets1.line_count()
        && line_block.end2 != line_offsets2.line_count()
        && subtext2.last() == Some(&u16::from(b'\n'))
        && is_equal_texts(subtext1, &subtext2[..subtext2.len() - 1], policy)
      {
        continue;
      }
      if line_block.end2 == line_offsets2.line_count()
        && line_block.end1 != line_offsets1.line_count()
        && subtext1.last() == Some(&u16::from(b'\n'))
        && is_equal_texts(&subtext1[..subtext1.len() - 1], subtext2, policy)
      {
        continue;
      }
    }

    result.push(WordLineBlock {
      line_range: *line_block,
      fragments: inner_fragments,
    });
  }

  Ok(result)
}

fn collect_line_block_sub_iterables(iterable: &DiffIterable, line_blocks: &[Range], line_offsets1: &LineOffsets, line_offsets2: &LineOffsets) -> Vec<DiffIterable> {
  let changed = iterable.changes();

  let mut index = 0;

  let mut sub_iterables = Vec::with_capacity(line_blocks.len());
  for line_block in line_blocks {
    let (sub_start1, sub_end1) = line_offsets1.lines_range(line_block.start1, line_block.end1, true);
    let (sub_start2, sub_end2) = line_offsets2.lines_range(line_block.start2, line_block.end2, true);

    while index < changed.len() {
      let range = changed[index];
      if range.end1 < sub_start1 || range.end2 < sub_start2 {
        index += 1;
        continue;
      }

      break;
    }

    sub_iterables.push(fair(DiffIterable::subiterable(changed, Range::new(sub_start1, sub_end1, sub_start2, sub_end2), index)));
  }
  sub_iterables
}

/// Collects 'changed line blocks' that correspond to the given 'by-word differences' (or 'by-word + delimiters').
///
/// The result is 'good enough' and may not be optimal from the human's point of view. For example, it leaves the
/// 'unmatched empty lines' inside an unmatched block, when a human would match these empty lines, breaking the
/// changed block into two changed blocks. The correct answer in this case may depend on the context, so we can't
/// blindly match these as post-processing.
///
/// The offsets are signed: `-1` and the text length stand for an imaginary `\n` before and after the text.
struct LineUplifter<'a> {
  text1: &'a [u16],
  text2: &'a [u16],
  line_offsets1: &'a LineOffsets,
  line_offsets2: &'a LineOffsets,
  line_blocks: Vec<Range>,
  block_start1: usize,
  block_start2: usize,
}

impl<'a> LineUplifter<'a> {
  const fn new(text1: &'a [u16], text2: &'a [u16], line_offsets1: &'a LineOffsets, line_offsets2: &'a LineOffsets) -> Self {
    Self {
      text1,
      text2,
      line_offsets1,
      line_offsets2,
      line_blocks: Vec::new(),
      block_start1: 0,
      block_start2: 0,
    }
  }

  fn build(mut self, changes: &DiffIterable) -> Vec<Range> {
    self.execute(changes);
    self.line_blocks
  }

  fn execute(&mut self, changes: &DiffIterable) {
    let mut last_index1: isize = -1;
    let mut last_index2: isize = -1;

    // The possible offset ranges are [-1, textLength + 1) to indicate that walk has reached the text boundaries.
    // We have imaginary '\n' at '-1' and 'textLength' offsets.

    // Walk from 'unchanged' regions deeper into the 'changed' territory, searching for '\n' or text end.
    // For each walk we find the 'closest' and 'furthest' newline, aborting the walking at the first non-whitespace symbol.
    // We try to add two blocks - one with 'unchanged' fragment, and one that contains whitespace-only changes, if any.

    for range in changes.unchanged() {
      let (start1, start2) = (signed(range.start1), signed(range.start2));
      if let Some((first1, first2)) = self.walk_forward(last_index1, last_index2, start1, start2) {
        self.walk_backward(first1, first2, start1, start2);
      } else {
        self.walk_backward(last_index1, last_index2, start1, start2);
      }
      last_index1 = signed(range.end1);
      last_index2 = signed(range.end2);
    }

    let text_end1 = signed(self.text1.len()) + 1;
    let text_end2 = signed(self.text2.len()) + 1;
    if let Some((first1, first2)) = self.walk_forward(last_index1, last_index2, text_end1, text_end2) {
      self.walk_backward(first1, first2, text_end1, text_end2);
    } else {
      self.walk_backward(last_index1, last_index2, text_end1, text_end2);
    }
    self.mark_next_block(self.line_offsets1.line_count(), self.line_offsets2.line_count());
  }

  /// The offsets of the nearest block, if found
  fn walk_forward(&mut self, start1: isize, start2: isize, end1: isize, end2: isize) -> Option<(isize, isize)> {
    // match the current 'changed block' edge
    let found_first1 = walk_side_forward(self.text1, start1, end1)?;
    let found_first2 = walk_side_forward(self.text2, start2, end2)?;

    // match the paired empty lines
    let mut match1 = found_first1;
    let mut match2 = found_first2;
    while let (Some(next1), Some(next2)) = (walk_side_forward(self.text1, match1 + 1, end1), walk_side_forward(self.text2, match2 + 1, end2)) {
      match1 = next1;
      match2 = next2;
    }

    // do not match unpaired empty lines during the forward walk if they can be used by backward walk later
    if match1 == end1 - 1 || match2 == end2 - 1 {
      // match unpaired empty lines if we fully cover the area
      if match1 != end1 - 1 && char_at(self.text1, end1 - 1) == u16::from(b'\n') && are_all_lines_empty(self.text1, match1, end1) {
        match1 = end1 - 1;
      }
      if match2 != end2 - 1 && char_at(self.text2, end2 - 1) == u16::from(b'\n') && are_all_lines_empty(self.text2, match2, end2) {
        match2 = end2 - 1;
      }
    }

    self.mark_next_block_offset(found_first1, found_first2);
    self.mark_next_block_offset(match1, match2);
    Some((match1, match2))
  }

  fn walk_backward(&mut self, start1: isize, start2: isize, end1: isize, end2: isize) {
    // match the current 'changed block' edge
    let Some(found_first1) = walk_side_backward(self.text1, start1, end1) else {
      return;
    };
    let Some(found_first2) = walk_side_backward(self.text2, start2, end2) else {
      return;
    };

    // match all the remaining empty lines as a separate block
    let mut found_last1 = found_first1;
    while let Some(found) = walk_side_backward(self.text1, start1, found_last1) {
      found_last1 = found;
    }

    let mut found_last2 = found_first2;
    while let Some(found) = walk_side_backward(self.text2, start2, found_last2) {
      found_last2 = found;
    }

    self.mark_next_block_offset(found_last1, found_last2);
    self.mark_next_block_offset(found_first1, found_first2);
  }

  fn mark_next_block_offset(&mut self, block_end_offset1: isize, block_end_offset2: isize) {
    let block_end1 = get_block_end_line(block_end_offset1, self.line_offsets1);
    let block_end2 = get_block_end_line(block_end_offset2, self.line_offsets2);
    self.mark_next_block(block_end1, block_end2);
  }

  fn mark_next_block(&mut self, block_end1: usize, block_end2: usize) {
    if self.block_start1 == block_end1 && self.block_start2 == block_end2 {
      return;
    }

    self.line_blocks.push(Range::new(self.block_start1, block_end1, self.block_start2, block_end2));
    self.block_start1 = block_end1;
    self.block_start2 = block_end2;
  }
}

fn are_all_lines_empty(text: &[u16], start: isize, end: isize) -> bool {
  (start..end).all(|index| is_space_enter_or_tab(char_at(text, index)))
}

fn walk_side_forward(text: &[u16], start: isize, end: isize) -> Option<isize> {
  for index in start..end {
    let ch = char_at(text, index);
    if ch == u16::from(b'\n') {
      return Some(index);
    }
    if !is_space_enter_or_tab(ch) {
      return None;
    }
  }
  None
}

fn walk_side_backward(text: &[u16], start: isize, end: isize) -> Option<isize> {
  for index in (start..end).rev() {
    let ch = char_at(text, index);
    if ch == u16::from(b'\n') {
      return Some(index);
    }
    if !is_space_enter_or_tab(ch) {
      return None;
    }
  }
  None
}

/// The char at an offset, `\n` at `-1` and at the text length
fn char_at(text: &[u16], offset: isize) -> u16 {
  match usize::try_from(offset) {
    Ok(offset) if offset < text.len() => text[offset],
    _ => u16::from(b'\n'),
  }
}

fn get_block_end_line(offset: isize, line_offsets: &LineOffsets) -> usize {
  match usize::try_from(offset) {
    Err(_) => 0,
    Ok(offset) if offset == line_offsets.text_length() => line_offsets.line_count(),
    Ok(offset) => line_offsets.line_number(offset) + 1,
  }
}

#[expect(clippy::cast_possible_wrap, reason = "a text is shorter than isize::MAX")]
const fn signed(value: usize) -> isize {
  value as isize
}

//
// Impl
//

#[expect(clippy::too_many_arguments, reason = "the two texts, their words, the matching and the shifts of AdjustmentPunctuationMatcher")]
fn match_adjustment_delimiters(
  text1: &[u16],
  text2: &[u16],
  words1: &[InlineChunk<'_>],
  words2: &[InlineChunk<'_>],
  changes: &DiffIterable,
  start_shift1: usize,
  start_shift2: usize,
  deadline: &Deadline,
) -> Result<DiffIterable, DiffTooBig> {
  AdjustmentPunctuationMatcher {
    text1,
    text2,
    words1,
    words2,
    start_shift1,
    start_shift2,
    deadline,
    builder: ChangeBuilder::new(text1.len(), text2.len()),
    last: None,
  }
  .build(changes)
}

fn match_adjustment_whitespaces(text1: &[u16], text2: &[u16], iterable: &DiffIterable, policy: ComparisonPolicy) -> DiffIterable {
  match policy {
    ComparisonPolicy::Default => default_correct(iterable, text1, text2),
    ComparisonPolicy::TrimWhitespaces => {
      let default_iterable = default_correct(iterable, text1, text2);
      trim_spaces_correct(&default_iterable, text1, text2)
    }
    ComparisonPolicy::IgnoreWhitespaces => ignore_spaces_correct(iterable, text1, text2),
  }
}

/// The changed ranges as fragments
pub fn convert_into_diff_fragments(changes: &DiffIterable) -> Vec<DiffFragment> {
  changes.changes().iter().map(|ch| DiffFragment::new(ch.start1, ch.end1, ch.start2, ch.end2)).collect()
}

/// Compares one text with two others, as if they were one.
///
/// Returns two fair iterables: (0, len1) - (0, len21) and (0, len1) - (0, len22)
fn compare_punctuation_2_side(text1: &[u16], text21: &[u16], text22: &[u16], deadline: &Deadline) -> Result<(DiffIterable, DiffIterable), DiffTooBig> {
  let text2 = [text21, text22].concat();
  let changes = by_char::compare_punctuation(text1, &text2, deadline)?;

  let (ranges1, ranges2) = split_iterable_2_side(&changes, text21.len());

  let iterable1 = fair(create_unchanged(ranges1, text1.len(), text21.len()));
  let iterable2 = fair(create_unchanged(ranges2, text1.len(), text22.len()));
  Ok((iterable1, iterable2))
}

fn split_iterable_2_side(changes: &DiffIterable, offset: usize) -> (Vec<Range>, Vec<Range>) {
  let mut ranges1 = Vec::new();
  let mut ranges2 = Vec::new();
  for ch in changes.unchanged() {
    if ch.end2 <= offset {
      ranges1.push(Range::new(ch.start1, ch.end1, ch.start2, ch.end2));
    } else if ch.start2 >= offset {
      ranges2.push(Range::new(ch.start1, ch.end1, ch.start2 - offset, ch.end2 - offset));
    } else {
      let len2 = offset - ch.start2;
      ranges1.push(Range::new(ch.start1, ch.start1 + len2, ch.start2, offset));
      ranges2.push(Range::new(ch.start1 + len2, ch.end1, 0, ch.end2 - offset));
    }
  }
  (ranges1, ranges2)
}

pub const fn is_word_chunk(chunk: &InlineChunk<'_>) -> bool {
  matches!(chunk, InlineChunk::Word(_))
}

fn is_leading_trailing_space(text: &[u16], start: Option<usize>) -> bool {
  is_leading_space(text, start) || is_trailing_space(text, start)
}

fn is_leading_space(text: &[u16], start: Option<usize>) -> bool {
  let Some(start) = start else {
    return false;
  };
  if start == text.len() {
    return false;
  }
  if !is_space_enter_or_tab(text[start]) {
    return false;
  }

  for &c in text[..start].iter().rev() {
    if c == u16::from(b'\n') {
      return true;
    }
    if !is_space_enter_or_tab(c) {
      return false;
    }
  }
  true
}

fn is_trailing_space(text: &[u16], end: Option<usize>) -> bool {
  let Some(end) = end else {
    return false;
  };
  if end == text.len() {
    return false;
  }
  if !is_space_enter_or_tab(text[end]) {
    return false;
  }

  for &c in &text[end..] {
    if c == u16::from(b'\n') {
      return true;
    }
    if !is_space_enter_or_tab(c) {
      return false;
    }
  }
  true
}

//
// Misc
//

fn count_newlines(words: &[InlineChunk<'_>]) -> usize {
  words.iter().filter(|word| matches!(word, InlineChunk::Newline(_))).count()
}

/// The words and the newlines of a text: a word is a run of letters and digits, a char of a continuous script is a
/// word of its own, and punctuation and whitespace are no words
pub fn get_inline_chunks(text: &[u16]) -> Vec<InlineChunk<'_>> {
  let mut chunks = Vec::new();

  let len = text.len();
  let mut offset = 0;

  let mut word_start: Option<usize> = None;
  let mut word_hash = 0_i32;

  while offset < len {
    let ch = code_point_at(text, offset);
    let char_count = char_count(ch);

    let is_alpha = is_alpha(ch);
    let is_word_part = is_alpha && !is_continuous_script(ch);

    if is_word_part {
      if word_start.is_none() {
        word_start = Some(offset);
        word_hash = 0;
      }
      word_hash = word_hash.wrapping_mul(31).wrapping_add_unsigned(ch);
    } else {
      if let Some(start) = word_start.take() {
        chunks.push(InlineChunk::Word(WordChunk::new(text, start, offset, word_hash)));
      }

      if is_alpha {
        // continuous script
        chunks.push(InlineChunk::Word(WordChunk::new(text, offset, offset + char_count, ch.cast_signed())));
      } else if ch == u32::from(b'\n') {
        chunks.push(InlineChunk::Newline(NewlineChunk { offset1: offset }));
      }
    }

    offset += char_count;
  }

  if let Some(start) = word_start {
    chunks.push(InlineChunk::Word(WordChunk::new(text, start, len, word_hash)));
  }

  chunks
}

//
// Punctuation matching
//

/// Matches the punctuation between the matched words.
///
/// ```text
/// sample: "[ X { A ! B } Y ]" "( X ... Y )" will lead to comparison of 3 groups of separators
///      "["  vs "(",
///      "{" + "}" vs "..."
///      "]"  vs ")"
/// ```
struct AdjustmentPunctuationMatcher<'a, 'b> {
  text1: &'a [u16],
  text2: &'a [u16],
  words1: &'a [InlineChunk<'b>],
  words2: &'a [InlineChunk<'b>],
  start_shift1: usize,
  start_shift2: usize,
  deadline: &'a Deadline,
  builder: ChangeBuilder,
  /// `lastStart1`, `lastStart2`, `lastEnd1`, `lastEnd2` of the source; `None` for its `-1`
  last: Option<[usize; 4]>,
}

impl AdjustmentPunctuationMatcher<'_, '_> {
  fn build(mut self, changes: &DiffIterable) -> Result<DiffIterable, DiffTooBig> {
    self.execute(changes)?;
    Ok(fair(self.builder.finish()))
  }

  fn execute(&mut self, changes: &DiffIterable) -> Result<(), DiffTooBig> {
    self.last = None;

    self.match_forward(None, None);

    for ch in changes.unchanged() {
      let count = ch.end1 - ch.start1;
      for i in 0..count {
        let index1 = ch.start1 + i;
        let index2 = ch.start2 + i;

        let start1 = self.get_start_offset1(index1);
        let start2 = self.get_start_offset2(index2);
        let end1 = self.get_end_offset1(index1);
        let end2 = self.get_end_offset2(index2);

        self.match_backward(index1, index2)?;

        self.builder.mark_equal_range(start1, start2, end1, end2);

        self.match_forward(Some(index1), Some(index2));
      }
    }

    self.match_backward(self.words1.len(), self.words2.len())
  }

  fn match_backward(&mut self, index1: usize, index2: usize) -> Result<(), DiffTooBig> {
    let start1 = if index1 == 0 { 0 } else { self.get_end_offset1(index1 - 1) };
    let start2 = if index2 == 0 { 0 } else { self.get_end_offset2(index2 - 1) };
    let end1 = if index1 == self.words1.len() {
      self.text1.len()
    } else {
      self.get_start_offset1(index1)
    };
    let end2 = if index2 == self.words2.len() {
      self.text2.len()
    } else {
      self.get_start_offset2(index2)
    };

    self.match_backward_range(start1, start2, end1, end2)?;
    self.last = None;
    Ok(())
  }

  /// `None` for the source's `-1`, the position before the first word
  fn match_forward(&mut self, index1: Option<usize>, index2: Option<usize>) {
    let start1 = index1.map_or(0, |index| self.get_end_offset1(index));
    let start2 = index2.map_or(0, |index| self.get_end_offset2(index));
    let next1 = index1.map_or(0, |index| index + 1);
    let next2 = index2.map_or(0, |index| index + 1);
    let end1 = if next1 == self.words1.len() { self.text1.len() } else { self.get_start_offset1(next1) };
    let end2 = if next2 == self.words2.len() { self.text2.len() } else { self.get_start_offset2(next2) };

    debug_assert!(self.last.is_none());
    self.last = Some([start1, start2, end1, end2]);
  }

  fn match_backward_range(&mut self, start1: usize, start2: usize, end1: usize, end2: usize) -> Result<(), DiffTooBig> {
    let [last_start1, last_start2, last_end1, last_end2] = self.last.expect("a forward match before the backward one");

    if last_start1 == start1 && last_start2 == start2 {
      // pair of adjustment matched words, match gap between ("A B" - "A B")
      debug_assert!(last_end1 == end1 && last_end2 == end2);

      return self.match_range(start1, start2, end1, end2);
    }
    if last_start1 < start1 && last_start2 < start2 {
      // pair of matched words, with few unmatched ones between ("A X B" - "A Y B")
      debug_assert!(last_end1 <= start1 && last_end2 <= start2);

      self.match_range(last_start1, last_start2, last_end1, last_end2)?;
      return self.match_range(start1, start2, end1, end2);
    }

    // one side adjustment, and other has non-matched words between ("A B" - "A Y B")
    self.match_complex_range([last_start1, last_start2, last_end1, last_end2], [start1, start2, end1, end2])
  }

  fn match_range(&mut self, start1: usize, start2: usize, end1: usize, end2: usize) -> Result<(), DiffTooBig> {
    if start1 == end1 && start2 == end2 {
      return Ok(());
    }

    let sequence1 = &self.text1[start1..end1];
    let sequence2 = &self.text2[start2..end2];

    let changes = by_char::compare_punctuation(sequence1, sequence2, self.deadline)?;

    for ch in changes.unchanged() {
      self.builder.mark_equal_range(start1 + ch.start1, start2 + ch.start2, start1 + ch.end1, start2 + ch.end2);
    }
    Ok(())
  }

  fn match_complex_range(&mut self, [start11, start12, end11, end12]: [usize; 4], [start21, start22, end21, end22]: [usize; 4]) -> Result<(), DiffTooBig> {
    if start11 == start21 && end11 == end21 {
      self.match_complex_range_left(start11, end11, [start12, end12, start22, end22])
    } else if start12 == start22 && end12 == end22 {
      self.match_complex_range_right(start12, end12, [start11, end11, start21, end21])
    } else {
      unreachable!("one side of a complex range is shared")
    }
  }

  fn match_complex_range_left(&mut self, start1: usize, end1: usize, [start12, end12, start22, end22]: [usize; 4]) -> Result<(), DiffTooBig> {
    let sequence1 = &self.text1[start1..end1];
    let sequence21 = &self.text2[start12..end12];
    let sequence22 = &self.text2[start22..end22];

    let (first, second) = compare_punctuation_2_side(sequence1, sequence21, sequence22, self.deadline)?;

    for ch in first.unchanged() {
      self.builder.mark_equal_range(start1 + ch.start1, start12 + ch.start2, start1 + ch.end1, start12 + ch.end2);
    }
    for ch in second.unchanged() {
      self.builder.mark_equal_range(start1 + ch.start1, start22 + ch.start2, start1 + ch.end1, start22 + ch.end2);
    }
    Ok(())
  }

  fn match_complex_range_right(&mut self, start2: usize, end2: usize, [start11, end11, start21, end21]: [usize; 4]) -> Result<(), DiffTooBig> {
    let sequence11 = &self.text1[start11..end11];
    let sequence12 = &self.text1[start21..end21];
    let sequence2 = &self.text2[start2..end2];

    let (first, second) = compare_punctuation_2_side(sequence2, sequence11, sequence12, self.deadline)?;

    // Mirrored ch.*1 and ch.*2 as we use "compare2Side" that works with 2 right sides, while we have 2 left here
    for ch in first.unchanged() {
      self.builder.mark_equal_range(start11 + ch.start2, start2 + ch.start1, start11 + ch.end2, start2 + ch.end1);
    }
    for ch in second.unchanged() {
      self.builder.mark_equal_range(start21 + ch.start2, start2 + ch.start1, start21 + ch.end2, start2 + ch.end1);
    }
    Ok(())
  }

  const fn get_start_offset1(&self, index: usize) -> usize {
    self.words1[index].offset1() - self.start_shift1
  }

  const fn get_start_offset2(&self, index: usize) -> usize {
    self.words2[index].offset1() - self.start_shift2
  }

  const fn get_end_offset1(&self, index: usize) -> usize {
    self.words1[index].offset2() - self.start_shift1
  }

  const fn get_end_offset2(&self, index: usize) -> usize {
    self.words2[index].offset2() - self.start_shift2
  }
}

//
// Whitespaces matching
//

/// Shrinks each change by the equal whitespace at its ends (`DefaultCorrector`)
fn default_correct(iterable: &DiffIterable, text1: &[u16], text2: &[u16]) -> DiffIterable {
  let mut changes = Vec::new();
  for range in iterable.changes() {
    let end_cut = expand_whitespaces_backward(text1, text2, range.start1, range.start2, range.end1, range.end2);
    let start_cut = expand_whitespaces_forward(text1, text2, range.start1, range.start2, range.end1 - end_cut, range.end2 - end_cut);

    let expand = Range::new(range.start1 + start_cut, range.end1 - end_cut, range.start2 + start_cut, range.end2 - end_cut);

    if !expand.is_empty() {
      changes.push(expand);
    }
  }

  create(changes, text1.len(), text2.len())
}

/// Matches whitespace where it can and ignores it where it cannot (`IgnoreSpacesCorrector`)
fn ignore_spaces_correct(iterable: &DiffIterable, text1: &[u16], text2: &[u16]) -> DiffIterable {
  let mut changes = Vec::new();
  for range in iterable.changes() {
    // match spaces if we can, ignore them if we can't
    let expanded = expand_whitespaces(text1, text2, *range);
    let trimmed = trim_range(text1, text2, expanded);

    if !trimmed.is_empty() && !is_equals_ignore_whitespaces(text1, text2, trimmed) {
      changes.push(trimmed);
    }
  }

  create(changes, text1.len(), text2.len())
}

/// Drops the leading and trailing whitespace of the lines from the changes (`TrimSpacesCorrector`)
pub(crate) fn trim_spaces_correct(iterable: &DiffIterable, text1: &[u16], text2: &[u16]) -> DiffIterable {
  let mut changes = Vec::new();
  for range in iterable.changes() {
    let mut start1 = range.start1;
    let mut start2 = range.start2;
    let mut end1 = range.end1;
    let mut end2 = range.end2;

    if is_leading_trailing_space(text1, Some(start1)) {
      start1 = trim_start(text1, start1, end1);
    }
    if is_leading_trailing_space(text1, end1.checked_sub(1)) {
      end1 = trim_end(text1, start1, end1);
    }
    if is_leading_trailing_space(text2, Some(start2)) {
      start2 = trim_start(text2, start2, end2);
    }
    if is_leading_trailing_space(text2, end2.checked_sub(1)) {
      end2 = trim_end(text2, start2, end2);
    }

    let trimmed = Range::new(start1, end1, start2, end2);

    if !trimmed.is_empty() && !is_equals(text1, text2, trimmed) {
      changes.push(trimmed);
    }
  }

  create(changes, text1.len(), text2.len())
}

//
// Helpers
//

/// A word or a newline of a text
#[derive(Clone, Copy, Debug)]
pub enum InlineChunk<'a> {
  Word(WordChunk<'a>),
  Newline(NewlineChunk),
}

impl InlineChunk<'_> {
  /// The start offset in the text
  pub const fn offset1(&self) -> usize {
    match self {
      Self::Word(word) => word.offset1,
      Self::Newline(newline) => newline.offset1,
    }
  }

  /// The end offset in the text
  pub const fn offset2(&self) -> usize {
    match self {
      Self::Word(word) => word.offset2,
      Self::Newline(newline) => newline.offset1 + 1,
    }
  }
}

impl PartialEq for InlineChunk<'_> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Word(word1), Self::Word(word2)) => word1.hash == word2.hash && is_equals_policy(word1.content(), word2.content(), ComparisonPolicy::Default),
      (Self::Newline(_), Self::Newline(_)) => true,
      _ => false,
    }
  }
}

impl Eq for InlineChunk<'_> {}

impl Hash for InlineChunk<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    match self {
      Self::Word(word) => state.write_i32(word.hash),
      Self::Newline(_) => state.write_i32(0),
    }
  }
}

/// A word: `[offset1, offset2)` of its text
#[derive(Clone, Copy, Debug)]
pub struct WordChunk<'a> {
  text: &'a [u16],
  offset1: usize,
  offset2: usize,
  hash: i32,
}

impl<'a> WordChunk<'a> {
  const fn new(text: &'a [u16], offset1: usize, offset2: usize, hash: i32) -> Self {
    Self { text, offset1, offset2, hash }
  }

  pub fn content(&self) -> &'a [u16] {
    &self.text[self.offset1..self.offset2]
  }
}

/// A `\n` at `offset1`
#[derive(Clone, Copy, Debug)]
pub struct NewlineChunk {
  offset1: usize,
}

/// A block of lines that `compare_and_split` found, with the changes inside it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineBlock {
  /// The changes, relative to the start of the block
  pub fragments: Vec<DiffFragment>,
  pub offsets: Range,
  pub newlines1: usize,
  pub newlines2: usize,
}

/// A changed block of lines that `compare_words_first` found, with the changes inside it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordLineBlock {
  pub line_range: Range,
  pub fragments: Vec<DiffFragment>,
}
