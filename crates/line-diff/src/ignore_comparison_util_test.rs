// Ported from `community/platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/IgnoreComparisonUtilTest.kt` of
// IntelliJ Platform (Apache License 2.0, Copyright 2000-2020 JetBrains s.r.o. and contributors).

use crate::cancellation::Deadline;
use crate::comparison_util_test_base::{Change, del, ins, modify};
use crate::fragments::LineFragment;
use crate::manager::{collect_ignored_ranges, compare_lines_with_ignored_ranges, compare_lines_with_ignored_ranges_in_range};
use crate::policy::InnerFragmentsPolicy;
use crate::range::Range;
use crate::test_support::{insert_range, ones};
use crate::text::Utf16Text;
use fixedbitset::FixedBitSet;
use pretty_assertions::assert_eq;

// jscpd:ignore-start: the cases repeat inputs of the Kotlin test, the oracle of the port
#[test]
fn simple() {
  IgnoreTest::new("", "", "", "", "", "").run();

  IgnoreTest::new("X", "Y", " ", " ", "-", "-").run();

  IgnoreTest::new("X", "Y", "+", " ", " ", "-").run();

  IgnoreTest::new("X", "Y", " ", "+", "-", " ").run();

  IgnoreTest::new("X", "Y", "+", "+", " ", " ").run();

  IgnoreTest::new("X", "", " ", "", "-", "").run();

  IgnoreTest::new("X", "", "+", "", " ", "").run();

  IgnoreTest::new("", "Y", "", "+", "", " ").run();
}

#[test]
fn spaces() {
  IgnoreTest::new("X Y Z", "A B Z", "++   ", "++   ", "  -  ", "  -  ").run();

  IgnoreTest::new("X Y Z", "A B Z", "   ++", "   ++", "---  ", "---  ").run();

  IgnoreTest::new("X Y Z", "A Y C", "++   ", "++   ", "    -", "    -").run();

  IgnoreTest::new("X Y Z", "A Y C", "+++++", "+++++", "     ", "     ").run();

  IgnoreTest::new("X Y Z", "A B C", "+   +", "+   +", "-----", "-----").run();

  IgnoreTest::new("A Y C", "A B C", "+   +", "+   +", "  -  ", "  -  ").run();

  IgnoreTest::new("A  B", "A B", " ++ ", " + ", "    ", "   ").run();

  IgnoreTest::new("A  B", "A X", " ++ ", " + ", "   -", "  -").run();

  IgnoreTest::new(" A  B", "X B ", "+ ++ ", " + +", " -   ", "-   ").run();

  IgnoreTest::new("A   B", "A B", "  +  ", "   ", " -   ", "   ").run();

  IgnoreTest::new("A   B", "A B", "   + ", "   ", " --  ", "   ").run();
}

#[test]
fn trim() {
  IgnoreTest::new("A B_", "", "    ", "", "----", "").run();

  IgnoreTest::new("A B_", "", "++++", "", "    ", "").run();

  IgnoreTest::new("A B_", "", "   +", "", "----", "").run();

  IgnoreTest::new("A B_", "", "+++ ", "", "----", "").run();

  IgnoreTest::new("A B_", "", "+  +", "", "----", "").run();

  IgnoreTest::new("A B_", "", "+ ++", "", "----", "").run();

  IgnoreTest::new("A B_", "", " +++", "", "----", "").run();

  IgnoreTest::new("A_B_C_", "", " +++++", "", "--    ", "").run();

  IgnoreTest::new("A_B_C_", "", "++++ +", "", "    --", "").run();
}

