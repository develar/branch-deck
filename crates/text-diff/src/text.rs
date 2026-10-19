//! A text as UTF-16 code units with its lines. Branch Deck's own: the comparison works on UTF-16, as the source does,
//! so the offsets of a result match the ported tests and reach JavaScript unchanged.

use crate::line_offsets::LineOffsets;
use serde::{Deserialize, Serialize};

/// The UTF-16 code units of a text and the offsets of its lines
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Utf16Text {
  units: Vec<u16>,
  line_offsets: LineOffsets,
}

impl Utf16Text {
  pub fn new(text: &str) -> Self {
    Self::from_units(text.encode_utf16().collect())
  }

  pub fn from_units(units: Vec<u16>) -> Self {
    let line_offsets = LineOffsets::new(&units);
    Self { units, line_offsets }
  }

  pub fn units(&self) -> &[u16] {
    &self.units
  }

  pub const fn line_offsets(&self) -> &LineOffsets {
    &self.line_offsets
  }

  pub const fn len(&self) -> usize {
    self.units.len()
  }

  pub const fn is_empty(&self) -> bool {
    self.units.is_empty()
  }
}

impl From<&str> for Utf16Text {
  fn from(text: &str) -> Self {
    Self::new(text)
  }
}

/// The end of one line
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineEnd {
  Lf,
  Crlf,
  /// A `\r` without a `\n` after it
  Cr,
}

/// The line ends of a whole file
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LineEndKind {
  Lf,
  Crlf,
  Cr,
  /// More than one kind
  Mixed,
}

/// A file as the comparison sees it: its lines as IntelliJ splits them, and the line ends they had.
///
/// `\n`, `\r\n` and a lone `\r` each end a line. The comparison runs on the text with each line end as `\n`, as
/// IntelliJ compares a document, which never holds a `\r`, so line ends compare as equal; `separator` tells them apart
/// for the whole file. A text has one line more than it has line ends: `"a\n"` is `a` and an empty line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceText {
  text: Utf16Text,
  /// The end of each line but the last, which has none
  line_ends: Vec<LineEnd>,
}

impl SourceText {
  pub fn new(source: &str) -> Self {
    let (cr, lf) = (u16::from(b'\r'), u16::from(b'\n'));
    let mut units: Vec<u16> = Vec::with_capacity(source.len());
    let mut line_ends = Vec::new();
    let mut after_cr = false;
    for unit in source.encode_utf16() {
      if unit == lf && after_cr {
        // the `\n` of a `\r\n`: the `\r` ended the line already
        if let Some(end) = line_ends.last_mut() {
          *end = LineEnd::Crlf;
        }
      } else if unit == lf || unit == cr {
        units.push(lf);
        line_ends.push(if unit == cr { LineEnd::Cr } else { LineEnd::Lf });
      } else {
        units.push(unit);
      }
      after_cr = unit == cr;
    }
    Self {
      text: Utf16Text::from_units(units),
      line_ends,
    }
  }

  /// The text with each line end as `\n`
  pub const fn text(&self) -> &Utf16Text {
    &self.text
  }

  /// The number of lines: one more than the line ends
  pub const fn line_count(&self) -> usize {
    self.line_ends.len() + 1
  }

  /// The lines that hold text or end with a line end: the empty line after a final line end, and the one line of an
  /// empty text, do not count. `text` holds every line end as `\n`, a lone `\r` too, so the last unit tells
  pub fn counted_lines(&self) -> usize {
    if self.text.units().last().is_none_or(|unit| *unit == u16::from(b'\n')) {
      self.line_count() - 1
    } else {
      self.line_count()
    }
  }

  /// Whether the text ends with a line end: `\n`, `\r\n` or a lone `\r`, each a `\n` in `text`
  pub fn ends_with_line_end(&self) -> bool {
    self.text.units().last() == Some(&u16::from(b'\n'))
  }

  /// The line ends of the file; `None` for a file without a line end
  pub fn separator(&self) -> Option<LineEndKind> {
    let first = *self.line_ends.first()?;
    Some(if self.line_ends.iter().all(|end| *end == first) {
      match first {
        LineEnd::Lf => LineEndKind::Lf,
        LineEnd::Crlf => LineEndKind::Crlf,
        LineEnd::Cr => LineEndKind::Cr,
      }
    } else {
      LineEndKind::Mixed
    })
  }
}
