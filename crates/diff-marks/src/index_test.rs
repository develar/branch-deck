//! The answers of the index as accio writes them: a `moves` of revision 1, without `reordered`, and one of revision 2,
//! with the order of each declaration it reordered, and the mark of a move whose declarations were reordered freely.

use crate::index::{MovesAnswer, Order};
use crate::reorder::ReorderMark;
use pretty_assertions::assert_eq;

const MOVED: &str = r#"{"file": "b.kt", "line": 3, "end_line": 5, "column": 4, "end_column": 7, "kind": "function", "name": "run", "language": "kotlin"}"#;

#[test]
fn a_moves_answer_without_reordered_reads_as_no_reorder() {
  let json = format!(r#"{{"moved": [{{"old": {MOVED}, "new": {MOVED}, "file": true, "owner": false}}], "total": 1, "files": 2, "complete": true, "quality": "exact"}}"#);
  let answer: MovesAnswer = serde_json::from_str(&json).unwrap();
  assert_eq!((answer.moved.len(), answer.total, answer.complete, answer.unavailable), (1, 1, true, false));
  assert!(answer.moved[0].file && !answer.moved[0].owner);
  assert_eq!(answer.moved[0].new.column, Some(4));
  assert_eq!(answer.reordered, []);
}

/// The orders accio writes, a later word read as `Unknown`; and the mark of a move by the declarations it holds: one
/// of free order within both ends is the mark, named by them in the order of their new lines; a declaration whose
/// order matters, one that reaches past an end, one of another file and a move that holds none are no mark
#[test]
fn a_move_of_declarations_whose_order_is_free_is_a_mark_and_one_that_holds_an_order_that_matters_is_none() {
  let place =
    |name: &str, line: u32, end_line: u32| format!(r#"{{"file": "a.kt", "line": {line}, "end_line": {end_line}, "kind": "function", "name": "{name}", "language": "kotlin"}}"#);
  let reordered = |name: &str, old: [u32; 2], new: [u32; 2], order: &str| {
    format!(
      r#"{{"old": {}, "new": {}, "order": "{order}", "why": "a member of a class"}}"#,
      place(name, old[0], old[1]),
      place(name, new[0], new[1])
    )
  };
  let answer =
    |entries: &[String]| -> MovesAnswer { serde_json::from_str(&format!(r#"{{"moved": [], "reordered": [{}], "total": 0, "complete": true}}"#, entries.join(", "))).unwrap() };
  let orders = answer(&[
    reordered("run", [2, 4], [10, 12], "free"),
    reordered("run", [2, 4], [10, 12], "matters"),
    reordered("run", [2, 4], [10, 12], "later"),
  ]);
  let orders: Vec<Order> = orders.reordered.iter().map(|it| it.order).collect();
  assert_eq!(orders, [Order::Free, Order::Matters, Order::Unknown]);

  // `run` and `stop` moved together from lines 2 to 8 to lines 9 to 15, a blank line between them
  let both = answer(&[reordered("stop", [6, 8], [13, 15], "free"), reordered("run", [2, 4], [9, 11], "free")]);
  let mark = |answer: &MovesAnswer, path: &str| ReorderMark::of_move(path, path, 1..8, 8..15, &answer.reordered);
  assert_eq!(
    mark(&both, "a.kt"),
    Some(ReorderMark {
      names: vec!["run".to_string(), "stop".to_string()],
      old_start: 1,
      old_len: 7,
      new_start: 8,
      new_len: 7,
    })
  );
  assert_eq!(mark(&both, "b.kt"), None);
  let matters = answer(&[reordered("stop", [6, 8], [13, 15], "matters"), reordered("run", [2, 4], [9, 11], "free")]);
  assert_eq!(mark(&matters, "a.kt"), None);
  let past = answer(&[reordered("stop", [6, 9], [13, 16], "free"), reordered("run", [2, 4], [9, 11], "free")]);
  assert_eq!(mark(&past, "a.kt"), None);
  let elsewhere = answer(&[reordered("far", [30, 32], [40, 42], "free")]);
  assert_eq!(mark(&elsewhere, "a.kt"), None);
}
