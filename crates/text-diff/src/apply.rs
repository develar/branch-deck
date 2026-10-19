//! The blocks of a diff applied to the bytes of a text, and the ids that name a block across the views and the command
//! line. Branch Deck's own: `bd move-hunk` moves the blocks the views show.
//!
//! Lines are the engine's (`SourceText`): `\n`, `\r\n` and a lone `\r` end a line, and a text has one line more than it
//! has line ends. Each line keeps its own end. The end of a line belongs to the line, so a block that reaches the end of
//! the old text also decides whether the line before it ends: an added final newline is a block on the empty last line.

use crate::model::DiffBlock;
use sha1::{Digest, Sha1};
use std::fmt::{Display, Formatter, Write as _};
use std::ops::Range;

/// A block of lines: 0-based, the end exclusive, as `DiffBlock` holds them
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineBlock {
  pub old_start: usize,
  pub old_end: usize,
  pub new_start: usize,
  pub new_end: usize,
}

impl LineBlock {
  /// The same block from the new text to the old one
  #[must_use]
  pub const fn reversed(self) -> Self {
    Self {
      old_start: self.new_start,
      old_end: self.new_end,
      new_start: self.old_start,
      new_end: self.old_end,
    }
  }

  const fn old_lines(&self) -> Range<usize> {
    self.old_start..self.old_end
  }

  const fn new_lines(&self) -> Range<usize> {
    self.new_start..self.new_end
  }
}

impl From<&DiffBlock> for LineBlock {
  fn from(block: &DiffBlock) -> Self {
    let index = |line: u32| line as usize;
    Self {
      old_start: index(block.old_start),
      old_end: index(block.old_end),
      new_start: index(block.new_start),
      new_end: index(block.new_end),
    }
  }
}

/// Why blocks do not apply
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyError {
  /// A block lies outside its text, or two blocks overlap
  OutOfRange(LineBlock),
}

impl Display for ApplyError {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    let Self::OutOfRange(block) = self;
    write!(formatter, "the block {} does not fit the file", BlockId::range_text(block))
  }
}

impl std::error::Error for ApplyError {}

/// The lines of a text, each with its line end; the last one has none and may be empty
pub fn split_lines(text: &[u8]) -> Vec<&[u8]> {
  let mut lines = Vec::new();
  let mut start = 0;
  let mut index = 0;
  while index < text.len() {
    match text[index] {
      b'\r' if text.get(index + 1) == Some(&b'\n') => {
        index += 1;
        lines.push(&text[start..=index]);
        start = index + 1;
      }
      b'\n' | b'\r' => {
        lines.push(&text[start..=index]);
        start = index + 1;
      }
      _ => {}
    }
    index += 1;
  }
  lines.push(&text[start..]);
  lines
}

/// The content of a line without its line end
fn content(line: &[u8]) -> &[u8] {
  let line = line.strip_suffix(b"\n").unwrap_or(line);
  line.strip_suffix(b"\r").unwrap_or(line)
}

/// The line end of a line, empty for the last line
fn end(line: &[u8]) -> &[u8] {
  &line[content(line).len()..]
}

