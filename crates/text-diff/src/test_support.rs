//! Helpers of the tests: the parsing of the texts and matchings of the ported tests, the bit sets they compare, and
//! the seed of the property tests.

use crate::text::Utf16Text;
use fixedbitset::FixedBitSet;
use proptest::test_runner::{Config, RngSeed};

/// The configuration of a property test: a fixed seed, so a run is the same in CI and locally, and `cases` cases. A
/// failure shrinks to a minimal case and is saved in `proptest-regressions/`, which later runs replay first.
pub(crate) fn proptest_config(cases: u32) -> Config {
  Config {
    cases,
    rng_seed: RngSeed::Fixed(0x7E57_D1FF),
    ..Config::default()
  }
}

/// A text of up to `max_lines` lines from `contents`, each ended by one of `ends`, the last one with or without its end
pub(crate) fn text_of_lines(contents: Vec<&'static str>, ends: Vec<&'static str>, max_lines: usize) -> impl proptest::strategy::Strategy<Value = String> {
  use proptest::prelude::{Strategy, any};
  let lines = proptest::collection::vec(proptest::sample::select(contents), 0..max_lines);
  let ends = proptest::collection::vec(proptest::sample::select(ends), max_lines);
  (lines, ends, any::<bool>()).prop_map(|(lines, ends, final_end)| {
    let mut text = String::new();
    for (index, line) in lines.iter().enumerate() {
      text.push_str(line);
      if index + 1 < lines.len() || final_end {
        text.push_str(ends[index]);
      }
    }
    text
  })
}

/// Sets `[start, end)`, and grows the set when it is shorter
pub(crate) fn insert_range(set: &mut FixedBitSet, start: usize, end: usize) {
  set.grow(end);
  set.insert_range(start..end);
}

/// The set bits in order, for an equality that does not depend on the length of a set
pub(crate) fn ones(set: &FixedBitSet) -> Vec<usize> {
  set.ones().collect()
}

// Ported and modified from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/DiffTestCase.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2022 JetBrains s.r.o. and contributors): the parsing and the
// assertions. A `Utf16Text` stands for a `Document`.

/// `_` stands for `\n` in the texts of a test
pub(crate) fn parse_source(text: &str) -> String {
  text.replace('_', "\n")
}

/// The changed chars of a text: a char other than space is changed, `.` is skipped
pub(crate) fn parse_matching(matching: &str, text: &Utf16Text) -> FixedBitSet {
  let pattern: Vec<u16> = matching.encode_utf16().filter(|&c| c != u16::from(b'.')).collect();
  assert_eq!(pattern.len(), text.len(), "the matching {matching:?} has another length than its text");

  let mut set = FixedBitSet::new();
  for (i, &c) in pattern.iter().enumerate() {
    if c != u16::from(b' ') {
      set.grow_and_insert(i);
    }
  }
  set
}

/// The changed lines of a text: `_` and `*` end a line, a line with a char other than space and `_` is changed
pub(crate) fn parse_line_matching(matching: &str, text: &Utf16Text) -> FixedBitSet {
  let matching: Vec<u16> = matching.encode_utf16().collect();
  assert_eq!(matching.len(), text.len());

  let is_separator = |c: &u16| *c == u16::from(b'_') || *c == u16::from(b'*');
  let lines1: Vec<&[u16]> = matching.split(is_separator).collect();
  let lines2: Vec<&[u16]> = text.units().split(|&c| c == u16::from(b'\n')).collect();
  assert_eq!(lines1.len(), lines2.len());
  for (i, (line1, line2)) in lines1.iter().zip(&lines2).enumerate() {
    assert_eq!(line1.len(), line2.len(), "line {i}");
  }

  let mut set = FixedBitSet::new();

  let mut index = 0;
  let mut line_number = 0;
  while index < matching.len() {
    let end = matching[index..].iter().position(is_separator).map_or(matching.len(), |position| index + position + 1);

    let line = &matching[index..end];
    if line.iter().any(|&c| c != u16::from(b' ') && c != u16::from(b'_')) {
      assert!(!line.contains(&u16::from(b' ')));
      set.grow_and_insert(line_number);
    }
    line_number += 1;
    index = end;
  }

  set
}

/// The line count of a test text, as `DiffUtil.getLineCount` of a document
pub(crate) const fn get_line_count(text: &Utf16Text) -> usize {
  text.line_offsets().line_count()
}

pub(crate) fn assert_sets_equals(expected: &FixedBitSet, actual: &FixedBitSet, message: &str) {
  let rendered: String = (0..=actual.len()).map(|i| if actual.contains(i) { '-' } else { ' ' }).collect();
  assert_eq!(ones(expected), ones(actual), "{message}: \"{rendered}\"");
}

/// The emoji of `DiffTestCase`, two UTF-16 code units each
pub(crate) const CH_SMILE: &str = "\u{1F602}";
pub(crate) const CH_GUN: &str = "\u{1F52B}";
pub(crate) const CH_MAN: &str = "\u{1F9D2}";
pub(crate) const CH_FACE: &str = "\u{1F92B}";
