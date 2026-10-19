// Ported and modified from `community/platform/util/diff/src/com/intellij/diff/comparison/ByLineRt.kt` of IntelliJ
// Platform (Apache License 2.0, Copyright 2000-2025 JetBrains s.r.o. and contributors). The merge fragments are
// `MergeRange`s, so `convertIntoMergeLineFragments` without a range has nothing to do.

//! The comparison of two or three lists of lines.

use crate::cancellation::{Deadline, DiffTooBig};
use crate::change_corrector::{ChangeCorrector, SmartLineChangeCorrector};
use crate::chunk_optimizer::optimize_line_chunks;
use crate::comparison_merge_util::{build_merge, build_simple};
use crate::comparison_util::{hash_code, is_equals, unimportant_line_char_count};
use crate::iterables::diff_iterable::DiffIterable;
use crate::iterables::diff_iterable_util::{ExpandChangeBuilder, create, diff, fair};
use crate::merge_range::MergeRange;
use crate::policy::ComparisonPolicy;
use crate::trim_util::{expand, is_space_enter_or_tab};
use std::hash::{Hash, Hasher};

/// Compares two lists of lines, the line contents without their `\n`
pub fn compare(lines1: &[&[u16]], lines2: &[&[u16]], policy: ComparisonPolicy, deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  deadline.check_canceled()?;
  do_compare(&get_lines(lines1, policy), &get_lines(lines2, policy), policy, deadline)
}

