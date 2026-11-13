// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/WordMergeComparisonUtilTest.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2016 JetBrains s.r.o. and contributors).

use crate::comparison_merge_util_test_base::words;

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port

#[test]
fn simple() {
  words(|t| {
    t.text("", "", "");
    t.default_matching("", "", "");
    t.test_all();
  });

  words(|t| {
    t.text("", "X", "");
    t.default_matching("", "-", "");
    t.test_all();
  });

  words(|t| {
    t.text("X", "", "");
    t.default_matching("-", "", "");
    t.test_all();
  });

  words(|t| {
    t.text("a b", "a b", "a b");
    t.default_matching("   ", "   ", "   ");
    t.test_all();
  });

  words(|t| {
    t.text("A b c", "a b c", "a b C");
    t.default_matching("-   -", "-   -", "-   -");
    t.test_all();
  });

  words(|t| {
    t.text("a c", "a c", "a X c");
    t.default_matching("   ", "   ", " --  ");
    t.ignore_matching("   ", "   ", "  -  ");
    t.test_all();
  });

  words(|t| {
    t.text("a X c", "a X c", "a c");
    t.default_matching(" --  ", " --  ", "   ");
    t.ignore_matching("  -  ", "  -  ", "   ");
    t.test_all();
  });

  words(|t| {
    t.text("a X c", "a c", "a Y c");
    t.default_matching(" --  ", "   ", " --  ");
    t.ignore_matching("  -  ", "   ", "  -  ");
    t.test_all();
  });

  words(|t| {
    t.text("a c", "a X c", "a Y c");
    t.default_matching("   ", " --  ", " --  ");
    t.ignore_matching("   ", "  -  ", "  -  ");
    t.test_all();
  });
}

#[test]
fn newlines() {
  words(|t| {
    t.text("i", "i_", "_i");
    t.default_matching("-", "--", "--"); // TODO
    t.trim_matching(" ", "  ", "  ");
    t.test_all();
  });

  words(|t| {
    t.text("_i", "i_", "i");
    t.default_matching("--", "--", "-");
    t.trim_matching("  ", "  ", " ");
    t.test_all();
  });

  words(|t| {
    t.text("i", "_i", "i_");
    t.default_matching(" ", "- ", " -");
    t.trim_matching(" ", "  ", "  ");
    t.test_all();
  });

  words(|t| {
    t.text("_i", "i", "i_");
    t.default_matching("- ", " ", " -");
    t.trim_matching("  ", " ", "  ");
    t.test_all();
  });
}

// jscpd:ignore-end
