//! The result of a merge composed on the bytes of its three texts (`compose_merge`): what IntelliJ's merge viewer
//! writes after it applies every change, which the sync commits when the words of the merge resolve it. Branch Deck's
//! own, over the lines of a text with their ends of `line_diff::apply`.

use crate::merge_model::{MergeBlock, MergeBlockKind, MergeResolution, TextMerge};
use line_diff::apply::{content, end, joined, split_lines};

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
          if text.is_empty() && !result.is_empty() {
            // nothing is left of the end of the texts: which line ends the result is not known. Without a line before
            // the block, the result is empty
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
