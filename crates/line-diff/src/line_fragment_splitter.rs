// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/LineFragmentSplitter.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors).

//! Given the matching of the words, splits a block of lines into 'logically different' blocks of lines.

use crate::by_word::InlineChunk;
use crate::comparison_util::is_equals;
use crate::iterables::diff_iterable::DiffIterable;
use crate::policy::ComparisonPolicy;
use crate::range::Range;

/// The indexes here are a bit tricky: -1 is the beginning of the file, `words.len()` the end of it, everything in
/// between an `InlineChunk` (a word or a newline)
pub(crate) struct LineFragmentSplitter<'a, 'b> {
  text1: &'a [u16],
  text2: &'a [u16],
  words1: &'a [InlineChunk<'b>],
  words2: &'a [InlineChunk<'b>],
  iterable: &'a DiffIterable,
  result: Vec<WordBlock>,
  last1: isize,
  last2: isize,
  pending_chunk: Option<PendingChunk>,
}

impl<'a, 'b> LineFragmentSplitter<'a, 'b> {
  pub(crate) const fn new(text1: &'a [u16], text2: &'a [u16], words1: &'a [InlineChunk<'b>], words2: &'a [InlineChunk<'b>], iterable: &'a DiffIterable) -> Self {
    Self {
      text1,
      text2,
      words1,
      words2,
      iterable,
      result: Vec::new(),
      last1: -1,
      last2: -1,
      pending_chunk: None,
    }
  }

  pub(crate) fn run(mut self) -> Vec<WordBlock> {
    let mut has_equal_words = false;
    for range in self.iterable.unchanged() {
      let count = range.end1 - range.start1;
      for i in 0..count {
        let index1 = range.start1 + i;
        let index2 = range.start2 + i;

        if is_newline(self.words1, index1) && is_newline(self.words2, index2) {
          // split by matched newlines
          self.add_line_chunk(signed(index1), signed(index2), has_equal_words);
          has_equal_words = false;
        } else {
          if is_first_in_line(self.words1, index1) && is_first_in_line(self.words2, index2) {
            // split by matched first word in line
            self.add_line_chunk(signed(index1) - 1, signed(index2) - 1, has_equal_words);
          }
          // TODO: split by 'last word in line' + 'last word in whole sequence' ?
          has_equal_words = true;
        }
      }
    }
    self.add_line_chunk(signed(self.words1.len()), signed(self.words2.len()), has_equal_words);

    if let Some(pending_chunk) = self.pending_chunk.take() {
      self.result.push(pending_chunk.block);
    }

    self.result
  }

  fn add_line_chunk(&mut self, end1: isize, end2: isize, has_equal_words: bool) {
    if self.last1 > end1 || self.last2 > end2 {
      return;
    }

    let chunk = self.create_chunk(self.last1, self.last2, end1, end2, has_equal_words);
    if chunk.block.offsets.is_empty() {
      return;
    }

    self.pending_chunk = Some(match self.pending_chunk.take() {
      Some(pending_chunk) if should_merge_chunks(&pending_chunk, &chunk) => merge_chunks(&pending_chunk, &chunk),
      Some(pending_chunk) => {
        self.result.push(pending_chunk.block);
        chunk
      }
      None => chunk,
    });

    self.last1 = end1;
    self.last2 = end2;
  }

  fn create_chunk(&self, start1: isize, start2: isize, end1: isize, end2: isize, has_equal_words: bool) -> PendingChunk {
    let start_offset1 = get_offset(self.words1, self.text1, start1);
    let start_offset2 = get_offset(self.words2, self.text2, start2);
    let end_offset1 = get_offset(self.words1, self.text1, end1);
    let end_offset2 = get_offset(self.words2, self.text2, end2);

    let start1 = index(start1 + 1);
    let start2 = index(start2 + 1);
    let end1 = index(end1 + 1).min(self.words1.len());
    let end2 = index(end2 + 1).min(self.words2.len());

    let block = WordBlock {
      words: Range::new(start1, end1, start2, end2),
      offsets: Range::new(start_offset1, end_offset1, start_offset2, end_offset2),
    };

    PendingChunk {
      has_equal_words,
      has_words_inside: self.has_words_inside(&block),
      is_equal_ignore_whitespaces: self.is_equals_ignore_whitespace(&block),
      block,
    }
  }

  fn is_equals_ignore_whitespace(&self, block: &WordBlock) -> bool {
    let sequence1 = &self.text1[block.offsets.start1..block.offsets.end1];
    let sequence2 = &self.text2[block.offsets.start2..block.offsets.end2];
    is_equals(sequence1, sequence2, ComparisonPolicy::IgnoreWhitespaces)
  }

  fn has_words_inside(&self, block: &WordBlock) -> bool {
    self.words1[block.words.start1..block.words.end1]
      .iter()
      .any(|word| !matches!(word, InlineChunk::Newline(_)))
      || self.words2[block.words.start2..block.words.end2]
        .iter()
        .any(|word| !matches!(word, InlineChunk::Newline(_)))
  }
}

