//! The highlighting of Java from its PSI, beside the golden files of `testdata/highlight/` (`highlight_test.rs`)

use crate::highlight::{Grammar, Role, highlight};

#[test]
fn a_keyword_is_a_keyword() {
  let runs = highlight(Grammar::Java, "class A {}\n").unwrap();
  assert_eq!(runs.line(0)[..2], [0, Role::Keyword.style()]);
}
