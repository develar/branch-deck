//! The highlighting of Kotlin from its PSI, beside the golden files of `testdata/highlight/` (`highlight_test.rs`)

use crate::highlight::{Grammar, Role, highlight};

#[test]
fn a_keyword_is_a_keyword_in_a_file_and_in_a_script() {
  for grammar in [Grammar::Kotlin, Grammar::KotlinScript] {
    let runs = highlight(grammar, "val x = 1\n").unwrap_or_else(|| panic!("{grammar:?}"));
    assert_eq!(runs.line(0)[..2], [0, Role::Keyword.style()], "{grammar:?}");
  }
}
