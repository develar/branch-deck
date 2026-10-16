use crate::numstat::{NumstatEntry, parse_numstats};
use pretty_assertions::assert_eq;

#[test]
fn parses_log_and_diff_tree_numstats() {
  let entry = |path: &str, old_path: Option<&str>, additions: Option<u32>, deletions: Option<u32>| NumstatEntry {
    path: path.to_string(),
    old_path: old_path.map(str::to_string),
    additions,
    deletions,
  };

  let log = b"\x1eaaaa\0\n-\t-\tbin\0\x31\t0\t\0old\0new name\0\x1ebbbb\0\n\x1ecccc\0\n2\t3\tf\0";
  let parsed = parse_numstats(log);
  assert_eq!(parsed["aaaa"], [entry("bin", None, None, None), entry("new name", Some("old"), Some(1), Some(0))]);
  assert_eq!(parsed["bbbb"], []);
  assert_eq!(parsed["cccc"], [entry("f", None, Some(2), Some(3))]);

  let diff_tree = b"aaaa\x002\t0\tg\0bbbb\x000\t1\t\0x\0y\0";
  let parsed = parse_numstats(diff_tree);
  assert_eq!(parsed["aaaa"], [entry("g", None, Some(2), Some(0))]);
  assert_eq!(parsed["bbbb"], [entry("y", Some("x"), Some(0), Some(1))]);
}
