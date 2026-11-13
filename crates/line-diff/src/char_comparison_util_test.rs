// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/CharComparisonUtilTest.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2015 JetBrains s.r.o. and contributors).

use crate::comparison_util_test_base::{chars, chars_raw, chars_smart, del, ins};
use crate::test_support::{CH_FACE, CH_GUN, CH_MAN, CH_SMILE};

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port
#[test]
fn equal_strings() {
  chars(|t| {
    t.text("", "");
    t.default_matching("", "");
    t.test_all();
  });

  chars(|t| {
    t.text("x", "x");
    t.default_matching(" ", " ");
    t.test_all();
  });

  chars(|t| {
    t.text("x_y_z_", "x_y_z_");
    t.default_matching("      ", "      ");
    t.test_all();
  });

  chars(|t| {
    t.text("_", "_");
    t.default_matching(" ", " ");
    t.test_all();
  });

  chars(|t| {
    t.text("xxx", "xxx");
    t.default_matching("   ", "   ");
    t.test_all();
  });

  chars(|t| {
    t.text("xyz", "xyz");
    t.default_matching("   ", "   ");
    t.test_all();
  });

  chars(|t| {
    t.text(".x!", ".x!");
    t.default_matching("   ", "   ");
    t.test_all();
  });
}

#[test]
fn trivial_cases() {
  chars(|t| {
    t.text("x", "");
    t.default_matching("-", "");
    t.test_all();
  });

  chars(|t| {
    t.text("", "x");
    t.default_matching("", "-");
    t.test_all();
  });

  chars(|t| {
    t.text("x", "y");
    t.default_matching("-", "-");
    t.test_all();
  });

  chars(|t| {
    t.text("x_", "");
    t.default_matching("--", "");
    t.trim_matching("- ", "");
    t.test_all();
  });

  chars(|t| {
    t.text("", "x_");
    t.default_matching("", "--");
    t.trim_matching("", "- ");
    t.test_all();
  });

  chars(|t| {
    t.text("x_", "y_");
    t.default_matching("- ", "- ");
    t.test_all();
  });

  chars(|t| {
    t.text("_x", "_y");
    t.default_matching(" -", " -");
    t.test_all();
  });
}

#[test]
fn simple_cases() {
  chars(|t| {
    t.text("xyx", "xxx");
    t.default_matching(" - ", " - ");
    t.test_all();
  });

  chars(|t| {
    t.text("xyyx", "xmx");
    t.default_matching(" -- ", " - ");
    t.test_all();
  });

  chars(|t| {
    t.text("xyy", "yyx");
    t.default_matching("-  ", "  -");
    t.test_all();
  });

  chars(|t| {
    t.text("x!!", "!!x");
    t.default_matching("-  ", "  -");
    t.test_all();
  });

  chars(|t| {
    t.text("xyx", "xx");
    t.default_matching(" - ", "  ");
    t.test_all();
  });

  chars(|t| {
    t.text("xx", "xyx");
    t.default_matching("  ", " - ");
    t.test_all();
  });

  chars(|t| {
    t.text("!...", "...!");
    t.default_matching("-   ", "   -");
    t.test_all();
  });
}

#[test]
fn whitespace_changes_only() {
  chars(|t| {
    t.text(" x y z ", "xyz");
    t.default_matching("- - - -", "   ");
    t.trim_matching("  - -  ", "   ");
    t.ignore_matching("       ", "   ");
    t.test_all();
  });

  chars(|t| {
    t.text("xyz", " x y z ");
    t.default_matching("   ", "- - - -");
    t.trim_matching("   ", "  - -  ");
    t.ignore_matching("   ", "       ");
    t.test_all();
  });

  chars(|t| {
    t.text("x ", "x");
    t.default_matching(" -", " ");
    t.trim_matching("  ", " ");
    t.test_all();
  });

  chars(|t| {
    t.text("x", " x");
    t.default_matching(" ", "- ");
    t.trim_matching(" ", "  ");
    t.test_all();
  });

  chars(|t| {
    t.text(" x ", "x");
    t.default_matching("- -", " ");
    t.trim_matching("   ", " ");
    t.test_all();
  });

  chars(|t| {
    t.text("x", " x ");
    t.default_matching(" ", "- -");
    t.trim_matching(" ", "   ");
    t.test_all();
  });
}

