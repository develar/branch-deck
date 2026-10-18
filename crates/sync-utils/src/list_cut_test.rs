use crate::list_cut::kept;

#[test]
fn a_list_one_longer_than_its_limit_is_kept_whole_and_a_longer_one_is_cut_at_the_limit() {
  assert_eq!(kept(0, 3), 0);
  assert_eq!(kept(3, 3), 3);
  // "and 1 more" never stands where the item would
  assert_eq!(kept(4, 3), 4);
  assert_eq!(kept(5, 3), 3);
  assert_eq!(kept(101, 100), 101);
  assert_eq!(kept(150, 100), 100);
  assert_eq!(kept(usize::MAX, usize::MAX), usize::MAX);
}
