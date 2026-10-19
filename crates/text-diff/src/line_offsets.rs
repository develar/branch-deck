// Ported and modified from IntelliJ Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and
// contributors): `community/platform/util/diff/src/com/intellij/diff/tools/util/text/LineOffsets.kt`,
// `LineOffsetsImpl.kt` and `getLinesRange` of `community/platform/util/diff/src/com/intellij/diff/util/DiffRangeUtil.kt`.

//! The start and end offsets of the lines of a text. Only `\n` ends a line, as in the source.

/// The lines of a text: a text has one more line than it has `\n` chars
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineOffsets {
  line_ends: Vec<usize>,
  text_length: usize,
}

impl LineOffsets {
  pub fn new(text: &[u16]) -> Self {
    let mut line_ends: Vec<usize> = text.iter().enumerate().filter(|(_, unit)| **unit == u16::from(b'\n')).map(|(index, _)| index).collect();
    line_ends.push(text.len());
    Self {
      line_ends,
      text_length: text.len(),
    }
  }

  pub fn line_start(&self, line: usize) -> usize {
    if line == 0 { 0 } else { self.line_ends[line - 1] + 1 }
  }

  /// The end of a line without its `\n`
  pub fn line_end(&self, line: usize) -> usize {
    self.line_ends[line]
  }

  /// The end of a line, after its `\n` when `include_newline` is set and the line has one
  pub fn line_end_with(&self, line: usize, include_newline: bool) -> usize {
    self.line_ends[line] + usize::from(include_newline && line != self.line_ends.len() - 1)
  }

  /// The line of an offset; an offset of a `\n` belongs to the line it ends
  pub fn line_number(&self, offset: usize) -> usize {
    assert!(offset <= self.text_length, "Wrong offset: {offset}. Available text length: {}", self.text_length);
    if offset == 0 {
      return 0;
    }
    if offset == self.text_length {
      return self.line_count() - 1;
    }
    match self.line_ends.binary_search(&offset) {
      Ok(line) | Err(line) => line,
    }
  }

  pub const fn line_count(&self) -> usize {
    self.line_ends.len()
  }

  pub const fn text_length(&self) -> usize {
    self.text_length
  }

  /// The offsets of lines `[line1, line2)`, with the last `\n` when `include_newline` is set
  pub fn lines_range(&self, line1: usize, line2: usize, include_newline: bool) -> (usize, usize) {
    if line1 == line2 {
      let line_start_offset = if line1 < self.line_count() { self.line_start(line1) } else { self.text_length };
      (line_start_offset, line_start_offset)
    } else {
      let start_offset = self.line_start(line1);
      let mut end_offset = self.line_end(line2 - 1);
      if include_newline && end_offset < self.text_length {
        end_offset += 1;
      }
      (start_offset, end_offset)
    }
  }
}
