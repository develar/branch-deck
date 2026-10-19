//! The blocks of a diff applied to the bytes of a text, the result of a merge composed on the bytes of its three texts,
//! and the ids that name a block across the views and the command line. Branch Deck's own: `bd move-hunk` moves the
//! blocks the views show, and the sync commits a merge that its words resolve.
//!
//! Lines are the engine's (`SourceText`): `\n`, `\r\n` and a lone `\r` end a line, and a text has one line more than it
//! has line ends. Each line keeps its own end. The end of a line belongs to the line, so a block that reaches the end of
//! the old text also decides whether the line before it ends: an added final newline is a block on the empty last line.

use crate::merge_model::{MergeBlock, MergeBlockKind, MergeResolution, TextMerge};
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
  Ok(joined(&result, old.len().max(new.len())))
}

/// The bytes of lines given as (content, line end)
fn joined(lines: &[(&[u8], &[u8])], capacity: usize) -> Vec<u8> {
  let mut bytes = Vec::with_capacity(capacity);
  for (index, (content, end)) in lines.iter().enumerate() {
    bytes.extend_from_slice(content);
    // a lone `\r` before an empty line that ends with `\n` would read as one `\r\n`: the line ends with `\r\n` instead
    let joins_next = *end == b"\r" && lines.get(index + 1).is_some_and(|(next, next_end)| next.is_empty() && next_end.first() == Some(&b'\n'));
    bytes.extend_from_slice(if joins_next { b"\r\n" } else { end });
  }
  bytes
}

/// The result of a merge as bytes (`compose_merge`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposedMerge {
  pub bytes: Vec<u8>,
  /// Every block resolves, and every line has the end that the sides give it: the result is what IntelliJ's merge
  /// viewer writes after it applies every change, and a commit can take it. False when a conflict does not resolve (its
  /// base is kept), or when the sides disagree on the end of a line that the merge takes as equal
  pub exact: bool,
}

/// The lines of a result as (content, line end)
type ResultLines<'a> = Vec<(&'a [u8], &'a [u8])>;

/// The result of `merge` (`merge_texts` of these three texts) as IntelliJ's merge viewer builds it: the base, with each
/// block replaced by the lines of the side that changed it or by the text of its resolve by words, and with the base of
/// a conflict that does not resolve. The lines keep their own ends; the lines of a resolve by words end as the lines of
/// their block end. The texts must be the ones that were merged, so that their lines match the blocks
pub fn compose_merge<'a>(left: &'a [u8], base: &'a [u8], right: &'a [u8], merge: &'a TextMerge) -> ComposedMerge {
  let sides = [split_lines(left), split_lines(base), split_lines(right)];
  let base_count = sides[1].len();
  let mut exact = true;
  let mut result: ResultLines<'a> = Vec::with_capacity(base_count);
  let mut next = [0, 0, 0];
  for block in &merge.blocks {
    let starts = [block.left_start, block.base_start, block.right_start].map(|line| line as usize);
    let ends = [block.left_end, block.base_end, block.right_end].map(|line| line as usize);
    // a block that reaches the end of the base decides how the line before it ends
    exact &= push_equal_lines(&mut result, &sides, next, starts[1] - next[1], ends[1] == base_count);
    if block.kind == MergeBlockKind::Conflict && block.resolution.is_none() {
      exact = false;
    }
    let at_end = ends[1] == base_count;
    match block_source(block) {
      BlockSource::Side(side) => {
        if block.left_changed && block.right_changed && line_ends(&sides[0][starts[0]..ends[0]]) != line_ends(&sides[2][starts[2]..ends[2]]) {
          // both sides made the same change, with other line ends: the left one would drop the ends of the right one
          exact = false;
        }
        if at_end
          && starts[side] > 0
          && let Some(before) = result.last_mut()
        {
          // the block reaches the end of the base: the line before it ends as in the side
          before.1 = end(sides[side][starts[side] - 1]);
        }
        result.extend(sides[side][starts[side]..ends[side]].iter().map(|line| (content(line), end(line))));
      }
      BlockSource::Text(text) => {
        let Some(separator) = resolved_separator(&sides, starts, ends) else {
          // the lines of the block end in more than one way: which end a resolved line takes is not known
          exact = false;
          result.extend(sides[1][starts[1]..ends[1]].iter().map(|line| (content(line), end(line))));
          next = ends;
          continue;
        };
        if at_end {
          if text.is_empty() {
            // nothing is left of the end of the texts: which line ends the result is not known
            exact = false;
          }
          // the line before the block ends as in the sides
          if let Some(before) = result.last_mut()
            && starts[0] > 0
            && starts[2] > 0
          {
            let (left_end, right_end) = (end(sides[0][starts[0] - 1]), end(sides[2][starts[2] - 1]));
            exact &= left_end == right_end;
            before.1 = left_end;
          }
        }
        let mut lines = text.as_bytes().split(|&b| b == b'\n').peekable();
        while let Some(line) = lines.next() {
          let last = lines.peek().is_none();
          result.push((line, if last && at_end { &b""[..] } else { separator }));
        }
      }
    }
    next = ends;
  }
  exact &= push_equal_lines(&mut result, &sides, next, base_count - next[1], false);
  ComposedMerge {
    bytes: joined(&result, base.len().max(left.len()).max(right.len())),
    exact,
  }
}

