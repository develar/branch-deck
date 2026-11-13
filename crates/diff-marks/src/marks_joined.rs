//! The blocks the line diff cut out of one change of formatting, read as one (`marks.rs`;
//! openspec/specs/commit/quiet-changes/spec.md, "Runs: Blocks the comparison cut out of one change are read as one").
//!
//! The comparison of lines anchors on a line that is the same text on both sides, though it is two lines of the code:
//! of a body indented again with its lambda wrapped, the old `    }` that ends the function is the new `    }` that
//! ends the lambda, and the one change is two blocks, the first without the `}` on its new side, the second the `}`
//! the function ends with now. Neither is the same tokens by itself; the two with the line between them are.
//!
//! 1. **Which blocks**: blocks in a row that each have a start after their own reading (`read_blocks`), with the lines
//!    between them that the comparison holds equal, at most `MAX_JOINED_BLOCKS` of them. A block the policy cut off
//!    ends the row.
//! 2. **The question before the tokens**: the lines from the first block to the last, those between them too, have
//!    the same characters but for their blanks on both sides (`code_tokens::same_without_blanks`), first by their
//!    count, from the counts of each block and of each row of lines between two. Every such region that its tokens
//!    prove answers yes.
//! 3. **The proof**: the region is read as one block (`runs_of_block`), and its runs are cut at the ends of its blocks:
//!    a part of a run in each block it crosses, with lines on one side alone where the block has no other. The region
//!    is taken only when those parts leave no line of any of its blocks to read: its every changed line stands in a
//!    run whose tokens are the same on both sides, and what else is in the run the comparison holds equal. Else its
//!    blocks keep the runs of their own readings.
//!
//! From each block on, the smallest region that is proven is taken, and the next region starts after it. The tokens of
//! a region count against `MAX_BLOCK_TOKENS`, its comparison against the work of the file, as those of a block.

use crate::marks::{MAX_BLOCK_TOKENS, SideText};
use crate::marks_runs::{Asked, BlockLines, runs_of_block};
use code_tokens::{Tokens, is_blank, same_without_blanks};
use line_diff::model::{BlockKind, DiffBlock, LineRun, starts_of};
use std::ops::Range;
use std::sync::Arc;

/// The most blocks one region holds (`read_joined`): a change the comparison cut at a line that stands twice is cut
/// into a few blocks, one more for each such line
pub const MAX_JOINED_BLOCKS: usize = 8;

