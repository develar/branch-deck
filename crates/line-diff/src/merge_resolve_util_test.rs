// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/MergeResolveUtilTest.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2016 JetBrains s.r.o. and contributors). Each case is
// `(base, left, right, expected)`, as in the source.

use crate::cancellation::Deadline;
use crate::merge_resolve_util::{try_greedy_resolve, try_resolve};
use pretty_assertions::assert_eq;

fn units(text: &str) -> Vec<u16> {
  text.encode_utf16().collect()
}

fn check_simple(base: &str, left: &str, right: &str, expected: Option<&str>) {
  let resolved = try_resolve(&units(left), &units(base), &units(right), &Deadline::NONE);
  assert_eq!(resolved.map(|text| String::from_utf16_lossy(&text)).as_deref(), expected, "simple");
}

fn check_greedy(base: &str, left: &str, right: &str, expected: Option<&str>) {
  let resolved = try_greedy_resolve(&units(left), &units(base), &units(right), &Deadline::NONE);
  assert_eq!(resolved.map(|text| String::from_utf16_lossy(&text)).as_deref(), expected, "greedy");
}

fn check(base: &str, left: &str, right: &str, expected: Option<&str>) {
  check_simple(base, left, right, expected);
  check_greedy(base, left, right, expected);
}

fn check_both(base: &str, left: &str, right: &str, expected_simple: Option<&str>, expected_greedy: Option<&str>) {
  check_simple(base, left, right, expected_simple);
  check_greedy(base, left, right, expected_greedy);
}

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port

#[test]
fn simple() {
  check("", "", "", Some(""));
  check("x x x", "x x x", "x x x", Some("x x x"));
  check("x x x", "x Y x", "x x x", Some("x Y x"));
  check("x x", "x x", "x Y x", Some("x Y x"));
  check("x X x", "x x", "x X x", Some("x x"));
}

#[test]
fn same_modification() {
  check("x x x", "x Y x", "x Y x", Some("x Y x"));
  check("x x", "x Y x", "x Y x", Some("x Y x"));
  check("x X x", "x x", "x x", Some("x x"));
}

#[test]
fn non_conflicting_changes() {
  check("x x x", "x Y x x", "x x Z x", Some("x Y x Z x"));
  check("x", "x Y", "Z x", Some("Z x Y"));
  check("x x", "x", "Z x x", Some("Z x"));
}

#[test]
fn failure() {
  check("x x x", "x Y x", "x Z x", None);
  check("x x", "x Y x", "x Z x", None);
}

#[test]
fn non_failure_conflicts() {
  check_greedy("x X x", "x x", "x X Y x", Some("x Y x"));
  check_greedy("x X x", "x x", "x Y X x", Some("x Y x"));
  check_greedy("x X Y x", "x X x", "x Y x", Some("x x"));
  check_greedy("x X Y Z x", "x X x", "x Z x", Some("x x"));
  check_greedy("x A B C D E F G H K x", "x C F K x", "x A D H x", Some("x x"));
}

#[test]
fn confusing_conflicts() {
  // these cases might be a failure as well
  check_greedy("x X x", "x x", "x Z x", Some("xZ x"));
  check_greedy("x X X x", "x X Y X x", "x x", Some("x Y x"));
  check_greedy("x X x", "x x", "x Y x", Some("xY x"));
  check_greedy("x X X x", "x Y x", "x X Y x", Some("x Y x"));
  // IDEA-200696
  check("version: 1.0.0", "version: 2.0.0", "version: 1.0.4", Some("version: 2.0.4"));
}

#[test]
fn regressions() {
  check_both("i\n", "i", "\ni", Some("i"), Some("i"));
  check_simple("Y X Y", "Y C\nX\nC Y", "Y \nX\n Y", Some("Y \nC\nX\nC Y"));
  check(
    "
  public static class ChangeData {
    @NotNull public final ChangeKind kind;
    public final int otherPath;

    public ChangeData(@NotNull ChangeKind kind, int otherPath) {
      this.kind = kind;
      this.otherPath = otherPath;
    }

    public boolean isRename() {
      return kind.equals(ChangeKind.RENAMED_FROM) || kind.equals(ChangeKind.RENAMED_TO);
    }

    @Override
    public int hashCode() {
      return Objects.hash(kind, otherPath);
    }
  }

  private enum ChangeKind {
    MODIFIED((byte)0),
    RENAMED_FROM((byte)1),
    RENAMED_TO((byte)2);",
    "
  public enum ChangeKind {
    NOT_CHANGED((byte)-1),
    MODIFIED((byte)0),
    ADDED((byte)1),
    REMOVED((byte)2);",
    "
  public static class ChangeData {
  @NotNull public final ChangeKind kind;
  public final int otherPath;

  public ChangeData(@NotNull ChangeKind kind, int otherPath) {
    this.kind = kind;
    this.otherPath = otherPath;
  }

  public boolean isRename() {
    return kind.equals(ChangeKind.RENAMED_FROM) || kind.equals(ChangeKind.RENAMED_TO);
  }

  @Override
  public int hashCode() {
    return Objects.hash(kind, otherPath);
  }
}

private enum ChangeKind {
  MODIFIED((byte)0),
  RENAMED_FROM((byte)1),
  RENAMED_TO((byte)2);",
    Some(
      "
  public enum ChangeKind {
    NOT_CHANGED((byte)-1),
  MODIFIED((byte)0),
  ADDED((byte)1),
  REMOVED((byte)2);",
    ),
  );
}
// jscpd:ignore-end
