// Ported and modified from
// `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/ComparisonMergeUtilTestBase.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors). The Kotlin builder DSL is a
// builder with methods: `("x" - "y" - "z")` is `text("x", "y", "z")`, `(" " - "-" - " ").matching()` is
// `matching(" ", "-", " ")`, `changes(mod(0, 0, 0, 1, 1, 1))` is `changes(&[modify(0, 0, 0, 1, 1, 1)])`, and
// `test(policy)` is `test_policy(policy)`.

//! The test builder of the three-way comparison tests: three texts, and the expected changes or changed chars or lines.

use crate::by_char;
use crate::cancellation::Deadline;
use crate::comparison_merge_util::build_simple;
use crate::manager::{compare_lines_three, merge_lines};
use crate::merge_range::{MergeRange, ThreeSide};
use crate::policy::ComparisonPolicy;
use crate::test_support::{assert_sets_equals, insert_range, parse_line_matching, parse_matching, parse_source};
use crate::text::Utf16Text;
use fixedbitset::FixedBitSet;
use pretty_assertions::assert_eq;

/// A change of `count1`, `count2` and `count3` lines (or chars) at `line1`, `line2` and `line3` (`mod` of the source)
pub(crate) const fn modify(line1: usize, line2: usize, line3: usize, count1: usize, count2: usize, count3: usize) -> MergeRange {
  MergeRange::new(line1, line1 + count1, line2, line2 + count2, line3, line3 + count3)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestType {
  Char,
  LineDiff,
  LineMerge,
}

pub(crate) struct MergeTestBuilder {
  test_type: TestType,
  executed: bool,
  texts: Option<[Utf16Text; 3]>,
  changes: Option<Vec<MergeRange>>,
  matching: Option<[FixedBitSet; 3]>,
}

impl MergeTestBuilder {
  /// The three texts: left, base and right
  pub(crate) fn text(&mut self, left: &str, base: &str, right: &str) {
    if self.texts.is_none() {
      self.texts = Some([left, base, right].map(|text| Utf16Text::new(&parse_source(text))));
    }
  }

  /// The changed chars (or lines) of each text
  pub(crate) fn matching(&mut self, left: &str, base: &str, right: &str) {
    assert!(self.matching.is_none());
    let texts = self.texts.as_ref().expect("the texts first");
    let parse = |matching: &str, text: &Utf16Text| {
      if self.test_type == TestType::Char {
        parse_matching(matching, text)
      } else {
        parse_line_matching(matching, text)
      }
    };
    self.matching = Some([parse(left, &texts[0]), parse(base, &texts[1]), parse(right, &texts[2])]);
  }

  pub(crate) fn changes(&mut self, expected: &[MergeRange]) {
    assert!(self.changes.is_none());
    self.changes = Some(expected.to_vec());
  }

  pub(crate) fn test(&mut self) {
    self.test_policy(ComparisonPolicy::Default);
  }

  pub(crate) fn test_policy(&mut self, policy: ComparisonPolicy) {
    self.executed = true;
    assert!(self.changes.is_some() || self.matching.is_some());
    let [left, base, right] = self.texts.as_ref().expect("the texts first");

    let actual = match self.test_type {
      TestType::Char => {
        assert_eq!(policy, ComparisonPolicy::Default);
        let iterable1 = by_char::compare(base.units(), left.units(), &Deadline::NONE).expect("no limits");
        let iterable2 = by_char::compare(base.units(), right.units(), &Deadline::NONE).expect("no limits");
        let fragments = build_simple(&iterable1, &iterable2);
        check_consistency(&fragments);
        fragments
      }
      TestType::LineDiff => compare_lines_three(left, base, right, policy, &Deadline::NONE).expect("no limits"),
      TestType::LineMerge => merge_lines(left, base, right, policy, &Deadline::NONE).expect("no limits"),
    };

    if let Some(matching) = &self.matching {
      check_diff_matching(&actual, matching);
    }
    if let Some(expected) = &self.changes {
      assert_eq!(expected, &actual);
    }
  }
}

/// The changes are in order, none is empty, and none starts where the previous one ends on every side
fn check_consistency(actual: &[MergeRange]) {
  let mut lasts: Option<[usize; 3]> = None;
  for change in actual {
    let mut empty = true;
    let mut squashed = true;
    for side in ThreeSide::ALL {
      let (start, end) = (change.start(side), change.end(side));
      if let Some(lasts) = lasts {
        assert!(lasts[side.index()] <= start);
      }
      assert!(start <= end);
      empty &= start == end;
      squashed &= lasts.is_some_and(|lasts| lasts[side.index()] == start);
    }
    assert!(!empty);
    assert!(!squashed);
    lasts = Some(ThreeSide::ALL.map(|side| change.end(side)));
  }
}

fn check_diff_matching(changes: &[MergeRange], matchings: &[FixedBitSet; 3]) {
  let mut sets = [FixedBitSet::new(), FixedBitSet::new(), FixedBitSet::new()];
  for change in changes {
    for side in ThreeSide::ALL {
      insert_range(&mut sets[side.index()], change.start(side), change.end(side));
    }
  }
  assert_sets_equals(&matchings[0], &sets[0], "Left");
  assert_sets_equals(&matchings[1], &sets[1], "Base");
  assert_sets_equals(&matchings[2], &sets[2], "Right");
}

fn run(test_type: TestType, f: impl FnOnce(&mut MergeTestBuilder)) {
  let mut builder = MergeTestBuilder {
    test_type,
    executed: false,
    texts: None,
    changes: None,
    matching: None,
  };
  f(&mut builder);
  assert!(builder.executed);
}

pub(crate) fn chars(f: impl FnOnce(&mut MergeTestBuilder)) {
  run(TestType::Char, f);
}

pub(crate) fn lines_diff(f: impl FnOnce(&mut MergeTestBuilder)) {
  run(TestType::LineDiff, f);
}

pub(crate) fn lines_merge(f: impl FnOnce(&mut MergeTestBuilder)) {
  run(TestType::LineMerge, f);
}

// Ported and modified from the three-side word tests of
// `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/ComparisonUtilTestBase.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors): `("x" - "y" - "z")` is
// `text("x", "y", "z")`, and `(" " - "-" - " ").default()` is `default_matching(" ", "-", " ")`.

/// The test builder of the three-side word comparison: the changed chars of each text under each policy; a policy
/// without its own matching takes the one of the policy before it
pub(crate) struct WordMergeTestBuilder {
  texts: Option<[Utf16Text; 3]>,
  default: Option<[FixedBitSet; 3]>,
  trim: Option<[FixedBitSet; 3]>,
  ignore: Option<[FixedBitSet; 3]>,
  executed: bool,
}

impl WordMergeTestBuilder {
  pub(crate) fn text(&mut self, left: &str, base: &str, right: &str) {
    self.texts = Some([left, base, right].map(|text| Utf16Text::new(&parse_source(text))));
  }

  fn parse(&self, left: &str, base: &str, right: &str) -> [FixedBitSet; 3] {
    let texts = self.texts.as_ref().expect("the texts first");
    [parse_matching(left, &texts[0]), parse_matching(base, &texts[1]), parse_matching(right, &texts[2])]
  }

  pub(crate) fn default_matching(&mut self, left: &str, base: &str, right: &str) {
    self.default = Some(self.parse(left, base, right));
  }

  pub(crate) fn trim_matching(&mut self, left: &str, base: &str, right: &str) {
    self.trim = Some(self.parse(left, base, right));
  }

  pub(crate) fn ignore_matching(&mut self, left: &str, base: &str, right: &str) {
    self.ignore = Some(self.parse(left, base, right));
  }

  pub(crate) fn test_all(&mut self) {
    self.executed = true;
    let [left, base, right] = self.texts.as_ref().expect("the texts first");
    for policy in [ComparisonPolicy::Default, ComparisonPolicy::TrimWhitespaces, ComparisonPolicy::IgnoreWhitespaces] {
      let matching = match policy {
        ComparisonPolicy::Default => self.default.as_ref(),
        ComparisonPolicy::TrimWhitespaces => self.trim.as_ref().or(self.default.as_ref()),
        ComparisonPolicy::IgnoreWhitespaces => self.ignore.as_ref().or(self.trim.as_ref()).or(self.default.as_ref()),
      };
      let matching = matching.expect("a matching");
      let fragments = crate::by_word::compare_three(left.units(), base.units(), right.units(), policy, &Deadline::NONE).expect("no limits");
      for fragment in &fragments {
        assert!(ThreeSide::ALL.iter().all(|&side| fragment.start(side) <= fragment.end(side)), "{policy:?}");
        assert!(!fragment.is_empty(), "{policy:?}");
      }
      check_diff_matching(&fragments, matching);
    }
  }
}

pub(crate) fn words(f: impl FnOnce(&mut WordMergeTestBuilder)) {
  let mut builder = WordMergeTestBuilder {
    texts: None,
    default: None,
    trim: None,
    ignore: None,
    executed: false,
  };
  f(&mut builder);
  assert!(builder.executed);
}