#[test]
fn whitespace_changes() {
  chars(|t| {
    t.text(" x ", "z");
    t.default_matching("---", "-");
    t.trim_matching(" - ", "-");
    t.test_all();
  });

  chars(|t| {
    t.text("x", " z ");
    t.default_matching("-", "---");
    t.trim_matching("-", " - ");
    t.test_all();
  });

  chars(|t| {
    t.text(" x", "z\t");
    t.default_matching("--", "-.-");
    t.trim_matching(" -", "-. ");
    t.test_all();
  });

  chars(|t| {
    t.text("x ", "\tz");
    t.default_matching("--", ".--");
    t.trim_matching("- ", ". -");
    t.test_all();
  });

  chars(|t| {
    t.text("x y z", "xy_z");
    t.default_matching(" - - ", "  - ");
    t.trim_matching(" - - ", "    ");
    t.ignore_matching("     ", "    ");
    t.test_all();
  });

  chars(|t| {
    t.text("x y \n z", "xy\nz");
    t.default_matching(" - -. - ", "  .  ");
    t.trim_matching(" -  .   ", "  .  ");
    t.ignore_matching("    .   ", "  .  ");
    t.test_all();
  });
}

#[test]
fn ignore_inner_whitespaces() {
  chars(|t| {
    t.text("x z y", "xmn");
    t.default_matching(" ----", " --");
    t.ignore_matching("  ---", " --");
    t.test_all();
  });

  chars(|t| {
    t.text("x y z ", "x y m ");
    t.default_matching("    - ", "    - ");
    t.test_all();
  });

  chars(|t| {
    t.text("x y z", "x y m ");
    t.default_matching("    -", "    --");
    t.trim_matching("    -", "    - ");
    t.test_all();
  });

  chars(|t| {
    t.text(" x y z", " m y z");
    t.default_matching(" -    ", " -    ");
    t.test_all();
  });

  chars(|t| {
    t.text("x y z", " m y z");
    t.default_matching("-    ", "--    ");
    t.trim_matching("-    ", " -    ");
    t.test_all();
  });

  chars(|t| {
    t.text("x y z", "x m z");
    t.default_matching("  -  ", "  -  ");
    t.test_all();
  });

  chars(|t| {
    t.text("x y z", "x  z");
    t.default_matching("  -  ", "    ");
    t.test_all();
  });

  chars(|t| {
    t.text("x  z", "x m z");
    t.default_matching("    ", "  -  ");
    t.test_all();
  });

  chars(|t| {
    t.text("x  z", "x n m z");
    t.default_matching("    ", "  ---  ");
    t.test_all();
  });
}