#[test]
fn lines() {
  IgnoreTest::new("X_", "X_Y_", "  ", "    ", "  ", "  --").changed_lines_number(0, 1).run();

  IgnoreTest::new("X_", "X_Y_", "  ", "  ++", "  ", "    ").run();

  IgnoreTest::new("X_", "X_Y_", "  ", "++  ", "- ", "  - ").changed_lines(&[modify(0, 1, 1, 1)]).run();

  IgnoreTest::new("X_Y_", "X_", "    ", "  ", "  --", "  ").run();

  IgnoreTest::new("X_Y_", "X_", "  ++", "  ", "    ", "  ").run();

  IgnoreTest::new("X_Y_Z", "X_B_Z", "     ", "     ", "  -  ", "  -  ").changed_lines_number(1, 1).run();

  IgnoreTest::new("X_Y_Z", "X_Y_Z", "     ", "     ", "     ", "     ").run();

  IgnoreTest::new("X_Y_Z", "X_Y_Z", " + + ", " + + ", "     ", "     ").run();

  IgnoreTest::new("X_Y_Z", "A_Y_Z", "     ", "     ", "-    ", "-    ").run();

  IgnoreTest::new("X_Y_Z", "A_Y_Z", "++   ", "     ", "     ", "--   ").changed_lines(&[ins(1, 0, 1)]).run();

  IgnoreTest::new("X_Y_Z", "A_Y_Z", "++   ", "++   ", "     ", "     ").run();

  IgnoreTest::new("X_Y_Z", "A_Y_Z", "     ", " ++  ", "-    ", "-    ").run();

  IgnoreTest::new("X_Y_Z", "X_Y_C", "     ", "     ", "    -", "    -").run();

  IgnoreTest::new("X_Y_Z", "X_Y_C", "     ", "    +", "    -", "     ").run();

  IgnoreTest::new("X_Y_Z", "X_Y_C", "    +", "     ", "     ", "    -").run();

  IgnoreTest::new("X_Y_Z", "X_B_Z", "     ", "     ", "  -  ", "  -  ").run();

  IgnoreTest::new("X_Y_Z", "X_B_Z", "  +  ", "  +  ", "     ", "     ").run();

  IgnoreTest::new("X_Y_Z", "X_B_Z", "  +  ", "     ", "     ", "  -  ").run();

  IgnoreTest::new("X_Y_Z", "X_B_Z", "  +  ", "     ", "     ", "  -  ").run();
}

#[test]
fn trim_lines() {
  IgnoreTest::new("X_W_Y_W_Z", "X_B_Z", " +++ +++ ", " + + ", "    -    ", "  -  ")
    .changed_lines_number(1, 1)
    .run();

  IgnoreTest::new("X_W_W_Z", "X_Z", " +++++ ", " + ", "       ", "   ").changed_lines_number(0, 0).run();

  IgnoreTest::new(
    "X_W 1 W_Y_W 2 W_Z",
    "X_W 3 W_B_W 4 W_Z",
    "    +       +    ",
    "    +       +    ",
    "        -        ",
    "        -        ",
  )
  .changed_lines_number(1, 1)
  .run();

  IgnoreTest::new("X_W W W_Z", "X_W W_ W_Z", " + + + + ", " + + ++ + ", "         ", "          ")
    .changed_lines_number(0, 0)
    .run();

  IgnoreTest::new("X_W M W_Z", "X_W W_ W_Z", " + + + + ", " + + ++ + ", "    -    ", "    -     ")
    .changed_lines_number(1, 2)
    .run();
}

#[test]
fn no_inner_changes() {
  IgnoreTest::new("X_W_Y_W_Z", "X_B_Z", " +++ +++ ", " + + ", "    --   ", "  -- ")
    .changed_lines_number(1, 1)
    .no_inner_changes()
    .run();

  IgnoreTest::new("X_W_W_Z", "X_Z", " +++++ ", " + ", "       ", "   ")
    .changed_lines_number(0, 0)
    .no_inner_changes()
    .run();

  IgnoreTest::new(
    "X_W 1 W_Y_W 2 W_Z",
    "X_W 3 W_B_W 4 W_Z",
    "    +       +    ",
    "    +       +    ",
    "        --       ",
    "        --       ",
  )
  .changed_lines_number(1, 1)
  .no_inner_changes()
  .run();

  IgnoreTest::new("X_W W W_Z", "X_W W_ W_Z", " + + + + ", " + + ++ + ", "  ------ ", "  ------- ")
    .changed_lines_number(1, 2)
    .no_inner_changes()
    .run();

  IgnoreTest::new("X_W M W_Z", "X_W W_ W_Z", " + + + + ", " + + ++ + ", "  ------ ", "  ------- ")
    .changed_lines_number(1, 2)
    .no_inner_changes()
    .run();
}

#[test]
fn trim_vs_trim_expand_for_inner_ranges() {
  IgnoreTest::new("X M Y Z", "A B Y C", "+     +", "+     +", " --    ", " --    ").run();

  IgnoreTest::new("X M Y Z", "A B Y C", "  +    ", "  +    ", "--    -", "--    -").run();

  IgnoreTest::new("X MZ Y Z", "A BZ Y C", "  +     ", "  +     ", "----   -", "----   -").run();

  IgnoreTest::new("ZX M Y Z", "A B Y C", " +     +", "+     +", "----    ", " --    ").run();

  IgnoreTest::new("X Y", "XY", " + ", "  ", "---", "--").changed_lines_number(1, 1).run();
}

