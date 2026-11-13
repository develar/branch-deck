// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/LineComparisonMergeUtilTest.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2018 JetBrains s.r.o. and contributors).

use crate::comparison_merge_util_test_base::{lines_diff, lines_merge, modify};
use crate::policy::ComparisonPolicy;

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port

#[test]
fn simple_cases() {
  lines_merge(|t| {
    t.text("", "", "");
    t.matching("", "", "");
    t.changes(&[]);
    t.test();
  });

  lines_merge(|t| {
    t.text("x_y_z", "x_y_z", "x_y_z");
    t.matching(" _ _ ", " _ _ ", " _ _ ");
    t.changes(&[]);
    t.test();
  });

  lines_merge(|t| {
    t.text("", "", "x");
    t.changes(&[modify(0, 0, 0, 1, 1, 1)]);
    t.test();
  });

  lines_merge(|t| {
    t.text("", "", "_x");
    t.matching("", "", "_-");
    t.changes(&[modify(1, 1, 1, 0, 0, 1)]);
    t.test();
  });

  lines_merge(|t| {
    t.text("x", "y", "x");
    t.matching("-", "-", "-");
    t.test();
  });

  lines_merge(|t| {
    t.text("x", "x_y", "x");
    t.matching(" ", " _-", " ");
    t.test();
  });

  lines_merge(|t| {
    t.text("x", "y_x", "x");
    t.matching(" ", "-_ ", " ");
    t.test();
  });

  lines_merge(|t| {
    t.text("x_y_z", "x_Y_z", "x_y_z");
    t.matching(" _-_ ", " _-_ ", " _-_ ");
    t.test();
  });

  lines_merge(|t| {
    t.text("x_y_z", "X_y_Z", "x_y_z");
    t.matching("-_ _-", "-_ _-", "-_ _-");
    t.test();
  });
}

#[test]
fn ignore_whitespaces() {
  lines_diff(|t| {
    t.text("x_ y _z", "x_y_z", "x_  y_  z");
    t.changes(&[]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_merge(|t| {
    t.text("x_ y _z", "x_y_z", "x_  y_  z");
    t.changes(&[modify(1, 1, 1, 2, 2, 2)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_diff(|t| {
    t.text("x_ y _z", "x_Y_z", "x_  y_  z");
    t.changes(&[modify(1, 1, 1, 1, 1, 1)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_merge(|t| {
    t.text("x_ y _z", "x_Y_z", "x_  y_  z");
    t.changes(&[modify(1, 1, 1, 1, 1, 1), modify(2, 2, 2, 1, 1, 1)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_merge(|t| {
    t.text("x_ y _z", "x_Y_z", "x_  y_z");
    t.changes(&[modify(1, 1, 1, 1, 1, 1)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_merge(|t| {
    t.text("x_ y _z", "x_y_z", "x_  y_z");
    t.changes(&[modify(1, 1, 1, 1, 1, 1)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_merge(|t| {
    t.text(" x _y_z", "x_y_z", "x_y_  z");
    t.changes(&[modify(0, 0, 0, 1, 1, 1), modify(2, 2, 2, 1, 1, 1)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_diff(|t| {
    t.text(" x_new_y_z", "x_ y _z", "x_y_new_ z");
    t.changes(&[modify(1, 1, 1, 1, 0, 0), modify(3, 2, 2, 0, 0, 1)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_merge(|t| {
    t.text(" x_new_y_z", "x_ y _z", "x_y_new_ z");
    t.changes(&[
      modify(0, 0, 0, 1, 1, 1),
      modify(1, 1, 1, 1, 0, 0),
      modify(2, 1, 1, 1, 1, 1),
      modify(3, 2, 2, 0, 0, 1),
      modify(3, 2, 3, 1, 1, 1),
    ]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_diff(|t| {
    t.text(" x_new_y1_z", "x_ y _z", "x_y2_new_ z");
    t.changes(&[modify(1, 1, 1, 2, 1, 2)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });

  lines_merge(|t| {
    t.text(" x_new_y1_z", "x_ y _z", "x_y2_new_ z");
    t.changes(&[modify(0, 0, 0, 1, 1, 1), modify(1, 1, 1, 2, 1, 2), modify(3, 2, 3, 1, 1, 1)]);
    t.test_policy(ComparisonPolicy::IgnoreWhitespaces);
  });
}

// jscpd:ignore-end
