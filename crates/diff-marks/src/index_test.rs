//! The answers of the index as accio writes them: a `moves` of revision 1, without `reordered`, and one of revision 2,
//! with the order of each declaration it reordered, and the mark of a free reorder.

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

#[test]
fn a_free_reorder_is_a_mark_and_one_whose_order_matters_is_none() {
  let place = |line: u32, end_line: u32| format!(r#"{{"file": "a.kt", "line": {line}, "end_line": {end_line}, "kind": "function", "name": "run", "language": "kotlin"}}"#);
  let reordered = |order: &str| format!(r#"{{"old": {}, "new": {}, "order": "{order}", "why": "a member of a class"}}"#, place(2, 4), place(10, 12));
  let json = format!(
    r#"{{"moved": [], "reordered": [{}, {}, {}], "total": 0, "complete": true}}"#,
    reordered("free"),
    reordered("matters"),
    reordered("later")
  );
  let answer: MovesAnswer = serde_json::from_str(&json).unwrap();
  let orders: Vec<Order> = answer.reordered.iter().map(|it| it.order).collect();
  assert_eq!(orders, [Order::Free, Order::Matters, Order::Unknown]);
  let marks: Vec<Option<ReorderMark>> = answer.reordered.iter().map(ReorderMark::of).collect();
  assert_eq!(
    marks,
    [
      Some(ReorderMark {
        name: "run".to_string(),
        old_lines: 1..4,
        new_lines: 9..12,
        why: "a member of a class".to_string(),
      }),
      None,
      None,
    ]
  );
}
