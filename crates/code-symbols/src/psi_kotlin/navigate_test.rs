//! Go to Declaration and Find Usages in Kotlin from its PSI, beside `navigate_test.rs`

use crate::highlight::Grammar;
use crate::navigate::{Position, ReferenceKind, reference_at, usages_of};

#[test]
fn the_name_of_a_parameter_used_in_the_body_goes_to_the_parameter() {
  let text = "fun f(size: Int) = size + 1\n";
  let reference = reference_at(Grammar::Kotlin, text, Position { line: 0, column: 19 }).unwrap();
  assert_eq!((reference.name.as_str(), reference.kind), ("size", ReferenceKind::Value));
  assert_eq!(reference.local, Some(Position { line: 0, column: 6 }));
  assert!(usages_of(Grammar::Kotlin, text, "size").is_empty(), "a parameter is a local");
}
