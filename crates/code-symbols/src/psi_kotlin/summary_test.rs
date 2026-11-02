//! The summaries of Kotlin files from their PSI, beside the golden files of `testdata/kotlin/` (`summary_test.rs`)

use crate::extract::summarize;
use crate::language::Language;
use crate::test_support::fixtures;

#[test]
fn every_kotlin_fixture_is_read() {
  for (name, source) in fixtures("kotlin", "kt") {
    assert!(!summarize(Language::Kotlin, &source).truncated, "{name}");
  }
}
