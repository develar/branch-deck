//! The mark of a free reorder (openspec/specs/commit/quiet-changes/spec.md, "Subject: A move of declarations whose
//! order is free says so"): a move within a file whose lines are declarations the index says a change put in another
//! order among the declarations of their owner, their text the same apart from their layout, each where no reader of
//! the code sees the order (`index::Order::Free`, as the index answers it: `MovesAnswer::reordered`). A label, as
//! every move is: its lines are read where they stand now, and its notes say the order is free. A declaration whose
//! order matters (an enum entry, a value whose initializer runs in its turn, a decorated method) makes no such mark
//! of the move that holds it: where it stands is behaviour.

use crate::index::{DeclaredAt, Reordered};
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// A move within a file whose declarations were reordered where their order is free: the lines of both its ends
/// (0-based, as the move names them: a move of the pair by its lines without the blank lines beside them, a move of
/// a declaration by the lines of the declaration at each end), and the names of those declarations in the order of
/// their new lines
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderMark {
  pub names: Vec<String>,
  pub old_start: u32,
  pub old_len: u32,
  pub new_start: u32,
  pub new_len: u32,
}

/// The lines of a declaration as the index names them (from 1, both ends included), 0-based with the end exclusive;
/// `None` for a place that names no line
pub fn lines_of(at: &DeclaredAt) -> Option<Range<u32>> {
  (at.line >= 1 && at.end_line >= at.line).then(|| at.line - 1..at.end_line)
}

impl ReorderMark {
  /// The mark of the move of the lines `old` of the file at `old_path` to its lines `new` at `new_path`, of the
  /// declarations of `reordered` (the answer of the index between the two trees): every declaration the index
  /// reordered in that file whose lines meet an end of the move stands within both ends and its order is free, and
  /// one does at least. `None` else: the move holds a declaration whose order matters, one that reaches past it, or
  /// none the index reordered
  pub fn of_move(old_path: &str, new_path: &str, old: Range<u32>, new: Range<u32>, reordered: &[Reordered]) -> Option<Self> {
    let meets = |lines: &Range<u32>, end: &Range<u32>| lines.start < end.end && end.start < lines.end;
    let within = |lines: &Range<u32>, end: &Range<u32>| end.start <= lines.start && lines.end <= end.end;
    let mut held: Vec<(u32, &str)> = Vec::new();
    for reordered in reordered.iter().filter(|it| it.old.file == old_path && it.new.file == new_path) {
      let (Some(was), Some(is)) = (lines_of(&reordered.old), lines_of(&reordered.new)) else {
        continue;
      };
      if !meets(&was, &old) && !meets(&is, &new) {
        continue;
      }
      if !reordered.order.is_free() || !within(&was, &old) || !within(&is, &new) {
        return None;
      }
      held.push((is.start, reordered.new.name.as_str()));
    }
    if held.is_empty() {
      return None;
    }
    held.sort_unstable();
    Some(Self {
      names: held.into_iter().map(|(_, name)| name.to_string()).collect(),
      old_start: old.start,
      old_len: old.end - old.start,
      new_start: new.start,
      new_len: new.end - new.start,
    })
  }
}
