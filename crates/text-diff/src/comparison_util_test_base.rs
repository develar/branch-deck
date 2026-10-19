// Ported and modified from
// `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/ComparisonUtilTestBase.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors). The Kotlin builder DSL is a
// builder with methods: `("x_y" - "x_z")` is `text("x_y", "x_z")`, `("--_  " - "--_  ").default()` is
// `default_matching("--_  ", "--_  ")`, and `default(mod(0, 0, 1, 1))` is `default(&[modify(0, 0, 1, 1)])`. Left out:
// the three-side word tests of the merge.

//! The test builder of the comparison tests: two texts, and the expected changes or changed chars under each policy.

use crate::by_char;
use crate::by_word::convert_into_diff_fragments;
use crate::cancellation::Deadline;
use crate::fragments::{DiffFragment, LineFragment};
use crate::manager::{compare_chars, compare_lines, compare_lines_inner, compare_words, process_blocks, squash};
use crate::policy::ComparisonPolicy;
use crate::test_support::{assert_sets_equals, get_line_count, insert_range, parse_line_matching, parse_matching, parse_source};
use crate::text::Utf16Text;
use fixedbitset::FixedBitSet;
use pretty_assertions::assert_eq;

/// An expected change: the lines (or offsets) of the first side and of the second side
pub(crate) type Change = [(usize, usize); 2];

/// A change of `count1` lines at `line1` into `count2` lines at `line2` (`mod` of the source)
pub(crate) fn modify(line1: usize, line2: usize, count1: usize, count2: usize) -> Change {
  assert_ne!(count1, 0);
  assert_ne!(count2, 0);
  [(line1, line1 + count1), (line2, line2 + count2)]
}

/// A deletion of `count1` lines at `line1`, before `line2` of the second side
pub(crate) fn del(line1: usize, line2: usize, count1: usize) -> Change {
  assert_ne!(count1, 0);
  [(line1, line1 + count1), (line2, line2)]
}

