//! Go to Declaration and Find Usages in Java from its PSI, beside `navigate_test.rs`

use crate::highlight::Grammar;
use crate::navigate::{Position, ReferenceKind, reference_at};

#[test]
fn the_name_of_a_parameter_used_in_the_body_goes_to_the_parameter() {
  let text = "class A {\n  int f(int size) { return size + 1; }\n}\n";
  let reference = reference_at(Grammar::Java, text, Position { line: 1, column: 29 }).unwrap();
  assert_eq!((reference.name.as_str(), reference.kind), ("size", ReferenceKind::Value));
  assert_eq!(reference.local, Some(Position { line: 1, column: 12 }));
}
