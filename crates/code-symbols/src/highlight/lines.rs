//! The runs of the styles of each line, in the format of the view: the lines as `buildLineTable` of
//! `src/lib/diff/line-table.ts` splits them (`\n`, `\r\n`, a lone `\r`; the empty line after a final line end is a line),
//! offsets in UTF-16 code units, a pair (offset, style) where the style changes, none while it is the default foreground.

/// The highlighting of a text: the runs of every line
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineRuns {
  /// Where the runs of each line start, in pairs: `line_count + 1` entries
  pub line_starts: Vec<u32>,
  /// Pairs of a UTF-16 offset in the line and a style
  pub runs: Vec<u32>,
}

impl LineRuns {
  pub(crate) fn builder(text: &str) -> Builder<'_> {
    Builder {
      bytes: text.as_bytes(),
      pos: 0,
      column: 0,
      line_style: 0,
      out: Self {
        line_starts: vec![0],
        runs: Vec::with_capacity(text.len() / 8),
      },
    }
  }

  pub const fn line_count(&self) -> usize {
    self.line_starts.len() - 1
  }

  /// The runs of a line: pairs of (offset, style)
  pub fn line(&self, line: usize) -> &[u32] {
    let start = self.line_starts[line] as usize * 2;
    let end = self.line_starts[line + 1] as usize * 2;
    &self.runs[start..end]
  }
}

/// Takes the painted spans in the order of the text and writes the runs of their lines
pub(crate) struct Builder<'a> {
  bytes: &'a [u8],
  pos: usize,
  /// The UTF-16 offset of `pos` in its line
  column: u32,
  /// The style of the last run of the line, the default foreground at its start
  line_style: u32,
  out: LineRuns,
}

impl Builder<'_> {
  /// The text up to byte `end` has `style`
  pub(crate) fn push(&mut self, end: usize, style: u32) {
    let end = end.min(self.bytes.len());
    while self.pos < end {
      let byte = self.bytes[self.pos];
      if byte == b'\n' || byte == b'\r' {
        self.pos += if byte == b'\r' && self.bytes.get(self.pos + 1) == Some(&b'\n') { 2 } else { 1 };
        self.out.line_starts.push(self.run_count());
        self.column = 0;
        self.line_style = 0;
        continue;
      }
      if style != self.line_style {
        self.out.runs.push(self.column);
        self.out.runs.push(style);
        self.line_style = style;
      }
      // the UTF-16 length of the character that starts here: a 4-byte sequence is a surrogate pair
      self.column += if byte >= 0xf0 { 2 } else { 1 };
      self.pos += 1;
      while self.pos < end && self.bytes[self.pos] & 0xc0 == 0x80 {
        self.pos += 1;
      }
    }
  }

  pub(crate) fn finish(mut self) -> LineRuns {
    let end = self.bytes.len();
    self.push(end, 0);
    let count = self.run_count();
    self.out.line_starts.push(count);
    self.out
  }

  fn run_count(&self) -> u32 {
    u32::try_from(self.out.runs.len() / 2).unwrap_or(u32::MAX)
  }
}
