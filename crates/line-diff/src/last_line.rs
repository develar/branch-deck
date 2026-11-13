//! The last line of a text: the empty line after a final line end (or the one line of an empty text), or a line without
//! an end. Branch Deck's own. IntelliJ compares it as any line of a document and its line end as equal, so the empty
//! last line can pair with a blank line of the other text, and a line without an end with one that has an end; git has
//! no empty last line, and a line without an end is another line than the same line with one. `"x\n"` to `"x\n\n"`
//! then adds the new empty last line, which no count takes, instead of the blank line, and a merge puts an insertion at
//! the end of a side after the empty last line of the base, where git puts it before. The comparison of the views
//! (`provider::compare_for_view`) pairs the empty last line only with its like (`with_empty_last_lines`) and keeps a
//! change of the final line end alone a change of that line; the merge (`merge_model::merge_texts`) pairs each last
//! line only with its like (`with_last_lines`), as git does.

use crate::iterables::diff_iterable::DiffIterable;
use crate::iterables::diff_iterable_util::{create, fair};
use crate::line_offsets::LineOffsets;
use crate::range::Range;

/// The number of lines of a text without its empty last line
pub fn content_lines(offsets: &LineOffsets) -> usize {
  let last = offsets.line_count() - 1;
  if offsets.line_start(last) == offsets.line_end(last) { last } else { last + 1 }
}

/// `iterable`, a comparison of the lines of two texts without their empty last lines, as a comparison of all their
/// `length1` and `length2` lines: the two empty last lines pair with each other; when only one text has one, it is a
/// change, the end of the change that reaches the ends of both compared parts or a change of its own
pub fn with_empty_last_lines(iterable: &DiffIterable, length1: usize, length2: usize) -> DiffIterable {
  let pair = length1 - iterable.length1() == length2 - iterable.length2();
  with_tail(iterable, length1, length2, pair)
}

/// `iterable`, a comparison of the lines of two texts without their last lines, as a comparison of all their `length1`
/// and `length2` lines: the two last lines pair when `last_lines_pair` (the empty last lines of both, or two equal lines
/// without an end), else they are a change, the end of the change that reaches the ends of both compared parts or a
/// change of its own
pub fn with_last_lines(iterable: &DiffIterable, length1: usize, length2: usize, last_lines_pair: bool) -> DiffIterable {
  debug_assert!(iterable.length1() + 1 == length1 && iterable.length2() + 1 == length2);
  with_tail(iterable, length1, length2, last_lines_pair)
}

/// `iterable` with the lines after its compared parts up to `length1` and `length2`: unchanged when they `pair`, else a
/// change, joined to the change that reaches the ends of both compared parts
fn with_tail(iterable: &DiffIterable, length1: usize, length2: usize, pair: bool) -> DiffIterable {
  let (content1, content2) = (iterable.length1(), iterable.length2());
  let mut changes = iterable.changes().to_vec();
  if !pair {
    match changes.last_mut() {
      Some(last) if last.end1 == content1 && last.end2 == content2 => (last.end1, last.end2) = (length1, length2),
      _ => changes.push(Range::new(content1, length1, content2, length2)),
    }
  }
  fair(create(changes, length1, length2))
}