#[test]
fn java_samples() {
  IgnoreTest::new(
    "System . out.println(\"Hello world\");",
    "System.out.println(\"Hello world\");",
    "      + +            .            .   ",
    "                   .            .   ",
    "                     .            .   ",
    "                   .            .   ",
  )
  .changed_lines_number(0, 0)
  .run();

  IgnoreTest::new(
    " System . out . println(\"Hello  world\") ; ",
    "System.out.println(\"Hello world\");",
    "+      + +   + +   .                  .  + +",
    "                   .            .   ",
    "                        .      -      .     ",
    "                   .            .   ",
  )
  .run();

  IgnoreTest::new(
    "import java.util.Random;_import java.util.List;__class Test {_}",
    "import java.util.List;_import java.util.Timer;__class Foo {_}",
    "+++++++++++++++++++++++++++++++++++++++++++++++++              ",
    "++++++++++++++++++++++++++++++++++++++++++++++++             ",
    "                                                       ----    ",
    "                                                      ---    ",
  )
  .changed_lines_number(1, 1)
  .run();

  IgnoreTest::new(
    "final_int x = 0;",
    "final int Y = 0;",
    "     +   + + +  ",
    "     +   + + +  ",
    "          -     ",
    "          -     ",
  )
  .changed_lines_number(2, 1)
  .changed_lines(&[modify(0, 0, 2, 1)])
  .run();

  IgnoreTest::new("int X = 0;", "intX = 0;", "   + + +  ", "    + +  ", "-----     ", "----     ")
    .changed_lines_number(1, 1)
    .run();

  IgnoreTest::new(
    "import xx.x;_import xx.y;_import xx.z;",
    "import xx.x;_// import xx.y;_import xx.z;",
    "++++++++++++++++++++++++++++++++++++++",
    "+++++++++++++               +++++++++++++",
    "                                      ",
    "             ----------------            ",
  )
  .changed_lines_number(0, 1)
  .run();

  IgnoreTest::new(
    "import xx.x;_import xx.y;_import xx.z;_import xx.a;",
    "//import xx.x;_import xx.y;_import xx.z;_//import xx.a;",
    "+++++++++++++++++++++++++++++++++++++++++++++++++++",
    "              +++++++++++++++++++++++++++              ",
    "                                                   ",
    "---------------                          --------------",
  )
  .changed_lines_number(0, 2)
  .run();

  IgnoreTest::new(
    "foo();_bar 'text';",
    "foo();_// TODO: bar 'text'",
    "          +       ",
    "      ++++++++++++++++++++",
    "       -----------",
    "                          ",
  )
  .changed_lines(&[del(1, 2, 1)])
  .changed_lines_number(1, 0)
  .run();

  IgnoreTest::new(
    "import x.A;_import x.B;_import x.C;__@C_class Test { }_",
    "import x.B;__@x.C_class Test {}_",
    "+++++++++++++++++++++++++++++++++++++  +     +    + + +",
    "+++++++++++++    +     +    +  +",
    "                                                       ",
    "              --                ",
  )
  .changed_lines_number(1, 1)
  .changed_lines(&[modify(4, 2, 1, 1)])
  .run();
}

#[test]
fn java_bad_samples() {
  IgnoreTest::new(
    "System.out.println (\"Hello  world\");",
    "System.out.println(\"Hello world\");",
    "              .   +               .   ",
    "                   .            .   ",
    "                  - .      -      .   ",
    "                   .            .   ",
  )
  .run();

  IgnoreTest::new(
    "private static final Cleaner NOP = () -> { };_",
    "private static final Cleaner NOP = () -> { _ };_",
    "       +      +     +       +   + +  +  + +  +",
    "       +      +     +       +   + +  +  + +++  +",
    "                                           -- ",
    "                                             -- ",
  )
  .run();

  IgnoreTest::new(
    "private static final Cleaner NOP = () -> { };",
    "private static final Cleaner NOP = () -> { _ };",
    "       +      +     +       +   + +  +  + +  ",
    "       +      +     +       +   + +  +  + +++  ",
    "                                             ",
    "                                               ",
  )
  .run();
}

