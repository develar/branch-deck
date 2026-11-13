// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/BlocksComparisonUtilTest.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2018 JetBrains s.r.o. and contributors). The comparison of
// explicit blocks of `HeavyDiffTestCase.compareExplicitBlocks` is `provider::compare_line_ranges`.

use crate::cancellation::Deadline;
use crate::fragments::LineFragment;
use crate::policy::{HighlightPolicy, WhitespacePolicy};
use crate::provider::compare_line_ranges;
use crate::range::Range;
use crate::side::Side;
use crate::test_support::{insert_range, ones, parse_source};
use crate::text::Utf16Text;
use fixedbitset::FixedBitSet;
use pretty_assertions::assert_eq;

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port
#[test]
fn simple_blocks() {
  BlocksTest::new("X_a_Y_c_Z", "X_  a__Y__ c_Z")
    .blocks(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .expected(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .default();

  BlocksTest::new("X_a_Y_c_Z", "X_  a__Y__ c_Z")
    .blocks(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .expected(&[Range::new(2, 2, 2, 3), Range::new(3, 3, 4, 5)])
    .trim()
    .ignore();

  BlocksTest::new("X_a_Y_c_Z", "X_  a__Y__ c_Z")
    .blocks(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .expected(&[])
    .ignore_chunks();
}

#[test]
fn changes_outside_of_blocks_are_ignored() {
  BlocksTest::new("X_a_Y_c_Z", "N_  a__N__ c_N")
    .blocks(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .expected(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .default();

  BlocksTest::new("X_X_X_a_Y_c_Z", "N_  a__N__ c_N")
    .blocks(&[Range::new(5, 6, 4, 6)])
    .expected(&[Range::new(5, 6, 4, 6)])
    .default();
}

#[test]
fn multiple_fragments_in_a_block() {
  BlocksTest::new("X_a_b_c_Y", "X_a1_ b_c2_Y")
    .blocks(&[Range::new(1, 4, 1, 4)])
    .expected(&[Range::new(1, 4, 1, 4)])
    .default();

  BlocksTest::new("X_a_b_c_Y", "X_a1_ b_c2_Y")
    .blocks(&[Range::new(1, 4, 1, 4)])
    .expected(&[Range::new(1, 2, 1, 2), Range::new(3, 4, 3, 4)])
    .trim()
    .ignore()
    .ignore_chunks();
}

#[test]
fn fragments_respect_ignore_options() {
  BlocksTest::new("X_a_b_c_Y", "X_a1_ b_c2_Y")
    .blocks(&[Range::new(1, 4, 1, 4)])
    .expected(&[Range::new(1, 4, 1, 4)])
    .default();

  BlocksTest::new("X_Y", "X_a_c_Y")
    .blocks(&[Range::new(1, 1, 1, 3)])
    .expected(&[Range::new(1, 1, 1, 3)])
    .default()
    .trim()
    .ignore()
    .ignore_chunks();

  BlocksTest::new("X_Y", "X___Y")
    .blocks(&[Range::new(1, 1, 1, 3)])
    .expected(&[Range::new(1, 1, 1, 3)])
    .default()
    .trim()
    .ignore();

  BlocksTest::new("X_Y", "X___Y").blocks(&[Range::new(1, 1, 1, 3)]).expected(&[]).ignore_chunks();
}

#[test]
fn leading_trailing_empty_lines() {
  BlocksTest::new("X_a__Y", "X__y_Y")
    .blocks(&[Range::new(1, 3, 1, 3)])
    .expected(&[Range::new(1, 2, 1, 1), Range::new(3, 3, 2, 3)])
    .default()
    .trim()
    .ignore();

  BlocksTest::new("X_a__", "X__y_")
    .blocks(&[Range::new(1, 3, 1, 3)])
    .expected(&[Range::new(1, 2, 1, 1), Range::new(3, 3, 2, 3)])
    .default()
    .trim()
    .ignore();

  BlocksTest::new("a__", "_y_")
    .blocks(&[Range::new(0, 2, 0, 2)])
    .expected(&[Range::new(0, 1, 0, 0), Range::new(2, 2, 1, 2)])
    .default()
    .trim()
    .ignore();

  BlocksTest::new("a__", "_y_")
    .blocks(&[Range::new(0, 3, 0, 3)])
    .expected(&[Range::new(0, 1, 0, 0), Range::new(2, 2, 1, 2)])
    .default()
    .trim()
    .ignore();

  BlocksTest::new("a_", "_y")
    .blocks(&[Range::new(0, 2, 0, 2)])
    .expected(&[Range::new(0, 1, 0, 0), Range::new(2, 2, 1, 2)])
    .default()
    .trim()
    .ignore();
}

#[test]
fn inner_changes() {
  BlocksTest::with_inner("X_a_Y", "X_b_Y", "  -  ", "  -  ")
    .blocks(&[Range::new(1, 2, 1, 2)])
    .default()
    .trim()
    .ignore()
    .ignore_chunks();

  BlocksTest::with_inner("X_a_Y", "X_  a_b_Y", "     ", "  --  -- ")
    .blocks(&[Range::new(1, 2, 1, 3)])
    .default();

  BlocksTest::with_inner("X_a_Y", "X_  a_b_Y", "     ", "      -- ")
    .blocks(&[Range::new(1, 2, 1, 3)])
    .expected(&[Range::new(2, 2, 2, 3)])
    .trim()
    .ignore();

  BlocksTest::with_inner("X_a b_Y", "X_a  b_Y", "       ", "   -    ")
    .blocks(&[Range::new(1, 2, 1, 2)])
    .default()
    .trim();

  BlocksTest::with_inner("X_a b_Y", "X_a  b_Y", "       ", "        ")
    .blocks(&[Range::new(1, 2, 1, 2)])
    .ignore()
    .ignore_chunks();

  BlocksTest::with_inner("X_a_Y_c_Z", "X_ a__Y__ c_Z", "         ", "  -  -  --   ")
    .blocks(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .expected(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .default();

  BlocksTest::with_inner("X_a_Y_c_Z", "X_ a__Y__ c_Z", "         ", "     -  -    ")
    .blocks(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .expected(&[Range::new(2, 2, 2, 3), Range::new(3, 3, 4, 5)])
    .trim();

  BlocksTest::with_inner("X_a_Y_c_Z", "X_ a__Y__ c_Z", "         ", "             ")
    .blocks(&[Range::new(1, 2, 1, 3), Range::new(3, 4, 4, 6)])
    .expected(&[Range::new(2, 2, 2, 3), Range::new(3, 3, 4, 5)])
    .ignore();
}

#[test]
fn inner_changes_for_inserted_empty_line() {
  BlocksTest::with_inner("X_Y", "X_ _Y", "   ", "  -- ").blocks(&[Range::new(1, 1, 1, 2)]).trim().default();

  BlocksTest::with_inner("X_Y", "X_ _Y", "   ", "     ")
    .blocks(&[Range::new(1, 1, 1, 2)])
    .expected(&[Range::new(1, 1, 1, 2)])
    .ignore();

  BlocksTest::with_inner("X_Y", "X_ _Y", "   ", "     ")
    .blocks(&[Range::new(1, 1, 1, 2)])
    .expected(&[])
    .ignore_chunks();
}

#[test]
fn inner_changes_with_leading_trailing_empty_lines() {
  BlocksTest::with_inner("X_a__Y", "X__y_Y", "  --  ", "   -- ")
    .blocks(&[Range::new(1, 3, 1, 3)])
    .expected(&[Range::new(1, 2, 1, 1), Range::new(3, 3, 2, 3)])
    .default()
    .trim()
    .ignore();

  BlocksTest::with_inner("a__", "_y_", "-- ", " --")
    .blocks(&[Range::new(0, 2, 0, 2)])
    .expected(&[Range::new(0, 1, 0, 0), Range::new(2, 2, 1, 2)])
    .default()
    .trim()
    .ignore();
}

// jscpd:ignore-end

struct BlocksTest {
  input1: &'static str,
  input2: &'static str,
  inner: Option<[&'static str; 2]>,
  blocks: Vec<Range>,
  expected: Option<Vec<Range>>,
}

impl BlocksTest {
  const fn new(input1: &'static str, input2: &'static str) -> Self {
    Self {
      input1,
      input2,
      inner: None,
      blocks: Vec::new(),
      expected: None,
    }
  }

  /// The changed chars of each side: `-` is changed
  const fn with_inner(input1: &'static str, input2: &'static str, inner1: &'static str, inner2: &'static str) -> Self {
    Self {
      input1,
      input2,
      inner: Some([inner1, inner2]),
      blocks: Vec::new(),
      expected: None,
    }
  }

  fn expected(&mut self, ranges: &[Range]) -> &mut Self {
    self.expected = Some(ranges.to_vec());
    self
  }

  fn blocks(&mut self, ranges: &[Range]) -> &mut Self {
    self.blocks = ranges.to_vec();
    self
  }

  fn default(&mut self) -> &mut Self {
    self.do_test(WhitespacePolicy::Default);
    self
  }

  fn trim(&mut self) -> &mut Self {
    self.do_test(WhitespacePolicy::Trim);
    self
  }

  fn ignore(&mut self) -> &mut Self {
    self.do_test(WhitespacePolicy::Ignore);
    self
  }

  fn ignore_chunks(&mut self) -> &mut Self {
    self.do_test(WhitespacePolicy::IgnoreBlankLines);
    self
  }

  fn do_test(&self, policy: WhitespacePolicy) {
    assert!(self.expected.is_some() || self.inner.is_some());

    let text1 = Utf16Text::new(&parse_source(self.input1));
    let text2 = Utf16Text::new(&parse_source(self.input2));
    let compare = |highlight: HighlightPolicy| -> Vec<LineFragment> {
      compare_line_ranges(&text1, &text2, &self.blocks, policy, highlight, &Deadline::NONE)
        .expect("no limits")
        .into_iter()
        .flatten()
        .collect()
    };

    if let Some(expected) = &self.expected {
      let fragments1 = compare(HighlightPolicy::ByLine);
      let fragments2 = compare(HighlightPolicy::ByWord);

      assert_eq!(expected, &to_ranges(&fragments1), "{policy:?}, by line");
      assert_eq!(expected, &to_ranges(&fragments2), "{policy:?}, by word");
    }

    if let Some([inner1, inner2]) = self.inner {
      let fragments = compare(HighlightPolicy::ByWord);

      assert_eq!(ones(&parse_inner_expected(inner1)), ones(&parse_inner_actual(&fragments, Side::Left)), "{policy:?}, left");
      assert_eq!(ones(&parse_inner_expected(inner2)), ones(&parse_inner_actual(&fragments, Side::Right)), "{policy:?}, right");
    }
  }
}

fn parse_inner_expected(inner: &str) -> FixedBitSet {
  let mut set = FixedBitSet::new();
  for (index, c) in inner.chars().enumerate() {
    if c == '-' {
      set.grow_and_insert(index);
    }
  }
  set
}

fn parse_inner_actual(fragments: &[LineFragment], side: Side) -> FixedBitSet {
  let mut set = FixedBitSet::new();
  for fragment in fragments {
    let ranges: Vec<Range> = match &fragment.inner_fragments {
      Some(inner) => inner
        .iter()
        .map(|it| {
          Range::new(
            fragment.start_offset1 + it.start_offset1,
            fragment.start_offset1 + it.end_offset1,
            fragment.start_offset2 + it.start_offset2,
            fragment.start_offset2 + it.end_offset2,
          )
        })
        .collect(),
      None => vec![fragment.offsets()],
    };
    for range in ranges {
      let (start, end) = side.select((range.start1, range.end1), (range.start2, range.end2));
      insert_range(&mut set, start, end);
    }
  }
  set
}

fn to_ranges(fragments: &[LineFragment]) -> Vec<Range> {
  fragments.iter().map(LineFragment::lines).collect()
}