/// The `count` lines from `starts` that the merge takes as equal on the three sides, from the base; false when a side
/// ends one of them otherwise, which the merge ignores. With `last_end_open`, the block after the lines decides the end
/// of the last one
fn push_equal_lines<'a>(result: &mut ResultLines<'a>, sides: &[Vec<&'a [u8]>; 3], starts: [usize; 3], count: usize, last_end_open: bool) -> bool {
  let mut same_ends = true;
  for offset in 0..count {
    let base_line = sides[1][starts[1] + offset];
    let compared = !(last_end_open && offset + 1 == count);
    if compared && (end(sides[0][starts[0] + offset]) != end(base_line) || end(sides[2][starts[2] + offset]) != end(base_line)) {
      same_ends = false;
    }
    result.push((content(base_line), end(base_line)));
  }
  same_ends
}

/// Where the result of a block takes its lines from, as `resultSourceOf` of the view says
enum BlockSource<'a> {
  /// 0 left, 1 base, 2 right
  Side(usize),
  Text(&'a str),
}

fn block_source(block: &MergeBlock) -> BlockSource<'_> {
  if block.resolution == Some(MergeResolution::Text)
    && let Some(text) = &block.resolved_text
  {
    return BlockSource::Text(text);
  }
  if block.resolution.is_none() && block.left_changed && block.right_changed {
    return BlockSource::Side(1);
  }
  // a side changed alone, or both changed the same way
  BlockSource::Side(if block.left_changed {
    0
  } else if block.right_changed {
    2
  } else {
    1
  })
}

fn line_ends<'a>(lines: &[&'a [u8]]) -> Vec<&'a [u8]> {
  lines.iter().map(|line| end(line)).collect()
}

/// The one line end of the lines of a block on the three sides, which the lines of its resolve by words take; `None`
/// when they end in more than one way. Lines without an end (the last line of each text) take none: `\n` stands for
/// the end that no line of the result then gets
fn resolved_separator<'a>(sides: &[Vec<&'a [u8]>; 3], starts: [usize; 3], ends: [usize; 3]) -> Option<&'a [u8]> {
  let mut separator: Option<&'a [u8]> = None;
  for side in 0..3 {
    for line in &sides[side][starts[side]..ends[side]] {
      let line_end = end(line);
      if line_end.is_empty() {
        continue;
      }
      match separator {
        None => separator = Some(line_end),
        Some(known) if known != line_end => return None,
        Some(_) => {}
      }
    }
  }
  Some(separator.unwrap_or(b"\n"))
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