#[test]
fn explicit_blocks() {
  IgnoreTest::new("X_a_Y_b_Z", "X_a 1 c_Y_b 1 c_Z", "         ", "   +++     +++   ", "         ", "      -          ")
    .ranged(Range::new(1, 2, 1, 2))
    .changed_lines_number(1, 1)
    .run();

  IgnoreTest::new("X_a_Y_b_Z", "X_a 1 c_Y_b 1 c_Z", "         ", "   ++++    +++   ", "         ", "                 ")
    .ranged(Range::new(1, 2, 1, 2))
    .changed_lines_number(0, 0)
    .run();

  IgnoreTest::new("X_a_Y_b_Z", "X_a 1 c_Y_b 1 c_Z", "         ", "              +  ", "         ", "           ---   ")
    .ranged(Range::new(3, 4, 3, 4))
    .changed_lines_number(1, 1)
    .run();

  IgnoreTest::new("X_a_Y_b_Z", "X_a 1 c_Y_b 1 c_Z", "         ", "              +  ", "  -      ", "          ----   ")
    .ranged(Range::new(1, 2, 3, 4))
    .changed_lines_number(1, 1)
    .run();

  IgnoreTest::new("X_a_Y_b_Z", "Y_b 1 c_Z", "         ", "      +  ", "         ", "   ---   ")
    .ranged(Range::new(3, 4, 1, 2))
    .changed_lines_number(1, 1)
    .run();

  IgnoreTest::new("X_a_Y_b_Z", "Y_c_d_b_Z", "         ", "  ++++   ", "         ", "         ")
    .ranged(Range::new(3, 4, 1, 4))
    .changed_lines_number(0, 0)
    .run();

  IgnoreTest::new("X_a_Y_Z", "Y_c_d_Z", "       ", "  ++++ ", "       ", "       ")
    .ranged(Range::new(3, 3, 1, 3))
    .changed_lines_number(0, 0)
    .run();

  IgnoreTest::new("X_a_Y_Z", "Y_c_d_Z", "       ", "       ", "       ", "  ---- ")
    .ranged(Range::new(3, 3, 1, 3))
    .changed_lines_number(0, 2)
    .run();

  IgnoreTest::new(
    "X_W 1 W_Y_W 2 W_Z",
    "X_W 3 W_B_W 4 W_Z",
    "    +       +    ",
    "    +       +    ",
    "        --       ",
    "        --       ",
  )
  .ranged(Range::new(2, 4, 2, 4))
  .changed_lines_number(1, 1)
  .no_inner_changes()
  .run();

  IgnoreTest::new(
    "X_W 1 W_Y_W 2 W_Z",
    "X_W 3 W_B_W 4 W_Z",
    "    +       +    ",
    "    +       +    ",
    "        --       ",
    "        --       ",
  )
  .ranged(Range::new(1, 5, 1, 5))
  .changed_lines_number(1, 1)
  .no_inner_changes()
  .run();

  IgnoreTest::new(
    "X_W 1 W_Y_W 2 W_Z",
    "X_W 3 W_B_W 4 W_Z",
    "    +       +    ",
    "    +       +    ",
    "                 ",
    "                 ",
  )
  .ranged(Range::new(1, 2, 3, 4))
  .changed_lines_number(0, 0)
  .no_inner_changes()
  .run();

  IgnoreTest::new("X_a Y_Z", "Y_aY_Z", "   + + ", "    + ", "  ---  ", "  --  ")
    .ranged(Range::new(1, 2, 1, 2))
    .changed_lines_number(1, 1)
    .run();

  IgnoreTest::new("X_a Y_Z", "X_aY_Z", "   + + ", "    + ", "  ---  ", "  --  ")
    .ranged(Range::new(1, 2, 1, 2))
    .changed_lines_number(1, 1)
    .run();
}

// jscpd:ignore-end

/// Two texts (`_` for `\n`), their ignored chars (`+`) and the expected changed chars (`-`); `.` is skipped
struct IgnoreTest {
  input1: &'static str,
  input2: &'static str,
  ignored1: String,
  ignored2: String,
  result1: String,
  result2: String,
  inner_policy: InnerFragmentsPolicy,
  changed_lines_number: Option<(usize, usize)>,
  range: Option<Range>,
  changed_lines: Option<Vec<Change>>,
}

