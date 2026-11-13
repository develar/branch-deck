//! The mark of a free reorder: a declaration that a change put in another order among the declarations of its owner,
//! its text the same apart from its layout, where no reader of the code sees the order (`index::Order::Free`, as the
//! index answers it: `MovesAnswer::reordered`). Its old lines and its new lines are one declaration at another place:
//! a view labels both ends, as it labels a move, and reads nothing of the old lines where they left. A declaration
//! whose order matters (an enum entry, a statement, a decorator) has no such mark: where it stands is behaviour.

use crate::index::Reordered;
use std::ops::Range;

/// A declaration that moved among its own where its order is free: its old lines and its new lines, 0-based, the end
/// exclusive, and its name
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReorderMark {
  pub name: String,
  pub old_lines: Range<u32>,
  pub new_lines: Range<u32>,
  /// Why its order is free, in the words of the index
  pub why: String,
}

impl ReorderMark {
  /// The mark of a declaration the index says was reordered, from its places in the two trees (lines from 1, both
  /// ends included); `None` where its order matters, or a place names no line
  pub fn of(reordered: &Reordered) -> Option<Self> {
    if !reordered.order.is_free() {
      return None;
    }
    let lines = |line: u32, end_line: u32| (line >= 1 && end_line >= line).then(|| line - 1..end_line);
    Some(Self {
      name: reordered.new.name.clone(),
      old_lines: lines(reordered.old.line, reordered.old.end_line)?,
      new_lines: lines(reordered.new.line, reordered.new.end_line)?,
      why: reordered.why.clone(),
    })
  }
}