/// `old` with the lines of `blocks` replaced by the lines of `new` they name: the change of those blocks applied, the
/// rest of `old` kept. The blocks come from a diff of `old` and `new` (`diff_texts`), in any order, without overlap
pub fn apply_blocks(old: &[u8], new: &[u8], blocks: &[LineBlock]) -> Result<Vec<u8>, ApplyError> {
  let old_lines = split_lines(old);
  let new_lines = split_lines(new);
  let mut sorted = blocks.to_vec();
  sorted.sort_by_key(|block| (block.old_start, block.old_end));

  // (content, line end) of each line of the result
  let mut result: Vec<(&[u8], &[u8])> = Vec::with_capacity(old_lines.len());
  let mut next_old = 0;
  for block in &sorted {
    if block.old_start < next_old || block.old_end > old_lines.len() || block.new_end > new_lines.len() || block.old_start > block.old_end || block.new_start > block.new_end {
      return Err(ApplyError::OutOfRange(*block));
    }
    result.extend(old_lines[next_old..block.old_start].iter().map(|line| (content(line), end(line))));
    if block.old_end == old_lines.len()
      && block.new_start > 0
      && let Some(before) = result.last_mut()
    {
      // the block reaches the end of the old text: the line before it ends as in the new text
      before.1 = end(new_lines[block.new_start - 1]);
    }
    result.extend(new_lines[block.new_lines()].iter().map(|line| (content(line), end(line))));
    next_old = block.old_end;
  }
  result.extend(old_lines[next_old..].iter().map(|line| (content(line), end(line))));

  let mut bytes = Vec::with_capacity(old.len().max(new.len()));
  for (index, (content, end)) in result.iter().enumerate() {
    bytes.extend_from_slice(content);
    // a lone `\r` before an empty line that ends with `\n` would read as one `\r\n`: the line ends with `\r\n` instead
    let joins_next = *end == b"\r" && result.get(index + 1).is_some_and(|(next, next_end)| next.is_empty() && next_end.first() == Some(&b'\n'));
    bytes.extend_from_slice(if joins_next { b"\r\n" } else { end });
  }
  Ok(bytes)
}

/// The id of a block: its lines as git names them (1-based starts, an empty side names the line before) and a hash of
/// its old and new lines, so that an id of a block that changed since fails. `12,3:12,5@1a2b3c4d`. The hash is of the
/// texts as the views have them: UTF-8, an invalid sequence replaced with U+FFFD
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlockId {
  pub block: LineBlock,
  /// 8 hex digits of the SHA-1 of the old lines, a NUL and the new lines, with their line ends
  pub hash: String,
}

impl BlockId {
  /// The id of `block` of the diff of `old` and `new`
  pub fn of(old: &[u8], new: &[u8], block: LineBlock) -> Self {
    Self::of_lines(&split_lines(old), &split_lines(new), block)
  }

  /// `of` for texts split once (`split_lines`), for the many blocks of one file
  pub fn of_lines(old_lines: &[&[u8]], new_lines: &[&[u8]], block: LineBlock) -> Self {
    Self {
      block,
      hash: block_hash(old_lines, new_lines, block),
    }
  }

  /// Parses `12,3:12,5@1a2b3c4d`
  pub fn parse(id: &str) -> Option<Self> {
    let (range, hash) = id.split_once('@')?;
    let (old, new) = range.split_once(':')?;
    let side = |side: &str| -> Option<Range<usize>> {
      let (start, count) = side.split_once(',')?;
      let (start, count): (usize, usize) = (start.parse().ok()?, count.parse().ok()?);
      let start = if count == 0 { start } else { start.checked_sub(1)? };
      Some(start..start + count)
    };
    let (old, new) = (side(old)?, side(new)?);
    (hash.len() == 8 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| Self {
      block: LineBlock {
        old_start: old.start,
        old_end: old.end,
        new_start: new.start,
        new_end: new.end,
      },
      hash: hash.to_ascii_lowercase(),
    })
  }

  /// `12,3:12,5`: the lines without the hash
  fn range_text(block: &LineBlock) -> String {
    let side = |range: Range<usize>| {
      let count = range.len();
      format!("{},{count}", if count == 0 { range.start } else { range.start + 1 })
    };
    format!("{}:{}", side(block.old_lines()), side(block.new_lines()))
  }
}

impl Display for BlockId {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
    write!(formatter, "{}@{}", Self::range_text(&self.block), self.hash)
  }
}

fn block_hash(old_lines: &[&[u8]], new_lines: &[&[u8]], block: LineBlock) -> String {
  let mut hasher = Sha1::new();
  for line in old_lines.get(block.old_lines()).unwrap_or_default() {
    hasher.update(line);
  }
  hasher.update([0]);
  for line in new_lines.get(block.new_lines()).unwrap_or_default() {
    hasher.update(line);
  }
  hasher.finalize().iter().take(4).fold(String::with_capacity(8), |mut hex, byte| {
    let _ = write!(hex, "{byte:02x}");
    hex
  })
}
