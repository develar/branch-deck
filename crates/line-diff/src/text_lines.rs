//! The lines of a text as the diffs count them, and the lines of git among them.
//!
//! A line of the diffs ends at `\n`, `\r\n` or a lone `\r` (`apply::split_lines`, `text::SourceText`): the blocks, the
//! hunks, the marks and the tokens name such lines. A line of git ends at `\n` alone, and so do the lines of a syntax
//! summary (`code_symbols::summary`: its declarations and its import statements). In a file with a lone `\r` the two
//! counts differ, and the lines of one read by the other are other lines: whoever sets a line of a summary beside a
//! block translates it first (`TextLines::of_git`), the diffs of the views (`file_diffs::service::side_facts`) and
//! the units of a review plan (`review_plan::units`) alike.

use std::ops::Range;

/// The lines of a text as the diffs count them: a line ends at `\n`, `\r\n` or a lone `\r`
/// (`apply::split_lines`), so the text has one more line than line ends
pub struct TextLines<'t> {
  /// Each line without its line end
  lines: Vec<&'t str>,
  /// The lines a lone `\r` ends: line ends of the diffs, none of git
  lone_cr: Vec<u32>,
}

impl<'t> TextLines<'t> {
  #[must_use]
  pub fn of(text: &'t str) -> Self {
    let (mut lines, mut lone_cr) = (Vec::new(), Vec::new());
    let mut rest = text;
    while let Some(end) = rest.find(['\n', '\r']) {
      let line_end = if rest[end..].starts_with("\r\n") {
        2
      } else {
        if rest[end..].starts_with('\r') {
          lone_cr.push(u32::try_from(lines.len()).unwrap_or(u32::MAX));
        }
        1
      };
      lines.push(&rest[..end]);
      rest = &rest[end + line_end..];
    }
    lines.push(rest);
    Self { lines, lone_cr }
  }

  /// Every line
  #[must_use]
  pub fn all(&self) -> &[&'t str] {
    &self.lines
  }

  /// The lines `lines` (from 0, the end excluded), as far as the text has them
  #[must_use]
  pub fn text(&self, lines: &Range<u32>) -> &[&'t str] {
    let (start, end) = ((lines.start as usize).min(self.lines.len()), (lines.end as usize).min(self.lines.len()));
    &self.lines[start..end.max(start)]
  }

  /// The lines that are `lines` as git counts them (a line ends at `\n` alone; from 0, the end excluded): the same
  /// lines unless a lone `\r` ends one
  #[must_use]
  pub fn of_git(&self, lines: Range<u32>) -> Range<u32> {
    // the line that starts the line `line` of git: each lone `\r` before it ends one more line here
    let at = |line: u32| {
      let mut view = line;
      for &ended in &self.lone_cr {
        if ended >= view {
          break;
        }
        view += 1;
      }
      view
    };
    let start = at(lines.start);
    start..at(lines.end).max(start)
  }

  /// The lines from `first` to `last` of git as a syntax summary names them (from 1, both included), named the same
  /// way among the lines of the diffs
  #[must_use]
  pub fn of_summary(&self, first: u32, last: u32) -> (u32, u32) {
    let range = self.of_git(first.saturating_sub(1)..last);
    (range.start + 1, range.end)
  }
}
