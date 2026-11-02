//! The summaries of Java files from their PSI, beside the golden files of `testdata/java/` (`summary_test.rs`)

use crate::extract::summarize;
use crate::language::Language;
use crate::test_support::fixtures;

#[test]
fn every_java_fixture_is_read() {
  for (name, source) in fixtures("java", "java") {
    assert!(!summarize(Language::Java, &source).truncated, "{name}");
  }
}
