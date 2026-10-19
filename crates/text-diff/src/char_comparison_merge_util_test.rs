// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/CharComparisonMergeUtilTest.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2015 JetBrains s.r.o. and contributors).

use crate::comparison_merge_util_test_base::{chars, modify};

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port

#[test]
fn simple_cases() {
  chars(|t| {
    t.text("", "", "");
    t.matching("", "", "");
    t.changes(&[]);
    t.test();
  });

  chars(|t| {
    t.text("xyz", "xyz", "xyz");
    t.matching("   ", "   ", "   ");
    t.changes(&[]);
    t.test();
  });

  chars(|t| {
    t.text("", "", "x");
    t.matching("", "", "-");
    t.changes(&[modify(0, 0, 0, 0, 0, 1)]);
    t.test();
  });

  chars(|t| {
    t.text("x", "yx", "x");
    t.matching(" ", "- ", " ");
    t.test();
  });

  chars(|t| {
    t.text("x", "xy", "x");
    t.matching(" ", " -", " ");
    t.test();
  });

  chars(|t| {
    t.text("xyz", "xYz", "xyz");
    t.matching(" - ", " - ", " - ");
    t.test();
  });

  chars(|t| {
    t.text("xyz", "XyZ", "xyz");
    t.matching("- -", "- -", "- -");
    t.test();
  });
}

#[test]
fn conflict_t_ypes() {
  chars(|t| {
    t.text("abcd", "abcd", "abXcd");
    t.matching("    ", "    ", "  -  ");
    t.test();
  });

  chars(|t| {
    t.text("abXcd", "abcd", "abXcd");
    t.matching("  -  ", "    ", "  -  ");
    t.test();
  });

  chars(|t| {
    t.text("abcd", "abXcd", "abXcd");
    t.matching("    ", "  -  ", "  -  ");
    t.test();
  });

  chars(|t| {
    t.text("abcd", "abXcd", "abcd");
    t.matching("    ", "  -  ", "    ");
    t.test();
  });

  chars(|t| {
    t.text("abcd", "abXcd", "abYcd");
    t.matching("    ", "  -  ", "  -  ");
    t.test();
  });

  chars(|t| {
    t.text("abXcd", "abXcd", "abYcd");
    t.matching("  -  ", "  -  ", "  -  ");
    t.test();
  });

  chars(|t| {
    t.text("abYcd", "abXcd", "abYcd");
    t.matching("  -  ", "  -  ", "  -  ");
    t.test();
  });

  chars(|t| {
    t.text("abYcd", "abXcd", "abZcd");
    t.matching("  -  ", "  -  ", "  -  ");
    t.test();
  });

  chars(|t| {
    t.text("abXcd", "abcd", "abYcd");
    t.matching("  -  ", "    ", "  -  ");
    t.test();
  });
}

#[test]
fn boundary_conflicts() {
  chars(|t| {
    t.text("abcd", "abcd", "abcdx");
    t.matching("    ", "    ", "    -");
    t.test();
  });

  chars(|t| {
    t.text("abcd", "abcdx", "abcdx");
    t.matching("    ", "    -", "    -");
    t.test();
  });

  chars(|t| {
    t.text("abcd", "abcdx", "abcdy");
    t.matching("    ", "    -", "    -");
    t.test();
  });

  chars(|t| {
    t.text("abcdz", "abcdx", "abcdy");
    t.matching("    -", "    -", "    -");
    t.test();
  });

  chars(|t| {
    t.text("xabcd", "abcd", "abcd");
    t.matching("-    ", "    ", "    ");
    t.test();
  });

  chars(|t| {
    t.text("xabcd", "yabcd", "abcd");
    t.matching("-    ", "-    ", "    ");
    t.test();
  });

  chars(|t| {
    t.text("abcd", "yabcd", "abcd");
    t.matching("    ", "-    ", "    ");
    t.test();
  });
}

#[test]
fn multiple_changes() {
  chars(|t| {
    t.text("XXbXcXXeX", "XyXzXXnXkX", "XqXXeXrXX");
    t.matching("  - -  - ", " - -  - - ", " -  - -  ");
    t.test();
  });

  chars(|t| {
    t.text("Ax", "z", "zA");
    t.matching("--", "-", "--");
    t.test();
  });

  chars(|t| {
    t.text("ayz", "xyz", "xyq");
    t.matching("- -", "- -", "- -");
    t.test();
  });
}

// jscpd:ignore-end
