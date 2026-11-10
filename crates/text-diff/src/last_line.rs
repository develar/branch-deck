//! The empty last line of a text: the line after a final line end, and the one line of an empty text. Branch Deck's
//! own. IntelliJ compares it as any line of a document, so it can pair with a blank line of the other text, and git
//! has no such line: `"x\n"` to `"x\n\n"` then adds the new empty last line, which no count takes, instead of the blank
//! line, and a merge puts an insertion at the end of a side after the empty last line of the base, where git puts it
//! before. The comparisons of the views (`provider::compare_for_view`) and of the merge (`merge_model::merge_texts`)
//! compare the lines without it and pair it only with its like.

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
  let (content1, content2) = (iterable.length1(), iterable.length2());
  let mut changes = iterable.changes().to_vec();
  if length1 - content1 != length2 - content2 {
    match changes.last_mut() {
      Some(last) if last.end1 == content1 && last.end2 == content2 => (last.end1, last.end2) = (length1, length2),
      _ => changes.push(Range::new(content1, length1, content2, length2)),
    }
  }
  fair(create(changes, length1, length2))
}