impl IgnoreTest {
  fn new(input1: &'static str, input2: &'static str, ignored1: &str, ignored2: &str, result1: &str, result2: &str) -> Self {
    let skip_dots = |text: &str| text.chars().filter(|&c| c != '.').collect::<String>();
    Self {
      input1,
      input2,
      ignored1: skip_dots(ignored1),
      ignored2: skip_dots(ignored2),
      result1: skip_dots(result1),
      result2: skip_dots(result2),
      inner_policy: InnerFragmentsPolicy::Words,
      changed_lines_number: None,
      range: None,
      changed_lines: None,
    }
  }

  fn no_inner_changes(&mut self) -> &mut Self {
    self.inner_policy = InnerFragmentsPolicy::None;
    self
  }

  fn changed_lines_number(&mut self, lines1: usize, lines2: usize) -> &mut Self {
    self.changed_lines_number = Some((lines1, lines2));
    self
  }

  fn ranged(&mut self, range: Range) -> &mut Self {
    self.range = Some(range);
    self
  }

  fn changed_lines(&mut self, expected: &[Change]) -> &mut Self {
    self.changed_lines = Some(expected.to_vec());
    self
  }

  fn run(&self) {
    let length = |text: &str| text.encode_utf16().count();
    assert_eq!(length(self.input1), length(&self.ignored1));
    assert_eq!(length(self.input1), length(&self.result1));
    assert_eq!(length(self.input2), length(&self.ignored2));
    assert_eq!(length(self.input2), length(&self.result2));

    let text1 = Utf16Text::new(&self.input1.replace('_', "\n"));
    let text2 = Utf16Text::new(&self.input2.replace('_', "\n"));

    let ignored1 = collect_ignored_ranges(&parse_ignored(&self.ignored1));
    let ignored2 = collect_ignored_ranges(&parse_ignored(&self.ignored2));

    let result = match self.range {
      Some(range) => compare_lines_with_ignored_ranges_in_range(range, &text1, &text2, &ignored1, &ignored2, self.inner_policy, &Deadline::NONE),
      None => compare_lines_with_ignored_ranges(&text1, &text2, &ignored1, &ignored2, self.inner_policy, &Deadline::NONE),
    }
    .expect("no limits");

    let expected = [parse_expected(&self.result1), parse_expected(&self.result2)].map(|set| ones(&set));
    let actual = parse_actual(&result).map(|set| ones(&set));
    assert_eq!(expected, actual, "{} - {}", self.input1, self.input2);

    if let Some(changed_lines_number) = self.changed_lines_number {
      assert_eq!(changed_lines_number, count_changed_lines(&result));
    }

    if let Some(changed_lines) = &self.changed_lines {
      let actual_changed_lines: Vec<Change> = result
        .iter()
        .map(|fragment| [(fragment.start_line1, fragment.end_line1), (fragment.start_line2, fragment.end_line2)])
        .collect();
      assert_eq!(changed_lines, &actual_changed_lines);
    }
  }
}

fn parse_ignored(ignored: &str) -> Vec<(usize, usize)> {
  assert!(ignored.chars().all(|c| c == ' ' || c == '+'));
  ignored.chars().enumerate().filter(|(_, c)| *c == '+').map(|(index, _)| (index, index + 1)).collect()
}

fn parse_expected(result: &str) -> FixedBitSet {
  assert!(result.chars().all(|c| c == ' ' || c == '-'));

  let mut set = FixedBitSet::new();
  for (index, c) in result.chars().enumerate() {
    if c == '-' {
      set.grow_and_insert(index);
    }
  }
  set
}

fn parse_actual(result: &[LineFragment]) -> [FixedBitSet; 2] {
  let mut set1 = FixedBitSet::new();
  let mut set2 = FixedBitSet::new();
  for fragment in result {
    match &fragment.inner_fragments {
      None => {
        insert_range(&mut set1, fragment.start_offset1, fragment.end_offset1);
        insert_range(&mut set2, fragment.start_offset2, fragment.end_offset2);
      }
      Some(inner) => {
        for inner in inner {
          insert_range(&mut set1, fragment.start_offset1 + inner.start_offset1, fragment.start_offset1 + inner.end_offset1);
          insert_range(&mut set2, fragment.start_offset2 + inner.start_offset2, fragment.start_offset2 + inner.end_offset2);
        }
      }
    }
  }
  [set1, set2]
}

fn count_changed_lines(result: &[LineFragment]) -> (usize, usize) {
  result.iter().fold((0, 0), |(count1, count2), it| {
    (count1 + it.end_line1 - it.start_line1, count2 + it.end_line2 - it.start_line2)
  })
}