//
// Helpers
//

/// A block of lines: its words and its offsets
#[derive(Clone, Copy, Debug)]
pub(crate) struct WordBlock {
  pub(crate) words: Range,
  pub(crate) offsets: Range,
}

struct PendingChunk {
  block: WordBlock,
  has_equal_words: bool,
  has_words_inside: bool,
  is_equal_ignore_whitespaces: bool,
}

const fn should_merge_chunks(chunk1: &PendingChunk, chunk2: &PendingChunk) -> bool {
  if !chunk1.has_equal_words && !chunk2.has_equal_words {
    // combine lines, that matched only by '\n'
    return true;
  }

  if chunk1.is_equal_ignore_whitespaces && chunk2.is_equal_ignore_whitespaces {
    // combine whitespace-only changed lines
    return true;
  }

  if !chunk1.has_words_inside || !chunk2.has_words_inside {
    // squash block without words in it
    return true;
  }

  false
}

const fn merge_chunks(chunk1: &PendingChunk, chunk2: &PendingChunk) -> PendingChunk {
  let block1 = chunk1.block;
  let block2 = chunk2.block;
  let new_block = WordBlock {
    words: Range::new(block1.words.start1, block2.words.end1, block1.words.start2, block2.words.end2),
    offsets: Range::new(block1.offsets.start1, block2.offsets.end1, block1.offsets.start2, block2.offsets.end2),
  };
  PendingChunk {
    block: new_block,
    has_equal_words: chunk1.has_equal_words || chunk2.has_equal_words,
    has_words_inside: chunk1.has_words_inside || chunk2.has_words_inside,
    is_equal_ignore_whitespaces: chunk1.is_equal_ignore_whitespaces && chunk2.is_equal_ignore_whitespaces,
  }
}

fn get_offset(words: &[InlineChunk<'_>], text: &[u16], index: isize) -> usize {
  if index == -1 {
    return 0;
  }
  let index = self::index(index);
  if index == words.len() {
    return text.len();
  }
  let chunk = words[index];
  debug_assert!(matches!(chunk, InlineChunk::Newline(_)));
  chunk.offset2()
}

const fn is_newline(words: &[InlineChunk<'_>], index: usize) -> bool {
  matches!(words[index], InlineChunk::Newline(_))
}

const fn is_first_in_line(words: &[InlineChunk<'_>], index: usize) -> bool {
  if index == 0 {
    return true;
  }
  matches!(words[index - 1], InlineChunk::Newline(_))
}

#[expect(clippy::cast_possible_wrap, reason = "a text is shorter than isize::MAX")]
const fn signed(value: usize) -> isize {
  value as isize
}

#[expect(clippy::cast_sign_loss, reason = "the splitter indexes only with values it proved non-negative")]
const fn index(value: isize) -> usize {
  debug_assert!(value >= 0);
  value as usize
}
