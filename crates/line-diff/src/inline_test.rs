//! The shape of changed blocks: the thresholds at their bounds.

use crate::inline::{LineRanges, inline_pairs, ranges_dropped};

fn line(length: usize, ranges: &[(usize, usize)]) -> LineRanges {
  LineRanges { length, ranges: ranges.to_vec() }
}

#[test]
fn a_pair_is_similar_below_a_third_changed() {
  // 2 of 6 chars changed is a third: not similar
  assert!(!inline_pairs(&[line(3, &[(0, 1)])], &[line(3, &[(0, 1)])]));
  // 2 of 8 chars changed: similar
  assert!(inline_pairs(&[line(4, &[(0, 1)])], &[line(4, &[(0, 1)])]));
}

#[test]
fn a_pair_with_more_than_four_ranges_is_not_similar() {
  let many = [(0, 1), (2, 3), (4, 5), (6, 7), (8, 9)];
  assert!(!inline_pairs(&[line(40, &many)], &[line(40, &[(0, 1)])]));
  assert!(inline_pairs(&[line(40, &many[..4])], &[line(40, &[(0, 1)])]));
}

#[test]
fn ranges_are_dropped_from_two_thirds_changed() {
  assert!(ranges_dropped(&[line(3, &[(0, 2)])], &[line(3, &[(0, 2)])]));
  assert!(!ranges_dropped(&[line(3, &[(0, 2)])], &[line(3, &[(0, 1)])]));
  assert!(!ranges_dropped(&[line(0, &[])], &[]));
}

#[test]
fn only_n_to_n_blocks_have_inline_pairs() {
  assert!(!inline_pairs(&[line(10, &[(0, 1)])], &[line(10, &[(0, 1)]), line(10, &[])]));
}

#[test]
fn empty_lines_pair_up() {
  assert!(inline_pairs(&[line(0, &[])], &[line(0, &[])]));
}
