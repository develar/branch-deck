// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/SplitComparisonUtilTest.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2015 JetBrains s.r.o. and contributors).

use crate::comparison_util_test_base::{del, ins, modify, splitter};

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port
#[test]
fn split_blocks() {
  splitter(false, false, |t| {
    t.text("x", "z");
    t.default(&[modify(0, 0, 1, 1)]);
    t.test_all();
  });
  splitter(false, false, |t| {
    t.text("x_y", "a_b");
    t.default(&[modify(0, 0, 2, 2)]);
    t.test_all();
  });
  splitter(false, false, |t| {
    t.text("x_y", "a_b y");
    t.default(&[modify(0, 0, 1, 1), modify(1, 1, 1, 1)]);
    t.test_all();
  });
  splitter(false, false, |t| {
    t.text("x y", "x a_b y");
    t.default(&[modify(0, 0, 1, 2)]);
    t.test_all();
  });
  splitter(false, false, |t| {
    t.text("x_y", "x a_y b");
    t.default(&[modify(0, 0, 1, 1), modify(1, 1, 1, 1)]);
    t.test_all();
  });

  splitter(false, false, |t| {
    t.text("x_", "x a_...");
    t.default(&[modify(0, 0, 2, 2)]);
    t.test_all();
  });
  splitter(false, false, |t| {
    t.text("x_y_", "a_b_");
    t.default(&[modify(0, 0, 2, 2)]);
    t.test_all();
  });
  splitter(false, false, |t| {
    t.text("x_y", " x _ y ");
    t.default(&[modify(0, 0, 2, 2)]);
    t.test_default();
  });
  splitter(false, false, |t| {
    t.text("x_y", " x _ y.");
    t.default(&[modify(0, 0, 1, 1), modify(1, 1, 1, 1)]);
    t.test_default();
  });

  splitter(false, false, |t| {
    t.text("a_x_b_", " x_");
    t.default(&[del(0, 0, 1), modify(1, 0, 1, 1), del(2, 1, 1)]);
    t.test_default();
  });
  splitter(false, false, |t| {
    t.text("a_x_b_", "!x_");
    t.default(&[del(0, 0, 1), modify(1, 0, 1, 1), del(2, 1, 1)]);
    t.test_all();
  });

  splitter(false, false, |t| {
    t.text("< i", "<i<_i");
    t.default(&[modify(0, 0, 1, 2)]);
    t.test_default();
  });
  splitter(false, false, |t| {
    t.text(".l j", "U_._l j+");
    t.default(&[ins(0, 0, 2), modify(0, 2, 1, 1)]);
    t.test_default();
  });

  splitter(false, false, |t| {
    t.text("      run();", "      //commentary_      while(run(true));");
    t.default(&[modify(0, 0, 1, 2)]);
    t.test_default();
  });
}

#[test]
fn squash() {
  splitter(true, false, |t| {
    t.text("x", "z");
    t.default(&[modify(0, 0, 1, 1)]);
    t.test_all();
  });
  splitter(true, false, |t| {
    t.text("x_y", "a_b");
    t.default(&[modify(0, 0, 2, 2)]);
    t.test_all();
  });
  splitter(true, false, |t| {
    t.text("x_y", "a_b y");
    t.default(&[modify(0, 0, 2, 2)]);
    t.test_all();
  });

  splitter(true, false, |t| {
    t.text("a_x_b_", " x_");
    t.default(&[modify(0, 0, 3, 1)]);
    t.test_default();
  });
  splitter(true, false, |t| {
    t.text("a_x_b_", "!x_");
    t.default(&[modify(0, 0, 3, 1)]);
    t.test_all();
  });
}

#[test]
fn trim() {
  splitter(false, true, |t| {
    t.text("_", "     _    ");
    t.default(&[modify(0, 0, 2, 2)]);
    t.trim(&[]);
    t.test_all();
  });

  splitter(false, true, |t| {
    t.text("", "     _    ");
    t.default(&[modify(0, 0, 1, 2)]);
    t.trim(&[ins(1, 1, 1)]);
    t.ignore(&[]);
    t.test_all();
  });

  splitter(false, true, |t| {
    t.text("     _    ", "");
    t.default(&[modify(0, 0, 2, 1)]);
    t.trim(&[del(1, 1, 1)]);
    t.ignore(&[]);
    t.test_all();
  });

  splitter(false, true, |t| {
    t.text("x_y", "z_ ");
    t.default(&[modify(0, 0, 2, 2)]);
    t.test_all();
  });

  splitter(false, true, |t| {
    t.text("z", "z_ ");
    t.default(&[ins(1, 1, 1)]);
    t.ignore(&[]);
    t.test_all();
  });

  splitter(false, true, |t| {
    t.text("z_ x", "z_ w");
    t.default(&[modify(1, 1, 1, 1)]);
    t.test_all();
  });

  splitter(false, true, |t| {
    t.text("__z__", "z");
    t.default(&[del(0, 0, 2), del(3, 1, 2)]);
    t.ignore(&[]);
    t.test_all();
  });
}
// jscpd:ignore-end