#[test]
fn empty_range_positions() {
  chars(|t| {
    t.text("x y", "x zy");
    t.default(&[ins(2, 2, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("x y", "xz y");
    t.default(&[ins(1, 1, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("x y z", "x  z");
    t.default(&[del(2, 2, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("x  z", "x m z");
    t.default(&[ins(2, 2, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("xyx", "xx");
    t.default(&[del(1, 1, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("xx", "xyx");
    t.default(&[ins(1, 1, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("xy", "x");
    t.default(&[del(1, 1, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("x", "xy");
    t.default(&[ins(1, 1, 1)]);
    t.test_all();
  });
}

#[test]
fn algorithm_specific() {
  // This is a strange example: "ignore whitespace" produces lesser matching, than "Default".
  // This is fine, as the main goal of "ignore whitespaces" is to reduce 'noise' of diff, and 1 change is better than 3 changes
  // So we actually "ignore" whitespaces during comparison, rather than "mark all whitespaces as matched".
  chars(|t| {
    t.text("x   y   z", "xX      Zz");
    t.default_matching("    -    ", " -      - ");
    t.ignore_matching("    -    ", " -------- ");
    t.test_all();
  });
}

#[test]
fn non_deterministic_cases() {
  chars(|t| {
    t.text("x", "  ");
    t.ignore(&[del(0, 0, 1)]);
    t.test_ignore();
  });

  chars(|t| {
    t.text("  ", "x");
    t.ignore(&[ins(0, 0, 1)]);
    t.test_ignore();
  });

  chars(|t| {
    t.text("x .. z", "x y .. z");
    t.default_matching("      ", "  --    ");
    t.ignore_matching("      ", "  -     ");
    t.default(&[ins(2, 2, 2)]);
    t.ignore(&[ins(2, 2, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text(" x _ y _ z ", "x z");
    t.default_matching("-  ------ -", "   ");
    t.trim_matching("     -     ", "   ");
    t.default(&[del(0, 0, 1), del(3, 2, 6), del(10, 3, 1)]);
    t.trim(&[del(5, 2, 1)]);
    t.test_all();
  });

  chars(|t| {
    t.text("x z", " x _ y _ z ");
    t.default_matching("   ", "-  ------ -");
    t.trim_matching("   ", "     -     ");
    t.default(&[ins(0, 0, 1), ins(2, 3, 6), ins(3, 10, 1)]);
    t.trim(&[ins(2, 5, 1)]);
    t.test_all();
  });
}

#[test]
fn two_steps() {
  chars_smart(|t| {
    t.text("  a", "a  ");
    t.default_matching("-- ", " --");
    t.test_default();
  });

  chars_raw(|t| {
    t.text("  a", "a  ");
    t.default_matching("  -", "-  ");
    t.test_default();
  });

  chars_smart(|t| {
    t.text("bba", "abb");
    t.default_matching("  -", "-  ");
    t.test_default();
  });

  chars_raw(|t| {
    t.text("bba", "abb");
    t.default_matching("  -", "-  ");
    t.test_default();
  });

  chars_smart(|t| {
    t.text(&format!("{CH_SMILE}{CH_FACE} {CH_MAN}"), &format!(" {CH_GUN}{CH_FACE}{CH_MAN} "));
    t.default_matching("--  -  ", "---    -");
    t.test_default();
  });

  chars_raw(|t| {
    t.text(&format!("{CH_SMILE}{CH_FACE} {CH_MAN}"), &format!(" {CH_GUN}{CH_FACE}{CH_MAN} "));
    t.default_matching("--   --", "---  -- ");
    t.test_default();
  });
}

#[test]
fn high_surrogates() {
  chars(|t| {
    t.text(CH_SMILE, CH_SMILE);
    t.default_matching("  ", "  ");
    t.test_all();
  });

  chars(|t| {
    t.text(CH_SMILE, CH_MAN);
    t.default_matching("--", "--");
    t.test_all();
  });

  chars(|t| {
    t.text(CH_SMILE, CH_GUN);
    t.default_matching("--", "--");
    t.test_all();
  });

  chars(|t| {
    t.text(CH_FACE, CH_GUN);
    t.default_matching("--", "--");
    t.test_all();
  });

  chars(|t| {
    t.text(&format!("{CH_SMILE}{CH_GUN}{CH_MAN}{CH_FACE}"), &format!("{CH_GUN}{CH_MAN}{CH_FACE}{CH_SMILE}"));
    t.default_matching("--      ", "      --");
    t.test_all();
  });

  chars(|t| {
    t.text(CH_SMILE, CH_GUN);
    t.default_matching("--", "--");
    t.test_all();
  });

  chars(|t| {
    t.text(CH_FACE, CH_GUN);
    t.default_matching("--", "--");
    t.test_all();
  });

  chars(|t| {
    t.text(&format!("{CH_SMILE} "), CH_GUN);
    t.default_matching("---", "--");
    t.trim_matching("-- ", "--");
    t.test_all();
  });

  chars(|t| {
    t.text(&format!(" {CH_SMILE}{CH_FACE} {CH_MAN}"), &format!("{CH_GUN}{CH_FACE}{CH_MAN}"));
    t.default_matching("---  -  ", "--    ");
    t.trim_matching(" --  -  ", "--    ");
    t.ignore_matching(" --     ", "--    ");
    t.test_all();
  });
}
// jscpd:ignore-end