/// Compares three lists of lines, the base in the middle: a change wherever a side differs from the base under the
/// policy
pub fn compare_three(lines1: &[&[u16]], lines2: &[&[u16]], lines3: &[&[u16]], policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<MergeRange>, DiffTooBig> {
  deadline.check_canceled()?;
  do_compare_three(&get_lines(lines1, policy), &get_lines(lines2, policy), &get_lines(lines3, policy), policy, deadline, false)
}

/// As `compare_three`, for a merge: a change that the policy ignores stays a change of its own, outside the conflicts
pub fn merge(lines1: &[&[u16]], lines2: &[&[u16]], lines3: &[&[u16]], policy: ComparisonPolicy, deadline: &Deadline) -> Result<Vec<MergeRange>, DiffTooBig> {
  deadline.check_canceled()?;
  do_compare_three(&get_lines(lines1, policy), &get_lines(lines2, policy), &get_lines(lines3, policy), policy, deadline, true)
}

/// The merge ranges of a part of the texts, shifted to the lines of the whole texts
pub fn convert_into_merge_line_fragments(conflicts: &[MergeRange], range: MergeRange) -> Vec<MergeRange> {
  conflicts
    .iter()
    .map(|conflict| {
      MergeRange::new(
        conflict.start1 + range.start1,
        conflict.end1 + range.start1,
        conflict.start2 + range.start2,
        conflict.end2 + range.start2,
        conflict.start3 + range.start3,
        conflict.end3 + range.start3,
      )
    })
    .collect()
}

//
// Impl
//

pub(crate) fn do_compare(lines1: &[Line<'_>], lines2: &[Line<'_>], policy: ComparisonPolicy, deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  deadline.check_canceled()?;

  if policy == ComparisonPolicy::IgnoreWhitespaces {
    let changes = compare_smart(lines1, lines2, deadline)?;
    let changes = optimize_line_chunks(lines1, lines2, &changes);
    Ok(expand_ranges(lines1, lines2, &changes))
  } else {
    let iw_lines1 = convert_mode(lines1, ComparisonPolicy::IgnoreWhitespaces);
    let iw_lines2 = convert_mode(lines2, ComparisonPolicy::IgnoreWhitespaces);

    let iw_changes = compare_smart(&iw_lines1, &iw_lines2, deadline)?;
    let iw_changes = optimize_line_chunks(lines1, lines2, &iw_changes);
    Ok(correct_changes_second_step(lines1, lines2, &iw_changes))
  }
}

/// The three-way comparison; with `keep_ignored_changes`, a change the policy ignores is not dropped, but it is not part
/// of a conflict either
pub(crate) fn do_compare_three(
  lines1: &[Line<'_>],
  lines2: &[Line<'_>],
  lines3: &[Line<'_>],
  policy: ComparisonPolicy,
  deadline: &Deadline,
  keep_ignored_changes: bool,
) -> Result<Vec<MergeRange>, DiffTooBig> {
  deadline.check_canceled()?;

  let iw_lines1 = convert_mode(lines1, ComparisonPolicy::IgnoreWhitespaces);
  let iw_lines2 = convert_mode(lines2, ComparisonPolicy::IgnoreWhitespaces);
  let iw_lines3 = convert_mode(lines3, ComparisonPolicy::IgnoreWhitespaces);

  let iw_changes1 = compare_smart(&iw_lines2, &iw_lines1, deadline)?;
  let iw_changes1 = optimize_line_chunks(lines2, lines1, &iw_changes1);
  let iterable1 = correct_changes_second_step(lines2, lines1, &iw_changes1);

  let iw_changes2 = compare_smart(&iw_lines2, &iw_lines3, deadline)?;
  let iw_changes2 = optimize_line_chunks(lines2, lines3, &iw_changes2);
  let iterable2 = correct_changes_second_step(lines2, lines3, &iw_changes2);

  Ok(if keep_ignored_changes && policy != ComparisonPolicy::Default {
    build_merge(&iterable1, &iterable2, &|index1, index2, index3| {
      equals_default_policy(lines1, lines2, lines3, index1, index2, index3)
    })
  } else {
    build_simple(&iterable1, &iterable2)
  })
}

fn equals_default_policy(lines1: &[Line<'_>], lines2: &[Line<'_>], lines3: &[Line<'_>], index1: usize, index2: usize, index3: usize) -> bool {
  let (content1, content2, content3) = (lines1[index1].content, lines2[index2].content, lines3[index3].content);
  is_equals(content2, content1, ComparisonPolicy::Default) && is_equals(content2, content3, ComparisonPolicy::Default)
}

fn correct_changes_second_step(lines1: &[Line<'_>], lines2: &[Line<'_>], changes: &DiffIterable) -> DiffIterable {
  // We want to fix invalid matching here:
  //
  // .{        ..{
  // ..{   vs  ...{
  // ...{
  //
  // first step will return matching (0,2)-(0,2). And we should adjust it to (1,3)-(0,2)
  //
  // From the other hand, we don't want to reduce number of IW-matched lines.
  //
  // .{         ...{
  // ..{    vs  ..{
  // ...{       .{
  //
  // first step will return (0,3)-(0,3) and 'correcting' it to (0,1)-(2,3) is wrong (and it will break ByWord highlighting).
  //
  // Idea:
  // 1. lines are matched at first step and equal -> match them
  // 2. lines are not matched at first step -> do not match them
  // 3. lines are matched at first step and not equal ->
  //    a. find all IW-equal lines in the same unmatched block
  //    b. find a maximum matching between them, maximising amount of equal pairs in it
  //    c. match equal lines using result of the previous step

  let mut step = SecondStep {
    lines1,
    lines2,
    builder: ExpandChangeBuilder::new(lines1, lines2),
    sample: None,
    last1: 0,
    last2: 0,
  };
  step.run(changes);
  fair(step.builder.finish())
}

struct SecondStep<'a, 'b> {
  lines1: &'a [Line<'b>],
  lines2: &'a [Line<'b>],
  builder: ExpandChangeBuilder<'a, Line<'b>>,
  sample: Option<&'b [u16]>,
  last1: usize,
  last2: usize,
}

impl SecondStep<'_, '_> {
  fn run(&mut self, changes: &DiffIterable) {
    for range in changes.unchanged() {
      let count = range.end1 - range.start1;
      for i in 0..count {
        let index1 = range.start1 + i;
        let index2 = range.start2 + i;
        let line1 = self.lines1[index1];
        let line2 = self.lines2[index2];

        if !self.sample.is_some_and(|sample| is_equals(sample, line1.content, ComparisonPolicy::IgnoreWhitespaces)) {
          self.flush(index1, index2);
          if line1 == line2 {
            self.builder.mark_equal(index1, index2);
          } else {
            self.sample = Some(line1.content);
          }
        }
      }
    }
    self.flush(changes.length1(), changes.length2());
  }

  fn flush(&mut self, line1: usize, line2: usize) {
    let Some(sample) = self.sample else {
      return;
    };

    let start1 = self.last1.max(self.builder.index1());
    let start2 = self.last2.max(self.builder.index2());

    let mut sub_lines1 = Vec::new();
    let mut sub_lines2 = Vec::new();
    for i in start1..line1 {
      if is_equals(sample, self.lines1[i].content, ComparisonPolicy::IgnoreWhitespaces) {
        sub_lines1.push(i);
        self.last1 = i + 1;
      }
    }
    for i in start2..line2 {
      if is_equals(sample, self.lines2[i].content, ComparisonPolicy::IgnoreWhitespaces) {
        sub_lines2.push(i);
        self.last2 = i + 1;
      }
    }

    debug_assert!(!sub_lines1.is_empty() && !sub_lines2.is_empty());
    self.align_exact_matching(&sub_lines1, &sub_lines2);

    self.sample = None;
  }

  fn align_exact_matching(&mut self, sub_lines1: &[usize], sub_lines2: &[usize]) {
    let n = sub_lines1.len().max(sub_lines2.len());
    // we use brute-force algorithm (C_n_k). This will limit search space by ~250 cases.
    let skip_aligning = n > 10 || sub_lines1.len() == sub_lines2.len(); // nothing to do

    if skip_aligning {
      for (&index1, &index2) in sub_lines1.iter().zip(sub_lines2) {
        if self.lines1[index1] == self.lines2[index2] {
          self.builder.mark_equal(index1, index2);
        }
      }
      return;
    }

    if sub_lines1.len() < sub_lines2.len() {
      let matching = get_best_matching_alignment(sub_lines1, sub_lines2, self.lines1, self.lines2);
      for (i, &index1) in sub_lines1.iter().enumerate() {
        let index2 = sub_lines2[matching[i]];
        if self.lines1[index1] == self.lines2[index2] {
          self.builder.mark_equal(index1, index2);
        }
      }
    } else {
      let matching = get_best_matching_alignment(sub_lines2, sub_lines1, self.lines2, self.lines1);
      for (i, &index2) in sub_lines2.iter().enumerate() {
        let index1 = sub_lines1[matching[i]];
        if self.lines1[index1] == self.lines2[index2] {
          self.builder.mark_equal(index1, index2);
        }
      }
    }
  }
}

/// The combination of `sub_lines2` for `sub_lines1` with the most equal pairs
fn get_best_matching_alignment(sub_lines1: &[usize], sub_lines2: &[usize], lines1: &[Line<'_>], lines2: &[Line<'_>]) -> Vec<usize> {
  debug_assert!(sub_lines1.len() < sub_lines2.len());
  let size = sub_lines1.len();

  let mut search = BestMatching {
    sub_lines1,
    sub_lines2,
    lines1,
    lines2,
    comb: vec![0; size],
    best: (0..size).collect(),
    best_weight: 0,
  };
  // find a combination with maximum weight (maximum number of equal lines)
  search.combinations(0, sub_lines2.len() - 1, 0);
  search.best
}

struct BestMatching<'a, 'b> {
  sub_lines1: &'a [usize],
  sub_lines2: &'a [usize],
  lines1: &'a [Line<'b>],
  lines2: &'a [Line<'b>],
  comb: Vec<usize>,
  best: Vec<usize>,
  best_weight: usize,
}

impl BestMatching<'_, '_> {
  fn combinations(&mut self, start: usize, n: usize, k: usize) {
    if k == self.sub_lines1.len() {
      self.process_combination();
      return;
    }

    for i in start..=n {
      self.comb[k] = i;
      self.combinations(i + 1, n, k + 1);
    }
  }

  fn process_combination(&mut self) {
    let weight = (0..self.sub_lines1.len())
      .filter(|&i| self.lines1[self.sub_lines1[i]] == self.lines2[self.sub_lines2[self.comb[i]]])
      .count();

    if weight > self.best_weight {
      self.best_weight = weight;
      self.best.copy_from_slice(&self.comb);
    }
  }
}

/// Compares lines in two steps:
///  - compare ignoring "unimportant" lines
///  - correct changes (compare all lines gaps between matched chunks)
fn compare_smart(lines1: &[Line<'_>], lines2: &[Line<'_>], deadline: &Deadline) -> Result<DiffIterable, DiffTooBig> {
  let threshold = unimportant_line_char_count();
  if threshold == 0 {
    return diff(lines1, lines2, deadline);
  }

  let (big_lines1, indexes1) = get_big_lines(lines1, threshold);
  let (big_lines2, indexes2) = get_big_lines(lines2, threshold);

  let changes = diff(&big_lines1, &big_lines2, deadline)?;
  SmartLineChangeCorrector {
    indexes1: &indexes1,
    indexes2: &indexes2,
    lines1,
    lines2,
    deadline,
  }
  .build(&changes)
}

fn get_big_lines<'a>(lines: &[Line<'a>], threshold: usize) -> (Vec<Line<'a>>, Vec<usize>) {
  lines
    .iter()
    .enumerate()
    .filter(|(_, line)| line.non_space_chars > threshold)
    .map(|(index, line)| (*line, index))
    .unzip()
}