/// An insertion of `count2` lines at `line2`, before `line1` of the first side
pub(crate) fn ins(line1: usize, line2: usize, count2: usize) -> Change {
  assert_ne!(count2, 0);
  [(line1, line1), (line2, line2 + count2)]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestType {
  Line,
  LineInner,
  Word,
  Char,
  CharSmart,
  CharRaw,
  Splitter,
}

/// A value for each policy; a policy without its own value takes the one of the policy before it
struct PolicyData<T> {
  default: Option<T>,
  trim: Option<T>,
  ignore: Option<T>,
}

impl<T> PolicyData<T> {
  fn get(&self, policy: ComparisonPolicy) -> Option<&T> {
    match policy {
      ComparisonPolicy::IgnoreWhitespaces => self.ignore.as_ref().or(self.trim.as_ref()).or(self.default.as_ref()),
      ComparisonPolicy::TrimWhitespaces => self.trim.as_ref().or(self.default.as_ref()),
      ComparisonPolicy::Default => self.default.as_ref(),
    }
  }

  fn slot(&mut self, policy: ComparisonPolicy) -> &mut Option<T> {
    match policy {
      ComparisonPolicy::Default => &mut self.default,
      ComparisonPolicy::TrimWhitespaces => &mut self.trim,
      ComparisonPolicy::IgnoreWhitespaces => &mut self.ignore,
    }
  }
}

pub(crate) struct TestBuilder {
  test_type: TestType,
  is_executed: bool,
  text: Option<[Utf16Text; 2]>,
  changes: PolicyData<Vec<Change>>,
  matchings: PolicyData<[FixedBitSet; 2]>,
  should_squash: bool,
  should_trim: bool,
}

impl TestBuilder {
  const fn new(test_type: TestType) -> Self {
    Self {
      test_type,
      is_executed: false,
      text: None,
      changes: PolicyData {
        default: None,
        trim: None,
        ignore: None,
      },
      matchings: PolicyData {
        default: None,
        trim: None,
        ignore: None,
      },
      should_squash: false,
      should_trim: false,
    }
  }

  /// The two texts, `_` for `\n`; a later call does not replace them
  pub(crate) fn text(&mut self, before: &str, after: &str) -> &mut Self {
    if self.text.is_none() {
      self.text = Some([Utf16Text::new(&parse_source(before)), Utf16Text::new(&parse_source(after))]);
    }
    self
  }

  /// The two texts as they are, `_` included (`plainSource`)
  pub(crate) fn plain_text(&mut self, before: &str, after: &str) -> &mut Self {
    self.text = Some([Utf16Text::new(before), Utf16Text::new(after)]);
    self
  }

  /// The changed chars (or lines) under the default policy
  pub(crate) fn default_matching(&mut self, before: &str, after: &str) -> &mut Self {
    self.matching(ComparisonPolicy::Default, before, after)
  }

  pub(crate) fn trim_matching(&mut self, before: &str, after: &str) -> &mut Self {
    self.matching(ComparisonPolicy::TrimWhitespaces, before, after)
  }

  pub(crate) fn ignore_matching(&mut self, before: &str, after: &str) -> &mut Self {
    self.matching(ComparisonPolicy::IgnoreWhitespaces, before, after)
  }

  fn matching(&mut self, policy: ComparisonPolicy, before: &str, after: &str) -> &mut Self {
    self.text(before, after);
    let [text1, text2] = self.text.as_ref().expect("the texts");
    let matching = if self.test_type == TestType::Line {
      [parse_line_matching(before, text1), parse_line_matching(after, text2)]
    } else {
      [parse_matching(before, text1), parse_matching(after, text2)]
    };
    let slot = self.matchings.slot(policy);
    assert!(slot.is_none());
    *slot = Some(matching);
    self
  }

  /// The expected changes under the default policy
  pub(crate) fn default(&mut self, expected: &[Change]) -> &mut Self {
    self.expect_changes(ComparisonPolicy::Default, expected)
  }

  pub(crate) fn trim(&mut self, expected: &[Change]) -> &mut Self {
    self.expect_changes(ComparisonPolicy::TrimWhitespaces, expected)
  }

  pub(crate) fn ignore(&mut self, expected: &[Change]) -> &mut Self {
    self.expect_changes(ComparisonPolicy::IgnoreWhitespaces, expected)
  }

  fn expect_changes(&mut self, policy: ComparisonPolicy, expected: &[Change]) -> &mut Self {
    let slot = self.changes.slot(policy);
    assert!(slot.is_none());
    *slot = Some(expected.to_vec());
    self
  }

  pub(crate) fn test_all(&mut self) {
    self.test_default();
    self.test_trim();
    self.test_ignore();
  }

  pub(crate) fn test_default(&mut self) {
    self.run(ComparisonPolicy::Default);
  }

  pub(crate) fn test_trim(&mut self) {
    self.run(ComparisonPolicy::TrimWhitespaces);
  }

  pub(crate) fn test_ignore(&mut self) {
    self.run(ComparisonPolicy::IgnoreWhitespaces);
  }

  fn run(&mut self, policy: ComparisonPolicy) {
    self.is_executed = true;

    let [before, after] = self.text.as_ref().expect("the texts");
    let changes = self.changes.get(policy).map(Vec::as_slice);
    let matchings = self.matchings.get(policy);
    assert!(changes.is_some() || matchings.is_some(), "Policy: {policy:?}");

    match self.test_type {
      TestType::Line => do_line_test(before, after, matchings, changes, policy),
      TestType::LineInner => {
        do_line_inner_test(before, after, matchings, changes, policy);
        do_word_test(before, after, matchings, changes, policy);
      }
      TestType::Word => do_word_test(before, after, matchings, changes, policy),
      TestType::Char => {
        do_char_test(before, after, matchings, changes, policy);
        if policy == ComparisonPolicy::Default {
          do_char_raw_test(before, after, matchings, changes);
        }
      }
      TestType::CharSmart => do_char_test(before, after, matchings, changes, policy),
      TestType::CharRaw => {
        if policy == ComparisonPolicy::Default {
          do_char_raw_test(before, after, matchings, changes);
        }
      }
      TestType::Splitter => {
        assert!(matchings.is_none());
        do_splitter_test(before, after, self.should_squash, self.should_trim, changes, policy);
      }
    }
  }
}

fn do_line_test(before: &Utf16Text, after: &Utf16Text, matchings: Option<&[FixedBitSet; 2]>, expected: Option<&[Change]>, policy: ComparisonPolicy) {
  let fragments = compare_lines(before, after, policy, &Deadline::NONE).expect("no limits");
  check_consistency_lines(&fragments, before, after);
  if let Some(matchings) = matchings {
    check_line_matching(&fragments, matchings, policy);
  }
  if let Some(expected) = expected {
    check_line_changes(&fragments, expected, policy);
  }
}

fn do_line_inner_test(before: &Utf16Text, after: &Utf16Text, matchings: Option<&[FixedBitSet; 2]>, expected: Option<&[Change]>, policy: ComparisonPolicy) {
  let raw_fragments = compare_lines_inner(before, after, policy, &Deadline::NONE).expect("no limits");
  let fragments = squash(raw_fragments);
  check_consistency_line_inner(&fragments, before, after);

  let diff_fragments = fragments[0].inner_fragments.as_deref().expect("inner fragments");
  if let Some(matchings) = matchings {
    check_diff_matching(diff_fragments, matchings, policy);
  }
  if let Some(expected) = expected {
    check_diff_changes(diff_fragments, expected, policy);
  }
}

fn do_word_test(before: &Utf16Text, after: &Utf16Text, matchings: Option<&[FixedBitSet; 2]>, expected: Option<&[Change]>, policy: ComparisonPolicy) {
  let fragments = compare_words(before.units(), after.units(), policy, &Deadline::NONE).expect("no limits");
  check_diff_fragments(&fragments, matchings, expected, policy);
}

fn do_char_test(before: &Utf16Text, after: &Utf16Text, matchings: Option<&[FixedBitSet; 2]>, expected: Option<&[Change]>, policy: ComparisonPolicy) {
  let fragments = compare_chars(before.units(), after.units(), policy, &Deadline::NONE).expect("no limits");
  check_diff_fragments(&fragments, matchings, expected, policy);
}

fn do_char_raw_test(before: &Utf16Text, after: &Utf16Text, matchings: Option<&[FixedBitSet; 2]>, expected: Option<&[Change]>) {
  let iterable = by_char::compare(before.units(), after.units(), &Deadline::NONE).expect("no limits");
  let fragments = convert_into_diff_fragments(&iterable);
  check_diff_fragments(&fragments, matchings, expected, ComparisonPolicy::Default);
}

/// The consistency, the changed chars and the changes of the fragments of a word or char test
fn check_diff_fragments(fragments: &[DiffFragment], matchings: Option<&[FixedBitSet; 2]>, expected: Option<&[Change]>, policy: ComparisonPolicy) {
  check_consistency_diff(fragments);
  if let Some(matchings) = matchings {
    check_diff_matching(fragments, matchings, policy);
  }
  if let Some(expected) = expected {
    check_diff_changes(fragments, expected, policy);
  }
}

fn do_splitter_test(before: &Utf16Text, after: &Utf16Text, squash: bool, trim: bool, expected: Option<&[Change]>, policy: ComparisonPolicy) {
  let fragments = compare_lines_inner(before, after, policy, &Deadline::NONE).expect("no limits");
  check_consistency_lines(&fragments, before, after);

  let fragments = process_blocks(fragments, before.units(), after.units(), policy, squash, trim);
  check_consistency_lines(&fragments, before, after);

  if let Some(expected) = expected {
    check_line_changes(&fragments, expected, policy);
  }
}

fn check_consistency_line_inner(fragments: &[LineFragment], before: &Utf16Text, after: &Utf16Text) {
  assert_eq!(fragments.len(), 1);
  let fragment = &fragments[0];

  assert_eq!(fragment.start_offset1, 0);
  assert_eq!(fragment.start_offset2, 0);
  assert_eq!(fragment.end_offset1, before.len());
  assert_eq!(fragment.end_offset2, after.len());

  // It could be null if there are no common words. We do not test such cases here.
  check_consistency_diff(fragment.inner_fragments.as_deref().expect("inner fragments"));
}

pub(crate) fn check_consistency_lines(fragments: &[LineFragment], before: &Utf16Text, after: &Utf16Text) {
  for fragment in fragments {
    assert!(fragment.start_offset1 <= fragment.end_offset1);
    assert!(fragment.start_offset2 <= fragment.end_offset2);

    assert!(fragment.start_line1 <= fragment.end_line1);
    assert!(fragment.start_line2 <= fragment.end_line2);

    assert!(fragment.start_line1 != fragment.end_line1 || fragment.start_line2 != fragment.end_line2);

    assert!(fragment.end_line1 <= get_line_count(before));
    assert!(fragment.end_line2 <= get_line_count(after));

    check_line_offsets(before, fragment.start_line1, fragment.end_line1, fragment.start_offset1, fragment.end_offset1);
    check_line_offsets(after, fragment.start_line2, fragment.end_line2, fragment.start_offset2, fragment.end_offset2);

    if let Some(inner_fragments) = &fragment.inner_fragments {
      check_consistency_diff(inner_fragments);
    }
  }
}

pub(crate) fn check_consistency_diff(fragments: &[DiffFragment]) {
  for fragment in fragments {
    assert!(fragment.start_offset1 <= fragment.end_offset1);
    assert!(fragment.start_offset2 <= fragment.end_offset2);
    assert!(fragment.start_offset1 != fragment.end_offset1 || fragment.start_offset2 != fragment.end_offset2);
  }
}

pub(crate) fn check_line_offsets(document: &Utf16Text, start_line: usize, end_line: usize, start_offset: usize, end_offset: usize) {
  let line_offsets = document.line_offsets();
  if start_line == end_line {
    let offset = if start_line == get_line_count(document) {
      document.len()
    } else {
      line_offsets.line_start(start_line)
    };
    assert_eq!(offset, start_offset);
    assert_eq!(offset, end_offset);
  } else {
    assert_eq!(line_offsets.line_start(start_line), start_offset);
    let mut offset = line_offsets.line_end(end_line - 1);
    if offset < document.len() {
      offset += 1;
    }
    assert_eq!(offset, end_offset);
  }
}

pub(crate) fn check_line_changes(fragments: &[LineFragment], expected: &[Change], policy: ComparisonPolicy) {
  let changes: Vec<Change> = fragments
    .iter()
    .map(|fragment| [(fragment.start_line1, fragment.end_line1), (fragment.start_line2, fragment.end_line2)])
    .collect();
  assert_eq!(expected, changes, "Policy: {policy:?}");
}

pub(crate) fn check_diff_changes(fragments: &[DiffFragment], expected: &[Change], policy: ComparisonPolicy) {
  let changes: Vec<Change> = fragments
    .iter()
    .map(|fragment| [(fragment.start_offset1, fragment.end_offset1), (fragment.start_offset2, fragment.end_offset2)])
    .collect();
  assert_eq!(expected, changes, "Policy: {policy:?}");
}

fn check_line_matching(fragments: &[LineFragment], matchings: &[FixedBitSet; 2], policy: ComparisonPolicy) {
  let mut set1 = FixedBitSet::new();
  let mut set2 = FixedBitSet::new();
  for fragment in fragments {
    insert_range(&mut set1, fragment.start_line1, fragment.end_line1);
    insert_range(&mut set2, fragment.start_line2, fragment.end_line2);
  }

  assert_sets_equals(&matchings[0], &set1, &format!("Before, policy {policy:?}"));
  assert_sets_equals(&matchings[1], &set2, &format!("After, policy {policy:?}"));
}

fn check_diff_matching(fragments: &[DiffFragment], matchings: &[FixedBitSet; 2], policy: ComparisonPolicy) {
  let mut set1 = FixedBitSet::new();
  let mut set2 = FixedBitSet::new();
  for fragment in fragments {
    insert_range(&mut set1, fragment.start_offset1, fragment.end_offset1);
    insert_range(&mut set2, fragment.start_offset2, fragment.end_offset2);
  }

  assert_sets_equals(&matchings[0], &set1, &format!("Before, policy {policy:?}"));
  assert_sets_equals(&matchings[1], &set2, &format!("After, policy {policy:?}"));
}

fn do_test(test_type: TestType, f: impl FnOnce(&mut TestBuilder)) {
  let mut builder = TestBuilder::new(test_type);
  f(&mut builder);
  assert!(builder.is_executed);
}

pub(crate) fn lines(f: impl FnOnce(&mut TestBuilder)) {
  do_test(TestType::Line, f);
}

pub(crate) fn lines_inner(f: impl FnOnce(&mut TestBuilder)) {
  do_test(TestType::LineInner, f);
}

pub(crate) fn words(f: impl FnOnce(&mut TestBuilder)) {
  do_test(TestType::Word, f);
}

pub(crate) fn chars(f: impl FnOnce(&mut TestBuilder)) {
  do_test(TestType::Char, f);
}

pub(crate) fn chars_raw(f: impl FnOnce(&mut TestBuilder)) {
  do_test(TestType::CharRaw, f);
}

pub(crate) fn chars_smart(f: impl FnOnce(&mut TestBuilder)) {
  do_test(TestType::CharSmart, f);
}

pub(crate) fn splitter(squash: bool, trim: bool, f: impl FnOnce(&mut TestBuilder)) {
  do_test(TestType::Splitter, |builder| {
    builder.should_squash = squash;
    builder.should_trim = trim;
    f(builder);
  });
}
