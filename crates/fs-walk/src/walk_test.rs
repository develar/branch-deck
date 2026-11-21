use crate::{PartBytes, TreeBytes, Walk, dir_listing, tree_size, walk_bytes, walk_tree, walk_tree_with_callback};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

/// A tree of `count` files of `size` bytes each under `root`, in nested directories
fn tree(root: &Path, count: usize, size: usize) {
  for n in 0..count {
    let dir = root.join(format!("{}/{}", n % 7, n % 3));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{n}.bin")), vec![u8::try_from(n % 251).unwrap(); size]).unwrap();
  }
}

/// The bytes of the parts of `tree`
fn parts(tree: &TreeBytes) -> u64 {
  tree.nodes.iter().chain(&tree.dirs).map(|part| part.bytes).sum()
}

#[test]
fn a_walk_counts_each_root_side_by_side_skips_what_counts_apart_and_a_hard_link_its_share() {
  let dir = tempfile::tempdir().unwrap();
  let roots: Vec<_> = (0..3).map(|n| dir.path().join(n.to_string())).collect();
  for (n, root) in roots.iter().enumerate() {
    tree(root, 300, 4096 * (n + 1));
  }
  let walks: Vec<Walk> = roots.iter().map(Walk::of).collect();
  let alone = walk_bytes(&walks);
  assert!(alone[0] >= 300 * 4096 && alone[1] >= 300 * 2 * 4096 && alone[2] >= 300 * 3 * 4096, "{alone:?}");
  let skipping = Walk {
    root: roots[0].clone(),
    skip: vec![roots[0].join("0")],
  };
  let [skipped] = walk_bytes(&[skipping])[..] else { panic!() };
  assert!(skipped < alone[0] && skipped > 0, "{skipped} of {}", alone[0]);
  let file = walk_bytes(&[Walk::of(roots[2].join("0/0/0.bin")), Walk::of(dir.path().join("gone"))]);
  assert!(file[0] >= 3 * 4096 && file[1] == 0, "{file:?}");
  // a second link inside the root: the file counts once in the root, half at each link
  #[cfg(unix)]
  {
    let file = walk_bytes(&[Walk::of(roots[0].join("0/0/0.bin"))])[0];
    std::fs::hard_link(roots[0].join("0/0/0.bin"), roots[0].join("1/linked")).unwrap();
    let after = walk_bytes(&walks);
    assert_eq!(after, alone);
    let [linked, original] = walk_bytes(&[Walk::of(roots[0].join("1/linked")), Walk::of(roots[0].join("0/0/0.bin"))])[..] else {
      panic!()
    };
    assert_eq!((linked, original), (file / 2, file / 2));
    // a link to a file outside every root: the root keeps its share
    std::fs::hard_link(roots[1].join("0/0/0.bin"), dir.path().join("outside")).unwrap();
    assert!(walk_bytes(&walks)[1] < alone[1]);
  }
}

#[test]
fn tree_size_uses_the_walk_and_counts_directory_bytes() {
  let dir = tempfile::tempdir().unwrap();
  tree(dir.path(), 30, 4096);
  let walked = walk_bytes(&[Walk::of(dir.path())])[0];

  assert!(tree_size(dir.path()) >= walked);
  assert_eq!(tree_size(&dir.path().join("gone")), 0);
  assert_eq!(tree_size(&dir.path().join("0/0/0.bin")), walk_bytes(&[Walk::of(dir.path().join("0/0/0.bin"))])[0]);
}

/// A root of 2 files and a big subtree `big` (2 directories of 3 files each) beside a small one (`small`, 1 file): with
/// at most 4 entries a node, `big` is above the frontier, its two directories and `small` are nodes, and the root and
/// `big` keep the bytes of their own files
#[test]
fn the_parts_of_a_tree_are_the_subtrees_of_at_most_the_node_entries_and_the_directories_above_them() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  for sub in ["big/a", "big/b", "small"] {
    std::fs::create_dir_all(root.join(sub)).unwrap();
  }
  std::fs::write(root.join("root.bin"), vec![1u8; 8192]).unwrap();
  std::fs::write(root.join("big/own.bin"), vec![1u8; 4096]).unwrap();
  for sub in ["big/a", "big/b"] {
    for n in 0..3 {
      std::fs::write(root.join(sub).join(format!("{n}.bin")), vec![2u8; 4096]).unwrap();
    }
  }
  std::fs::write(root.join("small/small.bin"), vec![3u8; 4096]).unwrap();
  let [walked] = walk_bytes(&[Walk::of(root)])[..] else { panic!() };
  let bytes = |path: &str| walk_bytes(&[Walk::of(root.join(path))])[0];

  let [tree] = &walk_tree(&[Walk::of(root)], 4)[..] else { panic!() };
  let mut nodes: Vec<&str> = tree.nodes.iter().map(|node| node.path.as_str()).collect();
  nodes.sort_unstable();
  let mut dirs: Vec<(&str, u64)> = tree.dirs.iter().map(|dir| (dir.path.as_str(), dir.bytes)).collect();
  dirs.sort_unstable();

  assert_eq!(tree.bytes, walked);
  assert_eq!(parts(tree), walked);
  assert_eq!(nodes, ["big/a", "big/b", "small"]);
  assert_eq!(dirs, [("", bytes("root.bin")), ("big", bytes("big/own.bin"))]);
  assert_eq!(tree.nodes.iter().find(|node| node.path == "big/a").unwrap().bytes, bytes("big/a"));
  // a tree within the limit is one node
  let [whole] = &walk_tree(&[Walk::of(root)], 1_000)[..] else { panic!() };
  assert_eq!(
    whole.nodes,
    [PartBytes {
      path: String::new(),
      bytes: walked
    }]
  );
  assert_eq!(whole.dirs, [] as [PartBytes; 0]);
  // the listing of a directory above the frontier: its own files and its directories
  let listing = dir_listing(&root.join("big")).unwrap();
  let mut names: Vec<String> = listing.dirs.iter().map(|name| name.to_string_lossy().into_owned()).collect();
  names.sort_unstable();
  assert_eq!((listing.bytes, names), (bytes("big/own.bin"), vec!["a".to_owned(), "b".to_owned()]));
}

#[test]
fn each_walk_is_told_once_as_it_ends_and_a_stop_names_no_bytes_of_an_unfinished_one() {
  let dir = tempfile::tempdir().unwrap();
  let roots: Vec<_> = (0..4).map(|n| dir.path().join(n.to_string())).collect();
  for root in &roots {
    tree(root, 60, 1024);
  }
  let walks: Vec<Walk> = roots.iter().map(Walk::of).collect();
  let order = Mutex::new(Vec::new());
  let running = AtomicBool::new(false);
  let trees = walk_tree_with_callback(&walks, 10, &running, |index, tree| order.lock().unwrap().push((index, tree.bytes)));
  let mut order = order.into_inner().unwrap();
  order.sort_unstable();

  assert_eq!(order, walk_bytes(&walks).into_iter().enumerate().collect::<Vec<_>>());
  assert!(trees.iter().all(Option::is_some));
  // stopped before the first directory: no walk of a directory ends, a walk of a missing root does
  let stopped = AtomicBool::new(true);
  let calls = Mutex::new(Vec::new());
  let mut with_gone = walks;
  with_gone.push(Walk::of(dir.path().join("gone")));
  let trees = walk_tree_with_callback(&with_gone, 10, &stopped, |index, _| calls.lock().unwrap().push(index));
  assert_eq!(trees.iter().map(Option::is_some).collect::<Vec<_>>(), [false, false, false, false, true]);
  assert_eq!(calls.into_inner().unwrap(), [4]);
}