fn expand_ranges(lines1: &[Line<'_>], lines2: &[Line<'_>], iterable: &DiffIterable) -> DiffIterable {
  let changes = iterable
    .changes()
    .iter()
    .map(|ch| expand(lines1, lines2, ch.start1, ch.start2, ch.end1, ch.end2))
    .filter(|expanded| !expanded.is_empty())
    .collect();

  fair(create(changes, lines1.len(), lines2.len()))
}

//
// Lines
//

fn get_lines<'a>(text: &[&'a [u16]], policy: ComparisonPolicy) -> Vec<Line<'a>> {
  text.iter().map(|line| Line::new(line, policy)).collect()
}

fn convert_mode<'a>(original: &[Line<'a>], policy: ComparisonPolicy) -> Vec<Line<'a>> {
  original
    .iter()
    .map(|line| if line.policy == policy { *line } else { Line::new(line.content, policy) })
    .collect()
}

/// A line compared under a policy
#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
  content: &'a [u16],
  policy: ComparisonPolicy,
  hash: i32,
  non_space_chars: usize,
}

impl<'a> Line<'a> {
  pub fn new(content: &'a [u16], policy: ComparisonPolicy) -> Self {
    Self {
      content,
      policy,
      hash: hash_code(content, policy),
      non_space_chars: count_non_space_chars(content),
    }
  }

  pub const fn content(&self) -> &'a [u16] {
    self.content
  }

  /// The number of chars other than `\n`, `\t` and space
  pub const fn non_space_chars(&self) -> usize {
    self.non_space_chars
  }
}

impl PartialEq for Line<'_> {
  fn eq(&self, other: &Self) -> bool {
    debug_assert_eq!(self.policy, other.policy);

    if self.hash != other.hash {
      return false;
    }

    is_equals(self.content, other.content, self.policy)
  }
}

impl Eq for Line<'_> {}

impl Hash for Line<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    state.write_i32(self.hash);
  }
}

fn count_non_space_chars(text: &[u16]) -> usize {
  text.iter().filter(|&&c| !is_space_enter_or_tab(c)).count()
}