/// The runs of `blocks`, each block's of its own reading in `runs`, with every region of the blocks the comparison cut
/// out of one change read as one, the parts of its runs in its blocks in place of theirs. `old` and `new` are the
/// texts, `tokens` their tokens, asked only for a region whose characters answer yes, `work` what is left of the work
/// of the file
pub(crate) fn read_joined(
  blocks: &[DiffBlock],
  [old, new]: [&SideText<'_>; 2],
  runs: &mut [Vec<LineRun>],
  asked: &Asked<'_>,
  tokens: &mut dyn FnMut() -> Option<[Arc<Tokens>; 2]>,
  work: &mut u64,
) {
  // the blocks a reader stops in after their own reading, with a character that is no blank, that the policy does
  // not cut off: lines blank on both sides are never marked
  let open: Vec<bool> = blocks
    .iter()
    .zip(runs.iter())
    .map(|(block, runs)| {
      block.kind != BlockKind::Ignored
        && !starts_of(std::slice::from_ref(block), runs, &[]).is_empty()
        && ((asked.has_text)(false, block.old_start..block.old_end) || (asked.has_text)(true, block.new_start..block.new_end))
    })
    .collect();
  if !open.windows(2).any(|pair| pair[0] && pair[1]) {
    return;
  }
  let solids = |side: &SideText<'_>, lines: Range<u32>| side.lines(lines).chars().filter(|character| !is_blank(*character)).count();
  // the characters that are no blanks of each block in a row with another, and of the lines after it up to the
  // next block of the row, on each side
  let in_row = |index: usize| open[index] && ((index > 0 && open[index - 1]) || open.get(index + 1) == Some(&true));
  let counts: Vec<[usize; 4]> = (0..blocks.len())
    .map(|index| {
      if !in_row(index) {
        return [0; 4];
      }
      let block = &blocks[index];
      let (old_gap, new_gap) = match blocks.get(index + 1).filter(|_| open[index + 1]) {
        Some(next) => (block.old_end..next.old_start, block.new_end..next.new_start),
        None => (0..0, 0..0),
      };
      [
        solids(old, block.old_start..block.old_end),
        solids(new, block.new_start..block.new_end),
        solids(old, old_gap),
        solids(new, new_gap),
      ]
    })
    .collect();

  let mut first = 0;
  while first < blocks.len() {
    if !open[first] {
      first += 1;
      continue;
    }
    // the solid characters of each side from the first block to the last one asked
    let (mut old_count, mut new_count) = (counts[first][0], counts[first][1]);
    let mut taken = None;
    for last in first + 1..blocks.len().min(first + MAX_JOINED_BLOCKS) {
      if !open[last] {
        break;
      }
      old_count += counts[last - 1][2] + counts[last][0];
      new_count += counts[last - 1][3] + counts[last][1];
      if old_count == 0 || old_count != new_count {
        continue;
      }
      let region = BlockLines {
        old: blocks[first].old_start..blocks[last].old_end,
        new: blocks[first].new_start..blocks[last].new_end,
      };
      if !same_without_blanks([old.lines(region.old.clone())], [new.lines(region.new.clone())]) {
        continue;
      }
      let Some(read) = tokens() else { return };
      if let Some(parts) = proven_region(&read, &blocks[first..=last], &region, asked, work) {
        taken = Some((last, parts));
        break;
      }
    }
    match taken {
      Some((last, parts)) => {
        for (runs, parts) in runs[first..=last].iter_mut().zip(parts) {
          *runs = parts;
        }
        first = last + 1;
      }
      None => first += 1,
    }
  }
}

/// The runs of `blocks`, a row of blocks the region `region` spans, read as one block and cut at the ends of each:
/// `None` when they leave a line of a block to read, or the region is over a limit of work
fn proven_region(tokens: &[Arc<Tokens>; 2], blocks: &[DiffBlock], region: &BlockLines, asked: &Asked<'_>, work: &mut u64) -> Option<Vec<Vec<LineRun>>> {
  let [old, new] = tokens;
  if old.places_in_lines(region.old.clone()).len() > MAX_BLOCK_TOKENS || new.places_in_lines(region.new.clone()).len() > MAX_BLOCK_TOKENS {
    return None;
  }
  // what a region that is not taken spent stays spent: the work of the file bounds the tries too
  let found = runs_of_block(old, new, region, asked, work, &mut |_, _| {})?;
  let parts: Vec<Vec<LineRun>> = blocks
    .iter()
    .map(|block| found.iter().filter(|run| run.quiet).filter_map(|run| part_in(run, block)).collect())
    .collect();
  let passed = blocks
    .iter()
    .zip(&parts)
    .all(|(block, parts)| starts_of(std::slice::from_ref(block), parts, &[]).is_empty());
  passed.then_some(parts)
}

/// The part of `run` in `block`: its lines on each side that are lines of the block, `None` when it has none. A side
/// the run has no line of in the block stands at the place of the block where the run passes it
fn part_in(run: &LineRun, block: &DiffBlock) -> Option<LineRun> {
  let within = |start: u32, end: u32, from: u32, to: u32| {
    let start = start.clamp(from, to);
    (start, end.clamp(start, to))
  };
  let (old_start, old_end) = within(run.old_start, run.old_end, block.old_start, block.old_end);
  let (new_start, new_end) = within(run.new_start, run.new_end, block.new_start, block.new_end);
  (old_start < old_end || new_start < new_end).then(|| LineRun {
    old_start,
    old_end,
    new_start,
    new_end,
    ..run.clone()
  })
}
